// Pre-paste modifier release for the Windows paste pipeline.
//
// At paste-trigger time the user hasn't released the hotkey yet, so any
// hotkey-modifier (e.g. Right-Alt for AltGr, or the user's custom hotkey
// modifier) is still physically down. If we just `SendInput(Ctrl, V)` the
// target sees `Mod+Ctrl+V` and may interpret it as a different shortcut
// (e.g. LINE app on Windows treats Alt+Ctrl+V as "send sticker", not
// "paste").
//
// This module probes each of the six modifier VKs with `GetAsyncKeyState`
// and emits explicit `KEYEVENTF_KEYUP` records for whichever ones are
// down — Windows analog of SayIt v0.6.0's macOS LINE-app fix.

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
};

use super::super::ClipboardError;

/// Probe each modifier VK with `GetAsyncKeyState` and build a KEYUP record
/// for whichever ones are still down. Pulled out as `pub(super)` so the
/// truth table can be unit-tested without touching the OS.
#[cfg(target_os = "windows")]
pub(super) fn build_modifier_release_records(state: &[bool; 6]) -> Vec<INPUT> {
    // Index order matches the `release_stuck_modifiers` probe order:
    // [VK_LMENU, VK_RMENU, VK_LCONTROL, VK_RCONTROL, VK_LSHIFT, VK_RSHIFT].
    const VKS: [VIRTUAL_KEY; 6] = [
        VK_LMENU,
        VK_RMENU,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LSHIFT,
        VK_RSHIFT,
    ];
    state
        .iter()
        .zip(VKS.iter())
        .filter(|(down, _)| **down)
        .map(|(_, vk)| keybd_input(*vk, true))
        .collect()
}

/// Probe each modifier with GetAsyncKeyState and KEYUP-release whichever are
/// still down. Failure of the SendInput call is surfaced as
/// `SendInputFailed` since a half-applied release would leave the user in a
/// stuck-modifier state.
#[cfg(target_os = "windows")]
pub(super) fn release_stuck_modifiers() -> Result<(), ClipboardError> {
    // SAFETY: GetAsyncKeyState is FFI-safe. We only inspect the high bit
    // ("currently pressed") per MSDN.
    let probe = |vk: VIRTUAL_KEY| -> bool {
        let s = unsafe { GetAsyncKeyState(vk.0 as i32) };
        // High bit indicates "key is currently down".
        (s as u16 & 0x8000) != 0
    };
    let state = [
        probe(VK_LMENU),
        probe(VK_RMENU),
        probe(VK_LCONTROL),
        probe(VK_RCONTROL),
        probe(VK_LSHIFT),
        probe(VK_RSHIFT),
    ];
    let records = build_modifier_release_records(&state);
    if records.is_empty() {
        return Ok(());
    }
    let n = unsafe { SendInput(&records, std::mem::size_of::<INPUT>() as i32) };
    if n != records.len() as u32 {
        return Err(ClipboardError::SendInputFailed {
            expected: records.len() as u32,
        });
    }
    Ok(())
}

/// Build a `INPUT` record for a virtual-key press or release.
#[cfg(target_os = "windows")]
fn keybd_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    /// Test 2a: build_modifier_release_records returns 0 records when no
    /// modifiers are down (the all-false slot of the truth table).
    #[test]
    fn modifier_release_empty_when_nothing_down() {
        let records = build_modifier_release_records(&[false; 6]);
        assert!(records.is_empty());
    }

    /// Test 2b: build_modifier_release_records returns 6 records when every
    /// modifier is down (the all-true slot of the truth table).
    #[test]
    fn modifier_release_six_when_all_down() {
        let records = build_modifier_release_records(&[true; 6]);
        assert_eq!(records.len(), 6);
        let expected_vks = [
            VK_LMENU,
            VK_RMENU,
            VK_LCONTROL,
            VK_RCONTROL,
            VK_LSHIFT,
            VK_RSHIFT,
        ];
        for (rec, vk) in records.iter().zip(expected_vks.iter()) {
            // SAFETY: we just built these as INPUT_KEYBOARD union members.
            unsafe {
                assert_eq!(rec.r#type, INPUT_KEYBOARD);
                assert_eq!(rec.Anonymous.ki.wVk, *vk);
                // All releases must have KEYEVENTF_KEYUP set.
                assert_ne!(rec.Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);
            }
        }
    }

    /// Test 2c: per-slot release — only LMENU + LCONTROL down (the AltGr
    /// pattern from the M4 challenger note) → 2 records in that order.
    #[test]
    fn modifier_release_subset_preserves_order() {
        // [LMENU=true, RMENU=false, LCONTROL=true, others false]
        let state = [true, false, true, false, false, false];
        let records = build_modifier_release_records(&state);
        assert_eq!(records.len(), 2);
        unsafe {
            assert_eq!(records[0].Anonymous.ki.wVk, VK_LMENU);
            assert_eq!(records[1].Anonymous.ki.wVk, VK_LCONTROL);
        }
    }
}
