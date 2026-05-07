// LLM polish IPC contract types — single source of truth for the
// `polish_text` Tauri command and the `polish:failed-fallback` event payload
// that bridges `src-tauri/src/plugins/llm_polish/*` (Rust-owned, M6 chunk 1)
// and the Vue voice-flow store + Settings UI.
//
// Naming convention (per `doc/plans/04-frontend-structure.md`):
//   * Const tuples for closed enums so call sites can both narrow at compile
//     time AND iterate at runtime (e.g. dropdown population).
//   * `Payload` suffix for Tauri event payloads (mirrors `events.ts`).
//
// Field names match the Rust `#[serde(rename_all = "camelCase")]` rendering
// of each struct — see `src-tauri/src/plugins/llm_polish/{providers,registry,
// error}.rs` (chunk 1) for the source-of-truth shape.

// ─── Provider id (Decision #3 — 4 free-tier providers + 2 deferred) ──────

/**
 * Active LLM polish providers in M6. Per Decision #3 the 4 free-tier
 * providers are:
 *   * `'groq'`       — Groq Cloud, OpenAI-compatible (also hosts M3 Whisper)
 *   * `'gemini'`     — Google Gemini, distinct request shape (`generateContent`)
 *   * `'openrouter'` — OpenRouter, OpenAI-compatible aggregator (`:free` models)
 *   * `'nvidia'`     — NVIDIA NIM, OpenAI-compatible (1000 free credits/month)
 */
export type LlmActivePolishProviderId =
  | "groq"
  | "openrouter"
  | "nvidia"
  | "gemini";

/**
 * Full provider id union — includes the 4 active M6 polish providers plus
 * `'openai'` / `'anthropic'` placeholders. The placeholders stay in
 * `LLM_PROVIDERS` (`src/lib/providers.ts`) marked `active: false` so the
 * Settings UI can render them as `(v0.2)` greyed entries without forcing
 * chunk-4 to restructure the dropdown when v0.2 wires them up. They have no
 * polish path in M6 (chunk 1 dispatcher only matches the 4 active ids).
 *
 * **Spec deviation note (chunk 0)**: the kickoff log specifies
 * `'groq' | 'openrouter' | 'nvidia' | 'gemini'` exactly. We widen here so
 * the existing `src/lib/providers.ts` `id: "openai" | "anthropic"` literal
 * entries still compile under `LlmProviderId`. Chunk 4 will own the
 * actual UI flip that removes / re-targets these inactive entries; this
 * deviation isolates the breaking change to chunk 4 rather than rippling
 * through chunk 0. The chunk-1 Rust `LlmProviderId` enum still pins the
 * 4-active set per spec — no Rust-side widening.
 */
export type LlmProviderId = LlmActivePolishProviderId | "openai" | "anthropic";

// ─── Prompt mode (Decision #2 — unit enum + separate custom prompt field) ─

/**
 * Closed list of prompt-mode identifiers. Five modes per Decision #2:
 *   * `default` — generic transcript polish (fix grammar / fillers, preserve voice)
 *   * `email`   — formal tone + paragraph structure
 *   * `chat`    — concise, conversational
 *   * `code`    — preserve code fences, technical terms
 *   * `custom`  — use user-supplied prompt from `Settings.llmCustomPrompt`
 *
 * Stored in `Settings.llmPromptMode` (camelCase). Rust mirrors with
 * `PromptMode` unit enum + `#[serde(rename_all = "lowercase")]`. The
 * separate `Settings.llmCustomPrompt: string | undefined` keeps the custom
 * text out of the discriminated union (enum stays unit-only — simpler
 * matching in chunk 1's `system_prompt` dispatcher).
 *
 * Exported as a const tuple so Vitest can assert `length === 5` and the
 * Settings UI can map over it for radio-group population without
 * hardcoding the order.
 */
export const LLM_PROMPT_MODES = [
  "default",
  "email",
  "chat",
  "code",
  "custom",
] as const;

export type LlmPromptMode = (typeof LLM_PROMPT_MODES)[number];

// ─── Model registry shape (chunk 1 fills LLM_MODEL_LIST in src/lib/) ──────

/**
 * Per-model metadata mirrored from the Rust `LlmModelConfig` registry in
 * `src-tauri/src/plugins/llm_polish/registry.rs` (chunk 1). Exposed to the
 * frontend so the Settings UI can populate the model dropdown grouped by
 * provider without invoking Rust on every render.
 *
 * The registry pins 8 default models (4 providers × 2 models each, see
 * Decision #3 model table). M6 ships these as the closed list; chunk 4's
 * Settings UI hides the `llmModelIdOverride` escape hatch (F4 — user
 * edits `settings.json` by hand for unlisted models).
 */
export interface LlmModelInfo {
  /** Stable model id passed verbatim to the provider API. */
  id: string;
  /** Owning provider id — restricted to the 4 active polish providers. */
  provider: LlmActivePolishProviderId;
  /** Human-readable label for the Settings UI dropdown. */
  displayName: string;
  /** Token context window — informational, not enforced client-side. */
  contextWindow: number;
  /** Whether this model has a free tier (M6 default `true` for all 8). */
  isFree: boolean;
}

// ─── polish_text command result (mirrors Rust PolishResult) ───────────────

/**
 * Result of `invoke<PolishResult>('polish_text', ...)`. Mirrors the Rust
 * struct `PolishResult` in `src-tauri/src/plugins/llm_polish/mod.rs`
 * (`#[serde(rename_all = "camelCase")]`).
 *
 * Token counts are forward-compat for M9 dogfood analytics (Decision #3
 * cascade / F17). M6 itself never displays them; chunk 2's
 * `useVoiceFlowStore` ignores `inputTokens` / `outputTokens` and only reads
 * `polishedText` for the paste path.
 */
export interface PolishResult {
  /** Polished text after sanity checks (F8 prefix-strip + length sanity). */
  polishedText: string;
  /** Wall-clock duration of the LLM round trip in ms. */
  durationMs: number;
  /** Provider-reported prompt token count, or `null` if not exposed. */
  inputTokens: number | null;
  /** Provider-reported completion token count, or `null` if not exposed. */
  outputTokens: number | null;
}

// ─── Polish failure taxonomy (F2 closed enum) ─────────────────────────────

/**
 * Closed set of `polish:failed-fallback` reason codes. Each variant maps 1:1
 * to a Rust `PolishError` variant (chunk 1 `error.rs`); the Rust side
 * sanitizes provider error bodies into one of these strings so the frontend
 * never sees raw HTTP payloads or vendor-specific error JSON.
 *
 * Variants:
 *   * `network`            — generic transport failure (no DNS-specific
 *                             classification surfaces here)
 *   * `rate_limited`       — HTTP 429 / `Retry-After` upstream
 *   * `auth`               — HTTP 401 / 403 (key revoked / wrong scope)
 *   * `parse`              — response shape mismatch (missing
 *                             `choices[0].message.content` / Gemini parts)
 *   * `timeout`            — request budget exceeded (3s test / 15s polish)
 *   * `server_error`       — HTTP 5xx
 *   * `safety_blocked`     — Gemini `finishReason: SAFETY` or OAI-compat
 *                             `choices[0].finish_reason: content_filter`
 *   * `empty_response`     — LLM returned empty / whitespace-only text
 *   * `truncated`          — `finish_reason: length` and output < raw input
 *   * `implausible_output` — sanity check failed (e.g. `As an AI...`,
 *                             output > 3× input)
 *   * `busy`               — `polish_busy` AtomicBool guard rejected
 *                             concurrent invocation
 *   * `cancelled`          — explicit cancellation (chunk 2 reserved;
 *                             ESC-during-enhancing currently no-op)
 *
 * Exported as a const tuple so Vitest can assert `length === 12` and the
 * chunk-3 HUD warning text can branch on the closed set without falling
 * back to a default.
 */
export const POLISH_FAILURE_REASONS = [
  "network",
  "rate_limited",
  "auth",
  "parse",
  "timeout",
  "server_error",
  "safety_blocked",
  "empty_response",
  "truncated",
  "implausible_output",
  "busy",
  "cancelled",
] as const;

export type PolishFailureReason = (typeof POLISH_FAILURE_REASONS)[number];

/**
 * Payload of the `polish:failed-fallback` event emitted by Rust when the
 * polish pipeline fell back to raw transcript paste (or both retry attempts
 * failed when retry is enabled, per Decision #5).
 *
 * `providerId` is restricted to `LlmActivePolishProviderId` — the chunk-1
 * Rust dispatcher only matches the 4 active polish providers (groq /
 * openrouter / nvidia / gemini), so a fallback event for openai / anthropic
 * is statically impossible in M6. The chunk-3 HUD warning + chunk-4 Settings
 * banner can name the responsible provider ("Groq polish failed → pasted
 * raw transcript"). Exact wording lives in i18n keys
 * (`polishError.{reason}` / `hud.warning.polishFailed`, F32).
 */
export interface PolishFallbackPayload {
  reason: PolishFailureReason;
  providerId: LlmActivePolishProviderId;
}
