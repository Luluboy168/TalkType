// Audio-recorder Tauri event payloads + emit helpers.
//
// Centralised because three different files need to emit them:
//
//   * `recording_thread.rs` emits `audio:recording-aborted` when the buffer
//     reaches `MAX_WAV_BYTES` (M3 size cap, M2 retro #1) or when cpal reports
//     a mic-disconnect (M2 retro mic-unplug). It also emits
//     `audio:mic-safety-warning` when `stream.pause()` fails on teardown.
//   * `preview.rs` emits `audio:mic-safety-warning` on the same pause-failure
//     path (mic-safety contract).
//   * `commands.rs` does not emit these events directly but the constants
//     below stay grep-able from a single module.
//
// Naming convention: keep camelCase serde rename in sync with the matching
// TypeScript types in `src/types/events.ts` (per architecture invariant #8).

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Tauri event name: emitted when an in-progress recording aborts because of
/// a hard size cap or device disconnect. Frontend should treat this as
/// "recording effectively stopped — refresh state".
pub const EVENT_RECORDING_ABORTED: &str = "audio:recording-aborted";

/// Tauri event name: emitted on `stream.pause()` failure. SECURITY-relevant
/// because it indicates the cpal stream may not have been torn down cleanly.
/// The matching `eprintln!` line stays for dev mode (where stderr is visible);
/// this event is the release-build-friendly companion (M2 retro #2).
pub const EVENT_MIC_SAFETY_WARNING: &str = "audio:mic-safety-warning";

/// Reasons a recording can be aborted. String literals match the frontend
/// payload via `serde(rename_all = "snake_case")`-style values; keep these
/// in sync with the union type in `src/types/events.ts`.
pub mod recording_aborted_reason {
    /// The recording reached `MAX_WAV_BYTES` — Groq cap + OOM defense.
    pub const MAX_SIZE: &str = "max_size";
    /// cpal reported a stream error suggesting the mic was disconnected.
    pub const MIC_UNPLUG: &str = "mic_unplug";
}

/// Payload of the `audio:recording-aborted` event.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordingAbortedPayload {
    /// One of `recording_aborted_reason::*` (e.g. "max_size", "mic_unplug").
    pub reason: String,
    /// Bytes accumulated in the i16 sample buffer at abort time
    /// (sample count × 2 bytes per i16). Lets the UI surface "you recorded
    /// ~13 minutes" UX.
    pub bytes_recorded: usize,
}

/// Payload of the `audio:mic-safety-warning` event. M5 will surface this as a
/// visible HUD warning; for now it's just logged (`App.vue` HUD listens and
/// `console.error`s).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MicSafetyPayload {
    /// Free-form human-readable detail, e.g. the cpal pause error or the
    /// `err_fn` message. Phase 1 has no PII concern (mic device names live in
    /// the recording-thread log, not here); M9 polish should mask the device
    /// name if anything PII-shaped slips in.
    pub detail: String,
}

/// Emit `audio:mic-safety-warning`. Helper so the eprintln + emit pattern
/// stays identical across `recording_thread.rs` and `preview.rs`.
///
/// `source_tag` is the `[audio-recorder]` / `[audio-preview]` prefix used in
/// the SECURITY: log line; keep it in sync with the rest of the eprintln
/// calls in that module so log scrubs find both sides.
pub fn emit_mic_safety_warning(app: &AppHandle, source_tag: &str, detail: String) {
    eprintln!("{source_tag} SECURITY: stream.pause() failed — mic may still be active: {detail}");
    let payload = MicSafetyPayload {
        detail: detail.clone(),
    };
    if let Err(e) = app.emit(EVENT_MIC_SAFETY_WARNING, payload) {
        // Don't escalate — emit failure shouldn't shadow the real warning.
        eprintln!("{source_tag} failed to emit {EVENT_MIC_SAFETY_WARNING}: {e}");
    }
}

/// Emit `audio:recording-aborted`. Used by the recording thread when the
/// buffer reaches `MAX_WAV_BYTES` or cpal reports a mic-disconnect.
pub fn emit_recording_aborted(app: &AppHandle, reason: &str, bytes_recorded: usize) {
    let payload = RecordingAbortedPayload {
        reason: reason.to_string(),
        bytes_recorded,
    };
    eprintln!(
        "[audio-recorder] {EVENT_RECORDING_ABORTED}: reason={reason} bytesRecorded={bytes_recorded}"
    );
    if let Err(e) = app.emit(EVENT_RECORDING_ABORTED, payload) {
        eprintln!("[audio-recorder] failed to emit {EVENT_RECORDING_ABORTED}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_aborted_payload_serializes_camel_case() {
        let p = RecordingAbortedPayload {
            reason: recording_aborted_reason::MAX_SIZE.to_string(),
            bytes_recorded: 25_000_000,
        };
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(
            json,
            "{\"reason\":\"max_size\",\"bytesRecorded\":25000000}"
        );
    }

    #[test]
    fn mic_safety_payload_serializes_camel_case() {
        let p = MicSafetyPayload {
            detail: "pause failed".to_string(),
        };
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "{\"detail\":\"pause failed\"}");
    }

    #[test]
    fn recording_aborted_reason_constants_match_frontend_union() {
        // Document-side constants. Update `src/types/events.ts`
        // RecordingAbortedReason if these change.
        assert_eq!(recording_aborted_reason::MAX_SIZE, "max_size");
        assert_eq!(recording_aborted_reason::MIC_UNPLUG, "mic_unplug");
    }
}
