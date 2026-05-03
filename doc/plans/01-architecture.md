# 系統架構

> **狀態**：Draft v1（M1 IPC contract 已落地）
> **最後更新**：2026-05-02

## 高層架構圖

```
 ┌──────────────────────────────────────────────────────────────────┐
 │                  Tauri Backend (Rust)                            │
 │                                                                  │
 │  lib.rs ─ orchestrator (plugin chain、setup、shutdown)           │
 │      │                                                           │
 │      ├── plugins/audio_recorder.rs       cpal + hound + FFT     │
 │      ├── plugins/audio_control.rs        WASAPI mute            │
 │      ├── plugins/clipboard_paste.rs      arboard + SendInput    │
 │      ├── plugins/hotkey_listener.rs      SetWindowsHookExW      │
 │      ├── plugins/keyboard_monitor.rs     post-paste UX (P2)     │
 │      ├── plugins/sound_feedback.rs       PlaySoundA             │
 │      ├── plugins/transcription_cloud.rs  Groq Whisper API       │
 │      ├── plugins/transcription_local.rs  whisper.cpp binding    │
 │      ├── plugins/credentials.rs          keyring (Cred Vault)   │
 │      └── plugins/database.rs             SQLite (Rust-owned)    │
 │                                                                  │
 │  ┌─── invoke() ──┐         ┌─── emit() ────┐                    │
 │  │               │         │               │                    │
 │  ▼               ▼         ▼               ▼                    │
 │ ┌──────────────┐         ┌──────────────────────┐               │
 │ │   HUD        │         │    Dashboard          │              │
 │ │ index.html   │         │   main-window.html    │              │
 │ │ App.vue      │         │   MainApp.vue + Router│              │
 │ │ HudOverlay   │         │   4 views + Settings  │              │
 │ │ .vue         │         │   shadcn-vue UI       │              │
 │ └──────────────┘         └──────────────────────┘               │
 │  label:main              label:main-window                      │
 │  400×100                  960×680 (min 720×480)                 │
 │  transparent              decorations                            │
 │  alwaysOnTop              預設隱藏                                │
 │  skipTaskbar              centered                               │
 └──────────────────────────────────────────────────────────────────┘
```

## 雙視窗（Dual-window）架構

採用 SayIt 的 dual-window 模式（HUD overlay + Dashboard），但對 SayIt 做幾個關鍵改進：

### 對 SayIt 的改進

| SayIt 問題 | TalkType 解法 |
|---|---|
| Frontend 雙視窗都開 SQLite connection pool（race condition） | **Rust 擁有 SQLite**，frontend 只透過 commands 讀寫；HUD 完全不 access DB |
| HUD 與 Dashboard 共用一個 capability set | **拆兩個 capability files**：`hud.json` 與 `dashboard.json`；HUD 沒 SQL/store 權限 |
| Settings 變更靠 cross-window event 同步、複雜易出錯 | **Settings 由 Rust state 持有**，frontend 變更呼叫 command、Rust 廣播 event 到所有 windows |
| 32 commands + 13 events 沒有 contract test | **加 IPC contract test**（透過 `tauri-reviewer` agent 自動 audit） |

### 視窗詳細設定

#### HUD Window (`label: main`)

- **HTML**：`index.html` → `src/main.ts` → `App.vue` → `HudOverlay.vue`
- **大小**：400 × 100（Phase 1 簡化版本）
- **位置**：螢幕水平置中、y=50（離頂部 50px）
- **flags**：
  - `decorations: false`
  - `transparent: true`
  - `alwaysOnTop: true`
  - `skipTaskbar: true`
  - `visible: false`（預設）
  - `focus: false`
  - `resizable: false`
  - `shadow: false`
- **Visual states**（Phase 1 簡化為 4 states）：
  - `idle`（hidden）
  - `recording`（顯示 + waveform）
  - `transcribing`（顯示 + spinner）
  - `success`（顯示 1s 後 hide）
  - `error`（顯示 3s 後 hide，可 click retry）

#### Dashboard Window (`label: main-window`)

- **HTML**：`main-window.html` → `src/main-window.ts` → `MainApp.vue` + Router
- **大小**：960 × 680（min 720 × 480）
- **flags**：
  - `decorations: true`
  - `resizable: true`
  - `centered: true`
  - 預設 `visible: false`（從 tray 開啟）
- **Routes**：
  - `/dashboard` — 簡單概覽（最近轉錄）
  - `/history` — 全部歷史（搜尋、分頁）
  - `/dictionary` — 詞彙管理
  - `/settings` — 設定
  - `/guide` — 使用指南

## IPC 契約（Phase 1）

### Tauri Commands（Frontend → Rust）

| Command | Module | 用途 |
|---|---|---|
| `ping` | lib.rs (M1) | IPC smoke test：emit `ipc:pong` 給所有 window |
| `start_recording` | audio_recorder | 啟動錄音 thread |
| `stop_recording` | audio_recorder | 停止錄音、回 WAV bytes 與 metadata |
| `transcribe_cloud` | transcription_cloud | 送 Groq Whisper 並回 raw text |
| `transcribe_local` | transcription_local | 用 whisper.cpp 本地轉錄 |
| `polish_text` | (Rust 不做、frontend 直接 fetch LLM API via plugin-http) | — |
| `paste_text` | clipboard_paste | 把 text 放剪貼簿 + 模擬 Ctrl+V |
| `copy_to_clipboard` | clipboard_paste | 純複製 |
| `capture_target_window` | clipboard_paste | Windows-only：記下 paste 目標 HWND |
| `update_hotkey_config` | hotkey_listener | 套新熱鍵 |
| `start_hotkey_recording` | hotkey_listener | 進入熱鍵錄製模式 |
| `cancel_hotkey_recording` | hotkey_listener | 取消熱鍵錄製 |
| `mute_system_audio` / `restore_system_audio` | audio_control | WASAPI mute |
| `list_audio_input_devices` | audio_recorder | 列出 mic 給 settings |
| `start_audio_preview` / `stop_audio_preview` | audio_recorder | Settings mic preview |
| `play_start_sound` / `play_stop_sound` / `play_error_sound` | sound_feedback | 音效 |
| `get_credential` / `set_credential` / `delete_credential` | credentials | API key 存取 Windows Credential Vault |
| `get_settings` / `update_settings` | (Rust state) | 設定統一在 Rust |
| `get_history_paged` / `add_history` / `delete_history` | database | SQLite 操作 |
| `get_vocabulary` / `add_vocabulary` / `update_vocabulary` / `delete_vocabulary` | database | 詞彙操作 |
| `download_whisper_model` | transcription_local | 下載本地 whisper.cpp 模型 |
| `list_local_models` | transcription_local | 列出已下載模型 |
| `request_app_restart` | lib.rs | 重啟 app |
| `debug_log` | lib.rs | Frontend log 進 Rust（dev only） |

### Tauri Events（Rust → Frontend）

| Event | Source | Payload |
|---|---|---|
| `ipc:pong` | lib.rs (M1) | `PongPayload { source: string, timestampMs: number }` — 由 `ping` command 全域 emit |
| `hotkey:pressed` | hotkey_listener | `HotkeyEventPayload { mode, action }` |
| `hotkey:released` | hotkey_listener | `HotkeyEventPayload` |
| `hotkey:toggled` | hotkey_listener | `HotkeyEventPayload` |
| `hotkey:error` | hotkey_listener | `{ error, message }` |
| `hotkey:recording-captured` | hotkey_listener | `{ keycode, modifiers }` |
| `hotkey:recording-rejected` | hotkey_listener | `{ reason }` |
| `escape:pressed` | hotkey_listener | `()` |
| `audio:waveform` | audio_recorder | `{ levels: [f32; 6] }` ~60fps |
| `audio:preview-level` | audio_recorder | `{ level: f32 }` ~33fps |
| `transcription:progress` | transcription_local | `{ percent: f32 }` (whisper.cpp) |
| `model:download-progress` | transcription_local | `{ modelId, downloaded, total }` |
| `settings:updated` | (lib.rs Rust state) | `Settings` snapshot |
| `history:added` | database | `TranscriptionRecord` |
| `vocabulary:changed` | database | `()` (frontend re-fetch) |

### Frontend-only Events（HUD ↔ Dashboard）

| Event | 發送方 | 接收方 | 用途 |
|---|---|---|---|
| `voice-flow:state-changed` | HUD | Dashboard | UI 同步顯示狀態（如 dashboard 的 recording indicator） |
| `transcription:completed` | HUD（after transcribe）| Dashboard | History 自動 refresh |

注意：相較 SayIt，**`settings:updated` 不再是 frontend 發**，而是 Rust 統一發。簡化心智模型。

## 依賴方向規則（Dependency Direction）

學 SayIt 但更嚴格：

```
  views/ ──→ components/ + stores/ + composables/
  stores/ ──→ lib/
  lib/ ──→ External APIs (LLM 4 providers)
  composables/ ──→ Tauri events 抽象層

  ❌ views/ 不可 import lib/
  ❌ components/ 不可 import stores/（除了 layout-level component）
  ❌ stores/ 不可 import 其他 stores（透過 events 解耦）
  ❌ 元件不可直接呼叫 invoke / listen（透過 composables）
  ❌ frontend 不可直接執行 SQL（透過 Rust commands）
```

## State Ownership（誰持有什麼 state）

對 SayIt 的最大改進：**Rust 擁有更多 state**。

### Rust 擁有

| State | 為什麼放 Rust |
|---|---|
| Settings（hotkey、provider、語言、開關等）| 雙視窗都要讀、避免同步問題 |
| Hotkey listener state | 跟 OS event tap 綁、必須 Rust |
| Audio recorder state | cpal stream 在 Rust |
| Audio control state（mute/restore）| WASAPI 在 Rust |
| Focus state（Windows HWND）| `GetForegroundWindow` 在 Rust |
| Transcription state（reqwest client、whisper context）| HTTP / FFI 在 Rust |
| **API keys** | **Windows Credential Vault** 在 Rust（不過 IPC 暴露 — 對 SayIt 改進） |
| SQLite connection pool | Rust 獨佔（對 SayIt 改進）|

### Frontend 擁有

| State | 為什麼 |
|---|---|
| HUD visual mode（CSS animation 用） | 純 UI 狀態 |
| Dashboard view-local state（pagination 等） | 純 UI 狀態 |
| 表單 draft state（settings 編輯中還沒 save）| 短暫 UI 狀態 |
| 歷史 / 詞彙 list cache | Read-through cache from Rust |

### API Key 流程（對 SayIt 重大改進）

SayIt 的 flow（**有問題**）：

```
Frontend Settings UI → tauri-plugin-store (plaintext JSON) → 每次轉錄
                                                              呼叫時把 key
                                                              傳進 Rust command
```

問題：

1. Plaintext on disk
2. API key 每次跨 IPC boundary（不必要）
3. Frontend 程式碼可能 leak key 到 Sentry breadcrumbs

TalkType 的 flow（**改進**）：

```
Frontend Settings UI → invoke("set_credential", { provider, key })
                       │
                       └→ Rust keyring crate → Windows Credential Vault
                                                  (encrypted by user account)

Transcription / polish:
  Rust 直接從 keyring 讀（不需要 frontend 傳 key）
```

效益：

- ✅ Plaintext on disk 消失
- ✅ API key 不再每次跨 IPC
- ✅ Frontend 程式碼根本拿不到 key（無 leak risk）
- ✅ 設定可以 export/import 而不洩漏 key
- ✅ Per-user 加密（由 Windows DPAPI 處理）

## Voice Flow State Machine

```
       ┌────────────┐
       │    idle    │
       └─────┬──────┘
             │ hotkey:pressed (Hold) / hotkey:toggled-on (Toggle)
             ▼
       ┌────────────┐
       │  recording │←── audio:waveform 60fps
       └─────┬──────┘
             │ hotkey:released (Hold) / hotkey:toggled-off (Toggle)
             │ ESC:pressed → cancelled
             ▼
       ┌──────────────┐
       │ transcribing │←── (cloud) Groq HTTP / (local) whisper.cpp progress
       └─────┬────────┘
             │ success
             ├──────────────────────► polish OFF → ┌──────────┐
             │                                     │  paste   │──► success → idle
             │                                     └──────────┘
             │ polish ON
             ▼
       ┌────────────┐
       │  enhancing │←── LLM HTTP (Groq/OpenAI/Anthropic/Gemini)
       └─────┬──────┘
             │ success
             ▼
       ┌──────────┐
       │   paste  │
       └─────┬────┘
             │
             ▼
       ┌──────────┐
       │ success  │── 1s ──► idle
       └──────────┘

       Error 任何階段 → ┌──────────┐
                        │  error   │── 3s 或 user click retry ──► idle / re-attempt
                        └──────────┘
```

實作要點：

- 用 simple enum + `transitionTo()` chokepoint（學 SayIt）
- **不用** XState（增加複雜度、Phase 1 不需要）
- Phase 2 評估升級到 XState 如果 visual modes 變多

## 啟動順序

```
1. main.exe 雙擊
   ↓
2. Tauri 啟動
   ↓
3. lib.rs::run()
   - Sentry init（Phase 2、條件式）
   - plugin chain register
   - command handler register
   - setup callback:
     ├── manage state structs
     ├── init keyring (read settings + verify API keys exist)
     ├── init SQLite + run migrations
     ├── build tray icon
     ├── configure HUD window（topmost、transparent）
     ├── auto-start hotkey listener（如果 settings 有預設）
     └── start single-instance lock
   ↓
4. HUD window 載入 index.html → main.ts → App.vue
   - subscribe Rust events（hotkey、audio:*、transcription:*、settings:updated）
   - 等熱鍵
   ↓
5. Dashboard 預設 hidden、tray 點擊才開
```

## 關鍵不變式（Invariants）

實作時不可違反的 hard rules：

1. **API key 從不在前端**（Rust 從 keyring 讀、用完不暴露）
2. **HUD 從不開 SQLite**（無 connection pool race）
3. **`useTauriEvents.ts` 是唯一 import Tauri event API 的地方**（centralized constants）
4. **`@tauri-apps/plugin-http` 是唯一 HTTP client**（不准 `window.fetch`）
5. **`invoke` 結果必須有 type annotation**（避免 `unknown`）
6. **Rust commands 永遠回 `Result<T, E>`**（never panic）
7. **Lock files 不可手動編輯**（pre-commit hook 強制）
8. **每個 Tauri event 都有 TypeScript payload type**（在 `src/types/events.ts`）
9. **新加 Tauri command 必須在 IPC 契約表更新**（在這個 doc）
10. **Single-instance lock 必須最先 register**（lib.rs plugin chain 第一個）

## 連結

- 技術選型 → [`00-tech-stack.md`](00-tech-stack.md)
- Rust 模組詳細 → [`03-rust-modules.md`](03-rust-modules.md)
- Frontend 結構 → [`04-frontend-structure.md`](04-frontend-structure.md)
- Data model → [`05-data-model.md`](05-data-model.md)
- SayIt IPC 契約 reference → [`../reference/sayit-frontend-analysis.md#9-tauri-bridge`](../reference/sayit-frontend-analysis.md)
- SayIt Rust state 設計 reference → [`../reference/sayit-backend-analysis.md#7-state-management`](../reference/sayit-backend-analysis.md)
- SayIt 雙視窗坑 → [`../reference/sayit-improvements.md#雙視窗-ipc-是-over-engineered`](../reference/sayit-improvements.md)
