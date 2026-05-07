# 資料模型

> **狀態**：Draft v1（M6 — Settings v1 schema 加 7 個 LLM polish optional fields：`llmPolishEnabled` (tri-state) / `llmProvider` / `llmModelId` / `llmModelIdOverride` (UI 不曝、v0.2 expose) / `llmPromptMode` / `llmCustomPrompt` / `llmPolishRetryEnabled`；無 migration、`schemaVersion` 維持 1）
> **最後更新**：2026-05-06

SQLite schema、settings JSON 格式、與 OS Credential Vault 儲存策略。

## 三層儲存策略

| 資料類型 | 儲存位置 | 為什麼 |
|---|---|---|
| **API keys** | Windows Credential Vault（透過 `keyring` Rust crate） | 加密、per-user、OS-managed | 
| **Settings**（hotkey、provider 選擇、開關等） | `tauri-plugin-store` JSON file | 易 export/import、跨 platform 一致 |
| **Audio files** | `app_data_dir/recordings/<uuid>.wav` | 大檔案、不適合 DB |
| **歷史 / 詞彙 / usage** | SQLite | 結構化查詢、分頁、搜尋 |

## SQLite Schema (Phase 1 — schema_version: 1)

### 檔案位置

`%APPDATA%\com.luluboy168.talktype\talktype.db`

### PRAGMAs

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
PRAGMA foreign_keys = ON;
```

### Tables

#### `schema_version`

```sql
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY
);
INSERT INTO schema_version (version) VALUES (1);
```

#### `transcriptions`

```sql
CREATE TABLE transcriptions (
    id                       TEXT PRIMARY KEY,             -- UUID v4
    created_at               TEXT NOT NULL,                -- ISO 8601 UTC
    raw_text                 TEXT NOT NULL,                -- Whisper 原始輸出
    processed_text           TEXT,                         -- LLM polish 後（如有）
    char_count               INTEGER NOT NULL,             -- processed_text 字數（用於統計）
    recording_duration_ms    INTEGER NOT NULL,
    transcription_duration_ms INTEGER NOT NULL,            -- Whisper 推論耗時
    enhancement_duration_ms  INTEGER,                      -- LLM polish 耗時
    whisper_provider         TEXT NOT NULL,                -- "groq" | "local"
    whisper_model_id         TEXT NOT NULL,                -- "whisper-large-v3-turbo" | "ggml-base"
    llm_provider             TEXT,                         -- 如 polish ON
    llm_model_id             TEXT,
    was_polished             INTEGER NOT NULL DEFAULT 0,   -- 0 | 1
    no_speech_probability    REAL,                         -- 0.0 ~ 1.0
    peak_energy_level        REAL,
    rms_energy_level         REAL,
    audio_file_path          TEXT,                         -- relative to recordings/
    status                   TEXT NOT NULL DEFAULT 'success',  -- 'success' | 'failed' | 'cancelled'
    error_message            TEXT,
    target_window_title      TEXT                          -- Phase 2 加：哪個 app paste 進去的
);

CREATE INDEX idx_transcriptions_created_at ON transcriptions (created_at DESC);
CREATE INDEX idx_transcriptions_raw_text   ON transcriptions (raw_text);
```

##### TypeScript 對應

```typescript
interface TranscriptionRecord {
  id: string;
  createdAt: string;  // ISO 8601 with Z suffix
  rawText: string;
  processedText: string | null;
  charCount: number;
  recordingDurationMs: number;
  transcriptionDurationMs: number;
  enhancementDurationMs: number | null;
  whisperProvider: 'groq' | 'local';
  whisperModelId: string;
  llmProvider: LlmProviderId | null;
  llmModelId: string | null;
  wasPolished: boolean;
  noSpeechProbability: number | null;
  peakEnergyLevel: number | null;
  rmsEnergyLevel: number | null;
  audioFilePath: string | null;
  status: 'success' | 'failed' | 'cancelled';
  errorMessage: string | null;
  targetWindowTitle: string | null;  // Phase 2
}
```

#### `vocabulary`

```sql
CREATE TABLE vocabulary (
    id          TEXT PRIMARY KEY,
    term        TEXT NOT NULL UNIQUE,
    weight      INTEGER NOT NULL DEFAULT 0,    -- Phase 2：被 paste text match 到時 +1
    source      TEXT NOT NULL DEFAULT 'manual', -- 'manual' | 'ai' (ai 為 Phase 2)
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_vocabulary_weight ON vocabulary (weight DESC);
CREATE INDEX idx_vocabulary_term   ON vocabulary (term);
```

##### TypeScript 對應

```typescript
interface VocabularyEntry {
  id: string;
  term: string;
  weight: number;
  source: 'manual' | 'ai';
  createdAt: string;
  updatedAt: string;
}
```

### Phase 2 加的 Tables

```sql
-- Phase 2: api_usage tracking
CREATE TABLE api_usage (
    id              TEXT PRIMARY KEY,
    transcription_id TEXT,                          -- FK transcriptions.id
    api_type        TEXT NOT NULL,                  -- 'whisper' | 'llm_enhancement' | 'vocabulary_analysis'
    provider        TEXT NOT NULL,
    model_id        TEXT NOT NULL,
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    audio_duration_ms INTEGER,
    cost_usd_ceiling REAL,                          -- 保守估計
    created_at      TEXT NOT NULL,
    FOREIGN KEY (transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE
);

CREATE INDEX idx_api_usage_created_at ON api_usage (created_at DESC);
CREATE INDEX idx_api_usage_provider   ON api_usage (provider, api_type);
```

## Settings JSON (`tauri-plugin-store`)

### 檔案位置

`%APPDATA%\com.luluboy168.talktype\settings.json`

### M4 chunk 3 v1 schema（已落地）

M4 chunk 3 ship 的最小 schema — 只有 `hotkey` + 一個 `schemaVersion` slot。後續 milestone（M5–M8）會 in-place 擴充：每加一個欄位只是 `Settings` struct + `SettingsPatch` 多一個 `Option<...>` field、`apply_patch` 多一個 merge arm，**不需要 migration**（unknown fields 由 `tauri-plugin-store` 自動保留）。

```rust
// src-tauri/src/settings.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,         // Phase 1 v1 = 1
    #[serde(default)]
    pub hotkey: HotkeyConfig,        // RightAlt + Hold default
}
```

對應的 store 內容（user-readable JSON）：

```json
{
  "settings": {
    "schemaVersion": 1,
    "hotkey": {
      "triggerKey": "right-alt",
      "triggerMode": "hold"
    }
  }
}
```

### Phase 1 完整目標 schema（M5–M8 補完）

下面是 Phase 1 Definition of Done 時 Settings 的完整形狀；M4 chunk 3 只 ship `hotkey`，其餘 milestone 把對應欄位塞進來。`schemaVersion` 維持 1（純加欄位、舊 settings.json 仍可 deserialize、no migration needed）。

```typescript
interface Settings {
  // ===== Schema metadata（M4 chunk 3 已 ship）=====
  schemaVersion: number;              // 1
  
  // ===== 熱鍵（M4 chunk 3 已 ship）=====
  hotkey: {
    triggerKey: TriggerKey;           // 預設 'right-alt'
    triggerMode: TriggerMode;         // 預設 'hold'
  };
  
  // ===== 轉錄（M5/M7 補完）=====
  whisperProvider?: 'groq' | 'local';   // M7 加入；預設 'groq'
  whisperModelId?: string;              // M7 加入
  languageTranscription?: string | null; // M7/M8
  
  // ===== LLM Polish（M6 已落地、7 fields）=====
  llmPolishEnabled?: boolean | null;     // Tri-state Decision #7：null = auto-detect via has_credential, true = explicit ON, false = explicit OFF
  llmProvider?: LlmProviderId;           // 'groq' | 'openrouter' | 'nvidia' | 'gemini' (M6 4 free providers; 'openai' / 'anthropic' defer to v0.2)
  llmModelId?: string;                   // 預設 by provider，user 可從 dropdown 選 ship 的 8 models (M6: 4 provider × 2 model)
  llmModelIdOverride?: string;           // F4 escape hatch - UI 不曝（v0.2 expose）；user 改 settings.json 手動 set；read 優先 get_effective_model_id() = override.or(model_id)
  llmPromptMode?: 'default' | 'email' | 'chat' | 'code' | 'custom';
  llmCustomPrompt?: string | null;       // chars().count() ≤ 1000 (CJK 1 char = 1 count、不是 bytes)
  llmPolishRetryEnabled?: boolean | null; // F34 retry toggle、tri-state Decision #5：null = ON default, true = ON, false = OFF (no retry)
  
  // ===== UI / 語言（M8）=====
  languageUi?: string;
  
  // ===== 音頻（M5/M8）=====
  audioInputDeviceName?: string | null;
  muteOnRecording?: boolean;
  soundEffectsEnabled?: boolean;
  
  // ===== 自動清理（M8）=====
  autoCleanupRecordingsEnabled?: boolean;
  autoCleanupDays?: number;
  
  // ===== 自啟動（M9）=====
  autoStartAtLogin?: boolean;
}

// Phase 1 ships preset-only — Custom / Combo deferred to Phase 2.
type TriggerKey =
  | 'right-alt' | 'left-alt'
  | 'right-control' | 'left-control'
  | 'right-shift' | 'left-shift';

type TriggerMode = 'hold' | 'toggle';
```

`SettingsPatch`（`update_settings` command 的 input）每個欄位 `Option<>`，sparse 更新；missing 欄位不動。

### Phase 2 加的 settings

```typescript
// Phase 2 additions
interface SettingsV2 extends Settings {
  // Telemetry
  telemetryEnabled: boolean;           // opt-in、預設 false
  
  // Smart dictionary（AI learning）
  smartDictionaryEnabled: boolean;     // 預設 true
  
  // Edit mode
  editModeEnabled: boolean;            // 預設 false（先 evaluate 安全性）
  
  // Threshold
  enhancementThresholdEnabled: boolean;
  enhancementThresholdCharCount: number;  // text < N 跳過 polish
  
  // Multi-monitor
  hudFollowActiveMonitor: boolean;     // 預設 true
  
  _settingsSchemaVersion: 2;
}
```

### Settings migration 策略

```rust
fn migrate_settings(raw: serde_json::Value) -> Result<Settings, Error> {
    let version = raw.get("_settingsSchemaVersion").and_then(|v| v.as_u64()).unwrap_or(0);
    
    let migrated = match version {
        0 => migrate_from_v0_to_v1(raw)?,
        1 => raw,
        v => return Err(Error::UnsupportedSettingsVersion(v)),
    };
    
    serde_json::from_value(migrated).map_err(Into::into)
}
```

## OS Credential Vault — API Key Storage

### 服務名 + key 命名

- Service: `com.luluboy168.talktype`
- Username (作為 key 名)：`groq` | `openai` | `anthropic` | `gemini`

### Windows Credential Manager 顯示

設定後在「控制台 → 認證管理員 → Windows 認證」會看到：

```
網際網路或網路位址：com.luluboy168.talktype/groq
使用者名稱：groq
密碼：(隱藏)
```

### Phase 2 macOS 對應

macOS Keychain Services 自動處理。同樣 `keyring` API。

### 注意事項

- ❌ **永遠不在 settings.json 存 API key**（即使加密）
- ❌ **永遠不在 SQLite 存 API key**
- ❌ **永遠不 log API key**（包括 Sentry breadcrumbs Phase 2）
- ❌ **永遠不 expose `get_credential` 給 frontend**（只 Rust internal 用）
- ✅ Frontend 只能呼叫 `set_credential` / `delete_credential` / `has_credential`
- ✅ 轉錄/polish 時 Rust 自己從 keyring 讀

## Audio File Storage

### 檔案位置

`%APPDATA%\com.luluboy168.talktype\recordings\<uuid>.wav`

### 格式

- 16 kHz mono
- 16-bit PCM
- WAV header（hound crate）

### Lifecycle

```
start_recording() → cpal stream → in-memory Vec<i16>
stop_recording() → encode WAV → store in AudioRecorderState::wav_buffer
transcribe_*() → take() WAV → send to API → success
add_transcription() → 如果 settings 有開保留 audio → save_recording_file(id) → write disk
                                                  → store path in transcription record
```

### 自動清理（Phase 1 預設關，Phase 2 可開）

```rust
// 每次啟動執行一次
if settings.auto_cleanup_recordings_enabled {
    cleanup_old_recordings(settings.auto_cleanup_days)?;
}
```

### Phase 2 加密

```rust
// Phase 2: 用 OS-derived key 加密 WAV
// Windows: DPAPI (CryptProtectData / CryptUnprotectData)
// macOS: Keychain-stored AES key
fn save_encrypted_recording(id: &str, wav: &[u8]) -> Result<()>;
fn read_encrypted_recording(id: &str) -> Result<Vec<u8>>;
```

## Whisper 本地模型存放

### 檔案位置

`%APPDATA%\com.luluboy168.talktype\models\<model_id>.bin`

### 模型清單（Phase 1）

| Model ID | 檔名 | 大小 | 用途 |
|---|---|---|---|
| `ggml-base-q5_1` | `ggml-base-q5_1.bin` | ~60 MB | Phase 1 預設、平衡延遲與品質 |

### Phase 2 加

| Model ID | 檔名 | 大小 |
|---|---|---|
| `ggml-tiny-q5_1` | `ggml-tiny-q5_1.bin` | ~30 MB |
| `ggml-small-q5_1` | `ggml-small-q5_1.bin` | ~180 MB |
| `ggml-medium-q5_1` | `ggml-medium-q5_1.bin` | ~530 MB |
| `ggml-large-v3-q5_1` | `ggml-large-v3-q5_1.bin` | ~1100 MB |

### 模型 metadata table

```sql
CREATE TABLE local_models (
    model_id     TEXT PRIMARY KEY,
    file_name    TEXT NOT NULL,
    file_size    INTEGER NOT NULL,
    sha256       TEXT NOT NULL,
    downloaded_at TEXT NOT NULL,
    last_used_at TEXT
);
```

### 下載來源

`https://huggingface.co/ggerganov/whisper.cpp/resolve/main/<filename>`

CSP 與 capability 必須 allow `https://huggingface.co/*`。

### SHA-256 校驗

下載完用 `sha2` crate 算 hash 比對。

## Logs

### 檔案位置

`%APPDATA%\com.luluboy168.talktype\logs\talktype.log`

### Format

Phase 1 簡單：

```
[2026-05-02T14:33:21Z] [INFO] [audio_recorder] Recording started: device=Default, rate=16000
[2026-05-02T14:33:25Z] [INFO] [transcription_cloud] Sending 320KB to Groq Whisper
[2026-05-02T14:33:26Z] [WARN] [hotkey_listener] Race condition detected: ignored
```

### Phase 2 升級

- Daily rotation
- 配 `tracing` crate（取代 println!）
- 配 Sentry breadcrumb integration

## Filesystem Layout 總覽

```
%APPDATA%\com.luluboy168.talktype\
├── settings.json                    Settings
├── talktype.db                      SQLite (history + vocabulary)
├── talktype.db-wal                  SQLite WAL
├── talktype.db-shm                  SQLite SHM
├── recordings\
│   ├── 550e8400-e29b-41d4-...wav   Audio files
│   └── ...
├── models\
│   └── ggml-base-q5_1.bin          Whisper local models
└── logs\
    └── talktype.log                 Application logs

Windows Credential Vault (per user):
└── com.luluboy168.talktype\
    ├── groq                         API key for Groq
    ├── openai                       API key for OpenAI
    ├── anthropic                    API key for Anthropic
    └── gemini                       API key for Gemini
```

## 對 SayIt data model 的差異

| 維度 | SayIt | TalkType | 為什麼 |
|---|---|---|---|
| API keys | settings.json plaintext | OS Credential Vault | 安全 |
| SQLite owner | Frontend (tauri-plugin-sql) | Rust (rusqlite) | 避免雙視窗 race |
| Audio at rest | Plaintext WAV | Plaintext (Phase 1) → Encrypted (Phase 2) | 隱私漸進改進 |
| `transcriptions.was_modified` | Yes | No (Phase 1) | quality monitor 是 Phase 2 |
| `transcriptions.is_edit_mode` | Yes | No (Phase 1) | edit mode 是 Phase 2 |
| `api_usage` | Yes | No (Phase 1) | usage tracking 是 Phase 2 |
| `vocabulary.weight` | 動態 | 固定 0 (Phase 1) | smart learning 是 Phase 2 |
| Settings schema versioning | Implicit migration | Explicit `_settingsSchemaVersion` | 易維護 |

## Backup / Export 策略（Phase 2 考慮）

- Settings JSON 自然可以複製
- SQLite 用 `.dump` / `VACUUM INTO`
- Recording files 直接複製目錄
- API keys **不**包進 export（永遠手動重輸）

## 連結

- 架構 → [`01-architecture.md`](01-architecture.md)
- Rust modules → [`03-rust-modules.md`](03-rust-modules.md)
- Hybrid transcription → [`06-hybrid-transcription.md`](06-hybrid-transcription.md)
- SayIt schema reference → [`../reference/sayit-frontend-analysis.md#7-libraryutils-layer-srclib`](../reference/sayit-frontend-analysis.md)（database.ts section）
