# TalkType Progress（給 Claude Code session 用）

> **Quick orientation**：開始任何 task 前先讀本檔 + 最新一篇 session log。
> 完成 task / milestone 後 append 一筆到當天的 session log（或新建一個）。

## 現在在哪

- **Phase**：Phase 1 — OSS MVP
- **進度**：M0 ✅ Done（2026-05-02）／ M1 ✅ Done（2026-05-02、PR #3 merged）／ M2 ✅ Done（2026-05-03、PR #4 merged）／ M3 ✅ Done（2026-05-04、plan refinements PR #5 + impl PR #6 merged）／ M4 📋 Planned next
- **正式 milestone 表**：[doc/plans/02-implementation-roadmap.md](../doc/plans/02-implementation-roadmap.md#進度-dashboard)
- **GitHub**：[Luluboy168/TalkType](https://github.com/Luluboy168/TalkType)、main 已含 M0 + M1 + M2 + M3 完整實作（PR #1 + #3 + #4 + #5 + #6 merged）

## 下個 session 接手 SOP（M4 開工前必做）

1. **Branch 切換**：當前 worktree 在 stale 的 `claude/m3-cloud-transcription` 分支（已 merged）。新 session 開始時：
   ```bash
   git fetch origin
   git checkout -b claude/m4-hotkey-paste origin/main
   ```
   舊 branch 可選刪：`git branch -d claude/m3-cloud-transcription` + `git push origin --delete claude/m3-cloud-transcription`
2. **User 待辦**：rotate Groq API key（chat history 出現過、視為已洩漏） — 進 [console.groq.com/keys](https://console.groq.com/keys) 刪舊生新、Settings 改用新的
3. **Plan-time challenger 必跑**（CLAUDE.md item #5）：M4 開工前同 message 平行 dispatch 一個 challenger 讀 roadmap M4 + reference SayIt hotkey_listener 1566-line 教訓，列出 perf / UX / OS-native edge cases 給主 session
4. **Retro challenger（可選）**：M3 退場前可考慮跑一次、findings 進 IDEAS；但 M3 已有 Q1-Q5 + plan-time challenger + 4 reviewer subagents 重重把關、retro ROI 較低、可省

## 最近的 session

| 日期 | Topic | Outcome |
|---|---|---|
| [2026-05-02](sessions/2026-05-02-m0-bootstrap.md) | M0 Repo bootstrap + memory setup | ✅ M0 done、PR #1 merged、CI green、memory 系統建立 |
| [2026-05-02](sessions/2026-05-02-m1-dual-window.md) | M1 Dual-window IPC + tray + single-instance | ✅ M1 done、3 chunks 落地、static checks all green、Tauri dev build smoke pass |
| [2026-05-03](sessions/2026-05-03-m2-audio-recorder.md) | M2 Audio Recorder Pipeline (cpal + hound + rustfft) | ✅ M2 done、3 chunks 落地、27 cargo tests pass、Settings mic picker + Dashboard record-test card 整合、static checks all green |
| [2026-05-04](sessions/2026-05-04-m3-cloud-transcription.md) | M3 Cloud Transcription (Groq Whisper + credentials + test connection) | ✅ M3 done、4 chunks 落地（retro 收尾 + credentials + transcription + test+UI+docs）、94 cargo tests pass、Settings 測試連線 + Dashboard 測試轉錄、static checks all green |

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
