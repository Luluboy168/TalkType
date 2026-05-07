# M6 LLM Polish 多 Provider — 手動驗收 SOP

> **狀態**：Active v1
> **最後更新**：2026-05-06
> **適用 milestone**：M6（LLM polish 多 provider — 4 free providers + 5 preset modes + tri-state polish_enabled + retry toggle + polish failure fallback + M5→M6 upgrade banner + per-step data-flow indicator + HUD enhancing visual + Dashboard sidebar enhancing badge）
> **Spec / Plan**：[`.claude/sessions/2026-05-06-m6-llm-polish-kickoff.md`](../.claude/sessions/2026-05-06-m6-llm-polish-kickoff.md)（plan refined commit `13fb76d`、roadmap pointer `b68026d`、user-confirmed 8 decisions）
> **Implementation session log**：[`.claude/sessions/2026-05-06-m6-llm-polish.md`](../.claude/sessions/2026-05-06-m6-llm-polish.md)

## TL;DR

跑 `pnpm tauri dev` 起 dev build、依序通過下列 18 個情境（含 4 個 free provider 切換 + 5 個 preset modes + tri-state polish_enabled 三個分支 + retry toggle on/off + 失敗 fallback amber bubble + 升級 banner + 資料流 indicator + HUD enhancing visual + Dashboard sidebar enhancing badge + ESC during enhancing no-op + API key invariant 驗證）。任何一條失敗 → 在 commit 之前修掉。

## 自動驗證摘要（all green required）

- `cargo test --lib` → **281 cargo tests pass**（M5 baseline 163 + chunk 1 llm_polish module +118 = 281）
- `vitest run` → **99 vitest pass**（M5 baseline 53 + chunk 0 llm-types +5 + chunk 2 voice flow polish +14 + chunk 3 HudOverlay/HudFlowBadge/aria-wording +14 + chunk 4 SettingsLlmPolishSection/providers/ProviderPrivacyDialog +13 = 99）
- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean

## 前置準備

```bash
cd E:/SynologyDrive/Projects/TalkType
pnpm install --frozen-lockfile
pnpm tauri dev
```

預設熱鍵：`Right Alt` + `Hold` mode（M4 已驗收）；HUD 在 `voice flow status === 'idle'` 時隱藏 bubble、其他狀態 visible（M5 已驗收）。

### 4 個 free provider 的 API key 來源

驗收期間建議準備 4 個 API key：

- **Groq**：[console.groq.com/keys](https://console.groq.com/keys) — 免費 tier 可用
- **OpenRouter**：[openrouter.ai/keys](https://openrouter.ai/keys) — 註冊送 free credits、`:free` 模型完全免費
- **NVIDIA NIM**：[build.nvidia.com](https://build.nvidia.com) — 1000 free credits / 月
- **Gemini**：[aistudio.google.com/apikey](https://aistudio.google.com/apikey) — 免費 tier

最少準備 1 個（Groq 推薦、M3 Whisper 已用）即可跑大部分 acceptance；4 provider 切換驗收（條件 8、10、11、12）需要全部 4 個。

---

## 18 條接受標準

### 1. Polish ON + Groq key works — enhancing transition fires + polished text pasted

**Setup**：
1. 開 Notepad、focus 文件區。
2. Settings → API Keys → Groq → 設好有效 key、點「測試連線」確認綠 ✅。
3. Settings → LLM Polish → toggle 啟用（`Some(true)`）+ provider = Groq + preset = default。

**Steps**：
1. 按住 `Right Alt`、對麥克風說「這個 呃 就是我覺得啊」、放開。
2. 觀察 HUD bubble 與最終 paste 文字。

**Expected**：
- HUD 進場順序：`recording`（紅 dot + 6-bar waveform）→ `transcribing`（spinner + 「轉錄中…」）→ **`enhancing`（spinner + 「優化中…」、amber tone 區別）** → `success`（綠 ✓ + 「完成」）→ idle。
- Notepad 出現 polished 中文文字（不是「這個 呃 就是我覺得啊」原始 raw）；典型輸出：「我覺得這個還可以」之類去贅詞 + 修標點。
- 沒有 amber warning bubble、沒有 console error。

**對應 chunk**：chunk 1 (Rust polish_text) + chunk 2 (voice flow polish branch) + chunk 3 (HUD enhancing visual)
**對應 finding**：F1（status union extend `'enhancing'`）+ F22（tri-state Some(true)）+ F26（HUD enhancing visual）

---

### 2. Polish OFF skips polish entirely — no enhancing transition + raw text pasted

**Setup**：
1. 保持條件 1 的 API key 設定。
2. Settings → LLM Polish → toggle 關閉（`Some(false)`）。
3. 開 Dashboard 的 devtools（F12）、console 開好觀察 invoke 紀錄。

**Steps**：
1. 在 devtools console 跑：
   ```javascript
   const origInvoke = window.__TAURI_INTERNALS__.invoke;
   window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
     console.log('[invoke]', cmd, args);
     return origInvoke(cmd, args);
   };
   ```
2. focus Notepad、按住 `Right Alt` 說「這個就是」、放開。
3. 觀察 HUD bubble + console invoke log。

**Expected**：
- HUD 進場順序：`recording` → `transcribing` → `success`（**沒有 enhancing**、不顯示 amber spinner）。
- Notepad 出現 raw Whisper text 「這個就是」（無 polish 修飾）。
- console **沒有** `[invoke] polish_text ...` log（polish 路徑完全跳過）。
- 沒有 amber warning bubble。

**對應 chunk**：chunk 2 (voice flow polish branch tri-state Some(false))
**對應 finding**：F22 polish ON 路徑 vs 直接 success

---

### 3. Polish auto-detect (None) silently skips when no LLM key

**Setup**：
1. 把所有 LLM provider API key 都刪除：Settings → API Keys → 對 Groq / OpenRouter / NVIDIA / Gemini 各按「刪除」。
2. Settings → LLM Polish → 把 toggle reset 成 default `None`（先打開再關閉再打開、再用 devtools 把 `llmPolishEnabled` 強制設 `null`，或直接 `delete %APPDATA%\com.luluboy168.talktype\settings.json` 重啟、UI toggle 視覺顯示 OFF — 因 None auto-detect 無 key → 預設 OFF）。

**Steps**：
1. 確認 Settings → LLM Polish toggle 視覺上 **OFF**（auto-detect 無 key）。
2. 觀察是否有黃色「未設定 [provider] API key…」warning banner — 應 **沒有**（因 user 沒明確 enable polish）。
3. focus Notepad、按住 `Right Alt` 說「測試」、放開。
4. 觀察 HUD bubble + paste 結果。

**Expected**：
- HUD：`recording` → `transcribing` → `success`（no enhancing，等同條件 2 polish OFF）。
- Notepad 出現 raw Whisper text。
- 沒有 amber warning bubble、沒有 yellow no-key banner。
- M5→M6 trust-transitive：user 沒 explicit 啟用 polish + 也沒任何 LLM key → 等同 polish OFF、frictionless 體驗。

**對應 chunk**：chunk 0 (settings tri-state default None) + chunk 2 (voice flow None auto-detect)
**對應 finding**：F22 None + hasKeyAtStart=false → shouldPolish=false

---

### 4. Polish auto-detect (None) auto-fires when has Groq key — M5 trust-transitive

**Setup**：
1. Settings → API Keys → Groq → 設好有效 key（其他 LLM provider 仍清空）。
2. Settings → LLM Polish → 把 `llmPolishEnabled` 設成 default `None`（同條件 3 步驟 2、或 reset Settings.json）。
3. UI toggle 視覺顯示 **ON**（因 None auto-detect + Groq has_credential = true）。

**Steps**：
1. 確認 Settings → LLM Polish toggle 視覺 **ON**、provider 顯示 Groq。
2. focus Notepad、按住 `Right Alt` 說「呃這個我覺得不錯」、放開。
3. 觀察 HUD bubble + paste 結果。

**Expected**：
- HUD 進場順序同條件 1：recording → transcribing → **enhancing** → success。
- Notepad 出現 polished 文字（典型「我覺得這個不錯」之類）。
- 沒有 amber warning（polish 走通）。
- 此即 M5→M6 trust-transitive 設計：user 已給 Groq Whisper key、polish 用同 vendor chat completions 自動延伸 trust、無需 user 重新 explicit 啟用。

**對應 chunk**：chunk 0 + chunk 2 (voice flow None auto-detect + has_credential gate)
**對應 finding**：F22 + Decision #7 tri-state None auto-detect

---

### 5. Polish failure → amber warning success bubble + raw text pasted

**Setup**：
1. Settings → API Keys → Groq → 設一個**錯誤** key（例：`gsk_invalid_key_test_12345`）、儲存（**不**點測試連線、避免 user 看到 401 錯誤、直接讓 polish_text 撞 401）。
2. Settings → LLM Polish → toggle ON + provider Groq。

**Steps**：
1. focus Notepad、按住 `Right Alt` 說「測試錯誤 key」、放開。
2. 觀察 HUD bubble。

**Expected**：
- HUD 進場：recording → transcribing → enhancing（spinner 短暫出現）→ success-warning（**amber `AlertTriangle` icon + 「優化失敗、已貼上原始轉錄」訊息**、不是綠 ✓）。
- 1500 ms 後 fade idle（M6 SUCCESS_LINGER_MS = 1500、Decision #8）。
- Notepad 出現 raw Whisper text「測試錯誤 key」（不阻擋 paste）。
- 沒有 error state（polish 失敗不切到 error、是 success-warning bubble）。
- console 應有 `polish:failed-fallback { reason: 'auth' }` event log（chunk 2 emit）。

**截圖參考**：[`m6-hud-success-warning.png`](screenshots/m6/m6-hud-success-warning.png)

**對應 chunk**：chunk 2 (polish 失敗 fallback to raw + polishWarning) + chunk 3 (HUD success bubble dual-mode)
**對應 finding**：F22 polish failure → fallback raw + Decision #5 success bubble dual-mode amber

---

### 6. Retry toggle ON + transient error retries once

**Setup**：
1. Settings → API Keys → Groq → 設**有效** key（先確認測試連線綠 ✅）。
2. Settings → LLM Polish → toggle ON + provider Groq + **retry toggle ON**（default ON、tri-state None=ON）。
3. Open dev tools、 console 開 invoke 監聽（同條件 2 步驟 1）。

**Steps**：
1. **準備**：把 Wi-Fi / Ethernet 暫時拔掉（airplane mode 或 disconnect Ethernet cable）— 模擬 transient network error。
2. focus Notepad、按住 `Right Alt` 說「測試 retry」、放開。
3. 觀察 console invoke log + HUD。

**Expected**：
- console 應顯示**至少 2 次** `[invoke] polish_text { rawText: ..., attempt: 1 }` 與 `[invoke] polish_text { rawText: ..., attempt: 2 }`（chunk 2 retry-same logic、attempt 1 fail → attempt 2 retry）。
- 若兩次都失敗（network 仍斷）：HUD success-warning amber bubble + 貼 raw（同條件 5）。
- 若中間恢復網路：第二次成功、polished text 貼上、HUD 綠 ✓。

**Notes**：可改用 `mockServer` 模擬第一次 500 + 第二次 200（更可控）；簡單 manual 用 airplane mode toggle。

**對應 chunk**：chunk 2 (voice flow retry-same logic via attempt param)
**對應 finding**：F34 retry policy + Decision #5 retry toggle ON

---

### 7. Retry toggle OFF + transient error single attempt

**Setup**：
1. 同條件 6 的 Groq + polish ON 設定。
2. Settings → LLM Polish → **retry toggle OFF**（明確 `Some(false)`）。

**Steps**：
1. 同條件 6：Wi-Fi 拔掉、focus Notepad、按 hotkey 錄音。
2. 觀察 console invoke log + HUD。

**Expected**：
- console 應顯示**僅 1 次** `[invoke] polish_text { ..., attempt: 1 }`（**沒有** attempt 2）。
- HUD 直接走 success-warning amber bubble（fallback to raw、不重試）。
- Notepad 出現 raw Whisper text。

**對應 chunk**：chunk 2 (retry toggle OFF 短路 retry)
**對應 finding**：F34 retry-disabled 路徑

---

### 8. 5 preset modes produce visibly different outputs

**Setup**：
1. Groq key + polish ON + retry ON。
2. 準備同一段測試錄音腳本：「我覺得這個還可以、可能要再改一下」（中文混雜口語）。

**Steps**：
1. 對 5 種 preset 各跑一次 polish：
   1. **Default**：preset = `default`、按 hotkey 錄音 → paste 結果記下。
   2. **Email**：切 preset = `email`、同腳本 → paste 結果。
   3. **Chat**：切 preset = `chat`、同腳本 → paste 結果。
   4. **Code**：切 preset = `code`、同腳本 → paste 結果。
   5. **Custom**：切 preset = `custom`、custom prompt 設「請把口語整理成簡潔書面語」、同腳本 → paste 結果。

**Expected**：
- 5 個 paste 結果**質感顯著不同**（手動視覺判斷）：
  - Default：輕度清理、保留口語特徵（「我覺得這個還可以，可能要再改一下」）。
  - Email：正式書面、加敬語可能（「這個版本可行，建議再進行修改」）。
  - Chat：保留 casual、短句（「這個還行，可能要改一下」）。
  - Code：保留原話、不加標點到 code 中（口語腳本 code preset 應 close to default）。
  - Custom：依 user prompt 簡潔書面（「此版本可行，需略作修改」）。
- 沒有 preset 都產出**完全相同**字串（若有 → preset 沒生效、bug）。

**對應 chunk**：chunk 1 (Rust prompts.rs 5 preset system_prompt strings)
**對應 finding**：F12 5 preset golden snapshot tests + Decision #2 PromptMode unit enum

---

### 9. Custom prompt 1001 chars rejected

**Setup**：
1. polish ON + provider Groq + preset = `custom`。
2. Settings → LLM Polish → 找 Custom prompt textarea。

**Steps**：
1. 把 1001 chars 內容貼進 textarea（可用 `'a'.repeat(1001)` console 產生）。
2. 觀察 textarea 下方的 char count 顯示。
3. 嘗試 save settings 或觸發 polish。

**Expected**：
- char count 顯示 `1001 / 1000` 或類似 destructive 紅色。
- save settings 不生效（或 textarea reject）。
- 即使 settings 保留 1001 chars value、invoke `polish_text` 應回 `InvalidPromptLength { actual: 1001, max: 1000 }` error → fallback to raw amber bubble。
- 1000 zh-TW chars (3000 bytes) 應**接受**（因 `chars().count()` 不是 `len()` bytes、F11）。

**對應 chunk**：chunk 1 (Rust prompts.rs validate_custom_prompt + InvalidPromptLength) + chunk 4 (UI char count)
**對應 finding**：F11 custom prompt validation + chunk 4 destructive char count

---

### 10. Test polish button works for 4 free providers

**Setup**：
1. 4 個 LLM provider 都先設好 valid key（4 個都測試連線綠）。
2. Settings → LLM Polish → polish ON。

**Steps**（對 4 個 provider 各跑一次）：
1. provider Select 切 Groq → click 「測試 Polish」button。
2. 同上切 OpenRouter、NVIDIA、Gemini。
3. 觀察按鈕下方 inline diff 區塊。

**Expected**（每個 provider）：
- 按下 button 後 ~2-15s 內 inline 顯示 before/after diff：
  - **Before**：「這個 呃 就是我覺得啊」（zh-TW i18n testSample、F30）/ en「Um, like, I think this is, you know, alright」。
  - **After**：polished 文字（依 preset + provider 不同）。
- diff 不消失、user 可比較。
- **不走 retry**（Decision #6 user 要 immediate feedback、retry 混淆 error 來源）— 失敗時直接顯示 error 訊息、不重試。

**截圖參考**：[`m6-settings-llm-polish-on-has-key.png`](screenshots/m6/m6-settings-llm-polish-on-has-key.png)

**對應 chunk**：chunk 4 (SettingsLlmPolishSection test polish button)
**對應 finding**：F30 test sample i18n + Decision #6 不走 retry

---

### 11. Test connection 4 providers all green

**Setup**：4 個 provider 都設好 valid key。

**Steps**：
1. Settings → API Keys → 對 4 個 provider 各 click 「測試連線」button：
   1. Groq → 應 ✅ + model count（如 7 個 models）。
   2. OpenRouter → 應 ✅ + model count（large list、如 200+ models）+ 確認 request 走 `https://openrouter.ai/api/v1/models` + Bearer auth + `HTTP-Referer: https://github.com/Luluboy168/TalkType` + `X-Title: TalkType` headers（F13 wiremock 已 cover、real run 用 devtools network panel 確認）。
   3. NVIDIA NIM → 應 ✅ + model count + 走 `https://integrate.api.nvidia.com/v1/models` + Bearer auth。
   4. Gemini → 應 ✅ + model count + 走 `https://generativelanguage.googleapis.com/v1beta/models` + `x-goog-api-key` header（**禁** `?key=` query string、F5 critical）。

**Expected**：
- 4 個 provider 全綠 ✅。
- devtools network panel：4 個 request 都看到正確 URL + 正確 auth header（特別 Gemini 的 URL 不含 `key=` query parameter、整段 query string 應 empty）。

**對應 chunk**：chunk 1 (Rust llm_polish/health.rs test_*_connection 3 個 + transcription/health.rs Groq M3 reuse)
**對應 finding**：F5 test endpoint contract + F13 per-provider header validation

---

### 12. Per-step data-flow indicator updates reactively

**Setup**：4 個 LLM provider 都 valid key + polish ON。

**Steps**：
1. Settings → LLM Polish → 找頂部 data-flow indicator（F29、polish ON 時顯示 5-step flow）。
2. provider Select 切 Groq → 觀察 indicator text。
3. 同上切 OpenRouter / NVIDIA / Gemini。

**Expected**：
- Indicator text 隨 provider 切換 reactive 更新：
  - Groq：`Audio → Groq Whisper → Groq Polish → Paste（無 retention）`
  - OpenRouter：`Audio → Groq Whisper → OpenRouter Polish → Paste（無 retention）`
  - NVIDIA：`Audio → Groq Whisper → NVIDIA NIM Polish → Paste（無 retention）`
  - Gemini：`Audio → Groq Whisper → Gemini Polish → Paste（無 retention）`
- polish OFF 時 indicator 改顯示 `Audio → Groq Whisper → Paste` 不含 polish step（F29、polish OFF 也 render）。

**截圖參考**：[`m6-settings-dataflow-groq.png`](screenshots/m6/m6-settings-dataflow-groq.png)、[`m6-settings-dataflow-openrouter.png`](screenshots/m6/m6-settings-dataflow-openrouter.png)

**對應 chunk**：chunk 4 (SettingsLlmPolishSection per-step data-flow indicator)
**對應 finding**：F29 dataflow indicator polish ON / OFF / per-provider reactive

---

### 13. No-key warning banner shows when polish ON + no key

**Setup**：
1. 把 Groq key 刪除。
2. Settings → LLM Polish → 明確 set polish ON（`Some(true)`、不是 None auto-detect）+ provider = Groq。

**Steps**：
1. 觀察 LlmPolishSection 內是否顯示黃色 warning banner。

**Expected**：
- 顯示黃色 banner：「未設定 Groq API key、潤飾路徑會跳過」（zh-TW）/ "Groq API key not set, polish path will be skipped"（en）。
- 切 provider 到 OpenRouter（無 key）→ banner 改顯示 OpenRouter 對應訊息。
- 設好 key 後（測試連線綠）→ banner 自動消失。

**截圖參考**：[`m6-settings-llm-polish-on-no-key.png`](screenshots/m6/m6-settings-llm-polish-on-no-key.png)

**對應 chunk**：chunk 4 (no-key warning banner)
**對應 finding**：F29 no-key warning + F22 explicit Some(true) + no key → silent fallback

---

### 14. M5→M6 upgrade banner shows on first launch + dismissable

**Setup**：
1. Open Dashboard devtools (F12)、console。
2. 跑 `localStorage.removeItem('talktype:m6_upgrade_seen')`（清除 first-load flag）。
3. Reload Dashboard（Ctrl+R）。

**Steps**：
1. Reload 後切到 Settings → LLM Polish section。
2. 觀察是否顯示 dismiss-able warning banner。
3. Click「dismiss / 知道了」button。
4. Reload Dashboard。
5. 再切 Settings → LLM Polish。

**Expected**：
- Step 2：banner 顯示 M5→M6 升級警語（F35）：
  ```
  ⚠️ 升級到 M6 LLM Polish

  M6 加入 LLM Polish 功能、預設行為「auto-detect」：
  - 你已設過 Groq API key (M3 Whisper)：因 Groq 同把 key 也支援 LLM、polish 預設啟用、轉錄文字會送進 Groq chat completions endpoint 加工。
  - 你還沒設任何 LLM provider key：polish 自動 OFF、原樣轉錄行為不變。

  如不希望 polish 啟用、進 Settings → LLM Polish → toggle 關閉。
  完整資料流請看下方 per-step data-flow indicator。
  ```
- Step 3：click 後 banner fade 消失。
- console：`localStorage.getItem('talktype:m6_upgrade_seen')` should return `'true'`。
- Step 5：banner **沒有再顯示**（user dismiss 過、永久不再 show）。

**截圖參考**：[`m6-settings-upgrade-banner-first-show.png`](screenshots/m6/m6-settings-upgrade-banner-first-show.png)

**對應 chunk**：chunk 4 (upgrade banner + localStorage flag)
**對應 finding**：F35 + Decision #7 + Decision #3 cascade（4 free providers）

---

### 15. HUD enhancing visual

**Setup**：條件 1 全套（Groq key + polish ON + 預設 settings）。

**Steps**：
1. focus Notepad、按住 `Right Alt`、說 5-7 秒清晰中文「測試我覺得這個應該還可以」、放開。
2. **重點觀察 HUD enhancing state**（在 transcribing 結束後 ~1-3 秒視窗）。

**Expected**：
- HUD 經過 5 個 visual states（M5 4 個 + M6 1 個 enhancing）：
  1. `recording`（紅 dot + 6-bar waveform + timer）
  2. `transcribing`（spinner + 「轉錄中…」）
  3. **`enhancing`（spinner + 「優化中…」、F26 amber tone 區別）** — M6 新加。
  4. `success`（綠 ✓ + 「完成」）
  5. `idle`（fade out）
- 5 個 state 視覺都明確區分（amber spinner vs zinc spinner、紅 dot vs amber dot 等）。
- click-through 在 enhancing state ON（同 transcribing、F27）、user 點 HUD 不能 dismiss。
- ARIA：SR 念出 `hud.aria.transcribing` 與 `hud.aria.enhancing` wording **顯著不同**（F28 strip non-letter chars + CJK 後 `notEqual`）。

**截圖參考**：[`m6-hud-enhancing.png`](screenshots/m6/m6-hud-enhancing.png)

**對應 chunk**：chunk 3 (HudOverlay enhancing branch + spinner reuse)
**對應 finding**：F1 status union + F26 enhancing visual + F27 click-through ON + F28 ARIA wording differ

---

### 16. Dashboard sidebar enhancing badge

**Setup**：
1. 開 Dashboard（tray icon click、確保視窗 visible）。
2. focus 別的 app（如 Notepad）使 HUD bubble 不擋 Dashboard sidebar。
3. polish ON + Groq key + 條件 1 設定。

**Steps**：
1. 同條件 15 流程：按熱鍵說中文 5-7 秒、放開。
2. 觀察 Dashboard sidebar footer badge state（M5 已加 recording badge、M6 加 enhancing badge）。

**Expected**：
- Recording 期間：sidebar footer 紅 pulse dot + 「錄音中」（M5 既有）。
- Transcribing 期間：badge 視 chunk 3 implementation 可能 hidden 或顯示 transcribing state（chunk 3 reviewer P2-3 說 dashboard sidebar hidden in transcribing/success states、M9 dogfood revisit）。
- **Enhancing 期間：sidebar footer amber dot + 「優化中」**（M6 新、Decision #5 cascade）。
- success linger 1500 ms 後 badge hidden（M5 既有、M6 SUCCESS_LINGER_MS 1500）。

**截圖參考**：[`m6-dashboard-sidebar-enhancing.png`](screenshots/m6/m6-dashboard-sidebar-enhancing.png)

**對應 chunk**：chunk 3 (HudFlowBadge enhancing badge)
**對應 finding**：F26 cascade to Dashboard sidebar

---

### 17. ESC during enhancing is no-op (M6 limitation)

**Setup**：條件 15 全套。

**Steps**：
1. 按熱鍵說 5-7 秒、放開（進 transcribing）。
2. 等 transcribing → enhancing transition（spinner 變 amber tone、約 1-3 秒視窗）。
3. **enhancing 期間** 按 `Esc` key。
4. 觀察 HUD + console。

**Expected**：
- HUD status 仍 stay enhancing（**不**切到 idle、ESC 不 cancel polish、F23 explicit M6 限制）。
- console 應有 `console.warn('[voice-flow] ESC during enhancing is no-op (M6 limitation)')` 或類似警語（chunk 2 console.warn）。
- polish 完成後正常進 success 狀態 + paste polished text。

**Notes**：
- M6 不投資 cancel channel（Decision F23 explicit no-op）。
- Phase 2 預定加 `cancel_polish` Tauri command + `tokio::select!` cancel channel。
- ESC during recording 仍正常 cancel（M4 既有、M6 不變）。

**對應 chunk**：chunk 2 (ESC during enhancing console.warn no-op)
**對應 finding**：F23 explicit ESC during enhancing 不 cancel polish

---

### 18. API key invariant verification — frontend cannot read keys

**Setup**：
1. Groq + OpenRouter + NVIDIA + Gemini 4 個 provider 都設好 key。
2. 開 Dashboard devtools (F12) → Console。

**Steps**：
1. 在 console 跑：
   ```javascript
   await window.__TAURI_INTERNALS__.invoke('get_credential', { provider: 'groq' });
   ```
2. 觀察 error 輸出。
3. 同上對 openrouter / nvidia / gemini 各試一次。

**Expected**：
- Step 2：return `Error: command not found` 或 `Error: unauthorized` 或 `IPC command 'get_credential' was not found`（Rust 不 register 此 command 進 IPC、`generate_handler!` 只 expose `set_credential` / `delete_credential` / `has_credential` / `get_credential_preview`）。
- 4 個 provider 跑下來都同樣 error（不 leak any key value to frontend、F18 keep `pub(crate) fn get_credential` Rust-only invariant）。
- 額外驗證：`grep -rE "get_credential" src/` （在 main session 跑）→ 結果 empty（frontend 完全不呼叫 get_credential、永遠不 expose key 內容、Q1 (a) Decision invariant）。

**對應 chunk**：chunk 1 (Rust credentials.rs `pub(crate) fn get_credential` 不註冊進 generate_handler!)
**對應 finding**：API key 不過 IPC invariant + Decision Q1 (a) marketing claim「Your API keys never cross the IPC boundary」

---

## 驗收完成後

1. 把 18 條一一 ✅ 在 PR description 或 session log。
2. 任何一條 ❌ → 開 issue / 補修 commit。
3. 全綠 → 在 `doc/plans/02-implementation-roadmap.md` 把 M6 acceptance 從 `📋 Planned` 改成 `✅ Done`、bump 「最後更新」、更新 `.claude/PROGRESS.md`「現在在哪」line。

---

## 已知 M6 dogfood / Phase 2 deferred 項目（不在 M6 scope、IDEAS 已記）

- **ESC during enhancing 不 cancel polish**（F23 explicit M6 限制）：M6 不投資 cancel channel；Phase 2 加 `cancel_polish` Tauri command + `tokio::select!` cancel channel。
- **Polish retry-other (secondary provider)**（Decision #5 defer to v0.2）：M6 ship retry-same（同 provider 重試 1 次）；retry-other 需 secondary_provider key 概念、UI conditional fields 變複雜、v0.2 才接。
- **OpenAI / Anthropic providers**（Decision #3 defer to v0.2）：M6 free-first MVP、4 free providers 為主；OpenAI / Anthropic 沒真免費 tier、v0.2 才 expose UI。Anthropic 需 special request shape (`system` field + `x-api-key` + `anthropic-version`)、v0.2 加 `build_anthropic_request`。
- **`llm_model_id_override` UI 不曝**（F4 escape hatch hidden）：M6 schema 加但 UI 不曝、user 改 `settings.json` 手動 set。v0.2 polish 時 surface UI（advanced section 或 modal）。
- **Vocabulary multi-vendor PII**（A3 Phase 2 privacy）：M6 vocabulary 跟 Whisper prompt 一起送 Groq、polish 又送 4 個 LLM vendor。M9 privacy disclosure 列 multi-vendor flow + 加 `llm_polish_send_vocabulary: bool` opt-out。
- **Custom prompt provenance**（A4 M9 docs）：user 從外部 LLM 頁面複製 custom prompt 有 injection 風險；M9 加 textarea 旁警語 + threat model 文件。
- **In-flight polish HTTP shutdown 漏 token billing**（D21 M9 polish）：`lib.rs::RunEvent::Exit` 8-step shutdown 沒等 in-flight polish_text future；M9 加「等 polish_busy 1s timeout」step。
- **SSE streaming polish**（C18 Phase 2 candidate）：M6 spec 不做 streaming（HTTP 1 round trip）；Phase 2 candidate render 漸進式 polish text。
- **3 enum HttpProviderError 抽 shared trait**（M9 polish）：`TestConnectionError` + `TranscriptionError` + `PolishError` 各 ~80% 重複；M9 dedicated commit 抽 trait + 各 module 拼 data-layer variant。
- **Token count → SQLite analytics**（I39 / J43 M8 + M9）：M6 PolishResult 有 input_tokens / output_tokens / durationMs 但不 write SQLite；M8 history persistence 寫 SQLite、M9 dogfood 用 token / latency 量 p50 p95。
- **Settings field-level deserializer 容錯**（H36 M9 polish）：partial-malformed settings.json 整個 fail → 全 default、user 失去正確 hotkey；M9 加 per-field permissive deserializer with logging。
- **PolishError variant → i18n key 對齊**（I37 M9 release verify）：M9 SOP 加 `grep -E "polishError\." src/locales/*.json | wc -l` 應 = `grep -E "PolishError::" src-tauri/ | wc -l × 2 langs` regression check。
- **Custom prompt token estimate**（E26 M9 polish UX）：UI 只 char count、CJK 1 char ≈ 1.5 tokens 不直觀；M9 加 estimated tokens display。
- **HUD success state 1500 ms linger 仍可能不夠**（M5 retro P1 carry、Decision #8 已 bump 1000→1500）：M6 加 enhancing 流程更長、user 對 success 視覺確認需求增；dogfood 觀察、可能 v0.2 加 settings `hud.success_linger_ms`。
- **SR 連續 transcribing → enhancing 不 announce**（F28 mitigation、Phase 2 a11y polish）：F28 改 ARIA wording 故意不同 mitigate；Phase 2 a11y testing 必驗實機 NVDA / Narrator 是否 both announce。
