# 2026-05-05 — M4 Global Hotkey + Paste

> **Session topic**：完成 Phase 1 Milestone 4（global hotkey + Windows paste pipeline）— `SetWindowsHookExW(WH_KEYBOARD_LL)` 命名 thread + atomic-only hot path + AltGr 抑制 + double-tap detection；`arboard` STA + `spawn_blocking` + 7-step paste pipeline（modifier residue release + IME composition complete + `SetForegroundWindow` 失敗 fallback emit）；HUD `WS_EX_NOACTIVATE`；Rust-owned `Settings` v1 schema（`tauri-plugin-store` backend、JSON 持久化、自動 hot-swap hotkey config）；HUD voice flow Pinia store；shadcn-vue Settings UI hotkey section + 13-condition manual acceptance SOP。
> **Outcome**：✅ M4 done、5 chunks 落地（chunk 0 deps + chunks 1+2 平行 dispatch + chunk 3 settings + chunk 4 UI）+ 2 reviewer passes（chunks 1+2 reviewer 找 4 P1 修了 #1+#3、其餘 #2+#4 落 IDEAS；chunk 3 reviewer 0 P0 / 0 P1）+ paste.rs 拆 6 sub-modules（reviewer P1 #4）+ user dogfood 找 2 P0 acceptance bugs 修了（issue 1 keystroke suppression + issue 2 rapid press v2 parallel transcribes + session counter + paste serialization）、**157 cargo tests + 16 vitest** pass、user acceptance 通過。

## What changed

### M4 implementation（5 chunks）

延續 M0-M3 subagent-driven 模式、規模最大（vs M3 的 4 chunks）。新工法：
1. **Plan-time challenger 與拆計畫平行 dispatch**（CLAUDE.md item #5 強制）— challenger 找出 7 P0 + 6 P1 + 4 P2、主 session refine 計畫前才 dispatch implementer
2. **Chunks 1+2 平行 dispatch**（user 偏好、不衝突）— 主 session pre-create stub `clipboard_paste/mod.rs` + `hotkey_listener/mod.rs` 讓 implementer 在 same worktree 下不撞 `plugins/mod.rs`；implementer 各自只動自己的 plugin dir，主 session 在 chunks 1+2 完工後才整合 lib.rs
3. **Chunk 3 reviewer + chunk 4 implementer 平行**：reviewer read-only、chunk 4 是 UI、不衝突
4. **paste.rs split refactor 在 chunk 4 跑時平行 dispatch**：refactor 內部 `clipboard_paste/paste/` 目錄、不碰 chunk 4 的 frontend 檔

| Commit | Type | 內容 |
|---|---|---|
| `7e80f25` | feat(m4) | Chunk 0 — deps + types + IPC contract pre-update：Cargo.toml 加 `arboard 3` / `windows 0.61`（7 features：`Win32_Foundation/Foundation/UI_*/System_*` 全列）/ `tauri-plugin-store 2`；TS types `HotkeyConfig` / `TriggerKey` (kebab-case) / `TriggerMode` / `HotkeyEventPayload` / `PasteFocusRestoreFailedPayload`；event constants `HOTKEY_*` + `ESCAPE_PRESSED` + `PASTE_FOCUS_RESTORE_FAILED`；IPC contract table 6 commands + 5 events 加 M4 標記；`03-rust-modules.md` `### M4 challenger findings 落實` subsections（hotkey: split atomics + AltGr suppression + RunEvent::Exit unhook；clipboard: 7-step pipeline + STA + modifier release + IME complete + SetForegroundWindow 回值檢查 + HUD `WS_EX_NOACTIVATE`） |
| `2b7b9e2` | feat(m4) | Chunks 1+2 modules — hotkey_listener + clipboard_paste：4-file `hotkey_listener/{mod,types,shared,windows}.rs`（hot path 全 atomic、shared.rs 純 logic state machine + 4 challenger-mandated tests + 30 enum/error tests = 34 total；windows.rs 命名 thread + `SetWindowsHookExW` + `OnceLock<HookContext>` + `GetMessageW` loop + `PostThreadMessageW(WM_QUIT)` shutdown + AltGr suppression detector）；2-file `clipboard_paste/{mod,paste}.rs`（FocusState、ClipboardError 8 變體、3 commands、`apply_hud_no_activate_style` helper；7-step pipeline 全在 `spawn_blocking` + `ComGuard` + `AttachThreadInputGuard` panic-safe RAII + `paste:focus-restore-failed` event emit + 10 unit tests）；plus `cargo fmt --all` fallout（rustfmt hook 對 audio_recorder/credentials/transcription 既有檔的 line wrapping） |
| `a30ee57` | feat(m4) | Wire chunks 1+2 into lib.rs：`.manage(HotkeyListenerState::new())` + `.manage(FocusState::new())`；setup callback `apply_hud_no_activate_style(&hud)` + `state.install(handle, HotkeyConfig::default())?`；6 commands 進 `generate_handler!`；`Builder::run` → `Builder::build(...)? + App::run(\|app, event\| ...)` 重構支援 `RunEvent::Exit` → `hotkey_listener.shutdown()`；`.claude/IDEAS.md` append plan-time challenger 6 P2 |
| `7b13241` | fix(m4) | Chunks 1+2 reviewer P1 #1 + #3：`paste_text` join error 改用新 `ClipboardError::TaskPanic { stage: 'paste' }` 變體取代誤標 `SendInputFailed`（同樣 fix `copy_to_clipboard`）；`apply_hud_no_activate_style` `SetWindowLongPtrW` 回值檢查 with `SetLastError(0)` + `GetLastError()`、失敗時 return `WindowHandleUnavailable`；P1 #2 (OnceLock re-install) + P1 #4 (paste.rs > 500 LOC) + 3 P2 append IDEAS |
| `7d8f487` | feat(m4) | Chunk 3 — settings.rs v1 + useVoiceFlowStore Pinia：`src-tauri/src/settings.rs`（new file、`Settings { schemaVersion, hotkey }` v1 schema、`SettingsState { Arc<RwLock<Settings>> }`、`load_or_default(app)` 從 `tauri-plugin-store` 讀 `app_data_dir/settings.json`、3-case fallback (no file / valid / malformed)、`update(patch)` lock→patch→persist→hot-swap `HotkeyListenerState::apply_config`→broadcast `settings:updated`、12 unit tests）；`src/stores/useVoiceFlowStore.ts`（state machine `idle/recording/transcribing/success/error`、4-event listener、`init()` returns cleanup fn、6 vitest tests with `vi.useFakeTimers()`）；lib.rs 加 `pub mod settings;` + `tauri-plugin-store` plugin + setup load SettingsState 取 hotkey snapshot 給 `install()` 取代 `default()` + 2 commands；main.ts (HUD entry) post-Pinia init voice flow store 並 stash cleanup 在 `window.__voiceFlowCleanup` |
| `40777eb` | docs(m4) | Chunk 3 reviewer P2 → IDEAS：reviewer 找出 0 P0 / 0 P1、僅 2 P2（async listener registration race in init() + PASTE_FOCUS_RESTORE_FAILED state collision）— append IDEAS、M5 HUD owns visual state 時處理 |
| `35cb653` | refactor(m4) | Split paste.rs into paste/ sub-modules（reviewer P1 #4）：`paste.rs` (794 LOC) → `paste/` 目錄、6 files 全 < 500 LOC（mod.rs 325、attach.rs 180、modifiers.rs 152、com.rs 122、input.rs 107、ime.rs 41）；10 unit tests 分配到對應 sub-module；零 functional change |
| `008eb2d` | feat(m4) | Chunk 4 — Settings UI hotkey section + acceptance SOP：`src/stores/useSettingsStore.ts`（read-through cache、`load`/`update`/`subscribe`/`unsubscribe` + 7 vitest tests with `vi.hoisted()` 解決 mock-hoisting trap）；`src/components/SettingsHotkeySection.vue`（shadcn-vue Card+Select+RadioGroup+Label+Button、isDirty draft、Save+Reset+saveSuccess linger 2s）；`src/views/SettingsView.vue` 加 import + render；i18n `views.settings.hotkey.*` 18 keys × 2 languages；`docs/m4-acceptance.md`（175 行、13 acceptance conditions + bonus RunEvent::Exit unhook）；`docs/screenshots/m4/` 6 vite-shape screenshots（zhTW empty/loaded/dirty/saved + en + en-dropdown）；shadcn-vue add 觸發 Google Fonts trap → revert `src/assets/index.css` 1-7 行（如 CLAUDE.md 「常見踩雷」 警告） |

### Files changed by chunk 4（最後一個 chunk）

#### 新增

- `src/stores/useSettingsStore.ts`（134 行；Pinia composition、readonly state、`vi.hoisted()` 友善的 listener mock）
- `src/components/SettingsHotkeySection.vue`（244 行；shadcn-vue Card+Select+RadioGroup+Label+Button + draft state machine）
- `src/__tests__/useSettingsStore.test.ts`（130 行、7 tests）
- `src/components/ui/{card,label,radio-group}/`（13 自動生成 shadcn-vue 元件檔）
- `docs/m4-acceptance.md`（175 行；13 acceptance conditions + bonus）
- `docs/screenshots/m4/m4-settings-hotkey-{en,en-dropdown,zhTW-empty,zhTW-loaded,zhTW-dirty,zhTW-saved}.png`（6 張 vite-shape mode 1280×800、interactive states all covered）

#### 改寫

- `src/types/settings.ts` — `Settings` + `SettingsPatch` interfaces 加在 chunk 0 的 `HotkeyConfig` 之後
- `src/views/SettingsView.vue` — import + render `<SettingsHotkeySection />` 在 API key section 之上、header comment 更新
- `src/i18n/locales/{zh-TW,en}.json` — `views.settings.hotkey.*` 18 keys 各語言
- `src/assets/index.css` — revert shadcn-vue add 注入的 Google Fonts `@import`（CLAUDE.md 「常見踩雷」 known regression）
- `doc/plans/02-implementation-roadmap.md` — bump 「最後更新」 + dashboard `M4 → ✅ Implementation done（待 user acceptance）`

## Key decisions

- **Plan-time challenger 與拆計畫同 message 平行 dispatch**（CLAUDE.md item #5 強制）：M4 開工前 challenger 找出 7 P0 + 6 P1 + 4 P2 設計 / 邊界條件問題（modifier residue / SetForegroundWindow Win11 anti-flash / arboard STA / Mutex hot path freeze / windows-rs 0.61 vs cpal 0.15 transitive / AltGr / IME / UAC / 5x burst / etc.）。主 session 對照計畫修：原 4-chunk plan 重組成 5-chunk + chunks 1+2 順序對換（hotkey 先做、chunk 2 implementer 才能手動驗 paste；持久化選 (c) settings.rs v1 schema 而非 ad hoc JSON file 避免 M5/M6 還要 migration；acceptance criteria 6 → 13 補 challenger 缺口）
- **Persistence verdict (c) — settings.rs v1 schema in M4**：選擇順手在 M4 做 settings.rs 而非 (a) 寫 JSON ad-hoc / (b) 直連 tauri-plugin-store。理由：M5 HUD options / M6 LLM polish / M7 local whisper / M8 dashboard 全要 settings 欄位；現在做 v1 schema 加 `schemaVersion + #[serde(default)]` 防呆、後續加欄位不破壞舊 JSON、避免 2 次 migration 痛
- **Chunks 1+2 真平行 dispatch**：user 明確希望「不衝突就平行」、main session 先 pre-create stub `clipboard_paste/mod.rs` + `hotkey_listener/mod.rs` + 寫 `pub mod` 到 `plugins/mod.rs`、讓 implementer 各自只動自己 dir 不撞共享檔。**explicit instruction「DO NOT modify lib.rs」** 給兩 implementer、main session 在 chunks 1+2 完工後一併整合 lib.rs。Chunks 3 reviewer + chunk 4 implementer 也用同模式平行（read-only reviewer 不衝突）；paste.rs split refactor 在 chunk 4 跑時平行（內部 refactor 不碰 frontend）
- **Hot path 全 atomic（無 Mutex）**：challenger P0#6 — `WH_KEYBOARD_LL` hook proc 若 > LowLevelHooksTimeout (300ms) → OS-wide keyboard freeze。`HotkeySharedState` 全 atomic：`trigger_key/trigger_mode: AtomicU8`、`is_pressed/is_toggled_on: AtomicBool`、`double_tap_last_release_ms: AtomicU64`。Cold path（Phase 2 custom recording）保留 `Mutex<Option<RecordingMode>>`
- **AltGr 抑制（challenger P1#8）**：歐洲鍵盤 AltGr = LControl + RMENU 同時 down 為輸入字元（@、€、#）。`hook_proc` 看到 RightAlt down 時若 `is_vk_pressed(VK_LCONTROL)` 為 true → suppress 熱鍵 dispatch。否則 DE / FR / PL user 打 AltGr+E 想輸入 € 都會誤觸錄音
- **Modifier residue release（challenger P0#1）**：`paste_text` step 5 用 `GetAsyncKeyState` 探測 6 modifier VKs 仍 down 的個別送 `KEYEVENTF_KEYUP` 鬆開、再 SendInput Ctrl+V。防 SayIt v0.6.0 LINE-bug 等價的 Windows 情境（Hold mode user 還沒鬆手 paste 已觸發 → target 收到 Alt+Ctrl+V 變成 Word Paste Special / Slack 加重）
- **IME composition complete（challenger P1#9）**：step 6 `ImmGetContext + ImmNotifyIME(NI_COMPOSITIONSTR, CPS_COMPLETE, 0) + ImmReleaseContext`。防中文 IME（Bopomofo / 倉頡）開啟時 paste 把 composition string 一起塞進去
- **`SetForegroundWindow` BOOL return + emit fallback（challenger P0#2）**：Windows 11 anti-flash policy 拒絕 background 程序的 SetForegroundWindow。`paste:focus-restore-failed` event with `{ hwnd, lastErrorCode, message }`、return `FocusRestoreFailed` error；剪貼簿仍有文字、HUD/UI 顯示「請手動 Ctrl+V」 fallback
- **`arboard` STA + `spawn_blocking`（challenger P0#4）**：arboard 3.x Windows backend 用 `OleSetClipboard`、要求 STA。Tauri command runtime 是 tokio multi-thread → 全 7-step pipeline 在 `spawn_blocking` 包裡跑、開頭 `ComGuard::enter()` 做 `CoInitializeEx(COINIT_APARTMENTTHREADED)` + `CoUninitialize` on Drop
- **HUD `WS_EX_NOACTIVATE`（challenger P0#2 part 2）**：tauri.conf.json 不直接暴露此 flag → chunk 2 提供 `apply_hud_no_activate_style(window)` 在 lib.rs setup 後 call、用 `SetWindowLongPtrW(GWL_EXSTYLE, ...|WS_EX_NOACTIVATE)` 補上。避免 paste target 的 `SetForegroundWindow` 把 HUD 算進「最近 active」搶 focus。Reviewer P1 #3 後續加 `SetLastError(0)` + `GetLastError()` 回值檢查
- **`AttachThreadInputGuard` RAII detach panic-safe**：`Drop` 計算 detach 即使 `restore_focus_to_target` panic，避免 leaked input queue attach 卡住 OS。Test 用 `std::panic::catch_unwind` + AtomicUsize counter 驗
- **`OnceLock<HookContext>` for Hook proc state（implementer choice）**：`extern "system" fn` C ABI 不能 capture state、要存 in static。`OnceLock` 比 `lazy_static` 簡潔；single-instance app 適用。Trade-off：process lifetime 內無法 re-install（reviewer P1 #2 — Phase 2 dev hot-reload 想用要改 `Mutex<Option<HookContext>>`，已 IDEAS）
- **paste.rs split (P1 #4) — 在 chunk 4 跑時平行做**：`paste.rs` (794 LOC) 超過 CLAUDE.md 500 軟性 budget。拆 6 sub-modules（mod.rs orchestration + com.rs + attach.rs + modifiers.rs + ime.rs + input.rs）每檔 < 500、tests 跟對應函式走、零 functional change、`super::super::ClipboardError` 兩 hop reach parent error enum
- **Chunk 4 shadcn-vue Google Fonts trap 確認重現**：CLI 對 `radio-group/card/label` 都注入 `@import url('https://fonts.googleapis.com/...')` 進 `src/assets/index.css` 1-7 行（M2 chunk 3 + M3 chunk 1 過去已踩兩次、M4 chunk 4 確認 trap 100% 重現）。Implementer subagent 知道並按 CLAUDE.md SOP revert。**M5+ 加 shadcn-vue 元件時 reviewer 必 grep 驗證**

## Surprises / 踩雷

- **rustfmt hook 對全 crate 跑、非單檔**：chunks 1+2 implementer 寫各自 plugin module 時，rustfmt PostToolUse hook（CLAUDE.md `.claude/settings.json` 設定的）對 entire crate 跑了一次格式化、產生 audio_recorder / credentials / transcription 14 個既有檔的 line wrapping fallout。Bundle 進 chunks 1+2 commit 並在 commit message 說明、不混入 lib.rs wiring commit
- **`tauri::generate_handler!` macro 對 sub-module re-export 不友善（M3 已知重現）**：M3 chunk 3 踩過、`pub use health::test_provider_connection;` 在 `transcription/mod.rs` re-export 後 `lib.rs` 寫 `transcription::test_provider_connection` 編譯失敗。M4 chunks 1+2 implementer 提前知道、直接寫 `transcription::health::test_provider_connection` / `clipboard_paste::paste_text` / `hotkey_listener::update_hotkey_config`（不靠 re-export）
- **`HHOOK` is `!Send + !Sync`**：`*mut c_void` 不能跨 thread / 存進 `Arc<Mutex<>>`。Implementer 把 HHOOK 留在 hook thread 內當 local、struct 只存 `JoinHandle` + `thread_id`；shutdown 走 `PostThreadMessageW(WM_QUIT)` 通知 thread 自己 unhook 後 exit、main thread 只 `join`
- **`tauri::WebviewWindow::hwnd()` 直回 `windows::Win32::Foundation::HWND`（非 windows-sys）**：chunk 2 implementer 預期需要 windows-sys → windows-rs 轉換、實測 Tauri 2.11 `Cargo.toml:412` 已 pin `windows = "0.61"`、ABI compat、直接 use 不用轉。確認後 `apply_hud_no_activate_style` 簡化
- **`vi.hoisted()` for vitest mock factory**：chunk 4 implementer 寫 `useSettingsStore.test.ts` 一開始用 top-level `const listenMock = vi.fn()` + `vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock, ... }))` → 「Cannot access 'listenMock' before initialization」錯誤、因為 `vi.mock` factory 被 hoist 到 top of file、執行時 `listenMock` 還沒定義。改用 `vi.hoisted(() => ({ listenMock: vi.fn(), callbacks: [] }))` 把 mock + 共享 array 放在 hoisted block 解決
- **`pnpm dlx shadcn-vue@latest add` 在 worktree fail**（`ERR_PNPM_NO_IMPORTER_MANIFEST_FOUND`）：dlx temp dir manifest 跟 worktree pnpm-lock 不同步。fallback `corepack pnpm exec shadcn-vue add radio-group card label` 成功
- **shadcn-vue Google Fonts trap 100% 重現**：M2 + M3 + M4 各踩 1 次。CLAUDE.md 「常見踩雷」 section 已 documented、chunk 4 implementer 預知並 revert。**M5+ 必 grep `fonts.googleapis.com` after `shadcn-vue add` 確認 revert**
- **Pinia composition store 暴露 `readonly(ref)` 解構後不再 reactive？** — chunk 3 implementer 一度擔心、實測 Pinia 自動 unwrap refs at store boundary、`store.recordingStartedAtMs` 直接是 value（非 ref）。OK
- **Async listener registration race in `useVoiceFlowStore.init()`**（chunk 3 reviewer P2）：`listenToEvent(...).then(unlisten => unlistenFns.push(unlisten))` — events fired between `init()` 呼叫與 Promise resolve 之間會丟。Phase 1 HUD bootstrap 比 user reflex 快、不踩；M5 HUD owns visual state 時 consider 改 `init()` 回 Promise。已 IDEAS
- **`paste:focus-restore-failed` 已 active state collision**（chunk 3 reviewer P2）：若此 event 在 `transcribing` 中 fire（極少 race），會 clobber 新 flow state。Phase 1 paste ordering 幾乎不可能；M5 加 collision guard。已 IDEAS

## Acceptance criteria（M4 13 conditions、待 user 跑）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| 1. Notepad Hold mode → 「你好世界」 | ✅ Code path（capture_target_window → start_recording → transcribe_audio → paste_text） | ✅ user 跑 `pnpm tauri dev` 後在 Notepad 按住 RightAlt 說話放開 |
| 2. Word Hold mode | ✅ Same code path | ✅ user 在 MS Word |
| 3. Slack web (Edge) Hold mode | ✅ Same code path | ✅ user 在 Edge 開 Slack web |
| 4. Edge URL bar Hold mode | ✅ Same code path | ✅ user 在 Edge 網址列 |
| 5. Toggle mode（按一下開、再按一下停） | ✅ Code path（HOTKEY_TOGGLED → toggled-on / toggled-off） | ✅ user 在 Settings 切 Toggle、按 RightAlt 兩次 |
| 6. ESC cancel → 不 paste | ✅ Code path（ESCAPE_PRESSED → handleCancel → clear_recording_buffer） | ✅ user 錄音中按 ESC |
| 7. 改熱鍵到 Right Control → 立刻生效 | ✅ Code path（update_settings { hotkey } → settings:updated → apply_config atomics hot swap） | ✅ user Settings 切 Right Control + Save、按 RightAlt 不再觸發、Right Control 觸發 |
| 8. **Modifier residue (Hold) → no Alt+Ctrl+V**（challenger P0#1）| ✅ Code path（release_stuck_modifiers step 5 in pipeline） | ✅ user 在 Word 試 Hold mode、確認 Word 沒打開 Paste Special 對話框 |
| 9. **5x burst RightAlt in 1s 不 crash + leak**（challenger P1#10）| ✅ Code path + state machine unit test (5x burst) | ✅ user 連按 5 次 in 1s、確認第 6 次正常運作 |
| 10. **持續按 RightAlt 5s + 同時改 trigger key 不 freeze 鍵盤**（challenger P0#6）| ✅ Code path（hot path 全 atomic、apply_config 也只改 atomic） | ✅ user 按住 RightAlt 5s、Dashboard 換 Right Control、確認 OS 鍵盤不 freeze |
| 11. **EU 鍵盤 AltGr+E → € 不誤觸**（challenger P1#8）| ✅ Code path（AltGr suppression in shared.rs unit test） | ✅ user OS 切 German layout、按 AltGr+E 確認沒 recording 開 |
| 12. **中文 IME 開啟時 paste 不殘留 composition**（challenger P1#9）| ✅ Code path（complete_ime_composition step 6 in pipeline） | ✅ user Bopomofo 開啟 + composing 中 trigger paste、確認 paste 沒含 composition glyphs |
| 13. **UAC elevated target (Task Manager) → 友善 fallback**（challenger P0#2）| ✅ Code path（SetForegroundWindow false → emit paste:focus-restore-failed → useVoiceFlowStore 顯示「請手動 Ctrl+V」+ 3s auto-revert）| ✅ user 開 Task Manager、試 paste 進 search field、確認 HUD/Dashboard 顯示友善訊息（非 crash）+ 剪貼簿仍有文字 |
| Bonus: RunEvent::Exit 期間 hotkey unhook + thread join 不 leak | ✅ Code path（state.shutdown() 在 RunEvent::Exit 呼叫、idempotent、PostThreadMessageW + join） | ✅ user tray Quit 退出後 Process Explorer 確認無 talktype.exe lingering thread |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → **14 pass**（3 files：smoke + voice flow 6 tests + settings store 7 tests）
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test --lib` → **154 passed**（M3 94 + chunk 1 hotkey_listener 34 + chunk 2 clipboard_paste 10 + chunk 3 settings 12 + 4 = 154）

### Vite-shape screenshots

- `docs/screenshots/m4/m4-settings-hotkey-zhTW-empty.png` — Settings 全域熱鍵 section title + labels 顯示、widgets 隱藏（vite-only mode `invoke('get_settings')` fail → settings null → `v-if="draft"` 不 render）
- `docs/screenshots/m4/m4-settings-hotkey-zhTW-loaded.png` — mocked Tauri runtime、`右 Alt` dropdown 顯示、Hold radio checked、Save disabled (灰)
- `docs/screenshots/m4/m4-settings-hotkey-zhTW-dirty.png` — user 切 Toggle radio → Save enabled (黑) + Reset 出現、interactive state 驗證對 (CLAUDE.md UI verification SOP)
- `docs/screenshots/m4/m4-settings-hotkey-zhTW-saved.png` — 點 Save 後 success state
- `docs/screenshots/m4/m4-settings-hotkey-en.png` — i18n switch 英文、所有 strings 翻譯（Global Hotkey / Trigger Key / Trigger Mode / Push-to-talk (Hold) / Tap to start / stop (Toggle) / Save / Reset）
- `docs/screenshots/m4/m4-settings-hotkey-en-dropdown.png` — dropdown opened、6 個 preset trigger key 全英文（Right Alt ✓ / Left Alt / Right Ctrl / Left Ctrl / Right Shift / Left Shift）

### Tauri runtime smoke

- `cargo check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --lib` → 編譯 + 26 commands 整合 + 154 tests 都 pass
- `pnpm tauri dev` 完整 launch 沒做（main session 無 Windows GUI session、no real keyboard input）— user 自己跑時驗 13 acceptance conditions

## Manual verification SOP（user 跑 `pnpm tauri dev` 後）

詳見 [`docs/m4-acceptance.md`](../../docs/m4-acceptance.md) — 13 acceptance conditions + 1 bonus、每條含「設定 / 步驟 / 預期」。任何 P0 fail 回 main session 修。

## Follow-ups for M5

- **HUD overlay 4 visual states**（M5 owns）：useVoiceFlowStore status → HUD render（idle 隱藏 / recording 顯示 + 6-bar waveform / transcribing spinner / success ✓ 1s autohide / error ✗ 3s autohide）
- **`prefers-reduced-motion`**（對 SayIt 改進 — accessibility 缺口）：M5 HUD 加 `@media (prefers-reduced-motion)` 關閉動畫
- **ARIA**（對 SayIt 改進）：HUD 加 `aria-live="polite"` + `aria-label` 讓 screen reader 講出狀態變化
- **`settings:updated` collision guard**（chunk 3 reviewer P2）：`PASTE_FOCUS_RESTORE_FAILED` 在 `transcribing` 時不 clobber 新 flow state
- **Async listener registration race**（chunk 3 reviewer P2）：`useVoiceFlowStore.init()` consider await all listeners + 回 Promise
- **Multi-monitor + DPI HUD positioning**（plan-time challenger P2）
- **Bluetooth keyboard double-tap latency**（plan-time challenger P2）

## Subagent dispatch pattern（M4 證實）

承襲 M0-M3 模式、規模最大（5 chunks vs M3 的 4 chunks）、**M4 加新工法**：

1. **Plan-time challenger 與拆計畫同 message 平行 dispatch**（CLAUDE.md item #5 強制）：challenger 找 7 P0 + 6 P1 + 4 P2、主 session refine 計畫前才 dispatch implementer。M4 P0 全進 chunks（split atomics / AltGr / modifier release / IME / SetForegroundWindow fallback / HUD WS_EX_NOACTIVATE / arboard STA）；P1 部份進 chunks 部份留 IDEAS
2. **5 個 implementation chunks**：
   - Chunk 0（小、deps + types + IPC contract docs）→ sequential 必要、後 chunks 依賴
   - Chunks 1+2（hotkey_listener + clipboard_paste）→ **真平行 dispatch**、user 偏好「不衝突就平行」、main session pre-create stub mod.rs 讓兩 implementer 不撞 plugins/mod.rs、explicit「DO NOT modify lib.rs」、main session 在兩 chunks 完工後一併整合 lib.rs
   - Chunk 3（settings.rs + voice flow store）→ 依賴 chunks 1+2 已 land、sequential
   - Chunk 4（Settings UI + acceptance）→ 依賴 chunk 3 commands、sequential、與 chunk 3 reviewer 平行（reviewer read-only 不衝突）
   - paste.rs split refactor → P1 #4 reviewer findings、與 chunk 4 平行 dispatch、refactor 內部目錄 不碰 frontend
3. **2 reviewer passes**（superpowers:code-reviewer subagent）：chunks 1+2 reviewer 找 4 P1（#1+#3 立修、#2+#4 IDEAS / 後續 refactor）+ 3 P2 + 3 commendations；chunk 3 reviewer 0 P0 / 0 P1 / 2 P2（M5 HUD 處理）
4. **Main session orchestration only**：8 commits 親手 commit、commit message 親寫、整合 lib.rs / 移 screenshots / 寫 session log / update PROGRESS / IDEAS append；不寫實作 code

對 M5（HUD overlay、SayIt `NotchHud.vue` 861 行教訓 + 0 ARIA + 0 prefers-reduced-motion 缺口）建議：plan-time challenger 必跑找 accessibility 與動畫整合問題、UI implementer subagent 用 Playwright 測 4 個 visual state interactive screenshots（CLAUDE.md UI verification SOP）。

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M4 done acceptance 通過、M5 將是下個 milestone）
2. 本檔（M4 session log）— 特別是「Follow-ups for M5」段 + 下面「## User dogfood acceptance addendum」
3. `.claude/IDEAS.md` 「## M4 plan-time challenger P2」+「## M4 chunks 1+2 reviewer findings」+「## M4 chunk 3 reviewer findings」三 sections（共 13 項目給 M5 / M9 / Phase 2 拾起）
4. `doc/plans/02-implementation-roadmap.md` M5 section（HUD overlay 完成、4 visual states、accessibility 對 SayIt 改進）
5. `doc/plans/04-frontend-structure.md` `## components/` HudOverlay / HudWaveform / HudStateIcon section
6. `doc/reference/sayit-improvements.md` 「## 3. UX / 產品 concerns」「Accessibility 基本上不存在」段（M5 必修）

## User dogfood acceptance addendum（2026-05-05 晚）

> 13 條 manual acceptance + 自由 dogfood 發現 2 個 P0、修了 2 commit 後 user 確認 acceptance 通過。M4 真正 done 是這個 addendum 的時間點。

### Issue 1 — Right Alt 與 target apps native shortcut 衝突（user requested as P0）

**現象**：Notepad / Word 都把 Right Alt 當 menu activator、user 按熱鍵時也會觸發目標 app 的選單。

**Root cause**：`hook_proc` 永遠 `CallNextHookEx` 把 keystroke 傳給 OS、target app 還能收到 Right Alt。M4 implementation 設計就是 observational not exclusive。User 明確要求改成 exclusive（按下 trigger key 後 target app 完全收不到）。

**Fix**（commit `8e8e3bf`）：加 `HotkeySharedState::should_suppress(KeyEvent) -> bool`、hook_proc 在它 returns true 時 `return LRESULT(1)` 吃掉 keystroke。Decision tree:
- AltGr active（LCONTROL + RMENU）→ pass through（保 EU 鍵盤輸入 € @ #）
- ESC → pass through（target app 還能收到 ESC 做別的事）
- vk == trigger_key → suppress（regardless of mode 或 down/up，對稱 suppress 防止 stray modifier-up event）
- 其他 → pass through

5 個新 unit tests 驗證 truth table。

### Issue 2 — Hold mode 快速連按沒反應（user dogfood P0）

**現象**：放開後 ~1 秒內再按、沒 trigger 新錄音（user 等的時間是 Groq HTTP round trip）。

**Root cause v1 misdiagnosis**：以為 1s success linger 是 culprit、commit `8e8e3bf` relax handleStart guard 只擋 `recording | transcribing`、success / error / idle 都允許開新錄音。User re-test 後說「還是不行」— 因為 ~1s 內 status 是 `transcribing`（等 Groq 回應）、被擋住。

**Real root cause**：transcribing window 0.5-2s 是 Groq HTTP round trip + Rust `transcribe_busy` AtomicBool guard reject 並行 transcribe。

**Fix v2**（commit `14198fb`）：4 部份改造：
1. **Rust 移除 `transcribe_busy` guard**：平行 transcribe 在 architecture 上 OK — WAV buffer 在每次 `transcribe_cloud_internal` 開頭被 `Mutex::take()` consume、不會 race。`TranscriptionError::Busy` enum variant 留著供 retry 分類器用、但不再 return。`BusyGuard` struct + 2 個 panic-unwind tests 移除（-2 tests）。
2. **JS handleStart 只擋 `recording`**：唯一還擋的、因為會撞 audio_recorder single-recording invariant。任何其他狀態都允許開新錄音。
3. **Session counter**：每次 handleStart 增 `currentSession`；handleStop 開頭 capture mySession、終端 transitions（success / idle / error）只在 `mySession === currentSession` 時寫入。防舊 handleStop 完成時把新錄音的 `recording` status clobber 成 `transcribing/success`。
4. **paste serialization**：module-level `pasteChain: Promise<void>` 串行所有 `paste_text` 呼叫。沒這 lock、兩個 transcribe 同時完成時會兩個 spawn_blocking 同時 set clipboard、後者覆蓋前者後兩個 SendInput 都 paste 後者文字 → user 失去前一段。

對應新 vitest「handleStart during transcribing starts a new recording in parallel」exercise session-takeover path、舊「no-op while transcribing」test 替換掉。

### Acceptance verdict（2026-05-05 晚）

- ✅ Issue 1：Right Alt 在所有 target app 完全失去 native function、user 確認
- ✅ Issue 2 v2：rapid press 在 transcribing window 內也能 trigger 新錄音、user 確認
- ✅ 13 條原始 acceptance（user dogfood 涵蓋）

最終 metrics：
- **9 M4 commits**（chunk 0-4 + reviewer fix + paste split + docs + 2 acceptance fixes）
- **157 cargo tests** + **16 vitest tests** all pass
- 5 chunks + 2 reviewer passes + 2 acceptance fix rounds
- Working tree clean、static checks 全綠

### 給 M5 implementer 的 transition note

M5 HUD overlay 接 4 個 visual states 來自 useVoiceFlowStore.status（已就緒）、不需動 Rust 端。但要注意：
- M4 acceptance v2 後 voice flow store 已有 **session counter + paste chain**、M5 不要重蹈覆轍
- HUD `WS_EX_NOACTIVATE` 已 apply（M4 chunk 2）、M5 不要 disable
- chunk 3 reviewer 提的 2 P2（async listener race + PASTE_FOCUS_RESTORE_FAILED collision）M5 owns、要在 HUD visual binding 時一起處理
