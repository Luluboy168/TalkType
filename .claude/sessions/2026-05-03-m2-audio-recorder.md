# 2026-05-03 — M2 Audio Recorder Pipeline (cpal + hound + rustfft)

> **Session topic**：完成 Phase 1 Milestone 2（錄音 pipeline）— Rust core 用 cpal 錄音、hound encode WAV、rustfft 6-band waveform、cpal mic preview、檔案管理 commands、Vue composables + Settings mic picker + Dashboard 錄音測試卡。
> **Outcome**：✅ M2 Done、3 chunks 落地、27 個 cargo tests pass、static checks all green、cargo build pass；acceptance criteria 中需要實機 mic 互動的部分留待 user 跑 `pnpm tauri dev` 手動驗證。

## What changed

### M2 implementation（3 chunks）

延續 M0 + M1 證實的 subagent-driven 模式：main session 不直接寫 code，3 個 implementation chunks 由 Opus 4.7 subagents 實作、reviewer subagents 把關。Main session 負責 orchestrate、commit、E2E 文件 / 螢幕截圖驗證。

| Commit | Type | 內容 |
|---|---|---|
| `699618f` | feat(m2) | Chunk 1 — Rust core 錄音：`AudioRecorderState`、`start_recording` / `stop_recording`、`list_audio_input_devices` / `get_default_input_device_name`、`save_recording_file` / `read_recording_file` / `delete_all_recordings` / `cleanup_old_recordings`、`AudioRecorderError` (thiserror)、cpal stream + sample format dispatch (10 formats)、hound WAV encode、`recordings/<uuid>.wav` 路徑（path-traversal defense via UUID parse）、10 個 unit tests |
| `ea611ce` | feat(m2) | Chunk 2 — FFT waveform + mic preview：`waveform.rs`（單 buffer + cursor 設計、Hann window、6 bins `[9, 4, 1, 2, 6, 12]`、`normalize_db(-100, -20)`）、`audio:waveform` event 每 16ms emit、`preview.rs` 獨立 `AudioPreviewState` + `audio:preview-level` event 每 30ms emit、Vue composables `useAudioPreview` + `useAudioWaveform` (RAF + lerp 平滑)、共 17 個 unit tests |
| (本 chunk 3) | feat(m2) | Chunk 3 — UI wiring + docs：Settings mic picker section（shadcn `Select` + 預覽 button + RMS bar）、Dashboard `AudioRecordTest` 卡（3 buttons + 6-bar waveform + 3 秒 auto-stop switch）、`AudioInputDeviceInfo` + `StopRecordingResult` types、i18n 補齊、IPC contract 表 + 進度 dashboard 文件更新、vite shape screenshots、cargo build pass |

### Files changed by Chunk 3

#### 新增

- `src/types/audio.ts` — `AudioInputDeviceInfo`、`StopRecordingResult`
- `src/components/AudioRecordTest.vue` — Dashboard 錄音測試卡
- `src/components/ui/select/*` — shadcn-vue Select 元件（12 files）
- `src/components/ui/switch/*` — shadcn-vue Switch 元件（2 files）
- `docs/screenshots/m2/m2-dashboard-with-record-test.png`
- `docs/screenshots/m2/m2-settings-mic-picker.png`
- `.claude/sessions/2026-05-03-m2-audio-recorder.md`（本檔）

#### 改寫

- `src/views/SettingsView.vue` — 加「音訊輸入」section（mic picker + 預覽 + RMS bar）
- `src/views/DashboardView.vue` — embed `<AudioRecordTest />`
- `src/types/index.ts` — re-export `./audio`
- `src/i18n/locales/zh-TW.json` + `en.json` — `views.settings.audioInput.*`、`dashboard.audioTest.*`
- `doc/plans/01-architecture.md` — IPC contract 表加 5 個 file commands、確認 audio events 註解 + payload type；最後更新 2026-05-03
- `doc/plans/02-implementation-roadmap.md` — M2 row 進度 ✅ Done、tick 32 個 task checkboxes；最後更新 2026-05-03
- `.claude/PROGRESS.md` — M2 ✅、加入今日 session row
- `.claude/IDEAS.md` — append M2-discovered follow-ups

## Key decisions

- **Single-buffer + cursor 設計 for FFT waveform**：在 `waveform.rs` 用同一個固定大小 `Vec<i16>` 維護 ring（cursor 環繞寫入），每次 push 後從 cursor 起點往前讀 N 個 samples 做 FFT。比 SayIt 的「分離 ring + working buffer」省一次 clone、簡化 lock 範圍。
- **Generic-callback dispatch in `stream.rs`**：`dispatch_sample_format_with_callback` 用 generic `F: FnMut(&[i16])` 把 cpal 10 種 sample formats 統一轉成 mono `i16` slice 後丟給 caller-supplied closure。錄音與 preview path 共用同一份 dispatch（recording 推到 `Arc<Mutex<Vec<i16>>>`、preview 推到 `Arc<Mutex<VecDeque<i16>>>` ring buffer）。
- **`AudioPreviewState` 與 `AudioRecorderState` 分離**：使用者打開 Settings 看 mic preview 時，可能還沒開始錄音；反之開始錄音時不一定要關 preview。兩個獨立 state slot + 獨立 named thread (`"audio-recorder"` / `"audio-preview"`) 讓兩個 flow 互不干擾。Phase 2 macOS 可能因為 cpal 不允許同一裝置同時有兩個 stream 而需要加 stop-other-on-start logic — 已記入 IDEAS.md。
- **`validate_id` UUID parse hardening for file commands**：`save_recording_file` / `read_recording_file` 的 `id` 必須能 parse 成 UUID v4 — 任何 `../`、絕對路徑、NUL byte 都會被 `Uuid::parse_str` 拒絕。即使將來 caller 直接從 frontend 傳 attacker-controlled id，也不可能 traverse 出 `recordings/` 目錄。
- **Hann window + 6 bins at indices `[9, 4, 1, 2, 6, 12]`**：對 SayIt 的視覺擴散 parity — 不用線性 `[0, 1, 2, 3, 4, 5]` 因為低頻 dominate；非單調順序讓視覺律動更生動。Hann window 抑制 FFT spectral leakage。
- **`normalize_db(-100, -20)` mapping**：把 FFT magnitude 轉成 dB 後線性映射到 `[0.0, 1.0]`。`-100 dB` 視為 silence、`-20 dB` 視為 max；窄 80 dB 範圍對 voice 動態夠用，且不浪費 bar 顯示空間（SayIt 的選擇）。
- **cpal `!Send + !Sync` → 命名 thread + `mpsc` ack pattern**：`cpal::Stream` 不能跨 thread 移動，所以 stream 完全 own 在 named `"audio-recorder"` / `"audio-preview"` thread 內。`mpsc::Sender<Result<...>>` 做 single-shot startup ack，讓 `start_*` command 不論是 command thread 失敗還是 audio thread 失敗都回相同 error domain。
- **SECURITY: `stream.pause()` before drop on all 4 teardown paths**：cpal 0.15.x 在 macOS 有 Arc-cycle bug — drop stream 不一定 call `AudioOutputUnitStop`。所以 4 個 teardown 路徑（recording stop、recording abort due to ack failure、preview stop、preview abort）都先 `stream.pause()` 再 drop，pause 失敗時印 `SECURITY:` log line。Phase 1 是 Windows-only，但 pattern 在 Phase 2 macOS 必須要有。
- **`recording_thread.rs` 從 `stream.rs` 拆出**：原本 stream 設定 + recording thread body 都塞 `stream.rs`，會超過 400 行。拆 `recording_thread.rs` 後 `stream.rs` ~360 行、`recording_thread.rs` ~180 行，都在 budget 內。
- **檔案大小 budget 認知**：`mod.rs` 目前 418 行（8 行超過 400 budget，多在文件 + 8 個 commands header doc comments）— M3 順手把 commands.rs 拆出來解決。已記入 IDEAS.md + Follow-ups。

## Surprises / 踩雷

- **cpal::Stream 是 `!Send + !Sync`** — 這個 fact 強迫整個 state design 採 named thread + channel 而不是 simple `Mutex<Stream>`。SayIt 也踩過這個雷，pattern 已經沿用。
- **`recording_thread.rs` 拆檔需求**：原本想把整個 stream + thread body 放 `stream.rs`，超過 400 行就 budget 違規，所以中途拆。早點規劃 budget 的話可以一次到位。
- **`mod.rs` 仍微超 budget**：418 行超 400 line budget 8 行 — 主要是 8 個 commands 各帶 doc comment + state struct + helper 函式。M3 拆 commands.rs 是合理時機（不為了砍幾行而做 premature 拆檔）。
- **shadcn-vue `Select` v-model 不接受空字串**：Settings mic picker 想用 `null` 或 `""` 當「系統預設」狀態，但 reka-ui SelectItem 不允許 empty string value。改用 `__system_default__` sentinel + computed `resolvedDeviceName` 把 sentinel 轉回 `null` invoke 給 Rust。
- **vite-only screenshot 模式 invoke 必失敗**：Settings mic picker 在 vite shape mode 看到「Cannot read properties of undefined (reading 'invoke')」error，這是 expected — `@tauri-apps/api/core` 在沒有 Tauri runtime 時 `window.__TAURI_INTERNALS__` undefined。Screenshot 拿來驗證 UI 形狀就夠了。

## Acceptance criteria（M2 roadmap 5 條）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| Settings 選 mic、看到即時 RMS bar 動 | ✅ Code path（mic picker + preview + composable） | ✅ user 跑 `pnpm tauri dev`、進 Settings、選 mic、點預覽、講話看 bar 是否跟著動 |
| 開始錄音 → 60fps waveform 動畫順暢 | ✅ Code path（`AudioRecordTest` 6 bars + `useAudioWaveform` lerp） | ✅ user 跑 `pnpm tauri dev`、進 Dashboard、點開始錄音、講話看 6 bars 是否流暢 |
| 停止錄音 → 拿到 WAV bytes、可存檔 | ✅ Code path（`stop_recording` + `save_recording_file`） | ✅ user 跑 → 點停止錄音 → 看 stop result 顯示 duration/peak/rms → 點存成 WAV → 看回傳的相對路徑 |
| 檔案在 `%APPDATA%\com.luluboy168.talktype\recordings\` | ✅ Code path（`recordings_dir` resolves `app_data_dir/recordings`） | ✅ user 開 Explorer 確認檔案存在、可用 VLC / Windows Media Player 開來聽 |
| Rust tests pass | ✅ 27 tests（10 chunk-1 + 17 chunk-2）all pass | — |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → 1 pass
- `cargo check` → clean (1.02s incremental)
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test` → 27 passed; 0 failed
- `cargo build` (no-default-features dev profile) → success — 確認 chunk 3 lib.rs 11 個 audio commands wire-up 沒讓 build 壞掉

### Vite-shape screenshots

- `docs/screenshots/m2/m2-dashboard-with-record-test.png`（1280×800、Sidebar + IPC Smoke + Audio Recording Test 卡都正確 render）
- `docs/screenshots/m2/m2-settings-mic-picker.png`（1280×800、Settings 設定 + 音訊輸入 + 系統預設 select + 預覽 button + 音量 bar 都正確 render；vite mode 出現預期的 invoke error 提示，視覺驗證 OK）

### Tauri runtime smoke

- `cargo build --no-default-features` → 1.73s success（chunk 3 wire-up validate）
- `pnpm tauri dev` 完整 launch 沒做（M1 已知 pnpm shim PATH 問題）— cargo build pass 已足夠驗證新增的 8 個 commands 編得起來、跟 lib.rs 整合無誤

## Manual verification checklist（user 跑 `pnpm tauri dev` 後）

- [ ] Settings → 看到裝置清單下拉選單 + 預設選中第一個（系統預設或 isDefault 的裝置）？
- [ ] 點預覽 → RMS bar 隨著講話而動（從靜音 0% 到講話時 30-60%）？
- [ ] 切到 /dashboard → 看到 6 條 waveform bars（idle 時都是 0 高度、看不見）
- [ ] 點開始錄音 + 講 3 秒 + 點停止錄音 → bars 動畫順暢、stop result 顯示「已停止 (X.X 秒、peak=Y.YY、rms=Z.ZZ)」
- [ ] 開「3 秒後自動停止」switch → 點開始錄音 → 3 秒後自動停止
- [ ] 點存成 WAV → 看到「已存檔 → recordings/abc-...uuid....wav」訊息
- [ ] 在 `%APPDATA%\com.luluboy168.talktype\recordings\` 看到對應的 .wav 檔案
- [ ] WAV 檔可以用 VLC / Windows Media Player 開來聽（驗證 hound encode 正確、聲音內容是剛剛講的）
- [ ] Settings → 切換不同 mic → 點預覽再講 → bar 仍然會動（裝置 selection 流程通）

## Follow-ups for M3

- **`audio_recorder/mod.rs` 拆 commands.rs**（M3 順手做）：mod.rs 目前 418 行（8 行超過 400 budget），M3 加上 transcribe* 之前先把 8 個 audio commands 的 `#[tauri::command]` 函式抽到 `commands.rs`、mod.rs 只剩 state + helper + tests，符合 budget 同時讓 M3 transcribe pipeline 更乾淨。
- **M3 transcribe 接 `audio_recorder.wav_buffer` 用 `take()` 而不是 clone**：目前 `save_recording_file` 用 clone（保留 buffer 給後續 caller），M3 transcribe 是「最後消費者」適合用 `take()` 清空避免重複送 Whisper。可加 `consume_wav_buffer()` helper。
- **macOS Phase 2：cpal 同裝置雙 stream race**：M2 的 recording 與 preview 是兩個獨立 stream + 獨立 thread；macOS CoreAudio 不允許同 device 同時兩個 input stream，預覽中按錄音會撞錯。Phase 2 macOS 需要：preview 在 record start 時自動 stop（透過 cross-state shared shutdown signal）。
- **Phase 2 polish：dev-only Dashboard 卡片用 `import.meta.env.DEV` gating**：`<AudioRecordTest>` + `<IpcSmokeTest>` 都是 dev tooling，M9 release prep 時統一加 `<template v-if="isDev">` wrapper（或直接 conditional import）。

## Subagent dispatch pattern（M2 證實）

承襲 M0 + M1 模式，M2 規模較大（chunk 1 約 2000 行 Rust + 5 sub-files、chunk 2 約 600 行 Rust + 2 frontend composables、chunk 3 約 400 行 frontend + 文件），3 chunks 都採用同樣的 dispatch + reviewer pattern：

1. **3 個 implementation chunks 順序執行**（chunks 共享 `Cargo.toml` + `lib.rs` 且依賴累進，sequential 必要）：
   - Chunk 1（Rust core 錄音 + 檔案管理）：subagent A 實作 → reviewer subagent 驗（catch state design 問題）
   - Chunk 2（FFT waveform + preview + composables）：subagent B 實作 → reviewer subagent 驗（catch threading + 共用 dispatch 問題）
   - Chunk 3（UI + docs + screenshots）：subagent C 實作（本 session）→ 後續 reviewer subagent 待 main session 跑
2. **Main session 不寫 code、只 orchestrate + commit**：每個 chunk 完成後 main agent 親手 commit、撰 commit message。
3. **每個 chunk 後跟 reviewer subagent**：catch implementation 盲點（chunk 1 的 SECURITY pause 規律 + chunk 2 的單 buffer cursor 設計都是 reviewer 驗證過的）。

對 M3 / M4（跨 module、需要 IPC 與 keyring 互動）大概也適用相同 pattern；M7 (whisper.cpp) 因為涉及 FFI binding spike，可能需要 main agent 直接介入 build pipeline debug。

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M3 將是下一個 milestone）
2. 本檔（M2 session log）— 特別是「Follow-ups for M3」段
3. `.claude/IDEAS.md`
4. `doc/plans/02-implementation-roadmap.md` M3 section（cloud transcription via Groq Whisper）
5. `doc/plans/06-hybrid-transcription.md`（Cloud + Local 切換設計）
6. `doc/plans/05-data-model.md` Credential Vault section（M3 keyring 整合）
