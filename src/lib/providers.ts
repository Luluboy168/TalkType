// Provider metadata registry — static list of LLM / Whisper providers
// supported by TalkType. Kept in `src/lib/` per the dependency-direction
// rule (`stores/` and `views/` may import this; `lib/` may not depend on
// stores/views). Used by the Settings API key section to render the
// provider dropdown, console-URL link, and key-prefix placeholder.
//
// **Phase 1 / M3** only activates `groq`. The other three are listed with
// `active: false` so the dropdown shape is finalized — M6 only flips the
// boolean rather than restructuring the UI.
//
// Adding a new provider requires updating BOTH this file and the
// `ALLOWED_PROVIDERS` allowlist in `src-tauri/src/plugins/credentials.rs`.

import type { LlmProviderId, ProviderInfo } from "@/types";

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
  {
    id: "gemini",
    displayName: "Gemini",
    consoleUrl: "https://aistudio.google.com/apikey",
    expectedPrefix: "",
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
