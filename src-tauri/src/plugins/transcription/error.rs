// Transcription error type (M3 chunk 2).
//
// Per CLAUDE.md "Error enum 手動 implement Serialize 為 string": the frontend
// receives a flat string for every variant via the manual `Serialize` impl
// below. The error taxonomy was split fine-grained on purpose during the M3
// plan-time challenger pass — each variant maps to a distinct user-facing
// remediation in the UI (e.g. `Offline` → "check your network" vs.
// `ApiKeyMissing` → "set your Groq key in Settings").
//
// `classify_reqwest_error` lives here so cloud.rs (and future M6 llm_polish)
// share the same network-error → enum mapping. Best-effort heuristics on
// reqwest's error string for DNS / TLS detection — reqwest 0.12 doesn't
// expose typed predicates for those.

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors returned by the transcription Tauri commands and `transcribe_cloud_internal`.
///
/// Variants split into four buckets so the frontend can map each to a
/// localized remediation message without string-matching on Display:
///
///   * Data layer (NoAudioData / AudioTooSmall / FileTooLarge / Busy)
///   * Auth (ApiKeyMissing)
///   * Network (Offline / DnsFailure / TlsFailure / Timeout / ConnectionRefused / NetworkOther)
///   * Service (RateLimited / ApiError / ParseError)
///   * Internal (LockPoisoned / Credentials)
#[derive(Error, Debug)]
pub enum TranscriptionError {
    // ─── Data layer ───────────────────────────────────────────────────────
    /// `wav_buffer` was None (caller didn't `stop_recording` first).
    #[error("No audio data in buffer (call stop_recording first)")]
    NoAudioData,

    /// Buffer is below the 1 KB floor — likely a sub-50 ms tap rather than
    /// real audio. We reject before any keyring / network round trip.
    #[error("Audio too small to transcribe ({0} bytes; minimum 1000)")]
    AudioTooSmall(usize),

    /// Buffer exceeds Groq's 25 MB upload cap. The recording thread caps at
    /// the same constant (`MAX_WAV_BYTES`) so this is a safety net for any
    /// future code path that might bypass the recorder.
    #[error("Audio exceeds Groq 25 MB limit ({actual_bytes} bytes; max {max_bytes})")]
    FileTooLarge { actual_bytes: usize, max_bytes: usize },

    /// Another transcription is in flight. The dispatcher's AtomicBool guard
    /// rejects concurrent calls so we don't race on `wav_buffer` or saturate
    /// the user's Groq quota with double-pressed-hotkey duplicates.
    #[error("A previous transcription is still in progress")]
    Busy,

    // ─── Auth ─────────────────────────────────────────────────────────────
    /// Provider exists in the credentials allowlist but no key is stored.
    /// The string param echoes the provider id so the UI can show
    /// "Set your $provider key in Settings".
    #[error("API key missing for provider {0} — set it in Settings")]
    ApiKeyMissing(String),

    // ─── Network ──────────────────────────────────────────────────────────
    /// Best-effort detection of "no network connectivity". reqwest doesn't
    /// expose a typed predicate, so we currently fall through to
    /// `NetworkOther` and reserve this variant for a future preflight.
    #[error("Network appears offline")]
    Offline,

    /// DNS resolution failed (best-effort string-match heuristic).
    #[error("DNS lookup failed: {0}")]
    DnsFailure(String),

    /// TLS handshake / certificate validation failed.
    #[error("TLS handshake failed: {0}")]
    TlsFailure(String),

    /// Request exceeded the configured timeout. The duration is included so
    /// the UI can show "timed out after 120s" rather than a generic message.
    #[error("Request timed out after {0}s")]
    Timeout(u64),

    /// TCP connect was actively refused (e.g. nothing listening on the port).
    #[error("Connection refused by server")]
    ConnectionRefused,

    /// Catch-all for reqwest errors that don't match the more specific
    /// variants above. Includes the original error text for debugging.
    #[error("Other network error: {0}")]
    NetworkOther(String),

    // ─── Service ──────────────────────────────────────────────────────────
    /// Provider returned 429. `retry_after_secs` is parsed from the
    /// `Retry-After` HTTP header when present.
    #[error("Rate limited (retry after {retry_after_secs:?}s)")]
    RateLimited { retry_after_secs: Option<u64> },

    /// Provider returned a non-2xx, non-429 status. `body` is the raw response
    /// body capped to a sensible length by the caller. 401 means invalid key,
    /// 413 means file-too-large from the server side, etc.
    #[error("Provider returned error {status}: {body}")]
    ApiError { status: u16, body: String },

    /// Failed to deserialize the provider's JSON response. Body prefix is
    /// included by the caller for debugging mismatched response shapes.
    #[error("Failed to parse provider response: {0}")]
    ParseError(String),

    // ─── Internal ─────────────────────────────────────────────────────────
    /// One of the `Mutex` lock calls returned `PoisonError`. Should be
    /// effectively impossible but we surface it instead of panicking.
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),

    /// Wraps `CredentialsError` to a flat string at the IPC boundary so we
    /// don't need to make `TranscriptionError` `From<CredentialsError>` and
    /// drag the keyring backend into this module's error type.
    #[error("Credentials error: {0}")]
    Credentials(String),
}

// Manual `Serialize` so the frontend receives a flat string rather than a
// tagged enum object. CLAUDE.md "Error enum 手動 implement Serialize 為
// string". Mirrors `AudioRecorderError` / `CredentialsError`.
impl Serialize for TranscriptionError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Best-effort classification of a reqwest::Error into a typed
/// `TranscriptionError` variant. reqwest 0.12 only exposes `is_timeout()`,
/// `is_connect()`, `is_request()` etc.; for DNS / TLS distinction we fall
/// back to substring matching on the rendered error chain.
///
/// **Default timeout assumption**: when `is_timeout()` fires we don't have
/// access to the actual configured timeout from inside the helper, so we
/// surface 120 — matching `TranscriptionState::new`'s `Client` timeout. Future
/// work (M6 llm_polish) can plumb a custom value through if needed.
pub(crate) fn classify_reqwest_error(e: reqwest::Error) -> TranscriptionError {
    if e.is_timeout() {
        return TranscriptionError::Timeout(120);
    }
    if e.is_connect() {
        return TranscriptionError::ConnectionRefused;
    }
    let lower = e.to_string().to_lowercase();
    if lower.contains("dns") || lower.contains("name not resolved") || lower.contains("no such host")
    {
        return TranscriptionError::DnsFailure(e.to_string());
    }
    if lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
        || lower.contains("handshake")
    {
        return TranscriptionError::TlsFailure(e.to_string());
    }
    TranscriptionError::NetworkOther(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_emits_flat_string() {
        let err = TranscriptionError::ApiKeyMissing("groq".to_string());
        let json = serde_json::to_string(&err).unwrap();
        // Must be a quoted string, not an object/struct.
        assert!(json.starts_with('"'));
        assert!(json.ends_with('"'));
        assert!(json.contains("groq"));
    }

    #[test]
    fn file_too_large_includes_byte_counts() {
        let err = TranscriptionError::FileTooLarge {
            actual_bytes: 26_000_000,
            max_bytes: 25_000_000,
        };
        let s = err.to_string();
        assert!(s.contains("26000000"));
        assert!(s.contains("25000000"));
    }

    #[test]
    fn busy_serializes_as_flat_string() {
        let json = serde_json::to_string(&TranscriptionError::Busy).unwrap();
        assert_eq!(json, "\"A previous transcription is still in progress\"");
    }

    #[test]
    fn rate_limited_with_retry_after_renders_secs() {
        let err = TranscriptionError::RateLimited {
            retry_after_secs: Some(30),
        };
        let s = err.to_string();
        assert!(s.contains("30"));
    }

    #[test]
    fn rate_limited_without_retry_after_renders_none() {
        let err = TranscriptionError::RateLimited {
            retry_after_secs: None,
        };
        let s = err.to_string();
        // `{retry_after_secs:?}` formats `None` as `None`.
        assert!(s.contains("None"));
    }

    #[test]
    fn api_error_renders_status_and_body() {
        let err = TranscriptionError::ApiError {
            status: 401,
            body: "Invalid API Key".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("401"));
        assert!(s.contains("Invalid API Key"));
    }

    #[test]
    fn audio_too_small_renders_byte_count() {
        let err = TranscriptionError::AudioTooSmall(500);
        let s = err.to_string();
        assert!(s.contains("500"));
        assert!(s.contains("1000"));
    }
}
