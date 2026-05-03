// Cloud transcription implementation (Groq Whisper).
//
// `transcribe_cloud_internal` is the only entry point. The dispatcher in
// `mod.rs` calls it with the shared `reqwest::Client` from `TranscriptionState`
// and the audio recorder's WAV buffer. M3 hardcodes Groq; M7 will share this
// dispatcher with a `transcribe_local_internal` for whisper.cpp.
//
// **Buffer-take ordering** (M3 plan-time challenger #4 — Q2 invariant): we
// pre-validate WAV size BEFORE calling `consume_wav_buffer()`. The WAV must
// stay recoverable when validation rejects (user can `save_recording_file`
// to keep the audio); only the success path takes ownership. On retry-able
// failures (`Timeout`, `RateLimited`) we still consume the buffer once and
// reuse the in-memory `Vec<u8>` for the second attempt — re-locking the
// recorder mutex would risk a race with a new recording.
//
// **Retry policy**: at most one retry, only for `Timeout` and `RateLimited`.
// 4xx (including 401 invalid key, 413 too-large) and `Offline` /
// `ApiKeyMissing` / `Busy` never retry — they're permanent for the current
// request.
//
// Helpers (vocabulary prompt formatting, `verbose_json` parser) live in the
// sibling `parser.rs` module so this file stays focused on the HTTP +
// retry plumbing.

use std::time::{Duration, Instant};

use reqwest::multipart::{Form, Part};

use super::error::{classify_reqwest_error, TranscriptionError};
use super::parser::{format_whisper_prompt, parse_groq_response};
use super::TranscriptionResult;
use crate::plugins::audio_recorder::{AudioRecorderState, MAX_WAV_BYTES};
use crate::plugins::credentials;

/// Groq's audio transcription endpoint. Test code uses
/// `post_to_groq_with_url` to point at a local wiremock server instead.
const GROQ_TRANSCRIBE_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Default Groq Whisper model. `whisper-large-v3-turbo` is the fastest
/// production model with comparable quality to `large-v3` for short utterances.
const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";

/// Below this floor we treat the buffer as "user fat-fingered the hotkey"
/// and skip the network call. ~31 ms of 16 kHz mono i16 audio.
const MIN_AUDIO_BYTES: usize = 1_000;

/// Internal cloud-transcribe path. Pre-validates buffer size BEFORE
/// `consume_wav_buffer()` (Q2 invariant; failure must leave WAV recoverable).
/// Reads API key from keyring (Rust-only). Builds multipart form, POSTs to
/// Groq, parses `verbose_json`, returns `TranscriptionResult`.
pub(crate) async fn transcribe_cloud_internal(
    client: &reqwest::Client,
    audio_state: &tauri::State<'_, AudioRecorderState>,
    vocabulary: Option<Vec<String>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    // ── Phase 1: pre-check size BEFORE take() ──────────────────────────────
    let buf_size = {
        let guard = audio_state
            .wav_buffer
            .lock()
            .map_err(|e| TranscriptionError::LockPoisoned(e.to_string()))?;
        match guard.as_ref() {
            None => return Err(TranscriptionError::NoAudioData),
            Some(b) => b.len(),
        }
    };
    if buf_size < MIN_AUDIO_BYTES {
        return Err(TranscriptionError::AudioTooSmall(buf_size));
    }
    if buf_size > MAX_WAV_BYTES {
        return Err(TranscriptionError::FileTooLarge {
            actual_bytes: buf_size,
            max_bytes: MAX_WAV_BYTES,
        });
    }

    // ── API key from keyring (Rust-only, never crosses IPC) ────────────────
    let api_key = credentials::get_credential("groq")
        .map_err(|e| TranscriptionError::Credentials(e.to_string()))?
        .ok_or_else(|| TranscriptionError::ApiKeyMissing("groq".to_string()))?;

    // ── Phase 2: take() the WAV buffer (last consumer; success path) ───────
    // Cloned for retry — see "Retry policy" in module doc.
    let wav_bytes = audio_state
        .consume_wav_buffer()
        .map_err(|e| TranscriptionError::LockPoisoned(format!("consume_wav_buffer: {e}")))?
        .ok_or(TranscriptionError::NoAudioData)?;

    let started = Instant::now();
    let response_result = post_to_groq(client, &api_key, &wav_bytes, vocabulary.as_deref()).await;

    // ── Retry once on Timeout / RateLimited only ───────────────────────────
    let response_body = match response_result {
        Ok(body) => body,
        Err(e) if is_retryable(&e) => {
            // Honor Retry-After if present.
            if let TranscriptionError::RateLimited {
                retry_after_secs: Some(secs),
            } = &e
            {
                eprintln!("[transcription] 429 — sleeping {secs}s before retry");
                tokio::time::sleep(Duration::from_secs(*secs)).await;
            } else {
                eprintln!("[transcription] retrying after {e}");
            }
            post_to_groq(client, &api_key, &wav_bytes, vocabulary.as_deref()).await?
        }
        Err(e) => {
            // Non-retryable: WAV is gone (already taken). Documented in the
            // module-level doc: chunk 3 may revisit "put back" semantics for
            // ApiError / ParseError / ApiKeyMissing if dogfood reveals UX pain.
            eprintln!("[transcription] non-retryable error: {e}");
            return Err(e);
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let parsed = parse_groq_response(&response_body).map_err(TranscriptionError::ParseError)?;

    Ok(TranscriptionResult {
        raw_text: parsed.text,
        transcription_duration_ms: duration_ms,
        no_speech_probability: parsed.min_no_speech,
    })
}

/// True iff the error category is one that a single retry has a reasonable
/// chance of fixing. Keep the matrix narrow: timeouts (transient network
/// blip) and explicit rate limits (the server told us to wait).
fn is_retryable(e: &TranscriptionError) -> bool {
    matches!(
        e,
        TranscriptionError::Timeout(_) | TranscriptionError::RateLimited { .. }
    )
}

/// Convenience wrapper around `post_to_groq_with_url` using the production
/// endpoint. Tests use `post_to_groq_with_url` directly to point at a
/// wiremock server.
async fn post_to_groq(
    client: &reqwest::Client,
    api_key: &str,
    wav_bytes: &[u8],
    vocabulary: Option<&[String]>,
) -> Result<String, TranscriptionError> {
    post_to_groq_with_url(client, GROQ_TRANSCRIBE_URL, api_key, wav_bytes, vocabulary).await
}

/// URL-parameterized variant for tests. Builds the multipart form, sends
/// the request, and returns the response body string on 2xx. Maps non-2xx
/// to typed errors (`RateLimited` for 429 with Retry-After parsing,
/// `ApiError { status, body }` for everything else).
///
/// On reqwest-level failures (TLS, DNS, timeout, connect) returns the
/// classified `TranscriptionError` from `classify_reqwest_error`.
async fn post_to_groq_with_url(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    wav_bytes: &[u8],
    vocabulary: Option<&[String]>,
) -> Result<String, TranscriptionError> {
    // Clone wav_bytes into the multipart body. reqwest's `Part::bytes` takes
    // a `Vec<u8>` because the request body must outlive the future. The
    // double allocation is one small price for retry support.
    let mut form = Form::new()
        .text("model", DEFAULT_MODEL)
        .text("response_format", "verbose_json");

    if let Some(prompt) = format_whisper_prompt(vocabulary) {
        form = form.text("prompt", prompt);
    }

    let part = Part::bytes(wav_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| TranscriptionError::NetworkOther(format!("multipart: {e}")))?;
    form = form.part("file", part);

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(classify_reqwest_error)?;

    let status = resp.status();
    if !status.is_success() {
        if status.as_u16() == 429 {
            let retry_after_secs = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            return Err(TranscriptionError::RateLimited { retry_after_secs });
        }
        // 401 / 403 / 413 / 5xx all flow through ApiError; the frontend can
        // string-match on the status int for localized remediation.
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(TranscriptionError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    resp.text().await.map_err(classify_reqwest_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: POST a tiny payload to `wiremock`'s URL via our internal
    /// helper. We don't construct an `AudioRecorderState` here because
    /// `transcribe_cloud_internal` wants a `tauri::State<'_, _>` which
    /// requires a full app handle. Higher-level integration is chunk 3.
    async fn post_to_mock(
        server: &MockServer,
        vocabulary: Option<&[String]>,
    ) -> Result<String, TranscriptionError> {
        let client = reqwest::Client::new();
        let url = format!("{}/openai/v1/audio/transcriptions", server.uri());
        // 1 KB of zeros — small enough to keep tests fast, large enough that
        // post_to_groq_with_url's multipart construction runs realistically.
        let wav = vec![0u8; 1024];
        post_to_groq_with_url(&client, &url, "test_key", &wav, vocabulary).await
    }

    #[tokio::test]
    async fn post_to_groq_happy_path_returns_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"text":"hello world","segments":[{"no_speech_prob":0.05}]}"#,
            ))
            .mount(&server)
            .await;

        let body = post_to_mock(&server, None).await.expect("happy path");
        assert!(body.contains("hello world"));

        let parsed = parse_groq_response(&body).expect("parse");
        assert_eq!(parsed.text, "hello world");
        assert!(parsed.min_no_speech.is_some());
        assert!((parsed.min_no_speech.unwrap() - 0.05).abs() < 1e-3);
    }

    #[tokio::test]
    async fn handles_401_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"Invalid API Key"}"#))
            .mount(&server)
            .await;

        let err = post_to_mock(&server, None).await.expect_err("401");
        match err {
            TranscriptionError::ApiError { status, body } => {
                assert_eq!(status, 401);
                assert!(body.contains("Invalid API Key"));
            }
            other => panic!("expected ApiError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_429_with_retry_after_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "30")
                    .set_body_string(r#"{"error":"rate limited"}"#),
            )
            .mount(&server)
            .await;

        let err = post_to_mock(&server, None).await.expect_err("429");
        match err {
            TranscriptionError::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, Some(30));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_429_without_retry_after_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(429).set_body_string(r#"{"error":"slow down"}"#))
            .mount(&server)
            .await;

        let err = post_to_mock(&server, None).await.expect_err("429 no header");
        match err {
            TranscriptionError::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, None);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_413_file_too_large() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(413).set_body_string(r#"{"error":"file too large"}"#))
            .mount(&server)
            .await;

        let err = post_to_mock(&server, None).await.expect_err("413");
        match err {
            TranscriptionError::ApiError { status, body } => {
                assert_eq!(status, 413);
                assert!(body.contains("file too large"));
            }
            other => panic!("expected ApiError(413), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_500_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
            .mount(&server)
            .await;

        let err = post_to_mock(&server, None).await.expect_err("500");
        match err {
            TranscriptionError::ApiError { status, body } => {
                assert_eq!(status, 500);
                assert!(body.contains("internal server error"));
            }
            other => panic!("expected ApiError(500), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_malformed_json_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let body = post_to_mock(&server, None).await.expect("200 body");
        // Status was 200 so the helper returned Ok; parse_groq_response
        // should reject the malformed body.
        if parse_groq_response(&body).is_ok() {
            panic!("malformed body should have failed to parse");
        }
    }

    #[tokio::test]
    async fn vocabulary_prompt_is_included_in_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/audio/transcriptions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"text":"with prompt","segments":[]}"#),
            )
            .mount(&server)
            .await;

        let vocab = vec!["TalkType".to_string(), "Tauri".to_string()];
        let body = post_to_mock(&server, Some(&vocab)).await.expect("ok");
        assert!(body.contains("with prompt"));
    }

    #[test]
    fn is_retryable_only_for_timeout_and_ratelimit() {
        assert!(is_retryable(&TranscriptionError::Timeout(30)));
        assert!(is_retryable(&TranscriptionError::RateLimited {
            retry_after_secs: None
        }));
        assert!(is_retryable(&TranscriptionError::RateLimited {
            retry_after_secs: Some(10)
        }));
        assert!(!is_retryable(&TranscriptionError::ApiKeyMissing(
            "groq".to_string()
        )));
        assert!(!is_retryable(&TranscriptionError::AudioTooSmall(500)));
        assert!(!is_retryable(&TranscriptionError::Offline));
        assert!(!is_retryable(&TranscriptionError::Busy));
        assert!(!is_retryable(&TranscriptionError::ApiError {
            status: 401,
            body: String::new()
        }));
        assert!(!is_retryable(&TranscriptionError::ConnectionRefused));
        assert!(!is_retryable(&TranscriptionError::DnsFailure(
            "x".to_string()
        )));
    }
}
