# TalkType Progress（給 Claude Code session 用）

> **Quick orientation**：開始任何 task 前先讀本檔 + 最新一篇 session log。
> 完成 task / milestone 後 append 一筆到當天的 session log（或新建一個）。

## 現在在哪

- **Phase**：Phase 1 — OSS MVP
- **進度**：M0 ✅ Done（2026-05-02）／ M1 ✅ Done（2026-05-02、PR #3 merged）／ M2 ✅ Done（2026-05-03、PR #4 merged）／ M3 ✅ Done（2026-05-04、plan refinements PR #5 + impl PR #6 merged）／ M4 ✅ Done（2026-05-05、acceptance 過、PR #8 merged）／ M5 ✅ Done（2026-05-05、acceptance 過、PR #9 merged）／ **M6 ✅ Done**（2026-05-06、5 chunks + retro P0 fix + 2 dogfood fixes、281 cargo + 106 vitest pass、acceptance 通過、待 PR）
- **正式 milestone 表**：[doc/plans/02-implementation-roadmap.md](../doc/plans/02-implementation-roadmap.md#進度-dashboard)
- **GitHub**：[Luluboy168/TalkType](https://github.com/Luluboy168/TalkType)、main 已含 M0–M5 完整實作（PR #1 + #3 + #4 + #5 + #6 + #8 + #9 merged）；M6 在 worktree branch `claude/silly-spence-3adbd2`、acceptance 通過、**待 push + PR #10 候選**

## 下個 session 接手 SOP（M6 PR + M7 Local whisper.cpp dispatch）

1. **M6 PR**（如果 PR #10 還沒開）：在 worktree `claude/silly-spence-3adbd2` 跑 `git push -u origin claude/silly-spence-3adbd2` + `gh pr create`、CI 過 → merge 進 main。
2. **讀 [`sessions/2026-05-06-m6-llm-polish.md`](sessions/2026-05-06-m6-llm-polish.md)**（M6 implementation session log）— 5 chunks summary + commit table + key decisions + acceptance addendum + Follow-ups for M7。重點是「Follow-ups for M7」段：4 LLM providers wired but only Groq is also a Whisper provider；M7 加 local Whisper alternative。
3. **讀 [`sessions/2026-05-06-m6-llm-polish-kickoff.md`](sessions/2026-05-06-m6-llm-polish-kickoff.md)**（M6 plan refinement reference）— 8 user-confirmed decisions + 45 challenger findings。
4. **M7 dispatch 前 spike**：whisper.cpp Rust binding 選 `whisper-rs` vs `whisper-cpp-2` vs 直接 FFI、寫 hello-world 跑 base 模型轉錄一個 5 秒 WAV、驗證 Windows build pipeline 不踩雷、量測延遲。pre-chunk 0 commit 1 個 spike PR。
5. **plan-time challenger 必跑**（CLAUDE.md item #5）：focus FFI safety / Windows build pipeline 踩雷 / latency 量測 / cross-platform binding choice / model file 下載 SHA-256 校驗。
6. **每 chunk 完成 → reviewer subagent**（Opus、`general-purpose`）prompt 用 plan 對應 chunk Reviewer checklist。**Rust binding chunk reviewer critical**：FFI safety 任何 panic 立即 P0
7. **chunk N 後 dispatch retro challenger**（CLAUDE.md item #6）：focus offline-first 體驗 / cloud→local fallback UX / cancel-on-shutdown

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
| [2026-05-06](sessions/2026-05-06-m6-llm-polish.md) | M6 LLM Polish 實作（4 free providers + 5 preset modes + tri-state + retry + HUD enhancing visual + Settings UI + upgrade banner） | ✅ M6 done、5 chunks + chunk 5 docs + retro P0 fix（Test polish IPC arg shape）+ 2 dogfood fixes（Gemini deprecated models + custom prompt save/cancel UX）；281 cargo + 106 vitest pass、靜態檢查全綠；user manual acceptance 18/18 通過、待 PR #10 |

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
