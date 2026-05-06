# Ideas / 想法 parking lot

> 不屬於當前 milestone 的想法、改進建議、待考慮事項。
> 進入某個 milestone 時可以掃過、把相關項目搬進 milestone 的 plan 或 session log。

## 待考慮（M1 收穫，M2+ 用得到）

- **HUD `visible: false` 讓 M1 視覺驗證很彆扭** — 每次想看 HUD 要手改 `tauri.conf.json`。考慮 M5 一起處理：
  - 加一個 dev-only env var（`TALKTYPE_DEV_HUD=1`）強制 HUD `visible: true`
  - 或者 dev-only Tauri command（`set_hud_visible_for_dev`）方便冒煙時手動 toggle
  - 或者在 dev mode 下 HUD 直接 visible（`#[cfg(debug_assertions)]`）
- **Tauri `beforeDevCommand: "pnpm dev"` PATH 踩雷** — Tauri child process 找不到 `pnpm`（M0 已知，M1 chunk 3 重現）。三個選項：
  1. 寫進 README（最簡單）
  2. 改成 `node ./node_modules/vite/bin/vite.js`（移除 pnpm 依賴，但較 ugly）
  3. 在 `package.json scripts` 加一個 `dev:tauri` wrapper 設好 PATH
  選 1 最簡單，M9 release prep 前一定要做。
- **Pong listener leak 防護**：M1 chunk 3 在 IpcSmokeTest 與 HudOverlay 都用 `onMounted` register + `onUnmounted` unlisten 模式；以後 composables 寫成 helper（`useEventListener(name, handler)`）統一處理 unlisten 是否值得？M2 加 `useAudioWaveform` 時順手做。

## 待考慮（M1+ 也許用得到）

- **`.vscode/extensions.json` 是否要 commit** 給未來 contributors？目前 `.gitignore` 把 `.vscode/` 整個 ignore。Pro：新手 clone 進來自動得到 Volar / tauri / rust-analyzer recommendations。Con：可能撞到別人偏好。
- **`vitest.config.ts` 用 `mergeConfig`** 共用 `vite.config.ts` 的 alias / plugins（避免 drift）— 目前重複只有 2–3 行，未來 vite config 變複雜時再做。
- **ESLint `--max-warnings`**：目前 25（baseline 18 + headroom）。M1 改寫 `App.vue` 後 baseline 會降到接近 0，可以收緊到 5。
- **`tsconfig.json` 拿掉 `baseUrl` + `ignoreDeprecations: "6.0"`**：實驗證明 paths 在 TS 6 沒 baseUrl 也能 work。等 M1 / M2 開發中順便實測 + 拿掉。
- **`tsconfig.node.json` `include`** 只 cover `vite.config.ts`，未來 `vitest.config.ts` / `playwright.config.ts` / `eslint.config.js` 也許該加進去（cosmetic 不急）。

## 想到的 process / DX 改進

- **`gh` CLI 應該加進 README dev requirement** — Phase 1 文件說自己 build，但 CI 互動需要 gh，沒裝會卡。
- **Subagent dispatch pattern 在 M0 證實 work**：sequential chunk + per-chunk reviewer + 最後 E2E。對下個 milestone（M1 雙視窗 / IPC）應該也適用，但 M4（hotkeys）/ M7（whisper.cpp）等高風險 milestone 可能需要 main agent 多介入 debug。
- **Hooks 在 setup session 不會 fire**（`.claude/settings.json` 在啟動後才寫進去）— 下個 session 會自動拿到。值得在 README 寫一句「修 hooks 後重啟 session」。

## 待考慮（M2 收穫，M3+ 用得到）

- **`audio_recorder/mod.rs` 拆 `commands.rs`**：目前 418 行（8 行超過 400 budget），M3 加 transcribe* 之前順手把 8 個 `#[tauri::command]` 函式抽出去；mod.rs 只留 state + helper + tests。
- **`tokio::task::spawn_blocking` for `fs::write` in `save_recording_file`**：當 WAV 檔變大（M3 cloud + 30s+ 錄音 → ~1MB+）時，sync `fs::write` 會擋住 tokio runtime。M9 polish 階段量測一下 latency，超過 50ms 就改 `spawn_blocking`。
- **macOS Phase 2：cpal 同裝置雙 stream race**：M2 的 `AudioRecorderState` 與 `AudioPreviewState` 完全分離，Windows WASAPI 允許同裝置兩個 input stream、macOS CoreAudio 不允許。Phase 2 macOS 需要：preview 在 record start 時自動 stop（cross-state shared shutdown signal、或單一 trait + 一個 mutex 包兩個 state）。
- **`<AudioRecordTest>` + `<IpcSmokeTest>` 用 `import.meta.env.DEV` gating**：M9 release prep 時統一加 `<template v-if="isDev">` wrapper（或直接 conditional import），避免 dev-only smoke 卡片進 production bundle。
- **Vite-only mode `invoke` error UX**：M2 chunk 3 reviewer 點出 `SettingsView.vue` 在 vite-only 模式（`pnpm dev` 不開 Tauri runtime）會把「Cannot read properties of undefined (reading 'invoke')」當作紅色錯誤訊息直接 render 給 user 看。**較友善的解**：用 `if (window.__TAURI_INTERNALS__)` 或類似 Tauri runtime 探測，在 vite-only 改顯示「需要 Tauri runtime（pnpm tauri dev）才能列出裝置」。或者用 `<ErrorBoundary>` 包起來。M9 polish 一起處理。

## M2 retro challenger findings（2026-05-03）

> 由 retrospective challenger subagent 對 M2 5 個 commits + session log 反向審視產出。已標記 priority（M3 = 立刻、M5 = HUD 整合時、M9 = polish 階段、Phase 2 = v0.2+）。

### Privacy / Security（M9 unless noted）
- **WAV 檔 plaintext on disk forever**：`auto_cleanup_recordings_enabled` 預設關。Privacy 隱憂（laptop 遺失 = 對話外洩）。M9 加 user 可見的 retention banner、考慮 default on。
- **單筆刪除 command 缺**：只有 `delete_all_recordings`、user 不能刪單筆。**M3 加 `delete_recording(id)` command**。
- **mic device name 可能含 PII**（"John's AirPods"）：當前 stderr 會印。M9 mask / redact。
- **Recordings dir ACL**：繼承 user-profile ACL、no encryption-at-rest。Phase 2 評估 DPAPI 加密（已在 plans/05-data-model 註記）。

### UX gaps
- **Mic permission denial**（第一次 cpal stream open Windows 會問）未處理：user 看到 raw `BuildStream` error。M5 加 detection 與 friendly fallback。
- **`togglePreview` 非 idempotent**（in-flight invoke 期間雙擊 race）：`SettingsView.vue:76`。**M3 加 `inFlight` ref guard**（small chunk 一起做）。
- **裝置切換時舊 preview 沒先 stop**：dropdown 改新裝置 → 應該自動 `stop_audio_preview` 再 `start`。**M3 修**。
- **錄音長度 UI**：當前無「快到 25 MB cap」警告。**M3 加 size approaching warning**（搭配 `MAX_WAV_BYTES` 統一處理）。
- **「存成 WAV」沒有「打開資料夾」affordance**：user 不會 navigate 到 `%APPDATA%`。M9 加 button。

### Production failure modes
- **Mic unplug mid-recording**：cpal `err_fn` 只 `eprintln!`、recording 繼續寫零、無錯誤 propagation。**M3 加 error → state propagation + `audio:recording-aborted { reason: 'mic_unplug' }`**。
- **`cleanup_old_recordings(0)` edge case**：cutoff = UNIX_EPOCH 結果不刪、行為違反直覺。**M3 加 unit test + 文件說明**。
- **Bluetooth profile switch (HFP↔A2DP)**：第一秒資料壞、未處理。M9 文件提醒。
- **同名雙裝置**（兩個 USB mic 都叫 "Microphone"）：name-based 選裝置任意挑一個。Phase 2 加 device ID（cpal 0.16 已 expose）。

### Performance
- **`encode_wav` 同步**（`mod.rs:244`）阻擋 tokio runtime：30 min 錄音 ~115 MB WAV encode 可能拖延。**M3 wrap `tokio::task::spawn_blocking`**。
- **`fs::write` in `save_recording_file`**：同問題，已記。M9。
- **`samples.lock().clone()` at `mod.rs:235`**：clone 整個 i16 buffer（5-min @ 16 kHz = 9.6 MB）。**M3 改 `mem::take`**。
- **Lerp 0.25 → 0.95 in 12 frames = 200 ms** latency at 60 fps。M9 在低階機器 measure。
- **`WaveformProcessor::push_samples` 每 sample pop_front + push_back**：M9 評估 `extend` + truncate-from-front。

### Test coverage（M3 + M5）
- **Zero integration tests** touch real cpal / filesystem。**M3 加 `tempfile`-based test 涵蓋 save/read/delete round-trip**。
- **`stream.pause()` 4 個 teardown path 沒有自動驗證**（只靠人工讀 code）：M5 加 `#[cfg(test)]` mock 或重構為 inject teardown closure。
- **`validate_id` 只測 UUID-shaped**：path-traversal-like input 沒 fuzz。**M3 加 property test**。
- **Repeated start/stop cycles 無 memory-leak test**：M9。

### Documentation / observability（M3 ASAP）
- **`SECURITY:` log convention 沒寫進 CLAUDE.md**：未來 subagent 可能 strip 掉。**現在加進 CLAUDE.md（搭配 chunk-3 commit）**。
- **`SECURITY:` log 在 release build 等於 /dev/null** — `eprintln!` 沒 capture。**M3 加 `audio:mic-safety-warning` event 給 frontend**（M2 retro #2 critical）。
- **Windows mic-permission flow 沒進 README**：M3 收尾時補。
- **無 metric 收集**：M9 polish gate 沒可量化指標。

### Architectural debt
- **`audio_recorder/mod.rs` 已 418 lines**：M3 加 transcribe* 會破 500。**M3 第一個 chunk 就拆 `commands.rs` 出來**（已加進 M3 task list）。
- **`AudioRecordTest.vue` + `IpcSmokeTest.vue` 在 `src/components/` 混在 production**：M9 移到 `src/components/dev/`。
- **Capabilities 仍 grant `core:default`** 寬鬆：M9 加 permission audit。

## 待考慮（M3 chunk 3 收穫，M4+ 用得到）

- **`AudioRecordTest` + `IpcSmokeTest` dev-only gating**（reinforce from M2 IDEAS）：M3 chunk 3 確認「測試轉錄」button 是 dev tooling，M9 release prep 一定要 `import.meta.env.DEV` gate 起來、避免進 production bundle。M2 已 flag、再次確認 M9 必做
- **Test connection 5s auto-clear 是否要 configurable？**：當前 hardcode `TEST_RESULT_CLEAR_MS = 5_000`，理由是「夠看完不擋下次測」。dogfood 期收 user feedback：有些人想要結果 stick 直到下次測 / 切 provider；可能加 settings `test_result_persist: 'auto-clear-5s' | 'until-action'`，M8 settings UI 拓展時看
- **`pub use health::test_provider_connection;` re-export 沒被 `tauri::generate_handler!` 接受**：chunk 3 踩雷後 `lib.rs` 改寫 `transcription::health::test_provider_connection` 直連，但 `mod.rs` 仍 keep `pub use` 給 Rust 內部用。考慮把所有 commands 都從 `transcription/commands.rs` 模組統一 export（學 audio_recorder 結構），M6 加 `polish_text` 時順手做
- **`TestConnectionError` vs `TranscriptionError` taxonomy 重複**：兩 enum 都有 `Network*` + `RateLimited` + `ApiError`，M6 加第三組 LLM polish 又要再 copy。M6 可以抽 shared `enum HttpProviderError` trait + 各 module 拼自己 data-layer 變體，避免 3 份 maintenance
- **Privacy disclosure dialog 第一次顯示時機**：當前是「`hasCredential === false` BEFORE save」觸發、覆寫已存 key 不再 show。dogfood 觀察 user 換 key 是不是想再看一次政策；可能 v0.2 加「永遠 show」option 給 enterprise compliance
- **AudioRecordTest 「測試轉錄」按完後留 result，但下次「開始錄音」會清掉**：是預期行為（新錄音要清舊結果）。但 user 可能想對同一筆錄音重複 transcribe 比 vocabulary diff — 目前 first transcribe 成功就 take() WAV buffer 沒了。考慮 M8 history view 加「retranscribe」按鈕從 SQLite 重讀 WAV 檔
- **TranscriptionError → vue 訊息 mapping 是 string-match**：`formatTranscribeError` 用 `raw.startsWith("Audio too small")` 等。如果 Rust `Display` 改字串、UI 訊息會默默 fallback 到 unknown。M9 polish 考慮 Rust 加 `error.code: String` 機器可讀欄位（不影響 user-friendly Display）給 frontend match
- **「測試轉錄」按鈕 i18n 文字當 status === 'transcribing' 時直接覆寫**（`t("transcribing")` vs `t("transcribe")`）：目前同一 button text 會跳動，改用 statusLabel 顯示更清楚？M9 UX polish
- **`SettingsApiKeySection.vue` 506 行（軟 budget 500 line）**：M3 chunk 3 加 test connection 後超過 6 行。內容有 cohesion（單一 settings section）但可拆 `useApiKeyTest` composable（test-connection state + formatTestError） + `useApiKeyForm` composable（save/delete + privacy dialog flow），SFC 只剩 wiring + template。M8 Settings 拆 sub-components 時順手做（roadmap 已規劃）
- **`::-ms-reveal` CSS 從 scoped 搬 global**：M3 polish 把雙 password reveal icon 修在 `SettingsApiKeySection.vue` 的 `<style scoped>`。等 M9 polish 順手把 rule 搬進 `src/assets/index.css` `@layer base`、未來任何新 password input 自動受惠。**搬時刪掉 SettingsApiKeySection.vue 的 scoped 那塊**避免重複。
- **Real-Groq smoke test under `--features` flag**：本次 session user 提案把 key 放 `.env.local` 給 automated tests 用。當下決定不做（wiremock 31 cases 已涵蓋、real-Groq 是 manual job），但**未來如果想加「Groq API contract 沒改」regression test**：用 `cargo test --features real-groq-smoke` gate、key 從 `GROQ_API_KEY` env var 讀、CI **不**跑、本地 dogfood 才跑。M9 polish candidate（評估 ROI）。

## M4 plan-time challenger P2（2026-05-05 — 不在 M4 scope、留下次）

> 由 M4 開工前 plan-time challenger subagent 找出、main session 對照 plan 後判定優先級。P0 與 P1 已落實在 M4 chunks（修計畫不修 code）；以下 P2 不阻擋 M4 ship、但是 M5 / M9 / Phase 2 候選。

- **多螢幕 + DPI 縮放下 HUD 位置（M5 owns）**：HUD x=center y=50 在 mixed-DPI（4K 主 + 1080p 副）只 cover primary monitor、user 在 secondary monitor 工作會看不到 HUD — 但 paste target 判斷靠 `GetForegroundWindow()`、不依賴 HUD 位置、所以 paste 本身沒 bug。M5 HUD overlay 完成時順手做 multi-monitor + DPI-aware positioning。
- **藍牙鍵盤 down/up 延遲對 double-tap detection（M5 dogfood）**：藍牙 keyboard 透過 LL hook 收到 batched events、350ms double-tap window 在實機可能不夠（藍牙延遲可達 200-300ms）。M5 dogfood 期收 telemetry 後 tune 到 500ms 或變成 settings。
- **Hook log keystroke leakage（M9 polish）**：M4 不接 Sentry（Phase 2 才接），但 chunk 1 implementer 若 `println!` debug 印具體 keycode、user 輸入密碼框時可能 leak 進 dev console。M9 audit `hotkey_listener` 所有 `println!` / `eprintln!`、確保只 log「triggered」+ summary、不印具體 vk / keycode / modifier。Phase 2 接 Sentry 時尤其重要（breadcrumb leakage）。
- **`SetForegroundWindow` 跨 virtual desktop 切換語意（M5 dogfood）**：user 按熱鍵時 target 在 virtual desktop A、轉錄期間 user 切到 desktop B、paste 時 target HWND 還在 A。`SetForegroundWindow` 行為取決於 Windows build（部分 build 自動切 desktop、部分拒絕）。M5 dogfood 觀察是否要 fall back 到 emit `paste:focus-restore-failed` event。
- **`AttachThreadInput` thread id 在 user 切視窗 race（M5 dogfood）**：`capture_target_window` 在熱鍵 down 時 capture HWND、user 在錄音中切到別 window、paste 時 attach 的是舊 HWND。SayIt 接受「user-intent locked at hotkey-press time」設計。M5 dogfood 看 user feedback 是否要改 paste-time re-capture（trade-off：re-capture 可能撞到 HUD 自己 vs 鎖在熱鍵時 user 預期）。
- **Antivirus / EDR 對 `SetWindowsHookExW(WH_KEYBOARD_LL)` 的反應（Phase 2 distribution）**：keyloggers 用同 API、企業 EDR（CrowdStrike / SentinelOne / Defender for Endpoint）會把全鍵盤 hook 標 alert / block。M4 不能 fix；Phase 2 distribution doc 加「企業環境可能誤判」警語、考慮 v0.2 加 code-signing 後 enroll Microsoft SmartScreen reputation。

## M4 chunks 1+2 reviewer findings（2026-05-05 — P1 #1+#3 已修、其餘留下次）

> 由 M4 chunks 1+2 完成後的獨立 code reviewer subagent 找出。P1 #1 (TaskPanic mislabel) 與 P1 #3 (SetWindowLongPtrW return ignored) 立即修了；P1 #2、P1 #4、P2s 留以下：

- **`paste.rs` 768 LOC > 500 軟性 budget — split into `paste/{commands, com, attach, modifiers, ime, input}.rs`（M4 chunk 4 後做、不阻擋 ship）**：CLAUDE.md「檔案大小」軟規則。實作完整 + 充分測試後再拆比較不會 churn。Chunk 4 完成 manual acceptance 後做一次 focused refactor commit。
- **`OnceLock<HookContext>` 阻擋 process lifetime 內任何 re-install（Phase 2 polish）**：`hotkey_listener/windows.rs` 用 `OnceLock` 存 `HookContext`、`shutdown` drop `HookHandle` 但**沒清** `OnceLock`。Phase 1 single-instance 不會 re-install 不影響、但 Phase 2 dev hot-reload / 測試 harness 需要 re-install 時會 reject。考慮換 `static SHARED_CTX: Mutex<Option<HookContext>>` 支援 `take()` on shutdown。
- **Slow-hook diagnostic threshold 50ms 在 dev mode 可能噴 log（M5 dogfood）**：`hotkey_listener/windows.rs:347` 警告 `app.emit > 50ms`。Vue HMR 期間 webview 可能 hung、每個 keystroke 觸發。M5 dogfood 看 log frequency；若太多改 200ms threshold 或加 rate-limit `AtomicU64 last_warn_ts`。
- **`apply_event` 回 `Vec<HotkeyEvent>` 但 Phase 1 永遠 0 或 1 個（micro-opt 候選）**：`hotkey_listener/shared.rs:145`。Phase 2 evaluate 改 `Option<HotkeyEvent>` 或 `SmallVec<[_; 1]>`。Trivial、defer。
- **`hook_proc` 尾端有一個 wasted atomic load（trivial）**：`hotkey_listener/windows.rs:404` `let _ = state.is_pressed.load(...)` 是 reserved-for-future tracing comment、目前是浪費的 atomic load。下次 touch 該檔時刪掉或實際用。

## M4 chunk 3 reviewer findings（2026-05-05 — 0 P0 / 0 P1、僅 P2）

> 由 M4 chunk 3 完成後的獨立 code reviewer subagent 找出。chunk 3 settings.rs + useVoiceFlowStore + lib.rs wiring 全 pass、CLAUDE.md 全合規、只有 2 個 P2：

- **Async listener registration race in `useVoiceFlowStore.init()`（M5 owns）**：`useVoiceFlowStore.ts:206-241` 用 `void listenToEvent(...).then(unlisten => unlistenFns.push(unlisten))`。`listenToEvent` 是 async 回 Promise、events fired between `init()` 呼叫與 Promise resolve 之間會丟失。Phase 1 HUD bootstrap 比 user reflex 快、不會踩；M5 加 HUD visual states 時 consider 改 `init()` 回 Promise + await all `listen` registrations 才宣告 ready。
- **`PASTE_FOCUS_RESTORE_FAILED` 已 active state collision（M5 owns）**：`useVoiceFlowStore.ts:230-241` 若此 event 在 `transcribing` 中發 (極少見的 race：paste 失敗剛好撞上下一個 HOTKEY_PRESSED 開新 flow)，會把已 active 的新 flow state 清成 `error`。Phase 1 paste ordering 幾乎不可能踩；M5 HUD 擁有 visual state 時要處理 collision (e.g. only transition if status === 'recording'/'transcribing' for THIS paste cycle)。

## M5 plan-time challenger findings（2026-05-05 — 不在 M5 scope、留下次）

> 由 M5 開工前 plan-time challenger subagent 找出（agentId `abe35088f7a648bf9`）。P0 + P1（除以下）已折進 spec + plan、main session 修計畫不修 code。以下 P1-2 + 3 個 P2 留 M9 / Phase 2。

- **P1-2：Vitest 測 `<Transition mode="out-in">` 在 jsdom 不可靠（M5 chunk 2 implementer 認知）**：jsdom + Vue 3.5 transition hooks 多半 sync、無 real animation。M5 chunk 2 vitest tests 只測 state→class mapping after `nextTick()`、transition timing 留 Playwright 驗證；implementer 要知道別跟 jsdom 的 transition timing 對打。
- **P2-1：Dashboard sidebar 只顯示 `recording`、不顯示 `transcribing` / `error`（Phase 2）**：M5 簡版 — user 按熱鍵走開、轉錄期 / 失敗時 sidebar 沒回饋。Phase 2 加多 state badge / tooltip。
- **P2-2：`aria-live="polite"` 連續同訊息 SR 不 announce（Phase 2 a11y polish）**：rapid hotkey press → recording → transcribing → recording → transcribing 第二輪 SR 可能略過。Phase 2 用 dummy aria-label change（加無意義 trailing space 或變數）強制 announce。
- **P2-3：HudSpinner 30 LOC 可考慮 inline 進 HudOverlay template（cosmetic）**：4-component 切分對 30 LOC 的 spinner 略 over-engineered；chunk 2 implementer 可自行決定 inline 與否、無強制。

## M5 chunk reviewer findings (2026-05-05 — chunk 2 & 3)

> 由 M5 chunks 2+3 完成後 reviewer subagent 找出。Chunk 1 reviewer P0（formatError pattern）已 fix in `6af0992`；以下 chunk 2 reviewer P1-3 + chunk 3 reviewer P2 留下次。

### Chunk 2 reviewer
- **P1-3 docstring gap**：`useAudioWaveform.start` `starting` flag docstring 沒提「rejection 後 retry」semantics — 加 1-line clarification
- **P1-5 fontsource preload for HUD entry (FOUT mitigation)**：`dist/index.html` HUD 載 Geist via fontsource、woff2 lazy load 後 CSS parse 後可能 FOUT。M9 polish 加 `<link rel="preload" as="font">`。

### Chunk 3 reviewer
- **P2-1 `SidebarFooter` empty wrapper artifact**（M9 polish）：idle 狀態下 `<SidebarFooter>` wrapper 仍 render 一個 padding 空 band；hoisting `v-if` 進 `AppSidebar.vue` parent 即可解。M9 dark mode polish 時順手做。
- **P2-2 i18n key duplication**（M9 i18n consolidation）：`sidebar.recordingBadge` 與 `dashboard.audioTest.recording` 都是 "錄音中"；M9 i18n audit 時 consolidate。
- **P2-3 `role="status"` on Dashboard badge 可能 SR 重複 announce**（Phase 2 a11y）：HUD + Dashboard 同時 `role="status"` 念兩次「Recording」；Phase 2 a11y testing 驗、可能改 Dashboard badge 為 `aria-hidden="true"`。
- **P2-4 listener 沒 future-proof `message` field**（Phase 2 tooltip）：`HudFlowBadge.vue:41` 只讀 `payload.status`、若 Phase 2 加 tooltip 顯示 HUD message text、要回頭改 listener。
- **P2-5 `main-window.ts` shim duplicated against `main.ts`**（M9 polish）：兩個 vite-only Tauri shim 重複；考慮抽 `src/dev/tauri-shim.ts` 共用 factory。

## M5 retro challenger findings (2026-05-05)

> 由 M5 完成後的 retro challenger subagent（CLAUDE.md item 6 強制）獨立 review 整合後 codebase + 跑自己的 Playwright pass 找出。Chunk reviewers 抓 chunk-level、retro challenger 抓 cross-chunk integration + 整體性問題、漏抓的 latent 問題。Playwright 截圖列在每條後（`.playwright-mcp/m5retro-*.png` 共 14 張：idle / recording / transcribing / success / error-long / error-short / error-dismissed / warn-yellow / warn-red / rm-recording / rm-transcribing / dashboard-idle / dashboard-recording / dashboard-transcribing）。

### P1 — long error message 截尾**會吃掉「（請手動 Ctrl+V）」hint**（M9 polish）

`src/components/HudOverlay.vue:163-172` `.bubble-label` 設 `text-overflow: ellipsis; max-width: 280px`。chunk 1 P0 fix 把 friendly hint「（請手動 Ctrl+V）」append 到 raw Rust error 字串**末尾**，但 ellipsis 從末尾截、所以 `Failed to restore focus to target HWND 0x12345678: GetLastError=5` 之後的 hint **完全 invisible**。Playwright `m5retro-05-hud-error-long.png` 證實：截為「Failed to restore focus to target HWND 0...」、user 永遠看不到要手動 Ctrl+V 的指示。修法選項：(1) reverse pattern — hint 改 prepend 到字串前面：「（請手動 Ctrl+V）<raw error>」、ellipsis 截掉技術 detail 而非 hint；(2) HudOverlay 改 2-line bubble allow `white-space: normal` + height: auto、bubble 高度動態；(3) 把 hint 拆成獨立 `<span>` 永遠可見、raw error 才 ellipsis。M9 polish 必修 — chunk 1 reviewer P0 fix 形同失效。

### P1 — recording state「6 條 bar」在 vite-only / 無音訊輸入時看起來像 reduced-motion 的 dot fallback（M6 / M9 polish）

`src/components/HudWaveform.vue:38-43` 的 6 個 `<span>` 每條 `w-1 rounded-full` (4px 寬 + 圓形)、初始 `height: 4px`、`gap-1`（4px 間距）。waveform 沒收到 audio data 時 6 個 4×4 圓點等距排成水平、視覺上**幾乎跟 reduced-motion 的單一 dot 沒差**（Playwright `m5retro-02-hud-recording.png` 對 `m5retro-10-hud-rm-recording.png` 對比看不出明顯差異）。M5 chunk 2 implementer 沒踩 — vitest 測 6 個 element 存在、但「形狀對」沒 cover。修法：(1) 初始 height 改 8px（對 reduced-motion 保留 4px dot）；(2) 改用 `w-0.5` (2px) 細條 — 讓 4px height 也看起來是 vertical line；(3) 用 base height 6px + per-bar phase-shifted CSS animation（idle pulsing）製造「等待 audio」視覺。M6 LLM polish 預留 enhancing state 時順手做 base-state 動畫 fallback。

### P1 — 200ms `<Transition mode="out-in">` 與 `setIgnoreCursorEvents` await 沒同步、real Tauri runtime race window（M9 polish）

`HudOverlay.vue:65-70` `watch(store.status)` 觸發 `syncClickThrough(next)`、async function 直接 `await getCurrentWindow().setIgnoreCursorEvents(...)`、跟 `<Transition>` 進場/出場 200ms 平行跑、無同步。失敗模式：error → idle 切換瞬間 — Vue Transition 還在 fade out（200ms）+ `setIgnoreCursorEvents(true)` IPC round trip（Tauri ~10-50ms）— user 在 200ms 視窗點 HUD bubble，OS 已切 click-through-on，用戶 click 穿透而非 dismiss。Playwright vite shim 不會 reproduce（IPC 是 sync no-op）。實機 Tauri runtime 也許 IPC 比 Vue transition 快、不會踩 — 但沒驗。M9 dogfood 觀察 user 是否 report「點 dismiss 沒動 / 點到下層 app」、若有則改 Vue Transition `@before-leave` hook 等 `setIgnoreCursorEvents` resolve 才 leave。

### P1 — `audio:waveform` listener 在非 recording state 仍跑 RAF、浪費 frame（M9 perf）

`HudWaveform.vue:18-20` `onMounted` 立刻 `start()`、`onUnmounted` 才 `stop()`。但 `HudWaveform` 是 `<HudOverlay>` `<Transition mode="out-in">` 內的 conditional child — recording → transcribing 切換時、Vue **先 leave recording** (`HudWaveform` unmount → stop) **再 enter transcribing**。OK。但 `<HudOverlay v-if="visible">` 整體 unmount 時（idle）所有 child unmount → stop。但 chunk 2 改用 `watch(reducedMotion)` start/stop、沒對應 `watch(store.status)` start/stop — 也就是說 transcribing / success / error state 期間 `useAudioWaveform` listener **依然 active**（雖 component unmounted、但是 listener 是 module-level 還存活到 stop call）— 實際上 OnUnmounted handle 對。等等 — 確認：`HudWaveform` 是 inner `<div v-if="store.status === 'recording'">` 子節點、status 切離 recording 時 inner div unmount → HudWaveform unmount → stop()。OK，但 `<Transition mode="out-in">` 會延遲 unmount 到 leave 動畫結束（200ms）— 200ms 期間 RAF tick + `audio:waveform` listener 仍 active 但 useless（user 已放熱鍵、Rust 不再 emit）。M9 polish 評估「if status !== 'recording' 立刻 stop()」是否值得 chunk-by-chunk reactivity。

### P1 — 短音訊 success state 1s linger 太短、user 看不到 ✓ 的視覺確認（M6 dogfood / M9 polish）

Spec §2 + `useVoiceFlowStore.ts:74` `SUCCESS_LINGER_MS = 1000`。Playwright `m5retro-04-hud-success.png` 看 1 秒 user 才剛把眼睛從 paste target focus 移到 HUD bubble、HUD 已 fade out 200ms 進 idle、實際可見 success ~600-800ms。M6 LLM polish 加 enhancing state 後流程更長、user 對 success 視覺確認需求更高。建議 M6 同步 bump 到 1.5s 或加 user-config（settings `hud.success_linger_ms`）。M9 dogfood 觀察。

### P1 — `prefers-reduced-motion: reduce` ON 時 6-bar fallback 跟 idle waveform 形狀**完全相同**（單一 dot），fallback 是否真有用值得質疑（Phase 2 a11y polish）

Reduce-motion 顯示一個 4×4 dot + label；non-reduce-motion 在沒 audio 時顯示 6 個 4×4 dot — 視覺上幾乎沒差別。User 開啟 reduced-motion 是想避免「動畫」、但這個 fallback 對「資訊量」沒幫助 — 6 條 vs 1 條只是減量、不是「不同視覺語言」。Spec §5.2「改顯示單一灰色 dot + 「錄音中」label」其實想加文字 label 但 implementer 只加了 dot 沒加 label（Playwright `m5retro-10-hud-rm-recording.png` 確認只 dot + timer、沒「錄音中」label）。修法：reduced-motion fallback 加文字「錄音中」+ dot、跟 non-reduced-motion (waveform + timer) 視覺有意義差異。Phase 2 a11y testing 階段必修。

### P1 — `transitionTo()` 改 async + `emitTo` await + `handleStart` `await transitionTo("recording")` — IPC failure 阻擋 status mutation 嗎？（M9 perf）

`useVoiceFlowStore.ts:293-309` `transitionTo` `status.value = next;` **先設 local ref**、再 `await emitTo(...)`。`emitTo` 失敗只 console.warn、不 throw — 所以 local status 一定 mutate。但 `await emitTo` 在 IPC 卡住（Tauri 內部 channel 滿、極罕見）期間 **caller 整個 chain block** — `handleStart` 的 `await transitionTo("recording")`、若 emitTo 卡 100ms、user 看到 100ms 的 visual lag。Spec §6.1 的 try/catch 語意正確、但 `await` 順序對 latency 不好。修法：先 sync 設 status / message、然後 `void emitTo(...)` 不 await（emit 失敗只 warn 不 break）— 或者更乾脆 wrap `emitTo` 在 `Promise.race(emitTo(...), timeout(50ms))` 給上限。M9 polish。

### P1 — Pinia `readonly()` unwrap 在 HudOverlay `store.recordingStartedAtMs` — props pass 是否 reactive 已驗（M6 verify）

`HudOverlay.vue:112` `<HudTimer :started-at-ms="store.recordingStartedAtMs" />` — `store.recordingStartedAtMs` 是 `readonly(ref)` 包裝、Pinia getter 會自動 unwrap、傳給 child 是 `number | null` value。HudTimer `defineProps<{ startedAtMs: number | null }>()` + `props.startedAtMs` reactive 讀。看起來對。但 M4 chunk 3 implementer 對「readonly + props 在 Pinia getter 邊界 reactive 行為」有過 doubt（IDEAS 記過）。M5 vitest 沒 cover「mid-recording 期間 startedAtMs 不變、timer 跑」這個 scenario — chunk 2 vitest 是用 fake timers 直接設 prop 測色彩、沒測 reactive 邊界。M6 第一個 reviewer 用真 Pinia store + `recordingStartedAtMs` mutation 寫 1 個 vitest regression test。

### P1 — HUD click-through 在 `success` state 是 ON、但 `success` 是有 ✓ icon + 「完成」label 的可點擊形狀、user 直覺以為可點 dismiss / 加快 fade idle（M6 / M9 UX polish）

Spec §2 表格定 success 是 click-through ON，user 點不到。但設計上 success = 「程序完成、HUD 1s 後消失」、user 點是想加速消失（impatience）。目前點 success bubble 會穿透到下層 app、user 困惑「為何 HUD 擋住卻又點不到」。Playwright 已驗 click-through ON 但無法 reproduce 多視窗點透 perception 問題。建議：success state 也允許 click → 立即 idle（透明對等於 error path 的早期 dismiss 概念）；或 click-through OFF + click → idle。M6 dogfood 評估。

### P2 — `audio:recording-aborted` 在 `transcribing` state 收到時 — handleError 直接 transition 到 error，但 transcribing pipeline 還在 in-flight、Rust transcribe 仍會回 result → `handleStop` 會嘗試 `pasteTextSerial`（M9 race fix）

User flow：press hotkey → recording → 25 MB cap → Rust emit `audio:recording-aborted` → vue listener 走 `handleError(...)` 立即 transition error。但同時 `handleStop` 已被 hotkey-up 觸發、`stop_recording` + `transcribe_audio` + `pasteTextSerial` 都還在 await 中、`mySession === currentSession` 仍 true（recording-aborted 沒 bump session）— transcribe 完成後 `pasteTextSerial(result.rawText)` 會 paste 一個截尾 audio 的 garbage transcription、然後 `transitionTo("success")` 會 clobber `error` state、最後 user 看到 success 但 paste 的是垃圾。修法：`handleError` 從 abort listener 走時 bump `currentSession` (++) 讓 in-flight handleStop 自己 bail。或 abort listener 加 `aborted: true` flag + handleStop 檢查 flag skip paste。M9 race fix。

### P2 — `useAudioWaveform.starting` flag 沒 reject 處理、若 `await listenToEvent` reject 則 flag 永遠 true、後續 start 全 short-circuit（M9 robustness）

`useAudioWaveform.ts:50-67` `starting = true; try { ... } finally { starting = false; }` 是對的、try/finally 包 await 所以 reject 也會 reset flag。但 chunk 2 reviewer P1-3 docstring gap（IDEAS 已記）只說「rejection 後 retry semantics 沒寫清」— 確認 code 是對的、only 文件待補。

### P2 — Dashboard sidebar 在 voice flow 中途 mount（user 開啟 Dashboard window）— 不會看到「正在錄音中」直到下一個 transition（spec §6.3 known limitation；UX gap）

Spec 已 documented 但實際 dogfood 體感：user 在錄音中突然想開 Dashboard 看歷史 / 切設定、開啟後 sidebar badge 是 idle、user 困惑「我明明在錄音」。M9 加 mount-time `request_current_voice_flow_state` Tauri command + Dashboard mount 時 invoke 一次 sync 當前狀態。Phase 2 已 deferred、但 dogfood 需求可能比想像高。

### P2 — HUD bubble `bg: var(--card)` 在 dark mode 沒驗、可能跟 OS dark mode + transparent window 撞色（M9 dark-mode polish）

`HudOverlay.vue:155-160` `.bubble` `background: var(--card)`。`--card` 在 dark mode 下會切深色、但 HUD window 是 transparent + alwaysOnTop — dark mode 下 HUD 在白色 app（Notepad）前面 vs 黑色 app（VS Code）前面對比度不同。M5 vite-only 沒測 dark mode。M9 dark mode polish + dogfood 兩種背景驗。

### P2 — `<SidebarFooter>` empty wrapper artifact 已 IDEAS、但 M5 retro 確認嚴重度比想像低（仍 M9）

Playwright `m5retro-12-dashboard-idle.png` vs `m5retro-13-dashboard-recording.png` 對比 — idle 時 footer 是 ~32px padding band、recording 時 badge 取代但同 32px height。所以 idle/recording 切換**不會 layout shift**、視覺感受可接受。但 idle 時看起來像 sidebar 底部莫名留白、新 user 困惑「那塊是什麼」。M9 polish 仍須處理（hoist v-if 進 AppSidebar.vue），但不阻擋 ship。

### P2 — `__hudDev` / `__dashboardDev` 在 production build tree-shake 透過 `import.meta.env.DEV` gate — Vite 確實 tree-shake 還是只 dead-code 留在 bundle？（M9 release verify）

`main.ts:132` 與 `main-window.ts:20` 都用 `if (import.meta.env.DEV)` 做 condition。Vite 4+ 對 `import.meta.env.DEV === false` 的 prod build 會做 dead-code elimination — 但只有 if 整個 block 才會 strip、block 內 referenced symbol 若被外部 import 則 keep。確認方式：M9 跑 `pnpm build` + `grep -r "__hudDev" dist/`、應該找不到。沒驗證過。M9 release verify SOP 加一條 grep。

### P2 — `<Transition mode="out-in">` 在同 status reactive update（recording → recording 但 message 變）會誤觸 transition（spec §17 implementer 留 question）

Spec §17 open question 留給 implementer。實作 `:key="store.status"` 對 status 比較、message 變不會 re-key、不會 transition。對。但 chunk 2 implementer 沒加 vitest 確認此 invariant — 若未來 refactor 不小心把 key 改成 `${status}-${message}` 就會 broken。M6 / M9 加 vitest「同 status 不同 message 不觸發 transition」regression test。

### P2 — HudTimer setInterval drift 在 toggle mode 30+ min 錄音超過 1s 累積誤差（M9 robustness）

`HudTimer.vue:42-46` `setInterval(... 1000)` 不是 RAF、Browser tab 在 background 會被節流。HUD window alwaysOnTop 不應 background、但 user lock screen / display sleep 期間 setInterval 可能 throttle。30 min recording 期間 timer 顯示 vs 實際 elapsed 差幾秒 / 幾十秒、user 看到 timer 還沒到 cap 但 Rust 已 abort。修法：setInterval 改 RAF + Date.now compute、或加 visibilitychange listener resync。M9 polish — Phase 2 行動裝置 battery drain 也類似 concern。


- **macOS dev setup**：目前 doc 只 describe 設計，沒實機跑過。Phase 2 啟動時要做 spike。
- **Sentry telemetry opt-in flow**：要設計清楚的 UI、預設 off、第一次用時的 dialog（學 SayIt 但 SayIt 預設 on，我們改 off）。
- **Code signing**：Phase 2 需要 cert provider（DigiCert / Sectigo / 個人 EV cert），預算考慮。
- **Streaming transcription（候選 milestone `M-streaming`）**：SayIt 沒做、user demand 高但複雜。設計方向（2026-05-03 Q2 用戶 input）：
  - **Chunking + naive concat + LLM polish 當 stitcher（user 提案）**：把錄音切 5-10s chunk + 2s overlap、各自 transcribe、純字串接、靠 M6 LLM polish 處理重複字 / 大寫不一致。**~1.5 週工**（vs forced alignment 的 2-3 週）。LLM polish 能解 95% 邊界問題、剩 5% 是 truncated word + 真正含義斷裂、賭 LLM context 推理。
  - **耦合**：「LLM polish 當 stitcher」要求 polish 永遠 ON、user 關掉 polish 時 raw chunked text 就會暴露 stitching ugly。Phase 2 要評估這個耦合接不接受。
  - **不為 live UX**：streaming + 邊講邊看文字出來還是要 forced alignment / LocalAgreement、~2-3 週、polish 不能解。
- **Per-app adaptive tone**（v0.2+）：Wispr Flow / Typeless 的招牌差異化。M6 已預留 `per_app_preset: HashMap` schema、v0.2 接 `GetForegroundWindow().GetProcessName()` 派 preset。
- **Voice editing commands**（v0.2+ 自己一個 milestone）：「make shorter」「change tone」這類語音指令、Typeless 高評。獨立 milestone candidate。
- **Vocabulary auto-learn**（v0.2）：Typeless 模式、出現 ≥3 次自動加。當前 manual list（M8）不夠 sticky。
- **Per-step data-flow indicator UI**：「Audio → Groq Whisper → OpenAI Polish → Paste（無 retention）」清楚 render 在 Settings。M9 polish。避免 Typeless 那種 marketing 失調翻車。
- **Vite-only mode `invoke` error UX**：`SettingsView.vue` 在 `pnpm dev`（沒 Tauri runtime）時把「Cannot read properties of undefined (reading 'invoke')」直接 render 為紅錯。M9 用 `if (window.__TAURI_INTERNALS__)` 探測 + friendly fallback。

## M6 plan-time challenger findings (2026-05-06 — 不在 M6 scope、留下次)

> 由 M6 開工前 plan-time challenger subagent（agentId `a5769f31fdf47c8c7`）找出 45 findings、main session 對照 plan + spec 後判定優先級。P0 + critical P1 已 fold 進 [`sessions/2026-05-06-m6-llm-polish-kickoff.md`](sessions/2026-05-06-m6-llm-polish-kickoff.md) refined chunks（F1-F33）；以下 P2 + 部分 P1 不阻擋 M6 ship、留 M9 / Phase 2。

### Privacy / Security（M9 unless noted）

- **Vocabulary 多 vendor PII（A3、Phase 2 privacy）**：M3 vocabulary 跟 Whisper prompt 一起送 Groq、M6 加上 OpenAI / Anthropic / Gemini polish prompt — 同 user vocabulary 變成送 4 個 vendor 的 server log。User 加自己名字「張小明」或公司術語進去、4 vendor 都 log。M9 privacy disclosure 必須列出 multi-vendor flow（不只「audio → Groq」）。考慮加 Settings `llm_polish_send_vocabulary: bool` opt-out（預設 true）給 privacy-conscious user。
- **Custom prompt provenance（A4、M9 docs）**：user 從外部 LLM 頁面（Claude / ChatGPT）複製進 TalkType custom prompt textarea — 那串內容若被 LLM-attacker 注入，polish 流程的 system prompt 變成 attacker-controlled、output 可能 inject 進 paste target。Phase 1 不解決、M9 加 textarea 旁警告「不建議直接複製來自 web LLM 的提示詞」+ 加進 `doc/plans/05-data-model.md` Custom prompt section threat model 文件。
- **In-flight polish HTTP shutdown 漏 token billing（D21、M9 polish）**：`lib.rs::RunEvent::Exit` 8-step shutdown 沒等 in-flight polish_text future。reqwest 強制 cancel 但 server-side 已 commit prompt tokens、user 月底看 LLM bill 多 N 次幽靈 charges。M9 加「step before audio mute restore：等 polish_busy 1s timeout」用 `tokio::time::timeout` + `polish_busy.load()` poll。

### UX gaps（dogfood + M9）

- **ESC during enhancing 不 cancel polish（F23 decision、M6 explicit no-op）**：M6 不投資 cancel channel、ESC during enhancing 只 console.warn「優化中無法取消」。User 撞到 15s timeout 不能中止。Phase 2 加 `cancel_polish` Tauri command + `tokio::select!` cancel channel。
- **Toggle hotkey burst during enhancing（F24 decision、polish_busy guard reject）**：second polish_text 期間返回 `Busy` → fallback to raw paste。Spec 留：可考慮 drop polish_busy 完全（mirror M4 transcribe_busy removal）每 polish 各自 future、無 shared state — 但 LLM 計費考量保留 guard 防 quota burn。Dogfood 期收 user feedback 看是否 friction。
- **SR 連續 transcribing → enhancing 不 announce（F28 mitigation、Phase 2 a11y polish）**：F28 改 ARIA wording 故意不同 + zero-width space mitigate；但 Phase 2 a11y testing 必須驗實機 NVDA / Narrator 是否 both announce。
- **HUD success state 1.5s linger 仍可能不夠**（M5 retro P1 carry、Decision #8 bump 1000→1500）：M6 加 enhancing → 整個 pipeline 變長、user 對 success 視覺確認需求增。Dogfood 觀察 1500ms 是否還短、可能 v0.2 改成 settings `hud.success_linger_ms`。
- **Custom prompt token estimate（E26、M9 polish UX）**：UI 只顯示 char count，CJK 1 char ≈ 1.5 tokens 不直觀。M9 加 estimated tokens display（`chars * 1.5` heuristic for CJK；ASCII-only 用 `chars * 0.25`）。

### Provider / API contract risks

- **Anthropic / OpenAI / Gemini 模型 deprecation（C13 partial fold、F4 escape hatch + M9 monitor）**：F4 已加 `llm_model_id_override` schema field、user 可手動 paste 任何 model ID（不需 app update）。但 `LLM_MODEL_LIST` 自動 refresh 仍是 M9 candidate（fetch live `/v1/models` 與 hardcoded 比、過期警告）。
- **Anthropic `anthropic-version: 2023-06-01` hardcode（C16、M9 release prep）**：M6 pin 2023-06-01。Anthropic 若 deprecate 此 version mid-Phase-1（如 2024 deprecate 2023-01-01）polish 全 break。M9 release prep 加 re-confirm version 步驟、加 `LLM_API_VERSIONS` const block + next-review-date comment。
- **SSE streaming polish（C18、Phase 2 candidate）**：M6 spec 不做 streaming（HTTP 1 round trip）。OpenAI / Anthropic / Gemini 都 support SSE。Phase 2 candidate：HUD bubble 漸進式 reveal polish text。defer 因 (a) SendInput Ctrl+V 不能 paste partial；(b) user expectation 是 paste-after-complete。

### Architectural debt（M9）

- **3 enum HttpProviderError 抽 shared trait（Plan §4 #1 + G33、M9 polish 候選）**：`TestConnectionError` + `TranscriptionError` + `PolishError` 三 enum 各 ~80% Network*/RateLimited/ApiError/ParseError 重複。M6 不抽（3 enum 是抽象 threshold 邊緣、抽會 risk M6 review noise hide bug）。M9 polish dedicated commit 抽 `HttpProviderError` trait + 各 module 拼自己 data-layer variant。
- **`enhancement_duration_ms` SQLite write（J43、M8 history persistence）**：M6 PolishResult 有 durationMs 但不 write SQLite（database.rs M8 才接）。M8 history persistence 從 PolishResult 讀 durationMs 寫進 transcriptions table。M6 dogfood 期僅 console.log。
- **Token count → SQLite analytics（I39、M8 + M9）**：F17 PolishResult 已預留 input_tokens / output_tokens optional 欄位、M6 不 display UI。M8 history persistence 寫進 SQLite、M9 dogfood 用 token / latency 量 p50 p95。

### Backward compat / observability

- **Settings field-level deserializer 容錯（H36、M9 polish）**：當前 `Settings::load_or_default` 處理 top-level 壞 JSON 走 default。但 partial-malformed（如 `{"hotkey": ..., "llmCustomPrompt": 12345}` wrong type）整個 deserialize fail → 全 default、user 失去正確的 hotkey 設定。M9 polish 加 per-field permissive deserializer with logging。Add unit test `settings_with_invalid_llm_field_preserves_hotkey`.
- **PolishError variant → i18n key 對齊（I37 部分 fold、M9 漏網 audit）**：F32 chunk 4 列 18 polishError keys、覆蓋 PolishError 全 variants。M9 release verify SOP 加一條：`grep -E "polishError\." src/locales/*.json | wc -l` 應 = `grep -E "PolishError::" src-tauri/ | wc -l × 2 langs`、確保新加 variant 一定有 i18n key。
- **In-flight polish reqwest cancel-on-shutdown（D21 carry、Phase 2 audit）**：M6 不 fix（`lib.rs` Exit 8-step 沒 polish wait）；user 在 polish 期間 quit app → in-flight HTTP cancel 但 server-side 已扣 token quota。M9 release verify 量 dogfood 是否真撞到。Phase 2 接 cancel channel 後一併修。

## M6 chunk reviewer findings (2026-05-06 — chunks 0-4)

> 由 M6 chunks 0-4 完成後 reviewer subagents 找出。chunk-internal P1 已 fold 進後續 chunks（如 chunk 0 reviewer P2 LLM_PROVIDERS inactive 由 chunk 4 fix；chunk 1 reviewer P1-1 stale `#[allow(dead_code)]` 由 chunk 2 fix；chunk 2 reviewer P2-3 F23 ESC during enhancing 由 chunk 3 加 test cover）。以下 ~35 surviving items 是 M9 polish / Phase 2 candidates、不阻擋 M6 ship。

### Chunk 0 reviewer (P2 only — 4 items)

- **P2-1 `credentials.ts` doc comment under-describes type widening**：chunk 0 把 `LlmProviderId` widening 進 credentials provider list、doc comment 沒明說 widening rationale。M9 polish docstring 加 reasoning。
- **P2-2 `LLM_PROVIDERS` 仍 inactive for openrouter/nvidia in chunk 0**（chunk 4 fixed）：chunk 0 加 type / Rust schema、chunk 4 才 flip `active: true`。M9 review 已 follow up done.
- **P2-3 `SUCCESS_LINGER_MS = 1000` 仍未 bump in chunk 0**（chunk 2 fixed by Decision #8）：chunk 0 不動 voice flow store、chunk 2 加 1000→1500 bump。M9 confirmed done.
- **P2-4 placeholder module style cosmetic**：`llm_polish/mod.rs` 22 LOC placeholder 風格不 align Rust idiom（chunk 1 重寫）。M9 cosmetic.

### Chunk 1 reviewer (P1 + P2 — 10 items)

- **P1-1 stale `#[allow(dead_code)]` on `get_credential`**（chunk 2 fixed）：chunk 1 加 `#[allow(dead_code)]` 因 lib.rs 還沒 register polish_text、chunk 2 register 後此 attr 應移除（已 done）。
- **P1-2 Retry-After comment misleading**（chunk 2 fixed via pre-fetch）：chunk 1 註解寫「retry only if Retry-After ≤ 2s」但實作是 ≤ 2s ✓ 但 ≤ 1s 才實際走 retry path（與註解 mismatch）。chunk 2 改 frontend retry-same logic 後此 comment 已 不適用、chunk 2 cleanup。
- **P2-3 No explicit OAI-compat 401/429/500 wiremock tests**（compensated by direct parser tests）：chunk 1 wiremock 主要 test happy path、4 xx/5xx 用 direct parser tests cover、wiremock 沒專門測 OAI-compat 4xx/5xx。M9 polish candidate（switch wiremock 系列若 M9 strictness 需要）。
- **P2-4 `insta` not used for prompt-string snapshots**（acceptable per "或等價"）：chunk 1 spec 寫 insta snapshot OR 等價、implementer 用 inline string compare assert 等價。M9 switch insta if strictness needed.
- **P2-5 `MaxOutputTokens` defensive fallback in OAI-compat path; consider `cfg(debug_assertions)` panic**：chunk 1 build_request OAI-compat 雖然只用 LegacyMaxTokens、但 fallback 到 MaxOutputTokens 防萬一。M9 polish 改 `cfg(debug_assertions)` panic 強化 invariant。
- **P2-6 Gemini truncation accepts `>= raw_len`（spec says `≥ 0.9× raw`、lenient by ~0.1）**：chunk 1 implementer 對 Gemini truncation 的接受 threshold 比 spec lenient。M9 polish 評估 threshold 調整。
- **P2-7 `unwrap_or_else(|| "groq".to_string())` defensive default in `polish_text` — could surface ParseError**：chunk 1 polish_text 對 settings provider 取 unwrap fallback `"groq"`、實際應 surface `ParseError`。M9 polish 改 `Result<>` propagate.
- **P2-8 Token count `(Option<u32>, Option<u32>)` flatten loses "no usage at all" vs "missing fields" distinction**：chunk 1 token count parse 用 (Option, Option) tuple、unable to distinguish provider 沒回 usage object vs 回了但 fields 缺。M9 polish 改 `Option<TokenUsage>` 再各自 Option。
- **P2-9 `_ => "groq"` fallback in `emit_fallback` could log warning for routing bug**：chunk 1 polish_text emit fallback 對 unknown provider fallback "groq"、應加 log warn 抓 routing bug。M9 polish add `tracing::warn!`.
- **P2-10 LOC budget overrun (3742 lines vs ~1100 budget)**：mostly tests + docstrings、5 sub-modules 各別超 budget。reviewer flagged but 不 P0 因 critical correctness 路徑值得多 test。**lesson**：Rust critical 路徑 LOC budget hard constraint 不適用、reviewer should audit 內容品質而非 LOC.

### Chunk 2 reviewer (P1 + P2 — 8 items)

- **P1-1 `isRetryablePolishError` doc claims "mirrors Rust `is_retryable`" but they actually diverge**：TS `Busy` 視為 retryable、Rust `is_retryable` 不視 Busy 為 retryable。Doc/code drift。M9 polish 修 doc 或 align logic.
- **P1-2 Stale `#[allow(dead_code)]` on Rust `is_retryable`**：Decision #5 routes retry to frontend、`is_retryable` Rust 函數變 dead code、chunk 2 應 remove `#[allow(dead_code)]` 或 delete 整 fn。M9 polish.
- **P2-1 polishEnabledAtStart Map race-out leak**：rare bounded leak（snapshot key 的 Map 會在 session crash 不 cleanup 時殘留）。M9 robustness fix（finally block clear）.
- **P2-2 polishWarning cross-session pollution**：rare race when older session's polish failure fires after newer recording starts。chunk 3 部分 mitigate（lifecycle clear in transitionTo idle/recording）、IDEAS append 為 M9 race fix（與 M5 retro `audio:recording-aborted` race fix 相關）.
- **P2-3 F23 ESC during enhancing test**（chunk 3 added）：chunk 2 console.warn no-op 加了、chunk 3 加 test cover.
- **P2-4 Polish failure NOT trigger 3s error linger not explicitly asserted**：chunk 2 vitest 雖然測 polish failure → polishWarning + raw paste、但沒 explicit 測「不走 3s error linger 路徑」（若 implementer 改寫成 走 error 而非 success-warning bubble、test 不會 catch）。M9 robustness add explicit assertion.
- **P2-5 None + hasCred=true (trust-transitive) path no unit test**：chunk 2 vitest 主要 cover `Some(true)` / `Some(false)` paths、F22 None auto-detect 路徑沒 dedicated unit test。M5→M6 trust-transitive 是 critical UX flow、M9 polish add test.
- **P2-6 F28 ARIA \W regex strips CJK**（chunk 3 fixed via \p{L}）：chunk 2 加的 ARIA wording differ assertion 用 `\W` regex、CJK context strip 中文後變 empty string、assertion 永遠 false。chunk 3 reviewer 抓出後改 `\p{L}` Unicode-aware letter class.

### Chunk 3 reviewer (P2 — 3 items)

- **P2-1 Playwright MCP screenshot leak vector to repo root**（chunk 4 added .gitignore patterns）：M5 chunk 4 已 cleanup `m5r-*.png`、M6 chunk 3 reviewer 仍踩雷。chunk 4 加 `.gitignore` 8 line patterns 阻擋 future leak.
- **P2-2 `<Transition>` icon swap timing regression test guard missing**：chunk 3 加 success bubble dual-mode（CheckCircle2 ↔ AlertTriangle 透過 `<Transition>`）、但 test 沒 cover transition timing regression（如未來 implementer 把 transition prop 從 `mode="out-in"` 改 default、icon 會 overlap 短暫）。M9 polish add Playwright timing guard.
- **P2-3 Dashboard sidebar hidden in transcribing/success states (M9 dogfood may revisit)**：chunk 3 only enhancing badge cascade、transcribing / success states sidebar badge hidden（M5 既定）。M9 dogfood 看 user 是否 want transcribing badge.

### Chunk 4 reviewer (P1 + P2 — 10 items)

- **P1-1 `SettingsLlmPolishSection.vue` 686 LOC vs 400 budget**（extract dataflow indicator + upgrade banner + no-key banner subcomponents）：超 budget 70%、chunk 4 reviewer flagged 應 extract subcomponents。implementer time 不夠、M9 polish refactor.
- **P1-2 Upgrade banner flash-of-hidden-then-shown (synchronous localStorage read at script setup top)**：原 `onMounted` async read、reload 時 banner 短暫 flash hidden 再 show。chunk 4 implementer 已修 sync read at script setup top.
- **P2-1 `apiKey.providerInactiveSuffix` `M6+` → `v0.2` change**（intentional, doc per Decision #3）：chunk 4 改 i18n suffix wording from "M6+" to "v0.2"、reflect Decision #3 cascade（OpenAI/Anthropic defer to v0.2、不是 M6 加）。Doc 已 cover.
- **P2-2 Reviewer screenshots write to repo root despite filename param (consider PowerShell wrapper)**：chunk 4 reviewer Playwright 仍寫 repo root（cwd issue not yet fixed by `.gitignore`）。M9 process improvement 寫 PowerShell wrapper `playwright-screenshot.ps1` 強制 path prefix `.playwright-mcp/<reviewer>/`.
- **P2-3 Test polish button enabled when polish ON + no key (acceptable diagnostic, could disable + tooltip)**：chunk 4 toggle ON + no key 時 test polish button 仍 enabled、user 點會看 ApiKeyMissing error。可 disable + tooltip「請先設 API key」更友善。M9 UX polish.
- **P2-4 `syncFromStore` provider whitelist hardcoded; should derive from `LLM_PROVIDERS.filter(p => p.active)`**：chunk 4 syncFromStore 對 provider 切換 hardcoded check `provider in ['groq', 'openrouter', 'nvidia', 'gemini']`、未來 v0.2 加 OpenAI/Anthropic 時 whitelist 沒 update。改 derive from `LLM_PROVIDERS.filter(p => p.active)` 自動 sync. M9 refactor.
- **P2-5 `watch + onMounted` slight redundancy**：chunk 4 SettingsLlmPolishSection 用 `watch` + `onMounted` 雙重 sync、redundancy。M9 polish consolidate.
- **P2-6 No `<Suspense>` boundary (brief flash of defaults before sync)**：chunk 4 mount 瞬間 user 看 default values flash 0.1s、再 sync from store。M9 polish 加 `<Suspense>` 包 settings load.
- **P2-7 Custom prompt blur-persist (no debounced fallback)**：chunk 4 custom prompt only blur 時 persist、user 中途切視窗會丟未 persist 內容。M9 polish 加 debounced auto-save。
- **P2-8 Pre-existing Audio Input section error (not chunk 4 — investigate as separate issue)**：reviewer 跑 settings smoke test 時 Audio Input section console error、不是 chunk 4 加的（M5 / M3 carry forward）。投單獨 issue investigate.
