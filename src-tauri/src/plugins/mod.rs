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
// Subsequent milestones add `audio_control`, `clipboard_paste`,
// `hotkey_listener`, `llm_polish`, etc.

pub mod audio_recorder;
pub mod credentials;
pub mod transcription;
