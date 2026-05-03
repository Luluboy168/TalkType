// Top-level plugin modules for TalkType.
//
// Each plugin owns one cross-cutting concern (audio capture, hotkeys, paste
// injection, etc.) and exposes Tauri commands + state structs registered by
// `lib.rs`. See `doc/plans/03-rust-modules.md` for the full plugin roadmap.
//
// M2 (current chunk): only `audio_recorder` exists. Subsequent milestones add
// `audio_control`, `clipboard_paste`, `hotkey_listener`, etc.

pub mod audio_recorder;
