// Voice flow store — orchestrates the HUD's hotkey → record → transcribe →
// paste pipeline (M4 chunk 3).
//
// **Scope (M4 chunk 3 only)**:
//   * Hold-mode: hotkey down → start record / hotkey up → stop + transcribe + paste
//   * Toggle-mode: each press XORs the recording state (driven by Rust side)
//   * ESC during recording → cancel without paste
//   * `paste:focus-restore-failed` → surface friendly "請手動 Ctrl+V" hint
//
// **Out of scope** (deferred to later milestones — do NOT add here):
//   * HUD visual mode wiring (M5 owns `HudOverlay.vue` 4 visual states)
//   * Audio mute/restore on recording (M5 — `mute_on_recording` setting)
//   * Sound effects (Phase 2)
//   * LLM polish (M6 — `polish_text` command)
//   * History persist (M8 — `add_history` SQLite command)
//   * Vocabulary read-through (M8 — `get_vocabulary`)
//
// **State machine** (mirrors `doc/plans/01-architecture.md` §Voice Flow):
//   idle ──hotkey:pressed/toggled-on──▶ recording
//                                  ╲
//                                   ╲──hotkey:released/toggled-off──▶ transcribing
//                                                                  ╲
//                                                                   ╲──ok──▶ success ──1s──▶ idle
//                                                                    ╲
//                                                                     ╲──err──▶ error ──3s──▶ idle
//   recording ──escape:pressed──▶ idle (cancel; clears WAV buffer)
//
// **Concurrency**: every transition goes through `transitionTo` so listeners
// see a consistent snapshot. The Rust side already has its own
// `transcribe_busy` AtomicBool guard (see `transcription/mod.rs`) so a
// double-press during transcribing returns `Busy` and lands in the error
// branch — no extra debounce needed here.
import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { readonly, ref } from "vue";

import {
  ESCAPE_PRESSED,
  HOTKEY_PRESSED,
  HOTKEY_RELEASED,
  HOTKEY_TOGGLED,
  listenToEvent,
  PASTE_FOCUS_RESTORE_FAILED,
} from "@/composables/useTauriEvents";
import type {
  HotkeyEventPayload,
  PasteFocusRestoreFailedPayload,
  TranscriptionResult,
} from "@/types/events";

/**
 * High-level voice-flow status. Renamed from SayIt's `HudStatus` because the
 * HUD visual binding is M5's job — this store just owns the logical state.
 *
 * `enhancing` (M6 LLM polish), `cancelled` (Phase 2 nuance), and other
 * states will be added when their owning milestone lands.
 */
export type VoiceFlowStatus =
  | "idle"
  | "recording"
  | "transcribing"
  | "success"
  | "error";

/** How long the `success` state stays visible before transitioning back to idle. */
const SUCCESS_LINGER_MS = 1000;
/** How long the `error` state stays visible before transitioning back to idle. */
const ERROR_LINGER_MS = 3000;

/**
 * Module-level paste serialization (M4 acceptance fix). Each `paste_text`
 * invocation appends to this Promise chain so concurrent transcribe
 * completions can't trample the OS clipboard. Without this lock, two
 * `paste_text` calls running close together would both spawn_blocking,
 * both call `arboard::Clipboard::set_text`, and the later set would
 * clobber the earlier one BEFORE its `SendInput Ctrl+V` fires — net
 * result: only the last text gets pasted, twice.
 *
 * Errors are swallowed off the chain so a single failed paste doesn't
 * poison subsequent ones.
 */
let pasteChain: Promise<void> = Promise.resolve();

async function pasteTextSerial(text: string): Promise<void> {
  const previous = pasteChain;
  pasteChain = (async () => {
    try {
      await previous;
    } catch {
      // ignore upstream paste failure — they handle their own error path
    }
    await invoke<void>("paste_text", { text });
  })();
  await pasteChain;
}

export const useVoiceFlowStore = defineStore("voiceFlow", () => {
  /** Logical voice-flow state. Read-only outside the store; mutated only via
   * `transitionTo`. */
  const status = ref<VoiceFlowStatus>("idle");
  /** User-facing message accompanying the state — primarily error text. */
  const message = ref<string>("");
  /** Wall-clock ms-since-epoch when the current recording began. `null` when
   * not recording. M5's HUD uses this to render the running timer; we
   * surface it here as raw ms so the rendering layer can choose its own
   * tick rate (RAF, setInterval, …). */
  const recordingStartedAtMs = ref<number | null>(null);

  /**
   * Monotonic counter incremented on each `handleStart`. Each `handleStop`
   * captures the active session and only writes terminal transitions
   * (success / idle / error) if its session is still current. This stops
   * a stale handleStop (e.g. transcribe 1 finishing) from clobbering a
   * fresh handleStart's "recording" status when the user does rapid press
   * during transcribing.
   *
   * Module-private: not exposed in the store's public surface.
   */
  let currentSession = 0;

  /** Hotkey-down → start the voice flow. Captures the foreground HWND FIRST
   * so the later paste knows which window to send Ctrl+V to. The capture
   * has to happen before we change focus — the HUD has `WS_EX_NOACTIVATE`
   * (M4 chunk 2) so this should be a no-op for focus, but we want
   * `GetForegroundWindow` to fire while the user's target is still on top.
   *
   * Phase 1 path:
   *   1. capture_target_window
   *   2. start_recording (deviceName: null = cpal default)
   *   3. transition to "recording"
   *
   * **Guard policy** (M4 acceptance fix v2): allow start from any state
   * EXCEPT `recording` (already capturing — would conflict with
   * audio_recorder's single-recording invariant). The earlier conservative
   * "block transcribing too" guard was too strict — it dropped fresh
   * presses during the 0.5-2s Groq round trip, which made rapid voice
   * typing feel broken. Now: a fresh press during transcribing starts a
   * new recording in parallel; the old transcribe + paste finish in the
   * background and chain through `pasteTextSerial`.
   *
   * Cross-cutting concerns:
   *   * Rust transcribe_busy guard was REMOVED — parallel transcribes are
   *     allowed; WAV buffer is consumed at each transcribe's start so
   *     there's no buffer race.
   *   * Frontend `pasteTextSerial` queues paste_text invocations so two
   *     simultaneous completions don't fight for the OS clipboard.
   *   * `currentSession` counter prevents stale handleStop's terminal
   *     transitions from clobbering the active recording's status.
   */
  async function handleStart(): Promise<void> {
    if (status.value === "recording") return;
    const mySession = ++currentSession;
    try {
      await invoke<void>("capture_target_window");
      await invoke<void>("start_recording", { deviceName: null });
      // If a newer handleStart raced ahead while we awaited Tauri commands,
      // bail without writing status — the newer session owns it.
      if (mySession !== currentSession) return;
      recordingStartedAtMs.value = Date.now();
      transitionTo("recording", "");
    } catch (err) {
      if (mySession === currentSession) handleError(err);
    }
  }

  /** Hotkey-up → stop recording → transcribe → paste. Phase 1 path only:
   * raw Whisper text, no LLM polish (M6 owns), no history persist (M8).
   *
   * Stop is **session-scoped**: if a newer handleStart has run, our
   * terminal transitions (transcribing/success/idle) skip so we don't
   * clobber the new recording's status. Transcribe + paste still run in
   * background so the user gets their audio transcribed + pasted; only
   * the visible state machine is gated. */
  async function handleStop(): Promise<void> {
    if (status.value !== "recording") return;
    const mySession = currentSession;
    transitionTo("transcribing", "");
    try {
      await invoke<void>("stop_recording");
      const result = await invoke<TranscriptionResult>("transcribe_audio", {
        // M4 chunk 3 ships without vocabulary — M8 wires `get_vocabulary` and
        // pipes top terms through here. Explicit `undefined` rather than `null`
        // so the Rust `Option<Vec<String>>` deserializes to `None`.
        vocabulary: undefined,
      });
      // Paste runs through the module-level chain so concurrent transcribe
      // completions don't trample the OS clipboard.
      await pasteTextSerial(result.rawText);
      // Only emit terminal transitions if WE are still the latest session —
      // otherwise the user has started a new recording and we'd clobber it.
      if (mySession === currentSession) {
        transitionTo("success", "");
        window.setTimeout(() => {
          if (status.value === "success" && mySession === currentSession) {
            transitionTo("idle", "");
          }
        }, SUCCESS_LINGER_MS);
      }
    } catch (err) {
      if (mySession === currentSession) handleError(err);
    } finally {
      // Only clear the timestamp if our session is still active —
      // otherwise we'd null out a newer recording's start time.
      if (mySession === currentSession) recordingStartedAtMs.value = null;
    }
  }

  /** ESC during recording → cancel without paste. Clears the recording
   * buffer to free RAM (M2 retro #3). The cleanup invokes are best-effort;
   * the cancel path always returns to idle even if `clear_recording_buffer`
   * errors (e.g. recorder already stopped) so the user can immediately
   * record again. */
  async function handleCancel(): Promise<void> {
    if (status.value !== "recording") return;
    try {
      await invoke<void>("stop_recording");
      await invoke<void>("clear_recording_buffer");
    } catch (err) {
      // Cancel cleanup failure is non-fatal — log but always reset state.
      console.warn("[voice-flow] cancel cleanup failed", err);
    }
    recordingStartedAtMs.value = null;
    transitionTo("idle", "");
  }

  /** Convert any thrown value to a user-facing error state. Auto-reverts to
   * idle after `ERROR_LINGER_MS` so the user can retry without manual
   * dismissal. Idempotent — only flips back if the state is still "error"
   * (a fresh hotkey press would have moved it forward already). */
  function handleError(err: unknown): void {
    transitionTo("error", formatError(err));
    window.setTimeout(() => {
      if (status.value === "error") transitionTo("idle", "");
    }, ERROR_LINGER_MS);
  }

  /** Single chokepoint for all state changes. M5's HUD will subscribe to
   * `status` directly via Pinia reactivity; the cross-window broadcast
   * (`voice-flow:state-changed` event for Dashboard sidebar indicator) is
   * intentionally deferred to M5 so M4 chunk 3 stays scoped. */
  function transitionTo(next: VoiceFlowStatus, msg: string): void {
    status.value = next;
    message.value = msg;
  }

  /** Best-effort error normalization. Rust commands serialize their
   * `thiserror` enums as flat strings (per CLAUDE.md "Error enum 手動
   * implement Serialize 為 string"), so the typical input is already a
   * string. We still handle the `Error` instance + arbitrary object
   * shapes defensively. */
  function formatError(err: unknown): string {
    if (typeof err === "string") return err;
    if (err instanceof Error) return err.message;
    if (err && typeof err === "object" && "message" in err) {
      const msg = (err as { message: unknown }).message;
      if (typeof msg === "string") return msg;
    }
    return "Unknown error";
  }

  /**
   * Wire Tauri event listeners for the hotkey + ESC + paste-focus events.
   * Returns a cleanup function the caller MUST call on unmount to avoid
   * leaked listeners. Typical usage from the HUD entry point:
   *
   * ```ts
   * const store = useVoiceFlowStore();
   * const cleanup = store.init();
   * onUnmounted(cleanup);
   * ```
   *
   * **Idempotency**: calling `init` twice would register duplicate
   * listeners. The HUD entry point only calls it once at module bootstrap
   * so we don't bother with internal guards — keep `init` callable from
   * tests too without side effects beyond the listener registration.
   */
  function init(): () => void {
    const unlistenFns: Array<() => void> = [];

    // Hold mode — Rust dispatches pressed on key down, released on key up.
    void listenToEvent<HotkeyEventPayload>(HOTKEY_PRESSED, () => {
      void handleStart();
    }).then((unlisten) => unlistenFns.push(unlisten));

    void listenToEvent<HotkeyEventPayload>(HOTKEY_RELEASED, () => {
      void handleStop();
    }).then((unlisten) => unlistenFns.push(unlisten));

    // Toggle mode — Rust XORs `is_toggled_on` and emits one event per tap.
    void listenToEvent<HotkeyEventPayload>(HOTKEY_TOGGLED, (event) => {
      const action = event.payload.action;
      if (action === "toggled-on") void handleStart();
      else if (action === "toggled-off") void handleStop();
    }).then((unlisten) => unlistenFns.push(unlisten));

    // ESC during recording → cancel.
    void listenToEvent<void>(ESCAPE_PRESSED, () => {
      void handleCancel();
    }).then((unlisten) => unlistenFns.push(unlisten));

    // Paste pipeline could not restore focus to the user's target window
    // (Windows 11 anti-flash policy). Text is on the clipboard already;
    // surface a friendly hint so the user knows to manually Ctrl+V. M5
    // will render this in the HUD with a dedicated visual state.
    void listenToEvent<PasteFocusRestoreFailedPayload>(
      PASTE_FOCUS_RESTORE_FAILED,
      (event) => {
        transitionTo(
          "error",
          `${event.payload.message}（請手動 Ctrl+V）`,
        );
        window.setTimeout(() => {
          if (status.value === "error") transitionTo("idle", "");
        }, ERROR_LINGER_MS);
      },
    ).then((unlisten) => unlistenFns.push(unlisten));

    return () => {
      for (const fn of unlistenFns) fn();
      unlistenFns.length = 0;
    };
  }

  return {
    // Read-only refs for consumers (HUD components, Dashboard sidebar).
    status: readonly(status),
    message: readonly(message),
    recordingStartedAtMs: readonly(recordingStartedAtMs),
    // Init returns the cleanup fn — the HUD entry calls this at bootstrap.
    init,
    // Exposed for unit tests; main session can grep these as the test-only
    // surface area to avoid accidental production usage.
    handleStart,
    handleStop,
    handleCancel,
  };
});
