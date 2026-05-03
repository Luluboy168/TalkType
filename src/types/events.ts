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

// ─── M7: local transcription (placeholders — populated in M7) ──────────────

export type TranscriptionProgressPayload = unknown;
export type ModelDownloadProgressPayload = unknown;

// ─── M8: settings + database (placeholders — populated in M8) ──────────────

export type SettingsUpdatedPayload = unknown;
export type HistoryAddedPayload = unknown;
export type VocabularyChangedPayload = unknown;

// ─── Frontend-only events (HUD ↔ Dashboard) ────────────────────────────────

export type VoiceFlowStateChangedPayload = unknown;
export type TranscriptionCompletedPayload = unknown;
