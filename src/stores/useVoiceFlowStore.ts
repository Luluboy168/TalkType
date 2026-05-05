// Voice flow store — orchestrates the HUD's hotkey → record → transcribe →
// paste pipeline (M4 chunk 3, evolved in M5 chunk 1).
//
// **Scope (M4 chunk 3 + M5 chunk 1)**:
//   * Hold-mode: hotkey down → start record / hotkey up → stop + transcribe + paste
//   * Toggle-mode: each press XORs the recording state (driven by Rust side)
//   * ESC during recording → cancel without paste
//   * `audio:recording-aborted` → surface user-facing error (max_size / mic_unplug)
//   * `dismissError()` → user-driven recovery (HUD click-to-dismiss in M5 chunk 2)
//   * Cross-window: emit `voice-flow:state-changed` on every transition
//     (HUD → Dashboard sidebar; explicit `emitTo("main-window", ...)` per P0-1)
//
// **Out of scope** (deferred to later milestones — do NOT add here):
//   * HUD visual mode wiring (M5 chunk 2 owns `HudOverlay.vue` 4 visual states)
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
//   error ──user click / dismissError()──▶ idle (M5 chunk 2 wires UI)
//   * → error (audio:recording-aborted) when Rust auto-aborts (size cap / mic unplug)
//
// **Concurrency**: every transition goes through `transitionTo` so listeners
// see a consistent snapshot. The Rust side already has its own
// `transcribe_busy` AtomicBool guard (see `transcription/mod.rs`) so a
// double-press during transcribing returns `Busy` and lands in the error
// branch — no extra debounce needed here.
import { invoke } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { readonly, ref } from "vue";

import {
  AUDIO_RECORDING_ABORTED,
  ESCAPE_PRESSED,
  HOTKEY_PRESSED,
  HOTKEY_RELEASED,
  HOTKEY_TOGGLED,
  listenToEvent,
  VOICE_FLOW_STATE_CHANGED,
} from "@/composables/useTauriEvents";
import type {
  HotkeyEventPayload,
  RecordingAbortedPayload,
  TranscriptionResult,
  VoiceFlowStateChangedPayload,
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
   *   1. position_hud_for_active_monitor (M5 chunk 0; best-effort, P1-8)
   *   2. capture_target_window
   *   3. start_recording (deviceName: null = cpal default)
   *   4. transition to "recording"
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
   *   * **P1-8 (M5 chunk 1)**: HUD positioning is decorative — wrap in
   *     try/catch so a positioning failure does NOT block the core
   *     recording flow. Worst case: HUD stays at last position.
   */
  async function handleStart(): Promise<void> {
    if (status.value === "recording") return;
    const mySession = ++currentSession;
    try {
      // P1-8: positioning failure must NOT block recording (decorative vs core)
      try {
        await invoke<void>("position_hud_for_active_monitor");
      } catch (err) {
        console.warn("[voice-flow] HUD positioning failed, using fallback", err);
      }
      await invoke<void>("capture_target_window");
      await invoke<void>("start_recording", { deviceName: null });
      // If a newer handleStart raced ahead while we awaited Tauri commands,
      // bail without writing status — the newer session owns it.
      if (mySession !== currentSession) return;
      recordingStartedAtMs.value = Date.now();
      await transitionTo("recording", "");
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
    await transitionTo("transcribing", "");
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
        await transitionTo("success", "");
        window.setTimeout(() => {
          if (status.value === "success" && mySession === currentSession) {
            // Best-effort emit; we're in a setTimeout callback so void it.
            void transitionTo("idle", "");
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
    await transitionTo("idle", "");
  }

  /**
   * User-driven error dismissal (M5 chunk 1; HUD click-to-dismiss wired in
   * M5 chunk 2 via `HudOverlay.vue`). Idempotent: only flips to idle when
   * status is currently `"error"` so a stale dismiss click during a fresh
   * recording can't clobber the new state.
   *
   * Why `void` return rather than `async`: callers (template `@click`
   * handlers in chunk 2) don't care about the emit completion; we don't
   * want to chain awaits through the click handler. The internal
   * `transitionTo` is awaited via `.catch()` on its own promise to satisfy
   * the lint without blocking the caller — but since `transitionTo`'s
   * catch is internal already, a bare `void transitionTo(...)` is fine.
   */
  function dismissError(): void {
    if (status.value === "error") {
      void transitionTo("idle", "");
    }
  }

  /** Convert any thrown value to a user-facing error state. Auto-reverts to
   * idle after `ERROR_LINGER_MS` so the user can retry without manual
   * dismissal. Idempotent — only flips back if the state is still "error"
   * (a fresh hotkey press would have moved it forward already). */
  function handleError(err: unknown): void {
    // handleError is sync (called from sync setTimeout callbacks too) so we
    // don't await transitionTo; emit is best-effort anyway.
    void transitionTo("error", formatError(err));
    window.setTimeout(() => {
      if (status.value === "error") void transitionTo("idle", "");
    }, ERROR_LINGER_MS);
  }

  /**
   * Single chokepoint for all state changes. Mutates the local refs and
   * also broadcasts a `voice-flow:state-changed` event to the Dashboard
   * window so its sidebar `HudFlowBadge` can mirror the live state (M5
   * chunk 3 wires the listen-side).
   *
   * **P0-1 fix (M5 chunk 1)**: use `emitTo("main-window", ...)` rather
   * than `emit(...)` so the event is delivered explicitly to Dashboard
   * (label `"main-window"`) and never echoed back to the HUD itself —
   * defends against future HUD listeners forming an echo loop. The
   * payload also carries `source: "hud"` so listeners that DO add a self-
   * filter can reject their own emits.
   *
   * The emit is best-effort: `try/catch` around it so a transient cross-
   * window IPC failure doesn't break the local state machine.
   */
  async function transitionTo(
    next: VoiceFlowStatus,
    msg: string,
  ): Promise<void> {
    status.value = next;
    message.value = msg;
    try {
      const payload: VoiceFlowStateChangedPayload = {
        status: next,
        message: msg,
        source: "hud",
      };
      await emitTo("main-window", VOICE_FLOW_STATE_CHANGED, payload);
    } catch (err) {
      console.warn("[voice-flow] cross-window emit failed", err);
    }
  }

  /** Best-effort error normalization. Rust commands serialize their
   * `thiserror` enums as flat strings (per CLAUDE.md "Error enum 手動
   * implement Serialize 為 string"), so the typical input is already a
   * string. We still handle the `Error` instance + arbitrary object
   * shapes defensively.
   *
   * **P0-3 fix (M5 chunk 1)**: pattern-match the Rust
   * `ClipboardError::FocusRestoreFailed` Display string to preserve the
   * friendly "請手動 Ctrl+V" hint that was previously appended by the
   * (now-removed) `paste:focus-restore-failed` listener (see Task 1.2).
   * This keeps the user-facing UX while consolidating error surface to
   * a single code path.
   */
  function formatError(err: unknown): string {
    const raw = (() => {
      if (typeof err === "string") return err;
      if (err instanceof Error) return err.message;
      if (err && typeof err === "object" && "message" in err) {
        const msg = (err as { message: unknown }).message;
        if (typeof msg === "string") return msg;
      }
      return "Unknown error";
    })();

    // P0-3: preserve M4's friendly "請手動 Ctrl+V" hint that was previously
    // appended by the (now-removed) paste:focus-restore-failed listener.
    // Pattern match Rust `ClipboardError::FocusRestoreFailed` — actual Display
    // string is `"Failed to restore focus to target HWND {hwnd:#x}: GetLastError={last_error}"`
    // (see src-tauri/src/plugins/clipboard_paste/mod.rs:96). Chunk 1 reviewer
    // (P0) caught that the original spec/plan pattern `"Focus restore failed"`
    // never matched the real Rust output.
    if (
      raw.startsWith("Failed to restore focus") ||
      raw.startsWith("Focus restore failed") ||
      raw.includes("FocusRestoreFailed")
    ) {
      return `${raw}（請手動 Ctrl+V）`;
    }

    return raw;
  }

  /**
   * Wire Tauri event listeners for the hotkey + ESC + recording-abort events.
   * Returns a cleanup function the caller MUST call on unmount to avoid
   * leaked listeners. Typical usage from the HUD entry point:
   *
   * ```ts
   * const store = useVoiceFlowStore();
   * const cleanup = await store.init();
   * onUnmounted(cleanup);
   * ```
   *
   * **Async refactor (M5 chunk 1)**: replaced the prior `void
   * listenToEvent(...).then(push)` fire-and-forget pattern with sequential
   * `await listenToEvent(...)` so callers can rely on a guarantee that the
   * returned cleanup fn unhooks every listener that was registered — no
   * race window where some `.then(push)` is still pending.
   *
   * Tauri's `listen()` registers atomically and the event queue starts
   * delivery only after registration completes, so events fired *during*
   * the await sequence are not lost (just delivered after the listener
   * is in place).
   *
   * **Idempotency**: calling `init` twice would register duplicate
   * listeners. The HUD entry point only calls it once at module bootstrap
   * so we don't bother with internal guards — keep `init` callable from
   * tests too without side effects beyond the listener registration.
   */
  async function init(): Promise<() => void> {
    const unlistenFns: Array<() => void> = [];

    // Hold mode — Rust dispatches pressed on key down, released on key up.
    unlistenFns.push(
      await listenToEvent<HotkeyEventPayload>(HOTKEY_PRESSED, () => {
        void handleStart();
      }),
    );

    unlistenFns.push(
      await listenToEvent<HotkeyEventPayload>(HOTKEY_RELEASED, () => {
        void handleStop();
      }),
    );

    // Toggle mode — Rust XORs `is_toggled_on` and emits one event per tap.
    unlistenFns.push(
      await listenToEvent<HotkeyEventPayload>(HOTKEY_TOGGLED, (event) => {
        const action = event.payload.action;
        if (action === "toggled-on") void handleStart();
        else if (action === "toggled-off") void handleStop();
      }),
    );

    // ESC during recording → cancel.
    unlistenFns.push(
      await listenToEvent<void>(ESCAPE_PRESSED, () => {
        void handleCancel();
      }),
    );

    // REMOVED in M5 chunk 1: paste:focus-restore-failed listener. The
    // listener was redundant — `handleStop`'s catch already routes the
    // FocusRestoreFailed Rust error through `handleError`. Keeping both
    // caused double-transition (M4 chunk 3 reviewer P2 flagged in
    // .claude/IDEAS.md). Friendly "請手動 Ctrl+V" hint is now appended by
    // `formatError` when it sees the Rust error pattern (Task 1.2.5
    // P0-3 fix). Rust side continues to emit the event for future
    // Dashboard tooltip use; the HUD just no longer subscribes.

    // M3 chunk 0 cap event — Rust auto-aborts when the WAV buffer hits
    // ~25 MB (~13 min @ 16 kHz mono) or when cpal detects mic
    // disconnection. We surface a user-facing zh-TW message via the
    // existing `handleError` path so the auto-revert + state collision
    // logic is reused (no duplicate timer).
    //
    // i18n: hardcoded zh-TW for M5 chunk 1; chunk 2 may convert to i18n
    // keys when HUD components introduce the i18n bundle.
    unlistenFns.push(
      await listenToEvent<RecordingAbortedPayload>(
        AUDIO_RECORDING_ABORTED,
        (event) => {
          const reasonMsg =
            event.payload.reason === "max_size"
              ? "錄音超過上限（~13 分鐘 @ 16 kHz）"
              : "麥克風已拔除";
          handleError(new Error(reasonMsg));
        },
      ),
    );

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
    // Init returns the cleanup fn (now Promise-wrapped — await it!) — the
    // HUD entry calls this at bootstrap and stashes cleanup on `window`.
    init,
    // User-driven recovery; HUD click handler in M5 chunk 2 calls this.
    dismissError,
    // Exposed for unit tests; main session can grep these as the test-only
    // surface area to avoid accidental production usage.
    handleStart,
    handleStop,
    handleCancel,
  };
});
