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
