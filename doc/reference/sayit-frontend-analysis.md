# SayIt 前端架構分析報告

> **來源**：Opus subagent (general-purpose) 對 `C:\Users\lulub\Downloads\SayIt` 的深度分析
> **日期**：2026-05-02
> **分析範圍**：Vue 3 + TypeScript 前端（不含 Rust 後端、CI、測試）

## 1. Overall Frontend Architecture

SayIt 是一個 **dual-window Tauri v2 desktop app**，兩個完全獨立的 Vue 3 entry points 共享同一個 `src/` tree，全部使用 `<script setup lang="ts">`（專案規則禁用 Options API）。

**視窗拓撲：**

- **HUD Window (`label: main`)** — 400x100 透明 always-on-top 「notch」 overlay，由 `index.html` → `src/main.ts` → `App.vue` → `NotchHud.vue` 渲染。閒置時隱藏，錄音/轉錄/增強流程中顯示，並呼叫 `setIgnoreCursorEvents(true)` 避免搶走點擊。
- **Dashboard Window (`label: main-window`)** — 標準裝飾 960x680（min 720x480）視窗，由 `main-window.html` → `src/main-window.ts` → `MainApp.vue` 配 `vue-router` 渲染 5 個 view。

**Entry points：**

- `src/main.ts` — 啟動 HUD：建立 Pinia、mount `App.vue`、呼叫 `initSentryForHud(app)`、attach `unhandledrejection` + `app.config.errorHandler` hooks。
- `src/main-window.ts` — async `bootstrap()`：建立 Pinia、呼叫 `initSentryForDashboard(app, router)`、await `initializeDatabase()` **在** `app.mount()` 之前（避免 View 的 `onMounted` 看到未初始化的 DB），然後 sequentially `loadSettings()` → `consumeUpgradeNotice()` → `initializeAutoStart()`。也透過 `document.addEventListener("contextmenu", e => e.preventDefault())` 停用 WebView 右鍵 context menu，並 queue 一個 microtask 跑 `cleanup_old_recordings`。

**Routing** (`src/router.ts`)：`createWebHashHistory`（為了配合 Tauri 的 file:// protocol）只在 Dashboard window 上。Routes：`/` → redirect `/dashboard`，加上 `/history`、`/dictionary`、`/settings`、`/guide`。HUD 沒有 router。

## 2. Directory Structure

```
src/
├── App.vue                  HUD root (NotchHud + voice flow init)
├── MainApp.vue              Dashboard root (Sidebar + RouterView + AlertDialogs)
├── main.ts                  HUD entry point
├── main-window.ts           Dashboard entry point
├── router.ts                Dashboard-only router
├── style.css                Tailwind v4 + tw-animate-css + theme tokens
├── assets/                  Static assets (logo-yan.png)
├── components/              App-specific Vue components
│   └── ui/                  shadcn-vue primitives (auto-generated, ESLint-ignored)
├── composables/             Reusable logic with refs (useTauriEvents, etc.)
├── i18n/                    vue-i18n setup, language config, prompt templates
│   └── locales/             zh-TW, en, ja, zh-CN, ko JSON message bundles
├── lib/                     Pure modules (no Vue/Pinia deps mostly): API clients, utils
├── stores/                  Pinia stores (4 stores)
├── types/                   Shared TS type definitions
└── views/                   Route-level components (Dashboard, History, etc.)
```

## 3. Component Library Setup

`components.json` 宣告 shadcn-vue 用 `style: "new-york"`、`baseColor: "neutral"`、CSS variables on、icons 用 `lucide`、alias map：`@/components`、`@/lib/utils`、`@/components/ui`、`@/lib`、`@/composables`。

**Tailwind v4** 透過 Vite plugin `@tailwindcss/vite` 整合（沒有 `tailwind.config.js` — config 寫在 `style.css` `@theme inline { ... }`）。Theme tokens 用 `oklch()` 與 hex values 宣告在 `:root` 與 `.dark`。語意色彩系統（`--background`、`--foreground`、`--card`、`--primary`、`--muted`、`--accent`、`--destructive`、`--warning`、`--border`、`--ring`、`--chart-1..5`、`--sidebar-*`）透過 `@theme inline { --color-foreground: var(--foreground); ... }` 對應到 Tailwind classes。所有 CSS 用 `bg-card`、`text-muted-foreground`，永遠不用 `bg-zinc-900`。

`src/components/ui/` 包含 21 個 shadcn-vue primitives（alert-dialog、avatar、badge、button、card、chart、checkbox、dropdown-menu、input、label、radio-group、select、separator、sheet、sidebar、skeleton、switch、table、tabs、textarea、tooltip）。Sidebar primitive 一個就 25 檔案（`SidebarProvider`、`SidebarMenuButton`、`SidebarTrigger` 等），底層建構在 `reka-ui`（Vue port of Radix）。`src/lib/utils.ts` export canonical `cn(...inputs: ClassValue[])` 用 `clsx + tailwind-merge`。`Button.vue` 用 `class-variance-authority` (`cva`) 做 variant prop system：`default | destructive | outline | secondary | ghost | link` × `default | sm | lg | icon | icon-sm | icon-lg`。

**Icons** — `lucide-vue-next` 是 canonical icon library。`@tabler/icons-vue` 在 package.json 但只用在三個 legacy 通用 shadcn templates（`AppSidebar.vue`、`NavMain.vue`、`NavUser.vue`、`NavSecondary.vue`、`NavDocuments.vue`、`SectionCards.vue`），這些是未使用的 stock components。CLAUDE.md 明確禁用 `@tabler/icons-vue` 之後的新程式碼。

**`tw-animate-css`** import 在 `style.css` 提供 reka-ui 用的動畫 keyframes（sidebar slide、dialog fade）。

## 4. State Management — Pinia Stores

所有 stores 用 **composition-API style**（`defineStore("name", () => { ... return { ... } })`）— 沒有 Options API。位於 `src/stores/`。

**`useSettingsStore` (`useSettingsStore.ts`, 1395 行)** — 持久化與設定 hub。背後是 `tauri-plugin-store` 透過 `load("settings.json")`（與 SQLite 區隔，遵循「❌ SQLite 存 API Key」規則）。管理：`hotkeyConfig` (TriggerKey + TriggerMode)、`apiKey` (Groq) + `openaiApiKey` + `anthropicApiKey` + `geminiApiKey`、`aiPrompt` + `promptMode` ('minimal' | 'active' | 'custom')、`selectedLlmProviderId`、`selectedLlmModelId`、`selectedWhisperModelId`、`selectedLocale`、`selectedTranscriptionLocale`、`isMuteOnRecordingEnabled`、`isSmartDictionaryEnabled`（Mac 預設 true — Windows 沒有 AX text-field reading）、`isSoundEffectsEnabled`、`isRecordingAutoCleanupEnabled` + `recordingAutoCleanupDays`、`selectedAudioInputDeviceName`、`customTriggerKey` + `customTriggerKeyDomCode`、`enhancementThresholdEnabled` + `enhancementThresholdCharCount`。每個 save method 發送 `SETTINGS_UPDATED` cross-window event，讓 HUD 的 `App.vue` 呼叫 `refreshCrossWindowSettings()`。透過 `syncHotkeyConfigToRust()` 呼叫 `invoke("update_hotkey_config")` 把熱鍵設定推給 Rust。自動 migrate 棄用的 model IDs（例如 Kimi K2 → Llama 3.3 70B）。首次啟動時透過 `detectSystemLocale()` 偵測系統 locale。

**`useVoiceFlowStore` (`useVoiceFlowStore.ts`, ~2000 行)** — 最大的 store；orchestrate 整個語音轉文字 pipeline：hotkey → record → transcribe → enhance → paste → quality monitor → correction monitor。管理 `status: HudStatus`（'idle' | 'recording' | 'transcribing' | 'enhancing' | 'editing' | 'success' | 'error' | 'cancelled'）、`message`、`recordingElapsedSeconds`、`editSourceText`、`lastFailedAudioFilePath`（for retry）、與 timer state。Subscribe events：`HOTKEY_PRESSED`、`HOTKEY_RELEASED`、`HOTKEY_TOGGLED`、`HOTKEY_ERROR`、`HOTKEY_MODE_TOGGLE`、`QUALITY_MONITOR_RESULT`、`CORRECTION_MONITOR_RESULT`、`ESCAPE_PRESSED`。Expose `handleStartRecording()`、`handleStopRecording()`、`handleRetryTranscription()`。實作 double-tap detection (`waitForDoubleTapResolution()` returns Promise resolving in 400ms)、HUD positioning per monitor (`repositionHudToCurrentMonitor` polls every 250ms during high-priority states using `get_hud_target_position`)、與精密的 correction-detection flow（paste 後每 500ms 輪詢 `read_focused_text_field`，然後送 diff 給 `analyzeCorrections()` 抽出 proper-noun corrections）。呼叫 `transitionTo()` 作為 state machine 設置 timers 與 visibility。

**`useHistoryStore` (`useHistoryStore.ts`, 580 行)** — SQLite-backed 透過 `getDatabase()` from `lib/database.ts`。管理 `transcriptionList`、`dashboardStats`、`recentTranscriptionList`、`dailyUsageTrendList`，paginated search (PAGE_SIZE=20)。所有 SQL constants 定義在 module scope（例如 `INSERT_SQL`、`SEARCH_PAGED_SQL`）。有 `mapRowToRecord()` snake_case → camelCase converter for `RawTranscriptionRow`。`addTranscription()` 後發送 `TRANSCRIPTION_COMPLETED` 給 `main-window` 讓 Dashboard 重新整理。也寫 `api_usage` rows（FK 依賴 transcriptions row，順序很重要）。Time-saved 估計用 `ASSUMED_TYPING_SPEED_CHARS_PER_MIN = 40`。

**`useVocabularyStore` (`useVocabularyStore.ts`, 200 行)** — 管理 user dictionary terms（manual + AI-suggested）。用 `crypto.randomUUID()` 做 IDs（TS 前端，不是 DB-side）。Computeds：`manualTermList`、`aiSuggestedTermList`、`termCount`。每個 mutation 後發送 `VOCABULARY_CHANGED` cross-window。`batchIncrementWeights()` 在貼上文字中出現某 term 時 bump weight。`getTopTermListByWeight(50)` 提供 vocabulary slice 給 Whisper 當 prompt biasing list 與 LLM enhancement。

**Inter-store dependencies** — `useVoiceFlowStore` import `useSettingsStore`、`useVocabularyStore`、`useHistoryStore`。`useHistoryStore` 直接呼叫 Tauri commands。遵循「views/ ──→ components/ + stores/ + composables/，stores/ ──→ lib/」規則 — views 永遠不直接 import lib。

## 5. Composables

位於 `src/composables/`：

- **`useTauriEvents.ts`** — Re-export `listen as listenToEvent`、`emit as emitEvent`、`emitTo as emitToWindow` 從 `@tauri-apps/api/event`，加上所有 event-name constants（例如 `HOTKEY_PRESSED = "hotkey:pressed" as const`）。專案規則明確禁止直接 import Tauri 的 event API；所有 event listening 都必須透過這層做集中化的 constants。
- **`useFeedbackMessage.ts`** — Tiny composable：`{ message: Ref<string>, type: Ref<'success'|'error'|''>, show, clearTimer }`。2500ms 後自動清除。被 SettingsView、DictionaryView 與 MainApp 的 update UI 使用。
- **`useAudioPreview.ts`** — Listen `AUDIO_PREVIEW_LEVEL` events from Rust，用 `lerp(0.2)` 配 `useRafFn` from `@vueuse/core` 把 `previewLevel` ref 動畫到 `targetLevel`。Expose `startPreview(deviceName)` / `stopPreview()`。透過追蹤 `startRequestId` 防止 race conditions。Used in SettingsView for mic device picker。
- **`useAudioWaveform.ts`** — Listen `AUDIO_WAVEFORM` events（Rust 送 6-element `levels: [f32; 6]`），interpolate via `lerp(0.25)` per-RAF，expose `waveformLevelList` ref、`startWaveformAnimation()`、`stopWaveformAnimation()`。Used by `NotchHud.vue` to render 錄音波形 bars。

## 6. Views

全部在 `src/views/`：

**`DashboardView.vue` (310 行)** — Stats grid（6 cards：total recording time、characters、time saved、transcriptions、avg chars、daily quota progress with hover tooltip showing per-API breakdown）、`DashboardUsageChart`（30-day line/area chart via `@unovis/vue` `VisXYContainer`）、與 recent transcriptions list。動態排除 LLM quota cards 當在 paid provider 上時（因為 `freeQuotaRpd === 0`）。Listen `TRANSCRIPTION_COMPLETED` 自動 refresh。

**`HistoryView.vue` (380 行)** — 可搜尋、可展開、paginated transcription list with infinite scroll via `IntersectionObserver` on `sentinelRef`。每個 row show raw text + processed text + timing breakdown + Play button。Audio playback 用 `invoke<number[]>("read_recording_file")` 然後 `new Blob([new Uint8Array(raw)])` + `URL.createObjectURL`（macOS WKWebView IPC quirk where binary 變成 `number[]`）。Search debounced at 300ms。Copy 用 `invoke("copy_to_clipboard")`（NOT browser clipboard API）。

**`DictionaryView.vue` (280 行)** — 兩段式 vocabulary management（AI-recommended + manually added）用 `Table` shadcn primitive，weight Badge with `getWeightVariant(weight)`（'default' if ≥30、'secondary' if ≥10、'outline' otherwise）。Inline duplicate detection via `vocabularyStore.isDuplicateTerm()`。Date formatted with locale-aware `toLocaleDateString`。SQLite 存 `created_at` as UTC without timezone suffix，所以 view appends `"Z"` 才能正確 parse。

**`SettingsView.vue` (~2000 行，最大的 view)** — 熱鍵錄製 UI（preset key dropdown OR custom key recording mode listening for `HOTKEY_RECORDING_CAPTURED` / `HOTKEY_RECORDING_REJECTED` events with 10s timeout）、Trigger Mode RadioGroup (hold/toggle)、Audio Input Device picker with live mic preview using `useAudioPreview`、LLM Provider Select (Groq、OpenAI、Anthropic、Gemini) with conditional API key inputs、LLM Model Select filtered by provider、Whisper Model Select、language Select (UI + transcription separate)、prompt mode RadioGroup (minimal/active/custom) with custom Textarea、enhancement threshold Switch + char count Input、mute-on-recording Switch、sound effects Switch、smart dictionary Switch (Mac only)、recording auto-cleanup Switch + days Input、AlertDialog for "delete all recordings" confirmation、social links footer。

**`FeatureGuideView.vue` (60 行)** — 靜態 i18n-driven feature guide；iterate `featureList` array of `{ key, icon, hasSteps }` rendering `Card` with title + description + optional steps。

## 7. Library/Utils Layer (`src/lib/`)

- **`database.ts` (493 行)** — SQLite connection management via `@tauri-apps/plugin-sql`（Tauri-side SQL plugin、不是 WASM）。兩個 entry points：`initializeDatabase()`（Dashboard-only — 建 schema + 跑 migrations）與 `connectToDatabase(maxRetries=100, retryDelayMs=100)`（HUD-only — poll 直到 Dashboard 的 pool ready，不呼叫 `Database.load()` 因為那會在 migration 中途取代 pool）。Migration runner uses `schema_version` table，目前在 v8。DDL `ALTER TABLE` 跑在 transactions **外**（tauri-plugin-sql driver 要求）。用 `BEGIN/COMMIT/ROLLBACK` for batched DDL。有 partial migrations 的 recovery logic。PRAGMAs：`journal_mode = WAL`、`synchronous = NORMAL`、`busy_timeout = 5000`。
- **`autoUpdater.ts`** — Wrap `@tauri-apps/plugin-updater`。Module-level `pendingUpdate: Update | null` cache。Functions：`checkForAppUpdate()` 回傳 `UpdateCheckResult { status: "up-to-date" | "update-available" | "error", version?, error? }`、`downloadUpdate()`、`installAndRelaunch()`（呼叫 `invoke("request_app_restart")`）、`downloadInstallAndRelaunch()`（one-shot manual flow）。
- **`enhancer.ts`** — `enhanceText(rawText, apiKey, options)` 透過 `@tauri-apps/plugin-http` `fetch`（**不是** browser fetch — 專案規則明確禁用 `window.fetch`）呼叫 LLM。Build system prompt with vocabulary appended via `<vocabulary>` XML tags（max 50 terms）。`withTimeout()` wrapper races against AbortSignal + timeout。`stripReasoningTags()` 移除推理模型如 Qwen3 的 `<think>...</think>` blocks。Throw `EnhancerApiError(statusCode, statusText, body)` for HTTP errors。
- **`llmProvider.ts`** — Multi-provider abstraction。`LLM_PROVIDER_LIST` 定義 4 providers (groq、openai、anthropic、gemini) with `baseUrl`、`consoleUrl`、`apiKeyPrefix`。`buildFetchParams(providerId, request, apiKey)` dispatch to `buildOpenAiCompatibleFetchParams`（Groq+OpenAI，OpenAI 用 `max_completion_tokens`、Groq 用 `max_tokens`）、`buildAnthropicFetchParams`（extract `system` to top level、用 `x-api-key` + `anthropic-version: 2023-06-01` headers）、或 `buildGeminiFetchParams`（`/models/{model}:generateContent`、用 `system_instruction` + `generationConfig`、`x-goog-api-key` header）。`parseProviderResponse` 同樣 dispatch。Per-provider timeout：groq=5s、openai/anthropic/gemini=30s。
- **`modelRegistry.ts`** — Static catalog of `LlmModelConfig` and `WhisperModelConfig` with `displayName`、`badgeKey` (i18n)、`speedTps`、`inputCostPerMillion`、`outputCostPerMillion`、`freeQuotaRpd`、`freeQuotaTpd`。Export `LLM_MODEL_LIST`（9 models）、`WHISPER_MODEL_LIST`（2 models）、`DECOMMISSIONED_MODEL_MAP` for auto-migration、helper functions `findLlmModelConfig`、`getModelListByProvider`、`getDefaultModelIdForProvider`、`getEffectiveLlmModelId`（with fallback chain）。
- **`apiPricing.ts`** — `calculateWhisperCostCeiling(audioDurationMs, modelId)` 與 `calculateChatCostCeiling(totalTokens, modelId)`。Whisper 強制 `WHISPER_MIN_BILLING_MS = 10_000`（Groq 的 per-call minimum）。Chat ceiling 用 `Math.max(input, output)` 保守估計。
- **`vocabularyAnalyzer.ts`** — `analyzeCorrections(pastedText, fieldText, apiKey, {modelId})` 送 system prompt 要 LLM 比對 `<original>` vs `<corrected>` 並 ONLY 回傳 new proper-noun corrections（不是 common words、不是 punctuation diffs、不是 user additions、不是 single Chinese chars）。回傳 `{ suggestedTermList, usage, rawResponse }`。對 non-JSON responses 有 fallback regex extraction (`/\[[\s\S]*?\]/`)。
- **`hallucinationDetector.ts`** — Pure functions、無 Vue/Pinia/Tauri dependency。`detectHallucination({rawText, recordingDurationMs, peakEnergyLevel, rmsEnergyLevel, noSpeechProbability})` 跑 two layers：Layer 1 = speed anomaly（recording <1s 但 text >10 chars = physically impossible）；Layer 2 = silence detection (peak < 0.01 OR (peak < 0.03 AND rms < 0.015 AND nsp > 0.7))。`detectEnhancementAnomaly()` flag length explosion >2x。
- **`errorUtils.ts`** — i18n-aware error message extractors：`getMicrophoneErrorMessage`、`getTranscriptionErrorMessage`（parse Groq HTTP status from message：401→invalidApiKey、429→rateLimited 等）、`getEnhancementErrorMessage`（handle `EnhancerApiError`）、`getHotkeyErrorMessage`、加上 generic `extractErrorMessage(unknown)`。
- **`formatUtils.ts`** — `formatTimestamp(ms)`、`truncateText(text, 50)`、`getDisplayText(record)`、`formatDurationFromMs/formatDuration/formatDurationMs`、`formatNumber`、`formatCostCeiling`。永遠用當前 i18n locale。
- **`keycodeMap.ts` (380 行)** — DOM `event.code` → platform-native keycode (macOS CGEvent u16 vs Windows VK u16)。Export lookup helpers for SettingsView's hotkey-recording UI。
- **`sentry.ts`** — `initSentryForHud(app)` 與 `initSentryForDashboard(app, router)` from `@sentry/vue`。讀 `import.meta.env.VITE_SENTRY_DSN`、`VITE_SENTRY_ENVIRONMENT`、`VITE_SENTRY_RELEASE`（預設 `sayit@${__APP_VERSION__}`）、`VITE_SENTRY_TRACES_SAMPLE_RATE`。只在 PROD 啟用。每個 Sentry event tag with `window: "hud"` or `"dashboard"`。`captureError(error, context)` 是 canonical reporting helper used throughout。
- **`utils.ts`** — 只有 `cn(...inputs)` for class merging。

## 8. TypeScript Types

位於 `src/types/`。命名慣例 per CLAUDE.md：

| 後綴 | 用途 | 範例 |
|---|---|---|
| `*Payload` | Tauri Event payload | `VoiceFlowStateChangedPayload`、`HotkeyEventPayload`、`WaveformPayload` |
| `*Record` | SQLite row | `TranscriptionRecord`、`ApiUsageRecord` |
| `*Config` | Settings object | `HotkeyConfig`、`LlmProviderConfig` |
| `*Entry` | Dictionary item | `VocabularyEntry` |
| `*Handle` | Resource control | `AudioAnalyserHandle` |
| `*Result` | API/IPC return | `TranscriptionResult`、`StopRecordingResult`、`EnhanceResult` |

`types/index.ts` export `HudStatus`、`HudState`、`TriggerMode`、`HudTargetPosition`。`types/settings.ts` 定義 tagged-union `TriggerKey = PresetTriggerKey | CustomTriggerKey | ComboTriggerKey` with type guards (`isPresetTriggerKey`、`isCustomTriggerKey`、`isComboTriggerKey`)。`types/events.ts` cover 所有 cross-window event payloads。`types/transcription.ts` 與 `types/audio.ts` 定義 IPC return shapes。`types/vocabulary.ts` 有 `VocabularyEntry` 與 `VocabularySource = "manual" | "ai"`。

## 9. Tauri Bridge

**Frontend → Rust** 用 `invoke<ReturnType>("command_name", { args })` from `@tauri-apps/api/core`。CLAUDE.md 列 30+ commands；frontend 呼叫的 key ones：`update_hotkey_config`、`paste_text`、`copy_to_clipboard`、`start_recording`/`stop_recording`/`save_recording_file`/`read_recording_file`、`transcribe_audio`/`retranscribe_from_file`、`mute_system_audio`/`restore_system_audio`、`start_quality_monitor`/`start_correction_monitor`、`read_focused_text_field`/`read_selected_text`、`start_audio_preview`/`stop_audio_preview`、`play_start_sound`/`play_stop_sound`/`play_error_sound`/`play_learned_sound`、`check_accessibility_permission_command`/`open_accessibility_settings`/`reinitialize_hotkey_listener`、`start_hotkey_recording`/`cancel_hotkey_recording`、`request_app_restart`、`debug_log`。

**Rust → Frontend** 用 Tauri events listened via `useTauriEvents.ts` abstraction（**不是**直接 import）。Constants：`HOTKEY_PRESSED`、`HOTKEY_RELEASED`、`HOTKEY_TOGGLED`、`HOTKEY_ERROR`、`HOTKEY_MODE_TOGGLE`、`ESCAPE_PRESSED`、`HOTKEY_RECORDING_CAPTURED`、`HOTKEY_RECORDING_REJECTED`、`QUALITY_MONITOR_RESULT`、`CORRECTION_MONITOR_RESULT`、`AUDIO_WAVEFORM`、`AUDIO_PREVIEW_LEVEL`。Frontend-only events（HUD ↔ Dashboard via `emitTo("main-window", ...)`）：`VOICE_FLOW_STATE_CHANGED`、`TRANSCRIPTION_COMPLETED`、`SETTINGS_UPDATED`、`VOCABULARY_CHANGED`、`VOCABULARY_LEARNED`。HUD subscribe `SETTINGS_UPDATED` 呼叫 `refreshCrossWindowSettings()` 讓 Dashboard 的設定 sync 回來。

**Window control** 用 `getCurrentWindow()` from `@tauri-apps/api/window` 呼叫 `show()`、`hide()`、`setFocus()`、`setIgnoreCursorEvents()`、`setPosition(new LogicalPosition(x, y))`。HUD 的 `App.vue` 在 startup 呼叫 `Window.getByLabel("main-window")` 把 dashboard 帶到前面。

**macOS-specific quirk**：Per CLAUDE.md，`tauri::ipc::Response` raw bytes 在 macOS WKWebView 是 `number[]`（JSON-serialized），不是 `ArrayBuffer`。HistoryView 處理方式：`new Blob([new Uint8Array(raw)], { type: "audio/wav" })`。

## 10. Build Configuration

**`vite.config.ts`** 用 `@vitejs/plugin-vue` 與 `@tailwindcss/vite`。`define: { __APP_VERSION__: JSON.stringify(version) }` 從 `package.json` 注入。Alias `@/* → ./src/*`。Multi-window build via `rollupOptions.input: { main: "index.html", "main-window": "main-window.html" }`。Server config：port 1420 (strict)、HMR 用 `TAURI_DEV_HOST` env for mobile/network testing。Watches ignore `**/src-tauri/**`。Sourcemaps gated by `VITE_SENTRY_SOURCEMAPS_ENABLED === "true"`。`envPrefix: ["VITE_", "TAURI_"]`。

**`tsconfig.json`** — strict mode on、`target: ES2021`、`module: ESNext`、`moduleResolution: "bundler"`、`noUnusedLocals` + `noUnusedParameters` + `noFallthroughCasesInSwitch` 啟用、`paths: { "@/*": ["./src/*"] }`。Build script 是 `vue-tsc --noEmit && vite build`，型別錯誤會 fail build。

**`eslint.config.js`** — flat config combining `@eslint/js` recommended、`typescript-eslint` recommended、`eslint-plugin-vue` flat/recommended。停用 `vue/multi-word-component-names`（shadcn primitives 是 single-word）、`@typescript-eslint/no-unused-vars`（deferred to vue-tsc）、Vue formatting rules（專案不用 Prettier）。Ignore `src/components/ui/**`（auto-generated）、`dist/**`、`src-tauri/**`。`@typescript-eslint/no-explicit-any: "warn"` 因為 Tauri IPC boundaries 有時需要 any。

## 11. Vitest Setup

**`vitest.config.ts`** — `environment: "jsdom"`、`globals: true`、`include: ["tests/unit/**/*.test.ts", "tests/component/**/*.test.ts"]`。Coverage via `@vitest/coverage-v8`、include `src/**/*.ts` 與 `src/**/*.vue`、exclude entry points 與 `*.d.ts`。Vue plugin 啟用。

**Test layout**：`tests/unit/` 包含 14 unit tests cover stores（`use-voice-flow-store.test.ts`、`use-settings-store.test.ts`、`use-history-store.test.ts`、`use-vocabulary-store.test.ts`）、pure libs（`enhancer.test.ts`、`error-utils.test.ts`、`format-utils.test.ts`、`hallucination-detector.test.ts`、`api-pricing.test.ts`、`auto-updater.test.ts`、`llmProvider.test.ts`）、與 `factories.test.ts`、`i18n-settings.test.ts`、`types.test.ts`。`tests/component/` 有 3 component tests（`NotchHud.test.ts`、`AccessibilityGuide.test.ts`、`i18n-smoke.test.ts`）。

**Mocking strategy**：`vi.hoisted()` 在 module imports 前宣告 mocks。Tauri APIs（`@tauri-apps/api/core`、`@tauri-apps/api/event`、`@tauri-apps/api/window`）在 module level mock with `vi.mock()`。Listener callbacks 抓進 `Map<eventName, callback>` 讓 tests 可合成 dispatch events。Pinia tests 在 `beforeEach` 用 `setActivePinia(createPinia())`。Faker-based factories at `tests/support/factories/` 產生 randomized but typed `TranscriptionRecord` / `VocabularyEntry` instances 避免 parallel-execution conflicts。

## 12. Notable Patterns and Conventions

1. **Two-window message bus** — Cross-window state sync 全部透過 Tauri events（無 shared state）。Settings save → emit `SETTINGS_UPDATED` → HUD refresh store。HUD detect vocabulary correction → emit `VOCABULARY_CHANGED` 與 `VOCABULARY_LEARNED` → Dashboard refresh；HUD show "Learned X" notification。

2. **Strict layering** — Per CLAUDE.md：`views → components/stores/composables`、`stores → lib`、`lib → external APIs`。Views 永遠不直接 import lib。Components 永遠不直接執行 SQL。

3. **State machine in HUD** — `useVoiceFlowStore.transitionTo(status, message)` 是 single chokepoint 處理 HUD show/hide、autohide timers (success=1s、cancelled=1s、error=3s、error+retry=6s)、與 event emission。Status values 是 exhaustive：idle、recording、transcribing、enhancing、editing、success、error、cancelled。

4. **NotchHud visual modes** — `NotchHud.vue` 有自己內部的 `VisualMode`（10 values：hidden、recording、morphing、transcribing、success、error、cancelled、collapsing、learned、mode-switch）decoupled from `HudStatus`。`morphing` 中間 state play 300ms transition animation。Use CSS `clip-path: path(...)` 配 computed SVG path 做動態形狀的 notch（不同 mode 不同參數）。

5. **Lerp animation** — `useAudioPreview` 與 `useAudioWaveform` 都用 `useRafFn` from `@vueuse/core` 在 60fps interpolate values toward Rust-pushed targets，decouple render rate 與 event arrival rate。

6. **Lazy module imports for plugins** — `await import("@tauri-apps/plugin-autostart")`、`await import("./lib/autoUpdater")`、`await import("./stores/useHistoryStore")` 在 handlers 內 dynamic-imported 來 defer initial bundle cost。

7. **SQLite naming convention** — Tables plural snake_case（`transcriptions`、`api_usage`、`vocabulary`）、columns snake_case 透過 explicit `mapRowToRecord()` 對應 camelCase（無 ORM）。Booleans 存 INTEGER、decode via `row.was_enhanced === 1`。Nullable booleans：`row.was_modified === null ? null : row.was_modified === 1`。UUIDs frontend 透過 `crypto.randomUUID()` 產生、parameter syntax `$1, $2`（tauri-plugin-sql）。

8. **Always-on-top HUD with click-through** — `App.vue` 在 show 後呼叫 `appWindow.setIgnoreCursorEvents(true)`、只在 `error` state 切回 `false` 讓使用者點 retry button。

9. **Smart correction detection** — paste 後 `useVoiceFlowStore` 每 500ms poll `read_focused_text_field`、listen `CORRECTION_MONITOR_RESULT` from Rust（detect key activity）、然後計算 overlap ratio (must be ≥30%) 才送 diff 給 LLM 抽 vocabulary。Skip when text unchanged or unrelated。

10. **Conservative cost estimates** — `formatCostCeiling` 永遠 show "≤ $X.XXXX" with the higher of input/output token cost、而不是 exact number、傳達不確定性給使用者。

## 13. Frontend Dependencies (`package.json`)

**Runtime dependencies：**

- `@sentry/vue ^10.42.0` — Error tracking、Vue integration with router instrumentation
- `@tabler/icons-vue ^3.38.0` — Icon set used by stock shadcn templates only（deprecated for new code）
- `@tanstack/vue-table ^8.21.3` — Headless table primitives
- `@tauri-apps/api ^2` — Core Tauri client (invoke、event、window、dpi)
- `@tauri-apps/plugin-autostart ^2.5.1` — OS auto-start at login
- `@tauri-apps/plugin-http ^2.5.7` — HTTP client（mandated over browser fetch）
- `@tauri-apps/plugin-process ^2.3.1` — Process control（used by autoUpdater）
- `@tauri-apps/plugin-shell ^2` — Open URLs/files in OS default app
- `@tauri-apps/plugin-sql ^2.3.2` — SQLite via Tauri（not WASM）
- `@tauri-apps/plugin-store ^2.4.2` — Persistent JSON KV store
- `@tauri-apps/plugin-updater ^2.10.0` — Code-signed binary updates
- `@unovis/ts ^1.6.4` + `@unovis/vue ^1.6.4` — Charts
- `@vueuse/core ^14.2.1` — `useRafFn` 等 utilities
- `class-variance-authority ^0.7.1` — `cva()` for shadcn variant props
- `clsx ^2.1.1` — Conditional className composition
- `lucide-vue-next ^0.576.0` — Canonical icon library
- `pinia ^3.0.4` — State management
- `reka-ui ^2.8.2` — Vue port of Radix UI
- `tailwind-merge ^3.5.0` — Used by `cn()` utility
- `vue ^3.5` — Composition API、`<script setup>`
- `vue-i18n ^11.3.0` — Internationalization (5 locales)
- `vue-router 5.0.3` — Routing (Dashboard window only, hash mode)

**Dev dependencies：**

- `@eslint/js ^10.0.1`、`eslint ^10.1.0`、`eslint-plugin-vue ^10.8.0`、`typescript-eslint ^8.57.2` — Linting
- `@faker-js/faker ^10.3.0` — Random test data factories
- `@playwright/test ^1.58.2` — E2E testing
- `@tailwindcss/vite ^4`、`tailwindcss ^4`、`tw-animate-css ^1.4.0` — Styling
- `@tauri-apps/cli ^2` — `tauri` command for dev/build
- `@vitejs/plugin-vue ^5`、`vite ^6` — Build tooling
- `@vitest/coverage-v8 ^4.0.18`、`vitest ^4.0.18`、`@vue/test-utils ^2.4.6`、`jsdom ^28.1.0` — Unit/component testing
- `typescript ^5.7`、`vue-tsc ^2` — Type-checking

Package manager pinned to `pnpm@10.28.2`、`onlyBuiltDependencies: ["esbuild"]` to skip postinstall scripts on other native modules。Node 24 (per `.nvmrc`)。
