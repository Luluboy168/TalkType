# SayIt Rust/Tauri 後端分析報告

> **來源**：Opus subagent (general-purpose) 對 `C:\Users\lulub\Downloads\SayIt\src-tauri\` 的深度分析
> **日期**：2026-05-02
> **分析範圍**：Rust 程式碼、Tauri 設定、Cargo dependencies（不含前端、CI、測試）

## 1. Overall Rust Architecture

SayIt 是 Tauri v2 desktop application（`com.sayit.app`、version 0.9.5），其 Rust 後端是 thin orchestrator (`lib.rs`) 加一個 platform-bridging plugin modules 目錄 (`src/plugins/*`) 的結構。Crate 在 `src-tauri/Cargo.toml` 設定三種 crate types (`["staticlib", "cdylib", "rlib"]`)，可以連結到 desktop binaries 與未來 mobile entry point。

Entry point 是 `src-tauri/src/main.rs`：

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() { sayit_lib::run() }
```

`windows_subsystem = "windows"` attribute 在 Windows release build 隱藏 console window。所有真正邏輯都在 `lib.rs::run()`：

1. 條件式初始化 Sentry
2. 建構 `tauri::Builder`、註冊 official plugins、in-tree `hotkey-listener` plugin、每個 `#[tauri::command]`、與一個大的 `setup` closure
3. 接 `on_window_event` 來抑制 `main-window` 的 destruction
4. 處理 `RunEvent::Exit` 對 stateful subsystems 做 explicit、ordered graceful shutdown

Module organization (`src/plugins/mod.rs`)：

```rust
pub mod audio_control;
pub mod audio_recorder;
pub mod clipboard_paste;
pub mod hotkey_listener;
pub mod keyboard_monitor;
pub mod sound_feedback;
pub mod text_field_reader;
pub mod transcription;
```

每個 module 跟同樣的形狀：optional state struct（透過 `app.manage` 管理）、platform-specific `mod macos { ... }` 與 `mod windows { ... }`（或 `mod windows_audio` / `mod windows_hook` / `mod windows_sound`）、platform-agnostic wrappers，然後是 `#[tauri::command]` functions exposed to frontend。只有 `hotkey_listener.rs` 真正包成 `tauri::plugin::TauriPlugin`（用 `Builder::new("hotkey-listener").setup(...).build()`）；其他都是 commands-and-state collections 直接在 `lib.rs` 註冊。

## 2. Cargo Dependencies

從 `src-tauri/Cargo.toml`：

**Build：**

- `tauri-build = "2"` — code generation for `tauri::generate_handler!` and `tauri::generate_context!`

**Core Tauri stack：**

- `tauri = "2"` with features `tray-icon`, `macos-private-api`, `image-png`, `protocol-asset`。macOS private API 是為 notch-overlay window level via `ns_window()`。
- `tauri-plugin-shell` — `shell:allow-open` permission 開啟 URL（例如 Accessibility settings）
- `tauri-plugin-http` — frontend HTTPS calls to LLM providers (Groq/OpenAI/Anthropic/Gemini)
- `tauri-plugin-sql = "2.3.1"` with `sqlite` feature — SQLite for transcription history
- `tauri-plugin-store = "~2.4"` — persistent JSON KV store for settings（規則明確禁止 SQLite 存 API keys；必須在這個 store）
- `tauri-plugin-autostart = "2.5.1"` — boot-on-login（用 `MacosLauncher::LaunchAgent`）
- `tauri-plugin-updater = "~2.10.0"` — minisign-signed updater pulling from GitHub Releases
- `tauri-plugin-process` — used by frontend to relaunch
- `tauri-plugin-single-instance` — 防止多 instance；第二次啟動會 focus existing main window via init 的 closure

**Serde / errors：**

- `serde = "1"` (`derive` feature)、`serde_json = "1"` — IPC (de)serialization
- `thiserror = "2"` — typed errors in `audio_recorder`、`clipboard_paste`、`transcription`。每個 error enum 都手動 implement `serde::Serialize` 為 string，frontend 永遠收到 flat error string

**Audio / DSP：**

- `cpal = "0.15"` — cross-platform audio capture for both recording and live preview
- `hound = "3.5"` — in-memory WAV encoding (RIFF header + 16-bit PCM mono)
- `rustfft = "6"` — FFT for the 6-bar HUD waveform visualization (`FFT_SIZE = 64`)

**Networking：**

- `reqwest = "0.12"` with `multipart` and `json` — Groq Whisper multipart upload

**Clipboard / paste：**

- `arboard = "3"` — cross-platform clipboard read/write

**Telemetry：**

- `sentry = "0.46"` — only initialized when `SENTRY_ENVIRONMENT == "production"` and `SENTRY_DSN` present and not a build-time placeholder

**macOS-only deps：**

- `core-graphics = "0.24"` — `CGEventTap`、`CGEvent`、`CGEventSource`、`CGEventFlags` for input simulation and listening
- `core-foundation = "0.10"` — `CFRunLoop`、`CFString`、`CFDictionary`、`CFURL`、`CFRelease`
- `objc = "0.2"` — `msg_send!` macro for `setLevel:`、`setCollectionBehavior:`、`setMovable:` on the HUD `NSWindow`

加上 `framework=CoreAudio` 從 `build.rs` 發出的 link line 與 direct `extern "C"` bindings to AudioToolbox / AX APIs。

**Windows-only deps：**

- `windows = "0.61"` with features `Win32_Foundation`、`Win32_UI_WindowsAndMessaging`、`Win32_UI_Input_KeyboardAndMouse`、`Win32_Media_Audio`、`Win32_Media_Audio_Endpoints`、`Win32_System_Com`、`Win32_System_Threading`。Cover `SetWindowsHookExW` + `KBDLLHOOKSTRUCT` for low-level keyboard hooks、`SendInput` for paste injection、WASAPI (`IMMDeviceEnumerator`、`IAudioEndpointVolume`) for system mute、`PlaySoundA` for sound feedback、`AttachThreadInput` for foreground-window restoration、`GetCursorPos` for HUD positioning

**Profiles：**

- `[profile.dev] incremental = true`
- `[profile.release] panic = "abort"; codegen-units = 1; lto = true; opt-level = "s"; strip = true;` — size-optimized

## 3. lib.rs / main.rs — Initialization

`lib.rs::run()` 是 orchestrator。重要部分：

**Sentry gating** (`get_sentry_dsn`、`get_sentry_environment`、`get_sentry_release`、`is_sentry_enabled`)：全都透過 `option_env!` 讀取，DSN 在 compile time bake 進去。Empty values 或開頭是 `"__"` (CI placeholders) 的 values 會被過濾。預設 release tag 是 `concat!("sayit@", env!("CARGO_PKG_VERSION"))`，與 frontend 共用 per CLAUDE.md release rules。

**Plugin chain**（順序：plugin 接收 setup callback 順序相同）：

```rust
.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| { show_main_window(app); }))
.plugin(tauri_plugin_shell::init())
.plugin(tauri_plugin_http::init())
.plugin(tauri_plugin_sql::Builder::default().build())
.plugin(tauri_plugin_store::Builder::default().build())
.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())
.plugin(plugins::hotkey_listener::init())
```

`single_instance` callback 在第二次啟動嘗試時 re-show `main-window`，把 app 當作 "background daemon with one Dashboard"。

**`invoke_handler`** 註冊 ~35 commands 從 `lib.rs` 加上所有 plugin modules — 完整列表在 section 5。

**`.setup(...)`** 在 main thread 上做：

1. `app.manage(...)` for every shared state struct: `KeyboardMonitorState`、`AudioControlState`、`FocusState`、`AudioRecorderState`、`AudioPreviewState`、`TranscriptionState`。（`HotkeyListenerState` 在自己 plugin 的 `setup` 內 manage。）
2. 從 embedded PNG (`include_bytes!("../icons/tray-icon.png")`) 建 tray icon，with `icon_as_template(true)` (macOS auto-tints)、wire 2-item menu (`open-dashboard`、`quit`)、`show_menu_on_left_click(true)`。
3. 對 `main` (HUD) 呼叫 `configure_macos_notch_window` 或 `configure_windows_topmost_window`。
4. 用 `calculate_centered_window_x` 把 HUD 在當前 monitor 水平置中、y=0。

**`.on_window_event(...)`** 攔截 `main-window` Dashboard 的 `WindowEvent::CloseRequested`，呼叫 `api.prevent_close()` 與 `window.hide()`，X button 變成 hide 而不是 destroy。

**`.run(|app_handle, event| { ... })`** 處理兩個 events：

- `RunEvent::Reopen`（macOS dock click）→ re-show Dashboard
- `RunEvent::Exit` → 8-step graceful shutdown：
  1. Restore system audio mute state
  2. Stop audio preview thread
  3. Stop cpal recording thread (`AudioRecorderState::shutdown`)
  4. Cancel `KeyboardMonitorState`
  5. Stop hotkey CGEventTap
  6. `sleep(200ms)` 讓 background threads 完成 cleanup
  7. Flush Sentry queue with 2-second timeout
  8. 如果 `RESTART_REQUESTED` 設定，spawn 新 copy of `current_exe()`。然後直接呼叫 `_exit(0)`（不是 `std::process::exit`）來 bypass Tauri 內建 restart logic 與任何 pending atexit destructors

Notch-window configuration 是 `lib.rs` 中 platform-divergent 最多的部分：

- macOS：set `setLevel:27`（`NSMainMenuWindowLevel + 3`、配合 BoringNotch project）、`setCollectionBehavior` to `canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary` (= 1 | 16 | 64 | 256)、`setMovable: false`
- Windows：加 `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` 到 `GWL_EXSTYLE`（從 Alt+Tab/taskbar 隱藏、防止 focus stealing）、然後 `SetWindowPos` with `HWND_TOPMOST` 與 `SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED` 強制 topmost

`get_hud_target_position` 是不平凡的 multi-monitor positioning command。macOS 用 `CGEventCreate`/`CGEventGetLocation`（with `CgEventGuard` Drop guard for `CFRelease`）取得 cursor coords；Windows 用 `GetCursorPos`。Pure function `find_monitor_for_cursor` (cursor x、y、monitor list、is_macos flag) 處理 cross-DPI logical/physical conversion（macOS positions 是 physical pixels 需要 `/sf`；Windows positions 是 virtual screen）。回傳 logical coordinates 給 frontend 是為了繞過 `tao` bug — `set_outer_position` 用 source monitor 的 scale factor 而非 destination 的。整個 positioning module 有 14+ unit tests cover single、dual horizontal、dual vertical、mixed-DPI、portrait、boundary、negative-coord、與 fallback cases。

## 4. Plugin Modules

### 4.1 `audio_recorder.rs` (cpal capture + WAV + waveform FFT)

用 `cpal::default_host()`。`start_recording(app, state, device_name)`：

1. 早早 acquire recording lock (`Mutex<Option<RecordingHandle>>`) 來贏 `start_audio_preview` 的 race（comment 稱 "F1+F7 race fix"）
2. 如果有 preview running、呼叫 `stop_audio_preview_inner` 釋放 device
3. 建 `RecordingInner { samples: Mutex<Vec<i16>> (pre-allocated for 30 s @ 16 kHz ≈ 938 KB)、should_stop: AtomicBool }`
4. Spawn named thread `"audio-recorder"` 跑 `run_recording_thread`。Bounded `mpsc::channel` 同步回傳選的 sample rate 或 error

`run_recording_thread` 透過 `select_input_device` 選 device（preferred `default_input_device()` whenever user-named device matches，作為 cpal 0.15.3 macOS Arc-cycle leak 的 workaround），然後 `determine_input_config` prefer 16 kHz mono (`min_by_key((mono_penalty, channels))`) 並 fallback to `default_input_config()`。透過 `build_typed_input_stream::<T>` instantiated for all 10 sample formats (`I8/I16/I32/I64/U8/U16/U32/U64/F32/F64`) 建 cpal stream。

cpal callback 內，samples 轉 mono `f32`（average channels）、clamp、scale 到 `i16`、push 到 `inner.samples`，**並** feed 進 64-sample ring buffer。每 ~16 ms (60fps) 跑 FFT (`rustfft`)、6 bins (indices `[9, 4, 1, 2, 6, 12]` — picked to match 視覺 spread frontend wants) 透過 `normalize_db` (`-100 dB → 0.0`、`-20 dB → 1.0`) normalize、發送 `audio:waveform` with payload `WaveformPayload { levels: [f32; 6] }`。

`should_stop` flip 後，loop 呼叫 `stream.pause()`（explicit defense — cpal 的 macOS disconnect listener 有 Arc cycle 防止 `Drop` 到 `AudioOutputUnitStop`）然後 drop stream。"SECURITY" comment 明確說明 pause failure means mic 可能還活著 — 整份 codebase 唯一 security-flagged log line。

`stop_recording` join thread、in-memory encode WAV via `hound::WavWriter::new(&mut Cursor)` with `WavSpec { channels: 1, sample_rate, bits_per_sample: 16, sample_format: Int }`、計算 `peak_energy_level` 與 `rms_energy_level`、把 WAV bytes 存到 `state.wav_buffer: Mutex<Option<Vec<u8>>>` 給 `transcribe_audio` 消費、回傳 `StopRecordingResult { recording_duration_ms, peak_energy_level, rms_energy_level }`。

`AudioPreviewState` 跑 analogous 但較簡單的 loop 在 `"audio-preview"` thread。Accumulate `(sum_squares: f64, sample_count: usize)` 在 single `Mutex`（comment 標註早期 race "F4 fix"）、每 30 ms 發送 `audio:preview-level` with single `f32` (RMS → dB → normalized to `[0, 1]` over `-60 dB..-20 dB`)。

File-management commands 用 `app.path().app_data_dir().join("recordings")`：

- `save_recording_file(id) → String` — write cached WAV to `<app_data>/recordings/<id>.wav`
- `read_recording_file(id) → tauri::ipc::Response` — return raw bytes；CLAUDE.md 註明 macOS 上會經 JSON `number[]`、frontend 需要 `new Uint8Array(raw)`
- `delete_all_recordings() → u32` — remove 所有 `.wav` files
- `cleanup_old_recordings(days) → Vec<String>` — remove files older than `cutoff = SystemTime::now() - days * 86400 s` 並回傳 deleted IDs

`AudioRecorderError` 是 `thiserror::Error` with variants `NoInputDevice`、`InputConfig`、`BuildStream`、`PlayStream`、`NotRecording`、`WavEncode`、`LockPoisoned`，加上 manual `Serialize` to a string。

### 4.2 `audio_control.rs` (system mute/restore)

State 是 `Mutex<Option<bool>>` 表示 pre-mute mute state（`None` = no pending restore、`Some(was_muted)` = pending restore）。`mute_system_audio` 是 idempotent — 如果有 value 就回傳 Ok 不動 system。`restore_system_audio` 在 attempt restore **之前** clear state，所以 failure 不能 permanently strand user 在 mute。`AudioControlState::shutdown` 是 `RunEvent::Exit` 時的最後防線。

**macOS** (`mod macos`)：`extern "C"` bindings to `AudioObjectGetPropertyData` / `AudioObjectSetPropertyData`（從 CoreAudio link via `build.rs`）。FourCC constants inline 為 `u32` literals (`0x644F7574 'dOut'`、`0x6D757465 'mute'`、`0x6F757470 'outp'`、`0x676C6F62 'glob'`)。Flow：enumerate default output device → `AudioObjectGetPropertyData(KAudioDevicePropertyMute)` → `AudioObjectSetPropertyData`。

**Windows** (`mod windows_audio`)：WASAPI via `windows` crate。用 `ComGuard` RAII wrapper around `CoInitializeEx(COINIT_APARTMENTTHREADED)`；如果 `CoInitializeEx` 回傳 `RPC_E_CHANGED_MODE (0x80010106)`、guard 記錄 "don't uninit" 不會 kill 別人的 COM apartment。Flow：`CoCreateInstance(MMDeviceEnumerator)` → `GetDefaultAudioEndpoint(eRender, eConsole)` → `Activate::<IAudioEndpointVolume>` → `GetMute()` / `SetMute(muted, null_guid)`。

### 4.3 `clipboard_paste.rs` (clipboard + paste injection)

用 `arboard::Clipboard` 做 read/write。`paste_text(app, focus_state, text)`：

1. Write text to clipboard
2. Sleep 50 ms for clipboard sync
3. 呼叫 platform paste：
   - **macOS**：`simulate_paste_via_cgevent` — 建 *Private* `CGEventSource` (`CGEventSourceStateID::Private`) 不繼承 physical-keyboard modifier state（comment 解釋這修復 Toggle-mode bug — held right-Option 產生 double pastes）、然後 post 4 events (Cmd↓、V↓、V↑、Cmd↑) 在 `CGEventTapLocation::Session`（也明確選 Session-level 來避免 macOS HID-pipeline duplicate-delivery bug）
   - **Windows**：先 `restore_target_window(saved_hwnd)` — 用 `AttachThreadInput(current_thread, target_thread, true)` + `SetForegroundWindow(target)` + detach 繞過 foreground-window restriction。然後 sleep 50 ms 呼叫 `simulate_paste_via_keyboard`、build 4 `INPUT` records 呼叫 `SendInput`。如果 SendInput 沒接受 4 個 records 回傳 error

`FocusState` 只在 Windows 存在：`target_hwnd: Mutex<isize>`。`capture_target_window(state)` 在 Windows 讀 `GetForegroundWindow()`（macOS no-op，因為 CGEvent process-global）。Frontend 在 hotkey fire 時且 HUD 取得 focus **之前** 呼叫這個。

`copy_to_clipboard(text)` 是 straight clipboard-write。

`capture_selected_text_via_clipboard()`（被 `text_field_reader::read_selected_text` 用）是不需要 Accessibility 的 universal selection-grabber：

1. Save current clipboard text
2. Set clipboard to `""` 為 sentinel
3. Simulate Cmd+C (macOS via 4 CGEvents) 或 Ctrl+C (Windows via SendInput)
4. Sleep 100 ms
5. Read back。如果 non-empty → that's the selection
6. Restore original clipboard

`ClipboardError` 是 `thiserror::Error { ClipboardAccess(String), KeyboardSimulation(String) }` with manual string `Serialize`。注意 `"🔴🔴🔴 [clipboard-paste] paste_text CALLED (#{})"` log with `AtomicU32` counter — 為 double-paste regressions debug instrumentation 留下的。

### 4.4 `hotkey_listener.rs` (~57 KB — 最大的 module)

這是唯一包成 `tauri::plugin::TauriPlugin` 的 module（`init() -> TauriPlugin<R>` calls `Builder::new("hotkey-listener").setup(...).build()`)。

**Public types：**

- `enum ModifierFlag { Command, Control, Option, Shift, Fn }` — `Hash + Eq` for `HashSet`
- `enum TriggerKey` (camelCase serde): `Fn`、`Option`、`RightOption`、`Command`、`RightAlt`、`LeftAlt`、`Control`、`RightControl`、`Shift`、`Custom { keycode: u16 }`、`Combo { modifiers: Vec<ModifierFlag>, keycode: u16 }`。Custom 與 Combo support 任意 keycodes
- `enum TriggerMode { Hold, Toggle }`

**Shared state：**

```rust
pub struct HotkeyListenerState {
    shared: Arc<Mutex<HotkeySharedState>>,  // trigger_key, trigger_mode, active_modifiers, double_tap, recording, toggle_long_press_fired
    is_pressed: Arc<AtomicBool>,
    is_toggled_on: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    run_loop_ref: Arc<Mutex<Option<core_foundation::runloop::CFRunLoop>>>,  // for shutdown / reinitialize
}
```

`HotkeyListenerState` 手動 implement `Clone`，可以 clone 進 event-tap closure 同時保持 manage。

**Detection logic：**

- `handle_key_event` 是 unified press/release dispatcher。Hold mode 偵測 double-tap（release → press within 350 ms after a tap < 300 ms long）發送 `hotkey:mode-toggle`；否則發送 `hotkey:pressed` / `hotkey:released`。Toggle mode XOR `is_toggled_on` 發送 `hotkey:toggled` with `HotkeyEventPayload { mode, action: Start | Stop }`。也 spawn 1000 ms long-press detector — 如果 key 還在 hold 就 fire `hotkey:mode-toggle`
- `matches_combo_trigger(keycode, combo_mods, combo_kc, active_mods)` 要求 non-empty combo modifiers、exact keycode、與 **exact** modifier set (`combo_modifiers.len() == active_mods.len()`) — extra modifiers reject。ESC (`53` mac / `0x1B` win) 是 reserved，永遠不允許作為 combo primary

**macOS implementation：**

- `check_accessibility_permission()` 呼叫 `AXIsProcessTrusted`。如果 false 在 plugin setup 時，呼叫 `AXIsProcessTrustedWithOptions` with `{"AXTrustedCheckOptionPrompt": true}` 觸發 system prompt
- `start_event_tap` spawn thread 呼叫 `CGEventTap::new` listen `FlagsChanged | KeyDown | KeyUp` 在 `CGEventTapLocation::Session` with `CGEventTapPlacement::HeadInsertEventTap` 與 `CGEventTapOptions::ListenOnly`。Closure hold state 並基於 `event_type` dispatch。Runloop handle stash 在 `run_loop_ref`，`shutdown()` 與 `reinitialize_hotkey_listener` 可以 stop
- Closure 順序處理三個 modes：(a) recording-mode capture (delegate to `handle_recording_event_macos`)、(b) Combo trigger matching、(c) single-key trigger matching。ESC 在 `KeyDown` 攔截、發送 `escape:pressed`
- Recording mode for capturing user-defined hotkeys 特殊處理 Fn — Fn 透過 `FlagsChanged` 而非 `KeyDown` 抵達（toggle-style — 第一次 sighting = pressed、第二次 = released）。Standard modifiers 從 `CGEventFlags` 透過 `extract_active_modifiers_macos` 抽出。Non-modifier `KeyDown` capture keycode + accumulated modifiers 發送 `hotkey:recording-captured`。Recording 中按 ESC 發送 `hotkey:recording-rejected { reason: "esc_reserved" }`

**Windows implementation：**

- `windows_hook::install` 把 `HookContext`（with `Box<dyn Fn>` callbacks for key/escape/recording）放進 `OnceLock<HookContext>` static，C-callable `hook_proc` 可以找到
- Spawn thread 呼叫 `SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)` 並 pump messages with `GetMessageW` + `TranslateMessage` + `DispatchMessageW` 直到 termination、然後 `UnhookWindowsHookEx`
- `hook_proc` 從 `lParam` 讀 `KBDLLHOOKSTRUCT`、從 `WM_KEYDOWN | WM_SYSKEYDOWN` 判斷 `is_key_down`、用同樣方式 dispatch recording → ESC → combo → single-key。Active modifiers 透過 `GetKeyState` (`is_vk_pressed(vk)` returns `(state & 0x8000) != 0`) 即時計算
- VK codes：`VK_LSHIFT = 0xA0`、`VK_LCONTROL = 0xA2`、`VK_RCONTROL = 0xA3`、`VK_LMENU = 0xA4`、`VK_RMENU = 0xA5`、`VK_LWIN = 0x5B`、`VK_RWIN = 0x5C`、`VK_ESCAPE = 0x1B`。Win key map to `ModifierFlag::Command` 保持 cross-platform mapping intuitive

**Platform default trigger key：** macOS = `TriggerKey::Fn`、Windows = `TriggerKey::RightAlt`、Linux = `TriggerKey::Control`。

**Commands**（all `#[tauri::command]`）：`check_accessibility_permission_command`、`open_accessibility_settings`（spawn `open x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`）、`reinitialize_hotkey_listener`（stop old tap、sleep 200 ms、reset state、restart）、`reset_hotkey_state`、`start_hotkey_recording`、`cancel_hotkey_recording`。`update_hotkey_config` command 在 `lib.rs` 但寫到這個 plugin 的 state。

### 4.5 `keyboard_monitor.rs` (post-paste UX feedback)

兩個獨立 monitor 共用一個 persistent CGEventTap / Windows hook：

- **Quality monitor** — paste 後 fire、watch 5 s、如果按 Backspace/Delete 設 `was_modified = true`（signal user 在修正轉錄、telemetry 用）。發送 `quality-monitor:result { wasModified }`
- **Correction monitor** — paste 後 fire、two-phase state machine：Phase 1 wait up to 5 s for any key；Phase 2 begin on first key、watch for Enter (send) 或 3 s idle (timeout) 或 15 s hard limit。Enter 有 500 ms debounce 因為 IME confirm 也會產生 Enter — 如果 debounce 中有新 key arriving、treat Enter as IME 並 continue。發送 `correction-monitor:result { anyKeyPressed, enterPressed, idleTimeout }`

Persistent tap 在 `KeyboardMonitorState::new()` create **once** 永不 destroy。Monitor 純粹透過 `AtomicBool` flags on/off。Comment 解釋此設計：之前 per-recording create/destroy tap 導致 "ghost Enter" issues (in-flight events)。

mac keycodes used: `BACKSPACE = 51`、`DELETE = 117`、`ENTER = 36`、`KEYPAD_ENTER = 76`。Win VK codes: `VK_BACK = 0x08`、`VK_DELETE = 0x2E`、`VK_RETURN = 0x0D`。Windows hook 用同樣 `OnceLock<HookState>` pattern。

`start_quality_monitor` 與 `start_correction_monitor` 是 public commands；都用 `wait_with_cancellation` 允許 re-start (cancel 前一個 monitor with 150 ms grace)。

### 4.6 `sound_feedback.rs` (system sound playback)

四個 commands：`play_start_sound`、`play_stop_sound`、`play_error_sound`、`play_learned_sound`。

**macOS**：`extern "C"` to AudioToolbox's `AudioServicesCreateSystemSoundID(CFURL) -> u32` 與 `AudioServicesPlaySystemSound(u32)`。Play built-in macOS sounds：`Funk.aiff`、`Bottle.aiff`、`Glass.aiff`、`Ping.aiff` from `/System/Library/Sounds/`。

**Windows**：`PlaySoundA` from `Win32::Media::Audio` with `SND_MEMORY | SND_ASYNC`。WAV bytes 在 compile time 透過 `include_bytes!("../../resources/sounds/start.wav")` 與 `stop.wav` embed。`learned_sound` reuse `start_sound`、`error_sound` reuse `stop_sound`（comment 說 "temporary"）。

### 4.7 `text_field_reader.rs` (focused field + selection reading)

兩個 commands：

- `read_focused_text_field() -> Result<Option<String>, String>` — macOS 讀 100-char excerpt around focused text element 的 cursor；Windows 是 no-op stub 回傳 `Ok(None)` (UI Automation 還沒實作)
- `read_selected_text() -> Result<Option<String>, String>` — delegate to `clipboard_paste::capture_selected_text_via_clipboard()` (cross-platform)

**macOS implementation** 透過 `extern "C"` 直接用 AX (Accessibility) C API：

- `AXUIElementCreateSystemWide()` → `AXFocusedApplication` → `AXFocusedUIElement` → check `AXRole` ∈ `{AXTextField, AXTextArea, AXComboBox, AXWebArea}`。`AXWebArea` 多 descend 一層（web pages 有 nested focus）
- 讀 `AXValue` for full text 與 `AXSelectedTextRange`（透過 `AXValueGetValue` with `kAXValueCFRangeType = 4` decode）for cursor position
- `extract_excerpt(text, cursor, 50)` 回傳 cursor 前後各 50 chars（用 `chars()` for proper CJK handling）；如果 cursor unknown、fallback to last 100 chars
- `FocusedElementContext` 有 `cleanup()` method `CFRelease` 每個 CFTypeRef 正確順序。沒有 `Drop` impl — 每個建構 context 的 code path 必須 explicit 呼叫 `cleanup()`。（稍微 footgunny 但 intentional given 在 `resolve_focused_text_element` 內 error paths 需要的 partial cleanup）

### 4.8 `transcription.rs` (Groq Whisper)

`TranscriptionState { client: reqwest::Client }` 在 `setup` 內 build once with 120-second timeout、provide connection pooling。

`transcribe_audio(state, transcription_state, api_key, vocabulary_term_list, model_id, language) -> Result<TranscriptionResult, TranscriptionError>`：

1. Validate `api_key` non-empty
2. **Consume** WAV from `AudioRecorderState::wav_buffer` via `take()` (single-shot)
3. Validate size：`>= 1000 bytes` 與 `<= 25 MB` (Groq free-tier limit)
4. Build `reqwest::multipart::Form` with `file: bytes (recording.wav, audio/wav)`、`model` (default `whisper-large-v3`)、`response_format: verbose_json`。Optional `language` (omit = auto-detect) 與 optional `prompt: "Important Vocabulary: term1, term2, ..."` (capped at 50 terms)
5. POST to `https://api.groq.com/openai/v1/audio/transcriptions` with `Authorization: Bearer <api_key>`
6. Parse verbose JSON `{ text, segments: [{ no_speech_prob }] }`。`no_speech_probability` 計算為 segments 的 **min**（comment 解釋：real speech 永遠有至少一個 low-NSP segment，所以 `min` 正確區分 noise vs speech）。如果無 segments → `1.0` (full silence)
7. 回傳 `TranscriptionResult { rawText, transcriptionDurationMs, noSpeechProbability }`

`retranscribe_from_file(file_path, ...)` 同樣 flow 但從 disk 透過 `std::fs::read` 讀 (synchronous、justified 因為 WAVs 通常很小)。

LLM-polishing step（產品描述提到的）**不在** Rust 後端 — per CLAUDE.md 在 frontend 透過 `tauri-plugin-http` 呼叫 OpenAI/Anthropic/Gemini。Rust 後端只做 Whisper transcription。

`TranscriptionError`：`NoAudioData`、`AudioTooSmall(usize)`、`FileTooLarge { size_mb, limit_mb }`、`ApiKeyMissing`、`RequestFailed`、`ApiError(u16, String)`、`ParseError`、`LockPoisoned` — `thiserror`-derived with manual string `Serialize`。

## 5. Tauri Command Surface

Full registration list from `lib.rs`：

| Command | Module | Signature highlights |
|---|---|---|
| `debug_log` | lib.rs | `(level: String, message: String)` — frontend log forwarding |
| `request_app_restart` | lib.rs | `(app: AppHandle)` — set atomic flag、exit |
| `update_hotkey_config` | lib.rs | `(app, trigger_key: TriggerKey, trigger_mode: TriggerMode) -> Result<(), String>` |
| `get_hud_target_position` | lib.rs | `(app: AppHandle) -> Result<HudTargetPosition, String>` (camelCase serde) |
| `mute_system_audio` / `restore_system_audio` | audio_control | `(state: State<AudioControlState>) -> Result<(), String>` |
| `capture_target_window` | clipboard_paste | `(state: State<FocusState>)` — Windows-only effect |
| `copy_to_clipboard` | clipboard_paste | `(text: String) -> Result<(), ClipboardError>` |
| `paste_text` | clipboard_paste | `(app, focus_state, text: String) -> Result<(), ClipboardError>` |
| `check_accessibility_permission_command` | hotkey_listener | `() -> bool` |
| `open_accessibility_settings` | hotkey_listener | `() -> Result<(), String>` |
| `reinitialize_hotkey_listener` | hotkey_listener | `(app: AppHandle) -> Result<(), String>` |
| `reset_hotkey_state` | hotkey_listener | `(state)` |
| `start_hotkey_recording` / `cancel_hotkey_recording` | hotkey_listener | `(state)` |
| `start_quality_monitor` / `start_correction_monitor` | keyboard_monitor | `(app: AppHandle)` |
| `read_focused_text_field` | text_field_reader | `() -> Result<Option<String>, String>` |
| `read_selected_text` | text_field_reader | `() -> Result<Option<String>, String>` |
| `get_default_input_device_name` | audio_recorder | `() -> Option<String>` |
| `list_audio_input_devices` | audio_recorder | `() -> Vec<AudioInputDeviceInfo>` |
| `start_audio_preview` / `stop_audio_preview` | audio_recorder | `(app, state, device_name)` |
| `start_recording` / `stop_recording` | audio_recorder | returns `Result<…, AudioRecorderError>` |
| `save_recording_file` / `read_recording_file` | audio_recorder | `(id, app, state)` — read returns `tauri::ipc::Response` (raw bytes) |
| `delete_all_recordings` / `cleanup_old_recordings` | audio_recorder | `(app)` / `(days, app)` |
| `transcribe_audio` / `retranscribe_from_file` | transcription | async、回傳 `Result<TranscriptionResult, TranscriptionError>` |
| `play_start_sound` / `play_stop_sound` / `play_error_sound` / `play_learned_sound` | sound_feedback | `()` |

## 6. Tauri Event Surface

所有 events 透過 `app.emit(name, payload)` 全域 emit：

| Event | Source | Payload type |
|---|---|---|
| `hotkey:pressed` | hotkey_listener | `HotkeyEventPayload { mode, action }` |
| `hotkey:released` | hotkey_listener | `HotkeyEventPayload` |
| `hotkey:toggled` | hotkey_listener | `HotkeyEventPayload` |
| `hotkey:mode-toggle` | hotkey_listener | `()` (double-tap or long-press) |
| `hotkey:error` | hotkey_listener | `serde_json::json!({ error, message })` (e.g., accessibility failure) |
| `escape:pressed` | hotkey_listener | `()` |
| `hotkey:recording-captured` | hotkey_listener | `RecordingCapturedPayload { keycode, modifiers }` |
| `hotkey:recording-rejected` | hotkey_listener | `RecordingRejectedPayload { reason }` |
| `quality-monitor:result` | keyboard_monitor | `{ wasModified }` |
| `correction-monitor:result` | keyboard_monitor | `{ anyKeyPressed, enterPressed, idleTimeout }` |
| `audio:waveform` | audio_recorder | `WaveformPayload { levels: [f32; 6] }` ~60fps during recording |
| `audio:preview-level` | audio_recorder | `AudioPreviewLevelPayload { level: f32 }` ~33fps during preview |

所有 structs 用 `#[serde(rename_all = "camelCase")]`、Vue 收到 `wasModified`、`anyKeyPressed`、`recordingDurationMs` 等。

## 7. State Management

每個 state struct 在 `setup` 透過 `app.manage(...)` 註冊、在 commands 透過 `tauri::State<'_, T>` 消費。

- `KeyboardMonitorState` — `Arc<AtomicBool>` for flags + `Arc<Mutex<Instant>>` for last-key time。Persistent listener spawned in `new()`
- `AudioControlState` — `Mutex<Option<bool>>`
- `FocusState` — `Mutex<isize>` (Windows only、holds HWND pointer)
- `AudioRecorderState` — `Mutex<Option<RecordingHandle>>` + `pub(crate) Mutex<Option<Vec<u8>>>` for handed-off WAV bytes
- `AudioPreviewState` — `Mutex<Option<PreviewHandle>>`
- `TranscriptionState` — wrap `reqwest::Client` for connection pooling
- `HotkeyListenerState` — composite `Arc<Mutex<HotkeySharedState>>` + 兩個 `Arc<AtomicBool>` + (macOS) `Arc<Mutex<Option<CFRunLoop>>>`

並發 primitives：`std::sync::Mutex`（無 `parking_lot`）、`Arc`、`AtomicBool`/`AtomicU32` with `Ordering::SeqCst` everywhere。Cross-thread setup-failure reporting 用 `std::sync::mpsc::channel`。唯一的 async 在 `transcription.rs` — `transcribe_audio` 與 `retranscribe_from_file` 是 `async fn` `.await` `reqwest`。其他 commands 都 sync。

## 8. Cross-Platform Handling

`#[cfg(target_os = "macos")]` 與 `#[cfg(target_os = "windows")]`（與其他 Unix targets 的 `cfg(not(any(...)))` fallback）gate 大量程式碼。Patterns：

- Module-scoped：`mod macos { ... }` 與 `mod windows { ... }`（或 `mod windows_audio` 等）。每個 export uniform `pub fn x() -> Result<...>` API；`platform_*` wrapper at file scope dispatch with `cfg`
- Function-scoped：`#[cfg(target_os = "macos")]` 直接放在 fn 上、通常配對 Windows alternative
- Inline within fn body：`#[cfg(target_os = "macos")] { ... }` blocks 在 platform-agnostic commands 內

Cross-platform cleanups：

- macOS keycodes 是 `u16`；Windows VK codes 是 `u32`（cast to `u16` in payloads）。`TriggerKey::Custom { keycode: u16 }` 存 platform 相應的 value
- `tauri::ipc::Response` payload 不同（binary vs JSON `number[]` on macOS）— known issue documented in CLAUDE.md
- `windows_subsystem = "windows"` 只在 release
- macOS 用 `extern crate objc;` with `#[macro_use]` for `msg_send!` macro
- `build.rs` 只在 macOS 加 CoreAudio framework linkage

## 9. tauri.conf.json

```json
"identifier": "com.sayit.app",
"productName": "SayIt",
"version": "0.9.5"
```

**Build：** `beforeDevCommand: pnpm dev`、`devUrl: http://localhost:1420`、`beforeBuildCommand: pnpm build`、`frontendDist: ../dist`

**App：**

- `withGlobalTauri: true` — expose `window.__TAURI__` to frontend
- `macOSPrivateApi: true` — required for `ns_window()`

**Two windows：**

1. `main` (HUD)：470×100、x=null y=0 center、`decorations: false`、`transparent: true`、`alwaysOnTop: true`、`skipTaskbar: true`、`visible: false`、`focus: false`、`resizable: false`、`shadow: false`。Load from default `index.html`
2. `main-window` (Dashboard)：960×680（min 720×480）、`url: "main-window.html"`、`decorations: true`、`titleBarStyle: "Overlay"`、`hiddenTitle: true`、`acceptFirstMouse: true`、預設 hidden、centered

**Security：**

- CSP：`default-src 'self'; connect-src 'self' https://api.groq.com https://generativelanguage.googleapis.com; style-src 'self' 'unsafe-inline'; script-src 'self'; media-src 'self' blob: http://asset.localhost`
- Asset protocol 啟用 with scope `$APPDATA/recordings/**`、recordings 可透過 `convertFileSrc` 播放

**Plugins：**

- Updater endpoint：`https://github.com/chenjackle45/SayIt/releases/latest/download/latest.json`
- Updater pubkey：minisign Ed25519 public key (Base64-wrapped) — 驗證 release 旁的 `.sig` file

**Bundle：**

- `targets: "all"`、`createUpdaterArtifacts: true`
- Icons：`32x32.png`、`128x128.png`、`128x128@2x.png`、`icon.icns`、`icon.ico`
- macOS：`entitlements: ./Entitlements.plist`、`signingIdentity: Developer ID Application: Tai-Cheng Chen (G9J8D2T6DV)`
- `Entitlements.plist`：`com.apple.security.device.audio-input = true`、`com.apple.security.automation.apple-events = true`
- `Info.plist`：只 `NSMicrophoneUsageDescription`

## 10. capabilities/

Single file `capabilities/default.json`：

```json
"identifier": "default",
"windows": ["main", "main-window"],
"permissions": [
  "core:default", "core:window:default",
  "core:window:allow-show", "core:window:allow-hide",
  "core:window:allow-set-ignore-cursor-events",
  "core:window:allow-set-focus", "core:window:allow-set-position",
  "core:window:allow-start-dragging", "core:window:allow-center",
  "core:event:default", "core:event:allow-listen",
  "core:event:allow-emit", "core:event:allow-emit-to",
  "shell:allow-open",
  "sql:default", "sql:allow-execute",
  "store:default",
  { "identifier": "http:default", "allow": [
    { "url": "https://api.groq.com/*" },
    { "url": "https://api.openai.com/*" },
    { "url": "https://api.anthropic.com/*" },
    { "url": "https://generativelanguage.googleapis.com/*" }
  ]},
  "autostart:default", "updater:default", "process:default"
]
```

兩個視窗共用 capability set。`core:window:allow-set-ignore-cursor-events` 對 HUD 的 pass-through interaction 是必要的。HTTP allowlist 是 frontend 可呼叫哪些 LLM providers 的 gatekeeper。

## 11. build.rs

```rust
fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    tauri_build::build()
}
```

兩個 jobs：(1) macOS 上 link CoreAudio 讓 `audio_control.rs` 的 `extern "C"` AudioObject* bindings 解析、(2) 呼叫 `tauri_build::build()` 產生 command/event handler glue 並 embed `tauri.conf.json` 到 binary。

## 12. Notable Patterns

**Error handling：** `thiserror::Error` enums in `audio_recorder`、`clipboard_paste`、`transcription`。每個手動 implement `serde::Serialize` 為 flat string (`serializer.serialize_str(&self.to_string())`) — frontend 永遠收到 user-facing `Display` text 而非 discriminated-union JSON。其他 modules 直接回傳 `Result<T, String>`。沒有 panics in production paths beyond `expect("Failed to build HTTP client")` (init-time) 與 `expect("monitors is non-empty")` (provably safe after prior emptiness check)。

**Logging：** Plain `println!`/`eprintln!` with prefixes like `[hotkey-listener]`、`[audio-recorder]`、`[clipboard-paste]` 等。**沒有 `tracing` 或 `log` crate**。一些 logs 用 `#[cfg(debug_assertions)]` gate 來保持 release builds 安靜；最 expensive 的（例如 `paste_text` print full text）只在 debug。一個 log line 用 `🔴🔴🔴` with `AtomicU32` counter — 為 double-paste regressions 留下的 hot-trace marker。

**Sentry：** Lazy global guard 從 `sentry::init` 回傳。只在 `SENTRY_ENVIRONMENT == "production"` 且 `SENTRY_DSN` set 且不是 `__placeholder__` 時 activate。Release tag `sayit@<CARGO_PKG_VERSION>` 在 compile time bake、Rust + frontend 共用一個 release name (per CLAUDE.md release rules)。`RunEvent::Exit` 時 flush up to 2 s。`send_default_pii: false`。

**RAII / cleanup：** 多處定義 ad-hoc Drop guards：

- `CgEventGuard` in `lib.rs` for `CFRelease` of `CGEvent`
- `ComGuard` in `audio_control.rs::windows_audio` for `CoUninitialize`、with flag to skip uninit when `RPC_E_CHANGED_MODE` returned
- `FocusedElementContext::cleanup()` in `text_field_reader.rs::macos`（manual、not Drop — callers 必須記得 call before going out of scope）

**Threading model：** Long-running native event loops (CGEventTap、Windows hook) 在 dedicated `std::thread::Builder::new().name("...").spawn(...)` threads。Cross-thread setup ack 用 single-shot `std::sync::mpsc::channel`、command 可以回真正 success/failure。Background `std::thread::spawn` 也用於 short-lived timers like toggle long-press detector 與 quality/correction monitor watchdogs。

**Testing：** 每個 module 有 `#[cfg(test)] mod tests` with focused unit tests — pure functions (`normalize_db`、`encode_wav`、`find_monitor_for_cursor`、`extract_excerpt`、`matches_combo_trigger`、`check_double_tap`、`format_whisper_prompt`) 加上 state-transition tests。

## 13. Security Notes

**API key handling：** API keys 永遠不在 Rust 後端 startup 進入或活在任何 persistent Rust state。Frontend 在 `tauri-plugin-store` 保留它們（per CLAUDE.md「禁止 SQLite 存 API Key」）並 per-call 作為 `String` parameter 傳給 `transcribe_audio` / `retranscribe_from_file`。Rust 端 validate key 不空、然後透過 `bearer_auth(&api_key)` attach — 無 logging。

**HTTP allowlist：** `tauri-plugin-http` 被 `capabilities/default.json` 限制到四個 hosts (Groq、OpenAI、Anthropic、Gemini)。Frontend 不能做任意 requests。

**CSP：** `default-src 'self'; script-src 'self'` — 無 `unsafe-eval`。`style-src 'self' 'unsafe-inline'` 是 shadcn-vue 需要的。`media-src 'self' blob: http://asset.localhost` 啟用 recorded WAV 透過 `convertFileSrc` 播放。

**IPC validation：** Type-driven (`serde::Deserialize` 在 request side、type-checked 在 Rust side)。`transcribe_audio` 在前面 reject empty API keys、oversized audio (`> 25 MB`)、與 undersized audio (`< 1000 bytes`)。`paste_text` **不**消毒 text — pasted content 是 LLM 產生的；trust boundary 是 user 在 paste 前審視 HUD。

**Updater：** Minisign Ed25519 public key embed in `tauri.conf.json` 驗證 GitHub Releases 附的 signed `.sig` files。Signing private key 持有 in `TAURI_SIGNING_PRIVATE_KEY` GitHub secret。Endpoint 是 GitHub HTTPS CDN。

**Single-instance：** 防止兩個 copies 互相 stomp（mic device contention、double-paste injection 等）。

**Microphone safety：** `audio_recorder.rs::run_recording_thread` 在 recording loop 後 explicit `stream.pause()` 並 log `SECURITY:` line if fails — flag mic 可能還在 active（因為 cpal Arc-cycle bug、drop stream alone 不一定 call `AudioOutputUnitStop`）。

**System mute fail-safe：** `restore_system_audio` 在 attempt restore **之前** clear state、`RunEvent::Exit` invoke `AudioControlState::shutdown()`、recording 中 crash 不能 strand user speakers in muted state。

**Accessibility prompt：** `AXIsProcessTrustedWithOptions({ AXTrustedCheckOptionPrompt: true })` 在 plugin setup 觸發 system permission dialog 一次（如果 app 還沒 trusted）；`open_accessibility_settings` command 讓 frontend deep-link to System Settings。

**Hardened Runtime / Notarization：** 透過 macOS signing identity (`Developer ID Application: Tai-Cheng Chen (G9J8D2T6DV)`) 加 `Entitlements.plist` 授予 `audio-input` 與 `automation.apple-events` 設定。Notarization 在 CI 進行 per CLAUDE.md (`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` secrets)。
