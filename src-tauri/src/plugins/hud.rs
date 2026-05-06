// HUD positioning + dev-visibility commands (M5).
//
// `position_hud_for_active_monitor` centers the 380x56 HUD on the monitor
// containing the OS cursor (logical y=50 from monitor top). It is invoked
// from the Vue voice-flow store's `handleStart` BEFORE recording starts so
// the HUD appears near the user's current focus rather than always on the
// primary monitor. Per challenger P1-8, callers wrap this in try/catch —
// positioning failure must not block recording (decorative vs core).
//
// `set_hud_visible_for_dev` is a debug-only helper: it shows/hides the HUD
// window directly so a developer can visually inspect the placeholder /
// idle state from the Vue devtools console without going through the
// real recording flow. Per challenger P1-6, it is gated by both
// `#[cfg(debug_assertions)]` on the function AND a two-path
// `generate_handler!` in lib.rs so the symbol is stripped from release
// builds entirely (an IPC call from a release build returns an unknown-
// command error rather than executing a debug-only path).
//
// Tauri 2 stable does not expose a cross-platform cursor-position API on
// Manager (challenger P0-2 caught this in the original spec draft). We
// therefore call the raw Win32 `GetCursorPos` directly. Phase 2 macOS port
// will need an `#[cfg(target_os = "macos")]` branch using Cocoa.
//
// Pure helpers (`pick_monitor`, `compute_centered_position`) take a
// `MonitorRect` plain struct rather than `tauri::Monitor` so they are
// cargo-testable without standing up a full Tauri runtime.

use serde::{Serialize, Serializer};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};
use thiserror::Error;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::POINT;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

const HUD_LABEL: &str = "main";
/// HUD window logical size — must stay in sync with `tauri.conf.json` `app.windows`
/// entry for label="main". Centering math uses this to compute the left edge.
const HUD_LOGICAL_SIZE: (f64, f64) = (380.0, 56.0);
/// Vertical inset from the monitor top edge (logical px). Matches the spec §3
/// "y = 50 logical" placement so the bubble sits just below the title-bar zone
/// of typical full-screen apps.
const HUD_Y_OFFSET_LOGICAL: f64 = 50.0;

/// Errors returned by the HUD Tauri commands. Manual `Serialize` impl below
/// flattens to a string at the IPC boundary per CLAUDE.md "Error enum 手動
/// implement Serialize 為 string" — same pattern as `AudioRecorderError` /
/// `TranscriptionError` / `ClipboardPasteError`.
#[derive(Error, Debug)]
pub enum HudError {
    /// `app.get_webview_window("main")` returned None — should be impossible
    /// in normal operation since `tauri.conf.json` declares the HUD window,
    /// but we surface it instead of panicking.
    #[error("HUD window not found")]
    WindowMissing,

    /// `WebviewWindow::available_monitors()` failed or returned an empty list.
    /// Tauri-internal error string is preserved for debugging.
    #[error("Failed to enumerate monitors: {0}")]
    MonitorEnumerationFailed(String),

    /// `WebviewWindow::set_position()` failed at the OS layer (e.g. window
    /// already destroyed by the time the command lands).
    #[error("Failed to set HUD position: {0}")]
    SetPositionFailed(String),

    /// Win32 `GetCursorPos` failed — typically signals a session-isolation
    /// boundary (UAC elevation, secure desktop) where the calling process
    /// can't query desktop input state.
    #[error("Failed to query cursor position: {0}")]
    CursorQueryFailed(String),

    /// `WebviewWindow::show()` / `hide()` failed (used by
    /// `set_hud_visible_for_dev`). Surfaces the underlying tauri error.
    #[error("HUD window operation failed: {0}")]
    WindowOpFailed(String),
}

// Manual `Serialize` so the frontend receives a flat string rather than a
// tagged enum object. Mirrors `AudioRecorderError` / `TranscriptionError`.
impl Serialize for HudError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Plain-data view of a monitor's geometry for the pure helpers below.
/// Constructing a `tauri::Monitor` outside a running Tauri runtime is
/// impractical, so we adapt to this struct in `position_hud_for_active_monitor`
/// and the unit tests build instances directly.
#[derive(Debug, Clone)]
pub struct MonitorRect {
    /// Top-left corner in physical (device) pixels — Tauri's `Monitor::position()` value.
    pub position: PhysicalPosition<i32>,
    /// Width / height in physical pixels — Tauri's `Monitor::size()` value.
    pub size: PhysicalSize<u32>,
    /// DPI scale factor; physical = logical * scale_factor.
    pub scale_factor: f64,
}

impl From<&tauri::Monitor> for MonitorRect {
    fn from(m: &tauri::Monitor) -> Self {
        Self {
            position: *m.position(),
            size: *m.size(),
            scale_factor: m.scale_factor(),
        }
    }
}

/// Choose the monitor containing the cursor; fall back to the first monitor
/// if the cursor is outside every rect (rare but possible during display
/// hot-plug or with HiDPI rounding edges). Caller guarantees the slice is
/// non-empty — `position_hud_for_active_monitor` enforces this with a
/// `MonitorEnumerationFailed("no monitors")` early return.
pub fn pick_monitor(monitors: &[MonitorRect], cursor: (i32, i32)) -> &MonitorRect {
    monitors
        .iter()
        .find(|m| {
            let x_in = cursor.0 >= m.position.x && cursor.0 < m.position.x + m.size.width as i32;
            let y_in = cursor.1 >= m.position.y && cursor.1 < m.position.y + m.size.height as i32;
            x_in && y_in
        })
        .unwrap_or(&monitors[0])
}

/// Compute the physical top-left position to place a logically-sized HUD
/// centered horizontally on `monitor`, with `y_offset_logical` from the
/// monitor's top edge. DPI-aware: logical inputs are scaled to physical
/// pixels using the monitor's `scale_factor`.
pub fn compute_centered_position(
    monitor: &MonitorRect,
    hud_logical: (f64, f64),
    y_offset_logical: f64,
) -> PhysicalPosition<f64> {
    let hud_physical_w = hud_logical.0 * monitor.scale_factor;
    let centered_x = monitor.position.x as f64 + (monitor.size.width as f64 - hud_physical_w) / 2.0;
    let y = monitor.position.y as f64 + y_offset_logical * monitor.scale_factor;
    PhysicalPosition::new(centered_x, y)
}

/// Query the OS cursor position in physical screen coordinates. Tauri 2
/// stable does not expose this; we go straight to Win32. Non-Windows builds
/// return an error (Phase 2 macOS port will replace with a Cocoa branch).
#[cfg(target_os = "windows")]
fn get_cursor_position() -> Result<(i32, i32), HudError> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is a valid stack-allocated POINT for the duration of
    // the call. `GetCursorPos` writes to it on success and returns Err on
    // failure (e.g. secure desktop). No invariants are violated here.
    unsafe {
        GetCursorPos(&mut point).map_err(|e| HudError::CursorQueryFailed(e.to_string()))?;
    }
    Ok((point.x, point.y))
}

#[cfg(not(target_os = "windows"))]
fn get_cursor_position() -> Result<(i32, i32), HudError> {
    Err(HudError::CursorQueryFailed(
        "cursor query not implemented on this platform".to_string(),
    ))
}

/// Tauri command: position the HUD window centered on the monitor containing
/// the OS cursor, at `y = 50` logical pixels from that monitor's top edge.
/// Idempotent — safe to call repeatedly. Per challenger P1-8 the caller wraps
/// in try/catch; positioning failure must not block recording.
#[tauri::command]
pub async fn position_hud_for_active_monitor(app: AppHandle) -> Result<(), HudError> {
    let hud = app
        .get_webview_window(HUD_LABEL)
        .ok_or(HudError::WindowMissing)?;
    let cursor = get_cursor_position()?;
    let tauri_monitors = hud
        .available_monitors()
        .map_err(|e| HudError::MonitorEnumerationFailed(e.to_string()))?;
    let monitors: Vec<MonitorRect> = tauri_monitors.iter().map(MonitorRect::from).collect();
    if monitors.is_empty() {
        return Err(HudError::MonitorEnumerationFailed(
            "no monitors available".to_string(),
        ));
    }
    let target = pick_monitor(&monitors, cursor);
    let position = compute_centered_position(target, HUD_LOGICAL_SIZE, HUD_Y_OFFSET_LOGICAL);
    hud.set_position(tauri::Position::Physical(PhysicalPosition::new(
        position.x as i32,
        position.y as i32,
    )))
    .map_err(|e| HudError::SetPositionFailed(e.to_string()))?;
    Ok(())
}

/// Debug-only tooling: show/hide the HUD window directly. Released builds
/// strip this entirely via the two-path `generate_handler!` in `lib.rs`
/// (challenger P1-6 — `#[cfg(debug_assertions)]` inside a single
/// `generate_handler!` invocation can collide with the macro expansion).
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn set_hud_visible_for_dev(app: AppHandle, visible: bool) -> Result<(), HudError> {
    let hud = app
        .get_webview_window(HUD_LABEL)
        .ok_or(HudError::WindowMissing)?;
    if visible {
        hud.show()
            .map_err(|e| HudError::WindowOpFailed(e.to_string()))?;
    } else {
        hud.hide()
            .map_err(|e| HudError::WindowOpFailed(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32, scale: f64) -> MonitorRect {
        MonitorRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(w, h),
            scale_factor: scale,
        }
    }

    #[test]
    fn pick_monitor_returns_monitor_containing_cursor() {
        let monitors = vec![rect(0, 0, 1920, 1080, 1.0), rect(1920, 0, 2560, 1440, 1.0)];
        // Cursor at (3000, 500) is inside monitor B (x in [1920, 4480), y in [0, 1440))
        let chosen = pick_monitor(&monitors, (3000, 500));
        assert_eq!(chosen.position.x, 1920);
        assert_eq!(chosen.size.width, 2560);
    }

    #[test]
    fn pick_monitor_falls_back_to_first_when_cursor_outside_all() {
        let monitors = vec![rect(0, 0, 1920, 1080, 1.0), rect(1920, 0, 2560, 1440, 1.0)];
        // Cursor at (-100, -100) is outside both — fall back to first.
        let chosen = pick_monitor(&monitors, (-100, -100));
        assert_eq!(chosen.position.x, 0);
        assert_eq!(chosen.size.width, 1920);
    }

    #[test]
    fn compute_centered_position_centers_on_monitor() {
        // 1920x1080 @ scale 1.0, HUD 380x56 logical, y_offset = 50.
        // Expected x = 0 + (1920 - 380*1.0) / 2 = 770
        // Expected y = 0 + 50 * 1.0 = 50
        let monitor = rect(0, 0, 1920, 1080, 1.0);
        let pos = compute_centered_position(&monitor, (380.0, 56.0), 50.0);
        assert_eq!(pos.x, 770.0);
        assert_eq!(pos.y, 50.0);
    }

    #[test]
    fn compute_centered_position_handles_dpi_scale() {
        // 3840x2160 monitor at (0, 0) with scale 2.0 (Windows "200%" mode).
        // HUD 380x56 logical → physical width 380 * 2 = 760.
        // Expected x = 0 + (3840 - 760) / 2 = 1540
        // Expected y = 0 + 50 * 2 = 100
        let monitor = rect(0, 0, 3840, 2160, 2.0);
        let pos = compute_centered_position(&monitor, (380.0, 56.0), 50.0);
        assert_eq!(pos.x, 1540.0);
        assert_eq!(pos.y, 100.0);
    }

    #[test]
    fn pick_monitor_handles_negative_origin_layout() {
        // User has secondary monitor LEFT of primary (Windows lets you do this).
        // Primary at (0, 0), secondary at (-1920, 0).
        let monitors = vec![rect(0, 0, 1920, 1080, 1.0), rect(-1920, 0, 1920, 1080, 1.0)];
        // Cursor on secondary
        let chosen = pick_monitor(&monitors, (-1000, 500));
        assert_eq!(chosen.position.x, -1920);
    }

    #[test]
    fn hud_error_serializes_as_flat_string() {
        let err = HudError::WindowMissing;
        let json = serde_json::to_string(&err).expect("serialize");
        assert_eq!(json, "\"HUD window not found\"");
    }
}
