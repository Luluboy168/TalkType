# 實作 Roadmap

> **狀態**：Draft v1
> **最後更新**：2026-05-02

依 milestone 順序拆解 Phase 1 全部任務。每個 milestone 給：deliverable、tasks、acceptance criteria、預估時間、依賴。

## 進度 dashboard

| Milestone | 狀態 | 起 | 訖 |
|---|---|---|---|
| M0：Repo bootstrap | ✅ Done | 2026-05-02 | 2026-05-02 |
| M1：基礎 IPC + 雙視窗 | 📋 Planned | TBD | TBD |
| M2：錄音 pipeline (Rust) | 📋 Planned | TBD | TBD |
| M3：Cloud transcription | 📋 Planned | TBD | TBD |
| M4：全域熱鍵 + paste | 📋 Planned | TBD | TBD |
| M5：HUD overlay | 📋 Planned | TBD | TBD |
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

- [ ] 設 `tauri.conf.json` 兩個 windows：`main`（HUD）+ `main-window`（Dashboard）
- [ ] HUD：transparent、alwaysOnTop、skipTaskbar、無 decorations、預設 hidden、無 focus
- [ ] Dashboard：normal decorated、預設 hidden、centered、min size 720×480
- [ ] 加 `index.html` + `main-window.html` + `src/main.ts` + `src/main-window.ts`
- [ ] Vite `rollupOptions.input` 兩個 entry
- [ ] HUD App.vue：簡單顯示「TalkType HUD」字樣
- [ ] Dashboard MainApp.vue：Sidebar + RouterView，五個 placeholder routes
- [ ] 加 vue-router (hash mode) 在 Dashboard
- [ ] 加 Pinia 在兩個 entry
- [ ] 加 vue-i18n 在兩個 entry，最小 zh-TW + en messages
- [ ] Rust：`lib.rs` 寫 `setup` callback 配置兩個視窗
- [ ] Rust：tray icon（embedded PNG via `include_bytes!`）+ menu (open dashboard / quit)
- [ ] Rust：實作 `ping` command 與 `pong` event 做 IPC smoke test
- [ ] Frontend：寫 ping/pong 測試，確認 cross-window event 通
- [ ] Single-instance plugin 註冊（第二次啟動 focus dashboard）
- [ ] Capabilities：拆 `hud.json` 與 `dashboard.json` 兩個 file
- [ ] `useTauriEvents.ts` composable 設好（centralized event constants）

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

- [ ] 加 Cargo deps：`cpal 0.15`、`hound 3.5`、`rustfft 6`
- [ ] `plugins/audio_recorder.rs`：
  - [ ] `AudioRecorderState`（`Mutex<Option<RecordingHandle>>` + `pub(crate) Mutex<Option<Vec<u8>>>` for WAV）
  - [ ] `start_recording(device_name) -> Result<()>` command
  - [ ] `stop_recording() -> Result<StopRecordingResult>` command（回傳 duration_ms、peak_energy_level、rms_energy_level）
  - [ ] `list_audio_input_devices() -> Vec<AudioInputDeviceInfo>`
  - [ ] `get_default_input_device_name() -> Option<String>`
  - [ ] cpal stream 在 named thread `"audio-recorder"`
  - [ ] Sample format dispatch（10 種 cpal sample formats）
  - [ ] WAV encode via `hound::WavWriter` to `Cursor` (in-memory)
  - [ ] Energy level 計算（peak + RMS）
- [ ] `plugins/audio_recorder.rs`：preview path
  - [ ] `AudioPreviewState`
  - [ ] `start_audio_preview(device_name) -> Result<()>`
  - [ ] `stop_audio_preview() -> Result<()>`
  - [ ] `audio:preview-level` event 每 30ms emit
- [ ] FFT 6-band waveform：
  - [ ] `audio:waveform` event 每 16ms emit `WaveformPayload { levels: [f32; 6] }`
  - [ ] `normalize_db(-100..-20)` helper
- [ ] File-management commands（檔案存 `app_data_dir/recordings/<uuid>.wav`）：
  - [ ] `save_recording_file(id) -> Result<String>`
  - [ ] `read_recording_file(id) -> Result<tauri::ipc::Response>`
  - [ ] `delete_all_recordings() -> u32`
  - [ ] `cleanup_old_recordings(days) -> Vec<String>`
- [ ] `AudioRecorderError` (thiserror enum + manual Serialize as string)
- [ ] **Defense**：`stream.pause()` 後再 drop（cpal Arc-cycle bug 防護）
- [ ] **Mic safety log**：pause failure 印 `SECURITY:` log
- [ ] Rust unit tests：`encode_wav`、`normalize_db`
- [ ] Frontend `useAudioPreview.ts` composable 用 RAF + lerp(0.2)
- [ ] Frontend `useAudioWaveform.ts` composable 用 RAF + lerp(0.25)
- [ ] Settings page 簡化版 mic picker：列裝置 + 點選即時 preview

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

### Tasks

- [ ] 加 Cargo dep：`reqwest 0.12` (multipart, json)
- [ ] `plugins/credentials.rs`：
  - [ ] 加 dep `keyring 3`
  - [ ] `set_credential(provider: String, key: String) -> Result<()>`
  - [ ] `get_credential(provider: String) -> Result<Option<String>>`
  - [ ] `delete_credential(provider: String) -> Result<()>`
  - [ ] Service name 用 `com.luluboy168.talktype`
  - [ ] User name 用 `provider_id`（如 `groq`、`openai`）
- [ ] `plugins/transcription_cloud.rs`：
  - [ ] `TranscriptionState { client: reqwest::Client }`（120s timeout、connection pool）
  - [ ] `transcribe_cloud(provider: String, vocabulary: Option<Vec<String>>, model_id: Option<String>, language: Option<String>) -> Result<TranscriptionResult, TranscriptionError>`
    - 從 `AudioRecorderState::wav_buffer` `take()` WAV
    - 從 keyring 讀 API key（**不從 frontend 傳**）
    - 驗證 size: `>= 1000` bytes、`<= 25 MB`
    - Build multipart form：`file`、`model`（預設 `whisper-large-v3-turbo`）、`response_format=verbose_json`、optional `language`、optional `prompt: "Important Vocabulary: t1, t2, ..."`
    - POST to `https://api.groq.com/openai/v1/audio/transcriptions`
    - 解析 verbose_json：`text`、`segments` 計算 `min(no_speech_prob)`
    - 回 `TranscriptionResult { rawText, transcriptionDurationMs, noSpeechProbability }`
- [ ] `TranscriptionError` enum：`NoAudioData`、`AudioTooSmall(usize)`、`FileTooLarge`、`ApiKeyMissing`、`RequestFailed`、`ApiError(u16, String)`、`ParseError`、`LockPoisoned`
- [ ] CSP 加 `connect-src https://api.groq.com`
- [ ] Capability 加 `http:default { allow: [{ url: "https://api.groq.com/*" }] }`
- [ ] Settings UI：
  - [ ] Provider 選擇（dropdown：Groq/OpenAI/Anthropic/Gemini，Phase 1 先 Groq）
  - [ ] API key input → invoke `set_credential` 存進 keyring
  - [ ] Show「✅ Saved」狀態（從 keyring 讀回確認）
  - [ ] **絕不 show 真實 key 內容**（防 over-shoulder attack）
- [ ] Rust unit tests：`format_whisper_prompt`、API key validation

### Acceptance criteria

- ✅ Settings 存 API key → keyring 看得到（用 Windows Credential Manager 開來看）
- ✅ Settings 不顯示 key 明文
- ✅ 點測試按鈕：錄 3 秒 → invoke transcribe_cloud → 回正確文字
- ✅ API key 錯：返回 ApiError(401)、UI 顯示友善 message
- ✅ 無網路：返回 RequestFailed、UI 顯示 "Network error"

### 預估時間：1 週

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

> **Deliverable**：4 個 LLM provider（Groq/OpenAI/Anthropic/Gemini）都能跑 polish、可開關、預設開

### Tasks

- [ ] `src/lib/llmProvider.ts`（從 SayIt 學）：
  - [ ] `LlmProviderId = "groq" | "openai" | "anthropic" | "gemini"`
  - [ ] `LLM_PROVIDER_LIST`：4 entries with `baseUrl`、`consoleUrl`、`apiKeyPrefix`、`apiKeyHeaderStyle`
  - [ ] `buildOpenAiCompatibleFetchParams(req, key)` — Groq + OpenAI（注意 `max_tokens` vs `max_completion_tokens`）
  - [ ] `buildAnthropicFetchParams` — extract `system`、`x-api-key`、`anthropic-version`
  - [ ] `buildGeminiFetchParams` — `/models/{model}:generateContent`、`x-goog-api-key`
  - [ ] `parseProviderResponse` dispatcher
  - [ ] Per-provider timeout：groq=5s、其他=30s
- [ ] `src/lib/modelRegistry.ts`：
  - [ ] `LLM_MODEL_LIST` — 至少 8 models（每個 provider 1-3 個）
  - [ ] `WHISPER_MODEL_LIST` — Phase 1 只 cloud `whisper-large-v3-turbo`
  - [ ] Helper：`findLlmModelConfig`、`getModelListByProvider`、`getDefaultModelIdForProvider`
- [ ] `src/lib/enhancer.ts`：
  - [ ] `enhanceText(rawText, providerId, modelId, vocabulary, options)`
  - [ ] System prompt（zh-TW + en 兩版）：「把口語轉書面語、去贅詞、修標點」
  - [ ] Vocabulary 注入：`<vocabulary>t1, t2, ...</vocabulary>`（max 50 terms）
  - [ ] AbortSignal + timeout
  - [ ] `stripReasoningTags` for `<think>...</think>`
  - [ ] `EnhancerApiError(statusCode, statusText, body)`
- [ ] **Important**：API key 從 Rust keyring 讀，不是 frontend store。Frontend 呼叫 invoke 拿 key（或 invoke wrap-call let Rust 直接 fetch — 評估）
  - **決策**：Phase 1 走 invoke 拿 key（簡單）；Phase 2 評估改 Rust-side fetch（更安全）
- [ ] `useVoiceFlowStore` 加 enhancement 步驟：
  - [ ] After transcribe success → 如果 polish ON → `transitionTo("enhancing")` → enhanceText → paste
  - [ ] Polish OFF → 直接 paste
- [ ] Settings UI：
  - [ ] LLM polish toggle Switch（**預設 ON**）
  - [ ] Provider Select（同 Whisper provider，但獨立選擇）
  - [ ] Model Select 過濾 by provider
  - [ ] Test 按鈕：送 dummy text、show polished output
- [ ] Vitest tests：`enhancer.test.ts`、`llmProvider.test.ts`
- [ ] CSP + capability 加其他 3 個 provider URL

### Acceptance criteria

- ✅ Settings 切 4 個 provider 都 work
- ✅ 中文「呃這個就是我覺得啊」polish 後變「我覺得這個還可以」之類
- ✅ Polish OFF → 原始 Whisper 文字直接 paste
- ✅ Polish 失敗（network / rate limit）→ fallback to 原始文字 + 顯示 warning
- ✅ Vitest unit coverage > 70% on `lib/enhancer.ts` 與 `lib/llmProvider.ts`

### 預估時間：1 週

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
