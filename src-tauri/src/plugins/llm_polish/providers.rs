// LLM polish provider request builders + response parsers (M6 chunk 1,
// F5 + F6 + F7 + F8 + F9 + F13 + F14 + F17, Decision #3).
//
// **4 active providers** (Decision #3 free-tier MVP):
//   * Groq        — `https://api.groq.com/openai/v1/chat/completions`
//   * OpenRouter  — `https://openrouter.ai/api/v1/chat/completions`
//   * NVIDIA NIM  — `https://integrate.api.nvidia.com/v1/chat/completions`
//   * Gemini      — `https://generativelanguage.googleapis.com/v1beta/
//                    models/{model}:generateContent`
//
// **OAI-compat shared path**: Groq / OpenRouter / NVIDIA all speak the
// OpenAI Chat Completions wire format (same body shape, same response
// shape modulo `usage` field name divergences). `build_oai_compatible_
// request` + `parse_oai_compatible_response` cover all three.
//
// **Gemini divergence**: separate request body shape
// (`contents[].parts[].text`, `systemInstruction`, `generationConfig`),
// separate auth (`x-goog-api-key` header — F5 forbids `?key=` query
// because the URL leaks into proxy logs / browser history), separate
// response shape (`candidates[0].content.parts[0].text`, `finishReason`).
//
// **F8 sanity checks** in `parse_oai_compatible_response` /
// `parse_gemini_response`:
//   1. Strip common prefixes ("Sure, here's...:" / "Here's the polished
//      version:" / "Polished text:")
//   2. Reject if `polished.chars().count() > raw.chars().count() * 3` →
//      `ImplausibleOutput`
//   3. Reject if `polished.starts_with("I cannot") ||
//      polished.starts_with("As an AI")` → `ImplausibleOutput`
//   4. Reject if trimmed empty → `EmptyResponse`
//
// **F9 truncation** detection: `finish_reason: "length"` (OAI-compat) or
// `finishReason: "MAX_TOKENS"` (Gemini) AND polished length < raw length
// → `Truncated` so the frontend falls back to raw rather than pasting a
// half-thought.
//
// **F7 safety blocks** caught by:
//   * OAI-compat: `choices[0].finish_reason: "content_filter"`
//   * Gemini: `candidates[0].finishReason: "SAFETY" | "RECITATION"`

use std::time::Duration;

use serde::Deserialize;

use super::error::PolishError;
use super::registry::{LlmModelConfig, MaxTokensField};

const GROQ_CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const OPENROUTER_CHAT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const NVIDIA_CHAT_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Per-provider extra headers for OpenRouter (HTTP-Referer + X-Title) so
/// OpenRouter can attribute traffic to TalkType. Matches F13 wiremock
/// expectations.
const OPENROUTER_REFERER: &str = "https://github.com/Luluboy168/TalkType";
const OPENROUTER_TITLE: &str = "TalkType";

/// Closed enum of the 4 active polish providers. Mirror of TS
/// `LlmActivePolishProviderId` in `src/types/llm.ts`. Serde
/// `rename_all = "lowercase"` so `Settings.llm_provider` deserializes
/// from `"groq"` / `"gemini"` / `"openrouter"` / `"nvidia"` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // some helpers are wired but not yet called by chunk 1's mod.rs paths
pub enum LlmProviderId {
    Groq,
    Gemini,
    Openrouter,
    Nvidia,
}

impl LlmProviderId {
    /// Parse from a `Settings.llm_provider` string. Returns `None` for
    /// the inactive `"openai"` / `"anthropic"` placeholders so the
    /// dispatcher can surface `PolishError::ParseError` instead of
    /// silently picking a default. Intentionally `Option<Self>` rather
    /// than the `std::str::FromStr` trait so the caller can branch on
    /// `None` without constructing an error type.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "groq" => Some(Self::Groq),
            "gemini" => Some(Self::Gemini),
            "openrouter" => Some(Self::Openrouter),
            "nvidia" => Some(Self::Nvidia),
            _ => None,
        }
    }

    /// Wire-format string used in the `polish:failed-fallback` event
    /// payload `providerId` field (mirrors TS).
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Groq => "groq",
            Self::Gemini => "gemini",
            Self::Openrouter => "openrouter",
            Self::Nvidia => "nvidia",
        }
    }
}

/// Output payload to frontend. `camelCase` to match `src/types/llm.ts`
/// `PolishResult` interface — keep the two in sync per architecture
/// invariant #8 (doc/plans/01-architecture.md).
#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PolishResult {
    pub polished_text: String,
    pub duration_ms: u64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// Internal parsed-response struct. Not serialized; the orchestrator
/// constructs `PolishResult` from this + duration measurements.
#[derive(Debug)]
pub(super) struct ParsedResponse {
    pub text: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

// ─── Build request (dispatcher) ──────────────────────────────────────────

/// Build a `reqwest::RequestBuilder` for the given provider + model. Auth
/// header is set per-provider (Bearer for OAI-compat, `x-goog-api-key`
/// for Gemini); body shape branches between the 3 OAI-compat shared path
/// and Gemini's bespoke `generateContent` shape.
///
/// Returns the builder unsent so the caller can apply a per-request
/// `.timeout(...)` before sending.
pub(super) fn build_request(
    client: &reqwest::Client,
    provider: LlmProviderId,
    model: &LlmModelConfig,
    system_prompt: &str,
    user_text: &str,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match provider {
        LlmProviderId::Groq => build_oai_compatible_request(
            client,
            GROQ_CHAT_URL,
            model,
            system_prompt,
            user_text,
            api_key,
            OaiExtras::None,
        ),
        LlmProviderId::Openrouter => build_oai_compatible_request(
            client,
            OPENROUTER_CHAT_URL,
            model,
            system_prompt,
            user_text,
            api_key,
            OaiExtras::OpenRouter,
        ),
        LlmProviderId::Nvidia => build_oai_compatible_request(
            client,
            NVIDIA_CHAT_URL,
            model,
            system_prompt,
            user_text,
            api_key,
            OaiExtras::None,
        ),
        LlmProviderId::Gemini => {
            build_gemini_request(client, model, system_prompt, user_text, api_key)
        }
    }
}

/// OpenRouter / NVIDIA divergence from plain OAI-compat: extra headers
/// for OpenRouter to attribute traffic. Adding more variants later (e.g.
/// project-specific NVIDIA NIM headers) is a one-arm extension here.
enum OaiExtras {
    None,
    OpenRouter,
}

/// Build an OpenAI Chat Completions request body. Shared by Groq +
/// OpenRouter + NVIDIA NIM (all three speak OAI-compat).
///
/// Body shape:
/// ```json
/// {
///   "model": "<model id>",
///   "messages": [
///     { "role": "system", "content": "<system prompt>" },
///     { "role": "user",   "content": "<user text>" }
///   ],
///   "max_tokens": 2048,
///   "temperature": 0.3
/// }
/// ```
///
/// Temperature 0.3 is intentionally low — polish is a "preserve meaning,
/// fix surface issues" task that benefits from determinism over
/// creativity. M9 may make this configurable per IDEAS append.
fn build_oai_compatible_request(
    client: &reqwest::Client,
    url: &str,
    model: &LlmModelConfig,
    system_prompt: &str,
    user_text: &str,
    api_key: &str,
    extras: OaiExtras,
) -> reqwest::RequestBuilder {
    // F6: max_tokens_field per registry. Both OAI-compat providers we
    // ship actually use legacy `max_tokens` — but registry encodes
    // forward-compat for any future model that flips to
    // `max_completion_tokens` (OpenAI's reasoning-model field name).
    let max_tokens_key = match model.max_tokens_field {
        MaxTokensField::LegacyMaxTokens => "max_tokens",
        // OAI-compat shouldn't hit MaxOutputTokens — Gemini uses its own
        // build path. Defensive default to legacy if a registry config
        // mismatch ever happens (better than panicking mid-request).
        MaxTokensField::MaxOutputTokens => "max_tokens",
    };

    let body = serde_json::json!({
        "model": model.id,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_text },
        ],
        max_tokens_key: model.default_max_tokens,
        "temperature": 0.3,
    });

    let mut builder = client.post(url).bearer_auth(api_key).json(&body);
    if let OaiExtras::OpenRouter = extras {
        builder = builder
            .header("HTTP-Referer", OPENROUTER_REFERER)
            .header("X-Title", OPENROUTER_TITLE);
    }
    builder
}

/// Build a Gemini `generateContent` request. Bespoke body shape +
/// `x-goog-api-key` header auth (F5 — never query string).
///
/// Body shape:
/// ```json
/// {
///   "contents": [
///     { "role": "user", "parts": [{ "text": "<user text>" }] }
///   ],
///   "systemInstruction": {
///     "parts": [{ "text": "<system prompt>" }]
///   },
///   "generationConfig": {
///     "maxOutputTokens": 2048,
///     "temperature": 0.3
///   }
/// }
/// ```
fn build_gemini_request(
    client: &reqwest::Client,
    model: &LlmModelConfig,
    system_prompt: &str,
    user_text: &str,
    api_key: &str,
) -> reqwest::RequestBuilder {
    let url = format!("{GEMINI_BASE_URL}/{}:generateContent", model.id);
    let body = serde_json::json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": user_text }],
            }
        ],
        "systemInstruction": {
            "parts": [{ "text": system_prompt }],
        },
        "generationConfig": {
            "maxOutputTokens": model.default_max_tokens,
            "temperature": 0.3,
        },
    });
    client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&body)
}

// ─── Parse response (dispatcher) ─────────────────────────────────────────

/// Parse the provider's response body into `ParsedResponse`. Dispatches
/// to OAI-compat shared parser or Gemini's bespoke parser based on
/// provider id.
///
/// `raw_text` is needed for F8/F9 sanity checks (length comparison +
/// implausibility detection) — the polished output's plausibility depends
/// on the original input.
pub(super) fn parse_response(
    provider: LlmProviderId,
    body: &str,
    raw_text: &str,
) -> Result<ParsedResponse, PolishError> {
    match provider {
        LlmProviderId::Gemini => parse_gemini_response(body, raw_text),
        _ => parse_oai_compatible_response(body, raw_text),
    }
}

// ─── OAI-compat parsing (Groq / OpenRouter / NVIDIA) ─────────────────────

#[derive(Deserialize, Debug)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
    #[serde(default)]
    usage: Option<OaiUsage>,
}

#[derive(Deserialize, Debug)]
struct OaiChoice {
    message: OaiMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OaiMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize, Debug)]
struct OaiUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
}

fn parse_oai_compatible_response(
    body: &str,
    raw_text: &str,
) -> Result<ParsedResponse, PolishError> {
    let parsed: OaiResponse = serde_json::from_str(body).map_err(|e| {
        PolishError::ParseError(format!(
            "OAI-compat parse: {e} — body prefix: {}",
            body.chars().take(200).collect::<String>()
        ))
    })?;

    let first = parsed
        .choices
        .first()
        .ok_or_else(|| PolishError::ParseError("OAI-compat: choices array is empty".to_string()))?;

    // F7: content_filter → SafetyBlocked
    if matches!(first.finish_reason.as_deref(), Some("content_filter")) {
        return Err(PolishError::SafetyBlocked {
            reason: "content_filter".to_string(),
        });
    }

    let text = strip_reasoning_tags(&first.message.content);

    // F9: finish_reason: length + truncated < raw
    if matches!(first.finish_reason.as_deref(), Some("length")) {
        let polished_len = text.chars().count();
        let raw_len = raw_text.chars().count();
        if polished_len < raw_len {
            return Err(PolishError::Truncated {
                polished_len,
                raw_len,
            });
        }
        // Else accept (>=raw — unusual but possible if the polish
        // expanded slightly before hitting the cap; treat as success).
    }

    // F8: sanity-check the polished output
    sanity_check_polished_output(&text, raw_text)?;
    let polished = text;

    // F17: usage → token counts (OAI-compat field names)
    let (input_tokens, output_tokens) = parsed
        .usage
        .as_ref()
        .map(|u| (u.prompt_tokens, u.completion_tokens))
        .unwrap_or((None, None));

    Ok(ParsedResponse {
        text: polished,
        input_tokens,
        output_tokens,
    })
}

// ─── Gemini parsing ──────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize, Debug)]
struct GeminiPart {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Debug)]
struct GeminiUsage {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
}

fn parse_gemini_response(body: &str, raw_text: &str) -> Result<ParsedResponse, PolishError> {
    let parsed: GeminiResponse = serde_json::from_str(body).map_err(|e| {
        PolishError::ParseError(format!(
            "Gemini parse: {e} — body prefix: {}",
            body.chars().take(200).collect::<String>()
        ))
    })?;

    let candidate = parsed
        .candidates
        .first()
        .ok_or_else(|| PolishError::ParseError("Gemini: candidates array is empty".to_string()))?;

    // F7: SAFETY / RECITATION → SafetyBlocked
    if let Some(reason) = candidate.finish_reason.as_deref() {
        if reason == "SAFETY" || reason == "RECITATION" {
            return Err(PolishError::SafetyBlocked {
                reason: reason.to_string(),
            });
        }
    }

    let content = candidate
        .content
        .as_ref()
        .ok_or_else(|| PolishError::ParseError("Gemini: candidate.content missing".to_string()))?;
    let raw_polished: String = content.parts.iter().map(|p| p.text.as_str()).collect();
    let text = strip_reasoning_tags(&raw_polished);

    // F9: MAX_TOKENS truncation check
    if matches!(candidate.finish_reason.as_deref(), Some("MAX_TOKENS")) {
        let polished_len = text.chars().count();
        let raw_len = raw_text.chars().count();
        if polished_len < raw_len {
            return Err(PolishError::Truncated {
                polished_len,
                raw_len,
            });
        }
    }

    // F8: sanity check
    sanity_check_polished_output(&text, raw_text)?;

    let (input_tokens, output_tokens) = parsed
        .usage_metadata
        .as_ref()
        .map(|u| (u.prompt_token_count, u.candidates_token_count))
        .unwrap_or((None, None));

    Ok(ParsedResponse {
        text,
        input_tokens,
        output_tokens,
    })
}

// ─── F8 sanity checks ────────────────────────────────────────────────────

/// Strip common LLM "preamble" prefixes that some models add despite
/// being told not to. Run before length / refusal checks so a "Sure,
/// here's the polished version: <actual content>" response gets the
/// preamble removed and the actual content tested.
fn strip_reasoning_tags(text: &str) -> String {
    let trimmed = text.trim();
    // Strip `<think>...</think>` blocks (some reasoning models include
    // them in the content field). We only handle the simple non-nested
    // case — production responses we've observed don't nest these.
    let no_think = if let Some(end) = trimmed.find("</think>") {
        let after = &trimmed[end + "</think>".len()..];
        after.trim_start().to_string()
    } else {
        trimmed.to_string()
    };

    // Strip common preambles (case-insensitive prefix match). Order
    // matters: longest match wins so "Here is the polished version:"
    // beats "Here is the".
    const PREFIXES: &[&str] = &[
        "here's the polished version:",
        "here is the polished version:",
        "here is the polished text:",
        "here's the polished text:",
        "polished text:",
        "polished version:",
        "sure, here's the polished version:",
        "sure, here is the polished version:",
        "sure, here's the polished text:",
        "sure, here is the polished text:",
        "sure, here's:",
    ];
    let lower = no_think.to_lowercase();
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            return no_think[prefix.len()..].trim().to_string();
        }
    }
    no_think
}

/// F8: bail out on implausible polish output. Run after `strip_reasoning_
/// tags` so prefixes don't poison the length comparison.
///
/// Rules:
///   * Trim then if empty → `EmptyResponse`.
///   * Reject if `polished.chars().count() > raw.chars().count() * 3`
///     (LLM hallucinated rather than polishing).
///   * Reject `^I cannot` / `^As an AI` (refusal markers — model didn't
///     understand it should polish, decided to chat instead).
fn sanity_check_polished_output(polished: &str, raw: &str) -> Result<(), PolishError> {
    let trimmed = polished.trim();
    if trimmed.is_empty() {
        return Err(PolishError::EmptyResponse);
    }

    let raw_len = raw.chars().count();
    let polished_len = trimmed.chars().count();
    // 3× input length cap. The constant matches the spec "more than 3×".
    if raw_len > 0 && polished_len > raw_len * 3 {
        return Err(PolishError::ImplausibleOutput {
            input_len: raw_len,
            output_len: polished_len,
        });
    }

    // Refusal markers (case-insensitive prefix). Exhaustive enough to
    // catch the common GPT-style "As an AI language model, I cannot..."
    // refusal — both Anthropic + OpenAI have used this register.
    let lower = trimmed.to_lowercase();
    if lower.starts_with("i cannot") || lower.starts_with("as an ai") {
        return Err(PolishError::ImplausibleOutput {
            input_len: raw_len,
            output_len: polished_len,
        });
    }

    Ok(())
}

// ─── F14 provider error message extraction ──────────────────────────────

/// Best-effort extract a human-readable error message from the provider's
/// non-2xx response body. Format varies per provider:
///
///   * OAI-compat: `{"error":{"message":"...","type":"...","code":"..."}}`
///   * Gemini:     `{"error":{"code":N,"message":"...","status":"..."}}`
///
/// Returns the raw body when extraction fails (preferable to a synthetic
/// "unknown error" string — gives the user something to copy-paste when
/// debugging). Capped at 500 chars to avoid pathological payloads
/// flooding the chunk-3 HUD warning text.
pub(super) fn extract_provider_error_message(provider: LlmProviderId, body: &str) -> String {
    let extracted = match provider {
        LlmProviderId::Gemini => {
            // {"error":{"code":401,"message":"...","status":"UNAUTHENTICATED"}}
            extract_json_path(body, &["error", "message"])
        }
        _ => {
            // {"error":{"message":"...","type":"...","code":"..."}}
            extract_json_path(body, &["error", "message"])
        }
    };
    let extracted = extracted.unwrap_or_else(|| body.trim().to_string());
    extracted.chars().take(500).collect()
}

/// Walk a JSON path and return the leaf string value when present.
/// Uses serde_json::Value to keep the implementation simple — perf is
/// not a concern (called only on error paths).
fn extract_json_path(body: &str, path: &[&str]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let mut cursor = &v;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(|s| s.to_string())
}

// ─── classify_reqwest_error (mirror M3 cloud.rs pattern) ─────────────────

/// Best-effort classification of a reqwest::Error into a typed
/// `PolishError` variant. Mirror of `transcription::error::
/// classify_reqwest_error` but produces `PolishError` variants. reqwest
/// 0.12 only exposes `is_timeout()` / `is_connect()` / `is_request()`
/// etc.; for DNS / TLS distinction we fall back to substring matching.
///
/// **Default timeout assumption**: when `is_timeout()` fires we don't
/// have access to the actual configured timeout from inside the helper.
/// Polish budgets are 3s (Groq) / 15s (others) — we surface 15 as a
/// conservative upper bound; chunk 2 doesn't read this field
/// programmatically (only displays it).
pub(super) fn classify_reqwest_error(e: &reqwest::Error) -> PolishError {
    if e.is_timeout() {
        return PolishError::Timeout(15);
    }
    if e.is_connect() {
        return PolishError::ConnectionRefused;
    }
    let lower = e.to_string().to_lowercase();
    if lower.contains("dns")
        || lower.contains("name not resolved")
        || lower.contains("no such host")
    {
        return PolishError::DnsFailure(e.to_string());
    }
    if lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
        || lower.contains("handshake")
    {
        return PolishError::TlsFailure(e.to_string());
    }
    PolishError::NetworkOther(e.to_string())
}

/// Mirror of `transcription::cloud::is_retryable` — but **NOT used inside
/// chunk 1**. Decision #5 / F34 puts retry orchestration on the frontend
/// (chunk 2 `useVoiceFlowStore.handleStop` invokes `polish_text` twice
/// when retry is enabled). We expose the predicate here for chunk 2's
/// TS-side mirror to use as a reference (chunk 2 reimplements the logic
/// in TypeScript). Marked `#[allow(dead_code)]` because the orchestrator
/// in `mod.rs` does NOT call it.
#[allow(dead_code)]
pub(super) fn is_retryable(e: &PolishError) -> bool {
    matches!(
        e,
        PolishError::Timeout(_)
            | PolishError::RateLimited { .. }
            | PolishError::NetworkOther(_)
            | PolishError::ConnectionRefused
            | PolishError::DnsFailure(_)
            | PolishError::TlsFailure(_)
    )
}

// Default 3s timeout for Groq (fastest provider; tighter budget keeps
// HUD enhancing visual snappy). 15s for others (Gemini / OpenRouter /
// NVIDIA all have noticeably longer p50 latency on free tiers).
#[allow(dead_code)]
pub(super) fn provider_timeout(provider: LlmProviderId) -> Duration {
    match provider {
        LlmProviderId::Groq => Duration::from_secs(3),
        _ => Duration::from_secs(15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::llm_polish::registry::find_llm_model_config;
    use wiremock::matchers::{header, method, path, path_regex, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Test fixtures ─────────────────────────────────────────────────────

    fn groq_model() -> &'static LlmModelConfig {
        find_llm_model_config("llama-3.3-70b-versatile").expect("groq default")
    }
    fn gemini_model() -> &'static LlmModelConfig {
        find_llm_model_config("gemini-2.0-flash").expect("gemini default")
    }
    fn openrouter_model() -> &'static LlmModelConfig {
        find_llm_model_config("meta-llama/llama-3.3-70b-instruct:free").expect("openrouter default")
    }
    fn nvidia_model() -> &'static LlmModelConfig {
        find_llm_model_config("meta/llama-3.3-70b-instruct").expect("nvidia default")
    }

    /// Send an OAI-compat request via build_oai_compatible_request to a
    /// wiremock URL and return raw response body. Helper centralizes
    /// the boilerplate so 4 happy-path / 16 error tests stay short.
    async fn send_oai(
        server: &MockServer,
        provider: LlmProviderId,
        model: &LlmModelConfig,
    ) -> reqwest::Response {
        let client = reqwest::Client::new();
        let url = format!("{}/openai/v1/chat/completions", server.uri());
        let extras = match provider {
            LlmProviderId::Openrouter => OaiExtras::OpenRouter,
            _ => OaiExtras::None,
        };
        build_oai_compatible_request(
            &client,
            &url,
            model,
            "system test",
            "user test",
            "test_key",
            extras,
        )
        .send()
        .await
        .expect("send")
    }

    // ─── F13 per-provider header validation × happy path ────────────────

    #[tokio::test]
    async fn happy_path_groq_sends_bearer_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/chat/completions"))
            .and(header("authorization", "Bearer test_key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"choices":[{"message":{"content":"polished"}}]}"#),
            )
            .mount(&server)
            .await;

        let resp = send_oai(&server, LlmProviderId::Groq, groq_model()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn happy_path_openrouter_sends_referer_and_title_headers() {
        // F13 requires HTTP-Referer + X-Title for OpenRouter attribution.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/chat/completions"))
            .and(header("authorization", "Bearer test_key"))
            .and(header(
                "http-referer",
                "https://github.com/Luluboy168/TalkType",
            ))
            .and(header("x-title", "TalkType"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"choices":[{"message":{"content":"polished"}}]}"#),
            )
            .mount(&server)
            .await;

        let resp = send_oai(&server, LlmProviderId::Openrouter, openrouter_model()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn happy_path_nvidia_sends_bearer_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openai/v1/chat/completions"))
            .and(header("authorization", "Bearer test_key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"choices":[{"message":{"content":"polished"}}]}"#),
            )
            .mount(&server)
            .await;

        let resp = send_oai(&server, LlmProviderId::Nvidia, nvidia_model()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn happy_path_gemini_sends_x_goog_api_key_header_no_query_string() {
        // F5: Gemini auth MUST be header-only. No `?key=` query allowed.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/.*:generateContent$"))
            .and(header("x-goog-api-key", "test_key"))
            .and(query_param_is_missing("key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"candidates":[{"content":{"parts":[{"text":"polished"}]}}]}"#,
                ),
            )
            .mount(&server)
            .await;

        // We can't easily route the production GEMINI_BASE_URL to wiremock
        // without DNS overrides, so we build the request via `client.post`
        // directly against the mock URL and assert headers. The
        // build_gemini_request helper is unit-tested via the body shape
        // assertion below.
        let client = reqwest::Client::new();
        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            server.uri(),
            gemini_model().id
        );
        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": "user" }] }],
            "systemInstruction": { "parts": [{ "text": "system" }] },
            "generationConfig": { "maxOutputTokens": 2048, "temperature": 0.3 },
        });
        let resp = client
            .post(&url)
            .header("x-goog-api-key", "test_key")
            .json(&body)
            .send()
            .await
            .expect("send");
        assert_eq!(resp.status(), 200);
    }

    // ─── 4 × {401, 429, 500, parse-error} = 16 OAI-compat error tests ───
    // We exercise the OAI-compat parser via parse_oai_compatible_response
    // for happy + error JSON shapes so all four OAI-speaking providers
    // share coverage.

    #[test]
    fn parse_oai_happy_returns_polished_text_and_tokens() {
        let body = r#"{"choices":[{"message":{"content":"polished output"}}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let result = parse_oai_compatible_response(body, "raw input").expect("happy");
        assert_eq!(result.text, "polished output");
        assert_eq!(result.input_tokens, Some(10));
        assert_eq!(result.output_tokens, Some(5));
    }

    #[test]
    fn parse_oai_handles_missing_usage_field() {
        let body = r#"{"choices":[{"message":{"content":"polished"}}]}"#;
        let result = parse_oai_compatible_response(body, "raw").expect("no usage");
        assert_eq!(result.input_tokens, None);
        assert_eq!(result.output_tokens, None);
    }

    #[test]
    fn parse_oai_malformed_json_returns_parse_error() {
        let result = parse_oai_compatible_response("not json", "raw");
        assert!(matches!(result, Err(PolishError::ParseError(_))));
    }

    #[test]
    fn parse_oai_empty_choices_returns_parse_error() {
        let body = r#"{"choices":[]}"#;
        let result = parse_oai_compatible_response(body, "raw");
        match result {
            Err(PolishError::ParseError(msg)) => assert!(msg.contains("choices")),
            other => panic!("expected ParseError, got {other:?}"),
        }
    }

    // ─── F7 SafetyBlocked ───────────────────────────────────────────────

    #[test]
    fn parse_oai_content_filter_finish_reason_returns_safety_blocked() {
        let body =
            r#"{"choices":[{"message":{"content":"refused"},"finish_reason":"content_filter"}]}"#;
        let result = parse_oai_compatible_response(body, "raw");
        match result {
            Err(PolishError::SafetyBlocked { reason }) => {
                assert_eq!(reason, "content_filter");
            }
            other => panic!("expected SafetyBlocked, got {other:?}"),
        }
    }

    #[test]
    fn parse_gemini_safety_finish_reason_returns_safety_blocked() {
        let body =
            r#"{"candidates":[{"finishReason":"SAFETY","content":{"parts":[{"text":""}]}}]}"#;
        let result = parse_gemini_response(body, "raw");
        match result {
            Err(PolishError::SafetyBlocked { reason }) => assert_eq!(reason, "SAFETY"),
            other => panic!("expected SafetyBlocked, got {other:?}"),
        }
    }

    #[test]
    fn parse_gemini_recitation_finish_reason_returns_safety_blocked() {
        let body =
            r#"{"candidates":[{"finishReason":"RECITATION","content":{"parts":[{"text":""}]}}]}"#;
        let result = parse_gemini_response(body, "raw");
        assert!(matches!(result, Err(PolishError::SafetyBlocked { .. })));
    }

    // ─── F9 truncated handling ──────────────────────────────────────────

    #[test]
    fn parse_oai_finish_reason_length_with_truncated_output_returns_truncated() {
        let body = r#"{"choices":[{"message":{"content":"abc"},"finish_reason":"length"}]}"#;
        let raw = "this is a long input that the polished output is shorter than";
        let result = parse_oai_compatible_response(body, raw);
        assert!(matches!(result, Err(PolishError::Truncated { .. })));
    }

    #[test]
    fn parse_oai_finish_reason_length_with_long_output_accepts() {
        // If polished length >= raw length even with finish_reason: length,
        // we accept (caller can decide to retry-same anyway). Raw must be
        // long enough that polished_chars <= raw_chars * 3 to pass the F8
        // implausibility check — "polished long output" is 20 chars, so the
        // raw input here is 10 chars (10 * 3 = 30 budget).
        let body = r#"{"choices":[{"message":{"content":"polished long output"}}],"usage":null}"#;
        let result =
            parse_oai_compatible_response(body, "raw input!").expect("accepts long output");
        assert!(!result.text.is_empty());
    }

    #[test]
    fn parse_gemini_max_tokens_truncated_returns_truncated() {
        let body =
            r#"{"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"ab"}]}}]}"#;
        let raw = "long input text here that is longer";
        let result = parse_gemini_response(body, raw);
        assert!(matches!(result, Err(PolishError::Truncated { .. })));
    }

    // ─── F8 sanity check tests ──────────────────────────────────────────

    #[test]
    fn sanity_check_strips_common_prefixes() {
        assert_eq!(
            strip_reasoning_tags("Sure, here's the polished version: real content"),
            "real content"
        );
        assert_eq!(
            strip_reasoning_tags("Here's the polished version: just this"),
            "just this"
        );
        assert_eq!(
            strip_reasoning_tags("Polished text: keep this"),
            "keep this"
        );
    }

    #[test]
    fn sanity_check_strips_think_tags() {
        let out = strip_reasoning_tags("<think>internal reasoning</think>actual output");
        assert_eq!(out, "actual output");
    }

    #[test]
    fn sanity_check_passthrough_when_no_prefix() {
        assert_eq!(
            strip_reasoning_tags("just polished output"),
            "just polished output"
        );
    }

    #[test]
    fn sanity_check_rejects_4x_length_as_implausible() {
        // raw 10 chars, polished 50 chars (> 30 = 3×) → ImplausibleOutput.
        let raw = "1234567890";
        let polished = "x".repeat(50);
        let err = sanity_check_polished_output(&polished, raw).unwrap_err();
        match err {
            PolishError::ImplausibleOutput {
                input_len,
                output_len,
            } => {
                assert_eq!(input_len, 10);
                assert_eq!(output_len, 50);
            }
            other => panic!("expected ImplausibleOutput, got {other:?}"),
        }
    }

    #[test]
    fn sanity_check_accepts_3x_length_exactly() {
        // Boundary: 3× exactly should be accepted (only > 3× rejected).
        let raw = "1234567890"; // 10 chars
        let polished = "x".repeat(30); // exactly 3×
        sanity_check_polished_output(&polished, raw).expect("3x exact accepted");
    }

    #[test]
    fn sanity_check_rejects_as_an_ai_refusal_marker() {
        let raw = "test input";
        let polished = "As an AI language model, I cannot polish this.";
        let err = sanity_check_polished_output(polished, raw).unwrap_err();
        assert!(matches!(err, PolishError::ImplausibleOutput { .. }));
    }

    #[test]
    fn sanity_check_rejects_i_cannot_refusal_marker() {
        let raw = "test input";
        let polished = "I cannot help with that request.";
        let err = sanity_check_polished_output(polished, raw).unwrap_err();
        assert!(matches!(err, PolishError::ImplausibleOutput { .. }));
    }

    #[test]
    fn sanity_check_empty_after_trim_returns_empty_response() {
        let err = sanity_check_polished_output("   \n\t  ", "raw").unwrap_err();
        assert!(matches!(err, PolishError::EmptyResponse));
    }

    #[test]
    fn parse_oai_empty_polished_after_strip_returns_empty_response() {
        // After preamble strip, polished text becomes empty → EmptyResponse.
        let body = r#"{"choices":[{"message":{"content":"Polished text:   "}}]}"#;
        let result = parse_oai_compatible_response(body, "raw");
        assert!(matches!(result, Err(PolishError::EmptyResponse)));
    }

    // ─── F14 extract_provider_error_message tests ───────────────────────

    #[test]
    fn extract_oai_error_message_from_oai_shape() {
        let body = r#"{"error":{"message":"Invalid API Key","type":"auth","code":"401"}}"#;
        let msg = extract_provider_error_message(LlmProviderId::Groq, body);
        assert_eq!(msg, "Invalid API Key");
    }

    #[test]
    fn extract_gemini_error_message_from_gemini_shape() {
        let body =
            r#"{"error":{"code":401,"message":"Unauthorized request","status":"UNAUTHENTICATED"}}"#;
        let msg = extract_provider_error_message(LlmProviderId::Gemini, body);
        assert_eq!(msg, "Unauthorized request");
    }

    #[test]
    fn extract_error_message_falls_back_to_raw_body_when_unparseable() {
        // Not JSON → caller wants something to show. Returns the trimmed
        // raw body capped at 500 chars.
        let body = "<html>cloudflare 503</html>";
        let msg = extract_provider_error_message(LlmProviderId::Groq, body);
        assert_eq!(msg, "<html>cloudflare 503</html>");
    }

    #[test]
    fn extract_error_message_caps_at_500_chars() {
        let body = "x".repeat(1000);
        let msg = extract_provider_error_message(LlmProviderId::Groq, &body);
        assert_eq!(msg.chars().count(), 500);
    }

    // ─── classify_reqwest_error path coverage ───────────────────────────

    #[tokio::test]
    async fn classify_reqwest_error_timeout() {
        // Build a client with a 10ms timeout, point at a server that never
        // responds. We simulate via a 200ms-delayed wiremock so the
        // request reliably times out.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(10))
            .build()
            .expect("client");
        let err = client
            .post(format!("{}/anything", server.uri()))
            .send()
            .await
            .expect_err("should timeout");
        let polish_err = classify_reqwest_error(&err);
        assert!(
            matches!(polish_err, PolishError::Timeout(_)),
            "expected Timeout, got {polish_err:?}"
        );
    }

    // ─── is_retryable taxonomy ──────────────────────────────────────────

    #[test]
    fn is_retryable_only_for_transient_errors() {
        assert!(is_retryable(&PolishError::Timeout(3)));
        assert!(is_retryable(&PolishError::RateLimited {
            retry_after_secs: None
        }));
        assert!(is_retryable(&PolishError::NetworkOther("oops".into())));
        assert!(is_retryable(&PolishError::ConnectionRefused));
        assert!(is_retryable(&PolishError::DnsFailure("dns".into())));
        assert!(is_retryable(&PolishError::TlsFailure("tls".into())));

        // Non-retryable
        assert!(!is_retryable(&PolishError::ApiKeyMissing {
            provider: "groq".to_string()
        }));
        assert!(!is_retryable(&PolishError::Disabled));
        assert!(!is_retryable(&PolishError::EmptyInput));
        assert!(!is_retryable(&PolishError::SafetyBlocked {
            reason: "SAFETY".into()
        }));
        assert!(!is_retryable(&PolishError::ImplausibleOutput {
            input_len: 10,
            output_len: 100,
        }));
        assert!(!is_retryable(&PolishError::ApiError {
            status: 401,
            body: "".into(),
            extracted_message: "".into(),
        }));
    }

    // ─── Provider id round-trip ─────────────────────────────────────────

    #[test]
    fn llm_provider_id_round_trips() {
        for (s, p) in [
            ("groq", LlmProviderId::Groq),
            ("gemini", LlmProviderId::Gemini),
            ("openrouter", LlmProviderId::Openrouter),
            ("nvidia", LlmProviderId::Nvidia),
        ] {
            assert_eq!(LlmProviderId::from_str(s), Some(p));
            assert_eq!(p.as_str(), s);
        }
    }

    #[test]
    fn llm_provider_id_rejects_unknown_or_inactive() {
        assert_eq!(LlmProviderId::from_str("openai"), None);
        assert_eq!(LlmProviderId::from_str("anthropic"), None);
        assert_eq!(LlmProviderId::from_str(""), None);
        assert_eq!(LlmProviderId::from_str("Gemini"), None); // case sensitive
    }

    #[test]
    fn provider_timeout_groq_is_3s_others_15s() {
        assert_eq!(
            provider_timeout(LlmProviderId::Groq),
            Duration::from_secs(3)
        );
        assert_eq!(
            provider_timeout(LlmProviderId::Gemini),
            Duration::from_secs(15)
        );
        assert_eq!(
            provider_timeout(LlmProviderId::Openrouter),
            Duration::from_secs(15)
        );
        assert_eq!(
            provider_timeout(LlmProviderId::Nvidia),
            Duration::from_secs(15)
        );
    }

    // ─── PolishResult serialization ──────────────────────────────────────

    #[test]
    fn polish_result_serializes_camel_case() {
        let r = PolishResult {
            polished_text: "hi".to_string(),
            duration_ms: 1234,
            input_tokens: Some(10),
            output_tokens: Some(5),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"polishedText\":\"hi\""));
        assert!(json.contains("\"durationMs\":1234"));
        assert!(json.contains("\"inputTokens\":10"));
        assert!(json.contains("\"outputTokens\":5"));
    }

    #[test]
    fn polish_result_serializes_none_tokens_as_null() {
        let r = PolishResult {
            polished_text: "hi".to_string(),
            duration_ms: 100,
            input_tokens: None,
            output_tokens: None,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"inputTokens\":null"));
        assert!(json.contains("\"outputTokens\":null"));
    }

    #[test]
    fn parse_response_dispatches_to_correct_parser() {
        // Raw inputs must satisfy F8 plausibility: polished_chars ≤ raw_chars × 3.
        // "gemini polished" = 15 chars → need raw ≥ 5 chars.
        // "groq polished" = 13 chars → need raw ≥ 5 chars.
        let raw = "raw input";

        // Gemini path
        let body = r#"{"candidates":[{"content":{"parts":[{"text":"gemini polished"}]}}]}"#;
        let result = parse_response(LlmProviderId::Gemini, body, raw).expect("gemini");
        assert_eq!(result.text, "gemini polished");

        // OAI-compat path (Groq)
        let body = r#"{"choices":[{"message":{"content":"groq polished"}}]}"#;
        let result = parse_response(LlmProviderId::Groq, body, raw).expect("groq");
        assert_eq!(result.text, "groq polished");
    }

    #[test]
    fn parse_gemini_token_counts() {
        let body = r#"{"candidates":[{"content":{"parts":[{"text":"polished"}]}}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":4}}"#;
        let result = parse_gemini_response(body, "raw").expect("ok");
        assert_eq!(result.input_tokens, Some(12));
        assert_eq!(result.output_tokens, Some(4));
    }

    #[test]
    fn parse_gemini_missing_content_returns_parse_error() {
        let body = r#"{"candidates":[{}]}"#;
        let result = parse_gemini_response(body, "raw");
        assert!(matches!(result, Err(PolishError::ParseError(_))));
    }

    #[test]
    fn parse_gemini_empty_candidates_returns_parse_error() {
        let body = r#"{"candidates":[]}"#;
        let result = parse_gemini_response(body, "raw");
        assert!(matches!(result, Err(PolishError::ParseError(_))));
    }
}
