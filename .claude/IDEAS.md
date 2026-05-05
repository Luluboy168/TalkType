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

## Phase 2 / 後期想法

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
