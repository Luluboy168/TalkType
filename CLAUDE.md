# TalkType — Claude Code 專案記憶檔

> 跨平台桌面語音輸入工具（Windows-first）
> 按住熱鍵說話 → 放開貼上文字
> Tauri v2 + Vue 3 + Rust，Hybrid Cloud (Groq) + Local (whisper.cpp) 轉錄

## 專案狀態

- **Phase**：Phase 1 — Implementation（M0 ✅ Done @ 2026-05-02、M1 📋 Planned）
- **Repo**：https://github.com/Luluboy168/TalkType
- **Bundle ID**：`com.luluboy168.talktype`

## Session Memory（給 Claude Code 接力用 — 開始 working 前先讀）

`.claude/` 下有跨 session 共用的 memory（committed 進 git）：

| File | 用途 |
|---|---|
| [`.claude/PROGRESS.md`](.claude/PROGRESS.md) | 整體進度 + entry point |
| [`.claude/sessions/`](.claude/sessions/) | 每個 working session 的詳細紀錄（What changed / Key decisions / Surprises / Follow-ups） |
| [`.claude/IDEAS.md`](.claude/IDEAS.md) | 不歸屬於當前 milestone 的想法 parking lot |

**對 Claude Code（你）**：開始任何 task 前，先讀 `.claude/PROGRESS.md` 與最新一篇 session log（順便 skim `IDEAS.md` 看有沒有相關項目）。Session 結束 / 重要 task 完成時，append summary 到當天的 session file（或新建 `sessions/YYYY-MM-DD-topic.md`）。

## 文件導覽（快速參照）

> 對 Claude Code：work 此 repo 前先讀 [`doc/goals/`](doc/goals/) 與相關的 [`doc/plans/`](doc/plans/) section。

| 想知道 | 看這裡 |
|---|---|
| 為什麼做 / 目標使用者 | [`doc/goals/00-vision.md`](doc/goals/00-vision.md) |
| Phase 1 範圍與里程碑 | [`doc/goals/01-phase1-mvp.md`](doc/goals/01-phase1-mvp.md) |
| Phase 2 發行規劃 | [`doc/goals/02-phase2-release.md`](doc/goals/02-phase2-release.md) |
| 成功指標 | [`doc/goals/03-success-criteria.md`](doc/goals/03-success-criteria.md) |
| 為什麼選 Tauri+Vue+Rust | [`doc/plans/00-tech-stack.md`](doc/plans/00-tech-stack.md) |
| 系統架構 / IPC 契約 | [`doc/plans/01-architecture.md`](doc/plans/01-architecture.md) |
| 實作 milestone 拆解 | [`doc/plans/02-implementation-roadmap.md`](doc/plans/02-implementation-roadmap.md) |
| Rust 模組規劃 | [`doc/plans/03-rust-modules.md`](doc/plans/03-rust-modules.md) |
| Vue 專案結構 / Pinia stores | [`doc/plans/04-frontend-structure.md`](doc/plans/04-frontend-structure.md) |
| SQLite schema / Settings JSON / Credential Vault | [`doc/plans/05-data-model.md`](doc/plans/05-data-model.md) |
| Cloud + Local 轉錄切換 | [`doc/plans/06-hybrid-transcription.md`](doc/plans/06-hybrid-transcription.md) |
| Phase 2 發行 / 簽名 / CI 細節 | [`doc/plans/07-phase2-distribution.md`](doc/plans/07-phase2-distribution.md) |
| **SayIt 完整分析（reference / inspiration）** | [`doc/reference/`](doc/reference/) |

## 內容撰寫規則（Writing Rules）

> 對 Claude Code：在這個 repo 編輯 / 新增 markdown 文件時，**必須遵守**以下規則。Code 規則另外在 plans/ 詳述。

### 語言

- **預設用繁體中文**寫文件（zh-TW），技術術語可保留英文（API、Component、Hook 等）
- **程式碼註解 / commit message** 用英文
- **Code identifier**（變數、函式、檔名）一律英文
- 文件中可用中英夾雜，例如：「我們用 `keyring` crate 把 API key 存進 Windows Credential Vault」
- 翻譯既有英文 SayIt 分析文件時：保留術語、句子結構可重新組織以符合中文閱讀習慣

### Markdown 風格

| 元素 | 規則 |
|---|---|
| 標題 | H1 一個檔案只一個（檔名同義）、層級遞增不跳級 |
| 斜體 | 用 `*單個 asterisk*`，不用 `_underscore_` |
| 強調 | 用 `**雙 asterisk**` |
| 行內 code | 用 backtick：``` `pnpm tauri dev` ``` |
| Code block | 必須標語言：` ```typescript`、` ```rust`、` ```bash` |
| Table | 標題與內容用 `\|---\|---\|` 對齊；複雜內容可用 unicode `─ │ ┌` 但避免 |
| List | `-` 開頭、不混用 `*`；nested 縮排 2 spaces |
| Link | 相對路徑優先：`[檔名](../goals/00-vision.md)`；外部 absolute URL |
| Quote | `>` 開頭，當作「引用」「重要警示」「來源標註」用 |

### 檔名規範

- **kebab-case**：`hybrid-transcription.md` ✅、`HybridTranscription.md` ❌、`hybrid_transcription.md` ❌
- 數字前綴用於排序：`00-vision.md`、`01-phase1-mvp.md`
- 副檔名統一 `.md`（不是 `.markdown`）

### 文件結構

每個 doc 以以下開頭：

```markdown
# 文件標題

> **狀態**：Draft v1 | Approved v2 | Deprecated
> **最後更新**：YYYY-MM-DD
> （視情況加） **前提**：依賴的其他文件

## TL;DR / 一句話描述

(可選；複雜文件建議放)

## 主要內容...
```

### 內文 conventions

| 標記 | 用途 |
|---|---|
| `✅` | 達成 / 完成 / 推薦做 |
| `❌` | 禁止 / 反例 / 不做 |
| `⚠️` | 警告 / 注意 |
| `★` | 對 SayIt 的差異 / 改進 |
| `📋` | 待規劃 / 未開始 |
| `🚧` | 進行中 |
| `🔒` | 安全 / secret 相關 |

範例：

> ★ **對 SayIt 改進**：把 SQLite 連線池放 Rust（避免雙視窗 race condition）

### Cross-reference

- **永遠用相對路徑**：`[link](../plans/01-architecture.md)`
- 連到 section：`[link](../plans/01-architecture.md#tauri-commands-frontend--rust)`
- 連到外部：完整 URL `https://...`
- **絕不**用 absolute path（例如 `/E:/SynologyDrive/...`）

### 對 SayIt 引用

- 一律標明來源：「來自 SayIt 的設計」「學 SayIt」「對 SayIt 改進」
- SayIt 分析文件在 `doc/reference/`
- 連結例：`參考 [SayIt 前端架構](../reference/sayit-frontend-analysis.md#9-tauri-bridge)`

### 何時更新文件

| 觸發 | 更新動作 |
|---|---|
| 改變設計決策 | 更新對應 `plans/*.md` 的「狀態」+「最後更新」 |
| Phase milestone 完成 | 更新 `plans/02-implementation-roadmap.md` 進度表 |
| 新加 Rust command / Tauri event | 更新 `plans/01-architecture.md` IPC 契約表 |
| Schema 變更 | 更新 `plans/05-data-model.md` 並 bump schema_version |
| 新加 dependency | 更新 `plans/00-tech-stack.md` |
| Phase 1 → Phase 2 過渡 | review 全部 `goals/*` 與 `plans/*` 標註過時內容 |

### 程式碼示例規範

- Rust：型別簽章完整、用 `Result<T, ErrorEnum>` 不用 `Result<T, String>` 在示例中（除非簡化）
- TypeScript：`async` 函數明確 return type、`invoke<T>()` 一定 type annotation
- 不省略重要錯誤處理（即使示例）
- 變數命名清楚（不用 `x`、`tmp`，用 `audioBuffer`、`apiKey`）

### 文件 review checklist（提交 PR 前）

- [ ] 語言一致（zh-TW + 英文術語）
- [ ] Code block 都有語言標籤
- [ ] 內部連結相對路徑
- [ ] 表格 syntax 正確
- [ ] 「最後更新」日期更新
- [ ] 沒有 `TBD` / `FIXME`（除非真要保留）
- [ ] Cross-reference 連得到（不死連結）

## Code 撰寫規則

> 詳細在 [`doc/plans/`](doc/plans/) 各 file。重點摘要：

### TypeScript / Vue

- **永遠** `<script setup lang="ts">`，不用 Options API
- **永遠** `invoke<T>()` 加 type annotation
- **唯一** import `@tauri-apps/api/event` 的地方是 `src/composables/useTauriEvents.ts`
- **唯一** HTTP client 是 `@tauri-apps/plugin-http`（不用 `window.fetch`）
- 用 `lucide-vue-next`（不用 `@tabler/icons-vue`）
- 用 shadcn-vue components（不手寫 button / input / table）
- 語意色彩 token：`bg-card`、`text-foreground`、不用 `bg-zinc-900`

### Rust

- 用 `thiserror` typed errors（不用 `Result<T, String>` 除了 simple cases）
- Error enum 手動 implement `Serialize` 為 string（frontend 收到 flat string）
- 跨平台：`#[cfg(target_os = "windows")]` 與 `#[cfg(target_os = "macos")]`
- Long-running event loop（CGEventTap、Windows hook）：named thread + `mpsc::channel` ack
- API key 從 `keyring` crate 讀，**永遠不從 frontend 拿**
- SQLite 由 Rust 擁有（不用 frontend `tauri-plugin-sql`）
- Logging：`println!`/`eprintln!` with prefix 例如 `[audio-recorder]`（Phase 2 升 `tracing`）

### 檔案大小

- 任何 file > 500 行 → 評估拆 sub-modules
- SayIt 教訓：`SettingsView.vue` 1907 行、`useVoiceFlowStore.ts` 1871 行、`hotkey_listener.rs` 1566 行 — 都太肥

### 依賴方向

```
views/ ──→ components/ + stores/ + composables/
stores/ ──→ lib/
lib/ ──→ External APIs

❌ views/ 不可 import lib/
❌ components/ 不可直接執行 SQL
❌ frontend 不可拿 API key
```

## Claude Code Hooks（settings.json）

Phase 1 設好以下 hooks（學 SayIt）：

| Hook | 觸發 | 行為 |
|---|---|---|
| `protect-config.sh` | PreToolUse (Edit/Write) | 🔴 Hard-block 修改 `Cargo.lock`、`pnpm-lock.yaml` |
| `typecheck.sh` | PostToolUse (Edit/Write `.ts`/`.vue`) | 跑 `vue-tsc --noEmit`，非阻斷回報 |
| `rustfmt.sh` | PostToolUse (Edit/Write `.rs`) | 自動 format |
| `eslint.sh` | PostToolUse (Edit/Write `.ts`/`.vue`) | `eslint --fix`，跳過 `components/ui/` |

### 保護檔案

| 檔案 | 等級 |
|---|---|
| `Cargo.lock`、`pnpm-lock.yaml` | 🔴 Hard block |
| `tauri.conf.json`、`Cargo.toml` | 🟡 警告（需確認必要性） |

## 常用指令（Phase 1 開始有）

| 指令 | 用途 |
|---|---|
| `pnpm install` | 裝 deps（用 `--frozen-lockfile` in CI） |
| `pnpm tauri dev` | 開發模式 |
| `pnpm build` | 完整 build（含 `vue-tsc --noEmit && vite build`） |
| `pnpm tauri build` | Build binary（含 Rust） |
| `pnpm test` | Vitest |
| `pnpm test:coverage` | 覆蓋率 |
| `npx vue-tsc --noEmit` | 純型別檢查 |
| `cd src-tauri && cargo clippy -- -D warnings` | Rust lint |
| `cd src-tauri && cargo test` | Rust test |
| `./scripts/release.sh X.Y.Z` | Phase 2：發版 |

## 開發環境需求

- **Node.js 24**（見 `.nvmrc`）
- **pnpm 10.x**（透過 `corepack enable && corepack prepare`）
- **Rust stable**（`rustup default stable`）
- **Windows 11**（Phase 1 開發 + 測試平台）
- **Visual Studio Build Tools**（Rust on Windows 編譯需要）

## Subagent-driven 工作流程（重要）

> 對 Claude Code：所有實作工作必須遵守此流程。Main session 只做 orchestration，重活交給 subagents。

1. **拆解任務**：先把工作拆成小的、獨立可平行的子任務（用 TodoWrite 追蹤）
2. **Dispatch subagents（Opus 4.7）執行實作**：每個子任務派遣 subagent 處理（指定 `model: opus`），主 session 不直接寫 code
3. **完成後 dispatch subagents（Opus 4.7）做 code review + 功能測試**：每個 feature / 任務完成後，派另一組 subagent（`model: opus`，可用 `superpowers:code-reviewer`）進行獨立 code review 與功能測試 — 避免 implementer 自己 review 的盲點
4. **UI 變更必須截圖驗證**：若涉及 UI，subagent 必須用 Playwright 截圖並用 Read 工具檢視（或附給主 session 檢視），確認 UI 符合預期才算完成
5. **Plan-time challenger（每個 milestone 開工前）**：與「拆計畫」**同一則 message 平行** dispatch 一個 challenger subagent（Opus 4.7、`general-purpose`），讀同樣的 spec，從 perf / UX / 安全 / 邊界條件 / 可測試性 / 依賴假設下手提出「沒考慮過的問題」。主 session 把 challenger 的 findings 對照計畫、**修計畫後**才 dispatch 真 implementer。修計畫比修代碼便宜。
6. **Retro challenger（每個 milestone 完成後）**：dispatch 一個 challenger subagent 讀完整 session log + 對應 commits，產出「latent 問題 / UX 缺口 / perf 風險」清單，append 進 `.claude/IDEAS.md`，抓 chunk-by-chunk reviewer 漏掉的整體性問題。

**為什麼**：
- 主 session 的 context 寶貴，subagent 可隔離執行重活、平行加速
- Independent reviewer 比 implementer 更容易發現 bug 與設計問題
- UI 視覺檢查比 type check 可靠（type check 過 ≠ UI 對）
- Reviewer 看「程式碼正不正確」、Challenger 看「計畫對不對 / 整體有沒有 latent 問題」— 兩者覆蓋不同盲區，不重複工
- Plan-time challenger 在 implementer 開工前介入，避免錯誤計畫被忠實實作出來；Retro challenger 在 milestone 完成後總結，把 chunk-level reviewer 抓不到的整體性問題沉澱成下個 milestone 可參考的 IDEAS

## 常見踩雷（Known recurring pitfalls）

### shadcn-vue CLI 會把 Google Fonts `@import` 加回 `src/assets/index.css`

每次跑 `pnpm dlx shadcn-vue@latest add <component>` 或 `corepack pnpm exec shadcn-vue add <component>` 加新 UI 元件時，**CLI 會在 `src/assets/index.css` 第 1-7 行重新插入**：

```css
@import url('https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&display=swap');

/*
   ---break---
   */
```

這違反 M0 follow-up「Geist Google Fonts → `@fontsource-variable/geist`（offline + CSP friendly）」。在 M2 chunk 3 與 M3 chunk 1 都已踩雷。

**規範**：implementer 跑完 `shadcn-vue add ...` **必須** revert 這 7 行（保留 `@import "tailwindcss"` 開始的部分）。Reviewer 必須 grep `fonts.googleapis.com` 在 `src/assets/index.css` 確認沒有 regression。

未來考慮：寫個 post-add `cleanup-shadcn.sh` script 或 git pre-commit hook 自動 strip。

## 工作流程提醒

對 Claude Code working in this repo：

1. **改任何 plans/* 文件**：bump「最後更新」日期
2. **加 Rust command 或 event**：同時更新 `plans/01-architecture.md` IPC 契約表
3. **加 dependency**：同時更新 `plans/00-tech-stack.md`
4. **變更 Schema**：bump `schema_version`、寫 migration、更新 `plans/05-data-model.md`
5. **完成 milestone**：更新 `plans/02-implementation-roadmap.md` 進度 dashboard 與 `.claude/PROGRESS.md`「現在在哪 / 最近的 session」
6. **Phase 1 milestone 卡關**：先看 `doc/reference/sayit-improvements.md`，可能 SayIt 已踩過雷
7. **設計新 feature**：先 brainstorm（用 superpowers:brainstorming skill）、產出 spec、再寫 plan
8. **修 bug**：先看 SayIt CHANGELOG 是否已知、不重蹈覆轍
9. **Session 結束 / 重要 task 完成**：append summary 進當天 `.claude/sessions/YYYY-MM-DD-*.md`（含 What changed / Key decisions / Surprises / Follow-ups）；如當天還沒 session log 就新建一個
10. **想到非當前 task scope 的點子**：丟進 `.claude/IDEAS.md` parking lot
11. **修了 hooks / `.claude/settings.json` 後**：提醒使用者重啟 Claude Code session 才會 pick up（child session 不會 reload settings）

## License

MIT — 詳見 [LICENSE](LICENSE)

## 致謝（Acknowledgments）

TalkType 的設計大量參考 **SayIt** by Tai-Cheng Chen（https://github.com/chenjackle45/SayIt）。SayIt 是 MIT 授權、Tauri v2 桌面語音輸入工具。我們站在他的肩膀上做進一步演進。完整 SayIt 分析在 [`doc/reference/`](doc/reference/)。
