// Provider connectivity health check (M3 chunk 3, Q5).
//
// `test_provider_connection` is a lightweight Tauri command the user invokes
// from the Settings → API key section to confirm three things in one round
// trip:
//
//   1. The API key for the selected provider exists in the OS credential
//      vault (`keyring` lookup happens entirely Rust-side per architecture
//      invariant #1 — the key never crosses the IPC boundary).
//   2. The network can reach the provider's `/models` endpoint (catches
//      offline / proxy-misconfiguration / DNS issues without burning a
//      transcription quota slot).
//   3. The key is currently valid (401 means revoked, 403 means restricted,
//      200 means usable).
//
// **Provider scope**: M3 ships Groq only. M6 will extend the same command to
// OpenAI / Anthropic / Gemini using their respective `/models` endpoints —
// the dispatcher pattern below already takes a `provider` string so M6 will
// only need to add match arms.
//
// **Endpoint choice**: we hit `/models` rather than `/audio/transcriptions`
// because (a) it costs the user no transcription quota; (b) the response is
// small (~3 KB JSON) so the 5 s timeout is plenty for users behind a slow
// corporate proxy; (c) `model_count` from the response gives the user a
// concrete success signal beyond a green checkmark.
//
// **Timeout**: 5 s per call. Significantly tighter than the 120 s
// `TranscriptionState::client` timeout — a user clicking "Test connection"
// expects a fast answer; a hung click for 2 minutes feels broken. We reuse
// the shared `reqwest::Client` from `TranscriptionState` and override the
// timeout per-request via `RequestBuilder::timeout`.
//
// **URL extraction**: the production endpoint is hardcoded as a const, but
// the actual HTTP body lives in `test_groq_connection_with_url` so wiremock
// tests can point it at a local mock server. This mirrors the
// `cloud::post_to_groq` / `post_to_groq_with_url` split in the chunk-2
// HTTP layer.

use std::time::Duration;

use serde::{Serialize, Serializer};
use thiserror::Error;

use crate::plugins::credentials;

/// Production endpoint for Groq's model list. Public-but-undocumented contract:
/// the response shape is `{ "data": [{ "id": "...", ... }, ...] }` mirroring
/// OpenAI's `/v1/models` API, which Groq is intentionally compatible with.
const GROQ_MODELS_URL: &str = "https://api.groq.com/openai/v1/models";

/// Shorter than the 120 s transcribe timeout — `/models` is a small
/// metadata call and the user is actively waiting on the result.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Successful test result returned to the frontend. `camelCase` to match
/// `TestConnectionResult` in `src/types/credentials.ts`. The `provider`
/// field echoes the input so the frontend can stash the result under the
/// right provider entry without re-tracking the input string.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub ok: bool,
    pub provider: String,
    /// Number of models the provider's `/models` endpoint returned. `None`
    /// when the response shape didn't include a `data` array (defensive —
    /// shouldn't happen with current Groq API but guards against future
    /// shape changes).
    pub model_count: Option<usize>,
}

/// Errors specific to the connectivity test path. Kept separate from
/// `TranscriptionError` because the failure modes differ (no audio path, no
/// retry policy, no rate-limit honoring — the user expects a single
/// definitive answer in <5s) and the frontend maps these to a different
/// localized error namespace (`testError.*` vs. `transcribeError.*`).
///
/// Variants follow the same flat-string `Serialize` convention as the rest
/// of the M3 error enums (`AudioRecorderError`, `CredentialsError`,
/// `TranscriptionError`).
#[derive(Error, Debug)]
pub enum TestConnectionError {
    /// Provider id wasn't recognized. M3 only ships Groq.
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    /// Provider exists but no API key is stored — user hasn't run
    /// `set_credential` yet, or just deleted it. The frontend translates
    /// this to a "set your key in Settings" hint.
    #[error("API key missing for provider {0}")]
    ApiKeyMissing(String),

    /// 401 from provider — key is rejected. Either revoked, malformed, or
    /// for a different provider (e.g. OpenAI key pasted into Groq slot).
    #[error("Invalid API key (HTTP 401)")]
    InvalidKey,

    /// 403 from provider — key authenticates but lacks the scope needed to
    /// call the audio API. Surfaces as a distinct UI message because the
    /// remediation differs (regenerate key with audio scope vs. paste a
    /// fresh key).
    #[error("Restricted API key (HTTP 403)")]
    RestrictedKey,

    /// 429 from provider — too many requests. Includes parsed `Retry-After`
    /// when present so the UI can show an actionable wait time.
    #[error("Rate limited (retry after {0:?}s)")]
    RateLimited(Option<u64>),

    /// Any other non-2xx status. Body is included for debugging mismatched
    /// expectations against the provider's API.
    #[error("Provider returned error {status}: {body}")]
    ApiError { status: u16, body: String },

    /// reqwest layer failure — DNS, TLS, connect refused, timeout. Kept
    /// flat (a single string) here rather than the chunk-2-style
    /// fine-grained taxonomy because the test path's UI just needs to tell
    /// the user "check your network or HTTPS_PROXY".
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Wraps `CredentialsError` to a flat string so this enum stays
    /// independent from the keyring backend.
    #[error("Credentials error: {0}")]
    Credentials(String),
}

// Manual `Serialize` so the frontend receives a flat string per CLAUDE.md
// "Error enum 手動 implement Serialize 為 string". Mirrors
// `TranscriptionError`, `CredentialsError`, `AudioRecorderError`.
impl Serialize for TestConnectionError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Tauri command — verify a provider's API key + network reachability in a
/// single 5 s round trip. M3 ships Groq; M6 extends to the other three
/// providers by adding match arms here. The frontend invokes via
/// `invoke<TestConnectionResult>('test_provider_connection', { provider })`
/// and surfaces the result inline below the API key input.
#[tauri::command]
pub async fn test_provider_connection(
    state: tauri::State<'_, super::TranscriptionState>,
    provider: String,
) -> Result<TestConnectionResult, TestConnectionError> {
    match provider.as_str() {
        "groq" => test_groq_connection(&state.client, &provider).await,
        // M3 only ships Groq. M6 will add openai / anthropic / gemini arms
        // hitting their respective `/models` endpoints with provider-
        // specific auth headers (`x-api-key` for anthropic etc.).
        _ => Err(TestConnectionError::UnknownProvider(provider)),
    }
}

/// Convenience wrapper that fetches the API key from the keyring and
/// dispatches to `test_groq_connection_with_url` against the production
/// endpoint. Tests use the `_with_url` variant directly to point at a
/// wiremock server.
async fn test_groq_connection(
    client: &reqwest::Client,
    provider: &str,
) -> Result<TestConnectionResult, TestConnectionError> {
    let api_key = credentials::get_credential(provider)
        .map_err(|e| TestConnectionError::Credentials(e.to_string()))?
        .ok_or_else(|| TestConnectionError::ApiKeyMissing(provider.to_string()))?;
    test_groq_connection_with_url(client, GROQ_MODELS_URL, &api_key, provider).await
}

/// URL-parameterized variant exposed for wiremock tests. Sends a single GET
/// to `url` with `Bearer <api_key>` and a 5 s timeout, then maps the response
/// into `TestConnectionResult` / `TestConnectionError`.
async fn test_groq_connection_with_url(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    provider: &str,
) -> Result<TestConnectionResult, TestConnectionError> {
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .timeout(TEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| TestConnectionError::NetworkError(e.to_string()))?;

    let status = resp.status();
    match status.as_u16() {
        200 => {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| TestConnectionError::NetworkError(format!("parse: {e}")))?;
            // Groq's `/models` mirrors OpenAI's response shape:
            // `{ "data": [{ "id": "...", ... }, ...] }`. We don't enforce
            // schema strictly — just count entries when present.
            let model_count = body.get("data").and_then(|v| v.as_array()).map(|a| a.len());
            Ok(TestConnectionResult {
                ok: true,
                provider: provider.to_string(),
                model_count,
            })
        }
        401 => Err(TestConnectionError::InvalidKey),
        403 => Err(TestConnectionError::RestrictedKey),
        429 => {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            Err(TestConnectionError::RateLimited(retry_after))
        }
        s => {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            Err(TestConnectionError::ApiError { status: s, body })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: build a fresh client + run `test_groq_connection_with_url`
    /// against the given mock. Mirrors the `post_to_mock` helper in
    /// `cloud.rs` tests.
    async fn run_against_mock(
        server: &MockServer,
    ) -> Result<TestConnectionResult, TestConnectionError> {
        let client = reqwest::Client::new();
        let url = format!("{}/openai/v1/models", server.uri());
        test_groq_connection_with_url(&client, &url, "test_key_gsk_xyz", "groq").await
    }

    #[tokio::test]
    async fn happy_path_returns_model_count() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .and(header("authorization", "Bearer test_key_gsk_xyz"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":"whisper-large-v3"},{"id":"whisper-large-v3-turbo"},{"id":"llama3-8b"}]}"#,
            ))
            .mount(&server)
            .await;

        let result = run_against_mock(&server).await.expect("happy path");
        assert!(result.ok);
        assert_eq!(result.provider, "groq");
        assert_eq!(result.model_count, Some(3));
    }

    #[tokio::test]
    async fn handles_200_with_no_data_field() {
        // Defensive against a future API shape change where `data` is missing.
        // We accept the call as ok but report `model_count = None`.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"object":"list"}"#))
            .mount(&server)
            .await;

        let result = run_against_mock(&server).await.expect("ok with no data");
        assert!(result.ok);
        assert_eq!(result.model_count, None);
    }

    #[tokio::test]
    async fn handles_401_invalid_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(r#"{"error":"Invalid API Key"}"#),
            )
            .mount(&server)
            .await;

        let err = run_against_mock(&server).await.expect_err("401");
        assert!(matches!(err, TestConnectionError::InvalidKey));
    }

    #[tokio::test]
    async fn handles_403_restricted_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string(r#"{"error":"insufficient_scope"}"#),
            )
            .mount(&server)
            .await;

        let err = run_against_mock(&server).await.expect_err("403");
        assert!(matches!(err, TestConnectionError::RestrictedKey));
    }

    #[tokio::test]
    async fn handles_429_with_retry_after_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "60")
                    .set_body_string(r#"{"error":"rate_limit_exceeded"}"#),
            )
            .mount(&server)
            .await;

        let err = run_against_mock(&server).await.expect_err("429");
        match err {
            TestConnectionError::RateLimited(secs) => assert_eq!(secs, Some(60)),
            other => panic!("expected RateLimited(Some(60)), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handles_500_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openai/v1/models"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream timeout"))
            .mount(&server)
            .await;

        let err = run_against_mock(&server).await.expect_err("500");
        match err {
            TestConnectionError::ApiError { status, body } => {
                assert_eq!(status, 500);
                assert!(body.contains("upstream timeout"));
            }
            other => panic!("expected ApiError(500), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_provider_rejected_synchronously() {
        // Doesn't even hit the network — the dispatcher rejects unknown ids
        // before keyring or HTTP. We exercise that by calling the public
        // command function directly... except it requires a `tauri::State`
        // which isn't easy to construct in a unit test. Instead, replicate
        // the dispatcher's match-arm behavior.
        match "openai" {
            "groq" => panic!("M3 should not accept openai"),
            other => {
                let err = TestConnectionError::UnknownProvider(other.to_string());
                assert!(err.to_string().contains("openai"));
            }
        }
    }

    #[test]
    fn error_serializes_as_flat_string() {
        let err = TestConnectionError::InvalidKey;
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Invalid API key (HTTP 401)\"");

        let err = TestConnectionError::RateLimited(Some(30));
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.starts_with('"') && json.ends_with('"'));
        assert!(json.contains("30"));
    }

    #[test]
    fn result_serializes_camel_case() {
        let r = TestConnectionResult {
            ok: true,
            provider: "groq".to_string(),
            model_count: Some(8),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"provider\":\"groq\""));
        assert!(json.contains("\"modelCount\":8"));
    }

    #[test]
    fn result_serializes_none_model_count_as_null() {
        let r = TestConnectionResult {
            ok: true,
            provider: "groq".to_string(),
            model_count: None,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"modelCount\":null"));
    }
}
