// LLM model registry (M6 chunk 1, F6 + Decision #3).
//
// 8 models pinned: 4 free-tier providers × 2 models each. The first model
// per provider is the default — chunk 4's Settings UI seeds the model
// dropdown from the order in `LLM_MODEL_LIST`.
//
// **`MaxTokensField`** divergence: Gemini's API uses
// `generationConfig.maxOutputTokens` (nested + different field name) while
// the OpenAI-compat providers (Groq / OpenRouter / NVIDIA NIM) use
// top-level `max_tokens`. Storing this per-model lets `providers.rs`
// `build_request` pick the right body shape without provider-specific
// branches. F6b.
//
// **F4 escape hatch**: `get_effective_model_id` honors
// `Settings.llm_model_id_override` first, then `Settings.llm_model_id`,
// then falls back to the per-provider default. The override is hidden from
// the M6 Settings UI (user edits `settings.json` by hand for unlisted
// models — useful when a model is deprecated mid-month and the registry is
// stale). chunk 4's UI exposure is deferred to v0.2.

use super::providers::LlmProviderId;

/// Body-field divergence between Gemini and OpenAI-compat providers. Drives
/// the `build_request` dispatcher: legacy `max_tokens` (Groq / OpenRouter /
/// NVIDIA) writes the value at the body root; `MaxOutputTokens` (Gemini)
/// writes it under `generationConfig.maxOutputTokens` (separate nesting +
/// rename).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxTokensField {
    /// `max_tokens: <n>` at the body root. OpenAI-compat default.
    LegacyMaxTokens,
    /// `generationConfig.maxOutputTokens: <n>`. Gemini-only divergence.
    MaxOutputTokens,
}

/// Per-model metadata. Mirrors the TS `LlmModelInfo` interface in
/// `src/types/llm.ts` (chunk 0) — chunk 4 will export a parallel
/// `LLM_MODEL_LIST` in `src/lib/providers.ts` so the Settings UI doesn't
/// invoke Rust on every render.
///
/// `'static` lifetime everywhere because the list is a `const` table (no
/// per-instance allocation; one shared snapshot for the lifetime of the
/// process).
#[derive(Debug, Clone)]
pub struct LlmModelConfig {
    /// Stable model id passed verbatim to the provider API.
    pub id: &'static str,
    /// Owning provider id — restricted to the 4 active polish providers.
    pub provider: LlmProviderId,
    /// Human-readable label for the Settings UI dropdown. Chunk 4 mirrors
    /// these into `src/lib/providers.ts`.
    #[allow(dead_code)]
    pub display_name: &'static str,
    /// Token context window — informational, not enforced client-side.
    #[allow(dead_code)]
    pub context_window: u32,
    /// Default `max_tokens` (or `maxOutputTokens` for Gemini) for the
    /// response budget. Chunk 1 polish ships at 2048 across all 8 models;
    /// future tuning can vary per model without API surface changes.
    pub default_max_tokens: u32,
    /// Body-field divergence between Gemini and OpenAI-compat. Drives
    /// `providers.rs::build_request` per F6b.
    pub max_tokens_field: MaxTokensField,
    /// Whether this model has a free tier. M6 default: all 8 free.
    #[allow(dead_code)]
    pub is_free: bool,
}

/// 8 default models per Decision #3 (4 providers × 2 models each). Order
/// is significant: the first model per provider is its default (used by
/// `get_default_model_id`). Models within a provider are listed by
/// "balanced first, alternate second" so chunk 4's dropdown shows the
/// recommended model on top.
pub const LLM_MODEL_LIST: &[LlmModelConfig] = &[
    // ─── Groq ────────────────────────────────────────────────────────────
    LlmModelConfig {
        id: "llama-3.3-70b-versatile",
        provider: LlmProviderId::Groq,
        display_name: "Llama 3.3 70B (Versatile)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
    LlmModelConfig {
        id: "llama-3.1-8b-instant",
        provider: LlmProviderId::Groq,
        display_name: "Llama 3.1 8B (Instant)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
    // ─── Gemini ──────────────────────────────────────────────────────────
    LlmModelConfig {
        id: "gemini-2.0-flash",
        provider: LlmProviderId::Gemini,
        display_name: "Gemini 2.0 Flash",
        context_window: 1_000_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::MaxOutputTokens,
        is_free: true,
    },
    LlmModelConfig {
        id: "gemini-1.5-flash",
        provider: LlmProviderId::Gemini,
        display_name: "Gemini 1.5 Flash",
        context_window: 1_000_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::MaxOutputTokens,
        is_free: true,
    },
    // ─── OpenRouter (free tier `:free` models) ───────────────────────────
    LlmModelConfig {
        id: "meta-llama/llama-3.3-70b-instruct:free",
        provider: LlmProviderId::Openrouter,
        display_name: "Llama 3.3 70B (OpenRouter free)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
    LlmModelConfig {
        id: "qwen/qwen-2.5-72b-instruct:free",
        provider: LlmProviderId::Openrouter,
        display_name: "Qwen 2.5 72B (OpenRouter free)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
    // ─── NVIDIA NIM (free credits) ───────────────────────────────────────
    LlmModelConfig {
        id: "meta/llama-3.3-70b-instruct",
        provider: LlmProviderId::Nvidia,
        display_name: "Llama 3.3 70B (NVIDIA NIM)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
    LlmModelConfig {
        id: "nvidia/llama-3.1-nemotron-70b-instruct",
        provider: LlmProviderId::Nvidia,
        display_name: "Nemotron 70B (NVIDIA tuned)",
        context_window: 128_000,
        default_max_tokens: 2048,
        max_tokens_field: MaxTokensField::LegacyMaxTokens,
        is_free: true,
    },
];

/// Look up a model config by id. Returns `None` if the id isn't in the
/// pinned registry — the caller should treat that as a hard error
/// (`PolishError::ParseError`) since the orchestrator already bypasses the
/// registry when `Settings.llm_model_id_override` is set.
pub fn find_llm_model_config(id: &str) -> Option<&'static LlmModelConfig> {
    LLM_MODEL_LIST.iter().find(|m| m.id == id)
}

/// All models grouped by provider. Used by chunk 4's Settings UI to seed
/// the model dropdown after the user picks a provider — also reused by
/// the per-provider sanity tests below.
#[allow(dead_code)]
pub fn get_models_by_provider(provider: LlmProviderId) -> Vec<&'static LlmModelConfig> {
    LLM_MODEL_LIST
        .iter()
        .filter(|m| m.provider == provider)
        .collect()
}

/// Default model id for a provider. Picks the first model in
/// `LLM_MODEL_LIST` whose provider matches; we hardcode the literal here
/// rather than walk the table so a `cargo test` failure surfaces a stale
/// reference if the table is ever reordered.
pub fn get_default_model_id(provider: LlmProviderId) -> &'static str {
    match provider {
        LlmProviderId::Groq => "llama-3.3-70b-versatile",
        LlmProviderId::Gemini => "gemini-2.0-flash",
        LlmProviderId::Openrouter => "meta-llama/llama-3.3-70b-instruct:free",
        LlmProviderId::Nvidia => "meta/llama-3.3-70b-instruct",
    }
}

/// F4: resolve the effective model id for a polish_text invocation.
/// Priority chain (first non-empty wins):
///   1. `Settings.llm_model_id_override` — F4 escape hatch (UI-hidden in M6)
///   2. `Settings.llm_model_id`           — user picked a model in Settings
///   3. provider default                  — never returns empty
///
/// Whitespace-only override / model_id values are ignored as if absent so
/// the user can clear the field via Settings JSON without hitting an
/// unhelpful "model id `   ` not found" error.
pub fn get_effective_model_id(settings: &crate::settings::Settings) -> String {
    if let Some(ref override_id) = settings.llm_model_id_override {
        let trimmed = override_id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(ref model_id) = settings.llm_model_id {
        let trimmed = model_id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let provider = settings
        .llm_provider
        .as_deref()
        .and_then(LlmProviderId::from_str)
        .unwrap_or(LlmProviderId::Groq);
    get_default_model_id(provider).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn registry_has_at_least_8_models() {
        assert!(
            LLM_MODEL_LIST.len() >= 8,
            "expected ≥ 8 models, got {}",
            LLM_MODEL_LIST.len()
        );
    }

    #[test]
    fn registry_covers_all_4_active_providers() {
        for provider in [
            LlmProviderId::Groq,
            LlmProviderId::Gemini,
            LlmProviderId::Openrouter,
            LlmProviderId::Nvidia,
        ] {
            let count = LLM_MODEL_LIST
                .iter()
                .filter(|m| m.provider == provider)
                .count();
            assert!(
                count >= 2,
                "provider {provider:?} should have ≥ 2 models, got {count}"
            );
        }
    }

    #[test]
    fn get_default_model_id_returns_valid_registry_entry_per_provider() {
        for provider in [
            LlmProviderId::Groq,
            LlmProviderId::Gemini,
            LlmProviderId::Openrouter,
            LlmProviderId::Nvidia,
        ] {
            let id = get_default_model_id(provider);
            let config = find_llm_model_config(id).unwrap_or_else(|| {
                panic!("default for {provider:?} missing from LLM_MODEL_LIST: {id}")
            });
            assert_eq!(
                config.provider, provider,
                "default model id {id} for {provider:?} mismapped"
            );
        }
    }

    #[test]
    fn find_llm_model_config_round_trips_known_id() {
        let config = find_llm_model_config("gemini-2.0-flash").expect("gemini-2.0-flash is pinned");
        assert_eq!(config.provider, LlmProviderId::Gemini);
        assert_eq!(config.max_tokens_field, MaxTokensField::MaxOutputTokens);
    }

    #[test]
    fn find_llm_model_config_rejects_unknown_id() {
        assert!(find_llm_model_config("not-a-real-model-id-2099").is_none());
    }

    #[test]
    fn gemini_uses_max_output_tokens_field() {
        let config = find_llm_model_config("gemini-2.0-flash").expect("gemini-2.0-flash");
        assert_eq!(config.max_tokens_field, MaxTokensField::MaxOutputTokens);
    }

    #[test]
    fn oai_compat_providers_use_legacy_max_tokens_field() {
        for id in [
            "llama-3.3-70b-versatile",                // Groq
            "meta-llama/llama-3.3-70b-instruct:free", // OpenRouter
            "meta/llama-3.3-70b-instruct",            // NVIDIA
        ] {
            let config = find_llm_model_config(id).unwrap_or_else(|| panic!("{id} missing"));
            assert_eq!(
                config.max_tokens_field,
                MaxTokensField::LegacyMaxTokens,
                "{id} should be LegacyMaxTokens"
            );
        }
    }

    #[test]
    fn get_models_by_provider_groq_has_at_least_two() {
        let models = get_models_by_provider(LlmProviderId::Groq);
        assert!(
            models.len() >= 2,
            "Groq should ship ≥ 2 models, got {}",
            models.len()
        );
        for m in models {
            assert_eq!(m.provider, LlmProviderId::Groq);
        }
    }

    // ─── F4 get_effective_model_id priority chain ────────────────────────

    #[test]
    fn effective_model_id_uses_override_first() {
        let settings = Settings {
            llm_provider: Some("groq".to_string()),
            llm_model_id: Some("llama-3.1-8b-instant".to_string()),
            llm_model_id_override: Some("custom/experimental-model".to_string()),
            ..Settings::default()
        };
        assert_eq!(
            get_effective_model_id(&settings),
            "custom/experimental-model"
        );
    }

    #[test]
    fn effective_model_id_falls_back_to_model_id_when_override_empty() {
        let settings = Settings {
            llm_provider: Some("groq".to_string()),
            llm_model_id: Some("llama-3.1-8b-instant".to_string()),
            llm_model_id_override: Some("   ".to_string()),
            ..Settings::default()
        };
        assert_eq!(get_effective_model_id(&settings), "llama-3.1-8b-instant");
    }

    #[test]
    fn effective_model_id_falls_back_to_provider_default_when_both_empty() {
        let settings = Settings {
            llm_provider: Some("groq".to_string()),
            llm_model_id: None,
            llm_model_id_override: None,
            ..Settings::default()
        };
        assert_eq!(get_effective_model_id(&settings), "llama-3.3-70b-versatile");
    }

    #[test]
    fn effective_model_id_defaults_to_groq_when_provider_unknown() {
        // Defensive: garbage `llm_provider` value falls through to Groq
        // default rather than panicking. Chunk 1's polish_text orchestrator
        // re-validates the provider id separately and returns ParseError
        // for unknowns, so this path is just a safe fallback for the model
        // resolution phase.
        let settings = Settings {
            llm_provider: Some("unknown-vendor-xyz".to_string()),
            ..Settings::default()
        };
        assert_eq!(get_effective_model_id(&settings), "llama-3.3-70b-versatile");
    }

    #[test]
    fn effective_model_id_defaults_to_groq_when_provider_none() {
        let settings = Settings {
            llm_provider: None,
            ..Settings::default()
        };
        assert_eq!(get_effective_model_id(&settings), "llama-3.3-70b-versatile");
    }

    #[test]
    fn effective_model_id_per_provider_default_returns_consistent_id() {
        for (provider_str, expected_default) in [
            ("groq", "llama-3.3-70b-versatile"),
            ("gemini", "gemini-2.0-flash"),
            ("openrouter", "meta-llama/llama-3.3-70b-instruct:free"),
            ("nvidia", "meta/llama-3.3-70b-instruct"),
        ] {
            let settings = Settings {
                llm_provider: Some(provider_str.to_string()),
                llm_model_id: None,
                llm_model_id_override: None,
                ..Settings::default()
            };
            assert_eq!(
                get_effective_model_id(&settings),
                expected_default,
                "default for provider {provider_str:?}"
            );
        }
    }
}
