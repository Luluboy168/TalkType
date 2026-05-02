# Ideas / 想法 parking lot

> 不屬於當前 milestone 的想法、改進建議、待考慮事項。
> 進入某個 milestone 時可以掃過、把相關項目搬進 milestone 的 plan 或 session log。

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

## Phase 2 / 後期想法

- **macOS dev setup**：目前 doc 只 describe 設計，沒實機跑過。Phase 2 啟動時要做 spike。
- **Sentry telemetry opt-in flow**：要設計清楚的 UI、預設 off、第一次用時的 dialog（學 SayIt 但 SayIt 預設 on，我們改 off）。
- **Code signing**：Phase 2 需要 cert provider（DigiCert / Sectigo / 個人 EV cert），預算考慮。
- **Streaming transcription**：SayIt 沒做、user demand 高但複雜。
