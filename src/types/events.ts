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

// ─── M2: audio recorder (placeholders — populated in M2) ───────────────────

export type WaveformPayload = unknown;
export type AudioPreviewLevelPayload = unknown;

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
