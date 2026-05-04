# 2026-05-04 — M3 Cloud Transcription (Groq Whisper + credentials + test connection)

> **Session topic**：完成 Phase 1 Milestone 3（cloud transcription）— OS credential vault（keyring）API key 管理、Rust-side Groq Whisper HTTP client、provider connectivity health check（Q5）、Settings 測試連線 / Dashboard 測試轉錄 dev surfaces、M2 retro 收尾（`MAX_WAV_BYTES` 防 OOM、`audio:mic-safety-warning` 釋出 release-build 不可見的 stderr 訊號、`clear_recording_buffer` 阻 RAM 漏）。
> **Outcome**：✅ M3 Done、4 chunks 落地、94 個 cargo tests pass（Chunk 0 → 27 + 5、Chunk 1 → 9、Chunk 2 → 32、Chunk 3 → 10）、static checks all green、vite shape screenshots 兩張；Tauri runtime smoke + 真 Groq key 由 user 跑 `pnpm tauri dev` 時手動驗證（main session 無 key）。

## What changed

### M3 implementation（4 chunks）

延續 M0 + M1 + M2 的 subagent-driven 模式：main session 不直接寫 code，4 個 implementation chunks 依序由 Opus 4.7 subagents 實作 + reviewer subagents 把關。Plan-time challenger 在開工前對 spec 提出 Q1–Q5（API key 政策、25 MB cap、vocabulary cap、proxy 支援、test connection scope）→ 主 session refine plan → dispatch implementer → 完成後 reviewer + retro challenger。

| Commit | Type | 內容 |
|---|---|---|
| `bc81a91` | feat(m3) | Chunk 0 — M2 retro 收尾：`audio_recorder/mod.rs` 拆 `commands.rs`（mod.rs 418 → 351 行）、`MAX_WAV_BYTES = 25_000_000` 常數 + recording_thread 監測 + `audio:recording-aborted` event、`audio:mic-safety-warning` event（M2 retro #2 release stderr 不可見問題）、`clear_recording_buffer` command（M2 retro #3 RAM 漏）、`consume_wav_buffer()` helper（transcribe 是 last consumer 用 take()）、`delete_recording(id)` 單筆刪除（M2 retro 缺）、5 個新 cargo tests |
| `c7efb2c` | feat(m3) | Chunk 1 — Credentials：`plugins/credentials.rs`（`set_credential` / `delete_credential` / `has_credential` 三個 Tauri commands、`get_credential` 是 `pub(crate)` Rust-only function 永不暴露 IPC）、provider 前綴粗檢（`gsk_*` for Groq / `sk-*` for OpenAI / `sk-ant-*` for Anthropic / Gemini 不檢查）、key trim、`MAX_KEY_LEN = 1024`、`SettingsApiKeySection.vue`（shadcn `<Input>` + `<Select>` + `<Button>`、Eye/EyeOff toggle、隱私揭露 dialog）、`ProviderPrivacyDialog.vue`、`LLM_PROVIDERS` registry + `findProvider` helper、9 個新 cargo tests + Cargo deps `keyring 3` (per-target features) + `reqwest 0.12` + `wiremock` (dev-only) |
| `541a930` | feat(m3) | Chunk 2 — Transcription dispatcher + Groq cloud：`plugins/transcription/{mod, cloud, parser, error}.rs`（4 file 拆分、challenger #4 強制 parser split 維持 < 500 行 cliff）、`TranscriptionState`（120s timeout reqwest client + AtomicBool busy guard + RAII BusyGuard）、`transcribe_audio` Tauri command（M3 hardcode Groq）、`transcribe_cloud_internal` (pre-check size BEFORE take() 是 Q2 invariant)、`format_whisper_prompt` 50 term + 600 char dual cap (Q3)、`parse_groq_response` `verbose_json` deserializer + `min(no_speech_prob)`、retry policy（Timeout / RateLimited only、honor Retry-After）、`TranscriptionError` 16 變體 + manual flat-string Serialize、`classify_reqwest_error` 共用 helper、32 個新 cargo tests（wiremock-based 8+ error variants） |
| (本 chunk 3) | feat(m3) | Chunk 3 — Test connection (Q5) + UX surfaces + docs：`plugins/transcription/health.rs`（新檔，`test_provider_connection` Tauri command、`TestConnectionResult { ok, provider, modelCount }`、`TestConnectionError` 8 變體 flat-string Serialize、URL-parameterized `test_groq_connection_with_url` 給 wiremock test 用）、Settings 測試連線 button + 5s 自動清結果 timer + Loader2 spinner、Dashboard `AudioRecordTest` 加第 4 顆「測試轉錄」按鈕 + i18n key gating + transcribe error 友善訊息 mapping、`TestConnectionResult` TypeScript type、`testError.*` / `transcribeError.*` i18n keys（zh-TW + en）、README「Behind a corporate proxy?」section（Q4）、文件更新（PROGRESS / roadmap M3 → Done / 本 session log / IPC contract 確認）、10 個新 cargo tests（總 94） |

### Files changed by Chunk 3

#### 新增

- `src-tauri/src/plugins/transcription/health.rs`（323 行；含 10 個 wiremock + serialize tests）
- `docs/screenshots/m3/m3-settings-with-test-connection.png`（vite shape mode 1280×800）
- `docs/screenshots/m3/m3-dashboard-with-transcribe-test.png`（vite shape mode 1280×800、4 顆 buttons 都 render）
- `.claude/sessions/2026-05-04-m3-cloud-transcription.md`（本檔）

#### 改寫

- `src-tauri/src/plugins/transcription/mod.rs` — re-export `health` 子模組 + `pub use health::test_provider_connection`
- `src-tauri/src/lib.rs` — 18 個 commands（17 + `test_provider_connection`）；comment 加 chunk-3 entry
- `src/components/SettingsApiKeySection.vue` — 「測試連線」button（在已儲存 key 旁）+ test result message + 5s timer + onUnmounted cleanup + `formatTestError` 字串對應 → i18n
- `src/components/AudioRecordTest.vue` — 「測試轉錄」button（status === stopped + hasGroqKey gated）+ `transcribing` / `transcribed` 兩個新 status、`transcriptionResult` ref + readonly textarea result + duration / no_speech_prob 顯示、`formatTranscribeError` mapping、`onMounted` + `watch(status)` 觸發 has_credential 重檢
- `src/types/credentials.ts` — `TestConnectionResult` type
- `src/i18n/locales/zh-TW.json` + `en.json` — `views.settings.apiKey.test*` / `testError.*`、`dashboard.audioTest.transcribe*` / `transcribeError.*`
- `README.md` — 「Behind a corporate proxy?」section（zh-TW、Q4）
- `doc/plans/02-implementation-roadmap.md` — M3 dashboard ✅ Done @ 2026-05-04 / 2026-05-04、ticking 全部 checkboxes、status note 升 v1
- `doc/plans/01-architecture.md` — 「最後更新」更新到 2026-05-04（IPC table 已 chunks 0-2 加完 `transcribe_audio` / `test_provider_connection` / `transcription:completed` 三 row）
- `.claude/PROGRESS.md` — M3 → ✅ Done、加入今日 session row
- `.claude/IDEAS.md` — chunk-3 discovered follow-ups append

## Key decisions

- **Q1 (a) 嚴格 Rust-only API key — 永不過 IPC**（plan-time challenger pass）：4 個理由
  1. Marketing：v0.1.0 README 想寫「Your API keys never cross the IPC boundary」（vs. Typeless 2025-11 隱私翻車的反面教材）
  2. M9 release audit scope：(a) 只 audit Rust、(b) 要 audit Rust + frontend + Sentry config — (a) 縮小好幾倍
  3. M3 ↔ M6 對稱性：M6 LLM polish 也是 Rust-side 4-provider fetch，與 M3 同 pattern
  4. OSS 標準：keyring vault 一旦 plaintext 過 IPC 就破功（Sentry breadcrumb / dev-tools / process dump 都會 leak）
- **Q2 (c) 硬拒 25 MB**：單一 `MAX_WAV_BYTES` 常數同時護住 (a) M2 retro #1 OOM 防止、(b) Groq upload cap、(c) UX 一致性。錄音中 recording_thread 監測達標 → emit `audio:recording-aborted { reason: 'max_size' }` + 自動 stop_recording；轉錄前 `transcribe_cloud_internal` 又驗一次 size BEFORE `take()`（Q2 invariant — pre-check 失敗 buffer 仍在、user 可手動 save_recording_file 留檔）
- **Q3 (a) Vocabulary 兩層 cap**：50 terms + 600 chars dual cap、超出取 prefix（不切 term 中間，Mandarin 不會半字）。原因：Groq prompt limit 約 896 chars 留 headroom 給 "Important Vocabulary: " prefix；50 個中文人名或長詞才不會觸頂。Char count 用 `chars().count()` 不是 `len()` byte count（UTF-8 multi-byte 安全）
- **Q4 (b) HTTPS_PROXY env var**：reqwest 預設讀環境變數、零程式碼。README 寫 PowerShell 一次性 + 永久兩種設法 + PAC fallback（Edge net-internals）
- **Q5 (a) test_provider_connection in M3**：M3 ship Groq、M6 extend 4 provider。GET `/models` 而非 POST `/audio/transcriptions`（不耗 quota、response 小 ~3 KB、5s timeout 對慢 proxy 都還夠）。回 `model_count` 給具體成功訊號（vs 純 ✓ checkmark）
- **Module split**：`transcription/{mod, cloud, parser, error, health}.rs` 5 file 結構。Chunk 2 reviewer challenger #4 點出 `cloud.rs` 一開始 620 行超過 500 line cliff、強制把 `format_whisper_prompt` + `parse_groq_response` 拆到 `parser.rs`（最後 cloud 411 行、parser 239 行、mod 202 行、error 227 行）。Chunk 3 把 `health` 獨立檔（323 行）保持每 file 都 < 500
- **Error taxonomy split 16 vs 8**：原本 8 變體合併網路類，challenger 點出 user 看到「網路錯誤」沒有 actionable 修法。拆細：`Offline` / `DnsFailure` / `TlsFailure` / `Timeout` / `ConnectionRefused` / `NetworkOther` 各對應不同 UI 提示（offline → 檢查網路、dns → 檢查 DNS server、tls → 系統時間 / cert、timeout → 重試或調 timeout、connection refused → 防火牆、other → fallback 訊息）
- **Retry policy 1-shot**：只在 `Timeout` / `RateLimited` retry、4xx + `ApiKeyMissing` + `Offline` + `Busy` 不 retry。`RateLimited` 解析 `Retry-After` header `tokio::time::sleep(secs)` 後重試（超過 3 次 user 被卡 60s 還更糟、所以只 1 次）
- **tokio explicit `["rt", "time"]` features**：原本只 `rt`（M2 chunk 1 `spawn_blocking`），`time` 為了 `tokio::time::sleep` 在 retry honor Retry-After。Tauri 拉 tokio transitive 但不保證 `time` feature ON、explicit declare 防將來 build break
- **keyring per-target features**：v3 default 是 no-op backend、必須指定。`#[cfg(target_os = "windows")] features = ["windows-native"]`、macOS `apple-native`、Linux `sync-secret-service` + `crypto-rust`。Phase 1 雖然只 Windows、其他平台預先連好讓 Phase 2 不重訪
- **shadcn-vue Google Fonts regression 防護**：M2 chunk 3 + M3 chunk 1 各踩雷 1 次（CLI 重新插入 `@import url('https://fonts.googleapis.com/...')` 到 `src/assets/index.css` 第 1-7 行違反 M0 follow-up 「offline + CSP friendly」）。CLAUDE.md 「常見踩雷」section 已 documented（在 chunk 1 一起做）；本 chunk 3 沒 add 新 shadcn component 故沒踩到，但 reviewer + main 都驗 `grep fonts.googleapis.com src/assets/index.css → 0 matches`
- **`Loader2` from lucide-vue-next（不 add Spinner）**：避免再 trigger shadcn-vue add CLI 的 Google Fonts regression、複用現成 `lucide-vue-next` 的 spinner icon + Tailwind `animate-spin` 已足夠

## Surprises / 踩雷

- **`tauri::generate_handler!` macro 不接受 `module_path::sub_module::cmd_alias`**：Chunk 3 一開始把 `pub use health::test_provider_connection;` 在 `transcription/mod.rs` re-export、`lib.rs` 寫 `transcription::test_provider_connection` 編譯失敗 `cannot find __tauri_command_name_test_provider_connection`。原因 macro 用 sibling identifier resolve、要直接寫 `transcription::health::test_provider_connection`。修法簡單但花了 1 個 cycle
- **vite-only mode UI 看不到 has_credential 後續 UI**：因為 `invoke('has_credential')` fail → `hasCredential.value = false` → 「測試連線」/「Delete」整個 row 不 render（`v-if="hasCredential"`）。所以 Settings 截圖只看得到 API key input；要看 Test Connection button 需要 Tauri runtime + 已存 key。這是 expected 但讓人懷疑是不是漏 render，已寫進 Manual verification checklist 提示 user
- **`AudioRecordTest` `watch(status)` 觸發 `has_credential` 重檢**：原本只 onMounted 一次、reviewer 指出 user 可能在 mount 後切到 Settings 設 key 再回來、`stopped` 狀態時 button 該 enable 但沒 enable。加 `watch(status, (next) => { if (next === 'stopped') refreshHasGroqKey() })` 讓 key 一旦設好下次 stop 就抓得到
- **shadcn-vue Google Fonts trap 沒踩**：本 chunk 3 沒 add 新 component（複用 chunk 1 的 `<Input>` / `<Select>` / `<Button>` / `<Switch>` + chunk 2 沒新增），所以 fonts.googleapis.com regression 沒重現。CLAUDE.md `## 常見踩雷` section 完整保留為下次 add 時的 reminder

## Acceptance criteria（M3 roadmap 8 條）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| Settings 存 API key → keyring 看得到 | ✅ Code path（`set_credential` → `keyring::Entry::set_password`） | ✅ user 跑 `pnpm tauri dev`、設 key → 開 Windows Credential Manager 看 `com.luluboy168.talktype/groq` |
| Settings 不顯示 key 明文 | ✅ Code path（`<Input type="password">`、Eye/EyeOff toggle 只 affect 當前輸入字串、stored key 永不 fetch back） | — |
| 點測試連線 → 1-2s 內顯示模型數或錯誤 | ✅ Code path（`test_provider_connection` GET `/models` 5s timeout、`TestConnectionResult { modelCount }` 顯示在 testResultMessage） | ✅ user 跑、設 key、點 button、看 ✅ N 個模型 |
| 錄 3s → 點測試轉錄 → 回正確文字 | ✅ Code path（`transcribe_audio` → `transcribe_cloud_internal` → multipart POST → `parse_groq_response` → 顯示 readonly textarea） | ✅ user 跑、錄音、點測試轉錄、看正確文字 |
| API key 錯 → InvalidKey UI | ✅ Code path（`TestConnectionError::InvalidKey` → `formatTestError` → `testError.invalidKey` i18n） | ✅ user 故意改錯 key 點測試 |
| 無網路 → friendly UI | ✅ Code path（`reqwest::Error` → `TestConnectionError::NetworkError(detail)` → `formatTestError` → `testError.network`） | ✅ user 拔網路點測試 |
| 錄音 > 25 MB → 自動 stop + UI | ✅ Code path（`recording_thread` monitor + emit `audio:recording-aborted { reason: 'max_size' }` + 自動 stop） | ✅ user 錄超過 13 分鐘 |
| In-flight transcribe + 按熱鍵 → reject + 提示 | ✅ Code path（`AtomicBool transcribe_busy.swap(true, AcqRel)` + RAII `BusyGuard` + `TranscriptionError::Busy` → `formatTranscribeError` → `transcribeError.busy`） | ✅ user 在 transcribe 中按錄音 |
| Rust tests pass | ✅ 94 tests（chunk 0 32 + chunk 1 9 + chunk 2 32 + chunk 3 10、累積） | — |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → 1 pass
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test --lib` → 94 passed; 0 failed

### Vite-shape screenshots

- `docs/screenshots/m3/m3-settings-with-test-connection.png`（1280×800、Settings 顯示 API 金鑰 section、Provider Groq、API key 輸入、儲存 button；vite-only mode 預期出現「Cannot read properties of undefined (reading 'invoke')」紅字 — 因為沒 Tauri runtime、`hasCredential` 為 false 所以「測試連線」/「Delete」button 整個 row 隱藏；user runtime 會看到完整 UI）
- `docs/screenshots/m3/m3-dashboard-with-transcribe-test.png`（1280×800、Dashboard 顯示 IPC 連線測試 + 錄音測試卡、4 顆 buttons：開始錄音 / 停止錄音 / 存成 WAV / **測試轉錄**；測試轉錄 disabled 因為 `status === 'idle'`、要先錄完 stop 才 enable）

### Tauri runtime smoke

- `cargo check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --lib` → 編譯 + 18 commands 整合 + 94 tests 都 pass
- `pnpm tauri dev` 完整 launch 沒做（main session 無 Groq key、無法 dogfood transcribe 全鏈）— user 自己用真 key 跑時驗 acceptance criteria 中需要實機的部分

## Manual verification checklist（user 跑 `pnpm tauri dev` 後）

- [ ] Settings → 設 Groq key（從 https://console.groq.com/keys 取得）→ 第一次設要看到 privacy dialog → 確認 → 看到「✅ 已儲存到 keyring」
- [ ] 點「測試連線」→ 1-2s 內顯示「✅ 連線成功（N 個模型可用）」（N 通常 ≥ 8）
- [ ] 故意改錯 key（前綴改成 `gsk_xxx_invalid`）→ 點測試 → 顯示「❌ API key 無效（401）— 請檢查 key 是否完整、無多餘空白」
- [ ] 拔網路 / 設無效 HTTPS_PROXY → 點測試 → 顯示「❌ 連線失敗 — 檢查網路或 HTTPS_PROXY 設定」
- [ ] 切到 Dashboard → 看到 4 顆 buttons（開始錄音 / 停止錄音 / 存成 WAV / **測試轉錄**）、測試轉錄 enabled 因為 key 已存
- [ ] 錄 3 秒 → 停 → 點「測試轉錄」→ 看到「轉錄結果」textarea + 「耗時 N ms · min(no_speech_prob)=X.XXX」訊息
- [ ] 錄音超過 13 分鐘（持續按住開始錄音、把「3 秒後自動停止」switch 關掉）→ 自動 abort + UI 訊息
- [ ] 在轉錄處理中按錄音 → 會被拒絕 + 「上一筆轉錄處理中」訊息（按熱鍵實作在 M4，目前手動 race 觸發）
- [ ] Open Windows Credential Manager → 看 `com.luluboy168.talktype/groq` entry 確實存在（不是 plaintext file）

## Follow-ups for M4

- **Hotkey listener Rust 部分**：`plugins/hotkey_listener.rs` Windows-only `SetWindowsHookExW(WH_KEYBOARD_LL, ...)` named thread + `GetMessageW` loop、`TriggerKey` enum、`TriggerMode { Hold, Toggle }`、events `hotkey:pressed` / `hotkey:released` / `hotkey:toggled`
- **`useVoiceFlowStore` Pinia store**：把 `hotkey:pressed` → `capture_target_window` → `start_recording` → `transitionTo("recording")` → `hotkey:released` → `stop_recording` → `transcribe_audio` → `paste_text` 串起來
- **HUD voice flow visual states**（M5）：HUD 顯示 idle / recording (waveform 6 bars) / transcribing (spinner) / success (✓) / error (✗) 5 個 states
- **Polish 失敗 fallback path**（M6）：如果 LLM polish ON 但 polish call fail，emit `polish:failed-fallback` + paste raw Whisper text + HUD 顯示 warning icon、不擋 paste 流程

## Subagent dispatch pattern（M3 證實）

承襲 M0 + M1 + M2 模式、規模較 M2 大（4 chunks vs M2 的 3 chunks），plan-time challenger 介入點是新增。完整 sequence：

1. **Plan-time challenger（並行 dispatch）**：在拆計畫的同一則 message dispatch challenger subagent（`general-purpose`、Opus 4.7）、讀同樣的 spec、產出 Q1–Q5 「沒考慮過的問題」。Main session 對 challenger findings refine plan、修計畫比修代碼便宜
2. **4 個 implementation chunks 順序執行**（chunks 共享 `Cargo.toml` + `lib.rs` 且依賴累進，sequential 必要）：
   - Chunk 0（M2 retro 收尾）：subagent A → reviewer 驗（catch `MAX_WAV_BYTES` 防呆 + `clear_recording_buffer` correctness）
   - Chunk 1（Credentials）：subagent B → reviewer 驗（catch keyring per-target features + `pub(crate) get_credential` 不洩 IPC）
   - Chunk 2（Transcription dispatcher + Groq cloud）：subagent C → reviewer 驗（catch `cloud.rs` 620 → split parser、Q2 invariant pre-check size BEFORE take()、retry policy 範圍）
   - Chunk 3（Test connection + UI + docs）：subagent D（本 session）→ 後續 reviewer + retro challenger 待 main session 跑
3. **Main session 不寫 code、只 orchestrate + commit + 文件 review**：4 個 commits 親手 commit、commit message 親寫
4. **Retro challenger（milestone 完成後）**：dispatch challenger subagent 對 4 個 commits + 本 session log 反向審視、產出「latent 問題 / UX 缺口 / perf 風險」清單 append `IDEAS.md`，抓 chunk-by-chunk reviewer 漏掉的整體性問題

對 M4（hotkey + paste、跨 OS API 重活）建議同 4-chunk 結構、但 chunk 1 - 2 因為 Windows hooks 的 thread + ack 設計很細，可能 reviewer 比 implementer 更花時間。M7 (whisper.cpp FFI binding spike) 因 build pipeline debug 風險高，可能 main session 直接介入。

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M4 將是下一個 milestone）
2. 本檔（M3 session log）— 特別是「Follow-ups for M4」段
3. `.claude/IDEAS.md`
4. `doc/plans/02-implementation-roadmap.md` M4 section（全域熱鍵 + paste）
5. `doc/plans/03-rust-modules.md` `hotkey_listener` + `clipboard_paste` sections
6. `doc/plans/04-frontend-structure.md` `useVoiceFlowStore` 設計
7. `doc/reference/sayit-improvements.md`（SayIt 的 hotkey listener 1566 行 + paste 流程踩過的雷）
