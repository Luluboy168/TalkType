// Hotkey listener public types — `TriggerKey`, `TriggerMode`, `HotkeyConfig`.
//
// Serde rendering MUST match the matching TypeScript types in
// `src/types/settings.ts` (per architecture invariant #8). Specifically:
//
//   * `TriggerKey`  → `kebab-case` enum (`right-alt`, `left-control`, ...)
//   * `TriggerMode` → `kebab-case` enum (`hold` / `toggle`)
//   * `HotkeyConfig` → `camelCase` struct (`triggerKey`, `triggerMode`)
//
// Phase 1 ships preset-only — `Custom { keycode }` and `Combo { modifiers,
// keycode }` from `doc/plans/02-implementation-roadmap.md` M4 are deferred to
// Phase 2 (roadmap line 302 "不做 custom recording — preset only — 簡化").
//
// **Stable u8 discriminants**: the enums double as `AtomicU8` storage in
// `shared.rs::HotkeySharedState`. We use *explicit* `to_u8 / from_u8`
// mappings rather than `mem::transmute` on the discriminant so a future
// reordering of the variants in source order doesn't silently corrupt the
// atomic-stored state. Reviewers: any change to the byte values must also
// update the `discriminant_round_trip` tests below.

use serde::{Deserialize, Serialize};

/// Allowed preset trigger keys for the global hotkey listener (Phase 1).
///
/// Values match the Rust `#[serde(rename_all = "kebab-case")]` rendering and
/// the TypeScript `TriggerKey` union in `src/types/settings.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerKey {
    RightAlt,
    LeftAlt,
    RightControl,
    LeftControl,
    RightShift,
    LeftShift,
}

impl TriggerKey {
    /// Windows virtual key code (`VK_*`) used by `hook_proc` to detect
    /// down/up events and by AltGr suppression to inspect modifiers via
    /// `GetAsyncKeyState`.
    ///
    /// Constants documented at https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
    /// and pulled from `windows::Win32::UI::Input::KeyboardAndMouse::*`:
    ///
    ///   * `VK_RMENU`    = `0xA5` (right Alt)
    ///   * `VK_LMENU`    = `0xA4` (left  Alt)
    ///   * `VK_RCONTROL` = `0xA3`
    ///   * `VK_LCONTROL` = `0xA2`
    ///   * `VK_RSHIFT`   = `0xA1`
    ///   * `VK_LSHIFT`   = `0xA0`
    pub fn virtual_key(self) -> u32 {
        match self {
            TriggerKey::RightAlt => 0xA5,
            TriggerKey::LeftAlt => 0xA4,
            TriggerKey::RightControl => 0xA3,
            TriggerKey::LeftControl => 0xA2,
            TriggerKey::RightShift => 0xA1,
            TriggerKey::LeftShift => 0xA0,
        }
    }

    /// Stable byte representation for `AtomicU8` storage. Changing these
    /// values is a persistence-layer breaking change — values are never
    /// written to disk by Phase 1 (settings.rs in M4 chunk 3 owns
    /// persistence) but a runtime `update_hotkey_config` call still relies
    /// on the round trip.
    pub fn to_u8(self) -> u8 {
        match self {
            TriggerKey::RightAlt => 0,
            TriggerKey::LeftAlt => 1,
            TriggerKey::RightControl => 2,
            TriggerKey::LeftControl => 3,
            TriggerKey::RightShift => 4,
            TriggerKey::LeftShift => 5,
        }
    }

    /// Reverse of `to_u8`. Returns `None` for unknown bytes — the caller
    /// (`HotkeySharedState::current_trigger_key`) treats `None` as a corrupted
    /// state and falls back to `Default::default()` (`RightAlt`).
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(TriggerKey::RightAlt),
            1 => Some(TriggerKey::LeftAlt),
            2 => Some(TriggerKey::RightControl),
            3 => Some(TriggerKey::LeftControl),
            4 => Some(TriggerKey::RightShift),
            5 => Some(TriggerKey::LeftShift),
            _ => None,
        }
    }
}

/// How the hotkey toggles the recording state.
///
///   * `Hold`   — push-to-talk: key down starts recording, key up stops.
///   * `Toggle` — tap-to-talk: each press XORs `is_toggled_on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerMode {
    Hold,
    Toggle,
}

impl TriggerMode {
    pub fn to_u8(self) -> u8 {
        match self {
            TriggerMode::Hold => 0,
            TriggerMode::Toggle => 1,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(TriggerMode::Hold),
            1 => Some(TriggerMode::Toggle),
            _ => None,
        }
    }
}

/// Hotkey configuration as stored in `Settings.hotkey` (Rust-owned).
///
/// Defaults are set by `Default::default()`:
///   * `trigger_key:  RightAlt`
///   * `trigger_mode: Hold`
///
/// These match `src/types/settings.ts::HotkeyConfig` defaults documented in
/// the M4 chunk 3 settings module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConfig {
    pub trigger_key: TriggerKey,
    pub trigger_mode: TriggerMode,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            trigger_key: TriggerKey::RightAlt,
            trigger_mode: TriggerMode::Hold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_key_round_trip() {
        for k in [
            TriggerKey::RightAlt,
            TriggerKey::LeftAlt,
            TriggerKey::RightControl,
            TriggerKey::LeftControl,
            TriggerKey::RightShift,
            TriggerKey::LeftShift,
        ] {
            assert_eq!(TriggerKey::from_u8(k.to_u8()), Some(k));
        }
    }

    #[test]
    fn trigger_key_unknown_byte_returns_none() {
        assert_eq!(TriggerKey::from_u8(6), None);
        assert_eq!(TriggerKey::from_u8(255), None);
    }

    #[test]
    fn trigger_mode_round_trip() {
        for m in [TriggerMode::Hold, TriggerMode::Toggle] {
            assert_eq!(TriggerMode::from_u8(m.to_u8()), Some(m));
        }
    }

    #[test]
    fn trigger_mode_unknown_byte_returns_none() {
        assert_eq!(TriggerMode::from_u8(2), None);
        assert_eq!(TriggerMode::from_u8(255), None);
    }

    #[test]
    fn default_config_is_right_alt_hold() {
        let cfg = HotkeyConfig::default();
        assert_eq!(cfg.trigger_key, TriggerKey::RightAlt);
        assert_eq!(cfg.trigger_mode, TriggerMode::Hold);
    }

    #[test]
    fn trigger_key_serialize_kebab_case() {
        // Keep in sync with src/types/settings.ts TriggerKey union.
        assert_eq!(
            serde_json::to_string(&TriggerKey::RightAlt).unwrap(),
            "\"right-alt\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerKey::LeftControl).unwrap(),
            "\"left-control\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerKey::RightShift).unwrap(),
            "\"right-shift\""
        );
    }

    #[test]
    fn trigger_key_deserialize_kebab_case() {
        let k: TriggerKey = serde_json::from_str("\"right-alt\"").unwrap();
        assert_eq!(k, TriggerKey::RightAlt);
        let k: TriggerKey = serde_json::from_str("\"left-shift\"").unwrap();
        assert_eq!(k, TriggerKey::LeftShift);
    }

    #[test]
    fn trigger_mode_serialize_kebab_case() {
        assert_eq!(
            serde_json::to_string(&TriggerMode::Hold).unwrap(),
            "\"hold\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerMode::Toggle).unwrap(),
            "\"toggle\""
        );
    }

    #[test]
    fn hotkey_config_serialize_camel_case() {
        // Field names must be camelCase to match
        // src/types/settings.ts::HotkeyConfig.
        let cfg = HotkeyConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"triggerKey\""));
        assert!(json.contains("\"triggerMode\""));
        assert!(json.contains("\"right-alt\""));
        assert!(json.contains("\"hold\""));
    }

    #[test]
    fn hotkey_config_deserialize_camel_case() {
        let json = r#"{"triggerKey":"right-control","triggerMode":"toggle"}"#;
        let cfg: HotkeyConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.trigger_key, TriggerKey::RightControl);
        assert_eq!(cfg.trigger_mode, TriggerMode::Toggle);
    }

    #[test]
    fn virtual_key_codes_match_win32_constants() {
        assert_eq!(TriggerKey::RightAlt.virtual_key(), 0xA5);
        assert_eq!(TriggerKey::LeftAlt.virtual_key(), 0xA4);
        assert_eq!(TriggerKey::RightControl.virtual_key(), 0xA3);
        assert_eq!(TriggerKey::LeftControl.virtual_key(), 0xA2);
        assert_eq!(TriggerKey::RightShift.virtual_key(), 0xA1);
        assert_eq!(TriggerKey::LeftShift.virtual_key(), 0xA0);
    }
}
