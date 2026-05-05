// IME composition completion for the Windows paste pipeline.
//
// When the user has an IME (Microsoft Bopomofo, Japanese IME, etc.) with an
// in-flight composition string, our `SendInput(Ctrl+V)` would land alongside
// the pending composition rather than after it. Best-effort fix: ask the
// target window's IME to commit any pending composition string BEFORE we
// inject the paste — `ImmGetContext(target)` gives us the IME context (or
// null when no IME is associated, which is fine), then
// `ImmNotifyIME(NI_COMPOSITIONSTR, CPS_COMPLETE, 0)` commits the
// composition.
//
// Best-effort: BOOL return is ignored because failing here means at worst
// the IME state survives and the paste runs anyway.

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HWND,
    UI::Input::Ime::{
        ImmGetContext, ImmNotifyIME, ImmReleaseContext, CPS_COMPLETE, NI_COMPOSITIONSTR,
    },
};

/// Best-effort: tell the IME to commit any in-flight composition string so
/// paste doesn't end up beside it. `ImmGetContext` returns null when no IME
/// is associated with the window — that's normal, not an error.
#[cfg(target_os = "windows")]
pub(super) fn complete_ime_composition(target_raw: isize) {
    let target = HWND(target_raw as *mut core::ffi::c_void);
    // SAFETY: ImmGetContext / ImmNotifyIME / ImmReleaseContext are FFI-safe;
    // null HIMC is a documented "no IME" sentinel.
    unsafe {
        let himc = ImmGetContext(target);
        if !himc.0.is_null() {
            // Commit the composition string. Best-effort: BOOL return is
            // ignored — failing here means at worst the IME state survives
            // and the paste runs anyway.
            let _ = ImmNotifyIME(himc, NI_COMPOSITIONSTR, CPS_COMPLETE, 0);
            let _ = ImmReleaseContext(target, himc);
        }
    }
}
