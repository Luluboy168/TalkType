// LLM polish error type (M6 chunk 1).
//
// Per CLAUDE.md "Error enum 手動 implement Serialize 為 string": the frontend
// receives a flat string for every variant via the manual `Serialize` impl
// below. The error taxonomy is fine-grained on purpose so chunk 2 can map
// each variant to a localized remediation message without string-matching
// on `Display`, and chunk 3's HUD can route polish failures to a closed
// `PolishFailureReason` set (mirrors `src/types/llm.ts`
// `POLISH_FAILURE_REASONS` 12-tuple).
//
// **F18 deletion**: `LockPoisoned` is intentionally absent from this enum.
// `LlmPolishState` only holds an `AtomicBool` (`polish_busy`) plus a shared
// `reqwest::Client`; neither can be poisoned (atomics have no `PoisonError`
// path, and the client's internal state is `Arc`-shared with `RwLock` only
// inside reqwest's own connection pool, which surfaces as `reqwest::Error`
// not a poison). Adding the variant here would be dead code.
//
// **Mapping convention**: `From<&PolishError> for PolishFailureReason` is
// the only place where the per-variant → `PolishFailureReason` mapping is
// defined. Reviewer hint: when a new `PolishError` variant lands later,
// extend the `match` arms in `From<&PolishError>` rather than introducing a
// parallel mapping site elsewhere.

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors returned by the `polish_text` Tauri command and its supporting
/// helpers. Variants split into seven buckets so the frontend can map each
/// to a distinct localized remediation message:
///
///   * Auth          — `ApiKeyMissing` / `Credentials`
///   * State         — `Disabled` / `Busy` / `Cancelled`
///   * Input         — `EmptyInput` / `InvalidPromptLength`
///   * Network       — `Offline` / `Timeout` / `TlsFailure` / `DnsFailure`
///     / `ConnectionRefused` / `NetworkOther`
///   * Service       — `RateLimited` / `ApiError`
///   * Output        — `EmptyResponse` / `Truncated` / `ImplausibleOutput`
///     / `SafetyBlocked`
///   * Internal      — `ParseError`
///
/// Manually implements `Serialize` to a flat string per CLAUDE.md.
#[derive(Error, Debug)]
pub enum PolishError {
    // ─── Auth ─────────────────────────────────────────────────────────────
    /// Provider exists in the credentials allowlist but no key is stored.
    /// The string param echoes the provider id so the UI can show
    /// "Set your $provider key in Settings".
    #[error("API key missing for provider {provider} — set it in Settings")]
    ApiKeyMissing { provider: String },

    /// Wraps `CredentialsError` to a flat string at the IPC boundary so we
    /// don't make `PolishError` `From<CredentialsError>` and drag the
    /// keyring backend into this module's error type.
    #[error("Credentials error: {0}")]
    Credentials(String),

    // ─── State ────────────────────────────────────────────────────────────
    /// Polish is explicitly disabled in settings (`llm_polish_enabled =
    /// Some(false)`). Defensive — the frontend should have skipped invoking
    /// `polish_text` in this case.
    #[error("LLM polish is disabled in settings")]
    Disabled,

    /// Another `polish_text` invocation is in flight. The orchestrator's
    /// AtomicBool guard rejects concurrent calls so the frontend retry loop
    /// (Decision #5 / F34) doesn't race itself.
    #[error("A previous polish request is still in progress")]
    Busy,

    /// Explicit cancellation. F23 reserves this variant for future
    /// ESC-during-enhancing support; M6 chunk 2 currently does not surface
    /// it (ESC during enhancing is a no-op + `console.warn`). Kept here so
    /// chunk 2's `PolishFailureReason` mapping can be exhaustive without
    /// later schema churn.
    #[allow(dead_code)]
    #[error("Polish cancelled")]
    Cancelled,

    // ─── Input ────────────────────────────────────────────────────────────
    /// Raw transcript was empty / whitespace-only. Defensive — the
    /// frontend should have skipped invoking `polish_text` in this case.
    #[error("Empty input — nothing to polish")]
    EmptyInput,

    /// Custom prompt exceeded the 1000-char cap. `actual` and `max` use
    /// `chars().count()` (not byte length) so CJK content is budgeted by
    /// visible character count. F11.
    #[error("Custom prompt is too long ({actual} chars; max {max})")]
    InvalidPromptLength { actual: usize, max: usize },

    // ─── Network (mirror M3 `TranscriptionError` taxonomy) ────────────────
    /// Reserved for a future preflight that detects "no network
    /// connectivity" before issuing the request. M6 currently never returns
    /// this variant — reqwest errors fall through to `NetworkOther` /
    /// `DnsFailure` / `ConnectionRefused`.
    #[allow(dead_code)]
    #[error("Network appears offline")]
    Offline,

    /// Request exceeded the per-provider timeout. The duration (3s for Groq,
    /// 15s for Gemini / OpenRouter / NVIDIA per chunk-1 spec) is included so
    /// the UI can show "timed out after 3s" rather than a generic message.
    #[error("Request timed out after {0}s")]
    Timeout(u64),

    /// TLS handshake / certificate validation failed.
    #[error("TLS handshake failed: {0}")]
    TlsFailure(String),

    /// DNS resolution failed.
    #[error("DNS lookup failed: {0}")]
    DnsFailure(String),

    /// TCP connect was actively refused.
    #[error("Connection refused by server")]
    ConnectionRefused,

    /// Catch-all for reqwest errors that don't match the more specific
    /// variants above.
    #[error("Other network error: {0}")]
    NetworkOther(String),

    // ─── Service ──────────────────────────────────────────────────────────
    /// Provider returned 429. `retry_after_secs` is parsed from the
    /// `Retry-After` HTTP header when present.
    #[error("Rate limited (retry after {retry_after_secs:?}s)")]
    RateLimited { retry_after_secs: Option<u64> },

    /// Provider returned a non-2xx, non-429 status. `body` is the raw
    /// response body; `extracted_message` is the per-provider error message
    /// extracted from the JSON shape (F14) so the frontend i18n can show a
    /// human-readable error instead of pretty-printing the entire JSON
    /// blob.
    #[error("Provider returned error {status}: {extracted_message}")]
    ApiError {
        status: u16,
        body: String,
        extracted_message: String,
    },

    // ─── Output ───────────────────────────────────────────────────────────
    /// LLM responded successfully but the polished text was empty /
    /// whitespace-only after trimming. F8 sanity check.
    #[error("Provider returned an empty response")]
    EmptyResponse,

    /// `finish_reason: length` (OAI-compat) or `MAX_TOKENS` (Gemini) and the
    /// truncated output is shorter than the raw input — implies the LLM
    /// exceeded the output token budget mid-response and we'd be pasting a
    /// truncated half-thought. F9.
    #[error("Polished output truncated ({polished_len} chars vs raw {raw_len})")]
    Truncated { polished_len: usize, raw_len: usize },

    /// Sanity check failed: output is more than 3× the input length, or
    /// starts with refusal markers like "I cannot" / "As an AI". Implies
    /// the LLM hallucinated rather than polishing. F8.
    #[error("Polished output is implausible ({input_len} → {output_len} chars)")]
    ImplausibleOutput { input_len: usize, output_len: usize },

    /// Gemini `finishReason: SAFETY | RECITATION` or OAI-compat
    /// `finish_reason: content_filter`. The reason string echoes the raw
    /// upstream code so the frontend can show "Gemini blocked: SAFETY" /
    /// "OpenRouter blocked: content_filter". F7.
    #[error("Provider blocked the response: {reason}")]
    SafetyBlocked { reason: String },

    // ─── Internal ─────────────────────────────────────────────────────────
    /// Failed to deserialize the provider's JSON response, or unrecognized
    /// settings field value (e.g. unknown provider id passed through from
    /// `Settings`). Body prefix is included by the caller for debugging
    /// mismatched response shapes.
    #[error("Failed to parse provider response: {0}")]
    ParseError(String),
}

// Manual `Serialize` so the frontend receives a flat string rather than a
// tagged enum object. CLAUDE.md "Error enum 手動 implement Serialize 為
// string". Mirrors `TranscriptionError`, `CredentialsError`,
// `AudioRecorderError`.
impl Serialize for PolishError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Closed enum mirror of `src/types/llm.ts` `POLISH_FAILURE_REASONS`. Used
/// in the `polish:failed-fallback` event payload so the chunk-3 HUD warning
/// can branch on the closed set without falling back to a default.
///
/// Each variant maps 1:1 to a string in the TS tuple; the `as_str` method
/// produces the wire format. The `From<&PolishError>` impl below is the
/// single chokepoint that classifies a `PolishError` into one of these
/// reasons — when adding new `PolishError` variants, extend the `match`
/// arms there rather than introducing a parallel mapping site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolishFailureReason {
    Network,
    RateLimited,
    Auth,
    Parse,
    Timeout,
    ServerError,
    SafetyBlocked,
    EmptyResponse,
    Truncated,
    ImplausibleOutput,
    Busy,
    Cancelled,
}

impl PolishFailureReason {
    /// Wire-format string used in the `polish:failed-fallback` event
    /// payload. Mirrors the TS const tuple verbatim — no spaces, no
    /// title-case. Stays in sync with `POLISH_FAILURE_REASONS` in
    /// `src/types/llm.ts`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::RateLimited => "rate_limited",
            Self::Auth => "auth",
            Self::Parse => "parse",
            Self::Timeout => "timeout",
            Self::ServerError => "server_error",
            Self::SafetyBlocked => "safety_blocked",
            Self::EmptyResponse => "empty_response",
            Self::Truncated => "truncated",
            Self::ImplausibleOutput => "implausible_output",
            Self::Busy => "busy",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Map a `PolishError` to its `PolishFailureReason`. Single chokepoint —
/// reviewer can grep `From<&PolishError>` to verify every new variant has
/// a mapping.
///
/// **HTTP status mapping for `ApiError`**:
///   * 401 / 403  → `Auth`         (key invalid / wrong scope)
///   * 4xx other  → `Parse`        (malformed request, the frontend can't
///     recover from a 400 by retrying)
///   * 5xx        → `ServerError`  (transient — frontend retry policy can
///     re-invoke once if `retry_enabled`)
///
/// Network / data variants fold into `Network`. Settings-issue variants
/// (`Disabled` / `EmptyInput` / `Credentials` / `InvalidPromptLength`) fold
/// into `Parse` since they're "request-level invalid" from the failure-
/// fallback HUD perspective; chunk 3's HUD doesn't need to distinguish a
/// settings-error from a parse-error remediation flow.
impl From<&PolishError> for PolishFailureReason {
    fn from(err: &PolishError) -> Self {
        use PolishError::*;
        match err {
            ApiKeyMissing { .. } => Self::Auth,
            Busy => Self::Busy,
            Cancelled => Self::Cancelled,
            EmptyResponse => Self::EmptyResponse,
            Truncated { .. } => Self::Truncated,
            ImplausibleOutput { .. } => Self::ImplausibleOutput,
            SafetyBlocked { .. } => Self::SafetyBlocked,
            RateLimited { .. } => Self::RateLimited,
            Timeout(_) => Self::Timeout,
            Offline | DnsFailure(_) | ConnectionRefused | NetworkOther(_) | TlsFailure(_) => {
                Self::Network
            }
            ApiError { status, .. } if *status == 401 || *status == 403 => Self::Auth,
            ApiError { status, .. } if (500..600).contains(status) => Self::ServerError,
            ApiError { .. }
            | ParseError(_)
            | InvalidPromptLength { .. }
            | Disabled
            | EmptyInput
            | Credentials(_) => Self::Parse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_emits_flat_string() {
        let err = PolishError::ApiKeyMissing {
            provider: "groq".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.starts_with('"'));
        assert!(json.ends_with('"'));
        assert!(json.contains("groq"));
    }

    #[test]
    fn serialize_busy_renders_static_string() {
        let json = serde_json::to_string(&PolishError::Busy).unwrap();
        assert_eq!(json, "\"A previous polish request is still in progress\"");
    }

    #[test]
    fn rate_limited_renders_retry_after_secs() {
        let err = PolishError::RateLimited {
            retry_after_secs: Some(2),
        };
        let s = err.to_string();
        assert!(s.contains('2'));

        let err = PolishError::RateLimited {
            retry_after_secs: None,
        };
        let s = err.to_string();
        assert!(s.contains("None"));
    }

    #[test]
    fn api_error_renders_status_and_extracted_message() {
        let err = PolishError::ApiError {
            status: 500,
            body: r#"{"error":{"message":"internal"}}"#.to_string(),
            extracted_message: "internal".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("500"));
        assert!(s.contains("internal"));
    }

    #[test]
    fn invalid_prompt_length_renders_actual_and_max() {
        let err = PolishError::InvalidPromptLength {
            actual: 1500,
            max: 1000,
        };
        let s = err.to_string();
        assert!(s.contains("1500"));
        assert!(s.contains("1000"));
    }

    #[test]
    fn truncated_renders_lengths() {
        let err = PolishError::Truncated {
            polished_len: 50,
            raw_len: 200,
        };
        let s = err.to_string();
        assert!(s.contains("50"));
        assert!(s.contains("200"));
    }

    #[test]
    fn safety_blocked_renders_reason() {
        let err = PolishError::SafetyBlocked {
            reason: "RECITATION".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("RECITATION"));
    }

    // ─── PolishFailureReason ──────────────────────────────────────────────

    #[test]
    fn polish_failure_reason_as_str_matches_ts_tuple() {
        // Each enum variant must produce the exact TS-tuple string. Mirrors
        // `POLISH_FAILURE_REASONS` in `src/types/llm.ts` line-by-line.
        assert_eq!(PolishFailureReason::Network.as_str(), "network");
        assert_eq!(PolishFailureReason::RateLimited.as_str(), "rate_limited");
        assert_eq!(PolishFailureReason::Auth.as_str(), "auth");
        assert_eq!(PolishFailureReason::Parse.as_str(), "parse");
        assert_eq!(PolishFailureReason::Timeout.as_str(), "timeout");
        assert_eq!(PolishFailureReason::ServerError.as_str(), "server_error");
        assert_eq!(
            PolishFailureReason::SafetyBlocked.as_str(),
            "safety_blocked"
        );
        assert_eq!(
            PolishFailureReason::EmptyResponse.as_str(),
            "empty_response"
        );
        assert_eq!(PolishFailureReason::Truncated.as_str(), "truncated");
        assert_eq!(
            PolishFailureReason::ImplausibleOutput.as_str(),
            "implausible_output"
        );
        assert_eq!(PolishFailureReason::Busy.as_str(), "busy");
        assert_eq!(PolishFailureReason::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn from_polish_error_auth_variants() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiKeyMissing {
                provider: "groq".to_string(),
            }),
            PolishFailureReason::Auth
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 401,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::Auth
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 403,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::Auth
        );
    }

    #[test]
    fn from_polish_error_server_error_for_5xx() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 500,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::ServerError
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 503,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::ServerError
        );
        // Boundary: 599 is still server error (matches contains 500..600).
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 599,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::ServerError
        );
        // 600 is NOT in 500..600 — folds into Parse.
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 600,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::Parse
        );
    }

    #[test]
    fn from_polish_error_parse_for_4xx_other() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 400,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::Parse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ApiError {
                status: 422,
                body: String::new(),
                extracted_message: String::new(),
            }),
            PolishFailureReason::Parse
        );
    }

    #[test]
    fn from_polish_error_network_variants() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::Offline),
            PolishFailureReason::Network
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::DnsFailure("dns".into())),
            PolishFailureReason::Network
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ConnectionRefused),
            PolishFailureReason::Network
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::NetworkOther("oops".into())),
            PolishFailureReason::Network
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::TlsFailure("tls".into())),
            PolishFailureReason::Network
        );
    }

    #[test]
    fn from_polish_error_state_variants() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::Busy),
            PolishFailureReason::Busy
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::Cancelled),
            PolishFailureReason::Cancelled
        );
    }

    #[test]
    fn from_polish_error_output_variants() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::EmptyResponse),
            PolishFailureReason::EmptyResponse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::Truncated {
                polished_len: 1,
                raw_len: 100,
            }),
            PolishFailureReason::Truncated
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ImplausibleOutput {
                input_len: 10,
                output_len: 1000,
            }),
            PolishFailureReason::ImplausibleOutput
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::SafetyBlocked {
                reason: "SAFETY".to_string(),
            }),
            PolishFailureReason::SafetyBlocked
        );
    }

    #[test]
    fn from_polish_error_settings_issues_fold_to_parse() {
        // Disabled / EmptyInput / Credentials / InvalidPromptLength all map
        // to Parse — chunk 3's HUD doesn't need to distinguish settings
        // errors from response-shape errors at the failure-fallback layer.
        assert_eq!(
            PolishFailureReason::from(&PolishError::Disabled),
            PolishFailureReason::Parse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::EmptyInput),
            PolishFailureReason::Parse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::Credentials("err".into())),
            PolishFailureReason::Parse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::InvalidPromptLength {
                actual: 1500,
                max: 1000,
            }),
            PolishFailureReason::Parse
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::ParseError("bad".into())),
            PolishFailureReason::Parse
        );
    }

    #[test]
    fn from_polish_error_timeout_and_rate_limit() {
        assert_eq!(
            PolishFailureReason::from(&PolishError::Timeout(3)),
            PolishFailureReason::Timeout
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::RateLimited {
                retry_after_secs: None,
            }),
            PolishFailureReason::RateLimited
        );
        assert_eq!(
            PolishFailureReason::from(&PolishError::RateLimited {
                retry_after_secs: Some(2),
            }),
            PolishFailureReason::RateLimited
        );
    }
}
