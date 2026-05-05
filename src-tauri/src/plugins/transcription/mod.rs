// Transcription plugin (M3 chunk 2).
//
// Public surface:
//
//   State         TranscriptionState   (managed by lib.rs)
//   Command       transcribe_audio
//   Internal API  transcribe_cloud_internal (in cloud.rs, pub(crate))
//
// **Dispatcher pattern**: `transcribe_audio` is the single Tauri command the
// frontend invokes. M3 hardcodes Groq cloud — when M7 ships local
// transcription, the dispatcher will read `whisper_provider` from
// `SettingsState` and route to either `cloud::transcribe_cloud_internal` or
// `local::transcribe_local_internal` while keeping the same
// `TranscriptionResult` shape so the UI doesn't branch.
//
// **Concurrency**: parallel transcribes are allowed so rapid voice typing
// works — user presses, releases, then presses again before the previous
// transcribe's network round-trip completes. Each invoke spawns its own
// tokio task; the WAV buffer is consumed at the start of `transcribe_
// cloud_internal` (`Mutex::take`) so two transcribes never race on the
// same bytes. Earlier M3 design used a `transcribe_busy` AtomicBool guard
// that rejected overlapping calls with `TranscriptionError::Busy`; the
// guard was removed in M4 acceptance because it made rapid press feel
// broken from the user's POV. The `Busy` enum variant stays defined for
// the retry-policy classifier and any future explicit serialization.
//
// **Paste serialization**: the FRONTEND serializes `paste_text` calls via
// a Promise chain in `useVoiceFlowStore` so two simultaneous transcribe
// completions can't race on the OS clipboard. Without that lock, two
// concurrent `clipboard.set_text` + `SendInput Ctrl+V` pipelines can
// trample each other's clipboard contents before either's SendInput fires.
//
// **Event broadcast**: on success we `app.emit("transcription:completed",
// result)` so both HUD and Dashboard windows can react. The HUD pastes the
// text (M5); the Dashboard refreshes its history list (M8).

pub mod cloud;
pub mod error;
pub mod health;
mod parser;

pub use error::TranscriptionError;
pub use health::test_provider_connection;

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::plugins::audio_recorder::AudioRecorderState;

/// Application-managed state for transcription.
///
/// Single shared `reqwest::Client` so we reuse the connection pool across
/// transcribe calls (TLS handshake reuse alone saves ~150 ms on the first
/// retry of a session). Parallel transcribes are allowed; the WAV buffer
/// is consumed at each `transcribe_cloud_internal` start so they don't
/// race on bytes.
pub struct TranscriptionState {
    pub(crate) client: reqwest::Client,
}

impl TranscriptionState {
    /// Build a `TranscriptionState` with TalkType's Groq-tuned defaults:
    /// 120 s overall timeout (matches Groq's worst-case latency for a 13 min
    /// recording at peak demand), 60 s pool idle, `User-Agent: TalkType/<ver>`.
    pub fn new() -> Result<Self, TranscriptionError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_idle_timeout(Duration::from_secs(60))
            .user_agent(format!("TalkType/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| TranscriptionError::NetworkOther(format!("client build: {e}")))?;
        Ok(Self { client })
    }
}

/// Output payload to frontend. `camelCase` to match `src/types/events.ts`
/// `TranscriptionResult` interface — keep the two in sync per
/// architecture invariant #8 (doc/plans/01-architecture.md).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub raw_text: String,
    pub transcription_duration_ms: u64,
    pub no_speech_probability: Option<f32>,
}

/// Frontend-facing transcription command.
///
/// M3 hardcodes Groq cloud. M7 will read `whisper_provider` from
/// `SettingsState` once local whisper.cpp is wired in.
///
/// Parallel invokes are allowed (M4 acceptance fix) — the WAV buffer is
/// consumed at the start of `transcribe_cloud_internal` so two transcribes
/// never race on the same bytes. Frontend serializes the resulting paste
/// calls via a Promise chain in `useVoiceFlowStore` so the OS clipboard
/// isn't trampled when two transcribes complete close together.
///
/// Steps:
///
///   1. Call into `cloud::transcribe_cloud_internal` which handles WAV
///      validation, keyring lookup, multipart POST, retry, and parsing.
///   2. Emit `transcription:completed` for both windows.
///   3. Return the `TranscriptionResult` to the original `invoke<T>()` call.
#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    transcription_state: State<'_, TranscriptionState>,
    audio_state: State<'_, AudioRecorderState>,
    vocabulary: Option<Vec<String>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    // M3: Groq is the only cloud provider. M7 dispatches based on settings.
    let result =
        cloud::transcribe_cloud_internal(&transcription_state.client, &audio_state, vocabulary)
            .await?;

    // Broadcast to both windows. We swallow emit errors because the
    // user-visible result is already returned via the Tauri command return
    // value — the broadcast is for sibling windows (Dashboard history
    // refresh in M8) and missing it isn't a failure of the transcribe.
    if let Err(e) = app.emit("transcription:completed", result.clone()) {
        eprintln!("[transcription] emit transcription:completed failed: {e}");
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    // Dispatcher-level tests focus on state construction + result shape.
    // The Groq HTTP layer has its own wiremock-based tests in cloud.rs.
    // End-to-end transcribe_audio testing requires a Tauri AppHandle which
    // is overkill for unit tests; chunk 3's manual smoke covers it.
    //
    // Earlier M3 design had `BusyGuard` + `transcribe_busy` AtomicBool tests
    // here. Those were removed in M4 acceptance because the busy guard was
    // dropped to allow rapid voice typing — see module-level comment.

    use super::*;

    #[test]
    fn transcription_state_builds_successfully() {
        let _state = TranscriptionState::new().expect("build");
    }

    #[test]
    fn transcription_result_serializes_camel_case() {
        let r = TranscriptionResult {
            raw_text: "hi".to_string(),
            transcription_duration_ms: 250,
            no_speech_probability: Some(0.1),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"rawText\":\"hi\""));
        assert!(json.contains("\"transcriptionDurationMs\":250"));
        assert!(json.contains("\"noSpeechProbability\":0.1"));
    }

    #[test]
    fn transcription_result_serializes_none_no_speech_as_null() {
        let r = TranscriptionResult {
            raw_text: "hi".to_string(),
            transcription_duration_ms: 100,
            no_speech_probability: None,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"noSpeechProbability\":null"));
    }
}
