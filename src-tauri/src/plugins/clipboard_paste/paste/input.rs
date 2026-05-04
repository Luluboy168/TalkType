// Ctrl+V SendInput injection for the Windows paste pipeline.
//
// The final step of the 7-step pipeline: after the clipboard is set, focus
// is restored, modifiers are released, and IME composition is completed,
// inject `Ctrl↓ V↓ V↑ Ctrl↑` via `SendInput`. Pulled into its own module so
// the record-layout truth table (`build_paste_input_records`) can be unit
// tested without invoking real Win32.

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};

use super::super::ClipboardError;

/// Build the 4 SendInput records for Ctrl+V (Ctrl↓ V↓ V↑ Ctrl↑).
///
/// Pulled out as `pub(super)` so the unit tests can assert exact field
/// values without invoking SendInput.
#[cfg(target_os = "windows")]
pub(super) fn build_paste_input_records() -> [INPUT; 4] {
    [
        keybd_input(VK_CONTROL, false),
        keybd_input(VK_V, false),
        keybd_input(VK_V, true),
        keybd_input(VK_CONTROL, true),
    ]
}

/// Send the prebuilt paste input records via SendInput.
///
/// Returns `SendInputFailed` if SendInput reports fewer than 4 events
/// successfully injected (typically a UIPI / locked-workstation block).
#[cfg(target_os = "windows")]
pub(super) fn send_paste_input() -> Result<(), ClipboardError> {
    let inputs = build_paste_input_records();
    // SAFETY: SendInput is FFI-safe. The cbsize argument is the size of a
    // single INPUT record (Win32 quirk, not the array length).
    let n = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if n != inputs.len() as u32 {
        return Err(ClipboardError::SendInputFailed {
            expected: inputs.len() as u32,
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

    /// Test 1: build_paste_input_records returns 4 records with the right
    /// VKs and KEYEVENTF_KEYUP flags in the right order.
    #[test]
    fn paste_input_records_have_correct_layout() {
        let inputs = build_paste_input_records();
        assert_eq!(inputs.len(), 4);

        // SAFETY: we know we just constructed these as INPUT_KEYBOARD union
        // members so reading `.ki` is sound.
        unsafe {
            // [0] Ctrl down
            assert_eq!(inputs[0].r#type, INPUT_KEYBOARD);
            assert_eq!(inputs[0].Anonymous.ki.wVk, VK_CONTROL);
            assert_eq!(inputs[0].Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);

            // [1] V down
            assert_eq!(inputs[1].r#type, INPUT_KEYBOARD);
            assert_eq!(inputs[1].Anonymous.ki.wVk, VK_V);
            assert_eq!(inputs[1].Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);

            // [2] V up
            assert_eq!(inputs[2].r#type, INPUT_KEYBOARD);
            assert_eq!(inputs[2].Anonymous.ki.wVk, VK_V);
            assert_ne!(inputs[2].Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);

            // [3] Ctrl up
            assert_eq!(inputs[3].r#type, INPUT_KEYBOARD);
            assert_eq!(inputs[3].Anonymous.ki.wVk, VK_CONTROL);
            assert_ne!(inputs[3].Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);
        }
    }
}
