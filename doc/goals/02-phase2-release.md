# Phase 2：完整發行

> **狀態**：Draft v1
> **最後更新**：2026-05-02
> **前提**：Phase 1 完成（見 [`01-phase1-mvp.md`](01-phase1-mvp.md)）
> **目標時程**：Phase 1 之後 1–2 個月

## Phase 2 一句話描述

> 從「需要自己 build 的 OSS 專案」變成「在 GitHub Releases 下載 .exe 一鍵安裝、會自動更新的 Windows app」，並擴展到 macOS。

## 完成定義

Phase 2 結束時：

- [ ] **Windows installer** (.msi 或 .exe via NSIS) 在 GitHub Releases 可下載
- [ ] **Auto-updater** 自動檢查 + 通知 + 一鍵更新
- [ ] **Code signing** — Windows 至少自簽 + SmartScreen 警告處理；理想是 OV/EV cert
- [ ] **Sentry telemetry**（opt-in、預設關）— crash + perf monitoring
- [ ] **macOS support**（intel + arm64）— 學 SayIt 的 dual-target build
- [ ] **macOS code signing + notarization** — Developer ID + notarytool
- [ ] **GitHub Actions release pipeline** — tag → matrix build → draft release → publish
- [ ] **Privacy policy** + **Terms of Service** — 在 README 連結
- [ ] **Audio at rest 加密** — Windows DPAPI / macOS Keychain key
- [ ] **AI smart dictionary** — 從 corrections 自動學習詞彙
- [ ] **進階 UX** — multi-monitor HUD、quality monitor、correction monitor、edit mode（評估後選）
- [ ] **Dashboard 統計圖表** — 用量、節省時間、每日趨勢
- [ ] **完整 i18n** — zh-TW、en、zh-CN、ja、ko 至少
- [ ] **網站 / Landing page**（可選但建議）— 介紹、下載連結

## Phase 2 範圍：新增功能

### A. 發行基礎建設

| 功能 | 細節 |
|---|---|
| Tauri updater plugin | minisign-signed updates、`latest.json` manifest 在 GitHub Releases |
| Code signing pubkey embed | `tauri.conf.json` `plugins.updater.pubkey` |
| GitHub Actions release.yml | matrix（windows-x64、macos-arm64、macos-x64）、tauri-action@v0、stable-name asset upload |
| 4-place version sync 腳本 | `scripts/release.sh`（學 SayIt） |
| `latest.json` 自動產生 | tauri-action 自動處理 |
| Sentry release 整合 | `sayit@<version>` ↔ TalkType 改成 `talktype@<version>`、sourcemap upload only macos-arm64 |

### B. 程式碼簽署

| 平台 | Phase 2 計畫 |
|---|---|
| Windows | **第一階段**：自簽（warning ok）；**第二階段**：申請 SignPath OSS 計畫的免費 OV cert，或購買 EV cert（~$300/年） |
| macOS | Apple Developer Program ($99/年) → Developer ID Application cert + notarytool |

詳細 secrets 與流程在 [`doc/plans/07-phase2-distribution.md`](../plans/07-phase2-distribution.md)。

### C. macOS 擴展

| 功能 | 細節 |
|---|---|
| HUD overlay | NSWindow 設 level=27（同 SayIt 學自 BoringNotch） |
| 全域熱鍵 | CGEventTap on Session level（學 SayIt） |
| Auto paste | CGEvent Cmd+V via Private source（學 SayIt 的 LINE-app fix） |
| Accessibility 權限 | AXIsProcessTrustedWithOptions prompt + 設定 deep link |
| 系統音量 mute | CoreAudio AudioObjectSetPropertyData（學 SayIt） |
| Audio file storage | `~/Library/Application Support/com.luluboy168.talktype/recordings/` |

### D. 進階功能（評估後選做）

| 功能 | 來源 | Phase 2 加？ |
|---|---|---|
| AI smart dictionary（從 corrections 學詞） | SayIt v0.7.0 | ✅ Yes — killer feature |
| Quality monitor（偵測 user 改了沒） | SayIt | ⚠️ 評估 — 需要看 telemetry 用途 |
| Correction monitor（偵測 user 修改後 extract proper nouns） | SayIt | ⚠️ 評估 |
| Multi-monitor HUD positioning | SayIt | ✅ Yes |
| Edit mode（選取文字 → AI 改寫） | SayIt v0.9.0 | ⚠️ 評估 — 安全考量（prompt injection from selected text） |
| Streaming transcription | TalkType 差異化 | ⚠️ 評估 — 看 whisper.cpp 是否支援 |
| Hallucination detection | SayIt | ✅ Yes — 簡單且有用 |
| Dashboard 統計圖表 | SayIt | ✅ Yes — 用 `@unovis/vue` |
| Audio at rest 加密 | TalkType 差異化 | ✅ Yes — 對 SayIt 改進 |
| Privacy mode（完全本地） | TalkType 差異化 | ✅ Yes — 一鍵切到完全離線 |

### E. Sentry telemetry（opt-in）

預設關，使用者明確開啟才送。送的內容：

- ✅ Rust panic / unhandled exception
- ✅ Frontend uncaught error
- ✅ E2E latency metrics（不含 audio 內容、不含 transcribed text）
- ❌ 不送 audio 檔
- ❌ 不送 transcribed text
- ❌ 不送 API keys（明顯）
- ❌ 不送 user identifiable info（沒 user account）

### F. Linux 支援？

**Phase 2 不做 Linux**。理由：

- 全域熱鍵 + paste injection 在 Linux（X11 vs Wayland）很 painful
- SayIt 也沒做 Linux
- Phase 3+ 評估

## Phase 2 vs Phase 1 差異總表

| 項目 | Phase 1 | Phase 2 |
|---|---|---|
| 平台 | Windows-only | Windows + macOS |
| 安裝方式 | Clone + build | Download installer |
| 更新 | Manual `git pull` | Auto-updater |
| 簽名 | 無 | Windows + macOS code-signed + notarized |
| Telemetry | 無 | Sentry (opt-in) |
| HUD states | 4 | 8+（含 morphing、learned、cancelled、mode-switch） |
| AI smart dictionary | 無（手動） | 有（從 corrections 學） |
| Multi-monitor | 不處理 | HUD follow active monitor |
| 統計圖表 | 純文字 list | Charts via `@unovis/vue` |
| Privacy mode | 透過設定切到 local-only | 一鍵 toggle |
| Audio 加密 | 無 | Windows DPAPI / macOS Keychain |
| i18n locales | 2 (zh-TW, en) | 5+ (zh-TW, en, zh-CN, ja, ko) |
| Privacy policy | README 段落 | 獨立文件 + landing page |

## Phase 2 風險

| 風險 | 機率 | 緩解 |
|---|---|---|
| Windows code signing 成本 | 中 | 先自簽 → 申請 SignPath OSS 計畫 → 不行才買 cert |
| macOS Developer ID 需要 Apple ID + $99/年 | 低 | Apple ID 已有，預算花得起 |
| Notarization 流程踩雷 | 中 | 學 SayIt 的 GitHub Actions 設定（已 documented in `doc/reference/sayit-cicd-analysis.md`） |
| Auto-updater minisign key 遺失 | 高 | 用 1Password / Bitwarden 備份；commit 進私人 repo |
| Sentry 隱私疑慮 | 中 | Opt-in、預設關、明確 disclosure |
| macOS WKWebView quirks（CSP、binary IPC） | 中 | SayIt CHANGELOG 記了所有踩過的雷 |
| Linux 用戶來吵 | 低 | README 明確說 Linux not in scope、PR welcome |

## Phase 2 推遲到未來的（Phase 3+）

- Linux 支援
- 行動 app（iOS / Android）
- 多人會議轉錄
- SaaS / cloud sync
- 訂閱服務（不打算做）
- Microsoft Store / Mac App Store 上架
- 企業版（SSO、IT 部署）

## Phase 2 商業模式

**永遠 free + open source**。

- 沒有訂閱
- 沒有 paid tier
- 沒有「pro features」
- BYOK（使用者自己付 LLM 錢）

如果之後想 monetize：

- ☕ GitHub Sponsors（被動接受 donation）
- 🎁 受贈者名單在 README
- 🚫 **不做** ads、tracking、freemium

## Phase 2 結束 = 成品

Phase 2 完成 = TalkType 變成一個真正可以「給朋友用」的產品。從這之後是 maintenance + 社群驅動的 feature requests。

## 下一步

詳細執行計畫：
- 技術選型 → [`doc/plans/00-tech-stack.md`](../plans/00-tech-stack.md)
- 系統架構 → [`doc/plans/01-architecture.md`](../plans/01-architecture.md)
- 實作 roadmap → [`doc/plans/02-implementation-roadmap.md`](../plans/02-implementation-roadmap.md)
- Phase 2 發行細節 → [`doc/plans/07-phase2-distribution.md`](../plans/07-phase2-distribution.md)
