# TalkType 產品願景

> **狀態**：Draft v1
> **最後更新**：2026-05-02

## 一句話定位

> 按住一個鍵說話，放開後文字直接貼到游標位置 — 跨平台、Windows 優先、開源的桌面語音輸入工具。

## 為什麼做這個

### 問題

中文使用者在桌面打字面臨幾個 pain points：

1. **CJK 輸入慢** — 注音/倉頡/拼音輸入法平均 30–50 字/分鐘，遠低於語音 150–200 字/分鐘
2. **長文撰寫疲勞** — Email、訊息、報告、會議摘要寫起來累
3. **既有方案不理想**：
   - **作業系統內建語音輸入** — Windows 語音辨識中文支援差、Siri Dictation 只在 Apple 生態內
   - **付費方案**（Otter / Whispr Flow / Wispr）— 訂閱費、隱私疑慮、不開源
   - **SayIt** — 已有的 OSS 解，但 Windows 是二等公民、強制依賴 Groq Cloud、API key plaintext

### 機會

SayIt 的 codebase 已經證明 Tauri v2 + Vue 3 + Rust 能做到 < 3 秒延遲的全域語音輸入。我們站在 SayIt 的肩膀上，做一個：

- **Windows 一等公民** 的版本（SayIt 原作者主要在 macOS 開發）
- **離線優先**（hybrid local whisper.cpp + cloud Groq）
- **真正安全的 secrets 儲存**（OS Credential Vault）
- **更乾淨的內部架構**（Rust 擁有 SQLite、簡化 IPC）

## 目標使用者

### 主要

- **CJK 重度打字使用者**：寫 email/Slack/Notion/Word 文件多的工程師、PM、行銷、文字工作者
- **Windows 桌面為主**（Phase 1 唯一支援）
- **願意申請 API key 或下載本地模型**（即不是 just-works 一鍵安裝的小白使用者）

### 次要

- **隱私敏感者**：希望音頻不外傳的使用者（會走本地 whisper.cpp 路線）
- **多語使用者**：英語、繁中混雜的場景

### 明確排除

- 行動裝置使用者（不做 mobile）
- 需要會議轉錄（多人、長時段）— 這是 Otter / Fireflies 的領域
- 需要即時字幕（live captioning）

## 核心價值主張

| 維度 | 我們的承諾 |
|---|---|
| **延遲** | E2E < 3 秒（Cloud 模式 < 2 秒，Local 模式 < 5 秒，視機器） |
| **隱私** | API key 存 Windows Credential Vault；音頻可選擇全本地處理；首次啟動明確揭露資料流向 |
| **整合** | 在任何 Windows app 都能用（Word、Excel、Slack、瀏覽器、IDE） |
| **品質** | LLM polish 把口語變書面語（去贅詞、修標點、整理句構） |
| **彈性** | Whisper provider 與 LLM provider 都可換（多 provider 支援） |
| **開源** | MIT 授權，不訂閱、不會員制 |

## 跟 SayIt 的關係

TalkType **不是 fork** — 它是 SayIt 的 **精神後繼者**（spiritual successor）：

- **參考**：SayIt 的 Tauri 架構、IPC 契約、Rust 音頻 pipeline、CGEvent paste 技巧
- **改進**：詳見 [`doc/plans/00-tech-stack.md`](../plans/00-tech-stack.md) 與 [`doc/reference/sayit-improvements.md`](../reference/sayit-improvements.md)
- **致謝**：README 與 CHANGELOG 會明確標示 inspired by SayIt
- **授權**：MIT（SayIt 也是 MIT，相容）

## 兩階段路線

| Phase | 範圍 | 預估時程 | 詳細文件 |
|---|---|---|---|
| **Phase 1 OSS MVP** | Windows-only、core flow work、放 GitHub、使用者 clone+build | 1–2 個月 | [`01-phase1-mvp.md`](01-phase1-mvp.md) |
| **Phase 2 完整發行** | Windows installer + auto-updater、code signing、Sentry、後續擴展 macOS | Phase 1 後 1–2 個月 | [`02-phase2-release.md`](02-phase2-release.md) |

## 不做的（Non-Goals）

明確 scope out 來保持專注：

- ❌ **行動 app**（iOS / Android）
- ❌ **多人會議轉錄**（speaker diarization、長時段錄音）
- ❌ **Linux Phase 1 支援**（Phase 2 才考慮）
- ❌ **雲端後端**（沒有 SaaS 訂閱、沒有 server）
- ❌ **App Store / Microsoft Store 上架**（純 GitHub Releases）
- ❌ **內建支付 / 訂閱**（永遠 BYOK，使用者自己付 LLM 費用）
- ❌ **AI 對話式介面**（不是 ChatGPT desktop client）
- ❌ **Windows 11 之前的版本**（Windows 10 也許支援，但 Win7/8 不在 scope）

## 成功指標

詳見 [`03-success-criteria.md`](03-success-criteria.md)。簡述：

- **技術**：E2E 延遲 P50 < 2.5s（Cloud）/ < 4s（Local）、binary < 30MB（不含 whisper 模型）、記憶體 idle < 100MB
- **品質**：Vitest unit coverage > 60%、Rust cargo clippy 0 warnings
- **採用**：Phase 1 結束時 GitHub stars > 50（保守）

## 風險與假設

| 風險 | 嚴重度 | 緩解 |
|---|---|---|
| Tauri v2 學習曲線陡 | 中 | SayIt codebase 是現成 reference；本專案 doc/reference/ 已整理 |
| whisper.cpp Windows binding 不穩 | 高 | 走 Cloud-first，本地當 fallback；模型下載失敗 graceful degradation |
| Groq free tier 政策變動 | 中 | 多 provider 支援（OpenAI/Anthropic/Gemini/local） |
| 全域熱鍵在某些 Windows 版本不穩 | 中 | 學 SayIt 用 SetWindowsHookExW 而不是 RegisterHotKey；早做 multi-instance lock |
| Windows code signing 成本高（Phase 2） | 低 | Phase 1 不簽，Phase 2 考慮 SignPath OSS 計畫或自簽 |

**關鍵假設**：

- Tauri v2 在 Windows 11 上穩定（已驗證 by SayIt v0.9.5）
- whisper.cpp 在 Windows 有可用的 Rust binding（待驗證 — 見 plans/06）
- Groq Cloud 免費 tier 對個人使用足夠（已驗證 by SayIt social proof）

## 引用與致謝

- **SayIt** by Tai-Cheng Chen — https://github.com/chenjackle45/SayIt — 主要 inspiration、技術 reference、許多設計決策的 starting point
- **whisper.cpp** by Georgi Gerganov — 預計用作本地 transcription engine
- **Tauri v2** — 跨平台桌面 app framework
