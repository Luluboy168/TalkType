// Clipboard paste plugin (M4 chunk 2).
//
// Public surface:
//   * `FocusState`        — Tauri-managed state holding the most recent
//                           foreground HWND (captured at hotkey-press time).
//   * `ClipboardError`    — typed error enum, manually `Serialize`-d as flat
//                           string per CLAUDE.md "Error enum 手動 implement
//                           Serialize 為 string".
//   * 3 Tauri commands    — `capture_target_window`, `paste_text`,
//                           `copy_to_clipboard`. Wired into `lib.rs` by main
//                           session integration (chunk 0 already declared
//                           the module in `plugins/mod.rs`).
//   * `apply_hud_no_activate_style` — Rust helper for the `lib.rs` setup hook
//                           to add `WS_EX_NOACTIVATE` to the HUD window AFTER
//                           creation (Tauri config does not expose it).
//
// Implementation lives in the sibling `paste.rs` module so this file stays
// focused on the public-surface declarations + dispatch — the 7-step Win32
// pipeline + RAII guards are kept off the public surface.
//
// See `doc/plans/03-rust-modules.md` "M4 challenger findings 落實" for the
// full design contract this module implements.

mod paste;

use std::sync::Mutex;

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Tauri-managed state for the paste pipeline. Holds the foreground HWND
/// captured at hotkey-press time so a later `paste_text` can restore focus
/// to that window before injecting Ctrl+V.
///
/// Stored as `isize` because the inner pointer of `windows::Win32::Foundation::HWND`
/// is `*mut c_void` which is neither `Send` nor `Sync` — we serialize it to
/// an integer at the boundary and reconstruct the HWND inside `paste.rs`.
pub struct FocusState {
    /// Most recent foreground HWND captured at hotkey-press time. Stored as
    /// `isize` so it serializes simply; converted back to HWND inside
    /// `paste.rs` for Win32 calls. Defaults to 0 (no target captured yet).
    pub(super) target_hwnd: Mutex<isize>,
}

impl FocusState {
    /// Build a fresh `FocusState` with no target captured. Wired into
    /// `lib.rs` via `app.manage(FocusState::new())` during setup.
    pub fn new() -> Self {
        Self {
            target_hwnd: Mutex::new(0),
        }
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by the clipboard_paste Tauri commands.
///
/// Manual `Serialize` impl below renders each variant as a flat string for
/// the frontend per CLAUDE.md "Error enum 手動 implement Serialize 為
/// string". Variants are intentionally split so the HUD / error panel can
/// localize each path independently:
///
///   * `ClipboardSetFailed` / `ClipboardGetFailed` — arboard / OleSetClipboard
///     failed (rare; likely RDP session or another process holding the
///     clipboard exclusively).
///   * `NoTargetWindow` — frontend invoked `paste_text` without a prior
///     `capture_target_window`. UX surface: "try again".
///   * `FocusRestoreFailed` — Windows 11 anti-flash policy refused
///     `SetForegroundWindow`. Text is still on the clipboard so the HUD
///     shows "請手動 Ctrl+V". Also emits the `paste:focus-restore-failed`
///     Tauri event with the same `hwnd` / `lastErrorCode`.
///   * `SendInputFailed` — `SendInput` injected fewer than the expected
///     events; typically UIPI / locked workstation.
///   * `ComInitFailed` — `CoInitializeEx` returned an unexpected HRESULT
///     during the STA hop for arboard. Should be effectively impossible.
///   * `LockPoisoned` — the `FocusState` mutex was poisoned by a prior panic.
///   * `WindowHandleUnavailable` — `tauri::WebviewWindow::hwnd()` failed
///     during `apply_hud_no_activate_style`. Surfaced from the setup hook
///     so a Tauri startup failure isn't silent.
#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("Failed to set clipboard text: {0}")]
    ClipboardSetFailed(String),

    #[error("Failed to read clipboard text: {0}")]
    ClipboardGetFailed(String),

    #[error("No target window captured (call capture_target_window first)")]
    NoTargetWindow,

    #[error("Failed to restore focus to target HWND {hwnd:#x}: GetLastError={last_error}")]
    FocusRestoreFailed { hwnd: isize, last_error: u32 },

    #[error("SendInput returned fewer than {expected} input events")]
    SendInputFailed { expected: u32 },

    #[error("CoInitializeEx failed (HRESULT {hresult:#x})")]
    ComInitFailed { hresult: i32 },

    #[error("FocusState lock poisoned")]
    LockPoisoned,

    #[error("Window handle unavailable: {0}")]
    WindowHandleUnavailable(String),

    /// `tokio::task::spawn_blocking` join failed because the inner closure
    /// panicked. Distinguishes a real panic from a synchronous error variant
    /// (which would have been propagated through the closure's `Result`).
    /// Reviewer P1 #1: previous code mislabeled all join errors as
    /// `SendInputFailed`, hiding the actual stage that panicked.
    #[error("Paste task panicked during stage '{stage}'")]
    TaskPanic { stage: &'static str },
}

// Manual `Serialize` so the frontend receives a flat string rather than a
// tagged enum object — mirrors `TranscriptionError` / `AudioRecorderError`.
impl Serialize for ClipboardError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Capture the current foreground HWND so a later `paste_text` can restore
/// focus to it before injecting Ctrl+V. Frontend calls this from the
/// hotkey-down handler in `useVoiceFlowStore` BEFORE `start_recording`.
///
/// On non-Windows builds this is a no-op that always succeeds (the paste
/// path will still return `NoTargetWindow`).
#[tauri::command]
pub async fn capture_target_window(
    state: tauri::State<'_, FocusState>,
) -> Result<(), ClipboardError> {
    paste::capture_target_window(&state).await
}

/// Drive the 7-step Windows paste pipeline: clipboard set → 50ms commit →
/// AttachThreadInput + SetForegroundWindow → 50ms settle → modifier release
/// → IME composition complete → SendInput Ctrl+V. The text hits the
/// clipboard regardless of focus-restore success, so the user always has a
/// manual Ctrl+V fallback. On focus failure, also emits
/// `paste:focus-restore-failed` so the HUD can surface a friendly hint.
#[tauri::command]
pub async fn paste_text(
    app: tauri::AppHandle,
    state: tauri::State<'_, FocusState>,
    text: String,
) -> Result<(), ClipboardError> {
    paste::paste_text(app, &state, text).await
}

/// Pure clipboard-set with no paste injection. Same `spawn_blocking` + STA
/// discipline as the paste path because arboard's Windows backend still
/// needs an apartment.
#[tauri::command]
pub async fn copy_to_clipboard(text: String) -> Result<(), ClipboardError> {
    paste::copy_to_clipboard(text).await
}

// ─── Setup helper ──────────────────────────────────────────────────────────

/// Add the `WS_EX_NOACTIVATE` extended style to the HUD window. Called from
/// `lib.rs` setup AFTER the HUD has been created — `tauri.conf.json` does
/// not expose this style directly, so we have to set it via SetWindowLongPtrW
/// post-creation.
///
/// Without this, `paste_text`'s `SetForegroundWindow(target_hwnd)` can be
/// fooled into treating the HUD as "the most recently active window" and
/// hand focus back to it instead of the user's actual target app.
///
/// No-op on non-Windows targets.
#[cfg(target_os = "windows")]
pub fn apply_hud_no_activate_style(window: &tauri::WebviewWindow) -> Result<(), ClipboardError> {
    paste::apply_hud_no_activate_style(window)
}

#[cfg(not(target_os = "windows"))]
pub fn apply_hud_no_activate_style(_window: &tauri::WebviewWindow) -> Result<(), ClipboardError> {
    Ok(())
}
