# 2026-05-02 — M1 Dual-window IPC + Tray + Single-instance

> **Session topic**：完成 Phase 1 Milestone 1（基礎 IPC + 雙視窗）— HUD + Dashboard、tray icon、single-instance、ping/pong smoke 走通
> **Outcome**：✅ M1 Done、static checks all green、Tauri dev build 通過冒煙、acceptance 中 OS-level 行為留待使用者 manual verify

## What changed

### M1 implementation（3 chunks）

延續 M0 證實的 subagent-driven 模式：main session 不直接寫 code，3 個 implementation chunks 由 Opus 4.7 subagents 並由 reviewer subagents 把關。Main session 負責 orchestrate、commit。

| Commit | Type | 內容 |
|---|---|---|
| `f236cec` | feat(m1) | Chunk 1 — Frontend dual-entry：`index.html` + `main-window.html`、`src/main.ts` + `src/main-window.ts`、Pinia + Vue Router (hash)、vue-i18n（zh-TW + en）、AppSidebar + 5 placeholder routes、`useTauriEvents.ts` constants、`src/types/events.ts` 型別、`HudOverlay.vue` placeholder |
| `2c85b7a` | feat(m1) | Chunk 2 — Rust dual-window + tray + single-instance：`tauri.conf.json` HUD `main` (transparent overlay) + Dashboard `main-window` (decorated)、tray icon (left-click + menu)、`tauri-plugin-single-instance`、Dashboard close-request 攔截為 hide、`ping` command + `ipc:pong` event |
| (本 chunk 3) | feat(m1) | Chunk 3 — UI 把 ping/pong 接起來：`IpcSmokeTest.vue` (Dashboard 點按發 ping、列最近 5 筆 pong)、HUD `pongCount` badge、i18n 補齊、IPC contract 表 + 進度 dashboard 文件更新 |

### Files changed by Chunk 3

- 新增 `src/components/IpcSmokeTest.vue`
- 改寫 `src/views/DashboardView.vue` 改 embed `<IpcSmokeTest />`
- 改寫 `src/components/HudOverlay.vue` 加 pong listener + count badge
- 補 i18n keys (`hud.pongCount`、`dashboard.ipcSmoke.{title,sendPing,waiting}`) 在 `zh-TW.json` + `en.json`
- 更新 `doc/plans/01-architecture.md` IPC 契約表（加 `ping` command + `ipc:pong` event）
- 更新 `doc/plans/02-implementation-roadmap.md` 進度 dashboard（M1 ✅ Done）
- 更新 `.claude/PROGRESS.md`（M1 ✅、新增 session row）
- 新增 `docs/screenshots/m1/m1-hud-vite.png`、`docs/screenshots/m1/m1-dashboard-vite.png`

## Key decisions

- **camelCase serde rename**：Rust 端 `PongPayload { source, timestamp_ms }` 用 `#[serde(rename_all = "camelCase")]`；TypeScript `PongPayload { source, timestampMs }` 對齊；以後新加 event 一律此 pattern。
- **Single-instance plugin 必須最先 register**：依 `01-architecture.md` invariant rule #10，desktop-only `#[cfg(desktop)]` gating、優先於其他 plugin。
- **Transparent body 透過 `data-window` attr**：`main.ts` 啟動時 `document.body.setAttribute("data-window", "hud")`，CSS 用 `[data-window=hud] body { background: transparent }`。Dashboard 用 `data-window=dashboard` 走預設背景。比靜態 HTML class 更乾淨（HMR + theme 切換友善）。
- **Capabilities 拆 hud.json + dashboard.json**：HUD 沒 SQL/store/keyring 權限；對 SayIt 改進。
- **拋棄 Tauri default `greet` command**：M0 scaffold 預留的 `greet` 只是 demo，M1 砍掉只留 `ping`。Frontend type / capability 同步移除。
- **HUD positioning 留到 M5**：M1 acceptance 描述 HUD「螢幕水平置中、y=50」，但 M1 只把 `visible: false` 設好；實際 show/hide + reposition 與 voice flow state machine 綁，留 M5 一起做。
- **Ping command 用 `Result<(), String>` 不用 thiserror enum**：唯一可能失敗是 `Emitter::emit` 序列化錯誤，扁平字串夠用；M2+ 才需要 typed error enum。
- **Dropped `greet` from frontend**：M0 預留的 demo 也清掉，避免將來誤用。
- **Pong badge in HUD 只顯示 count**：M1 階段不需要 source/timestamp 細節；HUD 是 dev-only smoke，等 M5 真正 voice flow 起來時整個 HudOverlay 會被改寫。

## Surprises / 踩雷

- **`pnpm dlx` 在 worktree 偶爾噴 ERR_PNPM_NO_IMPORTER_MANIFEST_FOUND**：M1 chunk 1 加 sidebar component 時遇到。改用 `corepack pnpm exec shadcn-vue add sidebar --yes` 規避（不走 dlx）。
- **`cargo` 不在 bash 預設 PATH**：M0 已知，每次 cargo command 前 `export PATH="$HOME/.cargo/bin:$PATH"`。
- **PowerShell cwd quirk**：背景 PowerShell 跨 session 不繼承當前 cwd，但路徑都用絕對路徑，沒實質影響。
- **Tauri `beforeDevCommand: "pnpm dev"` 在 child process PATH 找不到 pnpm**（M0 已知踩雷重現）：`corepack pnpm tauri dev` 會 fork `pnpm.cmd` 子進程而 `pnpm` 不在原生 PATH 上、Tauri 的 child process 不繼承 corepack 的 dynamic shim。
  - **規避**：建一個 `C:\Users\lulub\AppData\Local\Temp\pnpm-shim\pnpm.cmd` 包裝 `corepack pnpm`，從 PowerShell 把該目錄加到 `$env:PATH` 再起 `pnpm.cmd tauri dev`。Tauri sub-shell 找得到 pnpm。
  - **Follow-up**：寫進 README dev setup（M9 之前），或者考慮 chunk-1 把 `tauri.conf.json` 的 `beforeDevCommand` 改成 `node ./node_modules/vite/bin/vite.js`，繞過 pnpm 依賴。

## Acceptance criteria（M1 roadmap 4 條）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| Run `pnpm tauri dev`：HUD 透明 overlay 出現、Dashboard 隱藏 | Build 成功、`talktype.exe` 啟動 | ⚠️ HUD `visible: false`，看不到 overlay；user 可暫時改 `tauri.conf.json` HUD `visible: true` 驗證 |
| Click tray icon → Dashboard 顯示 | Code path 已實作（`focus_dashboard` + `on_tray_icon_event`） | ✅ user 操作 |
| 第二次跑 talktype.exe → 第一個 dashboard focus | Single-instance plugin 已註冊 | ✅ user 操作 |
| Ping/pong test 在兩個 window 都收到 | Vite-shape screenshots ✅、IPC 邏輯 ✅、Tauri runtime build pass ✅ | ✅ user 跑 tauri dev 點 Send ping 確認 HUD pong-count + Dashboard list 同步 |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → 1 pass
- `cargo check` → clean (0.85s incremental)
- `cargo clippy --all-targets -- -D warnings` → clean

### Tauri dev runtime smoke

- Vite 起來 ms 級
- `cargo run --no-default-features` → 5.07s（chunk 2 已預熱 cache）
- `talktype.exe` 啟動成功
- Process tree：cargo (PID 4676) + talktype.exe (PID 49672) — 兩者皆已 `Stop-Process -Force` 終止

### Vite-shape screenshots

- `docs/screenshots/m1/m1-hud-vite.png`（400×100，HUD pill 渲染正常）
- `docs/screenshots/m1/m1-dashboard-vite.png`（1280×800，sidebar + IPC smoke card「IPC 連線測試 / 發送 ping / 等待 pong…」）

## Manual verification checklist（user 跑 `pnpm tauri dev` 後）

- [ ] HUD overlay 可見？（⚠️ HUD `visible: false`，看不到也算 expected — 暫時改 conf 為 `true` 才能視覺驗證；M5 wires 正式 show/hide）
- [ ] Tray icon 出現在系統 tray？
- [ ] **右鍵 tray → "Open Dashboard"** → Dashboard 視窗顯示？
- [ ] **左鍵 tray icon** → Dashboard 視窗顯示？
- [ ] Dashboard 點 X 關掉 → 是 hide 而不是 exit（再點 tray 能再開回來）？
- [ ] Tray menu "Quit" → app 完全 exit（process tree 清空）？
- [ ] Dashboard 點 **"發送 ping"** → Dashboard 列出一筆 pong record（source = `main-window`）AND HUD pong-count 同步遞增？（HUD 必須先 visible 才能觀察）
- [ ] 第二次跑 `talktype.exe` 或 `pnpm tauri dev` → 第一個 instance 的 Dashboard re-focus（不會開出第二個視窗）？

## Follow-ups for M2

- **`SettingsView.vue` 拆 sub-components**（M8）：學 SayIt 1907 行教訓，預先設計 7 個 sub-component。
- **README dev setup 寫 pnpm shim 規避**：Tauri `beforeDevCommand` PATH 問題在 M9 release prep 之前要寫進 README，不然 contributor clone 後 `pnpm tauri dev` 立刻撞牆。
- **HUD `visible: true` dev override**：考慮加一個 dev-only command（例如 `set_hud_visible_for_dev`）讓未來 M2/M3/M4 不用每次手改 `tauri.conf.json` 才能視覺驗證。或者用 env `TALKTYPE_DEV_HUD=1`。
- **M2 audio_recorder.rs 起來**：500 行內、單一 mutex、定義好 `AudioRecorderError` enum。SayIt 教訓 = `audio_recorder.rs` 千行起跳是肥的。

## Subagent dispatch pattern（M1 證實）

承襲 M0 模式並驗證在 M1（IPC + 雙視窗）也適用：

1. **3 個 implementation chunks 順序執行**（chunk 共享 `tauri.conf.json` / `package.json`、不適合並行）：
   - Chunk 1（frontend）：subagent A 實作 → reviewer subagent 驗
   - Chunk 2（Rust）：subagent B 實作 → reviewer subagent 驗
   - Chunk 3（UI wiring + docs）：subagent C 實作（本 session）→ reviewer subagent 待跑
2. **Main session 不寫 code、只 orchestrate + commit**：每個 chunk 完成後 main agent 親手 commit、撰 commit message。
3. **每個 chunk 後緊接 reviewer subagent**：catch implementation 盲點（M1 reviewer 抓到的點之後補進這份 log）。

對 M2（audio recorder）大概也適用相同 pattern；M4（hotkey）/ M7（whisper）等高風險 milestone 可能需要 main agent 介入 debug、即時看 log。

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M2 將是下一個進度條）
2. 本檔（M1 session log）— 特別是「Follow-ups for M2」段
3. `.claude/IDEAS.md`
4. `doc/plans/02-implementation-roadmap.md` M2 section（audio pipeline）
5. `doc/plans/03-rust-modules.md` audio_recorder 章節
6. `doc/plans/06-hybrid-transcription.md`（雖然 M2 還不需要，但有助於理解 M3 cloud 接手）
