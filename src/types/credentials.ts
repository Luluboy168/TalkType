// Credentials IPC contract types — single source of truth for the API key
// management surface that bridges `src-tauri/src/plugins/credentials.rs` and
// the Settings UI (`src/views/SettingsView.vue` API key section, M3 chunk-1).
//
// Provider id strings MUST match the Rust `ALLOWED_PROVIDERS` allowlist in
// `plugins/credentials.rs`. Adding a new provider requires updating both.
//
// **Security note**: there is intentionally NO `getCredential`-like type
// here. The frontend cannot read API key contents — only existence
// (`hasCredential`), set (`setCredential`), or delete (`deleteCredential`).
// See `doc/plans/01-architecture.md` invariant #1.

/**
 * LLM / Whisper provider identifier. Used both for credential storage
 * (`set_credential` / `has_credential` / `delete_credential` Tauri commands)
 * and as the discriminator for provider-specific request shapes (M3 cloud
 * transcription, M6 LLM polish).
 *
 * Phase 1 (M3) only activates `groq`. The other three are pre-listed so the
 * Settings UI dropdown can render them as `(M6+)` placeholders without
 * needing structural changes once M6 lands.
 */
export type LlmProviderId = "groq" | "openai" | "anthropic" | "gemini";

/**
 * Static metadata for one provider — display name, console URL for the
 * "Get an API key" external link, and the expected key prefix used to
 * shape-validate user input. Kept in `src/lib/providers.ts` (the
 * `LLM_PROVIDERS` constant) rather than fetched from Rust — this is a
 * compile-time list that doesn't change at runtime.
 *
 * `expectedPrefix` may be empty: Gemini keys do not follow a single prefix
 * convention (some are `AIza`-prefixed, others base64-shaped), so the Rust
 * `validate_and_clean_key` skips the soft prefix check when the expected
 * prefix is empty.
 */
export interface ProviderInfo {
  /** Provider id matching `LlmProviderId`. */
  id: LlmProviderId;
  /** Human-readable name shown in the Settings dropdown. */
  displayName: string;
  /** External URL to the provider's API key console (Settings UI link). */
  consoleUrl: string;
  /** Expected key prefix (e.g. `gsk_` for Groq). Empty string disables the prefix check. */
  expectedPrefix: string;
  /** True when the provider is wired up end-to-end in the current milestone.
   * Phase 1 / M3 only sets this for `groq`; UI uses it to disable the
   * "Save" button and show an `(M6+)` suffix for the others. */
  active: boolean;
}

/**
 * Successful result of `invoke<TestConnectionResult>('test_provider_connection', ...)`.
 *
 * Mirrors the Rust struct `TestConnectionResult` in
 * `src-tauri/src/plugins/transcription/health.rs`
 * (`#[serde(rename_all = "camelCase")]`). Keep the two in sync per
 * architecture invariant #8.
 *
 * `modelCount` is `null` when the provider's `/models` response shape didn't
 * include a `data` array (defensive — current Groq API always returns one,
 * but the test path tolerates future shape changes).
 */
export interface TestConnectionResult {
  /** Always true on success — provider responded with HTTP 200 + parseable body. */
  ok: boolean;
  /** Echoed provider id from the request (e.g. `"groq"`). */
  provider: LlmProviderId;
  /** Number of models the provider reports as available. `null` when unparseable. */
  modelCount: number | null;
}
