// Tauri event payload types — single source of truth for IPC contracts.
// One type per event in `useTauriEvents.ts`. Most are placeholders (`unknown`)
// for M1; later milestones will fill them in as they bring the events online.
//
// Naming convention (per doc/plans/04-frontend-structure.md):
//   - `*Payload` for Tauri event payloads emitted by Rust.

// ─── M1: ping/pong smoke test ──────────────────────────────────────────────

export interface PingPayload {
  /** Identifies the originating window — "main" (HUD) or "main-window" (Dashboard). */
  source: string;
}

export interface PongPayload {
  /** Echoed back from the responder so the caller can confirm round-trip. */
  source: string;
  /** Responder timestamp in ms-since-epoch for latency diagnostics. */
  timestampMs: number;
}

// ─── M4: hotkey listener (placeholders — populated in M4) ──────────────────

export type HotkeyEventPayload = unknown;
export type HotkeyErrorPayload = unknown;
export type HotkeyRecordingCapturedPayload = unknown;
export type HotkeyRecordingRejectedPayload = unknown;

// ─── M2: audio recorder ────────────────────────────────────────────────────

/**
 * Payload of the `audio:waveform` event emitted ~every 16 ms while a
 * recording is active. The 6 values are normalized FFT magnitudes ([0.0, 1.0])
 * for hand-tuned frequency bins; index choice is intentionally non-linear
 * for visual spread (see `src-tauri/src/plugins/audio_recorder/waveform.rs`).
 */
export interface WaveformPayload {
  /** 6 normalized FFT magnitudes in `[0.0, 1.0]`. */
  levels: [number, number, number, number, number, number];
}

/**
 * Payload of the `audio:preview-level` event emitted ~every 30 ms while the
 * mic preview is active. `level` is the RMS amplitude over the last ~30 ms
 * window in `[0.0, 1.0]`.
 */
export interface AudioPreviewLevelPayload {
  /** RMS amplitude over the latest ~30 ms window in `[0.0, 1.0]`. */
  level: number;
}

/**
 * Reason values for `RecordingAbortedPayload.reason`. Mirrors the Rust-side
 * `recording_aborted_reason` consts in `audio_recorder/events.rs`.
 *
 * Forward-compatible: M3+ may add new variants (e.g. `'busy'`, `'permission'`)
 * without breaking listeners that handle the union.
 */
export type RecordingAbortedReason = "max_size" | "mic_unplug";

/**
 * Payload of the `audio:recording-aborted` event. Fired from Rust when an
 * in-progress recording auto-stops because the i16 buffer hit
 * `MAX_WAV_BYTES` (~25 MB / ~13 min @ 16 kHz mono) or because cpal reported
 * a stream error consistent with mic disconnection.
 */
export interface RecordingAbortedPayload {
  /** Why the recording was aborted. */
  reason: RecordingAbortedReason;
  /** Bytes in the i16 sample buffer at abort time (`sampleCount * 2`). */
  bytesRecorded: number;
}

/**
 * Payload of the `audio:mic-safety-warning` event. Fired from Rust when
 * `cpal::Stream::pause()` returns an error during teardown — a SECURITY-
 * relevant condition because it suggests the mic stream may not have been
 * fully torn down. The M2 retro flagged that the matching `eprintln!` is
 * invisible in release builds (stderr → /dev/null), so this event provides
 * a release-friendly alternative path for the HUD / error panel to surface
 * the warning.
 */
export interface MicSafetyPayload {
  /** Free-form human-readable detail (cpal pause error message). */
  detail: string;
}

// ─── M7: local transcription (placeholders — populated in M7) ──────────────

export type TranscriptionProgressPayload = unknown;
export type ModelDownloadProgressPayload = unknown;

// ─── M8: settings + database (placeholders — populated in M8) ──────────────

export type SettingsUpdatedPayload = unknown;
export type HistoryAddedPayload = unknown;
export type VocabularyChangedPayload = unknown;

// ─── M3 chunk 2: transcription dispatcher (Groq cloud) ────────────────────

/**
 * Result of `invoke<TranscriptionResult>('transcribe_audio', ...)` AND payload
 * of the `transcription:completed` event broadcast on success.
 *
 * Mirrors the Rust struct `TranscriptionResult` in
 * `src-tauri/src/plugins/transcription/mod.rs` (`#[serde(rename_all =
 * "camelCase")]`). Keep the two in sync per architecture invariant #8.
 */
export interface TranscriptionResult {
  /** Raw text returned by Groq's Whisper response. Trimmed by the Rust side. */
  rawText: string;
  /** Wall-clock duration of the Groq round trip in ms (excludes pre-validation). */
  transcriptionDurationMs: number;
  /**
   * Minimum `no_speech_prob` across `verbose_json` segments — lower means
   * higher speech confidence. `null` when the response had no segments
   * (rare; near-empty audio).
   */
  noSpeechProbability: number | null;
}

// ─── Frontend-only events (HUD ↔ Dashboard) ────────────────────────────────

export type VoiceFlowStateChangedPayload = unknown;

/**
 * Payload of `transcription:completed` event broadcast to both windows.
 *
 * Identical shape to `TranscriptionResult` — exposed as a separate alias so
 * the call site in HUD (paste handler) can express the dependency on the
 * event-payload contract distinct from the command-result contract.
 */
export type TranscriptionCompletedPayload = TranscriptionResult;
