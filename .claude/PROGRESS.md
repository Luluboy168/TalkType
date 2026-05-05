# TalkType Progress（給 Claude Code session 用）

> **Quick orientation**：開始任何 task 前先讀本檔 + 最新一篇 session log。
> 完成 task / milestone 後 append 一筆到當天的 session log（或新建一個）。

## 現在在哪

- **Phase**：Phase 1 — OSS MVP
- **進度**：M0 ✅ Done（2026-05-02）／ M1 ✅ Done（2026-05-02、PR #3 merged）／ M2 ✅ Done（2026-05-03、PR #4 merged）／ M3 ✅ Done（2026-05-04、plan refinements PR #5 + impl PR #6 merged）／ **M4 ✅ Done**（2026-05-05、acceptance 過、本 worktree branch `claude/m4-hotkey-paste` 等 push + PR）／ M5 📋 Planned next
- **正式 milestone 表**：[doc/plans/02-implementation-roadmap.md](../doc/plans/02-implementation-roadmap.md#進度-dashboard)
- **GitHub**：[Luluboy168/TalkType](https://github.com/Luluboy168/TalkType)、main 已含 M0 + M1 + M2 + M3 完整實作（PR #1 + #3 + #4 + #5 + #6 merged）；M4 在 `claude/m4-hotkey-paste` branch acceptance 通過、user 開 PR 後 merge

## 下個 session 接手 SOP（M5 / M4 PR merge 後）

1. **User 收尾**：在 `claude/m4-hotkey-paste` branch 跑 `git push -u origin claude/m4-hotkey-paste` + `gh pr create` → CI 過 → merge
2. **M4 merge 後 start M5**（HUD overlay 完成、4 個 visual states + ARIA + reduced-motion）
3. **M5 開工前**（同 M4 模式）：plan-time challenger 必跑、讀 SayIt `NotchHud.vue` 861-line 教訓 + `prefers-reduced-motion` 缺口
4. **M5 設計重點**：useVoiceFlowStore 已 emit 4 logical states（idle/recording/transcribing/success/error）、HUD 只負責 visual binding；M5 加 6-bar waveform 接 `audio:waveform` event（M2 已 emit 60fps）、spinner、success ✓ icon、error ✗ icon、auto-hide timers（success 1s、error 3s、recording 持續顯示）；HUD `WS_EX_NOACTIVATE`（M4 chunk 2 已加）保證 paste target 不被搶 focus

## 最近的 session

| 日期 | Topic | Outcome |
|---|---|---|
| [2026-05-02](sessions/2026-05-02-m0-bootstrap.md) | M0 Repo bootstrap + memory setup | ✅ M0 done、PR #1 merged、CI green、memory 系統建立 |
| [2026-05-02](sessions/2026-05-02-m1-dual-window.md) | M1 Dual-window IPC + tray + single-instance | ✅ M1 done、3 chunks 落地、static checks all green、Tauri dev build smoke pass |
| [2026-05-03](sessions/2026-05-03-m2-audio-recorder.md) | M2 Audio Recorder Pipeline (cpal + hound + rustfft) | ✅ M2 done、3 chunks 落地、27 cargo tests pass、Settings mic picker + Dashboard record-test card 整合、static checks all green |
| [2026-05-04](sessions/2026-05-04-m3-cloud-transcription.md) | M3 Cloud Transcription (Groq Whisper + credentials + test connection) | ✅ M3 done、4 chunks 落地（retro 收尾 + credentials + transcription + test+UI+docs）、94 cargo tests pass、Settings 測試連線 + Dashboard 測試轉錄、static checks all green |
| [2026-05-05](sessions/2026-05-05-m4-hotkey-paste.md) | M4 Global Hotkey + Paste (SetWindowsHookExW + arboard STA + AltGr suppression + modifier residue + IME complete + 7-step paste pipeline) | ✅ M4 done、5 chunks + 2 reviewers + paste.rs split + 2 acceptance fixes（issue 1 keystroke suppression + issue 2 v2 parallel transcribes + session counter + paste serialization）；157 cargo tests + 16 vitest pass、acceptance 通過 |

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
