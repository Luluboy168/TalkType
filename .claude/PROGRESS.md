# TalkType Progress（給 Claude Code session 用）

> **Quick orientation**：開始任何 task 前先讀本檔 + 最新一篇 session log。
> 完成 task / milestone 後 append 一筆到當天的 session log（或新建一個）。

## 現在在哪

- **Phase**：Phase 1 — OSS MVP
- **進度**：M0 ✅ Done（2026-05-02）／ M1 ✅ Done（2026-05-02、PR #3 merged）／ M2 ✅ Done（2026-05-03、PR #4 merged）／ M3 ✅ Done（2026-05-04、plan refinements PR #5 + impl PR #6 merged）／ M4 ✅ Done（2026-05-05、acceptance 過、PR #8 merged）／ M5 ✅ Done（2026-05-05、acceptance 過、PR #9 merged）／ **M6 🚧 In progress**（2026-05-06、plan refined、待 user 確認 8 decisions 後 dispatch implementer）
- **正式 milestone 表**：[doc/plans/02-implementation-roadmap.md](../doc/plans/02-implementation-roadmap.md#進度-dashboard)
- **GitHub**：[Luluboy168/TalkType](https://github.com/Luluboy168/TalkType)、main 已含 M0–M5 完整實作（PR #1 + #3 + #4 + #5 + #6 + #8 + #9 merged）

## 下個 session 接手 SOP（M6 chunk 0 dispatch）

1. **讀 [`sessions/2026-05-06-m6-llm-polish-kickoff.md`](sessions/2026-05-06-m6-llm-polish-kickoff.md)** — refined M6 spec、6 chunks、8 decisions、33 fold-in findings (F1-F33)。本檔即 implementer 接下來讀的 source-of-truth、超 plans/02 M6 section
2. **確認 8 decisions resolution**（特別 Decision #7 default polish ON + has_credential gate；Decision #4 Anthropic 404 fallback；Decision #5 success bubble dual-mode warning amber）
3. **dispatch chunk 0 implementer subagent（Opus）** prompt 用 kickoff log chunk 0 section
4. **每 chunk 完成 → reviewer subagent**（Opus、`general-purpose`）prompt 用 kickoff log 對應 chunk Reviewer checklist。**chunk 1 (Rust) reviewer critical**：API key invariant 任何 violation 立即 P0
5. **chunks 3 + 4 reviewer 必跑 Playwright SOP**（CLAUDE.md UI verification rule）
6. **chunk 5 後 dispatch retro challenger**（CLAUDE.md item #6）：focus Privacy / API key invariant / Typeless 隱私翻車反思

## 最近的 session

| 日期 | Topic | Outcome |
|---|---|---|
| [2026-05-02](sessions/2026-05-02-m0-bootstrap.md) | M0 Repo bootstrap + memory setup | ✅ M0 done、PR #1 merged、CI green、memory 系統建立 |
| [2026-05-02](sessions/2026-05-02-m1-dual-window.md) | M1 Dual-window IPC + tray + single-instance | ✅ M1 done、3 chunks 落地、static checks all green、Tauri dev build smoke pass |
| [2026-05-03](sessions/2026-05-03-m2-audio-recorder.md) | M2 Audio Recorder Pipeline (cpal + hound + rustfft) | ✅ M2 done、3 chunks 落地、27 cargo tests pass、Settings mic picker + Dashboard record-test card 整合、static checks all green |
| [2026-05-04](sessions/2026-05-04-m3-cloud-transcription.md) | M3 Cloud Transcription (Groq Whisper + credentials + test connection) | ✅ M3 done、4 chunks 落地（retro 收尾 + credentials + transcription + test+UI+docs）、94 cargo tests pass、Settings 測試連線 + Dashboard 測試轉錄、static checks all green |
| [2026-05-05](sessions/2026-05-05-m4-hotkey-paste.md) | M4 Global Hotkey + Paste (SetWindowsHookExW + arboard STA + AltGr suppression + modifier residue + IME complete + 7-step paste pipeline) | ✅ M4 done、5 chunks + 2 reviewers + paste.rs split + 2 acceptance fixes（issue 1 keystroke suppression + issue 2 v2 parallel transcribes + session counter + paste serialization）；157 cargo tests + 16 vitest pass、acceptance 通過 |
| [2026-05-05](sessions/2026-05-05-m5-hud-overlay.md) | M5 HUD Overlay (4 visual states + 6-bar waveform + ARIA + reduced-motion + active-monitor positioning + Dashboard sidebar badge) | ✅ M5 done、5 chunks + chunk 1 reviewer P0 fix + chunk 2 P1/P2 polish + retro P1 ellipsis fix + 3 acceptance fixes（HUD visible:true + capability set-ignore-cursor-events + user-select:none）；53 vitest + 163 cargo tests pass、acceptance 通過 |
| [2026-05-06](sessions/2026-05-06-m6-llm-polish-kickoff.md) | M6 LLM Polish kickoff — plan-time challenger refine | 📋 Plan refined：6 chunks (5.5d 預估)、8 decisions resolved、Plan-time challenger 45 findings 全 fold（F1-F33 critical 進 chunks、P2 進 IDEAS）。待 user 確認、未動 code |

## Memory file 結構

| File | 用途 |
|---|---|
| [`PROGRESS.md`](PROGRESS.md)（本檔） | Quick orientation entry point |
| [`sessions/YYYY-MM-DD-*.md`](sessions/) | 每個 working session 的詳細紀錄（What changed / Key decisions / Surprises / Follow-ups） |
| [`IDEAS.md`](IDEAS.md) | 不歸屬於當前 milestone 的想法、改進建議 parking lot |

## 接手 session 的 SOP（給 Claude）

1. **Read** 本檔 → 知道在哪
2. **Read** 最近一篇 session log → 知道上次做什麼、現在 follow-ups 是什麼
3. **Skim** [`IDEAS.md`](IDEAS.md) → 看有沒有跟當前 task 相關的想法可以撿回來
4. **Read** [`doc/plans/02-implementation-roadmap.md`](../doc/plans/02-implementation-roadmap.md) 找當前 milestone 的 task list
5. **Start** working

## 結束 session 的 SOP

1. 完成主要 task → append 到當天 `sessions/YYYY-MM-DD-*.md`（含 What changed / Key decisions / Surprises / Follow-ups）
2. 想到非當前 task scope 的點子 → 丟 [`IDEAS.md`](IDEAS.md)
3. Milestone 完成 → 同時更新本檔 + `doc/plans/02-implementation-roadmap.md` dashboard
