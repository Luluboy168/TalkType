// Transcription helpers split out of cloud.rs to keep that file focused on
// the HTTP + retry orchestration. M7 (local whisper.cpp) will reuse
// `format_whisper_prompt` for its `--prompt` flag, so the helper lives here
// instead of inside `cloud::*`.
//
// Public surface (pub(super)):
//
//   * `format_whisper_prompt` — vocabulary list → Whisper prompt with the
//     50-term + 600-char dual cap. M6 chunk 1 extracted the cap policy into
//     `crate::plugins::vocabulary::cap_terms` (F10) so the same caps apply
//     to LLM polish system-prompt vocabulary injection without duplicating
//     the truncation logic.
//   * `parse_groq_response` + `ParsedResponse` — `verbose_json` deserializer
//     extracting the trimmed text plus the per-segment minimum
//     `no_speech_prob`.

use serde::Deserialize;

use crate::plugins::vocabulary;

/// Max characters in the formatted vocabulary prompt body. Groq's
/// undocumented prompt limit is ~896 chars; 600 leaves headroom for the
/// "Important Vocabulary: " prefix and avoids surprise truncation.
pub(super) const VOCABULARY_CHAR_CAP: usize = 600;

/// Max term count regardless of total chars — defense against a 50-entry
/// list of single-byte ASCII terms hitting neither cap individually.
pub(super) const VOCABULARY_TERM_CAP: usize = 50;

/// Format the vocabulary list into a Whisper `prompt` string with the
/// 50-term + 600-char dual cap applied. Returns `None` if the list is empty
/// or every term was rejected by the cap.
///
/// Truncation policy delegates to `vocabulary::cap_terms`. The first term is
/// added without a separator; subsequent terms cost `2 + char_count` (", "
/// + chars) toward the cap. CJK terms count by chars not bytes.
pub(super) fn format_whisper_prompt(vocabulary_terms: Option<&[String]>) -> Option<String> {
    let terms = vocabulary_terms?;
    if terms.is_empty() {
        return None;
    }
    let accepted = vocabulary::cap_terms(terms, VOCABULARY_TERM_CAP, VOCABULARY_CHAR_CAP);
    if accepted.is_empty() {
        return None;
    }
    Some(format!("Important Vocabulary: {}", accepted.join(", ")))
}

/// Subset of Groq's `verbose_json` response we care about. Fields we don't
/// use (timestamps, language, etc.) are skipped — `serde` ignores unknown
/// fields by default which keeps us forward-compatible with API additions.
#[derive(Deserialize)]
struct GroqVerboseJson {
    text: String,
    #[serde(default)]
    segments: Vec<GroqSegment>,
}

#[derive(Deserialize)]
struct GroqSegment {
    /// Whisper's per-segment "this is silence" confidence in [0.0, 1.0].
    /// Lower = more confident speech. Used by M5's noise gate UX.
    #[serde(default)]
    no_speech_prob: f32,
}

/// Parsed response handed back to `cloud.rs`. Intentionally not `Debug` to
/// keep transient API state out of any accidental log output (the trimmed
/// text and `no_speech_prob` ratio aren't sensitive but neither side calls
/// `Debug` on this struct in normal flow).
pub(super) struct ParsedResponse {
    pub(super) text: String,
    pub(super) min_no_speech: Option<f32>,
}

/// Parse Groq's `verbose_json` response. Returns the trimmed transcription
/// text plus the minimum `no_speech_prob` across segments (lower is more
/// confident speech) — useful for the "noise gate" follow-up in M5.
pub(super) fn parse_groq_response(body: &str) -> Result<ParsedResponse, String> {
    let parsed: GroqVerboseJson = serde_json::from_str(body).map_err(|e| {
        format!(
            "verbose_json parse: {e} — body prefix: {}",
            body.chars().take(200).collect::<String>()
        )
    })?;

    let min_no_speech = if parsed.segments.is_empty() {
        None
    } else {
        let m = parsed
            .segments
            .iter()
            .map(|s| s.no_speech_prob)
            .fold(f32::INFINITY, f32::min);
        if m.is_finite() {
            Some(m)
        } else {
            None
        }
    };

    Ok(ParsedResponse {
        text: parsed.text.trim().to_string(),
        min_no_speech,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_whisper_prompt_empty_vocabulary_returns_none() {
        assert!(format_whisper_prompt(Some(&[])).is_none());
        assert!(format_whisper_prompt(None).is_none());
    }

    #[test]
    fn format_whisper_prompt_single_term() {
        let terms = vec!["foo".to_string()];
        let prompt = format_whisper_prompt(Some(&terms)).unwrap();
        assert_eq!(prompt, "Important Vocabulary: foo");
    }

    #[test]
    fn format_whisper_prompt_caps_at_50_terms() {
        let terms: Vec<String> = (0..100).map(|i| format!("term{i}")).collect();
        let prompt = format_whisper_prompt(Some(&terms)).unwrap();
        // 50 terms = 49 commas in the joined body.
        let comma_count = prompt.matches(", ").count();
        assert!(
            comma_count <= 49,
            "too many comma separators: {comma_count}"
        );
        // term0..term49 should appear, term50 should NOT.
        assert!(prompt.contains("term0,"));
        assert!(prompt.contains("term49"));
        assert!(!prompt.contains("term50"));
    }

    #[test]
    fn format_whisper_prompt_caps_at_600_chars() {
        // 50 long ASCII terms × ~24 chars each = ~1200 chars body. Should
        // hit the 600-char cap before the 50-term cap.
        let long: Vec<String> = (0..50)
            .map(|i| format!("verylongtermname_{i:08}"))
            .collect();
        let prompt = format_whisper_prompt(Some(&long)).unwrap();
        let body_len = prompt
            .strip_prefix("Important Vocabulary: ")
            .unwrap()
            .chars()
            .count();
        assert!(body_len <= VOCABULARY_CHAR_CAP, "body chars = {body_len}");
        // Should NOT include all 50 terms — the char cap kicks in first.
        let comma_count = prompt.matches(", ").count();
        assert!(
            comma_count < 49,
            "char cap should fire before 50-term cap: got {comma_count} commas"
        );
    }

    #[test]
    fn format_whisper_prompt_mandarin_terms_count_chars_not_bytes() {
        // 8 Mandarin characters per term × 50 terms = 400 chars + 49 × 2 = 498 chars body.
        // Should fit under 600 — every term accepted.
        let terms: Vec<String> = (0..50).map(|_| "甲乙丙丁戊己庚辛".to_string()).collect();
        let prompt = format_whisper_prompt(Some(&terms)).unwrap();
        let comma_count = prompt.matches(", ").count();
        assert_eq!(comma_count, 49, "all 50 terms should fit");

        // 24 Mandarin chars × 50 terms = 1200 chars. Should hit char cap.
        let long_mandarin: Vec<String> = (0..50)
            .map(|_| "甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉戌亥日月".to_string())
            .collect();
        let prompt = format_whisper_prompt(Some(&long_mandarin)).unwrap();
        let body_len = prompt
            .strip_prefix("Important Vocabulary: ")
            .unwrap()
            .chars()
            .count();
        assert!(
            body_len <= VOCABULARY_CHAR_CAP,
            "mandarin char cap: {body_len}"
        );
        // Char-cap test for Mandarin: assert it kicks in BEFORE 50-term cap.
        let comma_count = prompt.matches(", ").count();
        assert!(
            comma_count < 49,
            "mandarin: char cap should fire first (got {comma_count} commas)"
        );
    }

    #[test]
    fn parse_groq_response_extracts_min_no_speech() {
        let body = r#"{"text":"hi","segments":[{"no_speech_prob":0.8},{"no_speech_prob":0.1},{"no_speech_prob":0.4}]}"#;
        let parsed = parse_groq_response(body).expect("parse");
        assert_eq!(parsed.text, "hi");
        assert!((parsed.min_no_speech.unwrap() - 0.1).abs() < 1e-3);
    }

    #[test]
    fn parse_groq_response_no_segments_yields_none_no_speech() {
        let body = r#"{"text":"hi","segments":[]}"#;
        let parsed = parse_groq_response(body).expect("parse");
        assert!(parsed.min_no_speech.is_none());
    }

    #[test]
    fn parse_groq_response_missing_segments_field_yields_none() {
        // serde_default — a response without segments at all should also work.
        let body = r#"{"text":"hi"}"#;
        let parsed = parse_groq_response(body).expect("parse");
        assert_eq!(parsed.text, "hi");
        assert!(parsed.min_no_speech.is_none());
    }

    #[test]
    fn parse_groq_response_trims_text_whitespace() {
        let body = r#"{"text":"  hi  ","segments":[]}"#;
        let parsed = parse_groq_response(body).expect("parse");
        assert_eq!(parsed.text, "hi");
    }

    #[test]
    fn parse_groq_response_malformed_json_yields_error() {
        // Match by hand because ParsedResponse is intentionally not Debug.
        let body = "not json at all";
        if parse_groq_response(body).is_ok() {
            panic!("expected parse error");
        }
    }
}
