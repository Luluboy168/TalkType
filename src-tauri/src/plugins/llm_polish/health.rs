// Provider connectivity health checks for the 3 non-Groq polish providers
// (M6 chunk 1, F5 + F13 + F20).
//
// **F20 file split**: this module ships the OpenRouter / NVIDIA NIM /
// Gemini test paths. Groq's `test_groq_connection` stays in
// `transcription/health.rs` (M3 chunk 3) because it's the only provider
// shared between Whisper transcription and LLM polish — moving it would
// break M3's existing dispatcher contract.
//
// `lib.rs::test_provider_connection` (existing M3 command) gets updated to
// dispatch by provider id: Groq → `transcription::health`, the other 3 →
// this module.
//
// Reuses M3's `TestConnectionResult` + `TestConnectionError` shapes
// (re-exported via `crate::plugins::transcription::health`) so the
// frontend gets a single error namespace rather than fragmenting per
// provider.
//
// **F5 endpoints** (lifted from kickoff log Decision #4 table):
//   * OpenRouter: `GET https://openrouter.ai/api/v1/models`
//                  Bearer auth + HTTP-Referer + X-Title (F13)
//   * NVIDIA NIM: `GET https://integrate.api.nvidia.com/v1/models`
//                  Bearer auth
//   * Gemini:     `GET https://generativelanguage.googleapis.com/v1beta/models`
//                  `x-goog-api-key` header (F5: never `?key=` query)

use std::time::Duration;

use crate::plugins::credentials;
use crate::plugins::transcription::health::{TestConnectionError, TestConnectionResult};

const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const NVIDIA_MODELS_URL: &str = "https://integrate.api.nvidia.com/v1/models";
const GEMINI_MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

const OPENROUTER_REFERER: &str = "https://github.com/Luluboy168/TalkType";
const OPENROUTER_TITLE: &str = "TalkType";

/// Same 5s budget as M3's test_groq_connection. User clicked "Test
/// connection" and is actively waiting; longer than 5s feels broken.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Verify OpenRouter API key + reachability via `GET /api/v1/models`.
/// Sends Bearer auth + HTTP-Referer + X-Title headers (F13) so OpenRouter
/// can attribute traffic to TalkType.
pub async fn test_openrouter_connection(
    client: &reqwest::Client,
) -> Result<TestConnectionResult, TestConnectionError> {
    const PROVIDER: &str = "openrouter";
    let api_key = credentials::get_credential(PROVIDER)
        .map_err(|e| TestConnectionError::Credentials(e.to_string()))?
        .ok_or_else(|| TestConnectionError::ApiKeyMissing(PROVIDER.to_string()))?;
    test_openrouter_with_url(client, OPENROUTER_MODELS_URL, &api_key, PROVIDER).await
}

async fn test_openrouter_with_url(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    provider: &str,
) -> Result<TestConnectionResult, TestConnectionError> {
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .header("HTTP-Referer", OPENROUTER_REFERER)
        .header("X-Title", OPENROUTER_TITLE)
        .timeout(TEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| TestConnectionError::NetworkError(e.to_string()))?;

    classify_models_response(resp, provider).await
}

/// Verify NVIDIA NIM API key + reachability via `GET /v1/models`. Bearer
/// auth — no extra headers (NVIDIA NIM doesn't have an OpenRouter-style
/// attribution requirement).
pub async fn test_nvidia_connection(
    client: &reqwest::Client,
) -> Result<TestConnectionResult, TestConnectionError> {
    const PROVIDER: &str = "nvidia";
    let api_key = credentials::get_credential(PROVIDER)
        .map_err(|e| TestConnectionError::Credentials(e.to_string()))?
        .ok_or_else(|| TestConnectionError::ApiKeyMissing(PROVIDER.to_string()))?;
    test_nvidia_with_url(client, NVIDIA_MODELS_URL, &api_key, PROVIDER).await
}

async fn test_nvidia_with_url(
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

    classify_models_response(resp, provider).await
}

/// Verify Gemini API key + reachability via `GET /v1beta/models`. F5
/// **header-only** auth — no `?key=` query string (the URL would leak
/// into proxy logs / browser history). Caller-side asserts cover the
/// "no key= in URL" invariant in tests.
pub async fn test_gemini_connection(
    client: &reqwest::Client,
) -> Result<TestConnectionResult, TestConnectionError> {
    const PROVIDER: &str = "gemini";
    let api_key = credentials::get_credential(PROVIDER)
        .map_err(|e| TestConnectionError::Credentials(e.to_string()))?
        .ok_or_else(|| TestConnectionError::ApiKeyMissing(PROVIDER.to_string()))?;
    test_gemini_with_url(client, GEMINI_MODELS_URL, &api_key, PROVIDER).await
}

async fn test_gemini_with_url(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    provider: &str,
) -> Result<TestConnectionResult, TestConnectionError> {
    let resp = client
        .get(url)
        .header("x-goog-api-key", api_key)
        .timeout(TEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| TestConnectionError::NetworkError(e.to_string()))?;

    // Gemini's `/v1beta/models` returns `{"models":[{...}, ...]}` (note:
    // `models` key, not `data` like the OAI-compat providers). We handle
    // both shapes in `classify_models_response` so the user gets a
    // model_count regardless.
    classify_models_response(resp, provider).await
}

/// Shared response classifier for `/models` GET endpoints. Returns
/// `TestConnectionResult` on 2xx with a best-effort `model_count` parse
/// (handles both `data: [...]` and `models: [...]` response shapes).
/// Maps non-2xx to typed errors mirroring `transcription::health`'s
/// taxonomy.
async fn classify_models_response(
    resp: reqwest::Response,
    provider: &str,
) -> Result<TestConnectionResult, TestConnectionError> {
    let status = resp.status();
    match status.as_u16() {
        200 => {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| TestConnectionError::NetworkError(format!("parse: {e}")))?;
            // OAI-compat providers (Groq / OpenRouter / NVIDIA) use
            // `data: [...]`; Gemini uses `models: [...]`. We fall through
            // to whichever is present.
            let model_count = body
                .get("data")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .or_else(|| {
                    body.get("models")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                });
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
    use wiremock::matchers::{header, method, path, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ─── OpenRouter ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn openrouter_happy_path_returns_model_count() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .and(header("authorization", "Bearer test_key"))
            .and(header(
                "http-referer",
                "https://github.com/Luluboy168/TalkType",
            ))
            .and(header("x-title", "TalkType"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":"meta-llama/llama-3.3-70b-instruct:free"},{"id":"qwen/qwen-2.5-72b-instruct:free"}]}"#,
            ))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/models", server.uri());
        let result = test_openrouter_with_url(&client, &url, "test_key", "openrouter")
            .await
            .expect("happy");
        assert!(result.ok);
        assert_eq!(result.provider, "openrouter");
        assert_eq!(result.model_count, Some(2));
    }

    #[tokio::test]
    async fn openrouter_401_invalid_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"unauth"}"#))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/models", server.uri());
        let err = test_openrouter_with_url(&client, &url, "bad_key", "openrouter")
            .await
            .expect_err("401");
        assert!(matches!(err, TestConnectionError::InvalidKey));
    }

    #[tokio::test]
    async fn openrouter_429_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "30")
                    .set_body_string(r#"{"error":"rate limited"}"#),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/models", server.uri());
        let err = test_openrouter_with_url(&client, &url, "test_key", "openrouter")
            .await
            .expect_err("429");
        match err {
            TestConnectionError::RateLimited(secs) => assert_eq!(secs, Some(30)),
            other => panic!("expected RateLimited(Some(30)), got {other:?}"),
        }
    }

    // ─── NVIDIA NIM ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn nvidia_happy_path_returns_model_count() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer test_key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":"meta/llama-3.3-70b-instruct"},{"id":"nvidia/llama-3.1-nemotron-70b-instruct"}]}"#,
            ))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1/models", server.uri());
        let result = test_nvidia_with_url(&client, &url, "test_key", "nvidia")
            .await
            .expect("happy");
        assert!(result.ok);
        assert_eq!(result.model_count, Some(2));
    }

    #[tokio::test]
    async fn nvidia_401_invalid_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1/models", server.uri());
        let err = test_nvidia_with_url(&client, &url, "bad", "nvidia")
            .await
            .expect_err("401");
        assert!(matches!(err, TestConnectionError::InvalidKey));
    }

    #[tokio::test]
    async fn nvidia_429_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1/models", server.uri());
        let err = test_nvidia_with_url(&client, &url, "test", "nvidia")
            .await
            .expect_err("429");
        assert!(matches!(err, TestConnectionError::RateLimited(_)));
    }

    // ─── Gemini ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn gemini_happy_path_uses_header_auth_and_no_query_string() {
        // F5: Gemini auth MUST be header `x-goog-api-key`; query string
        // `?key=...` is forbidden because the URL leaks into proxy logs
        // and browser history.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .and(header("x-goog-api-key", "test_key"))
            .and(query_param_is_missing("key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"models":[{"name":"models/gemini-2.0-flash"},{"name":"models/gemini-1.5-flash"}]}"#,
            ))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1beta/models", server.uri());
        let result = test_gemini_with_url(&client, &url, "test_key", "gemini")
            .await
            .expect("happy");
        assert!(result.ok);
        assert_eq!(result.provider, "gemini");
        assert_eq!(result.model_count, Some(2));
    }

    #[tokio::test]
    async fn gemini_401_invalid_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .and(header("x-goog-api-key", "bad"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(r#"{"error":{"message":"bad key"}}"#),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1beta/models", server.uri());
        let err = test_gemini_with_url(&client, &url, "bad", "gemini")
            .await
            .expect_err("401");
        assert!(matches!(err, TestConnectionError::InvalidKey));
    }

    #[tokio::test]
    async fn gemini_429_rate_limited_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "60")
                    .set_body_string(r#"{"error":{"message":"quota"}}"#),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1beta/models", server.uri());
        let err = test_gemini_with_url(&client, &url, "test_key", "gemini")
            .await
            .expect_err("429");
        match err {
            TestConnectionError::RateLimited(secs) => assert_eq!(secs, Some(60)),
            other => panic!("expected RateLimited(Some(60)), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn gemini_500_server_error_includes_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream timeout"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/v1beta/models", server.uri());
        let err = test_gemini_with_url(&client, &url, "test_key", "gemini")
            .await
            .expect_err("500");
        match err {
            TestConnectionError::ApiError { status, body } => {
                assert_eq!(status, 500);
                assert!(body.contains("upstream"));
            }
            other => panic!("expected ApiError(500), got {other:?}"),
        }
    }
}
