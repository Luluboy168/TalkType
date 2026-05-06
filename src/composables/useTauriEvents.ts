// Centralized Tauri event API. This is the ONLY file in the codebase that is
// allowed to import from `@tauri-apps/api/event` — every other call site must
// go through the re-exported helpers and event-name constants below. See
// doc/plans/01-architecture.md "Invariants" rule #3 and
// doc/plans/04-frontend-structure.md for rationale.
import { emit, emitTo, listen } from "@tauri-apps/api/event";

// Re-exported with TalkType-specific names so call sites stay grep-able.
export const listenToEvent = listen;
export const emitEvent = emit;
export const emitToWindow = emitTo;

// ─── M1: IPC smoke test ────────────────────────────────────────────────────

export const PING = "ipc:ping" as const;
export const PONG = "ipc:pong" as const;

// ─── M4: hotkey listener ───────────────────────────────────────────────────

export const HOTKEY_PRESSED = "hotkey:pressed" as const;
export const HOTKEY_RELEASED = "hotkey:released" as const;
export const HOTKEY_TOGGLED = "hotkey:toggled" as const;
export const HOTKEY_ERROR = "hotkey:error" as const;
export const HOTKEY_RECORDING_CAPTURED = "hotkey:recording-captured" as const;
export const HOTKEY_RECORDING_REJECTED = "hotkey:recording-rejected" as const;
export const ESCAPE_PRESSED = "escape:pressed" as const;

// ─── M4: clipboard paste ───────────────────────────────────────────────────

/**
 * Emitted by `clipboard_paste` (M4 chunk 2) when `SetForegroundWindow` fails
 * during the paste pipeline (typically Windows 11 anti-flash policy refusing
 * to hand focus back). Text is still on the clipboard at this point — the
 * HUD shows a friendly "請手動 Ctrl+V" fallback. See `PasteFocusRestoreFailedPayload`
 * in `src/types/events.ts`.
 */
export const PASTE_FOCUS_RESTORE_FAILED = "paste:focus-restore-failed" as const;

// ─── M2: audio recorder ────────────────────────────────────────────────────

export const AUDIO_WAVEFORM = "audio:waveform" as const;
export const AUDIO_PREVIEW_LEVEL = "audio:preview-level" as const;

// ─── M3 chunk 0: audio recorder safety / abort events ─────────────────────

/** Emitted when the recording auto-aborts (size cap reached, mic unplugged, ...). */
export const AUDIO_RECORDING_ABORTED = "audio:recording-aborted" as const;
/** Emitted when `stream.pause()` fails — release-build alternative to the
 * dev-only `SECURITY:` stderr log. M5 will surface visibly in the HUD. */
export const AUDIO_MIC_SAFETY_WARNING = "audio:mic-safety-warning" as const;

// ─── M7: local transcription / model download ──────────────────────────────

export const TRANSCRIPTION_PROGRESS = "transcription:progress" as const;
export const MODEL_DOWNLOAD_PROGRESS = "model:download-progress" as const;

// ─── M8: settings + database ───────────────────────────────────────────────

export const SETTINGS_UPDATED = "settings:updated" as const;
export const HISTORY_ADDED = "history:added" as const;
export const VOCABULARY_CHANGED = "vocabulary:changed" as const;

// ─── M6: LLM polish fallback signal ────────────────────────────────────────

/**
 * Emitted by Rust `llm_polish` when polish fell back to raw transcript
 * paste — either polish was attempted and the LLM round-trip failed
 * (Decision #5 retry toggle decides whether retry-same fires before
 * fallback) or the input failed a sanity check (F8 / F9 / F11). Payload
 * shape: `PolishFallbackPayload` in `src/types/llm.ts`. The chunk-3 HUD
 * uses this to flip the success bubble to amber (warning) for the
 * unified 1500 ms linger window (Decision #8).
 */
export const POLISH_FAILED_FALLBACK = "polish:failed-fallback" as const;

// ─── Frontend-only events (HUD ↔ Dashboard) ────────────────────────────────

export const VOICE_FLOW_STATE_CHANGED = "voice-flow:state-changed" as const;
export const TRANSCRIPTION_COMPLETED = "transcription:completed" as const;

/**
 * Runtime-iterable map of every event constant. Useful for diagnostics
 * commands that want to dump or test every event without importing each name
 * individually.
 */
export const EVENT_NAMES = {
  PING,
  PONG,
  HOTKEY_PRESSED,
  HOTKEY_RELEASED,
  HOTKEY_TOGGLED,
  HOTKEY_ERROR,
  HOTKEY_RECORDING_CAPTURED,
  HOTKEY_RECORDING_REJECTED,
  ESCAPE_PRESSED,
  PASTE_FOCUS_RESTORE_FAILED,
  AUDIO_WAVEFORM,
  AUDIO_PREVIEW_LEVEL,
  AUDIO_RECORDING_ABORTED,
  AUDIO_MIC_SAFETY_WARNING,
  TRANSCRIPTION_PROGRESS,
  MODEL_DOWNLOAD_PROGRESS,
  SETTINGS_UPDATED,
  HISTORY_ADDED,
  VOCABULARY_CHANGED,
  POLISH_FAILED_FALLBACK,
  VOICE_FLOW_STATE_CHANGED,
  TRANSCRIPTION_COMPLETED,
} as const;
