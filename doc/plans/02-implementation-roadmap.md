# 實作 Roadmap

> **狀態**：Draft v1（M0–M5 done — M5 HUD overlay 4 visual states + 6-bar waveform + ARIA + reduced-motion + active-monitor positioning + Dashboard sidebar badge 已落地；待 user 跑 14 條 manual acceptance 後 PR merge）
> **最後更新**：2026-05-05

依 milestone 順序拆解 Phase 1 全部任務。每個 milestone 給：deliverable、tasks、acceptance criteria、預估時間、依賴。

## 進度 dashboard

| Milestone | 狀態 | 起 | 訖 |
|---|---|---|---|
| M0：Repo bootstrap | ✅ Done | 2026-05-02 | 2026-05-02 |
| M1：基礎 IPC + 雙視窗 | ✅ Done | 2026-05-02 | 2026-05-02 |
| M2：錄音 pipeline (Rust) | ✅ Done | 2026-05-03 | 2026-05-03 |
| M3：Cloud transcription | ✅ Done | 2026-05-04 | 2026-05-04 |
| M4：全域熱鍵 + paste | ✅ Done | 2026-05-05 | 2026-05-05 |
| M5：HUD overlay | ✅ Done | 2026-05-05 | 2026-05-05 |
| M6：LLM polish 多 provider | 📋 Planned | TBD | TBD |
| M7：Local whisper.cpp | 📋 Planned | TBD | TBD |
| M8：Dashboard 完整化 | 📋 Planned | TBD | TBD |
| M9：Polish + 發布 v0.1.0 | 📋 Planned | TBD | TBD |

---

## M0：Repo Bootstrap

> **Deliverable**：可跑的 Tauri v2 + Vue 3 + shadcn-vue 空 app、CI 通過、Claude hooks 設好

### Tasks

- [ ] `pnpm create tauri-app` 用 Vue + TypeScript + pnpm template
- [ ] 升 Tauri v2、Vue 3.5、Vite 6、TypeScript 5.7、Pinia 3、Vue Router 5
- [ ] 設 `pnpm-lock.yaml`、`Cargo.lock`、`.nvmrc` (Node 24)、`packageManager` pin
- [ ] 加 `pnpm-workspace.yaml`（即使單 package）`onlyBuiltDependencies: ["esbuild"]`
- [ ] 設 Tailwind v4 via `@tailwindcss/vite`
- [ ] 安裝 shadcn-vue（new-york style）— 透過 `npx shadcn-vue@latest init`
- [ ] 加 `lucide-vue-next`、`@vueuse/core`、`reka-ui`（自動 by shadcn-vue）
- [ ] 設 `tsconfig.json` strict mode + path alias `@/* → ./src/*`
- [ ] 設 `eslint.config.js`（flat config、學 SayIt 設定）
- [ ] 設 `vite.config.ts`：`define __APP_VERSION__`、alias、port 1420 strict
- [ ] 設 `vitest.config.ts`：jsdom、globals、include patterns
- [ ] 設 `playwright.config.ts`（跑 vite dev server）
- [ ] 加 `tauri.conf.json`：identifier `com.luluboy168.talktype`、productName `TalkType`、version `0.0.1`
- [ ] 設 Cargo profile.release：panic=abort、lto、opt-level=s、strip
- [ ] 加 `.gitignore`：node_modules、dist、target、.env*、.vscode、*.log、test-results、playwright-report、*.key、*.key.pub
- [ ] 加 `.claude/hooks/`：`protect-config.sh`、`typecheck.sh`、`rustfmt.sh`、`eslint.sh`（學 SayIt）
- [ ] 加 `.claude/settings.json` 連結 hooks
- [ ] 加 `.github/workflows/ci.yml`：vue-tsc + vitest + cargo check (windows-latest matrix)
- [ ] `pnpm tauri dev` 能跑、空白 Vue app 能 render
- [ ] `pnpm tauri build` 能 build 出 unsigned binary
- [ ] CI green

### Acceptance criteria

- ✅ Repo 可以 clone 後 `pnpm install && pnpm tauri dev` 開出空 Vue app
- ✅ CI workflow 跑通（vue-tsc + vitest + cargo check pass）
- ✅ Claude Code 編輯 `.ts` 自動跑 vue-tsc + eslint
- ✅ Claude Code 編輯 `.rs` 自動跑 rustfmt
- ✅ Claude Code 嘗試編輯 `pnpm-lock.yaml` 被 hard-block

### 預估時間：1 週

---

## M1：基礎 IPC 與雙視窗

> **Deliverable**：HUD（透明、always-on-top、clickthrough）+ Dashboard（裝飾標準視窗）兩個都開得起來、tray icon 能切換顯示、ping/pong event 通得過

### Tasks

- [x] 設 `tauri.conf.json` 兩個 windows：`main`（HUD）+ `main-window`（Dashboard）
- [x] HUD：transparent、alwaysOnTop、skipTaskbar、無 decorations、預設 hidden、無 focus
- [x] Dashboard：normal decorated、預設 hidden、centered、min size 720×480
- [x] 加 `index.html` + `main-window.html` + `src/main.ts` + `src/main-window.ts`
- [x] Vite `rollupOptions.input` 兩個 entry
- [x] HUD App.vue：簡單顯示「TalkType HUD」字樣
- [x] Dashboard MainApp.vue：Sidebar + RouterView，五個 placeholder routes
- [x] 加 vue-router (hash mode) 在 Dashboard
- [x] 加 Pinia 在兩個 entry
- [x] 加 vue-i18n 在兩個 entry，最小 zh-TW + en messages
- [x] Rust：`lib.rs` 寫 `setup` callback 配置兩個視窗
- [x] Rust：tray icon（embedded PNG via `include_bytes!`）+ menu (open dashboard / quit)
- [x] Rust：實作 `ping` command 與 `pong` event 做 IPC smoke test
- [x] Frontend：寫 ping/pong 測試，確認 cross-window event 通
- [x] Single-instance plugin 註冊（第二次啟動 focus dashboard）
- [x] Capabilities：拆 `hud.json` 與 `dashboard.json` 兩個 file
- [x] `useTauriEvents.ts` composable 設好（centralized event constants）

### Acceptance criteria

- ✅ Run `pnpm tauri dev`：HUD 透明 overlay 出現、Dashboard 隱藏
- ✅ Click tray icon → Dashboard 顯示
- ✅ 第二次跑 talktype.exe → 第一個 dashboard focus
- ✅ Ping/pong test 在兩個 window 都收到

### 預估時間：1 週

### 依賴：M0

---

## M2：錄音 Pipeline (Rust)

> **Deliverable**：Rust 能用 cpal 錄音、encode WAV 進 buffer、列出 audio devices、有 mic preview event

### Tasks

- [x] 加 Cargo deps：`cpal 0.15`、`hound 3.5`、`rustfft 6`
- [x] `plugins/audio_recorder.rs`：
  - [x] `AudioRecorderState`（`Mutex<Option<RecordingHandle>>` + `pub(crate) Mutex<Option<Vec<u8>>>` for WAV）
  - [x] `start_recording(device_name) -> Result<()>` command
  - [x] `stop_recording() -> Result<StopRecordingResult>` command（回傳 duration_ms、peak_energy_level、rms_energy_level）
  - [x] `list_audio_input_devices() -> Vec<AudioInputDeviceInfo>`
  - [x] `get_default_input_device_name() -> Option<String>`
  - [x] cpal stream 在 named thread `"audio-recorder"`
  - [x] Sample format dispatch（10 種 cpal sample formats）
  - [x] WAV encode via `hound::WavWriter` to `Cursor` (in-memory)
  - [x] Energy level 計算（peak + RMS）
- [x] `plugins/audio_recorder.rs`：preview path
  - [x] `AudioPreviewState`
  - [x] `start_audio_preview(device_name) -> Result<()>`
  - [x] `stop_audio_preview() -> Result<()>`
  - [x] `audio:preview-level` event 每 30ms emit
- [x] FFT 6-band waveform：
  - [x] `audio:waveform` event 每 16ms emit `WaveformPayload { levels: [f32; 6] }`
  - [x] `normalize_db(-100..-20)` helper
- [x] File-management commands（檔案存 `app_data_dir/recordings/<uuid>.wav`）：
  - [x] `save_recording_file(id) -> Result<String>`
  - [x] `read_recording_file(id) -> Result<tauri::ipc::Response>`
  - [x] `delete_all_recordings() -> u32`
  - [x] `cleanup_old_recordings(days) -> Vec<String>`
- [x] `AudioRecorderError` (thiserror enum + manual Serialize as string)
- [x] **Defense**：`stream.pause()` 後再 drop（cpal Arc-cycle bug 防護）
- [x] **Mic safety log**：pause failure 印 `SECURITY:` log
- [x] Rust unit tests：`encode_wav`、`normalize_db`
- [x] Frontend `useAudioPreview.ts` composable 用 RAF + lerp(0.2)
- [x] Frontend `useAudioWaveform.ts` composable 用 RAF + lerp(0.25)
- [x] Settings page 簡化版 mic picker：列裝置 + 點選即時 preview

### Acceptance criteria

- ✅ Settings 選 mic、看到即時 RMS bar 動
- ✅ 開始錄音 → 60fps waveform 動畫順暢
- ✅ 停止錄音 → 拿到 WAV bytes、可存檔
- ✅ 檔案在 `%APPDATA%\com.luluboy168.talktype\recordings\`
- ✅ Rust tests pass

### 預估時間：1 週

### 依賴：M1

---

## M3：Cloud Transcription（Groq）

> **Deliverable**：錄完音可以 invoke 一個 command 把 WAV 送 Groq Whisper、拿回原始文字

### 設計決策（2026-05-03 plan-time challenger + Q1–Q5 review 後定案）

- **API key 嚴格 Rust-only**（Q1）：`transcribe_cloud` 與後續 M6 LLM polish 一律 Rust-side fetch；frontend 永遠拿不到 key 內容、只能 `set_credential` / `delete_credential` / `has_credential`。對應 [01-architecture.md](01-architecture.md) 不變式 #1。
- **Hard reject > 25 MB**（Q2）：`MAX_WAV_BYTES = 25_000_000` 同時護住 M2 retro #1（OOM 防止）、Groq cap、UX 一致。錄音中達標 emit `audio:recording-aborted` 自動 stop；轉錄前再驗一次（**先驗 size 再 `take()`**）。
- **Vocabulary 兩層**（Q3）：Whisper prompt（probabilistic bias）+ M6 LLM polish（reliable enforcement）共用同一 list；cap **600 chars + 50 terms 雙保險**（避開 Groq prompt char limit ~896；中文人名 / 長詞才不會觸頂）。
- **Proxy 支援**（Q4）：reqwest 預設讀 `HTTPS_PROXY` / `HTTP_PROXY` env var；README 加說明、零程式碼。
- **Test connection 按鈕**（Q5）：M3 ship Groq、M6 extend 到其他 3 provider；同一 Rust command `test_provider_connection` 驗證 key + 網路 + provider 服務狀態。
- **Provider dispatcher 統一**：以 [06-hybrid-transcription.md](06-hybrid-transcription.md) 的 dispatcher pattern 為準（Rust 內部 `transcribe_audio` 派 cloud / local）。

### Tasks

#### 前置：M2 retro 收尾

- [x] **`audio_recorder/mod.rs` → `commands.rs` 拆分**（M2 retro 已 flagged、commands 量會增）
- [x] **`MAX_WAV_BYTES = 25_000_000`** 常數 + 防呆：
  - [x] `recording_thread.rs` 監測 buffer size、達標 emit `audio:recording-aborted` event + 自動 stop_recording（M2 retro #1 OOM 防止）
  - [x] `consume_wav_buffer()` helper（`Mutex::take()` 包裝、是 transcribe 的 last consumer；`save_recording_file` 仍 clone）
  - [x] `clear_recording_buffer` command（M2 retro #3：避免 RAM 漏；`AudioRecordTest` unmount 時呼叫）

#### Credentials

- [x] 加 Cargo dep：`keyring = "3"`、`reqwest = "0.12"` (features: `multipart`, `json`, `rustls-tls`)、（test only）`wiremock`
- [x] `plugins/credentials.rs`：
  - [x] `set_credential(provider: String, key: String) -> Result<()>`：自動 trim 前後空白、前綴粗檢（`gsk_*` for Groq、`sk-*` for OpenAI 等；錯就 reject 而非 401 surprise）
  - [x] `has_credential(provider: String) -> Result<bool>`（不暴露 key 內容給 frontend）
  - [x] `delete_credential(provider: String) -> Result<()>`
  - [x] **不暴露 `get_credential` 給 frontend**；只 Rust internal 用
  - [x] Service name `com.luluboy168.talktype`、user name = `provider_id`（`groq` / `openai` / `anthropic` / `gemini`）

#### Transcription dispatcher + Groq cloud

- [x] `plugins/transcription/mod.rs`：
  - [x] `TranscriptionState { client: reqwest::Client, transcribe_busy: Arc<AtomicBool> }`：120s timeout、connection pool、`User-Agent: TalkType/0.0.1`
  - [x] `transcribe_audio(vocabulary: Option<Vec<String>>) -> Result<TranscriptionResult, TranscriptionError>` dispatcher：
    - M3 hardcode Groq、M7 接 settings 後派 cloud / local（`SettingsState` 在 M8 才到位）
    - `transcribe_busy` AtomicBool guard：in-flight 期間 user 重啟錄音 → reject 新 `start_recording` with `Busy` error
- [x] `plugins/transcription/cloud.rs` `transcribe_cloud_internal(...)`（pub(crate)）：
  - **Pre-check**：`wav_buffer.is_some()`、size in `[1000, MAX_WAV_BYTES]`（**先驗 size 再 `take()`** — 避免 user 失敗時 WAV 被吃掉）
  - 從 keyring 讀 API key（Rust-only、不過 IPC）
  - **Vocabulary cap**：list 內 term 個數 ≤ 50 **且** 拼接後 ≤ 600 chars；超出取 prefix（不 truncate term 中間）
  - Build vocabulary prompt：format `"Important Vocabulary: t1, t2, ..."`
  - Multipart：`file`、`model`（預設 `whisper-large-v3-turbo`）、`response_format=verbose_json`、optional `prompt`
  - POST `https://api.groq.com/openai/v1/audio/transcriptions`
  - 解析 `verbose_json`：`text`、`segments` → `min(no_speech_prob)`
  - **`take()` only after pre-check**（pre-check 失敗時 buffer 仍在、user 可手動 `save_recording_file` 留檔）
  - Emit `transcription:completed` event 廣播給雙視窗（Dashboard 後續 history refresh）
  - 回 `TranscriptionResult { rawText, transcriptionDurationMs, noSpeechProbability }`

#### Error taxonomy + retry

- [x] `TranscriptionError` enum（thiserror、manual `Serialize` 為 flat string）：
  - 資料：`NoAudioData`、`AudioTooSmall(usize)`、`FileTooLarge { actual_bytes, max_bytes }`、`Busy`
  - 認證：`ApiKeyMissing`
  - 網路（拆細自原 `RequestFailed`）：`Offline`、`DnsFailure`、`TlsFailure(detail)`、`Timeout(secs)`、`ConnectionRefused`、`NetworkOther(detail)`
  - 服務：`RateLimited { retry_after_secs: Option<u64> }`（解析 Groq `Retry-After` header）、`ApiError { status, body }`、`ParseError(detail)`
  - 內部：`LockPoisoned`、`Credentials`
- [x] **Retry policy**：自動 retry 1 次只在 `Timeout` / `RateLimited`（後者 honor `Retry-After`）；4xx / `ApiKeyMissing` / `Offline` / `Busy` 不 retry

#### Test connection（Q5）

- [x] `test_provider_connection(provider: String) -> Result<TestConnectionResult, TestConnectionError>`：
  - 從 keyring 讀對應 provider key
  - GET `https://api.groq.com/openai/v1/models` + Bearer auth + 5s timeout（M3 限 Groq；M6 extend 到 OpenAI / Anthropic / Gemini 各自 `/models` endpoint）
  - 200 → `{ ok: true, modelCount: usize }`
  - 401 → `InvalidKey`、403 → `RestrictedKey`、429 → `RateLimited(secs)`、network → `NetworkError(detail)`

#### CSP + capabilities

- [x] CSP 加 `connect-src https://api.groq.com`
- [x] Capability 加 `http:default { allow: [{ url: "https://api.groq.com/*" }] }`

#### Settings UI

- [x] Provider 選擇（dropdown：Groq / OpenAI / Anthropic / Gemini、Phase 1 先 Groq）
- [x] API key 輸入框 → invoke `set_credential` 存進 keyring（自動 trim、前綴粗檢）
- [x] Show「✅ 已儲存」狀態（從 `has_credential` 確認、**絕不 show 真實 key 內容**）
- [x] **「測試連線」按鈕**（Q5）：點擊 → invoke `test_provider_connection` → 1-2s 顯示 ✅ 模型數 / ❌ 具體 reason
- [x] **Privacy disclosure**：第一次設 Groq key 時 dialog「Audio 將傳送到 Groq (US)、政策保留 14 天」（避免 Typeless 那種 marketing 失調）

#### Tests + docs

- [x] Rust unit tests：
  - `format_whisper_prompt`、API key trim、vocabulary cap（char + term 雙限）
  - `wiremock`-based：8+ 個 error variant mock 路徑（401 / 403 / 413 / 429 / 500 / parse error / vocab prompt / etc.）
  - `test_provider_connection` happy + 401 / 403 / 429 / 500 / no-data path（10 tests）
  - 完工：94 個 cargo tests pass
- [x] **README dev section**：加「Behind a corporate proxy?」一段（Q4）

### Acceptance criteria

- ✅ Settings 存 API key → keyring 看得到（用 Windows Credential Manager 開來看）
- ✅ Settings 不顯示 key 明文
- ✅ 點「測試連線」→ 1-2s 內顯示 Groq 模型清單長度（key 對）或「API key 無效」（key 錯）
- ✅ 點測試按鈕：錄 3 秒 → invoke transcribe_audio → 回正確文字
- ✅ API key 錯：返回 `InvalidKey`、UI 顯示「API key 無效，請檢查」
- ✅ 無網路：返回 `Offline`、UI 顯示「連線失敗（檢查網路或 HTTPS_PROXY 設定）」
- ✅ 錄音超過 25 MB：自動 stop + UI「錄音超過上限（~13 分鐘 @ 16 kHz）」
- ✅ 在 in-flight transcribe 期間按熱鍵：拒絕新 start_recording 並提示「上一筆轉錄處理中」

### 預估時間：1.5 週（含 Q5 test button + error taxonomy 拆細 + M2 retro 收尾）

### 依賴：M2

---

## M4：全域熱鍵 + Paste

> **Deliverable**：在任何 Windows app 按住 Right Alt 說話 → 放開後 → 文字貼到游標位置（end-to-end flow 跑通）

### Tasks

- [ ] 加 Cargo dep：`windows 0.61`（features 列表見 `00-tech-stack.md`）、`arboard 3`
- [ ] `plugins/hotkey_listener.rs`（Windows 部分）：
  - [ ] `enum TriggerKey { RightAlt, LeftAlt, RightControl, LeftControl, RightShift, LeftShift, Custom { keycode: u16 }, Combo { modifiers: Vec<ModifierFlag>, keycode: u16 } }`（Phase 1 簡化、不做 Fn）
  - [ ] `enum TriggerMode { Hold, Toggle }`
  - [ ] `HotkeyListenerState`：`Arc<Mutex<HotkeySharedState>>` + `Arc<AtomicBool> is_pressed` + `Arc<AtomicBool> is_toggled_on`
  - [ ] 包成 `tauri::plugin::TauriPlugin`（`Builder::new("hotkey-listener").setup(...).build()`）
  - [ ] `windows_hook::install`：spawn thread 跑 `SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, None, 0)` + `GetMessageW` loop
  - [ ] `hook_proc`：讀 `KBDLLHOOKSTRUCT`、判斷 down/up、dispatch
  - [ ] `is_vk_pressed(vk)` via `GetKeyState`
  - [ ] 發送 events：`hotkey:pressed`、`hotkey:released`、`hotkey:toggled`、`hotkey:mode-toggle`、`escape:pressed`
  - [ ] Hold mode：double-tap detection（350ms 內 release-press）
  - [ ] Toggle mode：XOR `is_toggled_on`
  - [ ] 1000ms long-press detector
- [ ] `update_hotkey_config(trigger_key, trigger_mode) -> Result<()>` command
- [ ] `start_hotkey_recording` / `cancel_hotkey_recording` commands（user 自訂熱鍵 UI）
- [ ] 預設熱鍵：`TriggerKey::RightAlt` + `TriggerMode::Hold`
- [ ] `plugins/clipboard_paste.rs`（Windows 部分）：
  - [ ] `FocusState { target_hwnd: Mutex<isize> }`
  - [ ] `capture_target_window(state)` — `GetForegroundWindow()` 存 HWND
  - [ ] `paste_text(text)`：
    1. `arboard::Clipboard::set_text(text)`
    2. `sleep(50ms)`
    3. `restore_target_window(saved_hwnd)`：`AttachThreadInput` + `SetForegroundWindow` + detach
    4. `sleep(50ms)`
    5. `simulate_paste_via_keyboard()`：build 4 INPUT records (Ctrl↓ V↓ V↑ Ctrl↑) → `SendInput`
  - [ ] `copy_to_clipboard(text)` — 純 set
  - [ ] `ClipboardError` enum
- [ ] Frontend `useVoiceFlowStore` (Pinia)：
  - [ ] State：`status`、`message`、`recordingElapsedSeconds`
  - [ ] Listen `hotkey:pressed` → `handleStartRecording`：
    - `capture_target_window` → `start_recording` → `transitionTo("recording")`
  - [ ] Listen `hotkey:released` → `handleStopRecording`：
    - `stop_recording` → `transcribe_cloud` → `paste_text` → `transitionTo("success")` → 1s → idle
- [ ] Settings UI：trigger key picker（preset dropdown，Phase 1 不做 custom recording）
- [ ] Settings UI：trigger mode RadioGroup（Hold / Toggle）

### Acceptance criteria

- ✅ 開 Notepad → 按住 Right Alt 說「你好世界」→ 放開 → 約 2 秒後 Notepad 出現「你好世界」
- ✅ 開 Word → 同上 → 文字進 Word
- ✅ 開 Slack web → 同上 → 文字進 Slack input
- ✅ 換 Toggle mode：按一下 Right Alt 開錄音、再按一下停止 → 文字 paste
- ✅ 錄音中按 ESC → 取消錄音、不 paste
- ✅ 改熱鍵到 Right Control → 立刻生效、Right Alt 不再觸發

### 預估時間：2 週（這是最複雜的 milestone）

### 依賴：M3

---

## M5：HUD Overlay 完成

> **Deliverable**：HUD 顯示 4 個 visual states（recording / transcribing / success / error），有 waveform、有狀態文字、自動 hide

### Tasks

- [ ] HudOverlay.vue：
  - [ ] 4 個 visual states：`hidden`、`recording`、`transcribing`、`success`、`error`
  - [ ] `recording` 狀態：6 條 waveform bar（用 `useAudioWaveform` 提供 levels）
  - [ ] `transcribing` 狀態：spinner 動畫
  - [ ] `success` 狀態：✓ icon + 1s autohide
  - [ ] `error` 狀態：✗ icon + message + 3s autohide / 可 click retry
  - [ ] 平滑 transition 動畫（CSS transform/opacity）
  - [ ] 加 `aria-live="polite"` + `aria-label`（**對 SayIt 改進** — 0 ARIA）
  - [ ] 加 `prefers-reduced-motion` media query 關閉動畫（**對 SayIt 改進**）
- [ ] App.vue（HUD）：
  - [ ] 啟動時 `setIgnoreCursorEvents(true)`（click-through）
  - [ ] `error` state 切到 `setIgnoreCursorEvents(false)`、可點 retry
  - [ ] Subscribe `useVoiceFlowStore` state changes、show/hide window
  - [ ] 接 `audio:waveform` event 傳給 useAudioWaveform composable
- [ ] HUD 顯示位置：螢幕水平置中、y=50（多螢幕 Phase 1 不處理）
- [ ] Dashboard 加最小 voice flow 監控（顯示「錄音中」icon 在 sidebar）

### Acceptance criteria

- ✅ 按熱鍵 → HUD 平滑出現、waveform 動畫順暢
- ✅ 放熱鍵 → spinner 出現、Groq 回來 → ✓ 1 秒 → 消失
- ✅ 無 API key → ✗ + "API key missing"、3 秒消失
- ✅ HUD 不擋滑鼠（除了 error state）
- ✅ 系統設 reduced motion → 動畫關閉
- ✅ Screen reader 講出狀態變化

### 預估時間：2 週

### 依賴：M4

---

## M6：LLM Polish 多 Provider

> **Deliverable**：4 個 LLM provider（Groq/OpenAI/Anthropic/Gemini）都能跑 polish、可開關、預設開、Rust-side fetch（API key 不過 IPC）

### 設計決策（2026-05-03 Typeless / 競品 research + Q1 (a) review 後定案）

- **架構大轉**：原計畫的 `src/lib/{llmProvider,enhancer,modelRegistry}.ts` 全部移到 Rust（`plugins/llm_polish/`）。理由：
  - Q1 (a) 守住 "API key 從不在前端" 不變式
  - Marketing 一致性：v0.1.0 README 想寫「Your API keys never cross the IPC boundary」這種 strong claim、(b) frontend invoke get_credential 寫不出來
  - M9 release audit 縮小 scope：(a) 只看 Rust、(b) 要看 Rust + frontend + Sentry config
  - Typeless 2025-11 隱私翻車的反面教材
- **Preset modes 改名**（Typeless / Wispr Flow research）：原 `minimal | active | custom` → `default (light cleanup) | email (formal) | chat (casual) | code (preserve format) | custom`。對 user 更直覺、對 LLM 提示更具體。
- **Custom prompt 1000 char cap**：避免 prompt injection 把 system 角色蓋掉、避免超 token context 限制；UI 顯示 token estimate。
- **Latency 收緊**（業界標 <2s、留 buffer）：Groq polish timeout 5s → **3s**、其他 provider 30s → **15s**；polish duration 寫進 `transcriptions.enhancement_duration_ms`（M8 schema 已有），dogfood 期收 p50/p95、M9 polish 時調。
- **Per-app preset hooks**（Wispr Flow 招牌、Typeless adaptive tone）：M6 settings struct 預留 `per_app_preset: HashMap<String, PromptMode>` 但 Phase 1 不實作；v0.2 才接 `GetForegroundWindow().GetProcessName()` → preset。先預留 schema 避免 v0.2 還要動 store。
- **Polish 失敗 fallback**：fallback 到原始 Whisper text + 顯示 warning（不阻擋 paste 流程）。

### Tasks

#### Rust LLM polish module（取代原本 frontend lib/）

- [ ] 加 Cargo deps（如 M3 沒加）：`reqwest 0.12`、`serde_json`、（test only）`wiremock`
- [ ] `plugins/llm_polish/mod.rs`：
  - [ ] `LlmPolishState { client: reqwest::Client, polish_busy: Arc<AtomicBool> }`
  - [ ] `polish_text(raw_text: String, vocabulary: Option<Vec<String>>) -> Result<PolishResult, PolishError>` 主 command：
    - 從 `SettingsState` 讀 `llm_provider`、`llm_model_id`、`llm_prompt_mode`、`llm_custom_prompt`
    - 從 keyring 讀 LLM provider 的 API key（**Rust-only、不過 IPC**）
    - Build provider-specific request、套 timeout（Groq 3s / 其他 15s）
    - 解析 response、`stripReasoningTags` for `<think>...</think>`
    - 回 `PolishResult { polishedText, durationMs }`
  - [ ] `polish_busy` AtomicBool guard（避免雙重 polish 撞同 key 配額）
- [ ] `plugins/llm_polish/providers.rs`（4 provider request 形狀）：
  - [ ] `LlmProviderId` enum: `Groq | OpenAi | Anthropic | Gemini`
  - [ ] `build_openai_compatible_request(req, key)` — Groq + OpenAI（注意 Groq `max_tokens`、OpenAI `max_completion_tokens`）
  - [ ] `build_anthropic_request` — `system` field + `x-api-key` + `anthropic-version: 2023-06-01`
  - [ ] `build_gemini_request` — `/v1beta/models/{model}:generateContent` + `x-goog-api-key`
  - [ ] `parse_provider_response` dispatcher → unified `String` output
- [ ] `plugins/llm_polish/prompts.rs`（preset modes、zh-TW + en 兩版）：
  - [ ] `PromptMode` enum: `Default | Email | Chat | Code | Custom(String)`
  - [ ] `default`: 「輕度清理：去贅詞、修標點、保留口語特徵」
  - [ ] `email`: 「轉成正式書面語、結構化段落、professional tone」
  - [ ] `chat`: 「保留口語、短句、casual tone、不過度修飾」
  - [ ] `code`: 「保留 inline code 與技術術語、不重寫格式、不加標點到 code 中」
  - [ ] `custom`: user-provided prompt（**1000 char cap**、reject if longer、UI 顯示 token estimate）
- [ ] `plugins/llm_polish/vocabulary.rs`：
  - [ ] 注入 `<vocabulary>t1, t2, ...</vocabulary>` 到 system prompt
  - [ ] 與 M3 共用同一 vocabulary list（max 50 terms 或 600 chars）
  - [ ] 提示詞「Preserve these specialized terms exactly as written」
- [ ] `plugins/llm_polish/registry.rs`（model 清單）：
  - [ ] `LLM_MODEL_LIST` ≥ 8 models（每 provider 1-3 個）
  - [ ] `WHISPER_MODEL_LIST` Phase 1 cloud `whisper-large-v3-turbo`、local `ggml-base-q5_1`（M7）
  - [ ] Helper：`find_llm_model_config`、`get_models_by_provider`、`get_default_model_id`
- [ ] `PolishError` enum（thiserror + manual `Serialize`）：
  - 認證：`ApiKeyMissing { provider }`
  - 網路（mirror M3 拆細）：`Offline`、`Timeout`、`TlsFailure`、`ConnectionRefused`
  - 服務：`RateLimited { retry_after_secs }`、`ApiError { status, body }`、`ParseError`、`Busy`
  - 設定：`InvalidPromptLength { actual, max: 1000 }`
  - 內部：`LockPoisoned`

#### `useVoiceFlowStore` 整合

- [ ] After transcribe success → 如果 polish ON → `transitionTo("enhancing")` → invoke `polish_text` → paste polished
- [ ] Polish OFF → 直接 paste raw Whisper text
- [ ] Polish 失敗 → fallback to raw Whisper text + emit `polish:failed-fallback` event（HUD 顯示 warning icon、Dashboard log）

#### Settings UI

- [ ] LLM polish toggle Switch（**預設 ON**）
- [ ] LLM provider Select（4 個、獨立於 Whisper provider）
- [ ] Model Select（filter by provider、預設 model 由 `get_default_model_id` 決定）
- [ ] **Preset Select**：`default | email | chat | code | custom` 5 個 radio
- [ ] Custom prompt textarea（**1000 char cap**、real-time char count、disabled if not custom mode）
- [ ] Test 按鈕：invoke `polish_text("這個 呃 就是我覺得啊", vocabulary)` → show before/after diff（學 Typeless 的 polish 透明化）
- [ ] **Per-step data-flow indicator**：「Audio → Groq Whisper → OpenAI Polish → Paste（無 retention）」清楚 render（避免 Typeless 隱私翻車）
- [ ] **「測試連線」按鈕** extend：M3 ship Groq 版、M6 加 OpenAI / Anthropic / Gemini

#### CSP + capabilities

- [ ] CSP 加 `connect-src` for OpenAI / Anthropic / Gemini
- [ ] Capability 加對應 URL pattern

#### Tests

- [ ] Rust unit tests（`wiremock`）：
  - 4 個 provider 各 happy path 1 個
  - 4 個 provider 各 401 / 429 / parse error mock
  - `PromptMode` 5 個 → system prompt 字串對齊
  - Custom prompt 1001 char → reject
  - Vocabulary cap 邏輯
- [ ] Integration test：raw text → polish → diff

### Acceptance criteria

- ✅ Settings 切 4 個 provider 都 work（test connection 全綠）
- ✅ Settings 切 5 個 preset mode 都產出**明顯不同**的 polish 風格（手動驗證）
- ✅ 中文「呃這個就是我覺得啊」polish 後變「我覺得這個還可以」之類
- ✅ Custom prompt 1001 chars → UI 拒絕儲存
- ✅ Polish OFF → 原始 Whisper 文字直接 paste、polish 路徑完全跳過
- ✅ Polish 失敗（network / rate limit）→ fallback to raw + HUD warning icon、不擋 paste
- ✅ Polish 平均耗時：Groq < 2s p50、< 3s p95（dogfood log 驗證）
- ✅ API key 全程在 Rust 進程：`grep -r "get_credential" src/` 結果為空（frontend 完全不呼叫 get_credential）
- ✅ Rust unit coverage > 70% on `plugins/llm_polish/`

### 預估時間：1.5 週（Q1 (a) 改 Rust-side、原 1 週 + 0.5 週 4-provider Rust client）

### 依賴：M5

---

## M7：Local whisper.cpp 整合

> **Deliverable**：使用者選擇 local provider → 從 UI 下載 whisper.cpp 模型 → 本地推論轉錄

### Tasks

- [ ] **Spike**：選 Rust binding（`whisper-rs` vs `whisper-cpp-2` vs 直接 FFI）
  - 寫 hello-world 跑 base 模型轉錄一個 5 秒 WAV
  - 驗證 Windows build pipeline 不踩雷
  - 量測延遲
- [ ] 加 Cargo dep：選定的 whisper binding crate
- [ ] `plugins/transcription_local.rs`：
  - [ ] `LocalTranscriptionState { context: Mutex<Option<WhisperContext>> }`（lazy load model）
  - [ ] `transcribe_local(model_id) -> Result<TranscriptionResult>`
  - [ ] `download_whisper_model(model_id) -> Result<()>` — 下載到 `app_data_dir/models/`
    - HTTP GET from Hugging Face（`https://huggingface.co/ggerganov/whisper.cpp/resolve/main/...`）
    - 進度 event：`model:download-progress { modelId, downloaded, total }`
    - SHA-256 verify
  - [ ] `list_local_models() -> Vec<LocalModelInfo>`
  - [ ] `delete_local_model(model_id) -> Result<()>`
- [ ] Phase 1 ship 一個模型：`ggml-base-q5_1.bin` (~60 MB)
- [ ] Provider switching logic（Rust）：
  - [ ] Settings `whisper_provider: "groq" | "local"`
  - [ ] Settings `local_model_id: String`
  - [ ] `transcribe_audio` Rust command 派發到 cloud 或 local
- [ ] Fallback logic：
  - [ ] 預設 cloud
  - [ ] 無 API key 但有 local model → fallback to local
  - [ ] 無 network 但 cloud selected → suggest fallback to local
- [ ] Settings UI：
  - [ ] Whisper provider Select（Cloud / Local）
  - [ ] 如選 Local：show 已下載 models list + 「Download」按鈕
  - [ ] Download 中：progress bar
  - [ ] Show model 大小、推估推論速度
- [ ] CSP 加 `connect-src https://huggingface.co`
- [ ] Capability 加 huggingface URL pattern

### Acceptance criteria

- ✅ Settings → Whisper provider 選 Local → 點 Download → 60MB 模型下載成功
- ✅ 切到 Local → 錄音 → 文字正確（可能比 cloud 慢 2–3x，可接受）
- ✅ 移除 API key + 設 local → 仍可 work（offline）
- ✅ 模型下載中斷可 resume（HTTP Range）— 如時間允許
- ✅ SHA-256 校驗失敗 → 重下

### 預估時間：2 週（whisper.cpp 整合是 wild card）

### 依賴：M6

---

## M8：Dashboard 完整化

> **Deliverable**：Dashboard 三個頁面（Settings/History/Dictionary）功能完整，能管理所有設定與歷史

### Tasks

- [ ] **SQLite 設置**：
  - [ ] **Rust 擁有 SQL**（對 SayIt 改進）— 不用 frontend `tauri-plugin-sql`
  - [ ] `plugins/database.rs`：
    - [ ] Connection pool（SQLite + PRAGMA `journal_mode=WAL` + `synchronous=NORMAL`）
    - [ ] Migration runner（v1: 初始 schema）
    - [ ] Schema 詳見 [`05-data-model.md`](05-data-model.md)
- [ ] History commands：
  - [ ] `add_history(record)`、`get_history_paged(offset, limit, search)`、`get_history_by_id(id)`、`delete_history(id)`、`delete_all_history()`、`get_dashboard_stats()`
- [ ] Vocabulary commands：
  - [ ] `add_vocabulary(term)`、`update_vocabulary(id, term)`、`delete_vocabulary(id)`、`get_vocabulary() -> Vec<VocabularyEntry>`
- [ ] HistoryView.vue：
  - [ ] List with pagination（IntersectionObserver infinite scroll）
  - [ ] Search bar（debounce 300ms）
  - [ ] Each row: timestamp + raw + processed + duration + Play + Copy + Delete
  - [ ] Audio playback：invoke `read_recording_file` → Blob URL（Phase 1 Windows 不需要 macOS hack）
- [ ] DictionaryView.vue：
  - [ ] Single section table（Phase 1 無 AI smart learning）
  - [ ] 加詞 input + 確認鍵
  - [ ] Edit / Delete inline
  - [ ] Duplicate detection
- [ ] SettingsView.vue（拆 sub-components 避免 1907 行）：
  - [ ] `SettingsHotkey.vue` — trigger key + mode
  - [ ] `SettingsTranscription.vue` — provider + model + language
  - [ ] `SettingsLlm.vue` — polish toggle + provider + model + custom prompt
  - [ ] `SettingsAudio.vue` — input device + mic preview + mute on recording
  - [ ] `SettingsApiKeys.vue` — 4 provider keys（連 keyring）
  - [ ] `SettingsAppearance.vue` — language（UI vs transcription）+ sound effects
  - [ ] `SettingsAdvanced.vue` — auto cleanup days + log level
- [ ] DashboardView.vue（簡單版）：
  - [ ] 概覽 cards：今日轉錄次數、總字數、節省時間估計
  - [ ] 最近 5 筆轉錄
- [ ] FeatureGuideView.vue：靜態 guide page
- [ ] vue-i18n messages：補完所有 UI 文字 zh-TW + en

### Acceptance criteria

- ✅ History 累積 100+ 筆、搜尋能 work
- ✅ Dictionary 加詞 → Whisper prompt 確實 bias（Phase 1 手動驗證）
- ✅ Settings 各 section 都能存 + 立即生效
- ✅ 切換 UI 語言 → 整個 dashboard 立刻變語言
- ✅ Vitest coverage > 60% on stores

### 預估時間：2 週（事情多但都是 CRUD）

### 依賴：M7

---

## M9：Polish + 發布 v0.1.0

> **Deliverable**：v0.1.0 tag、GitHub Release（不簽名 binary、僅 source code）、完整 README、CONTRIBUTING.md、CODE_OF_CONDUCT.md

### Tasks

- [ ] **Bug fixing 衝刺**：
  - [ ] 自己 daily-driver 一週、列 bug
  - [ ] 修所有 P0/P1 bug
  - [ ] 修所有 console error / warning
- [ ] **Quality checks**：
  - [ ] `cargo clippy -- -D warnings` clean
  - [ ] `vue-tsc --noEmit` clean
  - [ ] ESLint clean
  - [ ] Vitest pass、coverage report 看一眼
  - [ ] Manual smoke test on Windows 11
  - [ ] Performance test：E2E latency 量 10 次取 median
- [ ] **README.md**：
  - [ ] 中英對照
  - [ ] 安裝（clone + build）步驟
  - [ ] 使用方法 + GIF demo（用 ScreenToGif 錄）
  - [ ] 設定 API key 步驟
  - [ ] 致謝（SayIt、whisper.cpp、Tauri、各 LLM provider）
  - [ ] License
- [ ] **CONTRIBUTING.md**：開發環境設定、PR 流程、commit message 規範
- [ ] **CODE_OF_CONDUCT.md**：用 Contributor Covenant 2.1
- [ ] **CHANGELOG.md**：v0.1.0 entry（feature list）
- [ ] **LICENSE**：MIT
- [ ] **GitHub Issues templates**：bug / feature / question
- [ ] **`.github/workflows/ci.yml`** verify still pass
- [ ] **Tag v0.1.0** + push
- [ ] **GitHub Release**：title、description、附 source code zip（沒 binary）
- [ ] **README badge**：Build status、License、Stars
- [ ] **公告**：HN / Reddit / Twitter / 自己 blog（如有）

### Acceptance criteria

- ✅ Tag v0.1.0 published on GitHub
- ✅ README 寫好、有人 follow 步驟成功 build（self-test）
- ✅ Issues template 設好
- ✅ Definition of Done in [`../goals/01-phase1-mvp.md`](../goals/01-phase1-mvp.md) 全部 ✅

### 預估時間：1 週

### 依賴：M8

---

## 跨 milestone 注意事項

### Daily

- 編輯 `.ts` / `.vue` / `.rs` 後自動跑 hooks（vue-tsc / eslint / rustfmt）
- 每 commit 前手動 invoke `verify` skill

### Weekly

- 自己用 TalkType 一整週（dogfooding）
- 寫 weekly note 到 `doc/weekly-notes/YYYY-MM-DD.md`（如有時間）
- Review 卡關 issues、調整 milestone

### Milestone 完成 checklist

每個 M 結束時：

- [ ] Acceptance criteria 全 ✅
- [ ] Vitest pass
- [ ] CI pass
- [ ] 自己手動 smoke test
- [ ] CHANGELOG.md 加 entry（即使只是 internal）
- [ ] Tag intermediate version（如 v0.0.M-pre）方便回溯

## 風險緩解

詳見 [`../goals/01-phase1-mvp.md`](../goals/01-phase1-mvp.md) Phase 1 風險表。

關鍵風險：

- **whisper.cpp 整合（M7）**：可能 1 週搞定、也可能 2-3 週踩雷。考慮先 M0–M6 完成、確認核心 flow work、再回頭處理 M7
- **全域熱鍵 in Windows 11**：M4 進度直接決定整個 project 命運。預留 buffer

## 連結

- Goals → [`../goals/`](../goals/)
- 架構 → [`01-architecture.md`](01-architecture.md)
- 模組細節 → [`03-rust-modules.md`](03-rust-modules.md) + [`04-frontend-structure.md`](04-frontend-structure.md)
- Hybrid transcription → [`06-hybrid-transcription.md`](06-hybrid-transcription.md)
- Phase 2 distribution → [`07-phase2-distribution.md`](07-phase2-distribution.md)
