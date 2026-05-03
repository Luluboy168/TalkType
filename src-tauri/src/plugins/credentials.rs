// Credentials plugin (M3 chunk 1).
//
// Stores per-provider API keys in the OS credential vault via the `keyring`
// crate. Phase 1 ships Windows-native (Credential Manager); Cargo.toml
// pre-wires macOS (Keychain) and Linux (secret-service) backends so Phase 2
// cross-platform polish doesn't need to revisit this module.
//
// Public surface:
//
//   State          CredentialsState (zero-sized; kept for future expansion)
//   Commands       set_credential / delete_credential / has_credential
//   Internal API   get_credential (pub(crate) — never exposed to frontend)
//
// **Invariant (architecture rule #1)**: the API key NEVER crosses the IPC
// boundary in either direction. Frontend can only check existence
// (`has_credential`), set (`set_credential`), or delete (`delete_credential`).
// Reads stay inside Rust — `transcribe_cloud_internal` (M3 chunk 2) and
// `polish_text_internal` (M6) call `get_credential` directly via the
// `pub(crate)` import from `plugins`.
//
// Service / username convention (per `doc/plans/05-data-model.md`):
//
//   * Service:  `com.luluboy168.talktype` (matches Tauri bundle identifier)
//   * Username: provider id — `groq` | `openai` | `anthropic` | `gemini`
//
// On Windows this surfaces in Credential Manager as
// `com.luluboy168.talktype/groq` per provider entry.
//
// **Validation layers**:
//
//   1. `validate_provider`: provider id must be in the hard-coded allowlist.
//      Defends against frontend code accidentally passing arbitrary strings
//      to the keyring backend (the keyring crate sanitizes but defense in
//      depth is cheap).
//   2. `validate_and_clean_key`: trims whitespace, rejects empty or
//      pathologically-long inputs, and runs a soft prefix check so the user
//      gets immediate feedback instead of a surprise 401 on first
//      transcription.
//
// **Error contract**: thiserror enum with manual `Serialize` to a flat
// string, mirroring `AudioRecorderError`. The frontend receives a `string`
// from each `invoke<T>()` failure path, never a tagged-enum object.

use keyring::Entry;
use serde::{Serialize, Serializer};
use tauri::State;
use thiserror::Error;

/// Service name passed to the OS credential vault. Matches the Tauri bundle
/// identifier so Credential Manager shows entries grouped under the app.
const SERVICE_NAME: &str = "com.luluboy168.talktype";

/// Allowlist of provider ids accepted by the credentials API. Hard-coded so
/// the frontend cannot sneak arbitrary names into the keyring backend.
/// Must stay in sync with `LlmProviderId` in `src/types/credentials.ts`
/// and `LLM_PROVIDERS` in `src/lib/providers.ts`.
const ALLOWED_PROVIDERS: &[&str] = &["groq", "openai", "anthropic", "gemini"];

/// Maximum API key length we accept. No production API key from any of the
/// four providers above is anywhere near this cap; this exists purely to
/// reject obvious paste-by-mistake cases (e.g. user pastes a markdown block
/// or 32 KB of CSV into the input).
const MAX_KEY_LEN: usize = 1024;

/// Errors returned by the credentials Tauri commands.
///
/// Variants intentionally split user-facing input issues
/// (`UnknownProvider` / `EmptyKey` / `KeyTooLong` / `BadPrefix`) from
/// backend issues (`Keyring`) so the UI can localize messages without
/// string-matching on `Display`.
#[derive(Error, Debug)]
pub enum CredentialsError {
    /// Provider id was not in the `ALLOWED_PROVIDERS` allowlist.
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    /// User submitted an empty (or whitespace-only) key.
    #[error("Empty API key")]
    EmptyKey,

    /// User submitted a key longer than `MAX_KEY_LEN` chars (likely a paste
    /// error). The actual length is included so the UI can show an absolute
    /// number rather than a vague warning.
    #[error("API key is suspiciously long ({0} chars; likely a paste error)")]
    KeyTooLong(usize),

    /// Soft prefix check failed — the key doesn't start with the expected
    /// provider prefix (e.g. `gsk_` for Groq). `got` echoes the first eight
    /// characters of the user's input so they can spot wrong-vendor pastes
    /// without the UI ever showing the rest of the key.
    #[error("API key prefix does not match provider expectation: got {got:?}, expected starts_with {expected:?}")]
    BadPrefix { got: String, expected: String },

    /// Backend (OS credential vault) error wrapped as a string. We do not
    /// preserve the inner `keyring::Error` type so this enum stays
    /// `Serialize` without adding the keyring crate to the IPC surface.
    #[error("keyring error: {0}")]
    Keyring(String),
}

// Manual `Serialize` so the frontend receives a flat string, matching the
// pattern in `audio_recorder::AudioRecorderError`. CLAUDE.md "Error enum
// 手動 implement Serialize 為 string".
impl Serialize for CredentialsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Provider-specific API key prefix expectations. Used by
/// `validate_and_clean_key` for a soft pre-flight check. The prefixes are
/// well-known but may evolve — empty string disables the check for that
/// provider (used for Gemini, whose keys do not follow a single prefix
/// convention).
fn expected_prefix(provider: &str) -> &'static str {
    match provider {
        "groq" => "gsk_",
        "openai" => "sk-",
        "anthropic" => "sk-ant-",
        // Gemini keys have multiple shapes (`AIza...`, `base64`-ish blobs,
        // etc.) so we don't enforce a prefix — the test-connection step in
        // M6 will catch invalid keys for this provider.
        "gemini" => "",
        _ => "",
    }
}

/// Tauri-managed state slot for credentials. Currently empty — the keyring
/// crate manages its own thread safety and there is nothing to cache.
/// Kept as a unit struct so future expansion (e.g. an in-memory cache for
/// the duration of a transcribe call) can be added without changing the
/// `lib.rs` `manage()` call or the command signatures.
pub struct CredentialsState;

impl CredentialsState {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CredentialsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates that `provider` is one of the four supported provider ids.
/// Pure function (no side effects) so unit-testable without the OS keyring.
fn validate_provider(provider: &str) -> Result<(), CredentialsError> {
    if ALLOWED_PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(CredentialsError::UnknownProvider(provider.to_string()))
    }
}

/// Trims and shape-validates the API key. Returns the cleaned key (with
/// surrounding whitespace removed) on success.
///
/// Validation order (fail-fast):
///
///   1. Trim whitespace (incl. trailing newlines from clipboard pastes).
///   2. Reject empty result — `EmptyKey`.
///   3. Reject overlong result — `KeyTooLong(actual_len)`.
///   4. Soft prefix check — `BadPrefix { got, expected }` if expected is
///      non-empty and the cleaned key doesn't start with it.
fn validate_and_clean_key(key: &str, provider: &str) -> Result<String, CredentialsError> {
    let cleaned = key.trim().to_string();
    if cleaned.is_empty() {
        return Err(CredentialsError::EmptyKey);
    }
    if cleaned.len() > MAX_KEY_LEN {
        return Err(CredentialsError::KeyTooLong(cleaned.len()));
    }
    let expected = expected_prefix(provider);
    if !expected.is_empty() && !cleaned.starts_with(expected) {
        return Err(CredentialsError::BadPrefix {
            got: cleaned.chars().take(8).collect::<String>(),
            expected: expected.to_string(),
        });
    }
    Ok(cleaned)
}

/// Stores the API key for `provider` in the OS credential vault.
///
/// Validates `provider` against the allowlist and shape-checks the key
/// before any keyring round-trip — this avoids creating an empty Credential
/// Manager entry on validation failure.
#[tauri::command]
pub async fn set_credential(
    _state: State<'_, CredentialsState>,
    provider: String,
    key: String,
) -> Result<(), CredentialsError> {
    validate_provider(&provider)?;
    let cleaned = validate_and_clean_key(&key, &provider)?;
    let entry = Entry::new(SERVICE_NAME, &provider)
        .map_err(|e| CredentialsError::Keyring(e.to_string()))?;
    entry
        .set_password(&cleaned)
        .map_err(|e| CredentialsError::Keyring(e.to_string()))?;
    Ok(())
}

/// Deletes the stored API key for `provider`.
///
/// Idempotent: deleting a non-existent entry returns `Ok(())` rather than
/// an error so the UI's "Delete" button can be wired without checking
/// existence first.
#[tauri::command]
pub async fn delete_credential(
    _state: State<'_, CredentialsState>,
    provider: String,
) -> Result<(), CredentialsError> {
    validate_provider(&provider)?;
    let entry = Entry::new(SERVICE_NAME, &provider)
        .map_err(|e| CredentialsError::Keyring(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // NoEntry: nothing to delete is success from the caller's POV.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CredentialsError::Keyring(e.to_string())),
    }
}

/// Returns whether an API key is stored for `provider`. NEVER returns the
/// key value — the frontend can only know existence, not contents.
#[tauri::command]
pub async fn has_credential(
    _state: State<'_, CredentialsState>,
    provider: String,
) -> Result<bool, CredentialsError> {
    validate_provider(&provider)?;
    let entry = Entry::new(SERVICE_NAME, &provider)
        .map_err(|e| CredentialsError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(CredentialsError::Keyring(e.to_string())),
    }
}

/// Internal: Rust-only access to the API key. Used by transcription_cloud
/// (M3 chunk 2) and llm_polish (M6).
///
/// **NEVER expose this via a `#[tauri::command]`**. Architecture invariant
/// #1 (`doc/plans/01-architecture.md`): "API key 從不在前端".
///
/// Returns:
///
///   * `Ok(Some(key))` — provider has a stored key.
///   * `Ok(None)`      — provider exists in the allowlist but no key set.
///   * `Err(...)`      — invalid provider id or keyring backend error.
#[allow(dead_code)]
pub(crate) fn get_credential(provider: &str) -> Result<Option<String>, CredentialsError> {
    validate_provider(provider)?;
    let entry = Entry::new(SERVICE_NAME, provider)
        .map_err(|e| CredentialsError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CredentialsError::Keyring(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure-logic tests (no keyring backend). The validate_provider /
    // validate_and_clean_key functions cover all the user-facing error
    // paths; round-trip set/get/delete tests against the OS keyring are
    // skipped for now because:
    //
    //   * GitHub Actions Linux runners don't have dbus + gnome-keyring set
    //     up by default, and Windows runners would touch the real
    //     Credential Manager (test pollution).
    //   * Chunk 3's manual smoke covers the round-trip end-to-end via the
    //     "Save → has_credential → Delete" flow in the UI.
    //
    // If we want automated keyring coverage later, the `keyring` crate
    // ships a mock backend (`mock` feature) we can wire up under cfg(test).

    #[test]
    fn validate_provider_accepts_allowed() {
        for provider in ["groq", "openai", "anthropic", "gemini"] {
            assert!(
                validate_provider(provider).is_ok(),
                "provider {provider:?} should be allowed"
            );
        }
    }

    #[test]
    fn validate_provider_rejects_unknown() {
        for provider in ["", "google", "../groq", "groq.exe", "GROQ", "groq "] {
            assert!(
                matches!(
                    validate_provider(provider),
                    Err(CredentialsError::UnknownProvider(_))
                ),
                "provider {provider:?} should be rejected"
            );
        }
    }

    #[test]
    fn validate_and_clean_key_trims_whitespace() {
        let cleaned = validate_and_clean_key("  gsk_abc123  \n", "groq").expect("trim");
        assert_eq!(cleaned, "gsk_abc123");
    }

    #[test]
    fn validate_and_clean_key_trims_carriage_returns() {
        // Windows clipboard often appends \r\n on copy-paste.
        let cleaned = validate_and_clean_key("gsk_abc\r\n", "groq").expect("trim");
        assert_eq!(cleaned, "gsk_abc");
    }

    #[test]
    fn validate_and_clean_key_rejects_empty() {
        assert!(matches!(
            validate_and_clean_key("", "groq"),
            Err(CredentialsError::EmptyKey)
        ));
        assert!(matches!(
            validate_and_clean_key("   \n\t", "groq"),
            Err(CredentialsError::EmptyKey)
        ));
    }

    #[test]
    fn validate_and_clean_key_rejects_too_long() {
        let long = "x".repeat(MAX_KEY_LEN + 1);
        assert!(matches!(
            validate_and_clean_key(&long, "groq"),
            Err(CredentialsError::KeyTooLong(n)) if n == MAX_KEY_LEN + 1
        ));
    }

    #[test]
    fn validate_and_clean_key_accepts_exact_max_len() {
        // MAX_KEY_LEN should be inclusive — exact length OK, +1 rejected.
        // `gsk_` is 4 chars, so we pad to MAX_KEY_LEN total.
        let key = format!("gsk_{}", "x".repeat(MAX_KEY_LEN - 4));
        assert_eq!(key.len(), MAX_KEY_LEN);
        assert!(validate_and_clean_key(&key, "groq").is_ok());
    }

    #[test]
    fn validate_and_clean_key_rejects_wrong_prefix_for_groq() {
        let err = validate_and_clean_key("sk-this-is-openai-not-groq", "groq").unwrap_err();
        match err {
            CredentialsError::BadPrefix { got, expected } => {
                assert_eq!(expected, "gsk_");
                assert_eq!(got, "sk-this-"); // first 8 chars
            }
            other => panic!("expected BadPrefix, got {other:?}"),
        }
    }

    #[test]
    fn validate_and_clean_key_accepts_anthropic_prefix() {
        assert!(validate_and_clean_key("sk-ant-api03-xxx", "anthropic").is_ok());
        // Plain `sk-` (OpenAI shape) into anthropic slot must reject.
        assert!(matches!(
            validate_and_clean_key("sk-not-ant", "anthropic"),
            Err(CredentialsError::BadPrefix { .. })
        ));
    }

    #[test]
    fn validate_and_clean_key_skips_prefix_check_for_gemini() {
        // Gemini has no enforced single prefix (some keys are AIza-prefixed,
        // others are different shapes), so any non-empty cleaned key is
        // accepted by shape validation. Authenticity is verified by the
        // M6 test-connection roundtrip.
        assert!(validate_and_clean_key("AIzaSyAbcXyz123", "gemini").is_ok());
        assert!(validate_and_clean_key("any-other-shape", "gemini").is_ok());
    }

    #[test]
    fn validate_and_clean_key_short_key_bad_prefix_truncates_got_to_actual_length() {
        // Bug-bait: the `got` field uses .chars().take(8) — for keys shorter
        // than 8 chars we should still get a clean truncation rather than
        // panicking on the slice boundary.
        let err = validate_and_clean_key("xx", "groq").unwrap_err();
        match err {
            CredentialsError::BadPrefix { got, .. } => assert_eq!(got, "xx"),
            other => panic!("expected BadPrefix, got {other:?}"),
        }
    }

    #[test]
    fn credentials_error_serializes_as_flat_string() {
        let err = CredentialsError::UnknownProvider("foo".to_string());
        let json = serde_json::to_string(&err).expect("serialize");
        assert_eq!(json, "\"Unknown provider: foo\"");

        let err = CredentialsError::BadPrefix {
            got: "abc".to_string(),
            expected: "gsk_".to_string(),
        };
        let json = serde_json::to_string(&err).expect("serialize");
        // Prefix-check error contains the both fields in Debug-ish form;
        // primary check is just that it's a flat JSON string, not an object.
        assert!(json.starts_with('"') && json.ends_with('"'));
        assert!(json.contains("got"));
        assert!(json.contains("expected"));
    }
}
