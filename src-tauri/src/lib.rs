// TalkType Tauri orchestrator (lib.rs).
//
// Phase 1, M1 (基礎 IPC + 雙視窗):
//   - Two windows: HUD (`main`, transparent overlay) + Dashboard (`main-window`).
//   - Tray icon with "Open Dashboard" + "Quit" menu items, left-click focuses Dashboard.
//   - Single-instance plugin so a second launch refocuses the Dashboard.
//   - Single Tauri command (`ping`) emits a `ipc:pong` event for cross-window IPC smoke test.
//   - Dashboard close-request is intercepted -> hide instead of destroy (only tray "Quit" exits).
//
// Future modules (settings.rs, plugins/*) will be added in subsequent milestones; this file
// keeps a tight ~150-line orchestrator budget per doc/plans/03-rust-modules.md.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

// ─── Constants ─────────────────────────────────────────────────────────────

// HUD_LABEL is referenced from M5 onwards (HudOverlay show/hide). Keep it
// declared here as the source-of-truth window label even though M1 doesn't
// touch the HUD programmatically.
#[allow(dead_code)]
const HUD_LABEL: &str = "main";
const DASHBOARD_LABEL: &str = "main-window";
const EVENT_PONG: &str = "ipc:pong";
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

// ─── IPC payloads ──────────────────────────────────────────────────────────

/// Payload emitted on `ipc:pong`. Mirrors `PongPayload` in
/// `src/types/events.ts` — keep the camelCase serde rename in sync with the
/// frontend type. See doc/plans/01-architecture.md "Invariants" rule #8.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PongPayload {
    source: String,
    timestamp_ms: u64,
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Smoke-test command for the dual-window IPC. Frontend invokes `ping` with an
/// optional `source` (window label); Rust replies by emitting `ipc:pong`
/// globally so both HUD and Dashboard can observe it.
///
/// Uses `Result<(), String>` (not a thiserror enum) because the only failure
/// mode is `Emitter::emit` returning a serialization/transport error and
/// flattening to a string is sufficient for this minimal contract. Error
/// enums are introduced in M2+ when modules with richer failure modes arrive.
#[tauri::command]
async fn ping(app: AppHandle, source: Option<String>) -> Result<(), String> {
    let payload = PongPayload {
        source: source.unwrap_or_else(|| "unknown".to_string()),
        timestamp_ms: now_ms(),
    };
    app.emit(EVENT_PONG, payload).map_err(|e| e.to_string())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Reveal and focus the Dashboard window (used by tray click, tray menu, and
/// the single-instance handler).
fn focus_dashboard(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        eprintln!("[lib] focus_dashboard: window '{DASHBOARD_LABEL}' not found");
    }
}

fn build_tray_icon(app: &AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open_dashboard", "Open Dashboard", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &quit_item])?;

    let icon = Image::from_bytes(TRAY_ICON_BYTES)?;

    TrayIconBuilder::with_id("tray")
        .tooltip("TalkType")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_dashboard" => focus_dashboard(app),
            "quit" => app.exit(0),
            other => eprintln!("[lib] tray menu: unhandled id '{other}'"),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_dashboard(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

// ─── Entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Single-instance plugin must be registered FIRST per architecture invariant
    // (doc/plans/01-architecture.md "Invariants" rule #10). Desktop only —
    // single-instance does not exist on Android/iOS.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            eprintln!("[lib] second instance detected — focusing Dashboard");
            focus_dashboard(app);
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray_icon(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Dashboard close-request: hide instead of destroying so the user can
            // reopen via tray. HUD has no decorations so it never receives
            // user-driven close events. Real exit flows through the tray "Quit"
            // menu (`app.exit(0)`) which bypasses this handler.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == DASHBOARD_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
