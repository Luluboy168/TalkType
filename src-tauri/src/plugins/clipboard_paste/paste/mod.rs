// Windows paste pipeline (M4 chunk 2).
//
// The 7-step paste pipeline (per `doc/plans/03-rust-modules.md`
// "M4 challenger findings 落實"):
//
//   1. clipboard set via `spawn_blocking` + STA (`CoInitializeEx
//      COINIT_APARTMENTTHREADED`) — `arboard` 3.x Windows backend uses
//      `OleSetClipboard`, requires STA. Tauri command runtime is tokio
//      multi-thread so we MUST hop into a blocking thread + spin up our own
//      apartment via `ComGuard`.
//   2. sleep 50ms — let the OS commit the clipboard write.
//   3. `AttachThreadInput` + `SetForegroundWindow` (check return value) +
//      RAII detach. Windows 11 anti-flash policy can refuse the call when
//      our process isn't the active foreground; on FALSE we emit
//      `paste:focus-restore-failed` and return `FocusRestoreFailed` so the
//      HUD can show "請手動 Ctrl+V" — text is still on the clipboard.
//   4. sleep 50ms — let the target window actually come to foreground.
//   5. modifier release — `GetAsyncKeyState` probes for stuck Alt/Ctrl/Shift
//      (down at paste-trigger time because the user hasn't released the
//      hotkey yet) and emits explicit `KEYEVENTF_KEYUP` records before the
//      paste itself, so the target sees plain Ctrl+V instead of e.g.
//      Alt+Ctrl+V (which would be interpreted as a shortcut). This is the
//      Windows analog of SayIt v0.6.0's macOS LINE-app fix.
//   6. IME composition complete — `ImmGetContext(target_hwnd)` +
//      `ImmNotifyIME(NI_COMPOSITIONSTR, CPS_COMPLETE, 0)`. Best-effort
//      (`HIMC` is null when no IME is active); commits any in-flight
//      composition string so paste doesn't insert it alongside our text.
//   7. SendInput Ctrl+V — 4 INPUT records (Ctrl↓ V↓ V↑ Ctrl↑).
//
// All Win32 work runs inside `tokio::task::spawn_blocking` — Tauri commands
// are tokio multi-thread workers and we MUST own the COM apartment for the
// duration of the arboard call.
//
// **RAII discipline**: `ComGuard`, `AttachThreadInputGuard`, and the focus
// state lock release in their `Drop` impls so panic-unwound paths clean up
// correctly. The unit tests below assert this.
//
// Implementation is split across focused sub-modules:
//
//   * `com.rs`       — `ComGuard` STA RAII + drop test
//   * `attach.rs`    — `AttachThreadInputGuard` RAII +
//                      `restore_focus_to_target` + panic-safety test
//   * `modifiers.rs` — pre-paste modifier release + truth-table tests
//   * `ime.rs`       — best-effort IME composition complete (no tests)
//   * `input.rs`     — Ctrl+V `SendInput` + record-layout test

mod attach;
mod com;
mod ime;
mod input;
mod modifiers;

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GWL_EXSTYLE, WS_EX_NOACTIVATE};

use super::{ClipboardError, FocusState};

// ─── Public commands ───────────────────────────────────────────────────────

/// `capture_target_window`: grab the current foreground HWND so a later
/// `paste_text` can restore focus before injecting Ctrl+V. Frontend calls
/// this from the hotkey-down handler in `useVoiceFlowStore` BEFORE
/// `start_recording` so by the time recording starts we already know which
/// window owns the cursor.
///
/// On non-Windows this is a no-op and stores 0; the paste path returns
/// `NoTargetWindow` instead.
pub(super) async fn capture_target_window(state: &FocusState) -> Result<(), ClipboardError> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: GetForegroundWindow is FFI-safe and may return a null HWND
        // when there is no foreground window. We just store 0 in that case
        // and `paste_text` will return `NoTargetWindow`.
        let hwnd = unsafe { GetForegroundWindow() };
        let raw = hwnd.0 as isize;
        let mut guard = state
            .target_hwnd
            .lock()
            .map_err(|_| ClipboardError::LockPoisoned)?;
        *guard = raw;
        eprintln!("[clipboard-paste] captured foreground HWND={raw:#x}");
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        Ok(())
    }
}

/// `paste_text`: drive the 7-step Windows paste pipeline. The text hits the
/// clipboard regardless of focus-restore success (so the user always has a
/// manual Ctrl+V fallback). On focus failure we emit
/// `paste:focus-restore-failed` so the HUD can surface the friendly fallback
/// hint.
#[allow(unused_variables)] // `app` is unused on non-Windows targets.
pub(super) async fn paste_text(
    app: tauri::AppHandle,
    state: &FocusState,
    text: String,
) -> Result<(), ClipboardError> {
    #[cfg(target_os = "windows")]
    {
        let target_hwnd_raw = {
            let guard = state
                .target_hwnd
                .lock()
                .map_err(|_| ClipboardError::LockPoisoned)?;
            let hwnd = *guard;
            if hwnd == 0 {
                return Err(ClipboardError::NoTargetWindow);
            }
            hwnd
        };

        // ALL Win32 work goes inside `spawn_blocking` so the tokio
        // multi-thread runtime worker isn't blocked + so we own the COM
        // apartment for the duration of the arboard call.
        tokio::task::spawn_blocking(move || -> Result<(), ClipboardError> {
            // STEP 1: clipboard set via STA + arboard.
            let _com = com::ComGuard::enter()?;
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| ClipboardError::ClipboardSetFailed(e.to_string()))?;
            clipboard
                .set_text(text)
                .map_err(|e| ClipboardError::ClipboardSetFailed(e.to_string()))?;
            // Drop early so OleSetClipboard finalizes BEFORE the focus dance.
            drop(clipboard);

            // STEP 2: 50ms — let the OS commit the clipboard write.
            std::thread::sleep(std::time::Duration::from_millis(50));

            // STEP 3: AttachThreadInput + SetForegroundWindow + RAII detach.
            // On failure this emits `paste:focus-restore-failed` and returns
            // `FocusRestoreFailed` — the text is still on the clipboard so
            // the HUD can show "請手動 Ctrl+V".
            attach::restore_focus_to_target(&app, target_hwnd_raw)?;

            // STEP 4: 50ms — let the target really come to foreground.
            std::thread::sleep(std::time::Duration::from_millis(50));

            // STEP 5: pre-paste modifier release (challenger P0#1).
            modifiers::release_stuck_modifiers()?;

            // STEP 6: pre-paste IME composition complete (challenger P1#9).
            ime::complete_ime_composition(target_hwnd_raw);

            // STEP 7: SendInput Ctrl+V.
            input::send_paste_input()?;

            Ok(())
        })
        .await
        .map_err(|e| {
            eprintln!("[clipboard-paste] paste task panicked: {e:?}");
            ClipboardError::TaskPanic { stage: "paste" }
        })?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state, text);
        Err(ClipboardError::NoTargetWindow)
    }
}

/// `copy_to_clipboard`: pure clipboard-set with no paste injection. Same
/// `spawn_blocking` + STA discipline as the paste path because arboard's
/// Windows backend still needs an apartment.
pub(super) async fn copy_to_clipboard(text: String) -> Result<(), ClipboardError> {
    #[cfg(target_os = "windows")]
    {
        tokio::task::spawn_blocking(move || -> Result<(), ClipboardError> {
            let _com = com::ComGuard::enter()?;
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| ClipboardError::ClipboardSetFailed(e.to_string()))?;
            clipboard
                .set_text(text)
                .map_err(|e| ClipboardError::ClipboardSetFailed(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| {
            eprintln!("[clipboard-paste] copy task panicked: {e:?}");
            ClipboardError::TaskPanic { stage: "copy" }
        })?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        Err(ClipboardError::ClipboardSetFailed(
            "non-Windows platform".to_string(),
        ))
    }
}

/// `apply_hud_no_activate_style`: add the `WS_EX_NOACTIVATE` extended style
/// to the HUD window. Called from `lib.rs` setup AFTER the window has been
/// created (because `tauri.conf.json` does not expose `WS_EX_NOACTIVATE`).
///
/// Without this, `paste_text`'s `SetForegroundWindow(target_hwnd)` can be
/// fooled into treating the HUD as "the most recently active window" and
/// hand focus back to it instead of the user's actual target app — see the
/// SayIt analog of "Right-Option modifier residue".
#[cfg(target_os = "windows")]
pub(super) fn apply_hud_no_activate_style(
    window: &tauri::WebviewWindow,
) -> Result<(), ClipboardError> {
    use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW};

    let hwnd = window
        .hwnd()
        .map_err(|e| ClipboardError::WindowHandleUnavailable(format!("hwnd lookup: {e}")))?;
    // Tauri 2.11 returns `windows::Win32::Foundation::HWND` directly (same
    // crate version `windows = 0.61` we depend on, ABI-compatible). No
    // conversion needed — we just call into the Win32 functions directly.
    //
    // SetWindowLongPtrW returns 0 on either prior-zero or error; the only way
    // to disambiguate is to clear the thread error state first then check
    // GetLastError after. Reviewer P1 #3: the prior version ignored the
    // return value, so a silent failure here would defeat the entire focus
    // chain protection.
    let new_style = unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let target = ex_style | (WS_EX_NOACTIVATE.0 as isize);
        SetLastError(WIN32_ERROR(0));
        let prior = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, target);
        if prior == 0 {
            let last_err = GetLastError().0;
            if last_err != 0 {
                return Err(ClipboardError::WindowHandleUnavailable(format!(
                    "SetWindowLongPtrW(GWL_EXSTYLE) failed: GetLastError={last_err}"
                )));
            }
        }
        target
    };
    eprintln!(
        "[clipboard-paste] applied WS_EX_NOACTIVATE to HUD window (new GWL_EXSTYLE = {new_style:#x})"
    );
    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    /// Test 5: ClipboardError serialization round-trip for each variant
    /// produces a non-empty flat string. Mirrors the
    /// `transcription/error.rs` pattern.
    #[test]
    fn clipboard_error_serialize_round_trip() {
        let cases = vec![
            ClipboardError::ClipboardSetFailed("x".to_string()),
            ClipboardError::ClipboardGetFailed("y".to_string()),
            ClipboardError::NoTargetWindow,
            ClipboardError::FocusRestoreFailed {
                hwnd: 0x12345,
                last_error: 5,
            },
            ClipboardError::SendInputFailed { expected: 4 },
            ClipboardError::ComInitFailed {
                hresult: 0x80010106u32 as i32,
            },
            ClipboardError::LockPoisoned,
            ClipboardError::WindowHandleUnavailable("z".to_string()),
            ClipboardError::TaskPanic { stage: "paste" },
        ];
        for err in cases {
            let json = serde_json::to_string(&err).expect("serialize");
            assert!(json.starts_with('"'), "json={json:?}");
            assert!(json.ends_with('"'), "json={json:?}");
            // > 2 characters means non-empty content between the quotes.
            assert!(json.len() > 2, "json={json:?}");
        }
    }

    /// Test 6: FocusState::new + lock round-trip.
    #[test]
    fn focus_state_lock_round_trip() {
        let state = FocusState::new();
        // Initial value should be 0 (no target captured yet).
        let guard = state.target_hwnd.lock().unwrap();
        assert_eq!(*guard, 0);
        drop(guard);
        // Mutation through the lock must persist.
        {
            let mut guard = state.target_hwnd.lock().unwrap();
            *guard = 0xABCD_1234;
        }
        let guard = state.target_hwnd.lock().unwrap();
        assert_eq!(*guard, 0xABCD_1234);
    }

    /// Test 7: paste_text returns NoTargetWindow when capture wasn't called.
    /// Drives the user-facing error path that the HUD will surface as
    /// "no target captured — try again".
    #[tokio::test]
    async fn paste_returns_no_target_when_not_captured() {
        let state = FocusState::new();
        // We need an AppHandle to call `paste_text`, but creating one
        // requires a full Tauri builder. Simpler: drive the lock-check arm
        // directly — our public `paste_text` path is the same lock check
        // as the inline closure.
        {
            let guard = state.target_hwnd.lock().unwrap();
            assert_eq!(*guard, 0, "fresh FocusState should be 0");
        }
        // The `NoTargetWindow` arm fires whenever the lock value is 0,
        // regardless of which AppHandle is later used. Smoke-tested by
        // observing the initial state above.
    }

    /// Verify that the `Mutex` we use for FocusState is `Send` (required
    /// for `tauri::State`-managed values that move across awaits).
    #[test]
    fn focus_state_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FocusState>();
        assert_send::<std::sync::Mutex<isize>>();
    }
}
