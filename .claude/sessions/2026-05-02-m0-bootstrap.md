# 2026-05-02 — M0 Bootstrap + Session Memory Setup

> **Session topic**：完成 Phase 1 Milestone 0（Repo bootstrap）、建立 .claude session memory
> **Outcome**：✅ M0 Done、PR #1 merged、CI green、memory 系統建立並 commit

## What changed

### M0 implementation（5 commits）

依序 4 個 implementation chunks 全部由 Opus 4.7 subagent 完成，每個 chunk 後接一個 `superpowers:code-reviewer` subagent review。最後一個 E2E verification subagent 驗 acceptance criteria + screenshot UI。Main session 不直接寫 code，只 orchestrate + commit。

| Commit | Type | Files | 內容 |
|---|---|---|---|
| `43cedcb` | chore | 38 | C1 scaffold：create-tauri-app vue-ts → Tauri 2.11 / Vue 3.5.33 / Vite 8.0.10 / TS 6.0.3 / Pinia 3.0.4 / Vue Router 5.0.6 / pnpm 10.33.2 / Node 24；Cargo profile.release；tauri.conf identifier |
| `96f8591` | chore | 6 | C2 dev tooling：ESLint 10 flat、Vitest 4 + jsdom、Playwright 1.59、rust-toolchain.toml、tsconfig strict + alias、vite.config (port 1420 strict + `__APP_VERSION__`)、smoke test |
| `70b5d13` | feat | 3 | C3 UI stack：Tailwind v4.2.4 + `@tailwindcss/vite`、shadcn-vue 2.6.2 (reka-nova)、reka-ui 2.9.6、lucide-vue-next 1.0.0、`@vueuse/core` 14.3.0、components.json、`src/lib/utils.ts` |
| `8935ab5` | chore | 6 | C4 hooks + CI：4 個 hook scripts (protect-config / typecheck / rustfmt / eslint)、.claude/settings.json、.github/workflows/ci.yml on windows-latest |
| `a2dfb68` | docs | 2 | docs reconciliation：plans 對齊實際裝的版本（Vite 8 / TS 6 / shadcn-vue runtime / lucide v1 / reka-nova） |

### CI 結果

- PR #1 → CI on windows-latest → success in **3:39**
- 16 個 step 全綠、cargo check 占 104s、smoke test 2s、vue-tsc 1s、eslint 6s
- 18 個 ESLint warnings 在 default `App.vue`（cosmetic，M1 重寫 App.vue 後消失）

### Session memory setup（這次 session 的最後一步）

建立 `.claude/PROGRESS.md` + `.claude/sessions/` + `.claude/IDEAS.md`，更新 CLAUDE.md。

## Key decisions

- **接受 Vite 8 / TS 6 / `@vitejs/plugin-vue` 6 / vue-tsc 3** 而不 downgrade：`create-tauri-app` 4.7 預設都是 latest stable；C3 證實 shadcn-vue 在這組合上能 work。
- **shadcn-vue style spec drift**：roadmap 寫 "new-york"，但 v2 CLI 改成 vega/nova/maia/lyra/mira/luma；用 `--defaults` → `reka-nova`（new-york 的現代等價）。
- **shadcn-vue 2.x 是 runtime dep + CLI**（不只是 CLI），其 `dist/tailwind.css` 被 `src/assets/index.css` `@import`。**注意以後不要 `pnpm remove shadcn-vue`**。
- **TS 6 deprecated `baseUrl`**：用 `ignoreDeprecations: "6.0"` 暫時 keep。Reviewer 實驗證明拿掉 baseUrl 用 paths 也行。兩種都 work，先選 keep。
- **Hook schema**：用最新的 `hookSpecificOutput.permissionDecision: "deny"`，不是 legacy `decision: "block"`。
- **CI 不跑 Tauri build / Playwright e2e / `clippy -D warnings`**：Phase 1 太重，留 Phase 2。
- **Commit 拆 4 + 1**：4 個 implementation commits（按 chunk）+ 1 docs reconciliation commit。
- **PR + CI 流程**：必須開 PR 觸發 CI（ci.yml 只在 push-to-main / PR-to-main 觸發）。

## Surprises / 踩雷

- **`pnpm dlx create-tauri-app` 在 napi binary resolution 卡住** — fallback 到 `cargo install create-tauri-app` 4.7.0 才 scaffold 成功
- **Rust 不在用戶機器上** — agent 自己 rustup install 1.95.0 stable
- **corepack EPERM** on `C:\Program Files\nodejs\yarnpkg` — 用 `corepack pnpm` 規避
- **`pnpm` 不在 child process PATH** 影響 `pnpm tauri build`（Tauri 的 `beforeBuildCommand` 找不到 pnpm）— agent 用 user-bin shim 規避，build 完移除。**M1 README 要寫 setup 注意事項**
- **Geist Google Fonts external `@import`** — shadcn-vue init 預設加了 `@import url('https://fonts.googleapis.com/...')`，對 offline-first Tauri + 嚴格 CSP 是定時炸彈。**M1 改成 `@fontsource-variable/geist`**
- **`pnpm tauri build` 比預期快**：3 分鐘產出 3.06 MB unsigned exe + 1.63 MB MSI + 1.12 MB NSIS（first run）
- **CI 比預期快**：first run 3:39，總時 cargo check 占 104s。`Swatinem/rust-cache` 第一次寫入只花 28s
- **Hook scripts 在本 session 不會 fire**：因為 `.claude/settings.json` 是 session 啟動後才寫的；下次新開 session 才會 pick up
- **`gh` CLI 沒裝**：user 透過 `winget install --id GitHub.cli` 安裝後跑 `gh auth login` 互動完成 → 我用 full path `& "C:\Program Files\GitHub CLI\gh.exe"` 規避 PATH 沒 reload

## Follow-ups for M1+

- [ ] 把 `src/App.vue` 整個改寫（拿掉 hardcoded hex `#646cff` `#0f0f0f` `#f6f6f6`）— 用 shadcn token (`bg-background`, `text-foreground` 等)
- [ ] Geist Google Fonts → `@fontsource-variable/geist`（offline + CSP 友善）
- [ ] README 寫 Windows dev setup 注意事項：corepack pnpm PATH 問題、Rust install via rustup、VS Build Tools、`gh` CLI 推薦裝
- [ ] 加 `main-window.html` + 雙 entry vite config（HUD vs Dashboard）— M1 主任務
- [ ] **2026-06-02 deadline**：把 `actions/checkout@v4` / `actions/setup-node@v4` / `pnpm/action-setup@v4` bump 到 v5/v6（GitHub Node 20 deprecation）
- [ ] 有空可以實測拿掉 `tsconfig.json` 的 `baseUrl` + `ignoreDeprecations`（paths 在 TS 6 應該也 work）

## Subagent dispatch pattern（M0 證實有效）

對純 scaffold work：
1. Sequential chunks（不平行 — share `package.json` / lockfile）
2. 每 chunk：1 個 implementation agent (`Opus 4.7` `general-purpose`) → 1 個 review agent (`Opus 4.7` `superpowers:code-reviewer`)
3. 最後 1 個 E2E verification agent
4. Main session 不直接寫程式 — 只 orchestrate；commit 由 main agent 親手做

對下個 milestone（M1 IPC + 雙視窗）大概也 work；M4（hotkeys）/ M7（whisper）等高風險 milestone 可能需要 main agent 多介入 debug、即時看 log。

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point）
2. 本檔（M0 session log）— 特別是「Follow-ups for M1+」段
3. `.claude/IDEAS.md`
4. `doc/plans/02-implementation-roadmap.md` M1 section
5. （如要 IPC 設計細節）`doc/plans/01-architecture.md`、`doc/plans/03-rust-modules.md`、`doc/plans/04-frontend-structure.md`
