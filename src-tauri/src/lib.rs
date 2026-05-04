// TalkType Tauri orchestrator (lib.rs).
//
// Phase 1 progress:
//   - M1 (basic IPC + dual window) — done
//   - M2 (audio recorder pipeline) — done; 11 audio_recorder commands wired below
//   - M3 chunk 1 (credentials) — wires `CredentialsState` + 3 commands
//     (set / delete / has). `get_credential` stays Rust-only and is consumed
//     by `transcription` (chunk 2) + `llm_polish` (M6).
//   - M3 chunk 2 (transcription dispatcher + Groq cloud) — wires
//     `TranscriptionState` + `transcribe_audio` command. The dispatcher reads
//     keyring directly via `credentials::get_credential` and emits
//     `transcription:completed` for both windows on success.
//   - M3 chunk 3 (Q5 connectivity health + UX surfaces) — adds
//     `test_provider_connection` (Groq `/models` GET) so the Settings page
//     can verify a freshly-saved key works without burning a transcription
//     quota slot. Future M6 extends to other 3 providers.
//
// Window layout: HUD (`main`, transparent overlay) + Dashboard (`main-window`).
// Tray icon with "Open Dashboard" + "Quit" menu items, left-click focuses
// Dashboard. Single-instance plugin so a second launch refocuses the Dashboard.
// Dashboard close-request is intercepted -> hide instead of destroy (only tray
// "Quit" exits).
//
// Future modules (settings.rs, additional plugins) will be added in subsequent
// milestones; this file aims for the ~300-line orchestrator budget per
// doc/plans/03-rust-modules.md.

pub mod plugins;

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

use plugins::{audio_recorder, credentials, transcription};

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
        .manage(audio_recorder::AudioRecorderState::new())
        .manage(audio_recorder::AudioPreviewState::new())
        .manage(credentials::CredentialsState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray_icon(&handle)?;
            // `TranscriptionState::new()` is fallible (reqwest client builder
            // can fail on TLS init). Doing it here lets the error propagate
            // through the `setup` Result chain.
            app.manage(transcription::TranscriptionState::new()?);
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
        .invoke_handler(tauri::generate_handler![
            ping,
            audio_recorder::commands::start_recording,
            audio_recorder::commands::stop_recording,
            audio_recorder::commands::clear_recording_buffer,
            audio_recorder::commands::list_audio_input_devices,
            audio_recorder::commands::get_default_input_device_name,
            audio_recorder::preview::start_audio_preview,
            audio_recorder::preview::stop_audio_preview,
            audio_recorder::files::save_recording_file,
            audio_recorder::files::read_recording_file,
            audio_recorder::files::delete_recording,
            audio_recorder::files::delete_all_recordings,
            audio_recorder::files::cleanup_old_recordings,
            // M3 chunk-1: credentials. NOTE: `get_credential` is intentionally
            // NOT registered here — it is `pub(crate)` and called from
            // transcription / llm_polish modules in Rust only. Architecture
            // invariant #1: API key never crosses the IPC boundary.
            credentials::set_credential,
            credentials::delete_credential,
            credentials::has_credential,
            // M3 chunk-2: transcription. `transcribe_audio` is the single
            // frontend entry point; M7 will keep the same command and route
            // internally to local whisper.cpp when settings select it.
            transcription::transcribe_audio,
            // M3 chunk-3 (Q5): provider connectivity health check. M3 ships
            // Groq; M6 will extend the same command to OpenAI / Anthropic
            // / Gemini by adding match arms in `transcription/health.rs`.
            transcription::health::test_provider_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
