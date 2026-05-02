# Phase 1：OSS MVP

> **狀態**：Draft v1
> **最後更新**：2026-05-02
> **目標時程**：1–2 個月（pace 取決於投入時間）

## Phase 1 一句話描述

> 在 Windows 上能用、放上 GitHub、使用者 clone+build 後能體驗 SayIt-like full UX，但不發行安裝包、不簽名、不收 telemetry。

## 完成定義（Definition of Done）

Phase 1 結束時，以下全部達成才算 done：

- [ ] **核心流程能用**：在任何 Windows app 按住預設熱鍵（Right Alt）說話 → 放開後文字 < 3 秒貼上
- [ ] **HUD overlay**：錄音時顯示透明 overlay，至少 4 個 visual states（recording / transcribing / success / error）
- [ ] **Dashboard 視窗**：設定頁 + 歷史頁能 work
- [ ] **Hybrid transcription**：Cloud (Groq) 與 Local (whisper.cpp) 都能跑，settings 可切換
- [ ] **LLM polish**：預設開、可關閉，Groq/OpenAI/Anthropic/Gemini 都能 work
- [ ] **API key 安全儲存**：用 Windows Credential Vault（不是 plaintext JSON）
- [ ] **音效回饋**：start / stop / error sound
- [ ] **熱鍵自訂**：使用者可改成別的鍵
- [ ] **音頻裝置選擇**：可選用哪支 mic
- [ ] **歷史紀錄 (SQLite)**：每次轉錄存進去，Dashboard 能看
- [ ] **手動詞彙字典**：使用者加詞、Whisper prompt biasing
- [ ] **i18n**：至少 zh-TW + en
- [ ] **Single-instance lock**：兩個 instance 不會打架
- [ ] **README**：完整安裝/使用/開發說明（中英對照）
- [ ] **GitHub Actions CI**：每個 PR 跑 vue-tsc + vitest + cargo check
- [ ] **License + Code of Conduct + Contributing**：基本 OSS 配套
- [ ] **能 clone + build 出可執行的 dev binary**：Windows 機器跑 `pnpm tauri dev` 就能玩
- [ ] **Dev DX**：Claude Code hooks（typecheck、rustfmt、eslint、protect lockfiles）

## Phase 1 範圍：必要功能清單

### A. 核心 Voice Flow

| 功能 | 細節 |
|---|---|
| 全域熱鍵監聽 | Windows `SetWindowsHookExW`，支援 Hold/Toggle 模式 |
| 錄音 | `cpal` 16kHz mono PCM，動態長度（1s ~ 10min） |
| 轉錄 (Cloud) | Groq Whisper (whisper-large-v3-turbo)，HTTP multipart upload |
| 轉錄 (Local) | whisper.cpp via Rust binding，base 模型 |
| Provider 切換邏輯 | Settings 切換 + 自動 fallback（無 API key → Local） |
| LLM polish | 多 provider（Groq/OpenAI/Anthropic/Gemini），可開關 |
| Auto paste | `SendInput` 模擬 Ctrl+V 到當前 focused window |
| Mute system audio during recording | WASAPI（同 SayIt） |

### B. UI

| 元件 | 細節 |
|---|---|
| HUD overlay | 透明、always-on-top、無邊框、4 visual states |
| Dashboard 主視窗 | 標準 Windows 視窗、shadcn-vue UI |
| Settings 頁 | 熱鍵 / Provider / API keys / 音頻裝置 / 語言 / Polish 開關 |
| History 頁 | SQLite-backed list、可搜尋、可重播 audio |
| Dictionary 頁 | 手動加詞 / 編輯 / 刪除（不做 AI smart learning） |
| 系統匣 icon | 顯示 / 退出選單 |
| 音效回饋 | start / stop / error WAV |

### C. 持久化

| 項目 | 儲存位置 |
|---|---|
| API keys | Windows Credential Vault（透過 `keyring` crate） |
| 設定（熱鍵、provider 選擇、開關等） | `tauri-plugin-store` JSON |
| 歷史紀錄 | SQLite via `tauri-plugin-sql` |
| 詞彙 | SQLite |
| Audio files | `%APPDATA%\com.luluboy168.talktype\recordings\<uuid>.wav` |

### D. 開發者 DX

| 項目 | 細節 |
|---|---|
| Type checking | `vue-tsc --noEmit` 在 CI 與 Claude Code hook |
| Rust linting | `cargo clippy --` 在 CI |
| Auto formatting | `rustfmt`、`eslint --fix` 在 Claude Code post-edit hook |
| Lock file protection | Claude Code pre-edit hook block 修改 `Cargo.lock` 與 `pnpm-lock.yaml` |
| Pre-commit verify skill | 手動 invoke `verify` skill 跑全套檢查 |
| GitHub Actions CI | `ci.yml` 跑 type check + Vitest + cargo check |

### E. i18n

- zh-TW（主要 UI 語言）
- en（國際使用者）
- Whisper transcription 語言可獨立設定（與 UI 語言分離）

## 明確不做的（Phase 1 Out of Scope）

延遲到 Phase 2 或永遠不做：

| 功能 | 為什麼不在 Phase 1 |
|---|---|
| **macOS 支援** | Windows-first，macOS 在 Phase 2 |
| **Windows installer (MSI/NSIS)** | OSS 專案讓使用者自己 build；Phase 2 才包 |
| **Auto-updater (Tauri updater)** | 沒 installer 就不需要；Phase 2 加 |
| **Code signing** | 成本（憑證費）+ 流程複雜度；Phase 2 |
| **Sentry telemetry** | 隱私；Phase 2 評估後加，預設 opt-in |
| **AI smart dictionary learning** | 複雜度高、需要 LLM 計費考量；Phase 2 |
| **Quality monitor / correction monitor** | SayIt 的進階 UX；Phase 2 評估 |
| **Audio at rest 加密** | 增加複雜度；Phase 2 加 |
| **Multi-monitor HUD positioning** | 單螢幕中央夠用；Phase 2 |
| **Custom UI theme / dark mode** | 跟系統就好；Phase 2 |
| **Dashboard 統計圖表** | 純文字 list 已經夠；Phase 2 |
| **Sound feedback 自訂音效** | 內建一組就好；Phase 2 |
| **Edit mode（選取文字 → AI 改寫）** | SayIt v0.9.0 才加的；Phase 2 |
| **Streaming transcription** | 增加複雜度；Phase 2 評估 |

## 預計 Phase 1 不會踩的雷（取自 SayIt 教訓）

- ❌ Frontend 直接開 SQLite（雙視窗 connection pool race） → ✅ Rust 擁有 SQLite，frontend 只接 commands
- ❌ API key 存 plaintext JSON → ✅ Windows Credential Vault
- ❌ 9 個 minor versions 才加 single-instance lock → ✅ Day 1 就加
- ❌ Test pyramid bottom-heavy（443 unit / 1 e2e） → ✅ 做 IPC contract test 與基本 e2e
- ❌ 1907 行 SettingsView.vue → ✅ 拆 sub-components，每個 < 400 行
- ❌ HUD 0 個 ARIA hints → ✅ 加 aria-label / aria-live + `prefers-reduced-motion`

## Phase 1 風險

| 風險 | 機率 | 緩解 |
|---|---|---|
| whisper.cpp Rust binding 不穩 | 中 | 候選：`whisper-rs`、`whisper.cpp` 直接 FFI；先驗證再投入 |
| 全域熱鍵跟 Windows 11 某些 keymap 衝突 | 中 | 預設 `Right Alt`、提供熱鍵 recording UI 讓 user 自訂 |
| LLM 多 provider 維護 burden | 低 | Phase 1 只 ship 4 provider、API 形狀類似（OpenAI-compat 為主） |
| Tauri v2 + cpal 在 Windows 有未知 bug | 中 | 早做 prototype、跟 SayIt CHANGELOG 對照 known issues |
| 個人時間 / 專注度 | 高 | 拆 milestone，每個 < 1 週可完成 |

## Phase 1 里程碑（高層次）

詳細 task breakdown 在 [`doc/plans/02-implementation-roadmap.md`](../plans/02-implementation-roadmap.md)。

| Milestone | 內容 | 週 |
|---|---|---|
| **M0：Repo bootstrap** | Tauri v2 scaffold、Vue 3 + shadcn-vue、CI 跑起來 | 1 |
| **M1：基礎 IPC 與雙視窗** | HUD + Dashboard 都開得起來、event 通得過 | 1 |
| **M2：錄音 pipeline (Rust)** | cpal 錄音、WAV encode、save to disk | 1 |
| **M3：Cloud transcription (Groq)** | 錄音 → Groq Whisper → 拿到文字 | 1 |
| **M4：全域熱鍵 + paste** | 按 Right Alt 觸發、SendInput paste、E2E flow 跑通 | 2 |
| **M5：HUD overlay 完成** | 4 visual states、settings 頁能切 | 2 |
| **M6：LLM polish + 多 provider** | Polish 開關、4 provider 都能跑 | 1 |
| **M7：Local whisper.cpp 整合** | 模型下載 UX、本地 fallback 邏輯 | 2 |
| **M8：History + Settings + Dictionary 完成** | Dashboard 三個頁面 polish | 1 |
| **M9：Polish + 測試 + 發布** | Bug fix、README、Tag v0.1.0 | 1 |

**總計**：~12 週寬鬆估計（單人 part-time）；專注全力 4–6 週可達。

## 退出 Phase 1 的觸發條件

達到任一即可進入 Phase 2 規劃：

- 上述 Definition of Done 全部 ✅
- 自己每天用，沒嚴重 bug 連續 1 週
- GitHub 上有外部使用者的有意義 feedback（issues / discussions）

## 下一步

進 Phase 2 — 看 [`02-phase2-release.md`](02-phase2-release.md)。
