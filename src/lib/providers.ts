// Provider metadata registry — static list of LLM / Whisper providers
// supported by TalkType. Kept in `src/lib/` per the dependency-direction
// rule (`stores/` and `views/` may import this; `lib/` may not depend on
// stores/views). Used by the Settings API key section to render the
// provider dropdown, console-URL link, and key-prefix placeholder, and by
// the M6 LLM polish section to populate the provider + model dropdowns.
//
// **M6 chunk 4 (Decision #3 — 4 free-tier providers)**: flips Groq +
// OpenRouter + NVIDIA NIM + Gemini to `active: true`. OpenAI + Anthropic
// stay `active: false` because they have no genuine free tier — defer
// activation to v0.2.
//
// Adding a new provider requires updating BOTH this file and the
// `ALLOWED_PROVIDERS` allowlist in `src-tauri/src/plugins/credentials.rs`.
//
// **`LLM_MODEL_LIST`** mirrors the Rust source-of-truth registry at
// `src-tauri/src/plugins/llm_polish/registry.rs` (chunk 1). 8 models
// pinned: 4 free-tier providers × 2 models each. The first model per
// provider is its default — chunk 4's Settings UI seeds the model dropdown
// from the order in this array.

import type {
  LlmActivePolishProviderId,
  LlmModelInfo,
  LlmProviderId,
  ProviderInfo,
} from "@/types";

/**
 * Static metadata for every provider TalkType can authenticate against.
 * Order matters — `groq` first because it's the Phase 1 default and the
 * dropdown's preselected option.
 */
export const LLM_PROVIDERS: readonly ProviderInfo[] = [
  {
    id: "groq",
    displayName: "Groq",
    consoleUrl: "https://console.groq.com/keys",
    expectedPrefix: "gsk_",
    active: true,
  },
  {
    id: "openrouter",
    displayName: "OpenRouter",
    consoleUrl: "https://openrouter.ai/keys",
    expectedPrefix: "sk-or-",
    active: true,
  },
  {
    id: "nvidia",
    displayName: "NVIDIA NIM",
    consoleUrl: "https://build.nvidia.com/explore/discover",
    expectedPrefix: "nvapi-",
    active: true,
  },
  {
    id: "gemini",
    displayName: "Gemini",
    consoleUrl: "https://aistudio.google.com/apikey",
    expectedPrefix: "",
    active: true,
  },
  {
    id: "openai",
    displayName: "OpenAI",
    consoleUrl: "https://platform.openai.com/api-keys",
    expectedPrefix: "sk-",
    active: false,
  },
  {
    id: "anthropic",
    displayName: "Anthropic",
    consoleUrl: "https://console.anthropic.com/settings/keys",
    expectedPrefix: "sk-ant-",
    active: false,
  },
] as const;

/**
 * Look up a provider by id. Returns `undefined` if the id is not in the
 * registry — callers should narrow the type via `LlmProviderId` before
 * calling, so the only way to hit `undefined` in practice is if a new id
 * is added to `LlmProviderId` without updating this list.
 */
export function findProvider(id: LlmProviderId): ProviderInfo | undefined {
  return LLM_PROVIDERS.find((provider) => provider.id === id);
}

/**
 * Pinned LLM polish model registry — 8 entries (4 active providers × 2
 * models each). Mirrors the Rust `LLM_MODEL_LIST` in
 * `src-tauri/src/plugins/llm_polish/registry.rs` so the Settings UI can
 * populate the model dropdown without invoking Rust on every render.
 *
 * Order is significant: the first model per provider is its default
 * (consumed by `getDefaultModelId`). A vitest assertion verifies the
 * registry length stays >= 8 and that each active provider ships >= 2
 * models — drift triggers a deliberate update on both sides of the IPC
 * boundary.
 */
export const LLM_MODEL_LIST: readonly LlmModelInfo[] = [
  // ─── Groq ────────────────────────────────────────────────────────────
  {
    id: "llama-3.3-70b-versatile",
    provider: "groq",
    displayName: "Llama 3.3 70B (Versatile)",
    contextWindow: 128_000,
    isFree: true,
  },
  {
    id: "llama-3.1-8b-instant",
    provider: "groq",
    displayName: "Llama 3.1 8B (Instant)",
    contextWindow: 128_000,
    isFree: true,
  },
  // ─── Gemini ──────────────────────────────────────────────────────────
  {
    id: "gemini-2.0-flash",
    provider: "gemini",
    displayName: "Gemini 2.0 Flash",
    contextWindow: 1_000_000,
    isFree: true,
  },
  {
    id: "gemini-1.5-flash",
    provider: "gemini",
    displayName: "Gemini 1.5 Flash",
    contextWindow: 1_000_000,
    isFree: true,
  },
  // ─── OpenRouter (free `:free` models) ────────────────────────────────
  {
    id: "meta-llama/llama-3.3-70b-instruct:free",
    provider: "openrouter",
    displayName: "Llama 3.3 70B (OpenRouter free)",
    contextWindow: 128_000,
    isFree: true,
  },
  {
    id: "qwen/qwen-2.5-72b-instruct:free",
    provider: "openrouter",
    displayName: "Qwen 2.5 72B (OpenRouter free)",
    contextWindow: 128_000,
    isFree: true,
  },
  // ─── NVIDIA NIM (free credits) ───────────────────────────────────────
  {
    id: "meta/llama-3.3-70b-instruct",
    provider: "nvidia",
    displayName: "Llama 3.3 70B (NVIDIA NIM)",
    contextWindow: 128_000,
    isFree: true,
  },
  {
    id: "nvidia/llama-3.1-nemotron-70b-instruct",
    provider: "nvidia",
    displayName: "Nemotron 70B (NVIDIA tuned)",
    contextWindow: 128_000,
    isFree: true,
  },
] as const;

/**
 * All models for a given provider, in registry order. Used by chunk 4's
 * Settings UI to seed the model dropdown after the user picks a provider.
 */
export function getModelsByProvider(
  provider: LlmActivePolishProviderId,
): readonly LlmModelInfo[] {
  return LLM_MODEL_LIST.filter((m) => m.provider === provider);
}

/**
 * Default model id for a provider — the first entry in `LLM_MODEL_LIST`
 * whose `provider` matches. Mirrors the Rust `get_default_model_id`
 * dispatcher so a provider switch in the UI without an explicit model
 * picks the same default Rust would resolve.
 *
 * Throws if the provider has no models in the registry — should never
 * happen given the chunk-4 invariant of >= 2 models per active provider.
 */
export function getDefaultModelId(
  provider: LlmActivePolishProviderId,
): string {
  const first = LLM_MODEL_LIST.find((m) => m.provider === provider);
  if (!first) {
    throw new Error(
      `[providers] LLM_MODEL_LIST has no entries for provider ${provider}`,
    );
  }
  return first.id;
}
