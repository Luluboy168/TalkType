// Vocabulary helpers shared between transcription (Whisper prompt prefix)
// and LLM polish (system prompt vocabulary injection). Lives at the
// `plugins/` top level rather than inside a sub-folder so both
// `transcription/parser.rs::format_whisper_prompt` and
// `llm_polish/prompts.rs::inject_vocabulary` can `use crate::plugins::
// vocabulary::cap_terms` without crossing a sub-module boundary.
//
// Chunk 0 ships this stub so the module declaration in `plugins/mod.rs`
// does not break the build. The real `cap_terms` helper (50 terms / 600
// chars cap, mirrors M3 既定) lands in chunk 1 with 4 boundary tests
// (empty / under cap / term-cap / char-cap edge) plus the
// `transcription::parser` refactor that switches over to the shared impl.

#[allow(dead_code)]
pub(crate) fn _placeholder() {}
