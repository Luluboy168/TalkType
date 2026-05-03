// Audio-recorder error type.
//
// Per CLAUDE.md "Error enum 手動 implement Serialize 為 string": the frontend
// receives a flat string for every variant via the manual `Serialize` impl
// below, so the JS side just sees `string` rather than a `{type, message}`
// object. This matches SayIt's pattern and keeps `invoke<T>()` ergonomic on
// the Vue side.

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors returned by the audio_recorder Tauri commands.
///
/// Variants intentionally mirror the failure modes documented in SayIt's
/// `audio_recorder.rs` (`NoInputDevice` / `InputConfig` / `BuildStream` /
/// `PlayStream` / `NotRecording` / `WavEncode` / `LockPoisoned`) plus
/// file-system variants (`FileIo`, `RecordingNotFound`) for the recordings
/// directory commands.
#[derive(Error, Debug)]
pub enum AudioRecorderError {
    #[error("No input device available")]
    NoInputDevice,

    #[error("Failed to query input config: {0}")]
    InputConfig(String),

    #[error("Failed to build cpal stream: {0}")]
    BuildStream(String),

    #[error("Failed to play cpal stream: {0}")]
    PlayStream(String),

    #[error("Recorder is not currently recording")]
    NotRecording,

    #[error("WAV encode error: {0}")]
    WavEncode(String),

    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),

    #[error("File I/O error: {0}")]
    FileIo(String),

    #[error("Recording not found: {0}")]
    RecordingNotFound(String),
}

// Manual `Serialize` so the frontend receives a flat string rather than a
// tagged enum object. See CLAUDE.md "Error enum 手動 implement Serialize 為
// string".
impl Serialize for AudioRecorderError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_human_readable() {
        assert_eq!(
            AudioRecorderError::NoInputDevice.to_string(),
            "No input device available"
        );
        assert_eq!(
            AudioRecorderError::NotRecording.to_string(),
            "Recorder is not currently recording"
        );
        assert_eq!(
            AudioRecorderError::InputConfig("foo".to_string()).to_string(),
            "Failed to query input config: foo"
        );
    }

    #[test]
    fn serialize_emits_flat_string() {
        let err = AudioRecorderError::WavEncode("oops".to_string());
        let json = serde_json::to_string(&err).expect("serialize");
        assert_eq!(json, "\"WAV encode error: oops\"");
    }
}
