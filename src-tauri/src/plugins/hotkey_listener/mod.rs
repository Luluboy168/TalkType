// Hotkey listener plugin (M4 chunk 1).
//
// Public surface:
//
//   State           HotkeyListenerState — held in `tauri::State`, holds the
//                                          shared atomics + the platform
//                                          install handle (Windows only).
//   Commands        update_hotkey_config / start_hotkey_recording /
//                   cancel_hotkey_recording (the two recording stubs return
//                   `NotImplemented` in Phase 1).
//   Internal API    install / shutdown / apply_config — called by `lib.rs`
//                   in the `setup` callback and `RunEvent::Exit` handler.
//
// **Submodules**:
//
//   * `types.rs`   — `TriggerKey` / `TriggerMode` / `HotkeyConfig` (serde
//                    rename + atomic-byte conversions).
//   * `shared.rs`  — `HotkeySharedState` + pure-logic state machine
//                    (`apply_event`). Platform-agnostic; tested in unit
//                    tests with a fake `KeyEvent` source.
//   * `windows.rs` — `cfg(target_os = "windows")` only. Owns the hook
//                    thread + `SetWindowsHookExW` install / unhook. The
//                    hook proc reads from a `static OnceLock<HookContext>`
//                    so the C-ABI callback can reach `HotkeySharedState`.
//
// **Threading model** (mirrors audio_recorder):
//
//   * `install_hook` spawns a named `"hotkey-listener"` thread with an
//     mpsc-channel ack. The thread owns the `HHOOK` (`!Send + !Sync`) for
//     its lifetime.
//   * `shutdown` posts `WM_QUIT` to the thread + joins.
//   * The hook proc emits Tauri events directly (sub-ms `app.emit` is
//     well within the 300 ms `LowLevelHooksTimeout`). Documented choice
//     in `windows.rs` module comment.
//
// **Error contract**: thiserror enum with manual `Serialize` to a flat
// string, mirroring `AudioRecorderError` / `CredentialsError`. The
// frontend receives a `string` from each `invoke<T>()` failure path,
// never a tagged-enum object.

pub mod shared;
pub mod types;

#[cfg(target_os = "windows")]
mod windows;

use std::sync::Arc;
use std::sync::Mutex;

use serde::{Serialize, Serializer};
use thiserror::Error;

pub use types::{HotkeyConfig, TriggerKey, TriggerMode};

// ─── Tauri event names ─────────────────────────────────────────────────────
//
// Mirrors `src/composables/useTauriEvents.ts` — keep in sync with
// `doc/plans/01-architecture.md` "Tauri Events" table.

/// Hold-mode key-down. Payload: `HotkeyEventPayload { triggerMode: "hold",
/// action: "pressed" }`.
pub const HOTKEY_PRESSED: &str = "hotkey:pressed";

/// Hold-mode key-up. Payload: `HotkeyEventPayload { triggerMode: "hold",
/// action: "released" }`.
pub const HOTKEY_RELEASED: &str = "hotkey:released";

/// Toggle-mode XOR. Payload: `HotkeyEventPayload { triggerMode: "toggle",
/// action: "toggled-on" | "toggled-off" }`.
pub const HOTKEY_TOGGLED: &str = "hotkey:toggled";

/// ESC key-down at any time. Payload: `()`. Cancels in-progress
/// recording without paste.
pub const EVENT_ESCAPE_PRESSED: &str = "escape:pressed";

// ─── Event payloads ────────────────────────────────────────────────────────

/// Payload of `hotkey:pressed` / `hotkey:released` / `hotkey:toggled` —
/// must match `src/types/events.ts::HotkeyEventPayload`. Hold and Toggle
/// share the shape so the same listener can dispatch.
///
/// `&'static str` rather than `String` because every emit-site uses a
/// compile-time literal — saves the heap alloc on the hook hot path.
#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyEventPayload {
    /// `"hold"` | `"toggle"`. Matches `HotkeyConfig.triggerMode` at event
    /// time so the frontend listener can branch on mode without
    /// re-querying settings.
    pub trigger_mode: &'static str,
    /// `"pressed"` | `"released"` | `"toggled-on"` | `"toggled-off"`.
    pub action: &'static str,
}

// ─── Error type ────────────────────────────────────────────────────────────

/// Errors returned by the hotkey_listener Tauri commands and internal
/// install/shutdown paths.
#[derive(Error, Debug)]
pub enum HotkeyError {
    /// `SetWindowsHookExW` failed or the listener thread couldn't be
    /// spawned. Wraps the underlying `std::io::Error` so the call site
    /// in `lib.rs` setup can log a meaningful message.
    #[error("Failed to install Windows keyboard hook: {0}")]
    HookInstallFailed(#[source] std::io::Error),

    /// The hotkey listener thread panicked. Surfaced from the join handle
    /// when shutdown joins a panicked thread.
    #[error("Hotkey listener thread panicked")]
    ThreadPanic,

    /// `update_hotkey_config` was called before `install`. Should not
    /// happen in normal flow — `lib.rs` setup installs before any
    /// command can run.
    #[error("Hotkey listener not installed")]
    NotInstalled,

    /// Custom hotkey recording is Phase 2 only. Phase 1 returns this from
    /// `start_hotkey_recording` / `cancel_hotkey_recording` so the
    /// command surface stays stable while UI hides the buttons.
    #[error("Custom hotkey recording is not implemented in Phase 1")]
    NotImplemented,
}

// Manual `Serialize` so the frontend receives a flat string, matching the
// pattern in `AudioRecorderError` / `CredentialsError`.
impl Serialize for HotkeyError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ─── State ─────────────────────────────────────────────────────────────────

/// Tauri-managed state for the hotkey listener. Held inside `app.manage()`
/// from `lib.rs`. The platform-specific install handle (`HookHandle`) lives
/// behind a `Mutex<Option<...>>` so:
///
///   * `install` (called once from `setup`) plants the handle.
///   * `shutdown` (called from `RunEvent::Exit`) takes the handle and
///     posts WM_QUIT + joins.
///   * `apply_config` (called from `update_hotkey_config`) doesn't need
///     the handle — it just writes to the atomics inside `shared`.
///
/// `Arc<HotkeySharedState>` is shared between the install thread (via
/// `windows::SHARED_CTX`) and the command thread (via this struct). The
/// hook proc reads atomics; the command thread writes them. No `Mutex`
/// on the hot path.
pub struct HotkeyListenerState {
    pub shared: Arc<shared::HotkeySharedState>,
    /// Platform install handle. `None` before `install` and after
    /// `shutdown`. Wrapped in std `Mutex` because we don't need
    /// `parking_lot` (deps are constrained in chunk 0).
    #[cfg(target_os = "windows")]
    handle: Mutex<Option<windows::HookHandle>>,
    /// Non-Windows builds keep the field for symmetry but never set it.
    /// Phase 2 macOS will swap this for a `CGEventTap` handle.
    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    handle: Mutex<Option<()>>,
}

impl HotkeyListenerState {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(shared::HotkeySharedState::default()),
            handle: Mutex::new(None),
        }
    }

    /// Install the platform-specific hook + apply the initial config. Called
    /// from `lib.rs` `setup` once per process.
    ///
    /// On non-Windows builds this is a no-op that still applies the
    /// in-memory config so the state machine is consistent. Phase 2 macOS
    /// will replace the inner with `CGEventTap` install.
    #[allow(unused_variables)]
    pub fn install(&self, app: tauri::AppHandle, config: HotkeyConfig) -> Result<(), HotkeyError> {
        // Apply config first — the hook proc reads these atomics on every
        // event so we want them set before `SetWindowsHookExW` returns.
        self.apply_config(config);

        #[cfg(target_os = "windows")]
        {
            let hook_handle = windows::install_hook(self.shared.clone(), app)?;
            let mut guard = self.handle.lock().map_err(|_| HotkeyError::ThreadPanic)?;
            *guard = Some(hook_handle);
        }

        Ok(())
    }

    /// Tear down the hook. Idempotent — safe to call from both
    /// `RunEvent::Exit` and any cleanup path. On Windows posts WM_QUIT to
    /// the listener thread and joins.
    pub fn shutdown(&self) {
        #[cfg(target_os = "windows")]
        {
            let taken = self.handle.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = taken {
                handle.shutdown();
            }
        }
    }

    /// Hot-swap config without re-installing the hook. Just writes the
    /// atomics — `windows::keyboard_hook_proc` reads them on every event
    /// so the new config takes effect on the very next keystroke.
    pub fn apply_config(&self, config: HotkeyConfig) {
        self.shared.set_trigger_key(config.trigger_key);
        self.shared.set_trigger_mode(config.trigger_mode);
    }
}

impl Default for HotkeyListenerState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Apply a new hotkey configuration. Hot-swap — no hook reinstall.
///
/// Phase 1: `lib.rs` calls this directly; Phase M4 chunk 3 will route
/// through the unified `update_settings` command which fans out to all
/// state slots.
#[tauri::command]
pub async fn update_hotkey_config(
    state: tauri::State<'_, HotkeyListenerState>,
    config: HotkeyConfig,
) -> Result<(), HotkeyError> {
    state.apply_config(config);
    Ok(())
}

/// Phase 1 stub — Custom keycode recording is Phase 2 only. Returns
/// `NotImplemented` so the command surface stays stable while the UI
/// hides the corresponding button (per
/// `doc/plans/01-architecture.md` IPC contract row).
#[tauri::command]
pub async fn start_hotkey_recording() -> Result<(), HotkeyError> {
    Err(HotkeyError::NotImplemented)
}

/// Phase 1 stub — see `start_hotkey_recording`.
#[tauri::command]
pub async fn cancel_hotkey_recording() -> Result<(), HotkeyError> {
    Err(HotkeyError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_error_serializes_as_flat_string() {
        let err = HotkeyError::NotImplemented;
        let json = serde_json::to_string(&err).expect("serialize");
        assert_eq!(
            json,
            "\"Custom hotkey recording is not implemented in Phase 1\""
        );
    }

    #[test]
    fn hotkey_error_hook_install_failed_renders_inner() {
        let err = HotkeyError::HookInstallFailed(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        let json = serde_json::to_string(&err).expect("serialize");
        assert!(json.starts_with('"'));
        assert!(json.contains("denied"));
    }

    #[test]
    fn hotkey_event_payload_serializes_camel_case() {
        let p = HotkeyEventPayload {
            trigger_mode: "hold",
            action: "pressed",
        };
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "{\"triggerMode\":\"hold\",\"action\":\"pressed\"}");
    }

    #[test]
    fn hotkey_event_payload_toggle_serializes_camel_case() {
        let p = HotkeyEventPayload {
            trigger_mode: "toggle",
            action: "toggled-on",
        };
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(
            json,
            "{\"triggerMode\":\"toggle\",\"action\":\"toggled-on\"}"
        );
    }

    #[test]
    fn state_starts_with_default_config() {
        let state = HotkeyListenerState::new();
        assert_eq!(state.shared.current_trigger_key(), TriggerKey::RightAlt);
        assert_eq!(state.shared.current_trigger_mode(), TriggerMode::Hold);
    }

    #[test]
    fn apply_config_hot_swaps_atomics() {
        let state = HotkeyListenerState::new();
        state.apply_config(HotkeyConfig {
            trigger_key: TriggerKey::LeftControl,
            trigger_mode: TriggerMode::Toggle,
        });
        assert_eq!(state.shared.current_trigger_key(), TriggerKey::LeftControl);
        assert_eq!(state.shared.current_trigger_mode(), TriggerMode::Toggle);
    }

    #[test]
    fn shutdown_is_idempotent_when_uninstalled() {
        // Without an install, shutdown should be a clean no-op (the inner
        // Mutex<Option> is None). Important for early-error paths in lib.rs
        // that may call shutdown without ever having installed.
        let state = HotkeyListenerState::new();
        state.shutdown();
        // Calling again should still be fine.
        state.shutdown();
    }

    #[test]
    fn event_constants_match_architecture_doc() {
        // doc/plans/01-architecture.md "Tauri Events" rows:
        assert_eq!(HOTKEY_PRESSED, "hotkey:pressed");
        assert_eq!(HOTKEY_RELEASED, "hotkey:released");
        assert_eq!(HOTKEY_TOGGLED, "hotkey:toggled");
        assert_eq!(EVENT_ESCAPE_PRESSED, "escape:pressed");
    }
}
