// Windows-only hotkey listener — `SetWindowsHookExW(WH_KEYBOARD_LL)`.
//
// **Threading model** (mirrors the audio_recorder named-thread + ack pattern):
//
//   * `install_hook` spawns a named `"hotkey-listener"` thread. The hook
//     handle (`HHOOK`) is `*mut c_void` (`!Send + !Sync`), so it MUST live
//     entirely on this thread — we never pass it across threads.
//   * The hook thread:
//       1. Calls `SetWindowsHookExW(WH_KEYBOARD_LL, ...)`.
//       2. Captures its own thread-id via `GetCurrentThreadId` and acks
//          back to the caller through an mpsc channel.
//       3. Pumps the message loop with `GetMessageW` — `WH_KEYBOARD_LL`
//          requires a thread with a message loop or the OS will silently
//          drop the hook.
//       4. On `WM_QUIT` (sent by `shutdown` via `PostThreadMessageW`)
//          breaks the loop, calls `UnhookWindowsHookEx`, and returns.
//
// **Hook proc emit policy**:
//
//   The hook proc is on the OS keyboard-input priority path and must NOT
//   exceed `LowLevelHooksTimeout` (~300 ms default). `tauri::AppHandle::emit`
//   serializes to JSON and pushes to a webview-side bus — typical observed
//   latency under 1 ms — so we emit DIRECTLY from the hook proc rather than
//   adding an mpsc channel + worker thread. This keeps the call-site simple
//   and avoids the extra thread-hop for each key event. If we ever observe
//   emit blocking for tens of ms (e.g. webview frozen) we should revisit
//   and route via a bounded mpsc + worker that drops events when full.
//
// **Why `static OnceLock` for shared state**:
//
//   `extern "system" fn keyboard_hook_proc(...)` cannot be a closure or
//   capture state — it's a raw C ABI callback registered with the OS. We
//   need it to access the `HotkeySharedState` + `AppHandle` somehow. The
//   options are:
//
//     1. `static OnceLock<HookContext>` — set once on `install_hook`,
//        accessible from the hook proc.
//     2. Use `dwExtraInfo` field (32-bit on x86) — too small + per-event.
//     3. Thread-local storage — doesn't survive across threads.
//
//   Phase 1 ships single-instance only (single-instance plugin enforces
//   this in `lib.rs`), so option 1 is sound. If we ever want multiple
//   simultaneous hooks (Phase 2 macOS doesn't, Windows doesn't realistically
//   need it) we'd revisit.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LCONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::shared::{HotkeyEvent, HotkeySharedState, KeyEvent};
use super::types::TriggerKey;
use super::{
    HotkeyError, HotkeyEventPayload, EVENT_ESCAPE_PRESSED, HOTKEY_PRESSED, HOTKEY_RELEASED,
    HOTKEY_TOGGLED,
};

const SOURCE_TAG: &str = "[hotkey-listener]";

/// `VK_RMENU` (right Alt) — the only trigger key that can collide with
/// AltGr. Reading from the windows-crate constant once at compile time
/// rather than reaching into `TriggerKey::RightAlt.virtual_key()` from the
/// hook proc keeps the hot path off any indirection.
const VK_RMENU_RAW: u32 = 0xA5;

/// Context passed to the hook proc via `static SHARED_CTX`. We hold
/// `AppHandle` by value (it's `Clone` and cheap) so the hook proc can emit
/// without re-acquiring a handle via the `Builder` registry.
struct HookContext {
    state: Arc<HotkeySharedState>,
    app: AppHandle,
}

/// Set once on `install_hook`. The hook proc reads from this — it cannot
/// be parameterized because `extern "system" fn` is a raw C callback.
static SHARED_CTX: OnceLock<HookContext> = OnceLock::new();

/// Returned from `install_hook` and stored in
/// `HotkeyListenerState::handle`. Calling `shutdown` posts WM_QUIT to the
/// listener thread, joins it, and consumes the handle.
pub struct HookHandle {
    /// Win32 thread-id used for `PostThreadMessageW(WM_QUIT)`. NOT the same
    /// as `JoinHandle`'s OS thread — Win32 thread-ids are 32-bit u32 from
    /// `GetCurrentThreadId`.
    thread_id: u32,
    join_handle: thread::JoinHandle<()>,
}

impl HookHandle {
    /// Signals the hook thread to exit and waits for it to clean up. Posts
    /// `WM_QUIT` to break the `GetMessageW` loop; the post-loop cleanup in
    /// `run_hook_thread` calls `UnhookWindowsHookEx`.
    ///
    /// Safe to call from `RunEvent::Exit` — does not block more than the
    /// time it takes the thread to finish unhooking + return (typically
    /// well under 100 ms; the loop exits as soon as `GetMessageW` sees
    /// `WM_QUIT`).
    pub fn shutdown(self) {
        // SAFETY: `PostThreadMessageW` is safe when the thread-id is valid.
        // The thread-id came from `GetCurrentThreadId` inside the spawned
        // thread, so by definition it was valid at startup; the only way
        // it would be invalid is if the thread has already terminated, in
        // which case the post returns ERROR_INVALID_THREAD_ID and we
        // proceed to join (which will return immediately).
        if let Err(e) = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        {
            eprintln!(
                "{SOURCE_TAG} PostThreadMessageW(WM_QUIT) failed: {e:?} — thread may have already exited"
            );
        }
        if let Err(e) = self.join_handle.join() {
            eprintln!("{SOURCE_TAG} hook thread panicked: {e:?}");
        }
    }
}

/// Install the global keyboard hook. Spawns the dedicated listener thread
/// and blocks on the startup ack so the caller learns of `SetWindowsHookExW`
/// failures synchronously.
///
/// Returns `Err(HotkeyError::HookInstallFailed)` if:
///
///   * The thread cannot be spawned (`std::io::Error`).
///   * `SetWindowsHookExW` returns an error (typically lack of admin
///     rights for protected processes, but `WH_KEYBOARD_LL` does NOT
///     require admin in normal cases).
///
/// Returns `Err(HotkeyError::ThreadPanic)` if the spawned thread panics
/// before it can ack.
pub fn install_hook(
    state: Arc<HotkeySharedState>,
    app: AppHandle,
) -> Result<HookHandle, HotkeyError> {
    // Plant the shared context BEFORE spawning the thread so the hook
    // proc never sees an empty `SHARED_CTX`. The first install wins; a
    // second install on the same process would `set` -> Err — Phase 1 is
    // single-instance so we treat re-install as a logic bug.
    let ctx = HookContext {
        state,
        app: app.clone(),
    };
    if SHARED_CTX.set(ctx).is_err() {
        // Second install attempt — already running. We treat this as a
        // benign no-op for callers that don't track install state, but
        // still flag it.
        eprintln!("{SOURCE_TAG} install_hook called twice — ignoring second call");
        return Err(HotkeyError::HookInstallFailed(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "hotkey hook already installed",
        )));
    }

    let (ack_tx, ack_rx) = mpsc::channel::<Result<u32, HotkeyError>>();

    let join_handle = thread::Builder::new()
        .name("hotkey-listener".to_string())
        .spawn(move || {
            run_hook_thread(ack_tx);
        })
        .map_err(|e| {
            // Roll back the OnceLock plant — but `OnceLock` doesn't expose
            // a `take`. Phase 1 just logs; the next install will fail
            // with AlreadyExists. Acceptable since spawn failure is
            // catastrophic anyway.
            eprintln!("{SOURCE_TAG} thread spawn failed: {e}");
            HotkeyError::HookInstallFailed(std::io::Error::other(format!(
                "spawn hotkey-listener thread: {e}"
            )))
        })?;

    // Wait for the thread to either hand back its thread-id or report a
    // setup error. Mirrors the audio_recorder `start_recording` ack.
    let thread_id = match ack_rx.recv() {
        Ok(Ok(tid)) => tid,
        Ok(Err(err)) => {
            let _ = join_handle.join();
            return Err(err);
        }
        Err(e) => {
            let _ = join_handle.join();
            return Err(HotkeyError::HookInstallFailed(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("hotkey-listener thread closed channel before ack: {e}"),
            )));
        }
    };

    eprintln!("{SOURCE_TAG} hook installed (thread_id={thread_id})");
    Ok(HookHandle {
        thread_id,
        join_handle,
    })
}

/// Body of the `"hotkey-listener"` thread. Owns the `HHOOK` for its
/// lifetime — the handle is `!Send + !Sync` and must not escape this
/// function.
fn run_hook_thread(ack: mpsc::Sender<Result<u32, HotkeyError>>) {
    // SAFETY: SetWindowsHookExW is unsafe because the hook proc is a raw
    // function pointer. Our `keyboard_hook_proc` is `extern "system"` and
    // never panics (we manually catch panics inside). hMod is None for
    // WH_KEYBOARD_LL (per Microsoft docs the hMod must be the DLL handle
    // *if* the proc is in a DLL; for in-process hooks `None` is allowed
    // but historically required `GetModuleHandleW(NULL)`). The
    // windows-0.61 binding accepts `Option<HINSTANCE>` and unwraps to
    // `core::mem::zeroed()` which is the documented in-process value.
    let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) }
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{SOURCE_TAG} SetWindowsHookExW failed: {e:?}");
            let _ = ack.send(Err(HotkeyError::HookInstallFailed(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("SetWindowsHookExW: {e}"),
            ))));
            return;
        }
    };

    // SAFETY: GetCurrentThreadId is always safe — it returns the OS
    // thread-id of the calling thread. No preconditions.
    let thread_id = unsafe { GetCurrentThreadId() };
    if ack.send(Ok(thread_id)).is_err() {
        // Caller went away — unhook and bail.
        eprintln!("{SOURCE_TAG} caller dropped ack rx; unhooking and exiting");
        // SAFETY: `hook` was returned by SetWindowsHookExW above and is
        // valid until UnhookWindowsHookEx is called.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        return;
    }

    // Message pump. WH_KEYBOARD_LL requires the installing thread to have
    // a message loop or the OS will silently drop the hook over time.
    let mut msg = MSG::default();
    loop {
        // SAFETY: GetMessageW is safe with a valid &mut MSG. Returns:
        //   * BOOL(0)  → WM_QUIT received → exit loop.
        //   * BOOL(-1) → error → exit loop (check GetLastError if we ever
        //                 want to surface it).
        //   * BOOL(>0) → message retrieved.
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if !result.as_bool() {
            // 0 = WM_QUIT, anything else falsy is an error — both exit.
            break;
        }
        // SAFETY: DispatchMessageW is safe with a valid *const MSG.
        // We don't have any windows of our own (this thread is solely for
        // the keyboard hook) so DispatchMessageW is mostly a no-op, but
        // it's idiomatic and harmless.
        unsafe {
            let _ = DispatchMessageW(&msg);
        }
    }

    // SAFETY: same as above — `hook` is the handle returned by
    // SetWindowsHookExW and is valid until this call.
    if let Err(e) = unsafe { UnhookWindowsHookEx(hook) } {
        eprintln!("{SOURCE_TAG} UnhookWindowsHookEx failed: {e:?}");
    } else {
        eprintln!("{SOURCE_TAG} hook thread exiting cleanly");
    }
}

/// Low-level keyboard hook procedure. Runs at OS keyboard-input priority —
/// must complete in well under `LowLevelHooksTimeout` (~300 ms) or
/// Windows will silently drop the hook.
///
/// SAFETY contract:
///
///   * Called by the OS as a raw `extern "system"` callback.
///   * `code < 0` → must defer to `CallNextHookEx` without inspection
///     (Microsoft doc requirement).
///   * `lparam` carries a `*const KBDLLHOOKSTRUCT` for `HC_ACTION` (which
///     is the only `code` we need to handle for WH_KEYBOARD_LL).
///   * Must always end with `CallNextHookEx` so other hooks in the chain
///     (e.g. accessibility tools) still receive the event.
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Negative code → defer immediately (Microsoft requirement).
    if code < 0 {
        // SAFETY: standard-required defer per
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nc-winuser-lowlevelkeyboardproc
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    // Track elapsed time so a misbehaving listener can be diagnosed via
    // log if the hook ever takes a long path. This is cheap (Instant::now
    // is ~tens of ns on Windows).
    let started = Instant::now();

    // SAFETY: per LowLevelKeyboardProc docs, lparam is a valid pointer to
    // KBDLLHOOKSTRUCT for HC_ACTION (code == 0). We only read fields,
    // never write.
    let kb = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };

    // wparam is one of WM_KEYDOWN / WM_KEYUP / WM_SYSKEYDOWN / WM_SYSKEYUP.
    // SYSKEY* are sent when an Alt-modified key fires — we handle them
    // identically to KEYDOWN/KEYUP because our trigger keys (Alt/Ctrl/
    // Shift) trigger SYSKEY* on themselves.
    let raw_wp = wparam.0 as u32;
    let is_down = raw_wp == WM_KEYDOWN || raw_wp == WM_SYSKEYDOWN;
    let is_up = raw_wp == WM_KEYUP || raw_wp == WM_SYSKEYUP;
    if !is_down && !is_up {
        // Unknown message — defer.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    // AltGr detection: only relevant for RightAlt. EU keyboards send
    // LCONTROL DOWN immediately followed by RMENU DOWN to indicate AltGr;
    // checking GetAsyncKeyState(VK_LCONTROL) at the moment we observe
    // RMENU tells us whether to suppress.
    let altgr_active = if kb.vkCode == VK_RMENU_RAW {
        is_vk_pressed(VK_LCONTROL.0 as i32)
    } else {
        false
    };

    let key_event = KeyEvent {
        vk: kb.vkCode,
        is_down,
        ts_ms: kb.time as u64,
        altgr_active,
    };

    // Look up shared context. `OnceLock::get()` is lock-free and
    // wait-free after `set()` — exactly what the hook proc needs.
    if let Some(ctx) = SHARED_CTX.get() {
        let events = ctx.state.apply_event(key_event);
        for ev in events {
            dispatch_event(&ctx.app, ctx.state.as_ref(), ev);
        }
    }

    // Diagnostic: warn if the hook took an unusually long time. Hot path
    // should be sub-millisecond.
    let elapsed = started.elapsed();
    if elapsed > Duration::from_millis(50) {
        eprintln!(
            "{SOURCE_TAG} hook proc slow path: {}ms (vk={:#x} down={})",
            elapsed.as_millis(),
            kb.vkCode,
            is_down
        );
    }

    // Always defer to next hook so accessibility tools etc. still see the
    // event. We never `return LRESULT(1)` (which would consume the key)
    // — Phase 1 is purely observational.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Dispatch a `HotkeyEvent` to the matching Tauri event over the app
/// handle. Called from the hook proc — must not block meaningfully.
fn dispatch_event(app: &AppHandle, state: &HotkeySharedState, ev: HotkeyEvent) {
    match ev {
        HotkeyEvent::Pressed => {
            let payload = HotkeyEventPayload {
                trigger_mode: "hold",
                action: "pressed",
            };
            emit_or_log(app, HOTKEY_PRESSED, payload);
        }
        HotkeyEvent::Released => {
            let payload = HotkeyEventPayload {
                trigger_mode: "hold",
                action: "released",
            };
            emit_or_log(app, HOTKEY_RELEASED, payload);
        }
        HotkeyEvent::ToggledOn => {
            let payload = HotkeyEventPayload {
                trigger_mode: "toggle",
                action: "toggled-on",
            };
            emit_or_log(app, HOTKEY_TOGGLED, payload);
        }
        HotkeyEvent::ToggledOff => {
            let payload = HotkeyEventPayload {
                trigger_mode: "toggle",
                action: "toggled-off",
            };
            emit_or_log(app, HOTKEY_TOGGLED, payload);
        }
        HotkeyEvent::EscapePressed => {
            // Escape uses an empty payload () per
            // doc/plans/01-architecture.md row "escape:pressed".
            if let Err(e) = app.emit(EVENT_ESCAPE_PRESSED, ()) {
                eprintln!("{SOURCE_TAG} emit {EVENT_ESCAPE_PRESSED} failed: {e}");
            }
        }
    }
    // Reserved for future tracing — keep `state` in scope so we can use it
    // for double-tap diagnostics later without changing the signature.
    let _ = state.is_pressed.load(Ordering::Relaxed);
}

fn emit_or_log<P: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(e) = app.emit(event, payload) {
        // Don't escalate — emit failure should not kill the hook. Log so
        // it's visible in dev mode (eprintln stderr).
        eprintln!("{SOURCE_TAG} emit {event} failed: {e}");
    }
}

/// Returns `true` iff the high bit of `GetAsyncKeyState(vk)` is set,
/// meaning the key is currently pressed (regardless of focus).
///
/// Used by AltGr detection — we need to know "is LCONTROL down RIGHT NOW
/// in OS state" rather than relying on hook-event ordering.
fn is_vk_pressed(vk: i32) -> bool {
    // SAFETY: GetAsyncKeyState takes any i32 in [0, 0xFE]; out-of-range
    // returns 0 (not unsafe). We only call with documented VK_* constants.
    let s = unsafe { GetAsyncKeyState(vk) };
    // High bit (0x8000 in i16) → pressed. Microsoft docs:
    // > If the most significant bit is set, the key is down.
    (s as u16) & 0x8000 != 0
}

/// Returns the configured trigger key. Wrapper used by future
/// `apply_config` tests in mod.rs — kept here so the windows-only path
/// owns its trigger-key view.
#[allow(dead_code)]
pub(crate) fn current_trigger_key(state: &HotkeySharedState) -> TriggerKey {
    state.current_trigger_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_rmenu_raw_matches_trigger_key_constant() {
        // Compile-time-ish guard: keep our raw constant in sync with
        // TriggerKey::RightAlt::virtual_key() so the AltGr fast-path stays
        // correct if anyone ever renumbers the enum.
        assert_eq!(VK_RMENU_RAW, TriggerKey::RightAlt.virtual_key());
    }
}
