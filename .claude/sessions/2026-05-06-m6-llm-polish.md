# 2026-05-06 — M6 LLM Polish 多 Provider

> **Session topic**：完成 Phase 1 Milestone 6（LLM polish 多 provider — 4 free providers Groq/OpenRouter/NVIDIA NIM/Gemini + 5 preset modes default/email/chat/code/custom + tri-state polish_enabled `Option<bool>` (None auto-detect via has_credential) + retry toggle (None=ON default、Some(false)=OFF) + polish failure fallback to raw + amber success bubble + M5→M6 upgrade banner + per-step data-flow indicator + HUD `enhancing` state visual + Dashboard sidebar enhancing badge + ESC during enhancing no-op M6 limitation）— Rust `plugins/llm_polish/{mod,error,providers,prompts,registry,health}.rs` + shared `plugins/vocabulary.rs` cap helper；TypeScript `src/types/llm.ts` + voice flow polish branch + tri-state resolution + retry-same logic + polish:failed-fallback emit；Vue HudOverlay enhancing branch + success bubble dual-mode (CheckCircle2 / AlertTriangle) + click-through enhancing=ON + ARIA wording differ；HudFlowBadge enhancing badge cascade；SettingsLlmPolishSection 完整 UI + 4 free provider Select + 5 preset RadioGroup + custom prompt Textarea + retry toggle + per-step data-flow indicator + no-key warning banner + M5→M6 upgrade banner + dismiss localStorage flag + test polish button (i18n-aware sample text、不走 retry); ProviderPrivacyDialog 加 OpenRouter / NVIDIA / Gemini 3 個 body templates；CSP `connect-src` allowlist 4 free provider hosts + dashboard.json `http:default` 4 URL pattern；i18n ~104 keys × 2 locales 全到位；`docs/m6-acceptance.md` 18 條 manual SOP + 15 vite-shape screenshots。
> **Outcome**：✅ M6 implementation done、6 chunks（kickoff plan refinement → chunk 0 IPC contract types + CSP + scaffold → chunk 1 Rust llm_polish module → chunk 2 voice flow polish branch tri-state + retry → chunk 3 HUD enhancing visual + sidebar badge → chunk 4 Settings UI 4 provider + 5 preset + upgrade banner → chunk 5 milestone closure docs）；**281 cargo + 99 vitest tests pass**、靜態檢查全綠；user manual acceptance 18 條待跑（`docs/m6-acceptance.md`）。

## What changed

### M6 implementation（5 chunks + 1 milestone closure）

延續 M0-M5 subagent-driven 模式、規模略勝 M5（5 chunks → M6 5 chunks + retry toggle + tri-state semantic + 4 free provider 全變、4 free providers 換）。**M6 加新工法**：

1. **Plan-time challenger 與拆計畫平行 dispatch**（CLAUDE.md item #5 強制）— challenger（agentId `a5769f31fdf47c8c7`）找出 45 findings A-J 9 區（含 P0/P1/P2）、main session 對照 spec/plan refine 後 fold P0/P1 進 chunks（F1-F35）、P2 進 IDEAS。**Plan refinement commits**：`13fb76d`（plan refine）+ `b68026d`（roadmap pointer）。
2. **8 user-confirmed decisions** by user 2026-05-06：包含 Decision #3 重大改變（OpenAI/Anthropic → 4 free providers OpenRouter/NVIDIA NIM）+ Decision #5 加 retry toggle + Decision #7 改 tri-state `Option<bool>`。
3. **Chunk 1 implementer 中途 crash + finisher subagent 接手**（M6 special pattern、新工法）：chunk 1 implementer 寫完 `error.rs` / `providers.rs` / `prompts.rs` / `registry.rs` 5 個 sub-modules 後 crash、`mod.rs` + `lib.rs` registration 沒寫完。Main session 派 finisher subagent 接手剩餘工作 + 必要的 vocabulary helper 抽取。**lesson**：chunk 1 是 critical reviewer + Rust 全是 critical correctness path、implementer crash 不一定 fatal、finisher 從 commit-pointer 接手可行。
4. **Chunk 4 implementer 主動避開 shadcn-vue Google Fonts trap**：chunk 4 加 `<Textarea>` 元件時、implementer 沒 run `pnpm dlx shadcn-vue add textarea`、改自己手寫 41-line `Textarea.vue`（reuse `cn(...)` + class structure）。**結果**：完全避開 CLAUDE.md「常見踩雷」（Google Fonts `@import` 重新注入 `src/assets/index.css`）、reviewer grep `fonts.googleapis.com` 確認 empty。**新 best practice**：可考慮所有 simple wrapper element（Textarea / Input / Label）都手寫、避開 CLI side effect。

### Commit table

| Commit | Type | 內容 |
|---|---|---|
| `13fb76d` | docs(m6) | Plan refinement — 8 decisions resolved、Plan-time challenger 45 findings folded（F1-F35 critical 進 chunks、P2 進 IDEAS）。Spec 詳述：4 free providers Groq/OpenRouter/NVIDIA/Gemini（替換 OpenAI/Anthropic）、tri-state `Option<bool>` (None auto-detect)、retry toggle、Decision #1-#8 user confirm。 |
| `b68026d` | docs(m6) | Roadmap pointer — `doc/plans/02-implementation-roadmap.md` M6 section 加「stale-warning」box 指向 kickoff log 為 implementer authoritative source-of-truth、避免 implementer 讀 outdated OpenAI/Anthropic refs。 |
| `b20f133` | feat(m6) | Chunk 0 — IPC contract types + CSP + Settings scaffold：`src/types/llm.ts` (NEW、~80 LOC) `LlmModelInfo` + `LlmPromptMode` + `PolishResult` + `PolishFallbackPayload` + `PolishFailureReason` closed enum (12 variants F2) + `LLM_PROMPT_MODES` const tuple；`src/types/settings.ts` extend `Settings` 7 optional LLM fields（含 F4 `llmModelIdOverride`、F34 `llmPolishRetryEnabled`）；`src/types/events.ts` `VoiceFlowStateChangedPayload.status` extend `'enhancing'`（F1 audit 5 處 hardcoded narrowing site）；`src/composables/useTauriEvents.ts` `POLISH_FAILED_FALLBACK` constant；`src-tauri/tauri.conf.json` CSP `null` → 明確 `connect-src` allowlist 4 host（F3）；`src-tauri/src/plugins/llm_polish/mod.rs` placeholder + `vocabulary.rs` placeholder；3 vitest llm-types tests。 |
| `6ab4f2e` | feat(m6) | Chunk 1 — Rust `plugins/llm_polish/` 模組（5 sub-modules、3742 LOC、超出計畫 ~1100 LOC budget mostly tests + docstrings）：`error.rs` (583 LOC) `PolishError` thiserror enum 18 variants（F2 + F7 + F8 + F9 + F11 + F18 + F23）+ manual `Serialize` flat string + `PolishFailureReason` enum + `From<&PolishError>` mapping；`providers.rs` (1231 LOC) `LlmProviderId { Groq, OpenRouter, Nvidia, Gemini }` + serde lowercase、`build_request` dispatcher（OAI-compat shared 用於 Groq/OpenRouter/NVIDIA、Gemini 自己一支）+ F5 + F6 + F7 + F8 + F9 + F13 + F14 + F17 + 4-provider parse；`prompts.rs` (390 LOC) `PromptMode` 5 variants + 5 system_prompt × 2 langs + `validate_custom_prompt` chars().count() ≤ 1000 + insta snapshots（F11 + F12）；`registry.rs` (396 LOC) `LlmModelConfig` + `LLM_MODEL_LIST` 8 models（4 provider × 2、Decision #3）+ `get_effective_model_id` (F4 override)；`health.rs` (421 LOC) `test_openrouter_connection` / `test_nvidia_connection` / `test_gemini_connection` 3 個新 endpoint 不擴 transcription/health.rs（F20）；`mod.rs` (479 LOC) `polish_text` Tauri command + `LlmPolishState` + `BusyGuard` RAII (F15)；`vocabulary.rs` (141 LOC) `cap_terms` shared helper + 既有 `transcription/parser.rs::format_whisper_prompt` refactor (F10)；`lib.rs` register `polish_text` + extend `test_provider_connection` 4 arms。Tests +118（OAI-compat happy/401/429/500/parse × 4 + safety + sanity + truncated + insta + boundary + LLM_MODEL_LIST + health 3 provider × {happy, 401, 429} + Gemini key= query string禁 + OpenRouter Referer/Title header）。 |
| `589fac0` | feat(m6) | Chunk 2 — voice flow polish branch + tri-state + retry：`settings.rs` 加 7 LLM fields + 6 settings tests（含 F4 override / multi-byte 1000 chars / sparse patch / load legacy JSON / serialize default、Decision #7 default `llm_polish_enabled: None`、F34 default `llm_polish_retry_enabled: None`）；`useVoiceFlowStore.ts` (468 LOC、+329 LOC) `handleStop` polish 路徑：F21 `polishEnabledAtStart` snapshot、F22 tri-state resolution（Some(true) / Some(false) / None auto-detect via has_credential）、F34 retry-same 顯式 invoke `polish_text` 兩次（attempt 1 + attempt 2、不在 Rust 內部 retry）、F25 `polishWarning` lifecycle（transitionTo idle/recording 清掉）、F23 ESC during enhancing console.warn no-op、F24 `Busy` error 同等 polish failure；Decision #8 `SUCCESS_LINGER_MS = 1500`；14 vitest tests（polish ON+key、polish ON+no key silent skip、polish OFF 直接 success、polish failure → polishWarning + raw paste、polish 失敗 NOT trigger 3s error linger、polish OFF 不 invoke polish_text、polish failure on Some(false) provider 不送 invoke、retry ON+transient 兩次 invoke、retry OFF + transient 一次 invoke、polishWarning 在新 recording 清掉、F23 ESC during enhancing console.warn、Busy error fallback、isRetryablePolishError vs Rust is_retryable diverge note (P1 chunk 2 reviewer)）；i18n `hud.aria.enhancing` / `hud.enhancing` / `hud.warning.polishFailed` 7 keys 2 locales。Rust `credentials.rs` + `llm_polish/mod.rs` 微調（chunk 2 微觀整合）。 |
| `1930b0c` | feat(m6) | Chunk 3 — HUD enhancing visual + success dual-mode + sidebar badge：`HudOverlay.vue` (75 LOC、+50 LOC) 加 enhancing branch + `<Transition mode="out-in">` icon swap（CheckCircle2 ↔ AlertTriangle）+ ariaMessage 5 states cover + click-through enhancing=ON（F27 `syncClickThrough` switch case）+ HudSpinner reuse；`HudFlowBadge.vue` (27 LOC、+13 LOC) F26 enhancing badge cascade（amber dot + 「優化中」 zh-TW / "Enhancing" en）；i18n `sidebar.enhancingBadge` + `hud.aria.enhancing` 顯著 differ from `hud.aria.transcribing`（F28 strip non-letter chars + CJK \p{L} 後 notEqual）；4 vitest tests（HudOverlay enhancing mounts spinner + label、HudOverlay success-warning shows AlertTriangle、HudOverlay success-no-warning shows CheckCircle2、ariaMessage 5 states cover）+ 2 HudFlowBadge tests + 1 ARIA wording differ assertion；8 vite-shape screenshots in `docs/screenshots/m6/`（idle / recording / transcribing / enhancing / success-ok / success-warning / error / dashboard-sidebar-enhancing）+ `__hudDev.setPolishWarning(true)` helper for Playwright vite shape；total chunk 3 後 vitest 67 + cargo 281。 |
| `09a2923` | feat(m6) | Chunk 4 — Settings UI for LLM polish + 4 free providers active：`SettingsLlmPolishSection.vue` (NEW、686 LOC、超 400 LOC budget P1 chunk 4 reviewer flagged) 完整 UI：polish toggle + provider Select（4 free provider）+ model Select + 5 preset RadioGroup + custom prompt Textarea + retry toggle + per-step data-flow indicator（F29 always-visible、polish ON/OFF 都 render、polish ON 加 provider name）+ no-key warning banner（F22 / F29、polish ON + no key 顯示 yellow banner）+ M5→M6 upgrade banner（F35、first load show + dismiss localStorage flag `talktype:m6_upgrade_seen`）+ test polish button（F30 i18n-aware sample text、不走 retry Decision #6）+ F31 toggle OFF only test polish button disabled、其他 controls enabled；`SettingsView.vue` mount `<SettingsLlmPolishSection />`；`src/lib/providers.ts` flip openrouter/nvidia/gemini active、`LLM_MODEL_LIST` mirror Rust registry（4 free provider × 2 model = 8 models）；`ProviderPrivacyDialog.vue` 加 OpenRouter / NVIDIA / Gemini 3 個 body templates（不是 OpenAI / Anthropic、Decision #3）；i18n F32 完整 ~52 keys × 2 locales = ~104 entries（含 F34 retry + F35 upgrade banner）；4 vitest SettingsLlmPolishSection（provider switch / charCount destructive / toggle disable test button only / dataflow updates reactive）+ 1 providers test（LLM_MODEL_LIST 與 Rust registry length match ≥ 8）+ 1 ProviderPrivacyDialog test（4 provider distinct bodies）；7 vite-shape screenshots（settings-llm-polish-off / settings-llm-polish-on-has-key / settings-llm-polish-on-no-key / settings-dataflow-groq / settings-dataflow-openrouter / settings-custom-prompt-visible / settings-upgrade-banner-first-show）+ Read 確認；total chunk 4 後 vitest 99 + cargo 281。`Textarea.vue` 41 LOC implementer **手寫不跑 shadcn-vue add**（避開 CLAUDE.md Google Fonts trap、新 best practice）；`.gitignore` 加 8 line 阻擋 reviewer Playwright `m6r-*.png` leftover screenshots leak 到 repo root（chunk 3 reviewer P2-1 fix）。 |
| `<this commit>` | docs(m6) | Chunk 5 — milestone closure docs：`docs/m6-acceptance.md` (NEW) 18 條 manual SOP（4 free provider 切換 + 5 preset modes + tri-state polish_enabled 三個分支 + retry toggle on/off + 失敗 fallback amber bubble + 升級 banner + 資料流 indicator + HUD enhancing visual + Dashboard sidebar enhancing badge + ESC during enhancing no-op + API key invariant 驗證）；`.claude/sessions/2026-05-06-m6-llm-polish.md` 本檔；`.claude/IDEAS.md` 35 chunk reviewer findings (chunks 0-4 P1/P2) 增量 append；`.claude/PROGRESS.md` M6 status `🚧 In progress` → `✅ Done`；`doc/plans/02-implementation-roadmap.md` M6 dashboard ✅ + acceptance criteria 打勾；`doc/plans/01-architecture.md` 確認 Voice Flow State Machine 圖含 enhancing 行（已有、無需更新）；`doc/plans/05-data-model.md` 確認 Phase 1 完整目標 schema 列出 7 LLM fields（含 M6 加 `llmModelIdOverride` + `llmPolishRetryEnabled`、無需更新 schema 文件已 cover 5、剩 2 為 v0.2 hidden + retry toggle）。 |

### Files changed by chunk

#### Chunk 0（IPC contract types + CSP + Settings scaffold）

##### 新增

- `src/types/llm.ts`（~194 LOC；`LlmModelInfo` / `LlmPromptMode` 5 variants / `PolishResult` / `PolishFallbackPayload` / `PolishFailureReason` closed enum 12 variants）
- `src-tauri/src/plugins/llm_polish/mod.rs`（placeholder 22 LOC、chunk 1 fill）
- `src-tauri/src/plugins/vocabulary.rs`（placeholder 15 LOC、chunk 1 fill）
- `src/__tests__/llm-types.test.ts`（59 LOC、3 tests）

##### 改寫

- `src-tauri/src/plugins/mod.rs` — 加 `pub mod llm_polish;` + `pub mod vocabulary;`
- `src-tauri/src/settings.rs` — 加 7 LLM fields（含 F4 override、F34 retry toggle）
- `src-tauri/tauri.conf.json` — CSP `null` → 明確 `connect-src` allowlist 4 host（F3）
- `src/types/credentials.ts` — type 微調
- `src/types/events.ts` — `VoiceFlowStateChangedPayload.status` extend `'enhancing'`（F1）+ `PolishFallbackPayload` re-export
- `src/types/index.ts` — `export * from "./llm"`
- `src/types/settings.ts` — extend `Settings` 7 optional LLM fields
- `src/composables/useTauriEvents.ts` — 加 `POLISH_FAILED_FALLBACK` constant
- `src/main.ts:137` + `src/main-window.ts:82` — extend dev shim status type 加 `'enhancing'`（F1 grep audit）
- `src/stores/useVoiceFlowStore.ts:66` — `VoiceFlowStatus` union 加 `'enhancing'`（F1）

#### Chunk 1（Rust `plugins/llm_polish/` 模組）

##### 新增

- `src-tauri/src/plugins/llm_polish/error.rs`（583 LOC、PolishError enum 18 variants + manual Serialize + PolishFailureReason mapping）
- `src-tauri/src/plugins/llm_polish/providers.rs`（1231 LOC、LlmProviderId enum + build_request dispatcher 2 routes + 4-provider parse）
- `src-tauri/src/plugins/llm_polish/prompts.rs`（390 LOC、PromptMode 5 variants + system_prompt 5 × 2 langs + validate_custom_prompt + insta snapshots）
- `src-tauri/src/plugins/llm_polish/registry.rs`（396 LOC、LlmModelConfig + LLM_MODEL_LIST 8 models）
- `src-tauri/src/plugins/llm_polish/health.rs`（421 LOC、test_openrouter / test_nvidia / test_gemini 3 個新 endpoint）

##### 改寫

- `src-tauri/src/plugins/llm_polish/mod.rs`（22 LOC → 479 LOC、polish_text Tauri command + LlmPolishState + BusyGuard RAII）
- `src-tauri/src/plugins/vocabulary.rs`（15 LOC → 141 LOC、cap_terms shared helper + 4 unit tests）
- `src-tauri/src/plugins/transcription/parser.rs::format_whisper_prompt` — refactor 用 `vocabulary::cap_terms`（保留現有 8 unit tests pass）
- `src-tauri/src/plugins/transcription/health.rs` — 微整合
- `src-tauri/src/plugins/credentials.rs` — keep `pub(crate) fn get_credential` Rust-only（不 register IPC、F18 invariant）
- `src-tauri/src/lib.rs` — register `polish_text` command + `LlmPolishState` manage + extend `test_provider_connection` 4 arms（M3 Groq → transcription/health、其他 3 → llm_polish/health）

#### Chunk 2（voice flow polish branch + tri-state + retry）

##### 改寫

- `src-tauri/src/settings.rs` — 加 7 LLM fields 確認（chunk 0 已加 type、chunk 2 加 6 settings tests）
- `src-tauri/src/plugins/llm_polish/mod.rs` — 微調 polish_text command 與 chunk 2 整合
- `src-tauri/src/plugins/credentials.rs` — 微調
- `src/stores/useVoiceFlowStore.ts`（139 LOC → 468 LOC、+329 LOC）：
  - F21 `handleStart` capture `polishEnabledAtStart` snapshot
  - F22 `handleStop` tri-state resolution（Some(true) / Some(false) / None auto-detect）
  - F34 retry-same 顯式 invoke `polish_text` 兩次（attempt 1 + 2）
  - F25 `polishWarning` lifecycle
  - F23 ESC during enhancing console.warn no-op
  - F24 `Busy` error 同等 polish failure
  - Decision #8 `SUCCESS_LINGER_MS = 1500`
- `src/__tests__/useVoiceFlowStore.test.ts` — +14 vitest tests
- `src/i18n/locales/zh-TW.json` + `en.json` — 加 `hud.aria.enhancing` / `hud.enhancing` / `hud.warning.polishFailed` 7 keys

#### Chunk 3（HUD enhancing visual + success dual-mode + sidebar badge）

##### 新增

- `src/__tests__/aria-wording.test.ts`（84 LOC、F28 ARIA wording differ assertion）
- 8 vite-shape screenshots in `docs/screenshots/m6/`

##### 改寫

- `src/components/HudOverlay.vue`（25 LOC → 75 LOC、+50 LOC、enhancing branch + success bubble dual-mode）
- `src/components/HudFlowBadge.vue`（13 LOC → 27 LOC、+14 LOC、F26 enhancing badge cascade）
- `src/__tests__/HudFlowBadge.test.ts` — +2 tests（enhancing badge）
- `src/__tests__/HudOverlay.test.ts` — +4 tests
- `src/__tests__/useVoiceFlowStore.test.ts` — +5 tests（chunk 3 加的）
- `src/i18n/locales/zh-TW.json` + `en.json` — `sidebar.enhancingBadge`
- `src/main.ts` — `__hudDev.setPolishWarning(true)` helper for Playwright

#### Chunk 4（Settings UI + 4 free providers + 5 preset + upgrade banner）

##### 新增

- `src/components/SettingsLlmPolishSection.vue`（NEW、686 LOC、超 400 LOC budget、chunk 4 reviewer P1 flagged extract subcomponents）
- `src/components/ui/textarea/Textarea.vue`（41 LOC、implementer **手寫不跑 shadcn-vue add**、避開 Google Fonts trap）
- `src/components/ui/textarea/index.ts`（1 LOC）
- `src/__tests__/SettingsLlmPolishSection.test.ts`（355 LOC、4 tests）
- `src/__tests__/providers.test.ts`（68 LOC、1 test）
- `src/__tests__/ProviderPrivacyDialog.test.ts`（74 LOC、1 test）
- 7 vite-shape screenshots in `docs/screenshots/m6/`

##### 改寫

- `src/views/SettingsView.vue` — mount `<SettingsLlmPolishSection />`（3 LOC）
- `src/lib/providers.ts`（+157 LOC、flip openrouter/nvidia/gemini active、LLM_MODEL_LIST 8 models mirror Rust）
- `src/components/ProviderPrivacyDialog.vue`（+12 LOC、3 provider body templates）
- `src/i18n/locales/zh-TW.json` + `en.json` — F32 完整 ~52 keys × 2 locales（含 F34 retry + F35 upgrade banner）
- `.gitignore` — 8 line（`m6r-*.png` reviewer screenshot leak protection、chunk 3 reviewer P2-1）

#### Chunk 5（本檔）

##### 新增

- `docs/m6-acceptance.md`（NEW、18 條 manual SOP）
- `.claude/sessions/2026-05-06-m6-llm-polish.md`（本檔）

##### 改寫

- `.claude/PROGRESS.md` — M6 status / 下個 SOP / session table
- `.claude/IDEAS.md` — append chunks 0-4 reviewer P1/P2 findings ~35 items
- `doc/plans/02-implementation-roadmap.md` — dashboard `M6 → ✅ Done`、bump 「最後更新」、acceptance criteria 打勾
- `doc/plans/05-data-model.md` — confirm 7 LLM fields documented in Phase 1 完整目標 schema（5 既有 + 2 新加 `llmModelIdOverride` + `llmPolishRetryEnabled`）
- `doc/plans/01-architecture.md` — confirm Voice Flow State Machine 圖含 enhancing 行（已有、無需更新）

## Key decisions

8 個 user-confirmed decisions 由 user 2026-05-06 dispatch 前確認（kickoff log 為 source-of-truth）：

- **Decision #1**：`per_app_preset` schema **不加**（Phase 1 不需要、未來 v0.2 加是非破壞性 additive）
- **Decision #2**：`PromptMode` 用 **unit-only enum + 獨立 `llm_custom_prompt: Option<String>` field**（不在 enum payload）
- **Decision #3**（重大改變）：4 free providers **Groq + Gemini + OpenRouter + NVIDIA NIM**（OpenAI + Anthropic defer to v0.2）。Reasoning：OpenAI / Anthropic 沒真免費 tier；OSS MVP positioning 對齊 free-first。**Cascade**：`LlmProviderId` enum 換、CSP allowlist 換、`dashboard.json` http URL pattern 換、`LLM_MODEL_LIST` 8 models 全變、`build_anthropic_request` 整個 fn 不寫（~100 LOC ↓）、`build_openai_compatible_request` reuse 給 OpenRouter + NVIDIA。
- **Decision #4**：4 個 test endpoint URL + auth header form 明確列舉（F5）。Gemini **禁** `?key=` query string、用 `x-goog-api-key` header；OpenRouter 加 `HTTP-Referer: https://github.com/Luluboy168/TalkType` + `X-Title: TalkType` 識別 client。
- **Decision #5**：Polish failure HUD UI + retry strategy = (a) success bubble dual-mode 綠 ✓ ↔ amber ⚠ linger 1500ms、(b) Settings retry toggle `llm_polish_retry_enabled: Option<bool>` no-retry / retry-same M6 ship、(c) retry-other (secondary provider) defer to v0.2。
- **Decision #6**：Test polish button 在 `SettingsLlmPolishSection.vue`、test connection 留 `SettingsApiKeySection.vue`（M3 既定）；test polish **不**走 retry（user 要 immediate feedback）。
- **Decision #7**（重做）：Default `llm_polish_enabled = None` tri-state 動態預設（None auto-detect via has_credential、Some(true) = explicit ON、Some(false) = explicit OFF）+ M5 → M6 升級警語段。**Cascade**：`useVoiceFlowStore.handleStop` polish 判斷加 has_credential gate；Settings UI toggle visual reflect tri-state；F35 release notes 升級警語。
- **Decision #8**：`SUCCESS_LINGER_MS = 1500ms`（M5 1000 → M6 1500、apply 全部 success path）。

### 其他關鍵設計決策

- **Plan-time challenger 與拆計畫平行 dispatch**（M5 已用、M6 證實有效）：challenger 找 45 findings A-J 9 區、main session 對照 spec/plan 修：33 critical（F1-F33）進 chunks、12 P2 進 IDEAS。**修計畫比修代碼便宜**再次驗證。
- **Chunk 1 Rust pure module（不依賴 frontend）**：lib.rs register / settings.rs schema / state.rs all manage 由 Rust 自洽完成、frontend chunk 2 才 invoke。隔離 critical 路徑。
- **`#[cfg(debug_assertions)]` 不需 two-path generate_handler!**（M5 已適用、M6 不影響）：M6 加的 commands 都 release-safe、無 debug-only command。
- **Settings additive、無 migration**：M6 加的 7 fields 都 `Option<>` + `#[serde(default)]`、舊 settings.json 仍可 deserialize、`schemaVersion` 維持 1。
- **`emitTo("main-window", ...)` + `source: "hud"`**（M5 既有、M6 cascade）：M5 voice-flow:state-changed 既有 forward-compat 設計、M6 加 enhancing payload status 自動 forward 到 Dashboard sidebar 不需改 emit code。
- **`pub(crate) fn get_credential` 不 register IPC**（M3 既定、F18 invariant）：keep frontend 永不能 invoke `get_credential`、API key 不過 IPC（marketing claim「Your API keys never cross the IPC boundary」維持）。
- **`vocabulary::cap_terms` shared helper 抽**（F10、Plan §4 cross-chunk #4）：M3 既存 `transcription/parser.rs::format_whisper_prompt` 與 M6 新 `llm_polish/prompts.rs::inject_vocabulary` 共用 50 terms / 600 chars cap。M6 chunk 1 新建 `plugins/vocabulary.rs` 50 LOC、refactor M3 既有 8 unit tests 不 regression。
- **Custom prompt validation 雙層**（M6 chunks 1 + 4）：UI char count destructive 紅色（chunk 4）+ Rust `validate_custom_prompt` chars().count() ≤ 1000（chunk 1、F11）。F11 boundary tests 999/1000/1001 + 1000 zh-TW chars (3000 bytes) accept + EmptyInput。
- **`PolishError::LockPoisoned` 變體刪除**（F18）：state 只有 AtomicBool 不會 poisoned、無 source。
- **`transcription/parser.rs` 與 `llm_polish/providers.rs::parse_provider_response` 不抽 shared helper**（F19）：Whisper transcription vs LLM chat 是 different APIs、共用 helper 反而 confuse。Phase 2 candidate if pattern 出現再抽 `extract_first_string_field`。
- **OpenAI / Anthropic UI hidden** in M6（Decision #3 cascade）：`src/lib/providers.ts` 兩 provider `active: false`、Settings UI 不顯示。v0.2 才開、`active: true` flip 即可（無需改 enum / Rust schema）。
- **`llm_model_id_override` UI 不曝**（F4 escape hatch）：chunk 0 schema 加但 UI 不曝、user 改 `settings.json` 手動 set。chunks 0/2 對 `llm_model_id_override` 的 read 優先：`get_effective_model_id() = override.or(model_id)`。Phase 2 polish 時 surface UI（advanced section 或 modal）。
- **Chunk 4 implementer 主動避開 shadcn-vue Google Fonts trap**：chunk 4 加 `<Textarea>` 元件時 implementer 沒 run `pnpm dlx shadcn-vue add textarea`、自己手寫 41-line `Textarea.vue`（reuse `cn(...)` + class structure）。**完全避開 CLAUDE.md「常見踩雷」**（Google Fonts `@import` 重新注入 `src/assets/index.css`）。**新 best practice**：所有 simple wrapper element（Textarea / Input / Label）建議手寫、避開 CLI side effect。
- **`.gitignore` 加 reviewer Playwright screenshot leak protection**（chunk 3 reviewer P2-1 fix）：M5 chunk 4 已 cleanup `m5r-*.png` reviewer leftovers、M6 chunk 4 進一步加 `.gitignore` 8 line patterns 阻擋 `m6r-*.png` 之類 chunk-N reviewer screenshots leak 到 repo root。

## Surprises / 踩雷

- **Chunk 1 implementer 中途 crash + finisher subagent 接手**（M6 special pattern、新工法）：chunk 1 implementer 寫完 `error.rs` / `providers.rs` / `prompts.rs` / `registry.rs` 5 個 sub-modules 後 crash，`mod.rs` + `lib.rs` registration 沒寫完。Main session 派 finisher subagent 從 commit pointer 接手剩餘工作（mod.rs 22 → 479 LOC + lib.rs register polish_text + extend test_provider_connection 4 arms）+ 必要的 vocabulary helper 抽取。**lesson**：chunk 1 是 critical reviewer + Rust 全是 critical correctness path、implementer crash 不一定 fatal、finisher 從 commit-pointer 接手可行；chunks 1 LOC budget 1100 但 implementer 寫到 3742 LOC（mostly tests + docstrings、超 budget 但 acceptable）。
- **Chunk 1 LOC budget 嚴重超支（3742 vs 1100）**：mostly tests + docstrings、5 sub-modules 各別超 budget。implementer 寫了 ~118 chunk 1 cargo tests（vs M5 chunk 0 +6）、wiremock per-provider 4 × {happy, 401, 429, 500, parse-error} = 20 tests、insta snapshots 10 strings、boundary tests 999/1000/1001 等。**lesson**：Rust critical 路徑值得多寫 test、LOC budget hard constraint 對 critical reviewer 不適用、reviewer 應 audit 內容品質而非 LOC。
- **Chunk 2 reviewer P1：`isRetryablePolishError` doc claims 與 Rust `is_retryable` 實際 diverge**：TS `Busy` 視為 retryable、Rust `is_retryable` 不視 Busy 為 retryable。Doc 寫「mirrors Rust `is_retryable`」但實際邏輯 diverge。chunk 2 reviewer flagged P1、chunk 3+ 未修（IDEAS append、M9 polish）。
- **Chunk 3 ARIA wording 抓 `\W` regex strips CJK**（chunk 3 reviewer P2-6 fix）：F28 ARIA wording differ assertion 原計畫 `assert.notEqual(t1.replace(/\W/g, ''), t2.replace(/\W/g, ''))`、`\W` 在 zh-TW context strip 中文字元、變成兩 empty string、assertion 永遠 false。chunk 3 reviewer 抓出後改 `\p{L}`（Unicode-aware letter class）+ 對 CJK 仍 work。
- **Chunk 4 SettingsLlmPolishSection.vue 686 LOC vs 400 LOC budget**（chunk 4 reviewer P1）：超 budget 70%、chunk 4 reviewer flagged 應 extract subcomponents（DataflowIndicator + UpgradeBanner + NoKeyBanner）。implementer time 不夠、chunk 5 IDEAS append、未來 polish。
- **Chunk 4 reviewer Playwright screenshot leak vector to repo root**（chunk 3 reviewer P2-1 carry to chunk 4）：M5 chunk 4 已 cleanup 15 個 `m5r-*.png`、M6 reviewer 仍踩雷（cwd issue）。chunk 4 implementer 加 `.gitignore` 8 line patterns（`/.playwright-mcp/`、`/m6r-*.png` 等）防 future leak。**lesson**：未來 reviewer prompt 加「screenshots 必須存進 `.playwright-mcp/<reviewer>/...`」+ `.gitignore` 已 cover.
- **Chunk 4 upgrade banner flash-of-hidden-then-shown**（chunk 4 reviewer P1）：localStorage flag check 原放 `onMounted` async、reload 時 banner 短暫 flash hidden 再 show。chunk 4 reviewer 建議 sync read at script setup top（top-level）— implementer 已修 chunk 4 commit。
- **Chunk 4 Audio Input section pre-existing error**（chunk 4 reviewer P2-8）：reviewer 跑 settings smoke test 時 Audio Input section console error（不是 chunk 4 加的、M5 / M3 carry forward）。chunk 4 implementer 不修（out of scope）、append IDEAS 為 separate issue investigate。
- **i18n key 需 ~104 entries 一次寫齊**（chunk 4 大量 keys）：F32 完整列表 ~52 keys × 2 locales = ~104 entries（含 F34 retry + F35 upgrade banner + 18 polishError variants）。implementer 一次性寫齊比 incremental 加好（避免後 chunks 撞 missing keys）。
- **Decision #3 重大改變對既有計畫 cascade**：原計畫 OpenAI + Anthropic 兩個 paid providers、challenger 沒指出但 main session 接 user「OSS MVP free-first 對齊」反思後改 4 free providers（OpenRouter + NVIDIA NIM 補位）。**Cascade impact**：8 fields 變、~100 LOC ↓（不寫 Anthropic special request shape）、chunk 4 ProviderPrivacyDialog 3 provider body templates 全換。**lesson**：8 decisions resolution 階段 user feedback 比 challenger 更 strategic、user 對 product positioning 比 implementer 更敏感。
- **`shadcn-vue` Google Fonts trap 完全沒踩**（M6 first time、新 best practice）：chunk 4 implementer 主動手寫 Textarea 不跑 `pnpm dlx shadcn-vue add textarea`。CLAUDE.md「常見踩雷」M6 完全 0 重現。Reviewer 仍 grep `fonts.googleapis.com` 在 `src/assets/index.css` 確認 empty。
- **Chunk 2 + 3 reviewer 深入觀察 `polishWarning` lifecycle**（cross-session pollution P2）：rapid hotkey press 期間 polishWarning 在不同 session 殘留。chunk 2 reviewer flagged P2-2、chunk 3 部分 mitigate（lifecycle clear in transitionTo idle/recording）、IDEAS append 為 M9 race fix（與 M5 retro `audio:recording-aborted` race fix 相關）。

## Acceptance criteria（M6 18 conditions、待 user 跑）

| 條件 | 自動驗證 | 待 user 手動驗證 |
|---|---|---|
| 1. polish ON + Groq key works → enhancing transition + polished pasted | ✅ Vitest polish ON path | ✅ user 真 record + paste |
| 2. polish OFF skips polish entirely | ✅ Vitest polish OFF path | ✅ user devtools 確認無 polish_text invoke |
| 3. polish auto-detect (None) silently skips when no LLM key | ✅ Vitest None + no key | ✅ user 清 keys + reset settings |
| 4. polish auto-detect (None) auto-fires when has Groq key (M5 trust-transitive) | ✅ Vitest None + has key | ✅ user 設 Groq key + None default |
| 5. polish failure → amber warning success bubble + raw pasted | ✅ Vitest polish failure path + Chunk 3 success-warning visual | ✅ user 設錯 key 觸發 |
| 6. retry toggle ON + transient error retries once | ✅ Vitest retry ON + 2 invokes | ✅ user airplane mode 觸發 |
| 7. retry toggle OFF + transient error single attempt | ✅ Vitest retry OFF + 1 invoke | ✅ user airplane mode + retry OFF |
| 8. **5 preset modes produce visibly different outputs** | ❌ 純手動（real LLM 才知 distinct） | ✅ user 5 preset 跑同一句話 |
| 9. custom prompt 1001 chars rejected | ✅ Vitest validate_custom_prompt + chunk 4 char count destructive | ✅ user textarea 試 |
| 10. test polish button works for 4 free providers | ❌ 純手動（real provider keys 才能 dogfood） | ✅ user 4 provider 跑 test polish |
| 11. test connection 4 providers all green | ❌ 純手動（real keys） | ✅ user 4 provider test connection |
| 12. per-step data-flow indicator updates reactively | ✅ Vitest indicator switch | ✅ user 切 4 provider 看 |
| 13. no-key warning banner shows when polish ON + no key | ✅ Vitest banner conditional | ✅ user 刪 key 看 |
| 14. M5→M6 upgrade banner shows on first launch + dismissable | ✅ Vitest banner mount | ✅ user clear localStorage + reload |
| 15. HUD enhancing visual | ✅ Vitest HudOverlay enhancing branch + 5 ariaMessage states | ✅ user 真 record + 看 5 states |
| 16. Dashboard sidebar enhancing badge | ✅ Vitest HudFlowBadge enhancing | ✅ user dogfood 看 sidebar |
| 17. ESC during enhancing is no-op (M6 limitation) | ✅ Vitest console.warn no-op | ✅ user enhancing 期間按 ESC |
| 18. **API key invariant verification** — frontend cannot read keys | ✅ Cargo `pub(crate) fn get_credential` not in generate_handler! | ✅ user devtools invoke get_credential 試 |

### Static checks（all green）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → **99 pass**（M5 baseline 53 + chunk 0 llm-types +5 + chunk 2 voice flow polish +14 + chunk 3 HudOverlay/HudFlowBadge/aria-wording +14 + chunk 4 SettingsLlmPolishSection/providers/ProviderPrivacyDialog +13 = 99）
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test --lib` → **281 passed**（M5 baseline 163 + chunk 1 llm_polish module +118 = 281）

### Vite-shape screenshots（15 張、`docs/screenshots/m6/`）

| Filename | State | 驗證 |
|---|---|---|
| `m6-hud-idle.png` | idle | bubble 不 render |
| `m6-hud-recording.png` | recording | 6 bars + timer |
| `m6-hud-transcribing.png` | transcribing | spinner + 「轉錄中…」 |
| `m6-hud-enhancing.png` | enhancing (NEW) | spinner + 「優化中…」 amber tone |
| `m6-hud-success-ok.png` | success polished | 綠 CheckCircle2 + 「完成」 |
| `m6-hud-success-warning.png` | success-warning (NEW) | amber AlertTriangle + 「優化失敗、已貼上原始轉錄」 |
| `m6-hud-error.png` | error | XCircle + message |
| `m6-dashboard-sidebar-enhancing.png` | Dashboard enhancing | sidebar amber dot + 「優化中」 |
| `m6-settings-llm-polish-off.png` | Settings polish OFF | toggle OFF + indicator without polish step |
| `m6-settings-llm-polish-on-has-key.png` | Settings polish ON + has key | all controls enabled |
| `m6-settings-llm-polish-on-no-key.png` | Settings polish ON + no key | yellow warning banner |
| `m6-settings-dataflow-groq.png` | Settings polish ON Groq | indicator: Audio → Groq Whisper → Groq Polish → Paste |
| `m6-settings-dataflow-openrouter.png` | Settings polish ON OpenRouter | indicator: Audio → Groq Whisper → OpenRouter Polish → Paste |
| `m6-settings-custom-prompt-visible.png` | Settings preset=custom | textarea + char count visible |
| `m6-settings-upgrade-banner-first-show.png` | Settings first load | M5→M6 upgrade banner visible |

### Tauri runtime smoke

- `cargo check` + `cargo clippy --all-targets -- -D warnings` + `cargo build --release` + `cargo test --lib` → 編譯 + ~30 commands 整合 + 281 tests pass
- `pnpm tauri dev` 完整 launch 沒做（main session 無 GUI session、no real LLM API keys、no multi-monitor）— user 自己跑時驗 18 conditions

## Manual verification SOP（user 跑 `pnpm tauri dev` 後）

詳見 [`docs/m6-acceptance.md`](../../docs/m6-acceptance.md) — 18 acceptance conditions、每條含「設定 / 步驟 / 預期」+ 對應 chunk + F-finding 追蹤 + 截圖參考。任何 P0 fail 回 main session 修。

## Follow-ups for M7（Local whisper.cpp）

- **4 LLM providers wired but only Groq is also a Whisper provider**：M6 ship Groq + OpenRouter + NVIDIA + Gemini 4 個 LLM polish providers；只 Groq 同時支援 Whisper transcription（M3）。M7 加 local Whisper alternative（whisper.cpp）— 此時 user 可選 cloud Groq Whisper + local polish (其實沒這選項) / local Whisper + cloud LLM polish。M7 spec 應考慮 transcription provider switch 對 polish 路徑無影響（已 decoupled）。
- **`PolishResult` shape ready for M8 SQLite persist via `enhancement_duration_ms` field**：M6 PolishResult 有 `duration_ms` + `input_tokens` + `output_tokens` optional fields；M8 history persistence 從 PolishResult 讀 durationMs + token counts 寫進 transcriptions table（schema 已 cover `enhancement_duration_ms`）。M7 不直接動 SQLite、但 M7 可考慮 local transcription 也 emit 類似 metric。
- **Token counting wired in PolishResult.input_tokens + output_tokens for M9 dogfood analytics**：M6 各 provider parse usage：Groq / OpenRouter / NVIDIA (OAI-compat) `usage.prompt_tokens` + `usage.completion_tokens`；Gemini `usageMetadata.promptTokenCount` + `usageMetadata.candidatesTokenCount`。M9 dogfood 用此量 cost / latency p50 p95、評估 model 升級 ROI。
- **Settings schema additive — adding new fields in M7 same pattern (Option<> + serde default)**：M6 加 7 fields 都 `Option<>` + `#[serde(default)]`、`schemaVersion` 維持 1、舊 settings.json 仍 deserialize。M7 加 `whisper_provider` / `whisper_model_id` / `whisper_model_id_override` 同模式、不需 migration。
- **Chunk 1 LOC budget 不適用 critical correctness 路徑**：M6 chunk 1 寫 3742 LOC（mostly tests）vs 1100 budget、reviewer 不抓 P1。M7 whisper.cpp binding 是 critical 路徑、可預期 LOC budget 也會超、reviewer focus 內容品質而非 LOC。
- **OpenAI / Anthropic UI inactive in M6（v0.2 expose）**：v0.2 release 時 flip `src/lib/providers.ts` openai/anthropic `active: true`、加 Anthropic `build_anthropic_request`（不 reuse OAI-compat、需要 `system` field + `x-api-key` + `anthropic-version: 2023-06-01`）+ OpenAI 既有 OAI-compat reuse。Settings UI 自動 show 6 provider。

## Subagent dispatch pattern（M6 證實）

承襲 M0-M5 模式、規模略勝 M5、**M6 加新工法**：

1. **Plan-time challenger 與拆計畫同 message 平行 dispatch**（CLAUDE.md item #5 強制）：M5 已用、M6 證實有效。Challenger 找 45 findings、main session 對照 spec/plan 修：33 critical（F1-F33）進 chunks、12 P2 進 IDEAS。**修計畫比修代碼便宜**得到再次驗證。
2. **6 個 implementation chunks**：
   - Chunk 0（IPC contract types + CSP + scaffold）→ sequential、後 chunks 依賴 types
   - Chunk 1（Rust llm_polish module）→ 依賴 chunk 0 types + scaffold
   - Chunk 2（voice flow polish branch + tri-state + retry）→ 依賴 chunks 0 + 1
   - Chunk 3（HUD enhancing visual + sidebar badge）→ 依賴 chunks 0 + 2
   - Chunk 4（Settings UI 4 provider + 5 preset + upgrade banner）→ 依賴 chunks 0-3
   - Chunk 5（milestone closure docs）→ 依賴 chunks 0-4 全部
3. **每個 chunk 後 reviewer subagent**（chunks 0/1/2/3/4 各 1 reviewer）、reviewer 必跑 Playwright pass（CLAUDE.md UI verification SOP、chunks 3 + 4 必跑）
4. **Chunk 1 implementer 中途 crash + finisher subagent 接手**（M6 special pattern、新工法）：implementer 寫完 5 sub-modules 後 crash、finisher 從 commit pointer 接手 mod.rs + lib.rs registration。**lesson**：critical Rust 路徑 implementer crash 不一定 fatal、finisher 模式可行。
5. **Main session orchestration only**：~9 commits（plan refinement + roadmap pointer + 5 chunk + chunk 5 docs）親手 commit、commit message 親寫、整合 lib.rs / 移 screenshots / 寫 session log / update PROGRESS / IDEAS append；不寫實作 code。

對 M7（Local whisper.cpp）建議：
- Plan-time challenger 必跑、focus FFI safety / Windows build pipeline 踩雷 / latency 量測 / cross-platform binding choice
- whisper.cpp binding spike 階段：選 `whisper-rs` vs `whisper-cpp-2` vs 直接 FFI、pre-chunk 0 commit 1 個 hello-world 5s WAV transcribe
- M6 wired LLM polish 4 providers 對 M7 transcription 無影響（已 decoupled、polish 從 PolishResult.rawText 接、跟 transcription source 無關）

## 下個 session 開始時建議讀

1. `.claude/PROGRESS.md`（本 memory entry point — M6 implementation done、待 user acceptance、M7 local whisper.cpp next）
2. 本檔（M6 session log）— 特別是「Follow-ups for M7」段
3. `.claude/sessions/2026-05-06-m6-llm-polish-kickoff.md`（M6 plan refinement + 8 user-confirmed decisions + 45 challenger findings 完整 reference）
4. `.claude/IDEAS.md` 「## M6 chunk reviewer findings」+「## M6 plan-time challenger findings」section（共 ~35 + ~12 項目給 M7 / M9 / Phase 2 拾起）
5. `doc/plans/02-implementation-roadmap.md` M7 section（Local whisper.cpp、whisper-rs binding spike、HuggingFace download progress event）
6. `doc/plans/03-rust-modules.md` `## M7 transcription_local module 規劃` section
7. `doc/plans/06-hybrid-transcription.md` cloud / local switching dispatcher pattern

## User acceptance verdict（待 user 跑、append 後 mark M6 真正 Done）

User 跑 `docs/m6-acceptance.md` 18 條後、append 結果到本檔尾部「## User acceptance addendum」section（同 M5 模式、M5 acceptance addendum 抓出 3 P0 後修了 3 commit）。
