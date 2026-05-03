# TalkType Progress（給 Claude Code session 用）

> **Quick orientation**：開始任何 task 前先讀本檔 + 最新一篇 session log。
> 完成 task / milestone 後 append 一筆到當天的 session log（或新建一個）。

## 現在在哪

- **Phase**：Phase 1 — OSS MVP
- **進度**：M0 ✅ Done（2026-05-02）／ M1 ✅ Done（2026-05-02、PR #3 merged）／ M2 ✅ Done（2026-05-03、PR #4 merged）／ M3 🚧 In progress（起 2026-05-04、plan refinements PR #5 merged）
- **正式 milestone 表**：[doc/plans/02-implementation-roadmap.md](../doc/plans/02-implementation-roadmap.md#進度-dashboard)
- **GitHub**：[Luluboy168/TalkType](https://github.com/Luluboy168/TalkType)、main 已含 M0 + M1（PR #1 + #3 merged）

## 最近的 session

| 日期 | Topic | Outcome |
|---|---|---|
| [2026-05-02](sessions/2026-05-02-m0-bootstrap.md) | M0 Repo bootstrap + memory setup | ✅ M0 done、PR #1 merged、CI green、memory 系統建立 |
| [2026-05-02](sessions/2026-05-02-m1-dual-window.md) | M1 Dual-window IPC + tray + single-instance | ✅ M1 done、3 chunks 落地、static checks all green、Tauri dev build smoke pass |
| [2026-05-03](sessions/2026-05-03-m2-audio-recorder.md) | M2 Audio Recorder Pipeline (cpal + hound + rustfft) | ✅ M2 done、3 chunks 落地、27 cargo tests pass、Settings mic picker + Dashboard record-test card 整合、static checks all green |

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
