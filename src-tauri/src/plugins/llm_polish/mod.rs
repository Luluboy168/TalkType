// LLM polish plugin — multi-provider chat-completions wrapper (M6 chunk 1).
//
// Public surface:
//
//   State          LlmPolishState (managed by lib.rs)
//   Command        polish_text
//   Helper module  health (test_*_connection for the 3 non-Groq providers,
//                   dispatched from `transcription::health::test_provider_connection`)
//
// **Sub-module layout** (see each file's header for full context):
//   * `error.rs`     — `PolishError` thiserror enum + `PolishFailureReason`
//                       closed enum (mirrors frontend `src/types/llm.ts`).
//   * `providers.rs` — 4 provider request builders + response parsers
//                       (Groq / OpenRouter / NVIDIA NIM / Gemini, all
//                       free-tier models per Decision #3).
//   * `prompts.rs`   — 5 prompt-mode system prompts × 2 langs + custom
//                       prompt validation (chars().count() ≤ 1000).
//   * `registry.rs`  — `LlmModelConfig` + `LLM_MODEL_LIST` (8 models).
//   * `health.rs`    — `test_*_connection` for OpenRouter / NVIDIA / Gemini
//                       (Groq stays in `transcription/health.rs` per F20).
//
// **Concurrency**: a `polish_busy: AtomicBool` guard rejects overlapping
// `polish_text` invocations with `PolishError::Busy`. Decision #5 (F34) puts
// retry orchestration on the frontend — the JS side waits for the first
// invocation to complete (its `_guard` Drops + the AtomicBool flips back to
// false) before issuing attempt #2, so the two attempts never race the
// guard. The guard exists primarily to defend against the toggle-hotkey
// burst case (F24) where the user hits the hotkey again during enhancing.
//
// **BusyGuard RAII**: the AtomicBool is set ONLY by the
// `compare_exchange(false, true)` at the top of `polish_text`; it is reset
// ONLY by `BusyGuard::drop`. Reviewer hint: `polish_busy.store(false, ...)`
// MUST appear in exactly one place — `BusyGuard::drop` — to make the
// "released regardless of `?` early return" invariant locally checkable
// via `grep`.
//
// **Event broadcast**: on every failure path (typed `PolishError`) the
// orchestrator emits `polish:failed-fallback` with `{ reason, providerId }`
// (chunk 3 HUD warns the user; chunk 2 frontend already has the typed Err
// from `invoke<PolishResult, PolishError>()`, so the event is informational
// for sibling windows like the Dashboard). The event is best-effort —
// emit failures are logged but don't change the command's return value.

mod error;
pub mod health;
mod prompts;
mod providers;
mod registry;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

pub use error::{PolishError, PolishFailureReason};
pub use providers::{LlmProviderId, PolishResult};

/// Application-managed state for LLM polish.
///
/// Holds:
///   * `client` — shared `reqwest::Client` so we reuse the connection pool
///     across polish calls (TLS handshake reuse alone saves ~150 ms
///     on the first retry). Mirrors `TranscriptionState::client`.
///   * `polish_busy` — `AtomicBool` guarding overlapping `polish_text`
///     invocations (F24). `Arc<AtomicBool>` rather than
///     plain `AtomicBool` so `BusyGuard` can hold a borrow
///     without lifetime gymnastics in the command body.
pub struct LlmPolishState {
    pub(crate) client: reqwest::Client,
    polish_busy: Arc<AtomicBool>,
}

impl LlmPolishState {
    /// Build with TalkType-tuned defaults: 30 s overall pool budget (per-
    /// request `.timeout(...)` is tighter — 3 s for Groq, 15 s for others —
    /// see `polish_text` body), 60 s pool idle, `User-Agent: TalkType/<ver>`.
    /// Mirrors `TranscriptionState::new`.
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .user_agent(format!("TalkType/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            polish_busy: Arc::new(AtomicBool::new(false)),
        })
    }
}

/// RAII guard that releases the `polish_busy` AtomicBool on drop, regardless
/// of whether the command body returned via `?` early-exit or normal
/// completion. The drop ordering matters: Rust drops bindings in reverse
/// declaration order at end-of-scope, so `_guard` (declared after the
/// initial `compare_exchange` succeeds) drops AFTER any `result` binding
/// in scope, which is what we want — guard releases LAST so the busy flag
/// is held until the orchestrator returns to the caller.
struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        // `Release` ordering pairs with the `Acquire` half of
        // `compare_exchange` in `polish_text`. A subsequent attempt from a
        // different thread / re-invocation observes the flag flip via the
        // synchronizes-with edge.
        self.0.store(false, Ordering::Release);
    }
}

/// Frontend payload — `camelCase` to match `PolishTextArgs` in
/// `src/types/llm.ts`. `attempt` is reserved for future telemetry / log
/// correlation; chunk 1 doesn't branch on it but plumbing it through means
/// chunk 2's retry loop can pass `attempt: 1` / `attempt: 2` for log
/// disambiguation. F34.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PolishTextArgs {
    pub raw_text: String,
    #[serde(default)]
    pub vocabulary: Option<Vec<String>>,
    #[serde(default = "default_attempt")]
    pub attempt: u32,
}

fn default_attempt() -> u32 {
    1
}

/// Wire-format payload for the `polish:failed-fallback` event. Mirrors
/// `PolishFallbackPayload` in `src/types/llm.ts`. `provider_id` is restricted
/// to the 4 active polish providers (`groq` / `openrouter` / `nvidia` /
/// `gemini`) — the dispatcher rejects unknown ids before reaching the emit.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PolishFallbackPayload {
    reason: &'static str,
    provider_id: &'static str,
}

const POLISH_FAILED_FALLBACK_EVENT: &str = "polish:failed-fallback";

/// Emit `polish:failed-fallback` (best-effort — logs but does not change
/// command return value). Called from every typed-error branch of
/// `polish_text` so chunk 3's HUD + chunk 2's voice-flow store can observe
/// fallbacks without round-tripping through the typed `Err`.
fn emit_fallback(app: &AppHandle, err: &PolishError, provider_id: &str) {
    let payload = PolishFallbackPayload {
        reason: PolishFailureReason::from(err).as_str(),
        // We accept `&str` here because the orchestrator only emits with
        // already-validated provider ids that are 'static for our 4 active
        // providers. Using `LlmProviderId::as_str()` ensures the wire format
        // matches the TS `LlmActivePolishProviderId` literal type.
        provider_id: match provider_id {
            "groq" => "groq",
            "openrouter" => "openrouter",
            "nvidia" => "nvidia",
            "gemini" => "gemini",
            // Defensive: if we somehow get an unknown id here (we shouldn't
            // — `polish_text` validates first), default to "groq" rather
            // than emit a string the TS narrowing would reject. The
            // companion typed `Err` already carries the precise context.
            _ => "groq",
        },
    };
    if let Err(e) = app.emit(POLISH_FAILED_FALLBACK_EVENT, payload) {
        eprintln!("[llm-polish] emit polish:failed-fallback failed: {e}");
    }
}

/// Multi-provider LLM polish entry point. Frontend invokes via
/// `invoke<PolishResult, PolishError>('polish_text', { rawText, vocabulary, attempt })`
/// and surfaces the polished text or, on `Err`, falls back to raw paste +
/// listens for the `polish:failed-fallback` event for HUD warning state.
///
/// Pipeline:
///
///   1. Acquire `polish_busy` AtomicBool — overlapping invocations get
///      `PolishError::Busy` (also emit `polish:failed-fallback`).
///   2. RAII `BusyGuard` ensures release on every exit path.
///   3. Read `Settings` snapshot, resolve provider / model / prompt mode.
///   4. Defensive checks: polish disabled, empty input, unknown provider,
///      unknown model, unknown prompt mode, custom prompt too long.
///   5. Build system prompt + inject vocabulary (50-term / 600-char cap
///      via `vocabulary::cap_terms`, F10).
///   6. Read API key from keyring (Rust-only — architecture invariant #1).
///   7. POST to provider with per-provider timeout (3 s for Groq, 15 s
///      for OpenRouter / NVIDIA / Gemini).
///   8. On 2xx → parse + sanity-check (F8/F9) → return `PolishResult`.
///   9. On non-2xx / network error / parse error → emit `polish:failed-
///      fallback` + return typed `Err`.
#[tauri::command]
pub async fn polish_text(
    state: State<'_, LlmPolishState>,
    settings_state: State<'_, crate::settings::SettingsState>,
    app: AppHandle,
    args: PolishTextArgs,
) -> Result<PolishResult, PolishError> {
    // ─── Step 1: acquire busy guard ──────────────────────────────────────
    if state
        .polish_busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        // Already in flight. Emit fallback before returning the typed error
        // so chunk 2's `polish:failed-fallback` listener sees it on the same
        // turn. We don't have a known provider for the busy case — the
        // currently-running invocation owns that context — so we use the
        // settings snapshot's provider as a best-effort hint. Failure of
        // that read falls through to "groq" (architecture default).
        let provider_for_event = settings_state
            .snapshot()
            .ok()
            .and_then(|s| s.llm_provider)
            .unwrap_or_else(|| "groq".to_string());
        let err = PolishError::Busy;
        emit_fallback(&app, &err, &provider_for_event);
        return Err(err);
    }
    // RAII release — fires on every exit path below (including `?` early
    // returns). Reviewer hint: this is the ONLY `BusyGuard::new` site, and
    // `Drop` is the ONLY place that calls `polish_busy.store(false, ...)`.
    let _guard = BusyGuard(&state.polish_busy);

    let started = Instant::now();

    // ─── Step 2: snapshot settings + resolve config ──────────────────────
    let settings = settings_state
        .snapshot()
        .map_err(|e| PolishError::ParseError(format!("settings snapshot: {e}")))?;

    // Defensive: polish should not be invoked when disabled. Frontend chunk
    // 2 should have skipped this, but if a user toggled the setting while
    // a hotkey burst was in flight we'd rather surface a typed error than
    // silently call the LLM.
    if matches!(settings.llm_polish_enabled, Some(false)) {
        let err = PolishError::Disabled;
        let provider_for_event = settings
            .llm_provider
            .clone()
            .unwrap_or_else(|| "groq".to_string());
        emit_fallback(&app, &err, &provider_for_event);
        return Err(err);
    }

    // Reject empty / whitespace-only input. Frontend chunk 2 should have
    // skipped this, but if it doesn't we'd rather not waste a quota slot.
    if args.raw_text.trim().is_empty() {
        let err = PolishError::EmptyInput;
        let provider_for_event = settings
            .llm_provider
            .clone()
            .unwrap_or_else(|| "groq".to_string());
        emit_fallback(&app, &err, &provider_for_event);
        return Err(err);
    }

    // Resolve provider id (defaults to Groq if unset). Unknown provider →
    // ParseError so the frontend can show "set provider in Settings".
    let provider_str = settings
        .llm_provider
        .clone()
        .unwrap_or_else(|| "groq".to_string());
    let provider = match LlmProviderId::from_str(&provider_str) {
        Some(p) => p,
        None => {
            let err = PolishError::ParseError(format!("unknown provider: {provider_str}"));
            emit_fallback(&app, &err, "groq");
            return Err(err);
        }
    };

    // Resolve effective model id (override > model_id > provider default).
    let model_id = registry::get_effective_model_id(&settings);
    let model_config = match registry::find_llm_model_config(&model_id) {
        Some(c) => c,
        None => {
            let err = PolishError::ParseError(format!(
                "unknown model id: {model_id} for provider {provider_str}"
            ));
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    // Resolve prompt mode (defaults to Default if unset).
    let mode_str = settings
        .llm_prompt_mode
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let mode = match prompts::PromptMode::from_str(&mode_str) {
        Some(m) => m,
        None => {
            let err = PolishError::ParseError(format!("unknown prompt mode: {mode_str}"));
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    // Validate custom prompt body when in Custom mode (F11).
    let custom_validated = if mode == prompts::PromptMode::Custom {
        match prompts::validate_custom_prompt(settings.llm_custom_prompt.as_deref().unwrap_or("")) {
            Ok(s) => Some(s),
            Err(err) => {
                emit_fallback(&app, &err, &provider_str);
                return Err(err);
            }
        }
    } else {
        None
    };

    // ─── Step 3: build system prompt + inject vocabulary ─────────────────
    // Lang fallback: chunk 1 doesn't have a `Settings.language_ui` field,
    // so we default to zh-TW (TalkType is Windows-Taiwan-first). Chunk 4 may
    // pass through a real value once Settings exposes one.
    let system_base = prompts::system_prompt(mode, "zh-TW", custom_validated.as_deref());
    let vocab_terms = args.vocabulary.as_deref().unwrap_or(&[]);
    let system_with_vocab = prompts::inject_vocabulary(&system_base, vocab_terms);

    // ─── Step 4: read API key (Rust-only per architecture invariant #1) ──
    let api_key = match crate::plugins::credentials::get_credential(&provider_str) {
        Ok(Some(key)) => key,
        Ok(None) => {
            let err = PolishError::ApiKeyMissing {
                provider: provider_str.clone(),
            };
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
        Err(e) => {
            let err = PolishError::Credentials(e.to_string());
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    // ─── Step 5: build + send request with per-provider timeout ──────────
    let request = providers::build_request(
        &state.client,
        provider,
        model_config,
        &system_with_vocab,
        &args.raw_text,
        &api_key,
    );

    // Per-provider timeout: 3 s Groq, 15 s others. The shared client has a
    // 30 s pool budget; the per-request timeout is what actually fires
    // first.
    let timeout = match provider {
        LlmProviderId::Groq => Duration::from_secs(3),
        _ => Duration::from_secs(15),
    };

    let response = match request.timeout(timeout).send().await {
        Ok(r) => r,
        Err(e) => {
            let err = providers::classify_reqwest_error(&e);
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    // ─── Step 6: classify status ─────────────────────────────────────────
    let status = response.status();
    if !status.is_success() {
        // Pre-fetch the `Retry-After` header BEFORE `.text().await` consumes
        // the response. Mirrors the M3 pattern in
        // `transcription/health.rs::test_groq_connection_with_url` which proves
        // headers can be read first when the body needs to come second.
        // Chunk-1 P1 cleanup: forward-compat for v0.2 backoff. Decision #5
        // still says NO frontend backoff between retry attempts today, so
        // the parsed value is currently informational only — but having it
        // available means a future v0.2 change can flip on backoff without
        // re-touching the read order here.
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => {
                let err = PolishError::ParseError(format!("read error body: {e}"));
                emit_fallback(&app, &err, &provider_str);
                return Err(err);
            }
        };
        let extracted = providers::extract_provider_error_message(provider, &body);
        let err = if status.as_u16() == 429 {
            PolishError::RateLimited {
                retry_after_secs: retry_after,
            }
        } else {
            PolishError::ApiError {
                status: status.as_u16(),
                body,
                extracted_message: extracted,
            }
        };
        emit_fallback(&app, &err, &provider_str);
        return Err(err);
    }

    // ─── Step 7: parse response body ─────────────────────────────────────
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            let err = PolishError::ParseError(format!("read success body: {e}"));
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    let parsed = match providers::parse_response(provider, &body, &args.raw_text) {
        Ok(p) => p,
        Err(err) => {
            emit_fallback(&app, &err, &provider_str);
            return Err(err);
        }
    };

    // ─── Step 8: success ─────────────────────────────────────────────────
    Ok(PolishResult {
        polished_text: parsed.text,
        duration_ms: started.elapsed().as_millis() as u64,
        input_tokens: parsed.input_tokens,
        output_tokens: parsed.output_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_polish_state_builds_successfully() {
        let _state = LlmPolishState::new().expect("build");
    }

    #[test]
    fn polish_text_args_deserializes_camel_case() {
        // Frontend sends camelCase; we receive snake_case via serde rename.
        let json = r#"{"rawText":"hello","vocabulary":["foo","bar"],"attempt":2}"#;
        let args: PolishTextArgs = serde_json::from_str(json).expect("deserialize");
        assert_eq!(args.raw_text, "hello");
        assert_eq!(
            args.vocabulary,
            Some(vec!["foo".to_string(), "bar".to_string()])
        );
        assert_eq!(args.attempt, 2);
    }

    #[test]
    fn polish_text_args_defaults_attempt_to_one() {
        let json = r#"{"rawText":"hello"}"#;
        let args: PolishTextArgs = serde_json::from_str(json).expect("deserialize");
        assert_eq!(args.attempt, 1);
        assert_eq!(args.vocabulary, None);
    }

    #[test]
    fn busy_guard_releases_atomic_on_drop() {
        let flag = AtomicBool::new(true);
        {
            let _guard = BusyGuard(&flag);
            assert!(flag.load(Ordering::Acquire));
        }
        assert!(
            !flag.load(Ordering::Acquire),
            "guard should release on drop"
        );
    }

    #[test]
    fn polish_fallback_payload_serializes_camel_case() {
        let payload = PolishFallbackPayload {
            reason: "rate_limited",
            provider_id: "groq",
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(json.contains("\"reason\":\"rate_limited\""));
        assert!(json.contains("\"providerId\":\"groq\""));
    }
}
