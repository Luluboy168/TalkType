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
// **Concurrency guard**: `transcribe_busy: Arc<AtomicBool>` rejects
// overlapping calls with `TranscriptionError::Busy`. Acquired via
// `swap(true, AcqRel)` so two simultaneous frontend invokes can't both
// succeed; released via a RAII `BusyGuard` so any error path also resets
// the flag (otherwise a single panic would leave the app stuck in "busy"
// mode until restart).
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::plugins::audio_recorder::AudioRecorderState;

/// Application-managed state for transcription.
///
/// Single shared `reqwest::Client` so we reuse the connection pool across
/// transcribe calls (TLS handshake reuse alone saves ~150 ms on the first
/// retry of a session). Single `AtomicBool` busy guard so two overlapping
/// transcriptions can't race on the WAV buffer or the Groq quota.
pub struct TranscriptionState {
    pub(crate) client: reqwest::Client,
    pub(crate) transcribe_busy: Arc<AtomicBool>,
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
        Ok(Self {
            client,
            transcribe_busy: Arc::new(AtomicBool::new(false)),
        })
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
/// `SettingsState` once that lives in Rust (`SettingsState` arrives in M8).
///
/// Steps:
///
///   1. Acquire `transcribe_busy` exclusively (`swap(true, AcqRel)`) — if
///      another transcribe is in flight, return `Busy` immediately.
///   2. Bind a `BusyGuard` so any subsequent error path releases the flag
///      via `Drop`.
///   3. Call into `cloud::transcribe_cloud_internal` which handles WAV
///      validation, keyring lookup, multipart POST, retry, and parsing.
///   4. Emit `transcription:completed` for both windows.
///   5. Return the `TranscriptionResult` to the original `invoke<T>()` call.
#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    transcription_state: State<'_, TranscriptionState>,
    audio_state: State<'_, AudioRecorderState>,
    vocabulary: Option<Vec<String>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    // Guard against concurrent invokes. swap returns the PREVIOUS value;
    // if it was already true, someone else won the race — bail.
    let busy = transcription_state.transcribe_busy.clone();
    if busy.swap(true, Ordering::AcqRel) {
        return Err(TranscriptionError::Busy);
    }
    // RAII guard — Drop fires whether the call returns Ok / Err / panics.
    let _guard = BusyGuard(busy);

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

/// RAII drop-guard that flips `transcribe_busy` back to `false` whatever
/// happens. Holding `Arc<AtomicBool>` instead of `&AtomicBool` so the guard
/// outlives the `State` reference's lifetime within the command body.
struct BusyGuard(Arc<AtomicBool>);

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    // Dispatcher-level tests focus on the BusyGuard and state construction.
    // The Groq HTTP layer has its own wiremock-based tests in cloud.rs.
    // End-to-end transcribe_audio testing requires a Tauri AppHandle which
    // is overkill for unit tests; chunk 3's manual smoke covers it.

    use super::*;

    #[test]
    fn transcription_state_builds_with_default_busy_false() {
        let state = TranscriptionState::new().expect("build");
        assert!(!state.transcribe_busy.load(Ordering::Acquire));
    }

    #[test]
    fn busy_guard_releases_on_drop() {
        let flag = Arc::new(AtomicBool::new(true));
        {
            let _g = BusyGuard(flag.clone());
        }
        assert!(!flag.load(Ordering::Acquire));
    }

    #[test]
    fn busy_guard_releases_on_panic_unwind() {
        // Confirm Drop fires even when the surrounding code panics.
        let flag = Arc::new(AtomicBool::new(false));
        let outer = flag.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            outer.store(true, Ordering::Release);
            let _g = BusyGuard(outer.clone());
            panic!("simulated failure");
        }));
        assert!(result.is_err());
        assert!(
            !flag.load(Ordering::Acquire),
            "guard should reset on unwind"
        );
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
