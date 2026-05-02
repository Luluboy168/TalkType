# TalkType

> 按住熱鍵說話，放開後文字自動貼上 — Windows 優先、跨平台、開源的桌面語音輸入工具

🚧 **目前狀態：規劃階段（Pre-implementation）** — 程式碼尚未開始撰寫，本 repo 目前只有設計文件。

## 一句話描述

在任何 Windows app 中按住預設熱鍵說話，放開後語音透過 Whisper API（或本地 whisper.cpp）轉錄，再由 LLM polish 成書面語，直接貼到游標位置。

## 主要特色（規劃中）

- **口語到書面語** — AI 自動去除贅詞、修正標點、整理句構
- **全域熱鍵** — 在任何 Windows 應用程式中觸發，支援 Hold / Toggle 雙模式
- **Hybrid 轉錄** — Cloud (Groq Whisper) 速度優先 / 本地 (whisper.cpp) 隱私優先，可切換
- **多 LLM provider** — Groq / OpenAI / Anthropic / Gemini
- **API key 安全儲存** — Windows Credential Vault（不是 plaintext）
- **歷史記錄** — SQLite 本地保存所有轉錄
- **手動詞彙字典** — 確保專有名詞正確轉錄
- **完全 BYOK** — 無訂閱、無會員制、永遠免費

## 技術架構

```
Tauri v2 (Rust) + Vue 3 + TypeScript
+ shadcn-vue + Tailwind v4
+ cpal (audio) + whisper.cpp (local) + Groq Whisper (cloud)
+ keyring (OS Credential Vault)
+ SQLite (Rust-owned)
```

詳見 [`doc/plans/01-architecture.md`](doc/plans/01-architecture.md)。

## 開發階段

| Phase | 範圍 | 狀態 |
|---|---|---|
| **Phase 1** | OSS MVP（Windows-only、clone+build） | 🚧 規劃完成 |
| **Phase 2** | 完整發行（Windows installer + auto-updater + macOS）| 📋 設計中 |

詳見：
- [`doc/goals/01-phase1-mvp.md`](doc/goals/01-phase1-mvp.md)
- [`doc/goals/02-phase2-release.md`](doc/goals/02-phase2-release.md)

## 文件導覽

| 主題 | 文件 |
|---|---|
| 願景與目標使用者 | [`doc/goals/00-vision.md`](doc/goals/00-vision.md) |
| Phase 1 範圍 | [`doc/goals/01-phase1-mvp.md`](doc/goals/01-phase1-mvp.md) |
| 成功指標 | [`doc/goals/03-success-criteria.md`](doc/goals/03-success-criteria.md) |
| 技術選型 | [`doc/plans/00-tech-stack.md`](doc/plans/00-tech-stack.md) |
| 系統架構 | [`doc/plans/01-architecture.md`](doc/plans/01-architecture.md) |
| 實作 Roadmap | [`doc/plans/02-implementation-roadmap.md`](doc/plans/02-implementation-roadmap.md) |
| Hybrid 轉錄設計 | [`doc/plans/06-hybrid-transcription.md`](doc/plans/06-hybrid-transcription.md) |
| Claude Code 規範 | [`CLAUDE.md`](CLAUDE.md) |
| **SayIt 完整分析（reference）** | [`doc/reference/`](doc/reference/) |

## 與 SayIt 的關係

TalkType 的設計大量參考 **[SayIt](https://github.com/chenjackle45/SayIt)** by Tai-Cheng Chen。SayIt 是 MIT 授權的 Tauri v2 桌面語音輸入工具，主要在 macOS 開發。

TalkType 的差異化：

| 維度 | SayIt | TalkType |
|---|---|---|
| 主要平台 | macOS first | Windows first（Phase 2 加 macOS）|
| 轉錄 | Groq Cloud only | Hybrid Cloud + Local whisper.cpp |
| API key 儲存 | Plaintext JSON | OS Credential Vault |
| SQLite 擁有者 | Frontend (race condition) | Rust（避免 race） |
| 隱私模式 | 無 | 完全本地、不上 cloud |
| Accessibility | 無 ARIA | ARIA + prefers-reduced-motion |

完整 SayIt 分析（17,000+ 字）在 [`doc/reference/`](doc/reference/)。

## 安裝（Phase 2）

🚧 Phase 2 才會提供 installer。Phase 1 期間請參考下方「開發」section clone + build。

## 開發（Phase 1）

### 環境需求

- Windows 11
- Node.js 24（透過 [nvm-windows](https://github.com/coreybutler/nvm-windows) 或 [Volta](https://volta.sh/)）
- pnpm 10+（`corepack enable && corepack prepare pnpm@latest`）
- Rust stable（[rustup.rs](https://rustup.rs/)）
- Visual Studio Build Tools（C++ workload）

### 步驟

```bash
git clone https://github.com/Luluboy168/TalkType.git
cd TalkType
pnpm install
pnpm tauri dev
```

### 設定 API Key（首次使用）

1. 啟動 app
2. 開啟 Dashboard → Settings
3. 在 Whisper / LLM provider 區塊貼上 API key
4. Key 自動存進 Windows Credential Manager（不是 plaintext）

### 取得 API Key

| Provider | 連結 |
|---|---|
| Groq（推薦，免費）| https://console.groq.com/keys |
| OpenAI | https://platform.openai.com/api-keys |
| Anthropic | https://console.anthropic.com/settings/keys |
| Gemini | https://aistudio.google.com/apikey |

## 貢獻

🚧 Phase 1 開發階段歡迎 issue 與 discussion。PR 歡迎但建議先開 issue 討論。

詳見 `CONTRIBUTING.md`（待寫）。

## 路線圖

詳見 [`doc/plans/02-implementation-roadmap.md`](doc/plans/02-implementation-roadmap.md) 的 milestone 規劃：

- M0：Repo bootstrap
- M1：基礎 IPC + 雙視窗
- M2：錄音 pipeline (Rust + cpal)
- M3：Cloud transcription (Groq)
- M4：全域熱鍵 + paste injection（核心 milestone）
- M5：HUD overlay
- M6：LLM polish 多 provider
- M7：Local whisper.cpp 整合
- M8：Dashboard 完整化
- M9：Polish + 發布 v0.1.0

## License

[MIT](LICENSE)

## 致謝

- **[SayIt](https://github.com/chenjackle45/SayIt)** — 直接 inspiration、技術架構 reference
- **[whisper.cpp](https://github.com/ggerganov/whisper.cpp)** — 本地 transcription engine
- **[Tauri](https://tauri.app/)** — Cross-platform desktop framework
- **[Vue 3](https://vuejs.org/)** & **[shadcn-vue](https://www.shadcn-vue.com/)** — Frontend
- **[Groq](https://groq.com/)**、**[OpenAI](https://openai.com/)**、**[Anthropic](https://anthropic.com/)**、**[Google](https://ai.google.dev/)** — LLM providers
