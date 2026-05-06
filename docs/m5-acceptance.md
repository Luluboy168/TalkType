# M5 HUD Overlay — 手動驗收 SOP

> **狀態**：Active v1
> **最後更新**：2026-05-05
> **適用 milestone**：M5（HUD overlay 4 visual states + 6-bar waveform + ARIA + reduced-motion + active-monitor positioning + Dashboard sidebar badge）
> **Spec**：[`docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md`](superpowers/specs/2026-05-05-m5-hud-overlay-design.md)（commit `44b025f`）
> **Plan**：[`docs/superpowers/plans/2026-05-05-m5-hud-overlay-plan.md`](superpowers/plans/2026-05-05-m5-hud-overlay-plan.md)（commit `8c62865`）

## TL;DR

跑 `pnpm tauri dev` 起 dev build、依序通過下列 14 個情境（含 4 個 visual states + 6 challenger 補強條件 + 4 個 a11y / 多螢幕 / DPI / focus-restore 條件）+ 1 個 bonus（reduced-motion mid-recording 切換）。任何一條失敗 → 在 commit 之前修掉。

## 自動驗證摘要（all green required）

- `cargo test --lib` → **163 cargo tests pass**（M4 baseline 157 + chunk 0 hud +6 = 163）
- `vitest run` → **53 vitest pass**（M4 baseline 16 + chunk 1 useVoiceFlowStore +9 + chunk 2 HudOverlay/Timer/Waveform +21 + chunk 3 HudFlowBadge +5 + chunk 1 reviewer P0 fix tests +2 = 53）
- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean

## 前置準備

```bash
cd E:/SynologyDrive/Projects/TalkType
pnpm install --frozen-lockfile
pnpm tauri dev
```

預設熱鍵：`Right Alt` + `Hold` mode（M4 已驗收）；HUD 在 `voice flow status === 'idle'` 時隱藏 bubble、其他狀態 visible。

---

## 14 條接受標準

### 1. Recording state — 6-bar waveform 動 + timer 計時

**Setup**：開 Notepad（記事本）、focus 文件區。
**Steps**：
1. 按住 `Right Alt` 開始錄音、不放手。
2. 對麥克風持續說話 2-3 秒。
3. 觀察 HUD bubble。

**Expected**：
- HUD bubble 出現在 active monitor 螢幕水平中央偏上（y 約 50px）。
- 6 條 waveform bar 隨 mic level 上下跳動（M2 emit `audio:waveform` 60fps）。
- Timer 顯示 `0:01` → `0:02` → `0:03`。

**截圖參考**：[`m5-hud-recording-active.png`](screenshots/m5/m5-hud-recording-active.png)、[`m5-hud-recording-empty.png`](screenshots/m5/m5-hud-recording-empty.png)（剛起錄、bar 都在起點）。

---

### 2. Transcribing state — spinner + 「轉錄中…」label

**Setup**：保持條件 1。
**Steps**：
1. 放開 `Right Alt`。
2. 觀察 HUD bubble（≤ 2 秒視窗）。

**Expected**：
- 6-bar waveform 消失、改顯示 spinner 圓圈動畫。
- Label 文字變成「轉錄中…」（zh-TW）/「Transcribing…」（en）。
- HUD 仍 visible、不消失。

**截圖參考**：[`m5-hud-transcribing.png`](screenshots/m5/m5-hud-transcribing.png)。

---

### 3. Success state — ✓ + 「完成」、1s autohide

**Setup**：保持條件 2。
**Steps**：
1. 等 Groq 回 transcription（約 1-2 秒）。
2. 觀察 HUD。

**Expected**：
- spinner 消失、出現綠色 ✓（lucide-vue-next `CheckCircle2`）icon。
- Label 「完成」（zh-TW）/「Done」（en）。
- 1 秒後 HUD bubble fade out 進 idle、bubble 不 render。
- 文字已 paste 進 Notepad（M4 行為）。

**截圖參考**：[`m5-hud-success.png`](screenshots/m5/m5-hud-success.png)。

---

### 4. Error state — ✗ + message、6s linger / click dismiss

**Setup**：先在 Settings 把 Groq API key 拿掉（或改錯）→ 切回 Notepad。
**Steps**：
1. 按住 `Right Alt` → 說話 → 放開（會走到 transcribe error）。
2. 觀察 HUD。
3. 在 6 秒內、把滑鼠移到 HUD 上、左鍵 click HUD bubble。

**Expected**：
- Spinner / waveform 消失、出現紅色 ✗（lucide-vue-next `XCircle`）icon。
- Label 顯示具體錯誤（例：「API key 無效」）。
- HUD 在 error 狀態下 click-through OFF（`setIgnoreCursorEvents(false)`）→ 滑鼠可點到 HUD。
- Click 後 → HUD 立即進 idle、bubble fade out。
- 若不 click → 6 秒後自動回 idle。

**截圖參考**：[`m5-hud-error.png`](screenshots/m5/m5-hud-error.png)（pre-click）、[`m5-hud-error-after-click.png`](screenshots/m5/m5-hud-error-after-click.png)（post-click 已回 idle）。

---

### 5. Cap warning timer color — yellow ≥ 9 min、red ≥ 12 min、~13 min cap abort

**Setup**：把 trigger mode 切到 `Toggle`（Settings → Hotkey）、focus Notepad。
**Steps**：
1. 按一下 `Right Alt` 開始錄音、保持安靜（避免 noise gate）。
2. 等到 timer 顯示 `9:00`、觀察 timer 顏色。
3. 等到 timer 顯示 `12:00`、觀察 timer 顏色。
4. 持續錄音直到 ~13 分鐘（`MAX_WAV_BYTES = 25_000_000` cap、@ 16 kHz mono i16 ≈ 13 分鐘）。

**Expected**：
- < 9:00：timer 文字 muted（zinc-400）。
- ≥ 9:00：timer 文字變黃色（amber-500）。
- ≥ 12:00：timer 文字變紅色（red-500）。
- 達 cap：M2 自動 emit `audio:recording-aborted { reason: 'max_size' }`、useVoiceFlowStore listener 把 status 切到 `error`、HUD 顯示「錄音超過上限（~13 分鐘 @ 16 kHz）」。

**截圖參考**：[`m5-hud-recording-warn-yellow.png`](screenshots/m5/m5-hud-recording-warn-yellow.png)、[`m5-hud-recording-warn-red.png`](screenshots/m5/m5-hud-recording-warn-red.png)。

---

### 6. Active-monitor positioning — secondary monitor 工作

**Setup**：
1. 接第二顯示器（任何配置；可以是 1080p 副 + 4K 主、或同 DPI 兩台）。
2. Windows 設定 → Display → 確認 multi-monitor 開啟。
3. 把滑鼠 cursor 移到 secondary monitor、focus secondary monitor 上的 Notepad。

**Steps**：
1. 按住 `Right Alt` 說話、放開。
2. 觀察 HUD 出現在哪個 monitor。

**Expected**：
- HUD 出現在 **secondary monitor**（不是 primary）水平中央偏上。
- 文字 paste 進 secondary monitor 的 Notepad。
- 切回 primary monitor focus 後再試一次、HUD 應該在 primary。

**注意**：M5 用 `GetCursorPos` 判斷 active monitor，不是用 active window 的 monitor、避免 user 拖視窗到副螢幕但 cursor 在主螢幕的 corner case。

---

### 7. DPI handling — 100% / 150% / 200% + mixed-DPI

**Setup**：
1. Windows 設定 → System → Display → Scale and layout、把 primary monitor scale 改 100%（如果有多 monitor、可在 mixed-DPI 中試）。
2. focus Notepad。

**Steps**（每個 DPI 都跑一次）：
1. 100% / 150% / 200% 都試一次熱鍵 → 說話 → 放開的完整流程。
2. **多螢幕 mixed-DPI**：如果有 4K 主 (200%) + 1080p 副 (100%)、各自 cursor 在那 monitor 時觸發。

**Expected**：
- HUD 在每個 DPI scale 下都銳利、不糊（Tauri 自動 logical→physical 轉換）。
- HUD 邊緣不被 cropping、bar / icon / label 都正確尺寸。
- mixed-DPI：cursor 在 4K 200% monitor → HUD 被 OS 自動 scale 到 200%、不糊；切到 1080p 100% monitor → HUD 100%、不過大。

**注意**：純手動測試、無 automated screenshot 涵蓋（vite-only mode 無法模擬真實 DPI scale）。

---

### 8. `prefers-reduced-motion: reduce` ON — 動畫關閉

**Setup**：
1. Windows 設定 → Accessibility → Visual effects → Animation effects = OFF（也可在 Edge devtools `Rendering` panel emulate `reduced-motion: reduce`）。
2. focus Notepad。

**Steps**：
1. 按住 `Right Alt` 說話、放開。
2. 觀察 HUD 動畫。

**Expected**：
- recording state：6-bar waveform 改 **單一靜止 dot + label**（無 60fps lerp 動畫）。
- transcribing state：spinner 改 **「…」靜態文字**（無旋轉動畫）。
- 各 state 進場 / 出場：fade only（200ms）、無 scale 0.95→1 動畫。
- 切回 reduced-motion OFF 後恢復 6-bar / spinner / scale 動畫。

**截圖參考**：[`m5-hud-reduced-motion-recording.png`](screenshots/m5/m5-hud-reduced-motion-recording.png)、[`m5-hud-reduced-motion-transcribing.png`](screenshots/m5/m5-hud-reduced-motion-transcribing.png)。

---

### 9. ARIA — screen reader 念出 state 變化

**Setup**：
1. 安裝並開啟 Windows Narrator（`Ctrl+Win+Enter`）或 NVDA。
2. focus Notepad。

**Steps**：
1. 按住 `Right Alt` 說話、放開。
2. 聆聽 screen reader 念出哪些訊息。

**Expected**：
- recording state 進場：SR 念出「Recording」（en）/ 「錄音中」（zh-TW）。
- transcribing state：SR 念出「Transcribing」/「轉錄中」。
- success state：SR 念出「Done」/「完成」。
- error state：SR 念出「Error」+ message text。
- HUD root 有 `role="status"` + `aria-live="polite"` + `aria-label`、icons (`CheckCircle2` / `XCircle`) 都 `aria-hidden="true"` 避免冗讀。

**注意**：rapid hotkey press 重複同訊息 SR 可能略過（Phase 2 a11y polish 處理 — IDEAS.md 已記）。

---

### 10. Dashboard sidebar — 紅點 + 「錄音中」on recording

**Setup**：
1. 開啟 Dashboard（tray icon click 或 `pnpm tauri dev` 自動開）。
2. 視窗右下半 Dashboard sidebar 可見。
3. focus 切到任何其他 app（讓 Dashboard 不擋 HUD 截圖、但 sidebar 還能看到）。

**Steps**：
1. 按住 `Right Alt` 說話。
2. 看 Dashboard sidebar 底部 footer 區段。
3. 放開 `Right Alt`、繼續觀察。

**Expected**：
- recording 期間：sidebar footer 顯示 **紅色脈動 dot + 「錄音中」**（zh-TW）/ **「Recording」**（en）。
- transcribing / success / error / idle 期間：badge **隱藏**（M5 simplified、Phase 2 加 transcribing / error tooltip — IDEAS）。
- Cross-window event `voice-flow:state-changed` 從 HUD `emitTo("main-window", ...)` 帶 `source: "hud"`、Dashboard listener filter `source !== "hud"` 防 echo loop（無實際 echo、但 defense-in-depth）。

**截圖參考**：[`m5-dashboard-sidebar-badge.png`](screenshots/m5/m5-dashboard-sidebar-badge.png)（recording）、[`m5-dashboard-sidebar-idle.png`](screenshots/m5/m5-dashboard-sidebar-idle.png)（idle 隱藏）。

---

### 11. Click-through — recording / transcribing / success 滑鼠穿透、error 不穿透

**Setup**：focus Notepad、把 Notepad 視窗大小放大、確保 HUD bubble 範圍內仍有 Notepad 可點區。
**Steps**：
1. 按住 `Right Alt` 說話、不放手。
2. 在 HUD bubble visible 區域內 left-click 一次。
3. 重複 transcribing / success state 各 click 一次（用 toggle mode 或快速時間視窗）。
4. 觸發 error（拔 API key），HUD error state 出現後在 bubble 上 click。

**Expected**：
- recording / transcribing / success state：click 穿透 HUD、Notepad 收到 click（caret 移到 click 位置、選取被 clear）。
- error state：click 被 HUD 攔下、HUD 立即 dismiss 進 idle、Notepad **沒有**收到 click。
- HUD `setIgnoreCursorEvents(true)` 預設、watcher 在 status 變 `error` 時切 `false`、變 `recording`/`transcribing`/`success`/`idle` 時切回 `true`。

---

### 12. Dev visibility — `set_hud_visible_for_dev(true)` 強制顯示

**Setup**：
1. `pnpm tauri dev` debug build。
2. Dashboard webview 開 devtools（F12）。
3. 在 console 跑：

```javascript
await window.__TAURI_INTERNALS__.invoke('set_hud_visible_for_dev', { visible: true });
```

**Steps**：
1. 上面命令執行後、看 HUD window。

**Expected**：
- HUD window 強制 `setVisible(true)`、bypass `idle` 的 `v-if` 隱藏邏輯。
- bubble 內容是 idle state（無 waveform / spinner / icon、只有空 transparent bubble）。
- release build (`pnpm tauri build` 後跑)：command 不存在、symbol 完全 cfg-strip（`#[cfg(debug_assertions)]`）。

**注意**：command 只在 debug build 編進 `generate_handler!`、release build 執行 `invoke('set_hud_visible_for_dev')` 會回 command not found error（intended behavior）。

---

### 13. `paste:focus-restore-failed` → 「請手動 Ctrl+V」hint

**Setup**：以管理員身份開 Task Manager（`Ctrl+Shift+Esc` 或右鍵 → 系統管理員執行）。
**Steps**：
1. focus Task Manager 的 Search 過濾欄位。
2. 按住 `Right Alt` 說話、放開。
3. 觀察 HUD error state。

**Expected**：
- transcribe 成功、但 paste 走 `SetForegroundWindow` 因 UIPI 被 Win11 拒絕（M4 challenger P0#2 已 documented）。
- HUD error state message 結尾包含「**（請手動 Ctrl+V）**」hint（M5 chunk 1 P0-3 fix：`formatError` pattern-match `FocusRestoreFailed` Rust enum Display）。
- 6 秒 linger 或 click dismiss 後進 idle。
- 文字仍在 clipboard、user 手動 `Ctrl+V` 可貼進 Task Manager。

---

### 14. `paste:focus-restore-failed` listener 移除 — burst press 不撞舊 event

**Setup**：focus Notepad（任何 input 都行）。
**Steps**：
1. 按住 `Right Alt` 說話、放開。
2. 在 transcribing window（~1 秒）內立刻按住 `Right Alt` 開新錄音。
3. 觀察 HUD 是否被前一次 paste 失敗 event clobber。

**Expected**：
- 新錄音啟動、HUD 進 recording state。
- 即使前一次 paste 因 `FocusRestoreFailed` 走 error 路徑、新錄音的 status 不被舊 event clobber 成 `error`。
- M5 chunk 1 移除 `paste:focus-restore-failed` listener、由 `handleStop` catch path 統一處理 → IDEAS.md 「PASTE_FOCUS_RESTORE_FAILED state collision」P2 已 resolved。

---

## Bonus — Reduced-motion mid-recording 切換

**Setup**：Windows 設定 → Animation effects = ON（reduced motion OFF）。
**Steps**：
1. 按住 `Right Alt` 開始錄音、HUD 顯示 6-bar waveform。
2. **錄音中** 切到 Windows 設定、把 Animation effects 改 OFF。
3. 切回 Notepad、繼續錄音。
4. 觀察 HUD 是否切換成 dot fallback。

**Expected**：
- HUD `prefers-reduced-motion` `matchMedia('change')` listener pick up OS 設定變更、reactive ref 切換、HudWaveform 在下一個 RAF tick 從 6 bars → dot。
- 沒有 console error / warning。
- 沒有 visual glitch（bar 殘影、layout 跳動）。

---

## 驗收完成後

1. 把 14 條（+ bonus）一一 ✅ 在 PR description 或 session log。
2. 任何一條 ❌ → 開 issue / 補修 commit。
3. 全綠 → 在 `doc/plans/02-implementation-roadmap.md` 把 M5 acceptance 從 `📋 Planned` 改成 `✅ Done`、bump 「最後更新」、更新 `.claude/PROGRESS.md`「現在在哪」line。

---

## 已知 Phase 2 deferred 項目（不在 M5 scope、IDEAS 已記）

- **Dashboard sidebar 多 state badge / tooltip**：當前只顯示 `recording`、Phase 2 加 `transcribing` / `error` state 與 tooltip（chunk 3 reviewer P2-1）。
- **`aria-live="polite"` 連續同訊息 SR 不 announce**：rapid hotkey press 重複「Recording」第二輪 SR 略過、Phase 2 a11y polish 加 dummy aria-label change 強制 announce（plan-time challenger P2-2）。
- **`SidebarFooter` empty wrapper artifact**：idle 時 `<SidebarFooter>` 仍 render 一個 padding 空 band；M9 polish 把 `v-if` hoist 進 parent（chunk 3 reviewer P2-1）。
- **i18n key duplication**：`sidebar.recordingBadge` 與 `dashboard.audioTest.recording` 都是「錄音中」、M9 i18n consolidation（chunk 3 reviewer P2-2）。
- **`role="status"` 雙視窗重複 announce**：HUD + Dashboard badge 都 `role="status"`、SR 念兩次「Recording」、Phase 2 a11y testing 後可能改 Dashboard 為 `aria-hidden="true"`（chunk 3 reviewer P2-3）。
- **Vitest `<Transition>` 在 jsdom timing 不可靠**：transition timing 留 Playwright 驗、vitest 只測 state→class（plan-time challenger P1-2）。
- **Vue 3.5 `<Transition mode="out-in">` race**：與 `setIgnoreCursorEvents` async sequence 在 rapid state change 是否有殘留 cursor state — Phase 2 dogfood 觀察。
- **HUD width 380px vs 長 error message truncation**：spec §8.1 已加 `text-overflow: ellipsis` + `max-width: 280px`，極長 error 會截尾；M9 polish 看是否動態 resize。
