# M4 全域熱鍵 + Paste — 手動驗收 SOP

> **狀態**：Active v1
> **最後更新**：2026-05-05
> **適用 milestone**：M4（hotkey listener + clipboard paste + voice flow + Settings UI）

## TL;DR

跑 `pnpm tauri dev` 起 dev build、依序通過下列 13 個情境（含原始 6 條 + 7 條 challenger 補強條件）+ 1 個 bonus（RunEvent::Exit unhook）。任何一條失敗 → 在 commit 之前修掉。

## 前置準備

```bash
cd E:/SynologyDrive/Projects/TalkType
pnpm install --frozen-lockfile
pnpm tauri dev
```

預設熱鍵：`Right Alt` + `Hold` mode。第一次跑會在 `app_data_dir/settings.json` 建出預設 settings。

---

## 13 條接受標準

### 1. Notepad — Hold mode

**Setup**：開 Notepad（記事本），把 caret 放進空白文件。
**Steps**：
1. 按住 `Right Alt`、看到 HUD 顯示 recording 狀態。
2. 對麥克風說「你好世界」。
3. 放開 `Right Alt`。

**Expected**：~2-3 s 後，「你好世界」出現在 Notepad 游標位置。HUD 顯示 success 後 1s 內回到 idle。

---

### 2. Word — Hold mode

**Setup**：開 Microsoft Word、新空白文件、caret 在內文。
**Steps**：同條件 1，按住 `Right Alt` → 說話 → 放開。

**Expected**：文字出現在 Word，且 **Word 沒有跳出 Paste Special 對話框**（驗證沒有 modifier residue）。

---

### 3. Slack web — Hold mode

**Setup**：在 Edge 開 Slack web、focus message input。
**Steps**：同條件 1。

**Expected**：文字出現在 Slack message input。**不能** focus 跑掉、也不能 paste 到別的 channel / DM。

---

### 4. Edge URL bar — Hold mode

**Setup**：把 Edge focus 切到 address bar（網址列）。
**Steps**：同條件 1。

**Expected**：文字出現在 address bar，沒有 navigate 到隨機網址。

---

### 5. Toggle mode

**Setup**：
1. 打開 Dashboard → Settings → Hotkey。
2. Trigger Mode 切到 `Tap to start / stop (Toggle)`、按 Save。
3. 等 ✓ Saved 訊息出現。
4. 切回 Notepad。

**Steps**：
1. 按一下 `Right Alt`（不要按住）→ HUD 顯示 recording。
2. 等 1-2 秒、說一段話。
3. 再按一下 `Right Alt`（不要按住）。

**Expected**：第二次按下後，HUD 進 transcribing → success → 文字 paste 到 Notepad。**不需要按住** key。

---

### 6. ESC cancel

**Setup**：保持 Toggle mode 或切回 Hold mode 都可以；focus Notepad。
**Steps**：
1. 按住（或一下，依模式）`Right Alt` 開始錄音。
2. 錄音中、HUD 顯示 recording 狀態時，按 `ESC`。
3. 觀察 HUD + Notepad。

**Expected**：
- HUD 立即回到 idle。
- Notepad **不能**有任何文字 paste 進去。
- 再按熱鍵應該能正常開始下一次錄音。

---

### 7. Hot-swap hotkey

**Setup**：focus Settings → Hotkey 區段。
**Steps**：
1. Trigger Key 切到 `Right Ctrl`、按 Save。
2. 切到 Notepad。
3. 按住 `Right Ctrl` → 說話 → 放開。

**Expected**：
- 用 `Right Ctrl` 觸發成功（recording → paste）。
- **同一輪** 試 `Right Alt` → **沒反應**（不能再觸發）。
- 不需要重啟 app（hot-swap 在 Rust 端 atomically 換掉 listener config）。

---

### 8. Modifier residue（Hold mode）

**Setup**：focus Microsoft Word、把熱鍵切回 `Right Alt` + Hold（如果之前換過）。
**Steps**：
1. 按住 `Right Alt` → 說「測試」→ 放開。
2. 等 paste 完成。
3. 接著手動按 `V` 或 `Ctrl+V`。

**Expected**：
- 「測試」正常 paste 進 Word。
- **後續** 的鍵盤輸入 **不能** 觸發 Word 的 `Alt+Ctrl+V` Paste Special dialog。
- 也就是 paste 的 SendInput 的 `Alt up` 必須 confirm 已 release，沒有 leak `Alt down` 給 OS。

---

### 9. 5x burst（無錄音 spam）

**Setup**：focus Notepad（或任何 input 都可）。
**Steps**：
1. 在 1 秒內快速 tap `Right Alt` 5 次（不要按住、純按下放開連續 5 次）。
2. 看 HUD + log。
3. 第 6 次正常按住 + 說話 + 放開。

**Expected**：
- App **不能**當（沒有 panic、沒有 crash dialog）。
- HUD 沒有卡在錯誤狀態。
- 第 6 次正常 trigger 並完成 paste。
- 沒有 leaked recording state（看 dev console 沒有 `start_recording: Busy` 在閒置時候出現）。

---

### 10. Trigger key swap during press

**Setup**：focus Notepad、Hotkey = `Right Alt` + Hold。
**Steps**：
1. 按住 `Right Alt`（持續按住、不要放）。
2. **手** 不放、用滑鼠切到 Dashboard → Settings → Hotkey。
3. 把 Trigger Key 改成 `Right Ctrl`、按 Save（手依然按著 `Right Alt`）。
4. 鬆開 `Right Alt`。

**Expected**：
- 整個過程中 **鍵盤不能凍結**（如果 `apply_config` 在 hot path 拿 Mutex 會凍住整個 OS keyboard）。
- App 不 crash。
- 鬆開後 HUD 應該回到 idle（如果切換時保留了原 recording state，也應 stop+transcribe；如果 reset 也 OK，重點是 **不 crash + 不凍鍵盤**）。
- 再按 `Right Ctrl` 應該可以正常觸發。

---

### 11. EU keyboard layout（AltGr）

**Setup**：
1. Windows 設定 → Time & Language → Language & region → 加 `German (Germany)` 鍵盤。
2. 用工作列右下角的語言切換（或 `Win + Space`）切到 German layout。
3. focus Notepad。

**Steps**：
1. 按 `AltGr + E`（即 `Right Alt + E`）→ 應該打出 `€`。
2. 觀察 HUD。

**Expected**：
- `€` 字元正常輸入到 Notepad。
- **HUD 不能**啟動 recording（AltGr 在 Windows 是 `Ctrl + Right Alt` 的 synthetic combo；listener 必須 detect 並 suppress）。
- 之後按單獨 `Right Alt` 仍然可以正常觸發 recording（驗證 suppression 只針對 AltGr 而不是把 Right Alt 全 ban）。

切回原本 layout（English / 繁中）以免影響後續測試。

---

### 12. IME composition

**Setup**：
1. 切到 Bopomofo（注音）或 倉頡 IME。
2. focus Notepad。

**Steps**：
1. 開始 type composition：例如注音打 `ㄉ ㄚ`、字還沒 commit（候選字框還在）。
2. **不 commit**、直接按住 `Right Alt` → 說「你好」→ 放開。
3. 觀察 paste 結果。

**Expected**：
- 「你好」paste 進去。
- Paste 結果 **不能**包含先前 composition 的 leftover glyph（例如不能變成「ㄉㄚ你好」或「打你好」）。
- IME 應該在 paste 前被 cancel / commit / clear。

---

### 13. UAC elevated target

**Setup**：以管理員身份開 Task Manager（用 `Ctrl+Shift+Esc` 或右鍵 → 以系統管理員執行）。
**Steps**：
1. focus Task Manager 的 Search / 過濾欄位。
2. 按住 `Right Alt` → 說一段話 → 放開。
3. 觀察 HUD + Task Manager。

**Expected**：
- App **不能** crash（UIPI 阻擋 SendInput 是已知 Windows 限制、TalkType 必須 graceful handle）。
- HUD 顯示「請手動 Ctrl+V」提示（或同義訊息），不是空白 success。
- 文字應該還在 clipboard，使用者手動 `Ctrl+V` 可以 paste 進 Task Manager。

---

## Bonus — RunEvent::Exit unhook

**Setup**：app 跑 `pnpm tauri dev` 起來、確認 tray icon 出現。
**Steps**：
1. 對 tray icon 右鍵 → Quit / 退出（或主視窗 X 並從 tray 退出）。
2. 開 Process Explorer 或 Task Manager → Details tab。
3. 搜 `talktype.exe`。

**Expected**：
- **沒有** `talktype.exe` 殘留 process / thread。
- 如果有殘留 → Windows hook thread 沒被 RunEvent::Exit 卸載。

---

## 驗收完成後

1. 把 13 條（+ bonus）一一 ✅ 在 PR description 或 session log。
2. 任何一條 ❌ → 開 issue / 補修 commit。
3. 全綠 → 在 `doc/plans/02-implementation-roadmap.md` 把 M4 acceptance criteria 從 `📋 Planned` 改成 `✅ Done`、bump 「最後更新」。

---

## 已知 Phase 2 deferred 項目（不在驗收範圍）

- Custom keycode picker UI（`start_hotkey_recording` / `cancel_hotkey_recording` 是 stub）
- Combo（modifier + key）熱鍵
- macOS 對應實作（CGEventTap）
- HUD 4-state visual（M5）
- 1000 ms long-press detector（M5）
