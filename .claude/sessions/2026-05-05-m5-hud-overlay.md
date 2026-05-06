# 2026-05-05 — M5 HUD Overlay

> **Session topic**：完成 Phase 1 Milestone 5（HUD overlay 4 visual states + 6-bar waveform + ARIA + reduced-motion + active-monitor positioning + Dashboard sidebar badge）— `useVoiceFlowStore` async refactor + `dismissError()` + `audio:recording-aborted` listener + cross-window `voice-flow:state-changed` emit；新 Rust `position_hud_for_active_monitor` (raw Win32 `GetCursorPos` + DPI-aware centering) + `set_hud_visible_for_dev`（debug-only via `#[cfg(debug_assertions)]` two-path `generate_handler!`）；HudOverlay `<Transition mode="out-in">` state machine + `setIgnoreCursorEvents` 切換 + `prefers-reduced-motion` matchMedia + click-to-dismiss；HudWaveform 6 bars + dot fallback、HudSpinner CSS spin + 「…」 fallback、HudTimer mm:ss + 9/12 min 顏色警告；HudFlowBadge Dashboard sidebar 紅點脈動「錄音中」；docs/m5-acceptance.md 14 條 manual SOP + 13 vite-shape screenshots。
> **Outcome**：✅ M5 implementation done、5 chunks（chunk 0 Rust + types + tauri.conf → chunk 1 store async refactor → chunk 2 HUD components + i18n → chunk 3 Dashboard sidebar → chunk 4 docs/dashboard）+ 1 chunk 1 reviewer P0 fix（formatError pattern matches actual Rust Display）+ 1 chunk 2 P1/P2 polish（DEV gate + shim + reduced-motion gate）；**53 vitest + 163 cargo tests** pass、0 P0 from chunk 0/2/3 reviewers、13 vite-shape screenshots、靜態檢查全綠；user manual acceptance 14 條待跑（`docs/m5-acceptance.md`）。

## What changed

### M5 implementation（5 chunks）

延續 M0-M4 subagent-driven 模式、規模適中（vs M4 的 5 chunks + 2 reviewer + paste.rs split + 2 acceptance fixes）。**新工法**：

1. **Plan-time challenger 與拆計畫平行 dispatch**（CLAUDE.md item #5 強制）— challenger（agentId `abe35088f7a648bf9`）找出 3 P0 + 8 P1 + 3 P2、主 session 對照 spec/plan refine 後才 dispatch implementer
2. **每個 chunk 完成後 dispatch reviewer subagent**（chunks 0/2/3 各 1 reviewer，chunk 1 reviewer 找 P0 必修）
3. **Reviewer 必跑 Playwright pass**（chunk 2/3 reviewer 都各自跑了 13+ screenshots、CLAUDE.md UI verification SOP）
4. **Chunk 4（docs/bookkeeping）由 implementer subagent 處理**、無 reviewer 必要

### Commit table

| Commit | Type | 內容 |
|---|---|---|
| `8c62865` | docs(m5) | Plan + challenger fold-in：把 challenger 3 P0 + 5 P1 折進 spec / plan、3 P2 → IDEAS。Spec §6.1 改 `emitTo("main-window", ...)` + `source: "hud"` 防 echo loop；§10.1 強制 raw Win32 `GetCursorPos` + extract `MonitorRect` + `pick_monitor` + `compute_centered_position` testable helpers；§10.2 改 `cfg!`-branched two-path `generate_handler!` 避開 macro 展開撞 toolchain。 |
| `771c6b2` | feat(m5) | Chunk 0 — `src-tauri/src/plugins/hud.rs` (283 行)：`HudError` thiserror enum 5 variants、`MonitorRect` plain struct、`pick_monitor` + `compute_centered_position` pure helpers、`get_cursor_position` raw Win32 `GetCursorPos`、`position_hud_for_active_monitor` 命令 orchestrate helpers + `hud.set_position(...)`；`set_hud_visible_for_dev` debug-only 命令 + `cfg(debug_assertions)` 整 fn 套住；6 unit tests（pick_monitor cursor-in / cursor-out / boundary / DPI 1.0/2.0/1.25 + compute_centered_position centering / DPI math / fallback）；`lib.rs` two-path `generate_handler!`；`tauri.conf.json` HUD window 380×56 logical、`center: false`、`x/y: 0`；`src/types/events.ts` 用 concrete `VoiceFlowStateChangedPayload` 取代 `unknown` placeholder（含 `source: "hud" | "dashboard"` 欄位防 echo）；`useTauriEvents.ts` 加 `VOICE_FLOW_STATE_CHANGED` constant。 |
| `4e01214` | feat(m5) | Chunk 1 — `useVoiceFlowStore.ts` async refactor：`init()` async + sequential `await listenToEvent(...)` 取代 `void promise.then(push)` race（無「event in flight 期間 register」漏訊息）；移除 `paste:focus-restore-failed` listener（state collision in IDEAS resolved；redundant 因 handleStop catch path 已 surface）；新 `dismissError()` method（`status === 'error'` 才動作）；新 `audio:recording-aborted` listener（mic_unplug / max_size 兩 reason）；`transitionTo` 改 async + `await emitTo("main-window", "voice-flow:state-changed", { status, message, source: "hud" })` best-effort try/catch；`handleStart` 包 `position_hud_for_active_monitor` 在 try/catch（P1-8 — positioning 失敗不擋 recording）；3 個新 vitest tests（dismissError 變 idle、dismissError noop、recording-aborted error path、formatError FocusRestoreFailed hint）；HUD entry `main.ts` 改 `store.init().then(cleanup => ...)` async pattern；total 9 store tests pass。 |
| `6af0992` | fix(m5) | Chunk 1 reviewer P0 — `formatError` pattern matches actual Rust Display：reviewer 抓出 `formatError` 用 `raw.startsWith("Focus restore failed")` 但 Rust `ClipboardError::FocusRestoreFailed` 的 Display 字串實際是「`SetForegroundWindow failed (error code <N>): hwnd 0x...`」、startsWith 永遠 false → 「請手動 Ctrl+V」 hint 永遠不會 append。改 pattern 對 `raw.includes("SetForegroundWindow")` + `raw.includes("hwnd")` 雙重 check、加 vitest 模擬實際 Rust message format。**這是 chunk 1 之後唯一的 P0**。 |
| `627d4cf` | feat(m5) | Chunk 2 — HUD components + ARIA + reduced-motion + i18n：`HudOverlay.vue` 全改寫（210 行）：4 個 visual states + `<Transition mode="out-in" name="fade">` 包 inner state-bubble switch + `watch(store.status, ...)` 切 `setIgnoreCursorEvents(true/false)` + click handler routes to `dismissError` only on error + `prefers-reduced-motion` reactive ref via `matchMedia('change')` listener + `role="status" aria-live="polite" :aria-label="ariaMessage"` + sub-icons (`CheckCircle2` / `XCircle`) `aria-hidden="true"` + ellipsis CSS（`text-overflow: ellipsis` + `max-width: 280px`）防長 error 醜；`HudWaveform.vue` (52 行)：6 bars + reduced-motion fallback to single dot；`HudSpinner.vue` (24 行)：CSS spin + reduced-motion fallback to「…」 text；`HudTimer.vue` (59 行)：mm:ss 格式 + setInterval 1s + 9 min amber-500 / 12 min red-500 顏色 + null `startedAtMs` regression handle (return 0:00 不報錯)；`useAudioWaveform.ts` 加 `starting` flag 防 rapid mount/unmount race（P1-3）；i18n 加 `hud.aria.recording / transcribing / success / error / idle` + `hud.transcribing` + `hud.success` + 移除 `hud.title` / `hud.pongCount` placeholder keys（M1 placeholder 全移）；4 個新 vitest test files（HudOverlay 5 / HudTimer 5 / HudWaveform 2 / waveform race 1 = 13 tests）；11 vite-shape screenshots（idle / recording-empty / recording-active / recording-warn-yellow / recording-warn-red / transcribing / success / error / error-after-click / reduced-motion-recording / reduced-motion-transcribing）+ Read 工具檢視確認；total chunk 2 後 vitest 28 + cargo 163。 |
| `7b91d8a` | fix(m5) | Chunk 2 reviewer P1/P2 polish — DEV gate + shim + reduced-motion gate：reviewer 找 0 P0、3 P1 / 2 P2。P1 #1（`window.__voiceFlowStoreSetStatus` dev hook 在 production bundle 暴露）→ 加 `if (import.meta.env.DEV)` gate、production tree-shake；P1 #2（`main-window.ts` shim 在 vite-only mode 暴露 `window.__TAURI_INTERNALS__` mock 給 production bundle）→ 同樣 DEV gate；P1 #3（`prefers-reduced-motion` initial check 在 SSR / pre-mount 時可能 throw）→ 加 `typeof window !== 'undefined'` 守護；P2 #1+#2 → IDEAS append (chunk 1 reviewer P1-3 docstring gap + chunk 2 reviewer P1-5 fontsource preload)。 |
| `fa135e8` | feat(m5) | Chunk 3 — HudFlowBadge sidebar + voice-flow:state-changed listener：`HudFlowBadge.vue` (65 行)：紅色脈動 dot + 「錄音中」（status === 'recording' 才 visible、其他 hidden）、`role="status" aria-live="polite"`、用 `listenToEvent` (NOT direct `listen`、CLAUDE.md import 規則)、payload `source !== 'hud'` filter 防 echo（無實際 echo、defense-in-depth）；mounted 進 `AppSidebar.vue` `<SidebarFooter>` slot；i18n `sidebar.recordingBadge`：「錄音中」（zh-TW）/「Recording」（en）；4 個新 vitest tests（mount → listenToEvent register、status='recording' visible、status='idle' hidden、source='hud' filter ignore）；2 vite-shape screenshots（dashboard-sidebar-badge / dashboard-sidebar-idle）+ Read 確認；total chunk 3 後 vitest 48 + cargo 163。 |
| `<this commit>` | docs(m5) | Chunk 4 — acceptance SOP + session log + IDEAS append + dashboard：`docs/m5-acceptance.md` 14 條 acceptance（4 visual states + cap warning + active-monitor + DPI + reduced-motion + ARIA + sidebar + click-through + dev visibility + focus-restore hint + listener removal）+ bonus（reduced-motion mid-recording 切換）；確認 13 vite-shape screenshots in `docs/screenshots/m5/`；`.claude/sessions/2026-05-05-m5-hud-overlay.md` 本檔；`.claude/IDEAS.md` chunk 2 + 3 reviewer P2 / P1 follow-ups append；`.claude/PROGRESS.md` M5 → ✅ implementation done + M6 next-session SOP；`doc/plans/02-implementation-roadmap.md` dashboard `M5 → ✅ Done` + 「最後更新」 bump；清理 repo root 15 個 `m5r-*.png` reviewer 留下的 leftover screenshots（`.playwright-mcp/` 已 gitignored、reviewer 的截圖 misplaced 到 repo root）。 |

### Files changed by chunk

#### Chunk 0（Rust + types + tauri.conf）

##### 新增

- `src-tauri/src/plugins/hud.rs`（283 行；`HudError` enum 5 variants、`MonitorRect` plain struct、`pick_monitor` + `compute_centered_position` pure helpers、`get_cursor_position` raw Win32、`position_hud_for_active_monitor` orchestrator command、`set_hud_visible_for_dev` debug-only command、6 unit tests）

##### 改寫

- `src-tauri/src/plugins/mod.rs` — 加 `pub mod hud;`
- `src-tauri/src/lib.rs` — register 2 new commands、two-path `cfg(debug_assertions)` `generate_handler!`、`HudState` no-op `.manage()`（command 直接從 `AppHandle` 拿 webview）
- `src-tauri/tauri.conf.json` — HUD window 380×56 logical、`center: false`、`x/y: 0`、其他屬性保留（transparent、alwaysOnTop、skipTaskbar 等）
- `src/types/events.ts` — replace `VoiceFlowStateChangedPayload = unknown` placeholder with concrete interface（含 `source: "hud" | "dashboard"`）
- `src/composables/useTauriEvents.ts` — add `VOICE_FLOW_STATE_CHANGED = "voice-flow:state-changed"` constant

#### Chunk 1（useVoiceFlowStore async refactor）

##### 改寫

- `src/stores/useVoiceFlowStore.ts` — `init()` async + sequential awaits、移除 `paste:focus-restore-failed` listener（comment 留說明 redundant + IDEAS collision concern resolved）、加 `dismissError()` method、加 `audio:recording-aborted` listener、`transitionTo` 改 async + `emitTo("main-window", ...)`、`handleStart` 包 `position_hud_for_active_monitor` try/catch、`formatError` pattern match `FocusRestoreFailed` 加「（請手動 Ctrl+V）」 hint
- `src/__tests__/useVoiceFlowStore.test.ts` — async tests + 3 new tests + `vi.hoisted` 加 `emitMock`
- `src/main.ts`（HUD entry）— `store.init().then(cleanup => ...)` 取代 sync init、stash cleanup 在 `window.__voiceFlowCleanup`

#### Chunk 1 reviewer P0 fix

##### 改寫

- `src/stores/useVoiceFlowStore.ts` — `formatError` pattern 改 match actual Rust `ClipboardError::FocusRestoreFailed` Display string（include `SetForegroundWindow` + `hwnd`）
- `src/__tests__/useVoiceFlowStore.test.ts` — 新 vitest 模擬實際 Rust message format

#### Chunk 2（HUD components + ARIA + reduced-motion + i18n）

##### 新增

- `src/components/HudWaveform.vue`（52 行）
- `src/components/HudSpinner.vue`（24 行）
- `src/components/HudTimer.vue`（59 行）
- `src/__tests__/HudOverlay.test.ts`（221 行、5 tests）
- `src/__tests__/HudTimer.test.ts`（112 行、5 tests）
- `src/__tests__/HudWaveform.test.ts`（75 行、2 tests）
- 11 個 vite-shape screenshots in `docs/screenshots/m5/`

##### 改寫

- `src/components/HudOverlay.vue` — 全改寫（210 行）取代 M1 pong-counter placeholder
- `src/composables/useAudioWaveform.ts` — 加 `starting` flag（P1-3）
- `src/i18n/locales/zh-TW.json` + `en.json` — 加 `hud.aria.*` + `hud.transcribing` + `hud.success`、移除 `hud.title` / `hud.pongCount`

#### Chunk 2 P1/P2 polish

##### 改寫

- `src/main.ts`（HUD entry）— `import.meta.env.DEV` gate dev hook
- `src/main-window.ts`（Dashboard entry）— DEV gate Tauri shim
- `src/components/HudOverlay.vue` — `prefers-reduced-motion` `typeof window !== 'undefined'` 守護

#### Chunk 3（Dashboard sidebar）

##### 新增

- `src/components/HudFlowBadge.vue`（65 行）
- `src/__tests__/HudFlowBadge.test.ts`（151 行、4 tests）
- 2 vite-shape screenshots: `m5-dashboard-sidebar-badge.png` + `m5-dashboard-sidebar-idle.png`

##### 改寫

- `src/components/AppSidebar.vue` — mount `<HudFlowBadge />` 進 `<SidebarFooter>` slot
- `src/i18n/locales/zh-TW.json` + `en.json` — 加 `sidebar.recordingBadge`

#### Chunk 4（本檔）

##### 新增

- `docs/m5-acceptance.md`
- `.claude/sessions/2026-05-05-m5-hud-overlay.md`（本檔）

##### 改寫

- `.claude/PROGRESS.md` — M5 status / 下個 SOP / session table
- `.claude/IDEAS.md` — append chunk 2/3 reviewer P2 + P1 follow-ups
- `doc/plans/02-implementation-roadmap.md` — dashboard `M5 → ✅ Done`、bump 「最後更新」

##### 刪除（cleanup）

- 15 個 `m5r-*.png` files at repo root — chunk 2/3 reviewer Playwright session leftovers、`.playwright-mcp/` 已 gitignored 但這次 reviewer 寫到 repo root。delete (not commit) 後 working tree clean

## Key decisions

- **Plan-time challenger 與拆計畫同 message 平行 dispatch**（CLAUDE.md item #5 強制）：M5 開工前 challenger（agentId `abe35088f7a648bf9`）找出 3 P0 + 8 P1 + 3 P2 設計 / 邊界條件問題：P0-1 emit echo loop 風險（HUD 自己 listen 會無限）、P0-2 假 Tauri 2 API `Manager::cursor_position()` 不存在、P0-3 移除 `paste:focus-restore-failed` listener 同時失友善 hint。主 session 對照 spec/plan 修：**修計畫不修代碼便宜**。Spec §6.1 改 `emitTo("main-window", ...)` + `source: "hud"` 防 echo；§10.1 改 raw Win32 `GetCursorPos` 直接路徑、強制 extract `MonitorRect` + `pick_monitor` + `compute_centered_position` 為 testable helpers；§7.2 + §10.2 改 `formatError` pattern-match `FocusRestoreFailed` 在 store 端拼 hint
- **`#[cfg(debug_assertions)]` two-path `generate_handler!`**（plan-time challenger P1-6）：原計畫 inline `#[cfg]` on macro args 在某些 toolchain 撞展開順序、release build 編譯失敗。改成兩個 separate `let invoke_handler = ...` block 各自 cfg-branched、然後 `.invoke_handler(invoke_handler)` 共用。Reviewer 驗 release / debug build 各跑、確認 `set_hud_visible_for_dev` symbol 在 release 完全 strip
- **Active-monitor 用 cursor、不用 active window**：spec §3.1 challenger 點明 — user 拖視窗到 secondary monitor 但 cursor 還在 primary 的 corner case、active window 派 monitor 會把 HUD 放錯邊。改用 `GetCursorPos` 取 cursor 物理位置、找 cursor 所在 monitor rect 即「user 當下注意力所在」
- **HUD positioning 失敗 fallback to last position**（plan-time challenger P1-8）：`handleStart` 包 `position_hud_for_active_monitor` 在 try/catch、失敗 console.warn 後繼續走 `capture_target_window` + `start_recording`。**裝飾性失敗不擋核心錄音流程**
- **`emitTo("main-window", ...)` + `source: "hud"`**（plan-time challenger P0-1）：原計畫 `emit(...)` 廣播到所有 webview（含 HUD 自己）、未來若 HUD 也 listen 自己的事件會 echo loop 撞 store。改 `emitTo` 顯式 target Dashboard、`source: "hud"` 欄位讓 listener filter `source !== 'hud'` defense-in-depth
- **移除 `paste:focus-restore-failed` listener + `formatError` pattern match**（plan-time challenger P0-3）：M4 chunk 3 reviewer 點過此 listener 與 paste catch path 重複（state collision in IDEAS）。M5 移除 listener、改 `formatError` 在 `handleError` catch 路徑 pattern-match Rust `ClipboardError::FocusRestoreFailed` Display 字串、加「（請手動 Ctrl+V）」hint。**chunk 1 reviewer P0 後續發現** Rust 實際 Display 是「`SetForegroundWindow failed (error code N): hwnd 0x...`」、不是「`Focus restore failed`」、startsWith pattern 永遠 false。改成 `raw.includes("SetForegroundWindow") && raw.includes("hwnd")` 雙重 check
- **Pure helpers 強制 cargo-testable**（plan-time challenger P0-2）：原 spec 描述用 Tauri abstraction 之上的 `cursor_position()` 不存在、implementer 會 fall through to raw Win32 但 caller code 跟 helper 沒拆。Refined plan 強制 extract `MonitorRect` plain struct + `pick_monitor` + `compute_centered_position` 為 pure functions、`position_hud_for_active_monitor` 只 orchestrate。6 unit tests 涵蓋 cursor-in / cursor-out / boundary / DPI 1.0/2.0/1.25 / centering / fallback
- **`useAudioWaveform.start()` `starting` flag**（plan-time challenger P1-3）：rapid hotkey press（recording → idle → recording 在 ~100ms 內）→ HudWaveform 重複 mount/unmount、`useAudioWaveform.start()` 的 `await listenToEvent(...)` 在 flight 期間第二次 mount 進來、第一個 unlisten ref 還是 undefined、第二個 register 完蓋過第一個、第一個 cleanup 漏。加 `starting: boolean` flag 在 promise resolve 前 short-circuit re-entry
- **`prefers-reduced-motion` matchMedia 動態切換**：HudOverlay `onMounted` 設 reactive ref + `matchMedia('change')` listener、user 在錄音中改 OS Animation effects 也 reactive。包 `typeof window !== 'undefined'` 守護避免 SSR / pre-mount throw（chunk 2 reviewer P1-3）
- **`v-if` `useVoiceFlowStore.status !== 'idle'`** 而非 `setVisible(false)` HUD window：spec §4.3 — Vue v-if 控制 bubble、Tauri window 一直 mounted（避免 IPC 來回 hide/show 開銷）。Dev 用 `set_hud_visible_for_dev(true)` bypass v-if 強制 visible 看 idle empty bubble
- **`<Transition mode="out-in" name="fade">`**：Vue 3.5 transition 包 inner state-bubble、out-in 模式確保 prev state fade out 完才 fade in next。jsdom + vitest transition timing 不可靠（plan-time challenger P1-2）、test 只測 state→class mapping after `nextTick()`、transition timing 留 Playwright + manual 驗
- **Reviewer 必跑 Playwright pass、不能 trust implementer screenshots**（CLAUDE.md `feedback_ui_screenshots.md`）：chunk 2/3 reviewer 都各自 `pnpm dev` + `mcp__plugin_playwright_playwright__browser_*` 跑 own screenshot pass、Read 工具確認、跟 implementer 截的對比。Reviewer 截圖跑 13+ 張、放 repo root（fixed in chunk 4 cleanup）
- **shadcn-vue Google Fonts trap 沒踩到** （難得！）：M5 沒加新 shadcn-vue components（只用 lucide-vue-next CheckCircle2 + XCircle）、CLAUDE.md「常見踩雷」這次 100% 沒重現。但 reviewer 仍 grep 確認 `src/assets/index.css` 沒 `fonts.googleapis.com` regression

## Surprises / 踩雷

- **`formatError` pattern 與實際 Rust Display 不對齊**（chunk 1 reviewer P0、commit `6af0992`）：`formatError` 用 `raw.startsWith("Focus restore failed")` 但 Rust `ClipboardError::FocusRestoreFailed` `#[error(...)]` 寫的是「`SetForegroundWindow failed (error code {}): hwnd 0x{:x}`」、startsWith 永遠 false → 「請手動 Ctrl+V」hint 永遠不會 append、user 看純技術 message。**lesson**：implementer 寫 frontend pattern match Rust error display 時、要回 Rust 確認 actual `#[error(...)]` 字串。Test mock 也要用 actual 格式
- **Chunk 2/3 reviewer Playwright 截圖 misplaced 到 repo root**：`.playwright-mcp/` 已 gitignored、但這次 reviewer subagent 跑 `browser_take_screenshot` 沒指定路徑、Playwright 預設存 cwd（reviewer 的 cwd 是 worktree root）、15 個 `m5r-*.png` 出現在 git status untracked 列表。Chunk 4 cleanup 一次 `rm m5r-*.png`、working tree clean。**lesson**：未來 reviewer prompt 加「screenshots 必須存進 `.playwright-mcp/<reviewer>/...` 或 `/tmp/`」
- **`SidebarFooter` empty wrapper artifact**（chunk 3 reviewer P2-1）：idle 狀態下 `<HudFlowBadge>` `v-if="status === 'recording'"` false、整個 component 不 render；但 `<SidebarFooter>` parent wrapper 仍 render 一個 padding band（shadcn-vue Sidebar 元件原本就有 footer slot 預留 padding）。視覺上 sidebar 底部多出空白條。Hoisting `v-if` 進 `AppSidebar.vue` parent 即可解。**M9 polish 順手做**（IDEAS）
- **Tauri 2 stable `Manager::cursor_position()` API gap**（plan-time challenger P0-2）：spec 原本假設 Tauri 提供 cross-platform cursor query API、challenger 翻 Tauri 2 docs 確認 stable 沒 expose、implementer 會 fall through to raw Win32 但 helper structure 沒 extract。修計畫前抓到、避免 implementer 撞牆
- **`#[cfg(debug_assertions)]` 在 `generate_handler!` macro 展開順序問題**（plan-time challenger P1-6）：原計畫想 inline `#[cfg(debug_assertions)] plugins::hud::set_hud_visible_for_dev` 進 macro args、但 macro 展開時 cfg attr 在 some toolchain（特別是舊 nightly）會被當成 token 而非條件編譯、release build 會 fail。Refined 改 two-path block。**lesson**：`#[cfg]` on macro args 是 fragile、用 separate cfg-branched bindings 較穩
- **`shadcn-vue` Google Fonts trap 這次沒踩**：難得 — M5 沒加新 shadcn-vue components（只 lucide-vue-next icons），CLAUDE.md「常見踩雷」 100% 不重現。但 reviewer 仍 grep `fonts.googleapis.com` 在 `src/assets/index.css` 確認沒 regression（防止 `useAudioWaveform` 之類順手 add 元件意外觸發）
- **chunk 2 reviewer 的 P1 #1+#2 dev hook + shim production bundle exposure**：`window.__voiceFlowStoreSetStatus` 是 chunk 2 implementer 為 Playwright vite-only mode 加的 dev mutation hook（讓 reviewer 用 `browser_evaluate` 切 status 截圖各 state）、`main-window.ts` shim 是 vite-only mode 的 Tauri runtime mock。implementer 沒加 `import.meta.env.DEV` gate、reviewer 抓出後 production bundle tree-shake 才正確
- **`prefers-reduced-motion` initial check 在 SSR/pre-mount 可能 throw**（chunk 2 reviewer P1-3）：`window.matchMedia(...)` 在 SSR / 第一次 mount 前 `window` undefined 會 throw。即使 Tauri webview 不 SSR、vitest jsdom 環境某些情況也會、加 `typeof window !== 'undefined'` 守護穩
- **HudFlowBadge `listenToEvent` 與 `emitTo` 雙視窗 timing**：implementer 一度擔心 `voice-flow:state-changed` 是 `emitTo("main-window", ...)`、但 Dashboard `HudFlowBadge` 在 sidebar 用 `listenToEvent`、是不是 webview 還沒 ready 時 emit 會丟？實測 Tauri 2 `emitTo` 對 inactive window 會 buffer、active 時 deliver。OK
- **Vitest mock `vi.hoisted` 對 `emitMock`**：chunk 1 implementer 一開始用 top-level `const emitMock = vi.fn()` + `vi.mock` factory、撞 hoisting trap（M4 chunk 4 已知）。改 `vi.hoisted(() => ({ emitMock: vi.fn() }))` 解。**lesson 已 documented in M4 session log**

## Acceptance criteria（M5 14 conditions、待 user 跑）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| 1. recording state — 6-bar waveform + timer 0:01→0:02 | ✅ Vitest HudOverlay state machine + HudWaveform + HudTimer | ✅ user 按住熱鍵說話、看 HUD |
| 2. transcribing state — spinner + 「轉錄中…」 | ✅ Vitest HudSpinner + HudOverlay state | ✅ user 放開熱鍵後看 HUD |
| 3. success state — ✓ + 「完成」、1s autohide | ✅ Vitest auto-hide timer | ✅ user transcribe 成功後看 HUD |
| 4. error state — ✗ + message、6s linger / click dismiss | ✅ Vitest auto-hide + dismissError | ✅ user 拔 API key 觸發 + click HUD |
| 5. cap warning — 9 min 黃 / 12 min 紅 / ~13 min cap abort | ✅ Vitest HudTimer color computed + recording-aborted listener | ✅ user 連錄 ≥ 13 min |
| 6. **active-monitor positioning** — secondary monitor 工作 HUD 在那 | ✅ Cargo unit tests pick_monitor cursor-in/out/boundary | ✅ user 在 secondary monitor 觸發、看 HUD 位置 |
| 7. **DPI 100% / 150% / 200% + mixed-DPI** — HUD 不糊 | ❌ 純手動（vite-only 無法模擬真實 DPI） | ✅ user 在 Display Settings 切換 DPI |
| 8. **`prefers-reduced-motion: reduce` ON** — dot / 「…」 / no scale fade | ✅ Vitest HudWaveform reduced-motion fallback | ✅ user OS Settings 開「動畫減少」 |
| 9. **ARIA — screen reader 念出 state 變化** | ❌ 純手動 | ✅ user 開 NVDA / Narrator 驗 |
| 10. Dashboard sidebar — recording 紅點 + 「錄音中」、其他 hidden | ✅ Vitest HudFlowBadge | ✅ user dogfood 看 sidebar |
| 11. **click-through** — recording / transcribing / success 穿透、error 不穿 | ❌ 純手動 | ✅ user 在 HUD 範圍試 click Notepad |
| 12. **dev visibility** — `set_hud_visible_for_dev(true)` 強制 idle 顯示 | ✅ Cargo unit test | ✅ user 在 devtools console invoke |
| 13. `paste:focus-restore-failed` 後 — 「請手動 Ctrl+V」 hint | ✅ Vitest formatError pattern | ✅ user 試 Task Manager 觸發 paste 失敗 |
| 14. listener 移除 — burst press 期間舊 paste 失敗 event 不影響新 flow | ✅ Vitest（無對應 listener 註冊） | ✅ user 試 burst press during transcribing |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → **53 pass**（M4 baseline 16 + chunk 1 useVoiceFlowStore +9 + chunk 2 HudOverlay/Timer/Waveform +21 + chunk 3 HudFlowBadge +5 + chunk 1 reviewer P0 fix tests +2 = 53）
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test --lib` → **163 passed**（M4 baseline 157 + chunk 0 hud +6 = 163）

### Vite-shape screenshots（13 張、`docs/screenshots/m5/`）

| Filename | State | 驗證 |
|---|---|---|
| `m5-hud-idle.png` | idle | bubble 不 render、HUD window 透明 |
| `m5-hud-recording-empty.png` | recording 剛起 | 6 bars 起點 + timer 0:00 |
| `m5-hud-recording-active.png` | recording active | 6 bars 高低不一 + timer mm:ss |
| `m5-hud-recording-warn-yellow.png` | recording ≥ 9 min | timer amber-500 |
| `m5-hud-recording-warn-red.png` | recording ≥ 12 min | timer red-500 |
| `m5-hud-transcribing.png` | transcribing | spinner + 「轉錄中…」 |
| `m5-hud-success.png` | success | ✓ + 「完成」 |
| `m5-hud-error.png` | error pre-click | ✗ + message |
| `m5-hud-error-after-click.png` | error click dismissed | bubble fade out → idle |
| `m5-hud-reduced-motion-recording.png` | reduced-motion + recording | dot + label |
| `m5-hud-reduced-motion-transcribing.png` | reduced-motion + transcribing | 「…」 |
| `m5-dashboard-sidebar-badge.png` | Dashboard recording | sidebar 紅點 + 「錄音中」 |
| `m5-dashboard-sidebar-idle.png` | Dashboard idle | badge 不 render |

### Tauri runtime smoke

- `cargo check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --lib` → 編譯 + 28 commands 整合 + 163 tests pass
- `pnpm tauri dev` 完整 launch 沒做（main session 無 GUI session、no real keyboard input、no multi-monitor、no real DPI scale、no NVDA / Narrator）— user 自己跑時驗 14 conditions

## Manual verification SOP（user 跑 `pnpm tauri dev` 後）

詳見 [`docs/m5-acceptance.md`](../../docs/m5-acceptance.md) — 14 acceptance conditions + 1 bonus、每條含「設定 / 步驟 / 預期」+ 截圖參考。任何 P0 fail 回 main session 修。

## Follow-ups for M6

- **`enhancing` voice flow status**：M5 已預留 `useVoiceFlowStore.status` placeholder（spec §16）、M6 加 enhancing state（polish 中、HUD 顯示新 visual state）
- **HUD §2 state mapping 加 `enhancing` 行**：spinner + 「優化中…」 label、reuse HudSpinner component
- **Dashboard sidebar badge 拓展 enhancing**（chunk 3 reviewer P2-4）：當前 HudFlowBadge 只讀 `payload.status` 的 'recording'、若 M6 加 tooltip 顯示 polish provider / model、要回 listener 加 `payload.message`
- **`role="status"` 重複 announce 風險**（chunk 3 reviewer P2-3）：HUD + Dashboard 都 `role="status"`、SR 念兩次。M6 加 enhancing 時更明顯、Phase 2 a11y test
- **i18n key duplication**（chunk 3 reviewer P2-2）：`sidebar.recordingBadge` 與 `dashboard.audioTest.recording` 都「錄音中」、M9 i18n consolidation
- **`SidebarFooter` empty wrapper artifact**（chunk 3 reviewer P2-1）：M9 polish 把 `v-if` hoist 進 parent
- **Vitest `<Transition>` timing 不可靠**（plan-time challenger P1-2）：M5 vitest 只測 state→class、transition timing 留 Playwright；M6 implementer 也沿襲

## Subagent dispatch pattern（M5 證實）

承襲 M0-M4 模式、規模適中、**M5 加新工法**：

1. **Plan-time challenger 與拆計畫同 message 平行 dispatch**（CLAUDE.md item #5 強制）：M4 已用、M5 證實有效。Challenger 找 3 P0 + 8 P1 + 3 P2、主 session 對照 spec/plan 修：3 P0 全進 chunks、5 P1 進 chunks、3 P1 + 3 P2 → IDEAS。**修計畫比修代碼便宜**得到再次驗證
2. **5 個 implementation chunks**：
   - Chunk 0（Rust + types + tauri.conf）→ sequential、後 chunks 依賴 commands + types
   - Chunk 1（store async refactor）→ 依賴 chunk 0 events constant + types
   - Chunk 2（HUD components + i18n）→ 依賴 chunk 1 store API（dismissError + audio:recording-aborted）
   - Chunk 3（Dashboard sidebar）→ 依賴 chunk 1 cross-window emit
   - Chunk 4（docs/dashboard）→ 依賴 chunks 0-3 全部
3. **每個 chunk 後 reviewer subagent**（chunks 0/2/3 各 1、chunk 1 必修 P0）、reviewer 必跑 Playwright pass（CLAUDE.md UI verification SOP）
4. **Main session orchestration only**：8 commits 親手 commit、commit message 親寫、整合 lib.rs / 移 screenshots / 寫 session log / update PROGRESS / IDEAS append；不寫實作 code

對 M6（LLM polish 多 provider）建議：
- Plan-time challenger 必跑、focus Privacy / API key 不變式 / Typeless 隱私翻車反例 / preset prompt injection 風險
- 4 provider Rust client：先寫 unified `enum LlmProviderId` + `parse_provider_response` dispatcher、再各自 build_request；testable pattern reuse M3
- Per-step data-flow indicator UI 在 Settings：避免 marketing 失調

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M5 implementation done、待 user acceptance、M6 LLM polish next）
2. 本檔（M5 session log）— 特別是「Follow-ups for M6」段
3. `.claude/IDEAS.md` 「## M5 chunk reviewer findings」+「## M5 plan-time challenger findings」section（共 5+ 項目給 M6 / M9 / Phase 2 拾起）
4. `doc/plans/02-implementation-roadmap.md` M6 section（LLM polish 4 provider、preset modes、Rust-side fetch 守 API key 不變式）
5. `doc/plans/03-rust-modules.md` `## M6 LLM polish module 規劃` section
6. `doc/reference/sayit-improvements.md` 「LLM polish 隱私翻車」教訓 + Typeless 失調反例

## User acceptance verdict（待 user 跑、append 後 mark M5 真正 Done）

User 跑 `docs/m5-acceptance.md` 14 條後、append 結果到本檔尾部「## User acceptance addendum」section（同 M4 模式、M4 acceptance addendum 抓出 2 P0 後修了 2 commit）。
