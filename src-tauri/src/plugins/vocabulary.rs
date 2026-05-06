// Vocabulary helpers shared between transcription (Whisper prompt prefix) and
// LLM polish (system prompt vocabulary injection). Lives at the `plugins/`
// top level rather than inside a sub-folder so both
// `transcription/parser.rs::format_whisper_prompt` and
// `llm_polish/prompts.rs::inject_vocabulary` can `use crate::plugins::
// vocabulary::cap_terms` without crossing a sub-module boundary (F10).
//
// **Cap policy**: 50 terms / 600 chars (counted via `chars().count()` so
// multi-byte CJK terms are budgeted by visible character count, not byte
// length). Truncation is "drop from the end" — once adding the next term
// would exceed `max_chars`, we stop. The first term is added without a
// separator; subsequent terms cost `2 + char_count` (", " + chars) toward
// the cap. Empty / whitespace-only entries are skipped silently so callers
// can pass user-supplied lists without pre-filtering.
//
// The 50/600 numbers come from M3 Q3 (Groq Whisper undocumented prompt
// limit ~896 chars; 600 leaves headroom for the "Important Vocabulary: "
// prefix and avoids surprise truncation). LLM polish uses the same cap so
// system prompt + vocabulary stays comfortably under any provider's typical
// system-message budget.

/// Cap a vocabulary list to a maximum number of terms AND a maximum total
/// character count when joined with ", " separator. Used by both Whisper
/// transcription prompt (transcription::parser::format_whisper_prompt) and
/// LLM polish system prompt (llm_polish::prompts::inject_vocabulary).
///
/// Caller invariants:
/// - `max_terms` is the hard cap on number of terms (never exceeded)
/// - `max_chars` is the cap on cumulative chars (counted as `chars().count()`,
///   not bytes — accounts for multi-byte CJK)
/// - Returns terms in input order; truncates from the end when caps hit
/// - Empty terms (after `trim()`) are skipped
///
/// Returns borrowed `&str` slices into the input so callers can `join` /
/// format without re-allocating each term. The returned `Vec` allocates
/// (length proportional to accepted-term count) but the strings themselves
/// are shared with the input.
pub(crate) fn cap_terms(terms: &[String], max_terms: usize, max_chars: usize) -> Vec<&str> {
    let mut result = Vec::new();
    let mut total_chars: usize = 0;
    for term in terms {
        let trimmed = term.trim();
        if trimmed.is_empty() {
            continue;
        }
        if result.len() >= max_terms {
            break;
        }
        let term_chars = trimmed.chars().count();
        // 2 chars for ", " separator (none before first term).
        let separator_cost = if result.is_empty() { 0 } else { 2 };
        let prospective = total_chars + separator_cost + term_chars;
        if prospective > max_chars {
            break;
        }
        result.push(trimmed);
        total_chars = prospective;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn empty_input() {
        let result = cap_terms(&[], 50, 600);
        assert!(result.is_empty());
    }

    #[test]
    fn under_cap_returns_all_terms_in_order() {
        let terms = vec![s("foo"), s("bar"), s("baz")];
        let result = cap_terms(&terms, 50, 600);
        assert_eq!(result, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn term_count_cap_truncates_to_max_terms() {
        let terms: Vec<String> = (0..60).map(|i| format!("term{i}")).collect();
        let result = cap_terms(&terms, 50, 99_999);
        assert_eq!(result.len(), 50);
        // Earliest 50 are kept; later ones dropped.
        assert_eq!(result[0], "term0");
        assert_eq!(result[49], "term49");
    }

    #[test]
    fn char_cap_counts_multi_byte_chars_not_bytes() {
        // Each "中文一" is 3 chars × 3 bytes/char = 9 bytes. Char count must
        // win over byte count so a 20-char cap admits 3 such terms (3 + 2 +
        // 3 + 2 + 3 = 13 chars).
        let terms = vec![s("中文一"), s("中文二"), s("中文三")];
        let result_loose = cap_terms(&terms, 50, 20);
        assert_eq!(result_loose.len(), 3, "20-char cap should fit all three");

        // Tighten to 7 chars: only first term fits (3 chars). Adding second
        // costs 2 + 3 = 5 → would be 8, so we stop.
        let result_tight = cap_terms(&terms, 50, 7);
        assert_eq!(result_tight, vec!["中文一"]);
    }

    #[test]
    fn skips_empty_and_whitespace_only_terms() {
        let terms = vec![s("a"), s(""), s("   "), s("\t\n"), s("b")];
        let result = cap_terms(&terms, 50, 600);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn term_trimming_strips_surrounding_whitespace() {
        // The returned slice is the trimmed view, not the raw input — caller
        // doesn't have to re-trim before joining.
        let terms = vec![s("  foo  "), s("\nbar\t")];
        let result = cap_terms(&terms, 50, 600);
        assert_eq!(result, vec!["foo", "bar"]);
    }

    #[test]
    fn char_cap_matches_separator_cost_exactly() {
        // Boundary: cap exactly fits "ab, cd" = 6 chars (2 + 2 sep + 2). 5
        // chars rejects the second term.
        let terms = vec![s("ab"), s("cd")];
        assert_eq!(cap_terms(&terms, 50, 6), vec!["ab", "cd"]);
        assert_eq!(cap_terms(&terms, 50, 5), vec!["ab"]);
    }
}
