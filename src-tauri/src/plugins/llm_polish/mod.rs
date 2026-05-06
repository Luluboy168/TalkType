// LLM polish plugin — multi-provider chat-completions wrapper (M6).
//
// Chunk 0 ships only this placeholder so the parent `plugins/mod.rs` can
// declare the module without dragging in the implementation surface yet.
// The real layout lands in chunk 1:
//   * `error.rs`     — `PolishError` thiserror enum + `PolishFailureReason`
//                       closed enum (mirrors frontend `src/types/llm.ts`).
//   * `providers.rs` — 4 provider request builders + response parsers
//                       (Groq / OpenRouter / NVIDIA NIM / Gemini, all
//                       free-tier models per Decision #3).
//   * `prompts.rs`   — 5 prompt-mode system prompts × 2 langs + custom
//                       prompt validation (chars().count() ≤ 1000).
//   * `registry.rs`  — `LlmModelConfig` + `LLM_MODEL_LIST` (≥ 8 models).
//   * `health.rs`    — `test_provider_connection` for the 3 non-Groq
//                       providers (Groq stays in `transcription/health.rs`
//                       per F20).
//
// Chunks 0-1 do not register any Tauri command from this module yet; the
// `polish_text` command surface lands in chunk 1's `mod.rs` rewrite.

#[allow(dead_code)]
pub(crate) fn _placeholder() {}
