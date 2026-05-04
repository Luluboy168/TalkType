// Thread-input attachment + foreground-window restoration for the paste
// pipeline.
//
// Windows protects against unwanted focus stealing: an arbitrary process
// calling `SetForegroundWindow` is normally only honored when that process
// already owns the foreground or has special permission. To work around
// this for a legitimate paste, we attach our input queue to the target
// thread's queue (which gives us "the same focus rights as the target") and
// then call `SetForegroundWindow`.
//
// `AttachThreadInputGuard` makes the attach + detach automatic via RAII so
// the matching `AttachThreadInput(_, _, FALSE)` runs even if the paste
// closure panics — leaking an attachment in production hangs user input.
//
// `restore_focus_to_target` orchestrates the AttachThreadInput +
// SetForegroundWindow sequence and emits `paste:focus-restore-failed` on
// the failure paths so the HUD can surface the manual-paste fallback.

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{GetLastError, HWND},
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::WindowsAndMessaging::{GetWindowThreadProcessId, SetForegroundWindow},
};

use super::super::ClipboardError;

/// Attach our input queue to the foreground-window's thread, raise the
/// target to foreground, then detach. Windows 11's anti-flash policy can
/// reject the elevation; on FALSE we emit `paste:focus-restore-failed` and
/// return `FocusRestoreFailed`.
#[cfg(target_os = "windows")]
pub(super) fn restore_focus_to_target(
    app: &tauri::AppHandle,
    target_raw: isize,
) -> Result<(), ClipboardError> {
    use tauri::Emitter;

    let target = HWND(target_raw as *mut core::ffi::c_void);

    // SAFETY: GetCurrentThreadId / GetWindowThreadProcessId are FFI-safe.
    let our_tid = unsafe { GetCurrentThreadId() };
    let target_tid = unsafe { GetWindowThreadProcessId(target, None) };
    if target_tid == 0 {
        // Stale HWND (target window closed). Surface as focus-restore
        // failure so the HUD shows the manual-paste hint.
        let last_error = unsafe { GetLastError().0 };
        let _ = app.emit(
            "paste:focus-restore-failed",
            serde_json::json!({
                "hwnd": target_raw,
                "lastErrorCode": last_error,
                "message": format!("GetWindowThreadProcessId returned 0 (target window may have closed); last_error={last_error}"),
            }),
        );
        return Err(ClipboardError::FocusRestoreFailed {
            hwnd: target_raw,
            last_error,
        });
    }

    // The AttachThreadInput RAII guard ensures we detach even on panic.
    let _attach_guard = if our_tid != target_tid {
        Some(AttachThreadInputGuard::attach(our_tid, target_tid))
    } else {
        None
    };

    // SAFETY: SetForegroundWindow is FFI-safe; non-zero return means the
    // foreground was set. Windows 11 anti-flash policy can refuse the call.
    let result = unsafe { SetForegroundWindow(target) };
    if !result.as_bool() {
        let last_error = unsafe { GetLastError().0 };
        let _ = app.emit(
            "paste:focus-restore-failed",
            serde_json::json!({
                "hwnd": target_raw,
                "lastErrorCode": last_error,
                "message": format!("SetForegroundWindow returned FALSE; last_error={last_error}"),
            }),
        );
        return Err(ClipboardError::FocusRestoreFailed {
            hwnd: target_raw,
            last_error,
        });
    }

    Ok(())
    // _attach_guard drops here, calling AttachThreadInput(_, _, FALSE).
}

/// `AttachThreadInputGuard`: scope-bound `AttachThreadInput(from, to, TRUE)`
/// so we can `SetForegroundWindow` even when the target thread owns the
/// foreground. `Drop` calls the matching `AttachThreadInput(_, _, FALSE)` —
/// crucial that this runs on panic-unwound paths because a leaked
/// attachment can hang input focus on the user.
#[cfg(target_os = "windows")]
pub(super) struct AttachThreadInputGuard {
    from: u32,
    to: u32,
    /// Whether the attach succeeded — we only detach if it did.
    attached: bool,
    #[cfg(test)]
    counter: Option<&'static std::sync::atomic::AtomicUsize>,
}

#[cfg(target_os = "windows")]
impl AttachThreadInputGuard {
    pub(super) fn attach(from: u32, to: u32) -> Self {
        // SAFETY: AttachThreadInput is FFI-safe; failure leaves no resource
        // to clean up so we just track the result.
        let ok = unsafe { AttachThreadInput(from, to, true) };
        Self {
            from,
            to,
            attached: ok.as_bool(),
            #[cfg(test)]
            counter: None,
        }
    }

    /// Test-only: build a fake guard that increments `counter` on drop.
    /// We use this in `panic_safety_drop_runs` to verify Drop runs even
    /// after `panic::catch_unwind` unwinds through the guard.
    #[cfg(test)]
    pub(super) fn fake_for_test(counter: &'static std::sync::atomic::AtomicUsize) -> Self {
        Self {
            from: 0,
            to: 0,
            attached: false,
            counter: Some(counter),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for AttachThreadInputGuard {
    fn drop(&mut self) {
        if self.attached {
            // SAFETY: balances the attach in `attach`.
            unsafe {
                let _ = AttachThreadInput(self.from, self.to, false);
            }
        }
        #[cfg(test)]
        if let Some(c) = self.counter {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(super) struct AttachThreadInputGuard;

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test 4: AttachThreadInputGuard::Drop runs even when the guarded
    /// scope panics. This is the key panic-safety property — leaking an
    /// AttachThreadInput attachment in production would hang user input.
    #[test]
    fn attach_thread_input_guard_drop_runs_on_panic() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let before = COUNTER.load(Ordering::SeqCst);
        let result = std::panic::catch_unwind(|| {
            let _guard = AttachThreadInputGuard::fake_for_test(&COUNTER);
            // Counter must NOT have incremented yet (still inside scope).
            assert_eq!(COUNTER.load(Ordering::SeqCst), before);
            panic!("simulated panic inside guarded scope");
        });
        // Closure panicked — that's the test's setup.
        assert!(result.is_err());
        // But the guard's Drop ran during unwind, so counter ticked.
        assert_eq!(COUNTER.load(Ordering::SeqCst), before + 1);
    }
}
