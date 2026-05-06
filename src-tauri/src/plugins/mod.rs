// Top-level plugin modules for TalkType.
//
// Each plugin owns one cross-cutting concern (audio capture, hotkeys, paste
// injection, etc.) and exposes Tauri commands + state structs registered by
// `lib.rs`. See `doc/plans/03-rust-modules.md` for the full plugin roadmap.
//
// Status:
//   * `audio_recorder` — M2 + M3 chunk-0 (recording / preview / files).
//   * `credentials`    — M3 chunk-1 (OS keyring; API key storage). M3 chunk-2
//                         transcription imports the `pub(crate)`
//                         `get_credential` from this module directly.
//   * `transcription`  — M3 chunk-2 (Groq cloud Whisper dispatcher). M7 will
//                         add the local whisper.cpp branch behind the same
//                         `transcribe_audio` Tauri command.
//
//   * `clipboard_paste` — M4 chunk-2 (Windows paste pipeline). Module is
//                         declared here so chunks 1 + 2 can run in parallel
//                         without racing on this file; the implementation
//                         lives in `clipboard_paste/{mod, paste}.rs`.
//   * `hotkey_listener` — M4 chunk-1 (global hotkey via SetWindowsHookExW).
//                         Same parallel-dispatch reason; implementation lives
//                         in `hotkey_listener/{mod, windows, shared, types}.rs`.
//   * `hud`             — M5 chunk-0 (HUD positioning + dev visibility tooling).
//                         `position_hud_for_active_monitor` centers the HUD
//                         on the cursor's monitor; `set_hud_visible_for_dev`
//                         is debug-only.
//
// Subsequent milestones add `audio_control`, `llm_polish`, etc.

pub mod audio_recorder;
pub mod clipboard_paste;
pub mod credentials;
pub mod hotkey_listener;
pub mod hud;
pub mod transcription;
