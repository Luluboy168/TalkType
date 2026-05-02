# Rust 模組規劃

> **狀態**：Draft v1
> **最後更新**：2026-05-02

每個 Rust module 的責任、interface、與相對於 SayIt 的差異。

## 目錄結構

```
src-tauri/
├── Cargo.toml
├── Cargo.lock
├── tauri.conf.json
├── build.rs                      只 link CoreAudio (Phase 2 macOS)
├── capabilities/
│   ├── hud.json                  ★ 對 SayIt 改進：拆兩個 capability
│   └── dashboard.json
├── icons/
└── src/
    ├── main.rs                   只一行 fn main() { talktype_lib::run() }
    ├── lib.rs                    Orchestrator (~300 行)
    ├── settings.rs               ★ 新：Rust-owned Settings state
    ├── error.rs                  共用 error types
    └── plugins/
        ├── mod.rs                pub mod 列表
        ├── audio_recorder.rs     cpal + hound + FFT
        ├── audio_control.rs      WASAPI mute (Phase 2 加 macOS)
        ├── clipboard_paste.rs    arboard + SendInput
        ├── hotkey_listener.rs    SetWindowsHookExW (Phase 2 加 CGEventTap)
        ├── keyboard_monitor.rs   ★ Phase 2 only — quality/correction monitor
        ├── sound_feedback.rs     PlaySoundA
        ├── transcription_cloud.rs  Groq Whisper API
        ├── transcription_local.rs  ★ 新：whisper.cpp binding
        ├── credentials.rs       ★ 新：keyring crate
        └── database.rs          ★ 新：Rust-owned SQLite (對 SayIt 改進)
```

## lib.rs — Orchestrator

**Phase 1 目標 ~300 行**（SayIt 是 ~750 行、太肥）。

職責：

1. Sentry init（Phase 2 條件式）
2. Plugin chain register
3. Command handler register
4. `setup` callback：
   - `app.manage(...)` 所有 state structs
   - 初始化 `settings.rs`（從 JSON store 讀）
   - 初始化 `database.rs`（連接 + run migrations）
   - Init `credentials.rs`（不開啟 keyring，just config service name）
   - Build tray icon
   - Configure HUD window（Windows-only 設 topmost、`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`）
   - Center HUD horizontally
   - Auto-start hotkey listener（如 settings 有預設）
5. `on_window_event`：`main-window` close → hide instead of destroy
6. `RunEvent::Exit`：8-step shutdown（學 SayIt）
   - Restore audio mute
   - Stop preview
   - Stop recording
   - Cancel hotkey listener
   - Persist settings
   - Sleep 200ms
   - Flush Sentry (Phase 2)
   - 如 RESTART_REQUESTED → spawn new process
   - `_exit(0)`

### 對 SayIt 的差異

- ✅ 拆 `settings.rs` 出來、不在 lib.rs
- ✅ 拆 `database.rs` 出來、Rust 擁有 SQLite
- ✅ Capability 拆兩個 file
- ✅ tray icon 程式碼放單獨檔（`tray.rs`）

## settings.rs — Rust-owned Settings

**這是對 SayIt 的重大改進**。

SayIt 把 settings state 全放 Pinia store（`useSettingsStore.ts` 1395 行），靠 `tauri-plugin-store` 持久化。問題：

- 雙視窗都要讀 store → race condition
- 變更時要 emit cross-window event → 易忘
- API key 跟 settings 混在一起

TalkType：

```rust
pub struct Settings {
    pub hotkey: HotkeyConfig,         // trigger_key + trigger_mode
    pub trigger_mode: TriggerMode,
    pub whisper_provider: WhisperProvider,  // "groq" | "local"
    pub whisper_model_id: String,
    pub local_whisper_model: String,        // "ggml-base-q5_1"
    pub llm_provider: LlmProviderId,
    pub llm_model_id: String,
    pub llm_polish_enabled: bool,           // 預設 true
    pub llm_polish_prompt_mode: PromptMode, // minimal | active | custom
    pub llm_polish_custom_prompt: Option<String>,
    pub language_ui: String,                // BCP 47 (zh-TW, en, ...)
    pub language_transcription: Option<String>,  // None = auto
    pub audio_input_device: Option<String>,
    pub mute_on_recording: bool,
    pub sound_effects_enabled: bool,
    pub auto_cleanup_recordings_enabled: bool,
    pub auto_cleanup_days: u32,
    pub auto_start_at_login: bool,
}

pub struct SettingsState {
    inner: Arc<RwLock<Settings>>,
    store: Arc<tauri_plugin_store::Store<R>>,  // for persistence
}

impl SettingsState {
    pub fn load_or_default(store) -> Self;
    pub async fn get(&self) -> Settings;
    pub async fn update<F>(&self, app: &AppHandle, mutator: F) -> Result<(), Error>
        where F: FnOnce(&mut Settings);
    // 內部：
    //   1. mutate
    //   2. persist to store
    //   3. emit "settings:updated" globally
}
```

### Commands

- `get_settings() -> Settings`
- `update_settings(patch: SettingsPatch) -> Result<(), String>`
- 不需要每個 setting 一個 command — 一個 patch 命令搞定

### Frontend 用法

```typescript
// 讀
const settings = await invoke<Settings>("get_settings");

// 改（patch 模式）
await invoke("update_settings", {
  patch: { llmPolishEnabled: false }
});

// 訂閱
listenToEvent("settings:updated", (settings: Settings) => {
  // update UI
});
```

簡化前端心智模型：**單一 source of truth in Rust**。

### 為什麼 settings.json 還是用 tauri-plugin-store？

- 跨 platform 一致（自動找 `app_data_dir`）
- 自動 atomic write
- JSON 可 export / import
- 不需要 reinvent

## credentials.rs — API Key Storage

**對 SayIt 的重大安全改進**。

```rust
use keyring::Entry;

const SERVICE_NAME: &str = "com.luluboy168.talktype";

pub fn set_credential(provider: &str, api_key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, provider)?;
    entry.set_password(api_key)?;
    Ok(())
}

pub fn get_credential(provider: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE_NAME, provider)?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_credential(provider: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, provider)?;
    entry.delete_password()?;
    Ok(())
}

pub fn has_credential(provider: &str) -> Result<bool, String> {
    Ok(get_credential(provider)?.is_some())
}
```

### Commands

- `set_credential(provider: String, api_key: String) -> Result<(), String>`
- `delete_credential(provider: String) -> Result<(), String>`
- `has_credential(provider: String) -> Result<bool, String>`
- **不暴露 `get_credential` 給 frontend**！只 Rust internal 用

### 平台對應

| OS | 後端 |
|---|---|
| Windows | Windows Credential Manager (透過 wincred) |
| macOS (Phase 2) | Keychain Services |
| Linux (未來) | Secret Service (libsecret) |

## database.rs — Rust-owned SQLite

**對 SayIt 的重大改進**：Rust 擁有連線池，frontend 不開 DB。

```rust
use rusqlite::{Connection, params};
// 或用 r2d2 + r2d2-sqlite for connection pool

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn open(app_data_dir: &Path) -> Result<Self, Error> {
        let db_path = app_data_dir.join("talktype.db");
        let manager = SqliteConnectionManager::file(db_path)
            .with_init(|c| {
                c.execute_batch("
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    PRAGMA busy_timeout = 5000;
                ")
            });
        let pool = Pool::builder().max_size(4).build(manager)?;
        Self::run_migrations(&pool)?;
        Ok(Database { pool })
    }
    
    fn run_migrations(pool: &Pool<...>) -> Result<(), Error> {
        // 學 SayIt 的 schema_version table 模式
        // v1: 初始 schema
    }
}
```

### Schema (Phase 1) — 詳見 [`05-data-model.md`](05-data-model.md)

- `transcriptions` — 歷史紀錄
- `vocabulary` — 詞彙
- `schema_version` — migration tracking
- (Phase 2 加 `api_usage`)

### Commands

- `add_transcription(record) -> Result<String, Error>`（回傳 generated UUID）
- `get_transcription_paged(offset, limit, search) -> Result<Vec<TranscriptionRecord>, Error>`
- `get_transcription_by_id(id) -> Result<Option<TranscriptionRecord>, Error>`
- `delete_transcription(id) -> Result<(), Error>`
- `delete_all_transcriptions() -> Result<u32, Error>`
- `get_dashboard_stats() -> Result<DashboardStats, Error>`
- `add_vocabulary(term) -> Result<String, Error>`
- `update_vocabulary(id, term) -> Result<(), Error>`
- `delete_vocabulary(id) -> Result<(), Error>`
- `get_vocabulary() -> Result<Vec<VocabularyEntry>, Error>`

### 註：rusqlite vs tauri-plugin-sql

選 `rusqlite`（直接 Rust crate）：

- ✅ 不依賴 Tauri SQL plugin（避免雙視窗 race）
- ✅ 完整控制 connection lifecycle
- ✅ 更好的 error handling
- ✅ 可加 connection pool（r2d2）

不選 `tauri-plugin-sql`：

- ❌ 設計成 frontend-friendly（每個視窗 `Database.load()` 開新 pool）→ SayIt 踩雷

## audio_recorder.rs

學 SayIt 結構，拆部分 logic 進獨立 sub-modules：

```
audio_recorder/
├── mod.rs              # commands + state
├── stream.rs           # cpal stream config + callback
├── waveform.rs         # FFT + 6-band normalization
├── preview.rs          # preview-specific path
└── files.rs            # save / read / cleanup
```

### 行數預算

SayIt 一個 file 1116 行（太肥）。TalkType 拆後：

- `mod.rs` ~200
- `stream.rs` ~300
- `waveform.rs` ~150
- `preview.rs` ~150
- `files.rs` ~200

每個 < 400 行。

### 對 SayIt 的差異

- 拆 sub-modules
- WAV encode 從 `Cursor` 改成直接 `write_to_file`（避免 in-memory + read again）— Phase 1 不變、Phase 2 評估
- Phase 2 加 Opus encoding（`opus` crate）

## clipboard_paste.rs

```
clipboard_paste/
├── mod.rs              # commands + state (FocusState)
├── paste.rs            # Windows paste injection (SendInput)
└── selected_text.rs    # capture selected text via clipboard hack
```

### 對 SayIt 的差異

- Phase 1 只 Windows path（macOS Phase 2 加）
- 移除 SayIt 的 `🔴🔴🔴 paste_text CALLED (#{})` debug log（沒必要）
- AttachThreadInput 流程：用 RAII guard 確保 detach（不靠 manual call）

## hotkey_listener.rs

```
hotkey_listener/
├── mod.rs              # plugin builder + commands + types
├── windows.rs          # SetWindowsHookExW + hook_proc
├── shared.rs           # platform-agnostic state machine
└── recording.rs        # custom hotkey recording mode
```

### 對 SayIt 的差異

- SayIt 1566 行一個 file → TalkType 預算 < 1000 lines total across files
- Phase 1 不做 Combo trigger（保留 enum 但 UI 不曝露）
- Phase 1 不做 Custom keycode recording（preset only）— 簡化

## transcription_cloud.rs（Phase 1 只 Groq）

```rust
pub struct CloudTranscriptionState {
    client: reqwest::Client,
}

#[tauri::command]
pub async fn transcribe_cloud(
    state: State<'_, CloudTranscriptionState>,
    audio_state: State<'_, AudioRecorderState>,
    credentials_state: State<'_, CredentialsState>,
    settings_state: State<'_, SettingsState>,
    vocabulary: Option<Vec<String>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    // 1. 從 audio_state 拿 WAV bytes (take)
    // 2. 從 settings_state 拿 model_id, language
    // 3. 從 credentials 拿 API key (Rust-side, 不從 frontend 傳！)
    // 4. POST to Groq
    // 5. Parse + return
}
```

### Phase 2 加

- 多 provider transcription（OpenAI Whisper、Deepgram）— mirror `llmProvider` pattern

## transcription_local.rs（Phase 1 新加）

```rust
use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};

pub struct LocalTranscriptionState {
    context: Arc<Mutex<Option<WhisperContext>>>,
    current_model: Arc<Mutex<Option<String>>>,
}

#[tauri::command]
pub async fn transcribe_local(
    state: State<'_, LocalTranscriptionState>,
    audio_state: State<'_, AudioRecorderState>,
    settings_state: State<'_, SettingsState>,
    vocabulary: Option<Vec<String>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    // 1. 確認 model loaded（lazy load 第一次呼叫）
    // 2. 從 audio_state 拿 WAV
    // 3. WAV → f32 samples
    // 4. WhisperContext.full(params, samples)
    // 5. 集合 segments → text
    // 6. Return
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    // 1. 確認 URL（hardcoded Hugging Face URLs in modelRegistry）
    // 2. HTTP GET stream → write to disk + emit progress
    // 3. SHA-256 verify
    // 4. Atomic rename
}
```

### Phase 1 ship 的 model

只 ship 一個：`ggml-base-q5_1.bin`（~60MB、平衡延遲與品質）

### 詳見

[`06-hybrid-transcription.md`](06-hybrid-transcription.md)

## audio_control.rs

### Phase 1 只 Windows

```rust
mod windows_audio {
    // WASAPI flow：
    // CoCreateInstance(MMDeviceEnumerator)
    // → GetDefaultAudioEndpoint(eRender, eConsole)
    // → Activate::<IAudioEndpointVolume>
    // → GetMute() / SetMute()
    
    // ComGuard RAII（學 SayIt 處理 RPC_E_CHANGED_MODE）
}

pub struct AudioControlState(Mutex<Option<bool>>);

impl AudioControlState {
    pub fn mute(&self) -> Result<(), String>;
    pub fn restore(&self) -> Result<(), String>;
    pub fn shutdown(&self) -> Result<(), String>;  // 在 Exit 時叫
}
```

### Phase 2 加 macOS

`mod macos { ... }` 用 CoreAudio AudioObject API。

## sound_feedback.rs

### Phase 1 只 Windows

```rust
use windows::Win32::Media::Audio::{PlaySoundA, SND_MEMORY, SND_ASYNC};

const START_WAV: &[u8] = include_bytes!("../resources/sounds/start.wav");
const STOP_WAV: &[u8] = include_bytes!("../resources/sounds/stop.wav");
const ERROR_WAV: &[u8] = include_bytes!("../resources/sounds/error.wav");

#[tauri::command]
pub fn play_start_sound() {
    play(START_WAV);
}
// ...
```

### 資源

需要準備 3 個小 WAV file（< 50KB each）放 `src-tauri/resources/sounds/`：

- `start.wav` — 短促上揚音
- `stop.wav` — 短促下降音
- `error.wav` — 兩聲低音

可用 [Audacity](https://www.audacityteam.org/) 自製或從 [freesound.org](https://freesound.org/) 找 CC0 的。

## error.rs（共用）

```rust
use thiserror::Error;

#[derive(Error, Debug, serde::Serialize)]
#[serde(tag = "type", content = "message")]
pub enum AppError {
    #[error("Settings error: {0}")]
    Settings(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Credentials error: {0}")]
    Credentials(String),
    #[error("Audio error: {0}")]
    Audio(String),
    // ...
}

// Plus 各 module 自己的 thiserror enum 用 manual Serialize as string
```

## Cargo.toml profile

學 SayIt：

```toml
[profile.dev]
incremental = true

[profile.release]
panic = "abort"
codegen-units = 1
lto = true
opt-level = "s"
strip = true
```

## Crate features

```toml
[lib]
name = "talktype_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
tauri = { version = "2", features = [
    "tray-icon",
    "image-png",
    # 不加 "macos-private-api" until Phase 2
    # 不加 "protocol-asset" because we 用 IPC + Blob URL
] }
# ...
```

## 為什麼這樣拆

| 原則 | 應用 |
|---|---|
| **單一職責** | 每個 module 一個 concern |
| **檔案 < 500 行** | SayIt 1566 行 file 是反例 |
| **Test 可達** | 每個 plugin 都有 `#[cfg(test)] mod tests` |
| **Cross-platform 隔離** | `mod windows { ... }` 與 `mod macos { ... }` clearly separated |
| **Error 類型 typed** | 不用 `Result<T, String>`、用 thiserror enum |

## 行數預算

Phase 1 結束時 Rust code estimate：

- `lib.rs` ~300
- `settings.rs` ~200
- `credentials.rs` ~100
- `database.rs` ~400
- `audio_recorder/*` ~1000（5 files）
- `audio_control.rs` ~200
- `clipboard_paste/*` ~400（3 files）
- `hotkey_listener/*` ~800（4 files）
- `sound_feedback.rs` ~80
- `transcription_cloud.rs` ~200
- `transcription_local.rs` ~300
- `error.rs` ~50

**總計 ~4000 lines Rust**（vs SayIt ~6000 行）。少 33% 但 architecture 更清楚。

## 連結

- 架構 → [`01-architecture.md`](01-architecture.md)
- Frontend → [`04-frontend-structure.md`](04-frontend-structure.md)
- Data model → [`05-data-model.md`](05-data-model.md)
- Hybrid transcription → [`06-hybrid-transcription.md`](06-hybrid-transcription.md)
- SayIt Rust reference → [`../reference/sayit-backend-analysis.md`](../reference/sayit-backend-analysis.md)
