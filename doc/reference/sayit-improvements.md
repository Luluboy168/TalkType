# SayIt 批判性改進建議報告

> **來源**：Opus subagent (general-purpose) 對 SayIt 整體專案的批判性分析
> **日期**：2026-05-02
> **目的**：找出 successor / competitor app 應該避免或改進的地方
> **立場**：建設性批評，不為 SayIt 辯護也不誇大問題

## 1. Architecture-level concerns

### Tauri v2 是合理但不是 slam-dunk 的選擇

Tauri v2 是合理的選擇：small bundle、Rust-native global hotkeys、OS-level CGEvent / SendInput（Electron 沒 native modules 做不到）。但 changelog 顯示 riding Tauri v2 bleeding edge 的代價：

- WKWebView/WebView2 divergence keeps biting。`CLAUDE.md` 明確警告 `convertFileSrc` 在 macOS 產生 `asset://localhost/` 但 CSP 需要 `http://asset.localhost`、`tauri::ipc::Response` raw bytes 在 macOS serialize 為 `number[]` JSON 而非 `ArrayBuffer`。這些不 trivial — v0.8.6 必須發版因為 audio playback 在 production 是 silent 的。`autoUpdater.ts` 帶有 known limitation：`window.confirm` 在 WKWebView 被 silently swallow
- Hotkey listening 仍需要直接 CGEventTap / SetWindowsHookExW（`hotkey_listener.rs` 1566 行、整份都 `cfg(target_os = "macos")` / `windows` branches）。Tauri 的 `globalShortcut` plugin 沒辦法 capture modifier-only keys like Fn，所以 team 自己 roll 一個。這降低了選 Tauri 的 value

A successor app 應該重新評估：**Wails v3** (Go) 或 **native (Swift on macOS + .NET MAUI/WinUI on Windows)** 各自可以 consolidate 一個 OS 而非跟 WebView 戰鬥。Even **Electron** 值得 second look：bundle size 不再是 existential issue、而你 get single、predictable Chromium runtime — 拿掉 80% 的 changelog bugs。Honest answer：「Tauri 還行，但其最大賣點（small binary）對音頻 app 是錯的優化目標 — 反正每個 utterance 要送 ~1MB WAV 到 remote API」。

### 雙視窗 IPC 是 over-engineered

HUD (`label: main`、470x100) 與 Dashboard (`label: main-window`、960x680) 是 separate webviews communicate via Tauri events 加上 32-command IPC contract。每個 settings change 必須 `emitEvent(SETTINGS_UPDATED)` 保持兩個 windows 同步。

更糟，這引發真實 bugs：

- v0.8.3、v0.8.4、v0.8.5 是三個 back-to-back patch releases、全是「HUD window 的 `Database.load()` overrides Dashboard 的 connection pool、`DROP TABLE` 失去 transaction protection」。Mitigation (`src/lib/database.ts` 79-103) 是 HUD 改用 `Database.get(...)` 並 **poll up to 100×100 ms = 10 秒** 等 Dashboard。這是 code smell — architecture 強迫一個可能不可見的 window 依賴另一個 window 的 lifecycle

A successor 應該把 state 與 DB ownership 留在 **Rust**（Tauri commands return data）讓 HUD 是 pure dumb view 接收 serialized snapshots over events。Renderer 完全沒有 SQL connection pool。

### Vendor lock-in to Groq 是最大策略風險

`tauri.conf.json` line 51 把 Groq hardcode 進 CSP。Whisper transcription endpoint 也 hardcode 在 Rust (`src-tauri/src/plugins/transcription.rs` line 9 `GROQ_API_URL`)。Frontend 對 **LLM polishing** 有 multi-provider abstraction (`src/lib/llmProvider.ts` v0.9.2 加 OpenAI、Anthropic、Gemini) — 但 **transcription 本身是 Groq-only**。

Groq 今天快又便宜，但：free-tier RPDs 季變、models 棄用 (v0.7.1 必須在 mid-flight migrate users off Llama 4 Maverick)、launch day 的一個 rate-limit incident 會 fail every user。Successor 應該：

- 加 **whisper.cpp** as offline fallback。Repo 內 zero whisper.cpp / on-device support（`Grep` 找不到 matches in code）。即使「無 API key + offline model downloaded 時用 offline」UX 都會是 major differentiator
- 讓 users plug in **OpenAI Whisper、Deepgram、AssemblyAI、ElevenLabs Scribe**。Whisper-compatible adapter pattern (匹配 `llmProvider.ts` 結構) 直接

### 沒有 offline-first story

Architecture document 明確 acknowledge (line 84)「沒其他 cloud dependencies」但 entire core flow 需要 internet。沒有 queueing recordings 等 network 回來、沒有 offline transcription、沒有 local-first design。Bad Wi-Fi at coffee shop 的 user 收到 hard error。

## 2. 技術債與 code-smell signals

最大 file 是 smell signal：

| File | Lines | Concern |
|------|-------|---------|
| `src/views/SettingsView.vue` | **1907** | God-view；settings、hotkey recorder、mic preview、model picker、prompt editor、vocabulary import 全部都在這 |
| `src/stores/useVoiceFlowStore.ts` | **1871** | State machine + recording + transcription + enhancement + retry + edit-mode + correction monitor + sound feedback 全在一個 Pinia store |
| `src-tauri/src/plugins/hotkey_listener.rs` | **1566** | Single file hold Hold/Toggle modes、double-tap detection、long-press、combo、hotkey recording mode、macOS CGEventTap 與 Windows hook |
| `src/stores/useSettingsStore.ts` | **1395** | 23 documented sections per `project-context.md` |
| `src-tauri/src/plugins/audio_recorder.rs` | **1116** | Recording、preview、FFT、device enumeration、file I/O、cleanup |
| `src/components/NotchHud.vue` | **861** | One component、10-state visual-mode machine、multiple timers |

Rust `plugins/` 目錄 hold **~6000 lines across 9 files**。命名也 misleading — 這些不是 Tauri plugins（後者是 separate crates with `Builder::new()`）；它們是 internal modules 用 `#[command]` directly。Successor 應該：

- 把 `useVoiceFlowStore` 拆成 `useRecording`、`useTranscription`、`useEnhancement`、`useEditMode`、加 thin orchestrator state machine on top（XState 會 help — 專案目前手動管 7+ states with timers）
- 拆 `hotkey_listener.rs` 為 `hotkey/{macos.rs, windows.rs, combo.rs, double_tap.rs, recording.rs}` modules
- 1907 行 `SettingsView.vue` 應該 fan out 到 per-section views/components

### Test coverage gaps

`tests/unit/` 16 files / 443 `it`/`test` declarations。Healthy at unit layer for pure logic (enhancer、llmProvider、hallucination detector、error utils、settings store)。但：

- `tests/e2e/smoke.test.ts` 是 **literally one trivial test** 只 check page loads — 而且 assert `toHaveTitle(/whisper/i)`（對叫 "SayIt" 的產品而言錯了）。Hotkey → record → paste 的真實 flow 的 E2E coverage 不存在
- **沒有 Rust integration tests** — 只 inline `#[cfg(test)]` in `transcription.rs` (4 unit tests for `format_whisper_prompt`)
- CGEventTap installation、double-tap timing、paste-after-Cmd+V latency — 全沒有 automated regression coverage。CHANGELOG.md 大部分 bugs (v0.7.3、v0.8.3、v0.8.4、v0.8.5、v0.8.9、v0.9.3、v0.9.4、v0.9.5) 是 platform/timing/concurrency bugs，integration tests 會 catch

## 3. UX / 產品 concerns

### Cross-platform parity 落差是真的

Windows 是明顯的 second-class citizen：

- `src/stores/useSettingsStore.ts` line 69：`DEFAULT_SMART_DICTIONARY_ENABLED = navigator.userAgent.includes("Mac")` 因為 **Windows 沒有 `read_focused_text_field` AX API support**（三處 confirm、包括 `_bmad-output/project-context.md` line 287）
- 「edit selected text」feature (v0.9.0) 用 macOS AXSelectedText (`text_field_reader.rs`) — Windows path 回 `Ok(None)`
- macOS 預設熱鍵選 `Fn` vs Windows 選 `RightAlt` 反映 some Fn keys 不能被 SetWindowsHookExW 攔截 (Microsoft Surface Fn、manufacturer-handled at firmware level)。理解正確 — 但 changelog (v0.9.3)「Globe-key MacBooks 上 Fn key 在 Toggle mode 立刻 fire」顯示 brittle
- v0.9.5 揭露「Windows multi-instance bug — 第二次 SayIt launch 導致 duplicate paste」— single-instance lock 必須加。macOS Launch Services masked 這個 9 個 minor versions

### Latency claim < 3 秒主要靠 Groq

README 說「E2E < 3 秒含 AI」。Groq inference ~300–500 ms。所以 budget 實際是：

```
record stop → upload WAV → Whisper → poll for LLM enhance → paste
   ~0ms        500–1500ms     ~600ms     ~600–1500ms        ~50ms
```

Bottleneck 是 **WAV upload**。SayIt 送 raw 16-bit PCM (`hound` crate)、16kHz mono = ~32 kB/sec。10 秒 utterance ~320 kB on coffee-shop Wi-Fi。Successor 應該：

- 在 request 用 **Opus 或 AAC**（Groq Whisper accept 壓縮 audio）。~5–10× 更小 payload
- Stream chunks while still recording（Whisper streaming via partial uploads）。目前 code 等 record-stop 才 network call (`useVoiceFlowStore.ts` orchestrate：stop → save → transcribe)
- Fire LLM enhancement on partial transcription 而非等 full Whisper response

### 隱私是最大 UX/legal weakness

- **Repo 沒有 privacy policy**（沒 PRIVACY.md、沒 terms file）。Audio 送 Groq (US-based、無 DPA 不 GDPR-friendly)、但 README 與 Settings UI 從未 disclose 這事。`Grep` for `consent | data retention | disclosure` 只找到 BMad templates、不是 user-visible copy
- WAV files 寫成 **plaintext** to `$APPDATA/recordings/{uuid}.wav` (`audio_recorder.rs` line 882–905)。沒有 encryption-at-rest。Laptop 遺失 = transcribed conversations leaked。Auto-cleanup 預設 **off** (`useSettingsStore.ts` line 73 `DEFAULT_RECORDING_AUTO_CLEANUP_ENABLED = false`)
- Rust `read_recording_file` 在 v0.8.6 正確 hardened（now takes `id` not `path` to prevent arbitrary file read）、但 fix 是反應式
- Successor 必須 ship：(a) 明確「your audio is sent to Groq」first-launch disclosure、(b) opt-in audio storage、預設 OFF、(c) 至少 macOS-keychain-based encryption of any retained WAVs

### Accessibility 基本上不存在

`Grep` for `aria-label|aria-live|aria-hidden|role=|prefers-reduced` in `NotchHud.vue` 回 **零 matches**。HUD 是 custom SVG-shaped notch with state-driven CSS animation 與 0 screen-reader hints。`src/` 內無 `prefers-reduced-motion` media query。對 target user 是「打字多的人」— 包括有 motor disabilities 的 users（核心 voice-input personas 之一）— 這是 miss。

### Bad-network / rate-limit handling

`errorUtils.ts` 為 HTTP 429 (`rateLimited`) 回 localized messages。但沒有 retry、沒有 exponential backoff、沒有 queue。Groq 在 day 末打 free-tier RPD limits 時、user 只看到 error toast。Successor 應該 either pre-warn (「today 你有 3 requests left」) 或 auto-fallback to local whisper.cpp。

## 4. Cost & scaling

It's BYOK app、所以 SayIt 自己 cost zero。但 user-facing cost story brittle：

- Dashboard 追蹤 Whisper RPD / billed audio / LLM RPD / LLM TPD (`DashboardView.vue` lines 51–95)、靠讀 Groq response headers + SQL aggregation。Free tier limits hardcoded；Groq silently change 時、display 錯。Successor 應該從 provider 的 `/usage` endpoint 讀 limits where available、不是 constants
- 沒 multi-tenant story（intentional — single-user）。也沒 team / enterprise mode。Hardcoded signing identity in `tauri.conf.json` (`"signingIdentity": "Developer ID Application: Tai-Cheng Chen (G9J8D2T6DV)"`) 表示 fork 不能 build releases without editing config — minor friction

## 5. Platform-specific gotchas worth inheriting carefully

- **macOS Accessibility prompt** 處理得好（dedicated `AccessibilityGuide.vue` 與 `check_accessibility_permission_command`）。保留這 pattern
- **CGEvent paste via Session tap level + Private source** (`clipboard_paste.rs` line 59–87) 是 v0.6.0 LINE-app paste bug 與 right-Option modifier residue 的 clever fix。Hard-won knowledge — 保留
- **macOS `cpal` 0.15.3 CoreAudio Arc-cycle bug** (v0.8.9) 是 serious bug（mic indicator stuck ON after recording）。Successor 要 either upgrade 過 broken version 或 vendor cpal with fix
- **Windows Fn / Globe / virtual-desktop quirks** 在 v0.9.5、v0.9.3 documented — successor 要 day 1 就有 aggressively multi-instance lock、不是 v0.9.5
- **WKWebView CSP / asset:// vs http://asset.localhost** — successor 要早常 test production builds。不要 trust `pnpm tauri dev`

## 6. Security observations

| Concern | Current | Recommendation |
|---------|---------|----------------|
| **API key storage** | Plaintext JSON in `tauri-plugin-store` at `$APPDATA/settings.json`（architecture.md 明確說「明文 JSON、安全依賴 OS 檔案系統權限」） | 用 `keyring` Rust crate (macOS Keychain / Windows Credential Vault)。Plaintext 連基本 threat model 像 stolen disk image 都過不了 |
| **Audio at rest** | WAV plaintext、預設無 auto-cleanup | 用 OS-keychain key 加密、預設 7-day cleanup ON |
| **IPC trust boundary** | Rust commands 直接從 frontend take `api_key: String` (`transcription.rs` line 211)。Frontend 從 store pull 然後 pass down | Either store key in Rust state 從不 marshal through frontend、或至少 tag commands as backend-only。目前 Whisper API key 每個 recording 跨 IPC boundary |
| **Capabilities** | `default.json` 寬：`core:window:default`、`sql:default`、`store:default`、`process:default`。Single capability for both windows | Per-window capabilities、narrower scopes（HUD 不該 access SQL or store） |
| **CSP** | `default-src 'self'`、無 inline event handlers 的 `Content-Security-Policy`。`style-src 'unsafe-inline'` | `unsafe-inline` 可能是 Tailwind v4 dynamic classes 在某些 setup 需要、document why |
| **Updater** | Pubkey hardcoded in tauri.conf.json (correct pattern)、但無 rollback 或 staged rollout | OK as-is；考慮 release channels (stable/beta) |
| **Sentry PII** | `send_default_pii: false` (lib.rs line 393)。Good | Frontend Sentry config 也應 audit transcribed text 的 breadcrumb leakage |

## 7. DX (developer experience)

### BMad-method 對單人開發者太重

`_bmad-output/project-context.md` 794 行、宣告 `rule_count: 323`、README 說「261 rules」。`_bmad/` has commands、agents、workflows、tea-testarch knowledge bases。`_bmad-output/implementation-artifacts/` 18 stories + 17 tech-specs。

For solo / small-team app 這是 **wildly over-instrumented**。323 rules 的 signal-to-noise 可疑：很多 restate shadcn-vue conventions 可以 ESLint enforce。Successor 應該 keep maybe 20–30 invariant rules 並 lean on linters / type-checkers / pre-commit hooks for the rest。

### Claude hooks 設計得好

`.claude/settings.json` 自動跑 `protect-config.sh` (block lock-file edits)、`typecheck.sh`、`rustfmt.sh`、`eslint.sh`。值得 copy — 這些 hooks 在做 real work。

### Test pyramid 是 bottom-heavy 且缺中間

- 443 unit assertions (good)
- 0 Rust integration tests
- 1 trivial Playwright smoke test
- 沒 IPC contract tests（32-command contract documented in `CLAUDE.md` 但沒 auto-validated、即使有 `tauri-reviewer` subagent）

13 GitHub Secrets、dual Sentry DSNs、3-matrix release pipeline 暗示 real production rigor — 但 test layer 不 match。

## 8. What a successor should DO DIFFERENTLY

1. **Ship offline transcription via whisper.cpp** as default-with-fallback。SayIt 沒有的 killer feature — real competitive moat versus pure-cloud tools
2. **Move API key to OS keychain** from day one。Plaintext on disk unacceptable
3. **Audio sent to cloud must be opt-in with first-launch disclosure**。Provide privacy mode
4. **Use Opus/AAC compression for transcription uploads**。~5× smaller payloads、~5× lower mobile-network latency
5. **Adopt real state machine library (XState)**。Replace 手動 timers 與 10-state visual-mode machine in `NotchHud.vue` 與 orchestration in `useVoiceFlowStore.ts`
6. **Rust owns SQL；frontend never opens DB**。Eliminate connection-pool race that 產生 v0.8.3–v0.8.5
7. **Single window with HUD overlay**（或 separate native overlay window with **no** webview）。Webview-based HUD cost ~100MB 並複雜化 IPC for something draws 8 SVG paths
8. **Streaming transcription** — start sending audio chunks while recording continues
9. **Provider-pluggable transcription**、不只 LLM enhancement。Mirror `llmProvider.ts` abstraction for Whisper-compatible APIs
10. **Accessibility budget**：ARIA on HUD、`prefers-reduced-motion`、screen-reader announcements for state transitions

## 9. What SayIt does WELL — keep these

1. **Rust-native audio pipeline (cpal + hound + rustfft)** instead of MediaRecorder。v0.3.0 migration 是正確的；codebase 之後 much more reliable
2. **CGEvent Cmd+V via Session-level Private source** for paste — 解 LINE/Cmd+V failure (v0.6.0) 並 prevent Toggle-mode modifier residue。Hard-won、well-commented (`clipboard_paste.rs` lines 43–88)
3. **Clear IPC contract documentation** in `CLAUDE.md` — 32 commands + 13 events table 是任何 Tauri project 的 model
4. **Dual-window CSP / asset protocol research** documented for posterity。Save 2 weeks of pain
5. **i18n from v0.4.0 with 5 locales**、包括 separating UI locale from Whisper transcription locale (v0.6.0)。不要 ship as English-only

## 10. Open questions a redesign should answer

- **Latency budget**：what's actual P50/P95 end-to-end time on noisy network? README's "< 3 s" is aspirational。沒 tracing 或 metric collection (Sentry traces 在 production `tracesSampleRate=0`)
- **Free-tier sustainability**：what happens when Groq removes free tier or rate-limit aggressively? Does app become unusable 或 fallback to paid path?
- **Edit-mode safety**：v0.9.0 讓 selected text 被 AI output replace。如果 AI 從 Whisper output 加 instruction-injection 怎辦？Is there undo path? User 看到 diff before paste 嗎？
- **Smart-dictionary feedback loop**：v0.7.0 從 corrections 學 — 但 privacy story 是？User corrections 送給 LLM analyzer? (Yes — `vocabularyAnalyzer.ts` 用 corrected text 呼叫 LLM provider)。Disclosed?
- **Multi-monitor + DPI scaling on Windows**：HUD positioning (`get_hud_target_position`) — covered for macOS via `current_monitor()`、less clear on Windows mixed-DPI setups
- **Apple Notarization renewal**：hardcoded signing identity 表示 CI breakage if Developer ID expires
- **What's the upgrade story for the BMad-driven `_bmad/` directory?** Almost a megabyte of workflow definitions — maintained 還是 vestigial?

---

**Bottom line for a successor：** SayIt 是 polished、well-engineered single-purpose tool 其最大 weakness 是 external (Groq lock-in、無 offline path、weak privacy posture) 而非 internal。Copy Rust audio pipeline、IPC discipline、與 macOS paste tricks。重新考慮 dual-window architecture、secrets storage、test pyramid、與 BMad-method overhead。並 ship offline-first whisper.cpp with cloud as upgrade — 不是 the other way around。
