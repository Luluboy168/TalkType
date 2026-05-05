// COM apartment guard for the Windows paste pipeline.
//
// `arboard` 3.x Windows backend uses `OleSetClipboard`, which requires the
// calling thread to be in a Single-Threaded Apartment (STA). Tauri commands
// run on the tokio multi-thread runtime workers (which are MTA), so we must
// hop into a `spawn_blocking` thread and stand up our own STA via
// `CoInitializeEx(COINIT_APARTMENTTHREADED)` for the duration of the
// `arboard::Clipboard` call, then call `CoUninitialize` on the way out.
//
// The `ComGuard` RAII type makes that pairing automatic — including on
// panic-unwound paths.

#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use super::super::ClipboardError;

/// `ComGuard`: scope-bound `CoInitializeEx(COINIT_APARTMENTTHREADED)` so the
/// arboard call has an STA. Calls `CoUninitialize` on drop — including on
/// panic-unwound paths.
///
/// We tolerate `RPC_E_CHANGED_MODE`: that means the current thread already
/// has a different apartment (multi-threaded). In that case CoInitializeEx
/// returns the error but does NOT actually initialize, so we skip the
/// matching `CoUninitialize` (tracked via `should_uninit`).
///
/// On non-Windows targets this is a no-op — the type still exists so the
/// rest of the code can reference it without `cfg`.
#[cfg(target_os = "windows")]
pub(super) struct ComGuard {
    /// True iff we successfully called `CoInitializeEx` on this thread and
    /// must balance it with `CoUninitialize` on drop.
    should_uninit: bool,
    /// Test hook — counts Drop invocations across all guards in a process.
    /// Production code path leaves this `None`; tests inject a counter to
    /// assert Drop fired without touching real Win32.
    #[cfg(test)]
    counter: Option<&'static std::sync::atomic::AtomicUsize>,
}

#[cfg(target_os = "windows")]
impl ComGuard {
    /// Initialize an STA on the current thread. Returns Err on any HRESULT
    /// other than S_OK / S_FALSE / RPC_E_CHANGED_MODE.
    pub(super) fn enter() -> Result<Self, ClipboardError> {
        // SAFETY: CoInitializeEx is FFI-safe; we pass a null reserved arg.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // S_OK (0) — initialized fresh.
        // S_FALSE (1) — already initialized with the same apartment model.
        // RPC_E_CHANGED_MODE (0x80010106) — different apartment already; skip uninit.
        let raw = hr.0 as u32;
        if raw == 0 || raw == 1 {
            Ok(Self {
                should_uninit: true,
                #[cfg(test)]
                counter: None,
            })
        } else if raw == 0x80010106 {
            Ok(Self {
                should_uninit: false,
                #[cfg(test)]
                counter: None,
            })
        } else {
            Err(ClipboardError::ComInitFailed { hresult: hr.0 })
        }
    }

    /// Test-only: build a fake guard that increments `counter` on drop.
    /// Lets the unit test verify `Drop` actually fires without invoking
    /// real Win32 (which would require a properly initialized COM thread).
    #[cfg(test)]
    pub(super) fn fake_for_test(counter: &'static std::sync::atomic::AtomicUsize) -> Self {
        Self {
            should_uninit: false,
            counter: Some(counter),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            // SAFETY: balances the `CoInitializeEx` we made in `enter`.
            unsafe { CoUninitialize() };
        }
        #[cfg(test)]
        if let Some(c) = self.counter {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(super) struct ComGuard;

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test 3: ComGuard::Drop fires.
    ///
    /// We can't safely call `ComGuard::enter()` from a multi-thread tokio
    /// test runtime (it might mutate the apartment of the wrong thread), so
    /// we use the `fake_for_test` constructor that wires up a counter and
    /// skips the real `CoUninitialize` call.
    #[test]
    fn com_guard_drop_fires() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let before = COUNTER.load(Ordering::SeqCst);
        {
            let _guard = ComGuard::fake_for_test(&COUNTER);
            assert_eq!(COUNTER.load(Ordering::SeqCst), before);
        }
        // After scope exit Drop must have fired exactly once.
        assert_eq!(COUNTER.load(Ordering::SeqCst), before + 1);
    }
}
