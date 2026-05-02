# 成功指標

> **狀態**：Draft v1
> **最後更新**：2026-05-02

明確、可量測的指標來判斷 TalkType 各 phase 是否成功。

## 北極星指標 (North Star)

> **「我自己每天用 TalkType 取代手動打字 ≥ 30 分鐘的場景，且不想退回去用鍵盤」**

如果作者本人都不每天用，這個專案就失敗了。其他指標都是這個的 leading indicator。

## Phase 1 成功指標

### 技術指標（必須達成）

| 指標 | 目標 | 量測方式 |
|---|---|---|
| **E2E 延遲 P50（Cloud）** | < 2.5 秒（按放開鍵 → 文字出現）| 手動 stopwatch + Sentry traces (Phase 2) |
| **E2E 延遲 P50（Local whisper.cpp base）** | < 5 秒 | 同上 |
| **冷啟動時間** | < 2 秒（雙擊 .exe 到 tray icon 出現） | `Get-Process` timestamp |
| **Idle 記憶體佔用** | < 100 MB（兩個視窗都關時）| Task Manager |
| **Recording 記憶體佔用** | < 200 MB | Task Manager |
| **Binary size（不含 whisper 模型）** | < 30 MB | `ls -la talktype.exe` |
| **Vitest unit coverage** | > 60% on `src/lib/` 與 `src/stores/` | `pnpm test:coverage` |
| **Cargo clippy warnings** | 0（with `-D warnings`）| `cargo clippy -- -D warnings` |
| **vue-tsc errors** | 0 | `npx vue-tsc --noEmit` |
| **CI 通過率** | 100% on main | GitHub Actions |
| **CHANGELOG.md** | 維護、every release 有 entry | 人工 review |

### 品質指標（強烈期望）

| 指標 | 目標 |
|---|---|
| Crash rate（自己使用一週）| < 1 次/天 |
| 轉錄正確率（中文 + 英文混合，clean audio） | > 90%（主觀評估） |
| LLM polish 後的可讀性 | 比原始 Whisper 輸出好（主觀，但明顯） |
| 熱鍵響應延遲（按下 → HUD 出現） | < 100ms（user-perceptible 即時） |
| Paste 成功率（在常見 app 如 Word/Slack/Chrome）| > 99% |

### 採用指標（期望但不阻擋發版）

| 指標 | 目標 | 備註 |
|---|---|---|
| GitHub stars | > 50 | Phase 1 結束時，保守 |
| GitHub forks | > 5 | 顯示有人試 build |
| Issues opened by external users | > 3 | 顯示有人在用 |
| Discussions threads | > 1 | Community engagement |

### Definition of Failure（Phase 1）

如果以下任何一個發生，Phase 1 算 fail，要 reset 思路：

- 自己用了一週後不想用、想退回鍵盤
- E2E 延遲 P50 > 5 秒（Cloud 模式）
- Crash 一天 > 5 次
- Paste 在主流 app 上成功率 < 95%

## Phase 2 成功指標

### 發行指標

| 指標 | 目標 |
|---|---|
| GitHub Releases 下載數（首月）| > 200 |
| 安裝失敗回報率 | < 5% |
| Auto-update 成功率 | > 95% |
| Code signing 完成 | Windows + macOS |
| macOS notarization 通過 | ✅ |

### 技術指標（升級後）

| 指標 | Phase 1 目標 | Phase 2 目標 |
|---|---|---|
| E2E 延遲 P50（Cloud） | < 2.5s | < 2s（streaming + Opus 壓縮） |
| Binary size | < 30 MB | < 20 MB（macOS）/ < 25 MB（Windows） |
| Sentry crash-free sessions | n/a | > 99% |
| HUD 顯示延遲 | < 100ms | < 50ms |

### 採用指標

| 指標 | 目標 |
|---|---|
| GitHub stars | > 500（Phase 2 結束 6 個月內）|
| Active monthly users（透過 Sentry, opt-in）| > 100 |
| 留言/感謝/feedback issues | > 20 |
| 出現在開發者社群（Twitter/Reddit/HN）| 至少 1 次 organic mention |

### 品質指標

| 指標 | 目標 |
|---|---|
| 轉錄正確率（diverse audio types）| > 92% |
| Issue 平均回應時間 | < 7 天 |
| Bug fix release cadence | < 4 週 |
| 文件完整度（README + CONTRIBUTING + API docs） | 完整 |

## 反指標（Anti-metrics）

明確 **不** 追求的指標 — 追求這些反而是失敗：

- ❌ DAU / MAU 成長曲線 → 不是 SaaS、不需要 hockey stick
- ❌ 訂閱轉換率 → 沒訂閱
- ❌ 每使用者收益 → BYOK，沒收益
- ❌ Time-in-app（使用時長）→ 我們希望使用者 **少花時間**（vs 鍵盤輸入）
- ❌ Push notification 開啟率 → 不發 push
- ❌ Email open rate → 不發 email
- ❌ App Store ranking → 不上 store

## 個人/社群指標

| 維度 | Phase 1 結束 | Phase 2 結束 |
|---|---|---|
| 自己每天使用次數 | > 10 次 | > 30 次 |
| 推薦給朋友 | 至少 1 個 | 至少 5 個 |
| 在自己的 blog/twitter 寫文章 | 1 篇 | 3+ 篇 |
| 接受 PR | 1 個 | 5+ 個 |
| 累積 issue 解決數 | > 5 | > 20 |

## 量測工具與儀表板

### Phase 1（無 telemetry）

- **GitHub Insights** — stars、forks、traffic、clones
- **GitHub Actions** — CI pass rate、build time
- **手動記錄** — 自己用的延遲體感、bug encounter、會議筆記
- **每週 retro** — 寫 1 段「這週用 TalkType 的感想」進 `doc/weekly-notes/`（待加）

### Phase 2（有 Sentry）

- **Sentry dashboard** — crash rate、performance traces、release health
- **GitHub Releases analytics** — 下載數
- **GitHub Insights**
- 加 **Plausible / Umami**（自架）給 landing page traffic（如果有）

## 重新檢視時機

每個 phase 結束時做一次 retro：

- 哪些指標達成、哪些沒？為什麼？
- 哪些指標其實沒意義、應該換？
- 北極星指標還是對的嗎？

也許三個月後我們發現「streaming transcription」才是真正 differentiator、那目標就要重 align。指標是工具不是目的。

## 連結

- 願景 → [`00-vision.md`](00-vision.md)
- Phase 1 範圍 → [`01-phase1-mvp.md`](01-phase1-mvp.md)
- Phase 2 範圍 → [`02-phase2-release.md`](02-phase2-release.md)
- 實作 roadmap → [`../plans/02-implementation-roadmap.md`](../plans/02-implementation-roadmap.md)
