// Voice flow store — orchestrates the HUD's hotkey → record → transcribe →
// (optional polish) → paste pipeline (M4 chunk 3, evolved in M5 chunk 1, M6
// chunk 2).
//
// **Scope (M4 chunk 3 + M5 chunk 1 + M6 chunk 2)**:
//   * Hold-mode: hotkey down → start record / hotkey up → stop + transcribe
//                                                          + (polish?) + paste
//   * Toggle-mode: each press XORs the recording state (driven by Rust side)
//   * ESC during recording → cancel without paste
//   * ESC during enhancing → no-op + console.warn (M6 limitation; the
//     reqwest cancel channel is deferred to v0.2 — F23)
//   * `audio:recording-aborted` → surface user-facing error (max_size / mic_unplug)
//   * `dismissError()` → user-driven recovery (HUD click-to-dismiss in M5 chunk 2)
//   * Cross-window: emit `voice-flow:state-changed` on every transition
//     (HUD → Dashboard sidebar; explicit `emitTo("main-window", ...)` per P0-1)
//   * **M6 polish branch (Decisions #5 / #7 / #8)**:
//     - F21 polishEnabledAtStart snapshot at handleStart time (settings
//       changes mid-flow do not affect the in-flight pipeline)
//     - F22 tri-state polish gate: `Some(true)` always polish, `Some(false)`
//       always skip, `None` auto-detect via `has_credential(provider)`
//     - F25 polishWarning ref lifecycle: set on fallback-to-raw, cleared on
//       next idle/recording transition (chunk-3 HUD reads it for amber-vs-
//       green success bubble)
//     - F34 retry orchestration: frontend invokes `polish_text` twice on
//       transient errors (network / 5xx / Busy) when retry is enabled
//       (`Settings.llmPolishRetryEnabled` — `None` and `Some(true)` mean ON,
//       `Some(false)` means OFF). Non-retryable variants (ApiKeyMissing /
//       SafetyBlocked / etc.) skip retry. retry attempts are sequential
//       (the second invoke awaits the first; the Rust BusyGuard makes this
//       safe even if the user double-presses during the gap).
//     - Decision #8 SUCCESS_LINGER_MS = 1500 ms (M5 1000 → M6 1500 to
//       give the user time to read the polish-failed warning).
//
// **Out of scope** (deferred to later milestones — do NOT add here):
//   * HUD visual mode wiring (M5 chunk 2 owns `HudOverlay.vue` 4 visual states;
//     M6 chunk 3 extends with `enhancing` + success-warning amber)
//   * Audio mute/restore on recording (M5 — `mute_on_recording` setting)
//   * Sound effects (Phase 2)
//   * History persist (M8 — `add_history` SQLite command)
//   * Vocabulary read-through (M8 — `get_vocabulary`)
//
// **State machine** (mirrors `doc/plans/01-architecture.md` §Voice Flow):
//   idle ──hotkey:pressed/toggled-on──▶ recording
//                                  ╲
//                                   ╲──hotkey:released/toggled-off──▶ transcribing
//                                                                  ╲
//                                                                   ╲──polish ON──▶ enhancing ──▶ success
//                                                                   ╲                            (1.5 s linger)
//                                                                    ╲──polish OFF──▶ success ──1.5s──▶ idle
//                                                                     ╲
//                                                                      ╲──err──▶ error ──3s──▶ idle
//   recording ──escape:pressed──▶ idle (cancel; clears WAV buffer)
//   enhancing ──escape:pressed──▶ no-op (M6 limitation — F23)
//   error ──user click / dismissError()──▶ idle (M5 chunk 2 wires UI)
//   * → error (audio:recording-aborted) when Rust auto-aborts (size cap / mic unplug)
//
// **Concurrency**: every transition goes through `transitionTo` so listeners
// see a consistent snapshot. The Rust side already has its own
// `transcribe_busy` AtomicBool guard (see `transcription/mod.rs`) so a
// double-press during transcribing returns `Busy` and lands in the error
// branch — no extra debounce needed here. Polish has its own
// `polish_busy` AtomicBool (see `llm_polish/mod.rs::BusyGuard`); a fresh
// hotkey press during enhancing makes the next polish attempt return
// `PolishError::Busy`, which our retry classifier treats as transient
// (folds to `polish_failure_with_warning`).
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
import type { PolishResult } from "@/types/llm";
import type { Settings } from "@/types/settings";

/**
 * High-level voice-flow status. Renamed from SayIt's `HudStatus` because the
 * HUD visual binding is M5's job — this store just owns the logical state.
 *
 * M6 chunk 0 adds `'enhancing'` (LLM polish in flight, reached only on the
 * polish-ON branch of `handleStop` once chunk 2 lands the polish path).
 * Chunk 0 only extends the type union — the actual `transitionTo('enhancing')`
 * call lives in chunk 2. The 5 hardcoded 5-state narrowing sites (events.ts,
 * main.ts, main-window.ts, this file, HudFlowBadge derived type) all updated
 * in lockstep per F1.
 *
 * `cancelled` (Phase 2 nuance) and other states will be added when their
 * owning milestone lands.
 */
export type VoiceFlowStatus =
  | "idle"
  | "recording"
  | "transcribing"
  | "enhancing"
  | "success"
  | "error";

/**
 * How long the `success` state stays visible before transitioning back to
 * idle. M5 was 1000 ms; M6 chunk 2 bumped to 1500 ms (Decision #8) — the
 * post-M5 retro flagged the linger as too short, and M6 introduced an extra
 * `enhancing` stage plus the polish-failed warning bubble that wants a beat
 * longer to be readable. Applied uniformly to all success paths (polish ON,
 * polish OFF, polish-failed-fallback) so the HUD timing stays predictable.
 */
const SUCCESS_LINGER_MS = 1500;
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

/**
 * F34 retry classifier — module-private helper for `runPolishWithRetry`.
 * Mirrors the Rust reference predicate `plugins::llm_polish::providers::
 * is_retryable` (which is intentionally kept `#[allow(dead_code)]` on the
 * Rust side because Decision #5 routes retry orchestration through the
 * frontend instead of the Rust orchestrator).
 *
 * **Wire format**: Rust serializes `PolishError` to a flat string via the
 * manual `Serialize` impl (see `error.rs::PolishError::serialize`). Each
 * variant emits its `#[error("...")]` `Display` output verbatim. The Tauri
 * `invoke()` rejection in JS is the resulting string.
 *
 * **Variant Display strings** (from `error.rs`):
 *   * Retryable (transient — caller should retry once when enabled):
 *     - Timeout(N)            → `"Request timed out after Ns"`
 *     - RateLimited           → `"Rate limited (retry after ...s)"`
 *     - NetworkOther(s)       → `"Other network error: ..."`
 *     - ConnectionRefused     → `"Connection refused by server"`
 *     - DnsFailure(s)         → `"DNS lookup failed: ..."`
 *     - TlsFailure(s)         → `"TLS handshake failed: ..."`
 *     - Offline               → `"Network appears offline"`
 *     - Busy                  → `"A previous polish request is still in progress"`
 *       (F24: a hotkey burst during enhancing is the typical Busy source —
 *        the rejected attempt should retry once the in-flight polish is done)
 *
 *   * Non-retryable (caller skips retry, falls back to raw immediately):
 *     - ApiKeyMissing         → `"API key missing for provider ..."`
 *     - Credentials(s)        → `"Credentials error: ..."`
 *     - Disabled              → `"LLM polish is disabled in settings"`
 *     - Cancelled             → `"Polish cancelled"`
 *     - EmptyInput            → `"Empty input — nothing to polish"`
 *     - InvalidPromptLength   → `"Custom prompt is too long ..."`
 *     - ApiError              → `"Provider returned error N: ..."`
 *       (4xx / non-429 — retrying won't change a malformed request; 5xx
 *        per Decision #5 is also non-retryable here because the retry-same
 *        strategy doesn't help recover from the server-side condition.
 *        Hindsight: M9 dogfood may flip 5xx to retryable if data shows it
 *        helps.)
 *     - EmptyResponse         → `"Provider returned an empty response"`
 *     - Truncated             → `"Polished output truncated ..."`
 *     - ImplausibleOutput     → `"Polished output is implausible ..."`
 *     - SafetyBlocked         → `"Provider blocked the response: ..."`
 *     - ParseError            → `"Failed to parse provider response: ..."`
 *
 * The match runs case-insensitive on `lower(message)` for resilience to
 * minor Rust wording shifts (the variant prefixes are unlikely to drift,
 * but a casing change in `#[error(...)]` shouldn't break retry policy).
 */
function isRetryablePolishError(err: unknown): boolean {
  // Tauri rejections come through as strings (manual Serialize impl on
  // PolishError) but defensively handle Error / object shapes too.
  const raw = (() => {
    if (typeof err === "string") return err;
    if (err instanceof Error) return err.message;
    if (err && typeof err === "object" && "message" in err) {
      const msg = (err as { message: unknown }).message;
      if (typeof msg === "string") return msg;
    }
    return String(err);
  })();
  const lower = raw.toLowerCase();

  // Non-retryable — match the start of Display strings from error.rs.
  // Listed first because it short-circuits early when the error is one
  // of the typical user-action-required variants.
  const nonRetryablePatterns: ReadonlyArray<string> = [
    "api key missing", // ApiKeyMissing
    "credentials error", // Credentials
    "llm polish is disabled", // Disabled
    "polish cancelled", // Cancelled
    "empty input", // EmptyInput
    "custom prompt is too long", // InvalidPromptLength
    "provider returned error", // ApiError (any non-429)
    "provider returned an empty response", // EmptyResponse
    "polished output truncated", // Truncated
    "polished output is implausible", // ImplausibleOutput
    "provider blocked the response", // SafetyBlocked
    "failed to parse provider response", // ParseError
  ];
  for (const pattern of nonRetryablePatterns) {
    if (lower.includes(pattern)) return false;
  }

  // If we got here and the message looks like one of the retryable
  // variants, retry. Anything else (unknown future variants, malformed
  // strings) defaults to NON-retryable to be conservative — better to
  // skip retry once than to thrash an unknown failure mode.
  const retryablePatterns: ReadonlyArray<string> = [
    "request timed out", // Timeout
    "rate limited", // RateLimited
    "other network error", // NetworkOther
    "connection refused", // ConnectionRefused
    "dns lookup failed", // DnsFailure
    "tls handshake failed", // TlsFailure
    "network appears offline", // Offline
    "previous polish request is still in progress", // Busy (F24)
  ];
  for (const pattern of retryablePatterns) {
    if (lower.includes(pattern)) return true;
  }
  return false;
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

  /**
   * F21 — polish-on/off snapshot at handleStart time. Captured per-session
   * (NOT per-store) so a settings flip mid-flow can't repurpose an in-flight
   * pipeline (e.g. user toggling polish OFF while transcribing finishes still
   * runs the polish branch the press originally intended). The snapshot is
   * resolved against the tri-state Decision #7 semantics:
   *
   *   * `Settings.llmPolishEnabled === true`     → always ON
   *   * `Settings.llmPolishEnabled === false`    → always OFF
   *   * `Settings.llmPolishEnabled === undefined`→ auto-detect via
   *                                                 `has_credential(provider)`
   *
   * Map keyed by session id so concurrent sessions (rapid press during
   * transcribing — see existing test) each have their own snapshot. The
   * older-session entry stays alive until its handleStop reads it; we
   * delete the entry there to prevent unbounded growth.
   *
   * Module-private; reset between test cases via `setActivePinia` rebuild.
   */
  const polishEnabledAtStart = new Map<number, boolean>();

  /**
   * F25 — polishWarning ref. Set to `true` by handleStop's polish branch
   * when the polish pipeline fell back to raw transcript paste (single
   * attempt failed and retry disabled, OR both attempts failed when retry
   * enabled, OR ApiKeyMissing on explicit Some(true)). Cleared when
   * `transitionTo` moves into 'idle' or 'recording' so a fresh recording
   * doesn't carry the previous session's warning into its success bubble.
   *
   * Chunk-3 HUD reads this to switch the success bubble between green
   * CheckCircle2 (warning=false) and amber AlertTriangle (warning=true).
   * Exposed as a readonly ref via the store's public surface.
   */
  const polishWarning = ref<boolean>(false);

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

      // F21: snapshot polish-gating at hotkey-press time. Decoupled from
      // the core recording flow so a settings read failure cannot prevent
      // recording — fail-closed (polish OFF). Settings change mid-flow do
      // not affect this in-flight pipeline. The map entry is consumed
      // (and removed) by the matching handleStop call.
      polishEnabledAtStart.set(
        mySession,
        await resolvePolishEnabledAtStart(),
      );

      // If a newer handleStart raced ahead while we awaited Tauri commands,
      // bail without writing status — the newer session owns it.
      if (mySession !== currentSession) return;
      recordingStartedAtMs.value = Date.now();
      await transitionTo("recording", "");
    } catch (err) {
      if (mySession === currentSession) handleError(err);
    }
  }

  /**
   * F21 + F22 polish gating resolver. Fetches the settings snapshot via
   * `get_settings` and resolves the tri-state per Decision #7. Wrapped in
   * try/catch so any IPC failure (Settings store unavailable / corrupted
   * JSON / lock poisoned) folds to "polish OFF" — fail-closed protects the
   * core recording flow from settings-layer regressions.
   *
   * Auto-detect path (`Settings.llmPolishEnabled === undefined`) issues
   * `has_credential(provider)` which is the only credentials command that
   * doesn't expose secrets to the frontend (returns boolean only). The
   * provider id falls back to "groq" when settings are unavailable so the
   * M5→M6 trust-transitive default is preserved (Decision #7 release notes).
   *
   * **API key invariant #1**: this function MUST NOT call the credentials
   * read function that returns the actual key string. `has_credential`
   * (boolean) is the only frontend-allowed lookup. Reviewer grep over `src/`
   * for that read should remain empty (only the `_preview` masking variant
   * appears in M3 SettingsApiKeySection.vue).
   */
  async function resolvePolishEnabledAtStart(): Promise<boolean> {
    // Defensive: invoke<Settings> can both reject (Rust error) AND resolve
    // to a non-Settings value when the test harness doesn't queue a mock
    // (default `vi.fn()` resolves to `undefined`). Treat both as "no
    // settings" → polish OFF (fail-closed). This explicit `null` fallback
    // also keeps the eslint `no-useless-assignment` rule happy because the
    // initial binding is overwritten before being read.
    let settings: Settings | null;
    try {
      settings = (await invoke<Settings>("get_settings")) ?? null;
    } catch (err) {
      console.warn(
        "[voice-flow] get_settings failed during polish snapshot, defaulting OFF",
        err,
      );
      return false;
    }

    if (!settings) return false;

    const explicit = settings.llmPolishEnabled;
    if (explicit === true) {
      // Decision #7 Some(true): user-explicit ON. Polish runs even if no
      // key is stored — the Rust side surfaces ApiKeyMissing → fallback
      // raw paste + warning. This intentional behavior lets the user see
      // the warning and remediate via Settings rather than silently being
      // unable to use the polish toggle they switched on.
      return true;
    }
    if (explicit === false) {
      // Decision #7 Some(false): user-explicit OFF. Skip polish entirely.
      return false;
    }

    // Decision #7 None — auto-detect via has_credential. Fail-closed if the
    // capability check itself errors so a transient credentials backend
    // glitch doesn't push polish onto a user who never opted in.
    const provider = settings.llmProvider ?? "groq";
    try {
      return (await invoke<boolean>("has_credential", { provider })) === true;
    } catch (err) {
      console.warn(
        "[voice-flow] has_credential failed during polish snapshot, defaulting OFF",
        err,
      );
      return false;
    }
  }

  /** Hotkey-up → stop recording → transcribe → (polish?) → paste.
   *
   * Stop is **session-scoped**: if a newer handleStart has run, our
   * terminal transitions (transcribing/enhancing/success/idle) skip so we
   * don't clobber the new recording's status. Transcribe + paste still run
   * in background so the user gets their audio transcribed + pasted; only
   * the visible state machine is gated.
   *
   * **M6 polish branch (F22 + F34)**: when `polishEnabledAtStart` for this
   * session is true, we transition to `enhancing` and invoke `polish_text`.
   * Decision #5 retry: on transient errors (network / 5xx / Busy) and when
   * the user has retry enabled (default), we invoke `polish_text` a second
   * time with `attempt: 2`. Both attempts share the Rust `polish_busy`
   * BusyGuard — they're sequential by `await`, so the second invoke sees
   * the guard released. If both attempts fail, OR if a non-retryable
   * variant came back (ApiKeyMissing / SafetyBlocked / etc.), OR if retry
   * is disabled, we fall back to raw paste + set `polishWarning` for the
   * chunk-3 HUD's amber success bubble.
   *
   * Whichever text ends up pasted (raw OR polished) goes through
   * `pasteTextSerial` so the OS clipboard never races between the polish
   * fallback and an in-flight transcribe-paste from a parallel session.
   */
  async function handleStop(): Promise<void> {
    if (status.value !== "recording") return;
    const mySession = currentSession;
    // Read + remove the F21 snapshot so a stale entry can't leak across
    // sessions if a future handleStop bug forgot to consume it.
    const polishOnForThisSession =
      polishEnabledAtStart.get(mySession) ?? false;
    polishEnabledAtStart.delete(mySession);

    await transitionTo("transcribing", "");
    try {
      await invoke<void>("stop_recording");
      const result = await invoke<TranscriptionResult>("transcribe_audio", {
        // M4 chunk 3 ships without vocabulary — M8 wires `get_vocabulary` and
        // pipes top terms through here. Explicit `undefined` rather than `null`
        // so the Rust `Option<Vec<String>>` deserializes to `None`.
        vocabulary: undefined,
      });

      // M6 polish branch — outputs `pasteText` for the unified paste step
      // below. Reads the per-session F21 snapshot for tri-state gating; the
      // second-level retry decision reads the live settings (Decision #5).
      let pasteText = result.rawText;
      if (polishOnForThisSession) {
        if (mySession !== currentSession) {
          // Older session whose hotkey-up arrived after a newer hotkey-down.
          // Skip the visible enhancing transition (newer session owns the
          // visible state) but DO finish the polish + paste in the
          // background so the user's words get into the focused window.
          pasteText = await runPolishWithRetry(result.rawText);
        } else {
          await transitionTo("enhancing", "");
          pasteText = await runPolishWithRetry(result.rawText);
          if (mySession !== currentSession) {
            // A newer session took over while polish was awaiting. Pasting
            // is still appropriate (we have the text + the session-1 target
            // window was captured), but we must NOT emit terminal transitions
            // — those belong to the newer session. The polishWarning ref
            // stays as the polish branch left it; the newer session's
            // transitionTo('idle' | 'recording') clears it.
          }
        }
      }

      // Paste runs through the module-level chain so concurrent transcribe
      // completions don't trample the OS clipboard.
      await pasteTextSerial(pasteText);
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

  /**
   * Frontend retry orchestrator for `polish_text` (Decision #5 / F34).
   * Returns the text to paste — polished on success, raw on fallback.
   * Side effect: sets `polishWarning.value = true` on every fallback path
   * (chunk-3 HUD reads it for the amber success bubble); leaves it alone
   * on success.
   *
   * **Retry policy** mirrors Rust's reference predicate
   * `plugins::llm_polish::providers::is_retryable` — retry only on transient
   * variants (Timeout / RateLimited / NetworkOther / ConnectionRefused /
   * DnsFailure / TlsFailure / Busy). Non-retryable variants (ApiKeyMissing
   * / Disabled / EmptyInput / SafetyBlocked / InvalidPromptLength /
   * EmptyResponse / Truncated / ImplausibleOutput / Cancelled / generic
   * ApiError) take the single-attempt fallback path because retrying would
   * just hit the same upstream condition.
   *
   * **Retry disable**: when `Settings.llmPolishRetryEnabled === false`, we
   * skip retry even for transient variants and go straight to fallback.
   * `undefined` and `true` mean retry-enabled (default).
   *
   * **No backoff between attempts** (Decision #5): the polish path is
   * latency-critical for the user (HUD enhancing visual is on-screen), so
   * a sleep between attempts would degrade UX more than it would help with
   * rate-limit overshoot. The Retry-After header is parsed on the Rust
   * side (chunk-1 P1 cleanup #2) for forward-compat, but Decision #5 still
   * specifies no frontend backoff today.
   */
  async function runPolishWithRetry(rawText: string): Promise<string> {
    // Read live settings for retry-enabled. Failure → assume retry ON
    // (matches the tri-state default behavior — None / Some(true) = ON).
    let retryEnabled = true;
    try {
      const settings = await invoke<Settings>("get_settings");
      // Decision #5 tri-state: undefined and true = ON, false = OFF.
      if (settings?.llmPolishRetryEnabled === false) {
        retryEnabled = false;
      }
    } catch (err) {
      console.warn(
        "[voice-flow] get_settings failed during polish retry resolution, assuming retry ON",
        err,
      );
    }

    // Attempt 1.
    try {
      const polished = await invoke<PolishResult>("polish_text", {
        args: { rawText, vocabulary: undefined, attempt: 1 },
      });
      return polished.polishedText;
    } catch (firstErr) {
      if (retryEnabled && isRetryablePolishError(firstErr)) {
        // Attempt 2 (Decision #5 / F34 retry-same).
        try {
          const polished = await invoke<PolishResult>("polish_text", {
            args: { rawText, vocabulary: undefined, attempt: 2 },
          });
          return polished.polishedText;
        } catch (secondErr) {
          console.warn(
            "[voice-flow] polish failed on both attempts, falling back to raw",
            secondErr,
          );
          polishWarning.value = true;
          return rawText;
        }
      }
      // Either retry disabled OR error not retryable — single-attempt
      // fallback. Console.warn so we keep diagnostic breadcrumbs without
      // routing through the visible error state (polish-failed paste-raw
      // is a degraded-but-OK outcome, not a hard error).
      console.warn(
        "[voice-flow] polish failed, falling back to raw",
        firstErr,
      );
      polishWarning.value = true;
      return rawText;
    }
  }

  /** ESC during recording → cancel without paste. Clears the recording
   * buffer to free RAM (M2 retro #3). The cleanup invokes are best-effort;
   * the cancel path always returns to idle even if `clear_recording_buffer`
   * errors (e.g. recorder already stopped) so the user can immediately
   * record again.
   *
   * **F23 (M6 chunk 2)**: ESC during `enhancing` is a deliberate **no-op**
   * with a `console.warn` breadcrumb. Cancelling an in-flight LLM HTTP
   * request requires either a Tauri `cancel_polish` command or
   * `tokio::select!` with a cancel channel — both are non-trivial and
   * deferred to v0.2. Polishing is short (3 s Groq / 15 s others timeout),
   * so the user-visible inconvenience is bounded by those timeouts, and
   * any frontend retry continues normally. ESC during `transcribing` /
   * `success` / `error` is also intentionally a no-op — the only ESC-
   * meaningful state in M6 is `recording`.
   */
  async function handleCancel(): Promise<void> {
    if (status.value === "enhancing") {
      // F23: explicit no-op + diagnostic. Reviewer can grep this pattern
      // when v0.2 lands the cancel channel — the warn becomes the swap
      // site for the new `invoke<void>("cancel_polish")` call.
      console.warn(
        "[voice-flow] ESC during enhancing is a no-op (M6 limitation; cancel channel deferred to v0.2)",
      );
      return;
    }
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
   *
   * **F25 polishWarning lifecycle (M6 chunk 2)**: clear `polishWarning` on
   * any transition into `idle` or `recording`. These boundaries demarcate
   * "starting fresh" vs "ending the current cycle" — without the clear, a
   * polish-failed-warning success bubble (chunk 3 amber render) would
   * leak into the next session's success bubble even when that next
   * session's polish ran successfully (or polish was off). Done BEFORE
   * the emit so any listener reading state on the same tick sees the
   * cleared warning.
   */
  async function transitionTo(
    next: VoiceFlowStatus,
    msg: string,
  ): Promise<void> {
    if (next === "idle" || next === "recording") {
      polishWarning.value = false;
    }
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
    //
    // **Retro challenger P1 fix (M5 ship gate)**: hint is PREPENDED, not
    // appended. The HUD bubble has `text-overflow: ellipsis; max-width: 280px`
    // (chunk 2 P1-1) — appending puts the actionable hint at the truncated
    // tail where user can't see it. Prepending makes the hint always visible;
    // the technical Rust detail is what gets ellipsed instead.
    if (
      raw.startsWith("Failed to restore focus") ||
      raw.startsWith("Focus restore failed") ||
      raw.includes("FocusRestoreFailed")
    ) {
      return `請手動 Ctrl+V — ${raw}`;
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

  /**
   * Dev-only polish-warning toggle (M6 chunk 2). Allows `pnpm dev` (vite-only
   * mode) and chunk-3 Playwright screenshot helpers to flip the
   * `polishWarning` ref directly so the HUD's amber success bubble can be
   * captured without driving a real polish failure through the pipeline.
   * Production safety mirrors `__devSetStatus` — gated by `import.meta.env.DEV`
   * so a production-runtime caller is a no-op.
   */
  function __devSetPolishWarning(value: boolean): void {
    if (!import.meta.env.DEV) return;
    polishWarning.value = value;
  }

  /**
   * Dev-only state mutator (M5 chunk 2). Allows `pnpm dev` (vite-only mode,
   * no Tauri runtime) to drive the HUD through its 4 visual states for
   * Playwright screenshot captures, without going through the hotkey →
   * Rust pipeline (which can't run without Tauri).
   *
   * **Production safety (chunk 2 reviewer P1-1)**: function body gated by
   * `import.meta.env.DEV` so calls from production devtools no-op. The
   * function still ships in the bundle (return-value typing keeps the
   * symbol exported) but cannot mutate state — no risk of UI corruption
   * or misrender from an opportunistic caller.
   */
  function __devSetStatus(
    next: VoiceFlowStatus,
    msg: string,
    startedAtMs: number | null,
  ): void {
    if (!import.meta.env.DEV) return;
    status.value = next;
    message.value = msg;
    recordingStartedAtMs.value = startedAtMs;
  }

  return {
    // Read-only refs for consumers (HUD components, Dashboard sidebar).
    status: readonly(status),
    message: readonly(message),
    recordingStartedAtMs: readonly(recordingStartedAtMs),
    // F25 polishWarning: read-only ref. Chunk-3 HUD reads it to switch
    // the success bubble between green CheckCircle2 and amber AlertTriangle.
    polishWarning: readonly(polishWarning),
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
    // Dev-only: see fn doc above.
    __devSetStatus,
    __devSetPolishWarning,
  };
});
