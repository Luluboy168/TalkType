// LLM polish system-prompt strings + custom-prompt validation + vocabulary
// injection (M6 chunk 1, F11 + F12).
//
// 5 prompt modes × 2 langs = 10 const string slots. Strings are ASCII +
// CJK only (no markdown, no template substitution) so the LLM gets the
// instruction verbatim. Wording is intentionally direct — these are
// shipped to chat-completion `system` messages and any flowery language
// reduces the LLM's adherence to the polish-only contract.
//
// **Custom prompt validation (F11)**: 1000-char cap counted via
// `chars().count()`, so a 1000-char zh-TH prompt (3000 bytes UTF-8) is
// accepted. Empty / whitespace-only input is rejected as `EmptyInput`
// rather than silently treated as no-op so chunk 4's UI shows the user a
// concrete error.
//
// **Vocabulary injection (F10 + F33 cascade)**: vocabulary terms are
// joined and appended to the system prompt under a `<vocabulary>...</
// vocabulary>` tag, preceded by an explicit "preserve these terms"
// instruction. Cap policy delegates to `vocabulary::cap_terms` (50 terms /
// 600 chars).

use super::error::PolishError;
use crate::plugins::vocabulary;

/// Closed enum of prompt modes. Mirror of TS `LLM_PROMPT_MODES` 5-tuple in
/// `src/types/llm.ts`. Custom mode keeps the prompt body in
/// `Settings.llm_custom_prompt` rather than embedding inside the enum
/// variant — Decision #2 keeps the enum unit-only for simpler `match`
/// dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    Default,
    Email,
    Chat,
    Code,
    Custom,
}

impl PromptMode {
    /// Parse from a `Settings.llm_prompt_mode` string. Returns `None` for
    /// unknown values so the orchestrator can surface
    /// `PolishError::ParseError` rather than silently fall through to
    /// `Default`. Intentionally `Option<Self>` rather than the
    /// `std::str::FromStr` trait so the caller can branch on `None`
    /// without constructing an error type.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "default" => Some(Self::Default),
            "email" => Some(Self::Email),
            "chat" => Some(Self::Chat),
            "code" => Some(Self::Code),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

// ─── Prompt const strings (5 modes × 2 langs = 10) ───────────────────────
//
// Wording deliberately direct — system messages don't tolerate floweriness.
// All four non-custom modes share a "preserve speaker tone, only fix
// surface issues" core; the variation is in what counts as a "surface
// issue" per mode.

const PROMPT_DEFAULT_ZH: &str = "你是語音轉錄文字的潤飾助手。任務:移除「呃」「嗯」「那個」等語助詞、修正標點符號、修正明顯的轉錄錯誤。\n規則:\n1. 保留說話者原本的語氣與風格,不改寫意思。\n2. 不擴寫、不總結、不加新內容。\n3. 直接輸出潤飾後的文字,不要前綴(如「以下是潤飾後的版本」),不要解釋,不要使用 Markdown。\n4. 若無需修改,原樣輸出即可。";

const PROMPT_DEFAULT_EN: &str = "You polish speech-to-text transcripts. Task: remove filler words (um, uh, like, you know), fix punctuation, correct obvious transcription errors.\nRules:\n1. Preserve the speaker's tone and style; do not rewrite meaning.\n2. No expansion, summarization, or added content.\n3. Output the polished text directly. No prefix (e.g. \"Here's the polished version\"), no explanation, no markdown.\n4. If nothing needs fixing, output the input as-is.";

const PROMPT_EMAIL_ZH: &str = "你是電子郵件起草助手。任務:把語音轉錄文字改寫成正式的書面郵件段落,維持原本意思但讓語氣更專業。\n規則:\n1. 移除口語化用詞與語助詞,改用正式書面語。\n2. 把零碎句子整理成完整段落,但不增加原文沒有的資訊。\n3. 直接輸出改寫後的文字,不要主旨、不要稱謂、不要署名,不要前綴或解釋,不要使用 Markdown。";

const PROMPT_EMAIL_EN: &str = "You draft formal email paragraphs. Task: rewrite speech-to-text transcripts into professional written prose while preserving the original meaning.\nRules:\n1. Remove colloquialisms and filler words; use formal written register.\n2. Consolidate fragmentary sentences into full paragraphs, but do not add information beyond the source.\n3. Output the rewritten text directly. No subject line, no greeting, no signature, no prefix or explanation, no markdown.";

const PROMPT_CHAT_ZH: &str = "你是即時通訊文字的潤飾助手。任務:把語音轉錄文字整理成簡短、口語、自然的聊天訊息。\n規則:\n1. 保留說話者的口語感,但移除明顯的語助詞與重複。\n2. 句子要簡短直接,不要書面化、不要正式化。\n3. 直接輸出潤飾後的文字,不要前綴或解釋,不要使用 Markdown。";

const PROMPT_CHAT_EN: &str = "You polish casual chat messages. Task: clean up speech-to-text transcripts into short, conversational, natural chat lines.\nRules:\n1. Preserve the spoken feel; remove obvious fillers and repetition.\n2. Keep sentences short and direct; do not formalize or written-ify.\n3. Output the polished text directly. No prefix or explanation, no markdown.";

const PROMPT_CODE_ZH: &str = "你是程式碼相關語音轉錄文字的潤飾助手。任務:修正轉錄錯誤,但完整保留所有技術術語、函式名、變數名、檔名、路徑。\n規則:\n1. 技術術語(API、function、TypeScript、Rust、SQL、HTTP 等)維持原樣大小寫。\n2. 程式碼片段用 backtick 包起來:`getUserById`、`Result<T, E>`。\n3. 不要把 camelCase / snake_case / kebab-case 名稱「翻譯」成中文。\n4. 直接輸出潤飾後的文字,不要前綴或解釋。";

const PROMPT_CODE_EN: &str = "You polish coding-context transcripts. Task: fix transcription errors but preserve all technical terms, function names, variable names, file names, and paths exactly.\nRules:\n1. Keep technical terms (API, function, TypeScript, Rust, SQL, HTTP, etc.) in their original casing.\n2. Wrap inline code in backticks: `getUserById`, `Result<T, E>`.\n3. Do not \"translate\" camelCase / snake_case / kebab-case names into prose.\n4. Output the polished text directly. No prefix or explanation.";

/// Resolve the system prompt for a given mode + UI language. `lang` is a
/// BCP-47-ish prefix (e.g. `"zh-TW"`, `"en"`) — anything that starts with
/// `"zh"` gets the zh-TW prompts, everything else gets English. Custom
/// mode returns the user-supplied (already-validated) string.
///
/// **Lang fallback**: chunk 1 doesn't have a `Settings.language_ui` field
/// yet; the caller in `mod.rs` defaults to `"zh-TW"` for now. Chunk 2 may
/// pass through whatever Settings expose by then.
pub fn system_prompt(mode: PromptMode, lang: &str, custom: Option<&str>) -> String {
    let zh = lang.starts_with("zh");
    match mode {
        PromptMode::Default => if zh {
            PROMPT_DEFAULT_ZH
        } else {
            PROMPT_DEFAULT_EN
        }
        .to_string(),
        PromptMode::Email => if zh { PROMPT_EMAIL_ZH } else { PROMPT_EMAIL_EN }.to_string(),
        PromptMode::Chat => if zh { PROMPT_CHAT_ZH } else { PROMPT_CHAT_EN }.to_string(),
        PromptMode::Code => if zh { PROMPT_CODE_ZH } else { PROMPT_CODE_EN }.to_string(),
        PromptMode::Custom => custom.unwrap_or("").to_string(),
    }
}

/// Validate `Settings.llm_custom_prompt` (F11). Returns the trimmed
/// prompt body on success.
///
/// Rules:
///   * Trim leading/trailing whitespace.
///   * Reject empty result → `PolishError::EmptyInput`.
///   * Reject `chars().count() > 1000` → `PolishError::InvalidPromptLength`
///     with `actual` and `max: 1000`. CJK content (3 bytes/char) gets the
///     same 1000-char budget as ASCII so the cap stays user-comprehensible
///     ("under 1000 characters").
pub fn validate_custom_prompt(s: &str) -> Result<String, PolishError> {
    const MAX_CHARS: usize = 1000;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(PolishError::EmptyInput);
    }
    let char_count = trimmed.chars().count();
    if char_count > MAX_CHARS {
        return Err(PolishError::InvalidPromptLength {
            actual: char_count,
            max: MAX_CHARS,
        });
    }
    Ok(trimmed.to_string())
}

/// Append the user's vocabulary list to the system prompt. Returns the
/// prompt unchanged when `vocab` is empty (or every term is rejected by
/// the cap). Format:
///
/// ```text
/// {original system prompt}
///
/// <vocabulary>term1, term2, term3</vocabulary>
/// Preserve these specialized terms exactly as written.
/// ```
///
/// Cap policy delegates to `vocabulary::cap_terms(50, 600)` (F10) — same
/// budget as the Whisper transcription prompt to keep total system-message
/// payload comfortably under any provider's typical budget.
pub fn inject_vocabulary(prompt: &str, vocab: &[String]) -> String {
    if vocab.is_empty() {
        return prompt.to_string();
    }
    let capped = vocabulary::cap_terms(vocab, 50, 600);
    if capped.is_empty() {
        return prompt.to_string();
    }
    let vocab_str = capped.join(", ");
    format!(
        "{prompt}\n\n<vocabulary>{vocab_str}</vocabulary>\nPreserve these specialized terms exactly as written."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── PromptMode parsing ──────────────────────────────────────────────

    #[test]
    fn from_str_round_trips_5_modes() {
        assert_eq!(PromptMode::from_str("default"), Some(PromptMode::Default));
        assert_eq!(PromptMode::from_str("email"), Some(PromptMode::Email));
        assert_eq!(PromptMode::from_str("chat"), Some(PromptMode::Chat));
        assert_eq!(PromptMode::from_str("code"), Some(PromptMode::Code));
        assert_eq!(PromptMode::from_str("custom"), Some(PromptMode::Custom));
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert_eq!(PromptMode::from_str(""), None);
        assert_eq!(PromptMode::from_str("DEFAULT"), None); // case-sensitive
        assert_eq!(PromptMode::from_str("default "), None); // no trim
        assert_eq!(PromptMode::from_str("essay"), None);
    }

    // ─── system_prompt 5 modes × 2 langs = 10 string slots ────────────────

    #[test]
    fn default_zh_prompt_mentions_filler_word_examples() {
        let p = system_prompt(PromptMode::Default, "zh-TW", None);
        assert!(!p.is_empty());
        // Filler words for zh prompt — content must mention at least one
        // common filler so the LLM has a concrete example.
        assert!(
            p.contains("呃") || p.contains("嗯") || p.contains("那個"),
            "default zh prompt should list common Chinese fillers, got: {p}"
        );
    }

    #[test]
    fn default_en_prompt_mentions_filler_words() {
        let p = system_prompt(PromptMode::Default, "en", None);
        assert!(!p.is_empty());
        let lower = p.to_lowercase();
        assert!(
            lower.contains("filler") || lower.contains("um") || lower.contains("uh"),
            "default en prompt should mention filler words, got: {p}"
        );
    }

    #[test]
    fn email_mode_prompts_mention_formal_register() {
        let zh = system_prompt(PromptMode::Email, "zh-TW", None);
        assert!(zh.contains("正式") || zh.contains("專業"));
        let en = system_prompt(PromptMode::Email, "en", None);
        assert!(en.to_lowercase().contains("formal") || en.to_lowercase().contains("professional"));
    }

    #[test]
    fn chat_mode_prompts_mention_short_conversational() {
        let zh = system_prompt(PromptMode::Chat, "zh-TW", None);
        assert!(zh.contains("聊天") || zh.contains("簡短") || zh.contains("口語"));
        let en = system_prompt(PromptMode::Chat, "en", None);
        let lower = en.to_lowercase();
        assert!(
            lower.contains("chat") || lower.contains("short") || lower.contains("conversational")
        );
    }

    #[test]
    fn code_mode_prompts_mention_technical_terms_or_backticks() {
        let zh = system_prompt(PromptMode::Code, "zh-TW", None);
        assert!(zh.contains("API") || zh.contains("function") || zh.contains("backtick"));
        let en = system_prompt(PromptMode::Code, "en", None);
        let lower = en.to_lowercase();
        assert!(lower.contains("api") || lower.contains("function") || lower.contains("backtick"));
    }

    #[test]
    fn custom_mode_returns_user_supplied_text_when_present() {
        let custom = "Polish only the punctuation. Preserve everything else.";
        let p = system_prompt(PromptMode::Custom, "en", Some(custom));
        assert_eq!(p, custom);
    }

    #[test]
    fn custom_mode_returns_empty_when_no_user_text() {
        // Defensive: orchestrator should validate before reaching here, but
        // an empty custom param yields empty prompt rather than panic.
        let p = system_prompt(PromptMode::Custom, "en", None);
        assert!(p.is_empty());
    }

    #[test]
    fn lang_fallback_non_zh_picks_english() {
        // Anything that doesn't start with "zh" goes to English. Includes
        // unknown locales so a future "ja" / "ko" user gets English rather
        // than zh-TW (chunk 4 may add ja/ko prompts later — see IDEAS).
        for lang in ["en", "en-US", "fr", "ja", "ko", ""] {
            let zh = system_prompt(PromptMode::Default, "zh-TW", None);
            let other = system_prompt(PromptMode::Default, lang, None);
            assert_ne!(zh, other, "lang {lang:?} should NOT pick zh prompt");
        }
    }

    #[test]
    fn lang_zh_variants_all_pick_zh_prompt() {
        let canonical = system_prompt(PromptMode::Default, "zh-TW", None);
        for lang in ["zh", "zh-TW", "zh-CN", "zh-Hant", "zh-Hans"] {
            let p = system_prompt(PromptMode::Default, lang, None);
            assert_eq!(p, canonical, "lang {lang:?} should match zh-TW prompt");
        }
    }

    // ─── F11 validate_custom_prompt boundary tests ───────────────────────

    #[test]
    fn validate_custom_prompt_accepts_999_chars() {
        let s = "a".repeat(999);
        let result = validate_custom_prompt(&s).expect("999 chars OK");
        assert_eq!(result.chars().count(), 999);
    }

    #[test]
    fn validate_custom_prompt_accepts_exactly_1000_chars() {
        // Boundary: 1000 must be inclusive (max).
        let s = "a".repeat(1000);
        let result = validate_custom_prompt(&s).expect("1000 chars OK");
        assert_eq!(result.chars().count(), 1000);
    }

    #[test]
    fn validate_custom_prompt_rejects_1001_chars_with_actual_count() {
        let s = "a".repeat(1001);
        let err = validate_custom_prompt(&s).unwrap_err();
        match err {
            PolishError::InvalidPromptLength { actual, max } => {
                assert_eq!(actual, 1001);
                assert_eq!(max, 1000);
            }
            other => panic!("expected InvalidPromptLength, got {other:?}"),
        }
    }

    #[test]
    fn validate_custom_prompt_zh_tw_1000_chars_3000_bytes_accepted() {
        // 1000 zh-TW chars × 3 bytes/char = 3000 bytes UTF-8. Must be
        // accepted because the cap is char-count, not byte-count.
        let s: String = "中".repeat(1000);
        assert_eq!(s.len(), 3000); // bytes
        assert_eq!(s.chars().count(), 1000); // chars
        let result = validate_custom_prompt(&s).expect("1000 zh chars OK");
        assert_eq!(result.chars().count(), 1000);
    }

    #[test]
    fn validate_custom_prompt_rejects_empty_or_whitespace() {
        for input in ["", "   ", "\n\t\r", "  \n  "] {
            let err = validate_custom_prompt(input).unwrap_err();
            assert!(
                matches!(err, PolishError::EmptyInput),
                "input {input:?} should be EmptyInput"
            );
        }
    }

    #[test]
    fn validate_custom_prompt_trims_surrounding_whitespace() {
        let result = validate_custom_prompt("  hello  ").expect("trim OK");
        assert_eq!(result, "hello");
    }

    // ─── F10 inject_vocabulary tests ─────────────────────────────────────

    #[test]
    fn inject_vocabulary_empty_list_returns_prompt_unchanged() {
        let p = "system prompt";
        assert_eq!(inject_vocabulary(p, &[]), "system prompt");
    }

    #[test]
    fn inject_vocabulary_appends_under_xml_tag() {
        let p = "system prompt";
        let vocab = vec!["TalkType".to_string(), "Tauri".to_string()];
        let result = inject_vocabulary(p, &vocab);
        assert!(result.starts_with("system prompt"));
        assert!(result.contains("<vocabulary>TalkType, Tauri</vocabulary>"));
        assert!(result.contains("Preserve these specialized terms exactly as written."));
    }

    #[test]
    fn inject_vocabulary_caps_at_50_terms() {
        let p = "sys";
        let vocab: Vec<String> = (0..60).map(|i| format!("term{i}")).collect();
        let result = inject_vocabulary(p, &vocab);
        // term49 should appear, term50 should NOT (50-term cap).
        assert!(result.contains("term49"));
        assert!(!result.contains("term50"));
    }

    #[test]
    fn inject_vocabulary_caps_at_600_chars() {
        let p = "sys";
        // 50 long ASCII terms × ~24 chars × 2 sep = ~1300 chars. Should
        // hit the 600-char cap before the 50-term cap.
        let vocab: Vec<String> = (0..50)
            .map(|i| format!("verylongtermname_{i:08}"))
            .collect();
        let result = inject_vocabulary(p, &vocab);
        // Extract the contents inside <vocabulary>...</vocabulary>
        let start = result.find("<vocabulary>").unwrap() + "<vocabulary>".len();
        let end = result.find("</vocabulary>").unwrap();
        let body = &result[start..end];
        assert!(
            body.chars().count() <= 600,
            "vocabulary body chars = {}",
            body.chars().count()
        );
    }

    #[test]
    fn inject_vocabulary_skips_empty_terms() {
        let p = "sys";
        let vocab = vec![
            "foo".to_string(),
            "".to_string(),
            "  ".to_string(),
            "bar".to_string(),
        ];
        let result = inject_vocabulary(p, &vocab);
        assert!(result.contains("<vocabulary>foo, bar</vocabulary>"));
    }
}
