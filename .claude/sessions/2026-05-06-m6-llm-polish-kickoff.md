# 2026-05-06 — M6 LLM Polish Kickoff（plan refined）

> **Session topic**：M6（LLM polish 多 provider）開工前 plan refinement — Plan subagent 拆 6 chunks（5.5d 預估）+ Plan-time challenger 找 45 findings（A-J 9 區、含 P0/P1/P2）、main session 整合後 fold P0/P1 進 spec、P2 進 IDEAS、解 8 個 open decisions。**本檔即 implementer 接下來讀的 refined source-of-truth**。
> **Outcome**：📋 Plan refined、**8 decisions all confirmed by user 2026-05-06**、ready dispatch chunk 0 implementer。尚未動 code。

## 8 個 decisions（user-confirmed final 2026-05-06）

| # | Decision | 結論 | 理由 |
|---|---|---|---|
| 1 | `per_app_preset` schema 是否預留 | **不加** | Phase 1 不需要、未來 v0.2 加是非破壞性 additive、不浪費磁碟。 |
| 2 | `PromptMode` enum 形狀 | **unit-only enum + 獨立 `llm_custom_prompt: Option<String>` field** | TS `'default' \| 'email' \| 'chat' \| 'code' \| 'custom'`。Custom prompt 字串存 `Settings::llm_custom_prompt`、不在 enum payload。`#[serde(rename_all = "lowercase")]` Rust 對齊。 |
| 3 | **4 個 provider（free-first MVP）** | **Groq + Gemini + OpenRouter + NVIDIA NIM**（OpenAI + Anthropic defer to v0.2） | OpenAI / Anthropic 沒真免費 tier；OpenRouter 有大量 `:free` 模型（OAI-compat、加進 cost 極低）；NVIDIA NIM 1000 free credits/月（OAI-compat）。OSS MVP positioning 對齊 free-first。 |
| 3b | 預設 model IDs（每 provider pin 具體 ID） | Groq `llama-3.3-70b-versatile` / Gemini `gemini-2.0-flash` / OpenRouter `meta-llama/llama-3.3-70b-instruct:free` / NVIDIA NIM `meta/llama-3.3-70b-instruct` | 對齊「fast + free + 中等品質」polish 需求；user 進 Settings 可改其他 model（每 provider ship 2 個 model 進 dropdown）。 |
| 4 | test_provider_connection endpoint contract | **4 個 endpoint enumerate 清楚**（見 F5 表）：Groq + OpenRouter + NVIDIA = `GET /models` Bearer auth、Gemini = `GET /v1beta/models` `x-goog-api-key` header（**禁** `?key=` query string） | 4 個 provider 都有穩定 `/models` endpoint、不需 fallback。OpenRouter 加 `HTTP-Referer: https://github.com/Luluboy168/TalkType` + `X-Title: TalkType` headers 識別 client。 |
| 5 | Polish failure HUD UI + retry strategy | **(a) Success bubble dual-mode**（綠 ✓ ↔ amber ⚠、linger 1500ms） + **(b) Settings retry toggle `llm_polish_retry_enabled: Option<bool>`** = no-retry / retry-same（M6 ship） + **(c) Retry-other（secondary provider）defer to v0.2** | (a) Visual state 不擴張（5 states 不變）。(b) Retry-same 解 90% transient failure（network blip / rate limit overshoot）。(c) Retry-other 需 secondary provider key 概念、UI conditional fields 變複雜、Phase 1 不投資。Retry 失敗 → paste raw + warning（守「polish 失敗永不擋 paste」不變式）。 |
| 6 | Test polish button location | **`SettingsLlmPolishSection.vue` 內**（test connection 留 `SettingsApiKeySection.vue`、M3 既定）。Test polish **不**走 retry（user 要 immediate feedback、retry 混淆 error 來源） | 兩個 button 測不同東西（連線 vs 完整 polish pipeline）、放在 user 改設定的 context 旁、認知最小。 |
| 7 | **Default `llm_polish_enabled`** semantics | **Tri-state `Option<bool>` 動態預設**：`None` = 「auto-detect via `has_credential(provider)`」、`Some(true)` = explicit ON、`Some(false)` = explicit OFF。**Default `llm_polish_enabled = None`**（settings::default）+ M5 → M6 升級 release notes 警語。 | M5 user 已給 Groq Whisper 信任、polish 用同 vendor chat completions 是 trust-transitive 自然延伸；無 key 自動 OFF（無 surprise）；user 進 Settings 可明確 override。對 OSS user privacy posture + frictionless 平衡最佳。 |
| 8 | `SUCCESS_LINGER_MS` | **1500 ms**（M5 1000 → M6 1500）。Apply 全部 success path（polish ON / OFF / failure-fallback 統一 1500）。 | M5 retro P1 已 flag 1s 太短、M6 enhancing 進來流程更長、warning 訊息需更多閱讀時間。 |

### Decision change log（vs 2026-05-06 草稿）

- **Decision #3 重大改變**：原推薦 `Groq + OpenAI + Anthropic + Gemini`（含付費）→ 改 `Groq + Gemini + OpenRouter + NVIDIA NIM`（全免費 tier）。連帶影響 chunk 1 不寫 Anthropic 特殊 request shape（`system` field + `x-api-key` + `anthropic-version`）— 整體 LOC ↓ 約 100 行。OpenRouter + NVIDIA 都 OpenAI-compatible、reuse `build_openai_compatible_request`。
- **Decision #4 簡化**：原 Anthropic `/v1/models` 404 fallback 已不需要（Anthropic 不在 M6 scope）。
- **Decision #5 擴充**：原只 success bubble dual-mode → 加 `llm_polish_retry_enabled` toggle（no-retry / retry-same）。Retry-other defer v0.2。
- **Decision #7 重做**：原推薦 `default true + has_credential gate` → 改 tri-state `Option<bool>` 動態預設（None auto-detect）。語意更精準、user explicit override 可保留。

### Cascade impact 摘要（chunk 0 / 1 / 2 / 4）

| 影響面 | 變更 |
|---|---|
| `LlmProviderId` enum (Rust + TS) | `{Groq, OpenRouter, Nvidia, Gemini}`（不是 OpenAI / Anthropic） |
| CSP `connect-src` allowlist | `https://api.groq.com https://generativelanguage.googleapis.com https://openrouter.ai https://integrate.api.nvidia.com`（4 個 free 對應 host） |
| `dashboard.json` `http:default` URL pattern | 同上 4 個 host pattern |
| `LLM_MODEL_LIST` registry | 4 provider 各 2 model = 8 models（見 F6 list） |
| `build_anthropic_request` / `build_gemini_request` 拆解 | Anthropic 整個 fn 不寫；Gemini 仍寫（不同 shape）；新 OpenRouter / NVIDIA 用 reuse `build_openai_compatible_request` 加 URL 切換 + OpenRouter 加 2 個 extra headers |
| Settings 新增 fields（`Option<>` + `#[serde(default)]`） | `llm_polish_enabled` (None default tri-state) / `llm_provider` (default Groq) / `llm_model_id` / `llm_model_id_override` / `llm_prompt_mode` / `llm_custom_prompt` / **`llm_polish_retry_enabled` (None=ON default、tri-state 同 polish_enabled)** / `llm_secondary_provider` (defer v0.2 不加) |
| `useVoiceFlowStore.handleStop` polish branch | tri-state polish 判斷 + retry 1 次 logic（見 F22）|
| Settings UI（chunk 4） | LlmPolishSection 加 retry toggle row（在 fallback 描述下方）|
| i18n keys | F32 + retry-related keys（`llmPolish.retry.label / retry.description`）|
| chunk 5 docs | Release notes M5→M6 升級警語段（見新 F35）|

## P0 + critical P1 challenger findings（已 fold 進 chunks）

> 完整 45 findings 在 plan-time challenger output（preserve 在 IDEAS 「## M6 plan-time challenger findings」section）。以下是 fold 後 chunks 必須處理的關鍵項。

### 必須在 chunk 0 處理（type / contract / capability foundation）

- **F1（Challenger B5）**：`VoiceFlowStateChangedPayload.status` union 從 5 states extend `'enhancing'`。**5 處 hardcoded 5-state union 必須 audit + extend**：
  1. `src/types/events.ts:177` — interface 定義
  2. `src/main.ts:137` — HUD entry dev shim
  3. `src/main-window.ts:82` — Dashboard entry dev shim
  4. `src/stores/useVoiceFlowStore.ts:66` `VoiceFlowStatus` exported type
  5. `src/components/HudFlowBadge.vue:28` `type VoiceFlowStatus = VoiceFlowStateChangedPayload["status"]`（automatic via union extend、grep 確認 narrowing 沒漏）
  
  Implementer 跑 `grep -E "'recording'|'transcribing'|'success'|'error'|'idle'" src/` 確認所有 narrowing site 已加 `'enhancing'` 處理。

- **F2（Challenger A2 + D20）**：`PolishFallbackPayload.reason` 為 closed enum string（不是 free-form）：
  ```ts
  type PolishFailureReason = 
    | 'network'
    | 'rate_limited'
    | 'auth'
    | 'parse'
    | 'timeout'
    | 'server_error'
    | 'safety_blocked'
    | 'empty_response'
    | 'truncated'
    | 'implausible_output'
    | 'busy'
    | 'cancelled';
  ```
  Rust `PolishFailureReason` enum mirror 同 variant。**所有 reason value 在 Rust producer 端 sanitize、絕不放 raw HTTP body**。

- **F3（Challenger J40）**：CSP 從 `null` → 明確 `connect-src` allowlist（Decision #3 4 free providers）：
  ```json
  "csp": "default-src 'self'; connect-src 'self' https://api.groq.com https://generativelanguage.googleapis.com https://openrouter.ai https://integrate.api.nvidia.com; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:;"
  ```
  Capability `dashboard.json` 的 `http:default` allowlist mirror 4 個 URL pattern：
  ```json
  [
    {"url": "https://api.groq.com/*"},
    {"url": "https://generativelanguage.googleapis.com/*"},
    {"url": "https://openrouter.ai/*"},
    {"url": "https://integrate.api.nvidia.com/*"}
  ]
  ```
  hud.json 不加 http permission（HUD 永不 fetch、Rust 才 fetch）。

- **F4（Challenger C13）**：加 Settings 逃生口 `llm_model_id_override: Option<String>`（model deprecation 時 user 可手動 paste 任何 model ID）。chunk 0 schema 加；chunk 4 UI exposeed in advanced section 或先不曝、留隱藏 settings.json edit。**Decision**：chunk 0 加進 schema 但 chunk 4 不曝 UI（隱藏）— 等 v0.2 polish 時再 surface。chunks 0/2 對 `llm_model_id_override` 的 read 優先：`get_effective_model_id() = override.or(model_id)`。

### 必須在 chunk 1 處理（Rust llm_polish 模組）

- **F5（Challenger A1、Decision #4）**：4 provider test endpoint URL + auth header form 明確列舉：
  | Provider | URL | Auth header | 額外 header | Notes |
  |---|---|---|---|---|
  | Groq | `https://api.groq.com/openai/v1/models` | `Authorization: Bearer ...` | — | M3 已 ship |
  | Gemini | `https://generativelanguage.googleapis.com/v1beta/models` | `x-goog-api-key: ...` | — | **禁** `?key=` query string；test 必須 assert request URL 不含 `key=` |
  | OpenRouter | `https://openrouter.ai/api/v1/models` | `Authorization: Bearer ...` | `HTTP-Referer: https://github.com/Luluboy168/TalkType` + `X-Title: TalkType` | OAI-compat、build 用 `build_openai_compatible_request` + extra headers |
  | NVIDIA NIM | `https://integrate.api.nvidia.com/v1/models` | `Authorization: Bearer ...` | — | OAI-compat、build 用 `build_openai_compatible_request` |

  4 個 provider 都有穩定 `/models` GET endpoint、不需 fallback。所有 health.rs test mock 須 wiremock 加 per-provider header validation（F13）。

- **F6（Challenger C14、Decision #3 4 free providers）**：`LlmModelConfig` registry struct 加 `max_tokens_field: enum { LegacyMaxTokens, MaxCompletionTokens }`。`build_openai_compatible_request` 從 registry 讀此 field、不 hardcode。8 個 default models（4 provider × 2 model 各）：

  | Provider | Model ID | max_tokens_field | 備註 |
  |---|---|---|---|
  | Groq | `llama-3.3-70b-versatile`（default） | LegacyMaxTokens | 平衡品質與延遲 |
  | Groq | `llama-3.1-8b-instant` | LegacyMaxTokens | 較快 |
  | Gemini | `gemini-2.0-flash`（default） | MaxOutputTokens（不同 field 名！見下 F6b） | 最新 fast model |
  | Gemini | `gemini-1.5-flash` | MaxOutputTokens | fallback |
  | OpenRouter | `meta-llama/llama-3.3-70b-instruct:free`（default） | LegacyMaxTokens | OAI-compat |
  | OpenRouter | `qwen/qwen-2.5-72b-instruct:free` | LegacyMaxTokens | 對中文較好 |
  | NVIDIA NIM | `meta/llama-3.3-70b-instruct`（default） | LegacyMaxTokens | OAI-compat |
  | NVIDIA NIM | `nvidia/llama-3.1-nemotron-70b-instruct` | LegacyMaxTokens | NVIDIA tuned for instruction following |

- **F6b（Decision #3 cascade）**：Gemini API 用 `generationConfig.maxOutputTokens`、不在 messages body root 用 `max_tokens`。`build_gemini_request` 自己處理（mirror M3 path、跟 OAI-compat dispatcher 分流）。

- **F7（Challenger C15、Decision #3 cascade）**：Gemini `parse_response` 必須 catch `candidates[0].finishReason: "SAFETY" | "RECITATION"` → `PolishError::SafetyBlocked { reason }`。**OpenAI-compat providers (Groq / OpenRouter / NVIDIA)** 任何 `choices[0].finish_reason: "content_filter"` → 同（OpenRouter 把 underlying provider 的 safety filter 標準化成 OAI shape）。新 enum variant `SafetyBlocked` 加進 PolishError + PolishFailureReason mapping。

- **F8（Challenger C17）**：`parse_polish_response` 加 4 個 sanity check：
  1. Strip common prefixes（`^Sure, here's...:\n` / `^Here's the polished version:\n` / `^Polished text:\n`）
  2. Reject if `polished.len() > raw.len() * 3` → `PolishError::ImplausibleOutput { input_len, output_len }`
  3. Reject if `polished.starts_with("I cannot") || polished.starts_with("As an AI")` → `ImplausibleOutput`
  4. Trim then if empty → `PolishError::EmptyResponse`

- **F9（Challenger D23、Decision #3 cascade）**：truncated response handling：Gemini `finishReason: "MAX_TOKENS"` / OAI-compat providers `choices[0].finish_reason: "length"` → 若 `polished.len() < raw.len()` → `PolishError::Truncated` → fallback。若 `>= raw.len() * 0.9` → 接受但 emit warning。

- **F10（Plan §4 cross-chunk #4）**：`vocabulary cap` helper 抽到 `src-tauri/src/plugins/vocabulary.rs`（~50 LOC）：
  ```rust
  pub(crate) fn cap_terms(terms: &[String], max_terms: usize, max_chars: usize) -> Vec<&str>;
  ```
  共用 between `transcription/parser.rs::format_whisper_prompt` 與 `llm_polish/prompts.rs::inject_vocabulary`。Cap 50 terms / 600 chars（M3 既定）。

- **F11（Challenger F31 + F26）**：custom prompt validation `validate_custom_prompt`：
  - Cap 用 `chars().count()`、不是 `len()` bytes（CJK 1 char = 3 bytes 處理）
  - Boundary tests：999/1000/1001 三 case、加 1000 zh-TW chars (3000 bytes) 接受 case
  - Empty input：`polish_text("")` 或 `"   "` → `PolishError::EmptyInput` 不 round-trip 到 LLM

- **F12（Challenger F30）**：`PromptMode → system prompt` golden-string snapshot tests（用 `insta` crate 或等價）。覆蓋 5 modes × 2 langs = 10 strings。任何 wording change 強制 deliberate `cargo insta review` 更新。

- **F13（Challenger F29、Decision #3 cascade）**：wiremock 必須 per-provider header validation：
  - Groq: `header("authorization", matching("^Bearer "))`
  - OpenRouter: `header("authorization", matching("^Bearer "))` + `header("http-referer", equal_to("https://github.com/Luluboy168/TalkType"))` + `header("x-title", equal_to("TalkType"))`
  - NVIDIA NIM: `header("authorization", matching("^Bearer "))`
  - Gemini: `header("x-goog-api-key", any())` + URL `path` 不含 `key=` query

- **F14（Challenger I38、Decision #3 cascade）**：`ApiError` 加 `extracted_message: String` field（從各 provider error JSON shape parse 出來）：
  - Groq / OpenRouter / NVIDIA NIM (OAI-compat): `{"error":{"message":"...","type":"...","code":"..."}}`
  - Gemini: `{"error":{"code":N,"message":"...","status":"..."}}`
  - 加 `extract_provider_error_message(provider, body) -> String` per-provider helper。Frontend i18n key `polishError.apiError` template `{status}: {extractedMessage}`。

- **F15（Plan §4 cross-chunk #5）**：`polish_busy` AtomicBool guard 用 RAII drop-guard（mirror M3 `BusyGuard`）：
  ```rust
  struct BusyGuard<'a>(&'a AtomicBool);
  impl<'a> Drop for BusyGuard<'a> { fn drop(&mut self) { self.0.store(false, Release); } }
  ```
  Reviewer 必須 trace 每個 `?` 確認 guard 在 first fallible call 之前 acquire。

- **F16（Challenger G33）**：Polish retry policy 明確（mirror M3 但更緊）：
  - **不 retry** Timeout（3s/15s budget too tight）
  - **Retry 1 次** RateLimited if `Retry-After` ≤ 2s 才 retry、否則 fallback
  - **不 retry** 其他 error variant

- **F17（Challenger I39，optional、Decision #3 cascade）**：`PolishResult` 加 token count 欄位（不在 UI display、留 M9 dogfood analytics）：
  ```rust
  pub struct PolishResult {
      polished_text: String,
      duration_ms: u64,
      input_tokens: Option<u32>,
      output_tokens: Option<u32>,
  }
  ```
  各 provider parse：Groq / OpenRouter / NVIDIA (OAI-compat) `usage.prompt_tokens` / `usage.completion_tokens`；Gemini `usageMetadata.promptTokenCount` / `usageMetadata.candidatesTokenCount`。**M6 不存 SQLite（M8 才接 history persist）**。

- **F18（Challenger J45）**：`PolishError::LockPoisoned` 變體 **刪除**（state 只有 AtomicBool 不會 poisoned、無 source）。

- **F19（Challenger J42）**：`transcription/parser.rs` 與 `llm_polish/providers.rs::parse_provider_response` **不抽 shared helper**。M6 文件 comment 說明「different APIs (Whisper transcription vs LLM chat)」。Phase 2 候選若 pattern 出現再抽 `extract_first_string_field`。

- **F20（Challenger J41、Decision #3 cascade）**：M6 在 `plugins/llm_polish/health.rs` **新檔** 寫 OpenRouter / NVIDIA NIM / Gemini test 路徑、**不擴 transcription/health.rs**。Whisper-only 的 Groq cloud transcription test 留在 `transcription/health.rs`（M3 既有）。`test_provider_connection` Tauri command 在 `lib.rs` register 一次、內部 dispatch by provider id (4 arms: Groq → transcription/health、其他 3 → llm_polish/health)。

### 必須在 chunk 2 處理（store + settings + voice flow）

- **F21（Challenger B7）**：polish-on/off **snapshot at hotkey press time**。`handleStart` capture `polishEnabledAtStart = settings.llmPolishEnabled !== false && hasCredentialAtStart`、傳給 `handleStop` 用。Settings 在 mid-flow 改動不影響 in-flight pipeline。

- **F22（Decision #7 tri-state + Decision #5 retry toggle）**：`handleStop` polish 路徑用 **tri-state 動態預設** + retry 1 次 logic：

  ```ts
  // F21 snapshot at handleStart time
  const polishExplicit = settings.llmPolishEnabled  // None | Some(true) | Some(false)
  const polishProvider = settings.llmProvider ?? 'groq'
  const retryEnabled = settings.llmPolishRetryEnabled !== false  // None or Some(true) = ON default
  const hasKeyAtStart = await invoke<boolean>('has_credential', { provider: polishProvider })

  // Decision #7 tri-state resolution
  let shouldPolish: boolean
  if (polishExplicit === true) {
    shouldPolish = true   // user explicit ON (即使無 key 也試、走 ApiKeyMissing fallback)
  } else if (polishExplicit === false) {
    shouldPolish = false  // user explicit OFF
  } else {
    shouldPolish = hasKeyAtStart  // None auto-detect
  }

  if (!shouldPolish) {
    // 直接 paste raw、不 transitionTo enhancing、不 emit fallback warning
    await pasteTextSerial(result.rawText)
    transitionTo('success')  // 純綠 ✓
    return
  }

  // Polish 路徑
  transitionTo('enhancing')
  let polishedText: string
  let attempt = 1
  try {
    const polishResult = await invoke<PolishResult>('polish_text', { rawText: result.rawText, vocabulary, attempt })
    polishedText = polishResult.polishedText
  } catch (firstErr) {
    if (retryEnabled) {
      // Decision #5 retry-same: 再試 1 次（同 provider、同 settings）
      attempt = 2
      try {
        const polishResult = await invoke<PolishResult>('polish_text', { rawText: result.rawText, vocabulary, attempt })
        polishedText = polishResult.polishedText
      } catch (secondErr) {
        // Both attempts failed → fallback to raw
        polishedText = result.rawText
        polishWarning = true
      }
    } else {
      // No retry: directly fallback
      polishedText = result.rawText
      polishWarning = true
    }
  }

  await pasteTextSerial(polishedText)
  if (mySession === currentSession) {
    transitionTo('success')  // polishWarning=true 在 chunk 3 切 amber bubble
  }
  ```

  **Notes**：
  - `attempt` param 傳給 Rust 端、Rust polish_text 可記錄 attempt 給 dogfood analytics（不 retry 進 Rust 內部、frontend 顯式 invoke 兩次）
  - `polish_busy` AtomicBool guard 仍在 Rust 內部、保護同 process 並發 polish。Retry 兩次必須 sequential（second invoke 等 first complete）— 用 `await` 自然序列化
  - F25 `polishWarning` lifecycle：在新 `transitionTo('idle' | 'recording')` 清掉

- **F23（Challenger B11）**：ESC during enhancing → cancel in-flight LLM HTTP（reqwest future drop）+ paste raw + emit `polish:failed-fallback { reason: 'cancelled' }`。新 `PolishError::Cancelled` enum variant 加進 chunk 1 + `PolishFailureReason` enum。**chunk 2 frontend** 加 `escape:pressed` listener 在 `enhancing` state、cancel polish_busy guard token（但 reqwest cancellation 是 Rust-side、需新 Tauri command `cancel_polish` 或用 `tokio::select!` 接 cancel channel — 簡化方案：**M6 不接 ESC during enhancing**、ESC 只在 recording 有效（同 M4），enhancing 期間 ESC 只 emit warning「優化中無法取消」進 console.warn）。**Decision**: ESC during enhancing 不 cancel polish (M6 不投資)、explicit document in spec acceptance「ESC during enhancing → no-op + console.warn」。Phase 2 加 cancel channel。

- **F24（Challenger B12）**：toggle hotkey burst during enhancing race。**Decision**：保留 `polish_busy` guard、second polish_text invoke 期間返回 `PolishError::Busy`。useVoiceFlowStore 把 `Busy` 同等於 polish failure → fallback to raw + emit `polish:failed-fallback { reason: 'busy' }`。

- **F25（Plan §4 cross-chunk #6）**：`polishWarning` ref lifecycle — `transitionTo` whenever `next === 'idle' || next === 'recording'` 清掉 polishWarning。否則 rapid press 會 carry warning 進下一個 success state。

### 必須在 chunk 3 處理（HUD + sidebar）

- **F26（Challenger B6）**：HudFlowBadge **必須** 顯示 enhancing state（不 silent inherit M5 recording-only）。設計：
  - `recording`：紅 dot + 「錄音中」（M5 既有）
  - `enhancing`：amber dot + 「優化中」（M6 新）
  - 其他 state：hidden（M5 既有）
  
  Listener 仍只讀 `payload.status`（chunk 3 reviewer P2-4 future-proof tooltip 在 Phase 2 才需 message field）。

- **F27（Challenger B9）**：HUD click-through state matrix加 `enhancing` 行 = ON（同 transcribing）。`HudOverlay.vue:syncClickThrough` switch 加 case。`error` 仍是唯一 click-through OFF。

- **F28（Challenger B10）**：ARIA `hud.aria.transcribing` 與 `hud.aria.enhancing` 故意 wording 不同（避免 SR 連續同訊息不 announce）。Vitest assert 兩字串 differ by more than punctuation（`assertions.notEqual(t1.replace(/\W/g, ''), t2.replace(/\W/g, ''))`）。

### 必須在 chunk 4 處理（Settings UI）

- **F29（Challenger E25）**：per-step data-flow indicator 在 SettingsLlmPolishSection **頂部**、polish OFF 也 render（顯示 `Audio → Groq Whisper → Paste` 不含 polish step）；polish ON + no key → 加 yellow warning banner「未設定 [provider] API key、潤飾路徑會跳過」。

- **F30（Challenger E27）**：Test polish button 用 i18n-aware sample text：
  - zh-TW: `「這個 呃 就是我覺得啊」`
  - en: `"Um, like, I think this is, you know, alright"`
  - **M6 不 user-editable**（Phase 2 加 try-your-own-text field）

- **F31（Challenger E28）**：polish toggle OFF 時 provider/model/preset/custom 控制 **保持 enabled**（user 可 pre-configure），只 disable Test button + 加 tooltip「啟用 LLM 潤飾後可測試」。

- **F32（Challenger I37、Decision #5 retry toggle 加 keys）**：i18n keys 列表（chunk 4 必須一次寫齊、zh-TW + en 兩 file）：
  ```
  llmPolish.title
  llmPolish.enable / enableDescription
  llmPolish.providerLabel / modelLabel / presetLabel / customPromptLabel
  llmPolish.preset.{default,email,chat,code,custom}.{label, description}  // 5 × 2 = 10
  llmPolish.customPrompt.placeholder / charCount / tooLong
  llmPolish.dataFlow.{audio,whisper,polish,paste,noRetention,withoutPolish}
  llmPolish.testButton / testing / testBefore / testAfter / testFailed
  llmPolish.testSample  // 「這個 呃 就是我覺得啊」/ en 對應
  llmPolish.disabled    // 「(已停用)」/ "(disabled)"
  llmPolish.banner.noKey  // 「未設定 [provider] API key、潤飾路徑會跳過」
  llmPolish.retry.label  // 「失敗時自動重試 1 次」 / "Retry once on failure"
  llmPolish.retry.description  // 「polish 第一次失敗時用同 provider 重試 1 次。重試仍失敗則貼上原始轉錄並顯示警告。」 / en 對應
  
  polishError.{apiKeyMissing, offline, timeout, tlsFailure, dnsFailure, connectionRefused, networkOther, rateLimited, apiError, parseError, busy, invalidPromptLength, safetyBlocked, emptyResponse, truncated, implausibleOutput, cancelled, disabled}  // 18 keys
  
  hud.aria.enhancing / hud.enhancing / hud.warning.polishFailed
  
  sidebar.enhancingBadge

  upgradeNote.m5ToM6.title  // 「升級到 M6 LLM Polish」/ "Upgrade to M6 LLM Polish"
  upgradeNote.m5ToM6.body   // 升級警語完整內容、F35 描述
  ```
  Total ~52 keys × 2 locales = ~104 string entries。

- **F33（Challenger G33）**：shadcn-vue Google Fonts trap：chunk 4 若需 add `<Tabs>` / `<RadioGroup>` 等新元件、implementer **必須** revert `src/assets/index.css` 7-line `@import url('https://fonts.googleapis.com/...')` 重新注入。**Reviewer `grep fonts.googleapis.com src/assets/index.css` 必須 empty**。

- **F34（Decision #5 retry toggle 細節）**：retry 策略明確規格（chunk 1 polish_text + chunk 2 useVoiceFlowStore 共同實作）：
  - Frontend `handleStop` 顯式 invoke `polish_text` 兩次（不在 Rust 內部 retry、frontend 控制）— 第二次傳 `attempt: 2` 給 Rust 紀錄 dogfood analytics
  - retry **不**對所有 error variant、只對 `Timeout / RateLimited / NetworkOther / ConnectionRefused / DnsFailure / TlsFailure` 等 transient 錯誤 retry
  - **不 retry**：`ApiKeyMissing`（user action needed）、`InvalidPromptLength`（settings issue）、`SafetyBlocked`（content issue）、`Disabled`（settings issue）、`EmptyInput`（input issue）、`ImplausibleOutput`（model issue）— 這些 retry 仍會撞同錯
  - retry 中間沒有 backoff（這是 user-facing latency-critical 路徑、加 backoff 反而 UX 差）
  - retry 兩次都失敗 → fallback to raw + emit `polish:failed-fallback { reason }`、reason 用第二次 attempt 的 PolishError → `From<&PolishError>` mapping
  - **Test polish button 不走 retry**（Decision #6 user 要 immediate feedback）
  - polish_busy 不釋放 between retries（attempt 1 & 2 共享 BusyGuard、保證 sequential、不互相 race）

- **F35（Decision #7 + Decision #3 cascade、release notes）**：M5 → M6 升級警語必寫進 v0.1.0 README + GitHub Release notes：
  ```
  ⚠️ M5 → M6 升級行為變化：
  
  M6 加入 LLM Polish 功能、預設行為「auto-detect」：
  - 你已設過 Groq API key (M3 Whisper)：因 Groq 同把 key 也支援 LLM、polish 預設啟用、轉錄文字會送進 Groq chat completions endpoint 加工。
  - 你還沒設任何 LLM provider key：polish 自動 OFF、原樣轉錄行為不變。
  
  如不希望 polish 啟用、進 Settings → LLM Polish → toggle 關閉。
  完整資料流請看 Settings 內的 per-step data-flow indicator。
  ```
  - chunk 5 owns: 寫進 `README.md` v0.1.0 section + GitHub Release notes (chunk 5 IDEAS append)
  - chunk 4 加 i18n 進 `upgradeNote.m5ToM6.{title,body}` keys、Settings UI 在 LlmPolishSection 頂部 first-load 時 render dismiss-able warning（user dismiss 後永久不再 show、用 localStorage flag `talktype:m6_upgrade_seen`）
  - acceptance criteria 加：「M5 settings.json (只有 hotkey field) load 進 M6、polish_enabled = None 自動 detect、有 Groq key → polish ON 自動」test case

## P2 challenger findings（不阻擋 M6 ship、append IDEAS）

> 列入 IDEAS.md 「## M6 plan-time challenger findings (2026-05-06 — 不在 M6 scope、留下次)」section。

- **A3** vocabulary 多 vendor PII：M9 privacy disclosure 列出全 vendor flow + Settings 加 `llm_polish_send_vocabulary: bool` opt-out。
- **A4** custom prompt provenance（user 從 LLM page paste 進有 injection 風險）：M9 加「複製自 LLM 頁面警告」textarea 旁。
- **C18** SSE streaming polish：Phase 2 candidate（render 漸進式 polished text）。
- **C19** Gemini `?key=` query string 禁用：F5 已強制 header form、加 unit test 阻 regression。
- **D21** in-flight polish HTTP 在 app shutdown 漏 token 計費：M9 加 `lib.rs` 8-step shutdown 第 N 步「等 polish_busy 1s timeout」。
- **D22** polish 返回空字串（非 safety block、純空）：F8 已 cover EmptyResponse、確認沒漏。
- **E26** custom prompt token estimate（chars × 1.5 CJK heuristic）：M9 polish UX 加 estimated token display。
- **G33（Plan）** 3 enum HttpProviderError 抽 shared trait：M9 polish 候選、M6 三份 maintain（Plan §4 #1）。
- **H35** M5 → M6 upgrade 路徑文檔：本 session log 已 cover、加進 acceptance「user 升 M5 settings.json → M6 polish_enabled default 後行為」測試。
- **H36** Settings field-level deserializer 容錯：M9 polish 加 per-field permissive deserializer。
- **I39** PolishResult token 寫進 SQLite：M8 history persistence 整合（F17 已預留 PolishResult shape）。
- **J43** `enhancement_duration_ms` SQLite write：M8 history persistence integration、M6 只 return PolishResult.durationMs。
- **J44** Polish retry policy 明確：F16 已 fold（不 retry Timeout、only RateLimited if Retry-After ≤ 2s）。

## Refined chunked plan

### 🧱 Chunk 0 — Cargo deps + IPC contract types + `tauri.conf` CSP + capabilities（0.5d）

**Deps**：none.

**Rust files changed**:
- `src-tauri/Cargo.toml` — verify `reqwest 0.12` / `serde_json` / `wiremock` from M3 (no-op verify)
- `src-tauri/src/plugins/mod.rs` — add `pub mod llm_polish;` + `pub mod vocabulary;`（vocabulary helper extraction）
- `src-tauri/tauri.conf.json` — CSP from `null` → 明確 connect-src whitelist（F3）
- `src-tauri/capabilities/dashboard.json` — `http:default` allow 4 URL pattern；hud.json 不變（不加 http permission）

**Frontend files changed**:
- `src/types/llm.ts` (NEW, ~80 LOC) — `LlmModelInfo`, `LlmPromptMode`, `PolishResult`, `PolishFallbackPayload`, `PolishFailureReason` closed enum (F2), `LLM_PROMPT_MODES` const tuple
- `src/types/index.ts` — `export * from "./llm"`
- `src/types/settings.ts` — extend `Settings` with 6 optional LLM fields（含 F4 `llmModelIdOverride`）+ `SettingsPatch` 同擴
- `src/types/events.ts` — `VoiceFlowStateChangedPayload.status` extend `'enhancing'`（F1）+ `PolishFallbackPayload` re-export
- `src/composables/useTauriEvents.ts` — `POLISH_FAILED_FALLBACK = "polish:failed-fallback" as const`
- `src/main.ts:137` + `src/main-window.ts:82` — extend dev shim status type 加 `'enhancing'`（F1 grep audit）
- `src/stores/useVoiceFlowStore.ts:66` — `VoiceFlowStatus` union 加 `'enhancing'`（F1）
- `src/components/HudFlowBadge.vue:28` — automatic via union extend、grep narrowing 確認

**Settings schema additions**（F4 included、no migration、all `Option<>` + `#[serde(default)]`）:
- `llm_polish_enabled: Option<bool>` — **tri-state Decision #7**：`None` = auto-detect via has_credential、`Some(true)` = explicit ON、`Some(false)` = explicit OFF。`Settings::default()` 留 `None`、不 hardcode bool default
- `llm_provider: Option<LlmProviderId>`（default `Groq`）
- `llm_model_id: Option<String>`
- `llm_model_id_override: Option<String>`（**F4 escape hatch**、UI 不曝露 in M6）
- `llm_prompt_mode: Option<PromptMode>`（default `Default`）
- `llm_custom_prompt: Option<String>`
- **`llm_polish_retry_enabled: Option<bool>` — Decision #5 retry toggle**：`None` 同 `Some(true)` 視為 ON（90% transient 用 retry-same fix）、`Some(false)` = no-retry。Settings UI 顯示 toggle、default 視覺反映 ON
- （**不加** `per_app_preset`，Decision #1；**不加** `llm_secondary_provider`，Decision #5 retry-other 留 v0.2）

**Test plan**:
- `pnpm build` + `vue-tsc --noEmit` green
- 3 vitest tests: `LLM_PROMPT_MODES.length === 5` + `PolishFallbackPayload.reason` enum closed + Settings new fields accept undefined

**Reviewer checklist**:
- [ ] CSP `connect-src` 列 4 provider 完整（F3）
- [ ] `dashboard.json` `http:default` 4 URL pattern；hud.json 沒 http permission
- [ ] `grep -E "'recording'|'transcribing'|'success'|'error'|'idle'" src/` audit 5 處 hardcoded narrowing 都加 `'enhancing'` case（F1 critical）
- [ ] `grep fonts.googleapis.com src/assets/index.css` empty
- [ ] vue-tsc green、cargo clippy green
- [ ] `LLM_MODEL_LIST.length >= 8`（chunk 1 才實值、chunk 0 type 先加）

---

### 🦀 Chunk 1 — `plugins/llm_polish/*` Rust 模組（2d）

**Deps**：chunk 0.

**Rust files added**（mirror `transcription/` layout、總 LOC ~1100）:
- `src-tauri/src/plugins/llm_polish/mod.rs` (~150 LOC) — `LlmPolishState`, `polish_text` Tauri command, `BusyGuard` RAII（F15）
- `src-tauri/src/plugins/llm_polish/error.rs` (~180 LOC) — `PolishError` thiserror enum、manual `Serialize` flat string（F2 + F7 + F8 + F9 + F11 + F18 + F23）含 variants:
  - `ApiKeyMissing { provider }` / `Disabled` / `Busy` / `EmptyInput` / `EmptyResponse`
  - `Offline | Timeout | TlsFailure | DnsFailure | ConnectionRefused | NetworkOther`
  - `RateLimited { retry_after_secs }` / `ApiError { status, body, extracted_message }`（F14） / `ParseError`
  - `InvalidPromptLength { actual, max: 1000 }`（F11）
  - `SafetyBlocked { reason }`（F7）/ `Truncated { polished_len, raw_len }`（F9） / `ImplausibleOutput { input_len, output_len }`（F8）
  - `Cancelled`（F23）
  - `Credentials(String)`
  - **不含** `LockPoisoned`（F18 刪）
  - `PolishFailureReason` enum mirror frontend、`From<&PolishError>` impl mapping
- `src-tauri/src/plugins/llm_polish/providers.rs` (~280 LOC) — F5 + F6 + F7 + F8 + F9 + F13 + F14 + F17（**Decision #3 free 4 providers，Anthropic 不寫**）:
  - `LlmProviderId { Groq, Gemini, OpenRouter, Nvidia }` + serde lowercase（`openrouter` / `nvidia`）
  - `build_request` dispatcher per provider（2 routes：OAI-compat shared 用於 Groq/OpenRouter/NVIDIA、Gemini 自己一支）
  - `build_openai_compatible_request(provider, model, system, user, key)`：
    - `LlmProviderId::Groq` → URL `https://api.groq.com/openai/v1/chat/completions`、Bearer auth
    - `LlmProviderId::OpenRouter` → URL `https://openrouter.ai/api/v1/chat/completions`、Bearer auth + `HTTP-Referer: https://github.com/Luluboy168/TalkType` + `X-Title: TalkType` headers
    - `LlmProviderId::Nvidia` → URL `https://integrate.api.nvidia.com/v1/chat/completions`、Bearer auth
    - 共用 body：`{model, messages: [{role:system,content:...},{role:user,content:...}], max_tokens, temperature: 0.3}`（F6 max_tokens_field 讀 LlmModelConfig）
  - `build_gemini_request`（F5 header form `x-goog-api-key`、禁 query-string-key）：
    - URL `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`
    - body：`{contents: [{role:user, parts: [{text}]}], systemInstruction: {parts: [{text: system_prompt}]}, generationConfig: {maxOutputTokens, temperature: 0.3}}`
  - `parse_response` dispatcher + 各 provider parse（F7 safety block、F8 sanity check、F9 truncated handling、F14 error message extract、F17 token count parse）
- `src-tauri/src/plugins/llm_polish/prompts.rs` (~180 LOC) — F11 + F12:
  - `PromptMode { Default, Email, Chat, Code, Custom }` unit enum
  - `system_prompt(mode, lang, custom_text) -> String` — 5 prompts × 2 langs = 10 const strings + custom 走 validate_custom_prompt
  - `validate_custom_prompt(s)` — `chars().count() ≤ 1000`（F11）、empty input → EmptyInput
  - `inject_vocabulary(prompt, vocab)` — 用 F10 shared helper
  - **insta snapshot tests**（F12）覆蓋 10 strings
- `src-tauri/src/plugins/llm_polish/registry.rs` (~120 LOC) — F6:
  - `LlmModelConfig { id, provider, max_tokens_field, default_max_tokens, ... }`
  - `LLM_MODEL_LIST` ≥ 8 models（Decision #3 pinned IDs）
  - `find_llm_model_config` / `get_models_by_provider` / `get_default_model_id` / `get_effective_model_id(settings)`（F4：override.or(model_id)）
- `src-tauri/src/plugins/llm_polish/health.rs` (~200 LOC) — **F20 新檔不擴 transcription/health.rs**（**Decision #3** 4 free providers、Anthropic 不寫）：
  - `test_openrouter_connection`（GET `https://openrouter.ai/api/v1/models` Bearer + Referer + Title headers）
  - `test_nvidia_connection`（GET `https://integrate.api.nvidia.com/v1/models` Bearer）
  - `test_gemini_connection`（GET `https://generativelanguage.googleapis.com/v1beta/models` `x-goog-api-key` header、F5 禁 query-string-key）
  - 共用 `TestConnectionResult` shape（M3 reuse）
  - `lib.rs::test_provider_connection` dispatcher 加 4 arms（M3 Groq → transcription/health、其他 3 → llm_polish/health）
- `src-tauri/src/plugins/vocabulary.rs` (~50 LOC) — **F10 新 shared helper**:
  - `pub(crate) fn cap_terms(terms: &[String], max_terms: usize, max_chars: usize) -> Vec<&str>`
  - 共用 50 terms / 600 chars cap from M3 既定
  - 4 unit tests（empty / under cap / term-cap / char-cap edge）

**Rust changes**（既有 file）:
- `src-tauri/src/plugins/transcription/parser.rs::format_whisper_prompt` — refactor 用 `vocabulary::cap_terms`（保留現有 8 unit tests pass）
- `src-tauri/src/lib.rs` — register `polish_text` command + `LlmPolishState` manage + extend `test_provider_connection` arms

**Frontend files**：none this chunk（Rust-pure）。

**Test plan**:
- Cargo unit tests in `providers.rs`:
  - 4 happy-path wiremock × 4 provider（Groq / OpenRouter / NVIDIA / Gemini、含 F13 header validation per provider）
  - 4 × `{401, 429 with Retry-After, 500, parse-error}` = 16 tests
  - `parse_response` 各 happy + malformed × 4 = 8 tests
  - F7 SafetyBlocked × 2 path（Gemini SAFETY finishReason / OAI-compat `content_filter`）
  - F8 sanity check（`"As an AI..."` reject / 4× length reject / prefix strip）
  - F9 truncated（`> 0.9× raw` accept / `< 0.9× raw` reject）
- Cargo unit tests in `prompts.rs`:
  - F12 insta snapshots × 10 strings
  - F11 boundary 999/1000/1001 + 1000 zh-TW chars (3000 bytes) accept + EmptyInput
- Cargo unit tests in `registry.rs`:
  - `LLM_MODEL_LIST.len() >= 8`、`get_default_model_id` × 4 provider 都 valid id
  - F4 `get_effective_model_id` override 優先 / 否則 fallback model_id
- Cargo unit tests in `health.rs`:
  - 3 新 wiremock × {happy, 401, 429}（OpenRouter / NVIDIA / Gemini 各一組）
  - Gemini test request URL 不含 `key=` query string assertion（F5）
  - OpenRouter test request 含 `HTTP-Referer` + `X-Title` headers assertion（F13）
- Cargo unit tests in `vocabulary.rs`:
  - F10 4 boundary tests + 1 既有 transcription/parser 不 regression test

**Reviewer checklist**:
- [ ] `grep -rE "polish.*get_credential|llm.*get_credential" src/` 空（API key invariant）
- [ ] `cargo clippy --all-targets -- -D warnings` 綠
- [ ] `cargo test --workspace` 綠（M5 baseline 163 + chunk 1 ~70 = ~233）
- [ ] F13 wiremock per-provider header assert 都齊
- [ ] F4 `llm_model_id_override` 已 schema、UI 還沒 expose（intentional）
- [ ] BusyGuard RAII 沒 bypass: `grep "polish_busy.store" src-tauri/` 只在 BusyGuard impl 出現
- [ ] F18 `LockPoisoned` variant 不在 PolishError enum

---

### 🔄 Chunk 2 — Settings 擴 + useVoiceFlowStore enhancing + paste fallback（1d）

**Deps**：chunks 0 + 1.

**Rust files changed**:
- `src-tauri/src/settings.rs` — 加 6 fields（含 F4 override）到 Settings + SettingsPatch + apply_patch；`Settings::default()` `llm_polish_enabled: Some(true)` 反映 Decision #7（**注意：default ON、但 has_credential gate 在 frontend chunk 2 處理**）
- 6 新 settings tests（含 F4 override field 存取 / multi-byte custom prompt 1000 chars / sparse patch / load legacy JSON / serialize default）

**Frontend files changed**:
- `src/stores/useVoiceFlowStore.ts`:
  - F1 `VoiceFlowStatus` 加 `'enhancing'`（chunk 0 已加 import 自 events.ts、本 chunk 用上 in handleStop）
  - **F21** `handleStart` capture `polishEnabledAtStart` snapshot
  - **F22** `handleStop` polish 路徑：
    ```
    if polishEnabledAtStart:
      keyExists = await invoke('has_credential', { provider })
      if !keyExists: pasteRaw + transitionTo('success'); return
      transitionTo('enhancing')
      try { polishedText = await invoke('polish_text') }
      catch { polishWarning=true; polishedText = result.rawText }  // F25 lifecycle
    else: pasteRaw + transitionTo('success')
    ```
  - **F25** `polishWarning` ref + transitionTo 在 `next === 'idle' || 'recording'` 時清掉
  - **F23** ESC during enhancing → no-op + console.warn（M6 不接 cancel）
  - **F24** polish_busy `Busy` error 同等 polish failure → fallback raw + emit
  - **Decision #8** `SUCCESS_LINGER_MS = 1500`
- 7 新 vitest tests:
  - polish ON + key → enhancing → success
  - polish ON + no key → silent skip enhancing → success（F22）
  - polish OFF → 直接 success（不 invoke polish_text）
  - polish failure → polishWarning=true、paste raw、success（不 error）
  - polish 失敗 NOT trigger 3s error linger
  - settings undefined → treated as polish ON
  - F25 polishWarning 在新 recording session 清掉

**i18n keys added**（chunk 2 部分、chunk 4 完整補齊）:
- `hud.aria.enhancing` / `hud.enhancing`
- `hud.warning.polishFailed`

**Reviewer checklist**:
- [ ] F22 has_credential gate path 走通、無 key 時無 fallback warning event
- [ ] F21 polishEnabledAtStart snapshot 正確、settings mid-flow 改不影響 in-flight
- [ ] F23 ESC during enhancing 不 cancel polish（console.warn only）
- [ ] F25 polishWarning 在 idle / recording transition 清掉
- [ ] `SUCCESS_LINGER_MS === 1500`
- [ ] `formatError` 不 call for polish errors（silent console.warn）
- [ ] vue-tsc 綠、vitest 綠

---

### 🎨 Chunk 3 — HUD enhancing + warning overlay + Dashboard sidebar enhancing badge（1d）

**Deps**：chunks 0 + 2.

**Frontend files changed**:
- `src/components/HudOverlay.vue` — 加 enhancing branch + success bubble dual-mode（Decision #5）+ click-through enhancing=ON（F27）
- `src/components/HudFlowBadge.vue` — F26 加 enhancing visual（amber dot + label）
- `src/components/HudSpinner.vue` — reused（reduced-motion fallback `…` 不變）

**i18n keys added**:
- `sidebar.enhancingBadge`
- F28 `hud.aria.transcribing` 與 `hud.aria.enhancing` 故意 wording 不同（assert by Vitest）

**Test plan**:
- 4 新 HudOverlay 測試（enhancing mounts spinner + label / success-warning shows AlertTriangle / success-no-warning shows CheckCircle2 / ariaMessage 5 states cover）
- 2 新 HudFlowBadge 測試（enhancing → amber dot / non-recording-non-enhancing → hidden）
- 1 F28 ARIA 文字差異 assertion test
- Playwright vite-shape 7 screenshots：idle / recording / transcribing / **enhancing**（NEW）/ success-ok / **success-warning**（NEW）/ error
- 加 `__hudDev.setPolishWarning(true)` helper 給 Playwright 切 success-warning state

**Reviewer checklist**:
- [ ] F26 enhancing dot color amber、recording dot red、視覺區分
- [ ] F27 click-through state matrix `enhancing=ON`（HudOverlay.vue:syncClickThrough switch 看一眼）
- [ ] F28 ARIA wording 差 by more than punctuation（vitest assert）
- [ ] Playwright 7 screenshots in `.playwright-mcp/m6-hud-*.png`、reviewer Read 過
- [ ] `grep fonts.googleapis.com src/assets/index.css` 空
- [ ] 5 visual states 都 work（M5 4 + M6 1）、無 regression

---

### 🛠️ Chunk 4 — SettingsLlmPolishSection + 4-provider test + dataflow indicator（1.5d）

**Deps**：chunks 0–3.

**Frontend files added/changed**（**Decision #3 + #5 + #7 cascade**）:
- `src/components/SettingsLlmPolishSection.vue` (NEW、~400 LOC、加 retry toggle + upgrade banner 略增) — F29 + F30 + F31 + F34 + F35:
  - polish toggle + provider Select（4 free provider）+ model Select + preset RadioGroup + custom prompt Textarea
  - **retry toggle（F34）**：「失敗時自動重試 1 次」 default ON、tri-state semantic（None=ON、Some(false)=OFF）
  - **per-step data-flow indicator**（F29 頂部 always-visible、polish OFF render `Audio → Groq Whisper → Paste`、polish ON render `Audio → Groq Whisper → <provider> Polish → Paste（無 retention）`）
  - **no-key warning banner**（F22）：polish ON + no key 時顯示 yellow banner「未設定 [provider] API key、潤飾路徑會跳過」
  - **M5→M6 upgrade banner（F35）**：first load 顯示 dismiss-able warning + localStorage flag `talktype:m6_upgrade_seen`
  - test polish button（F30 i18n-aware sample text）— 不走 retry（Decision #6）
  - F31 toggle OFF：only test polish button disabled、其他 controls enabled
- `src/views/SettingsView.vue` — mount `<SettingsLlmPolishSection />`
- `src/components/SettingsApiKeySection.vue` — flip `active: true` for openrouter/nvidia/gemini（在 `src/lib/providers.ts`，**Decision #3**）。M6 不啟用 openai/anthropic（保留 inactive、v0.2 才開）
- `src/lib/providers.ts` — `LLM_MODEL_LIST` mirror Rust registry（4 free provider × 2 model = 8 models）+ `LLM_PROVIDERS` flip 4 free active（OpenAI / Anthropic 仍 inactive、UI hidden）
- `src/components/ProviderPrivacyDialog.vue` — 加 3 provider body templates（**OpenRouter / NVIDIA NIM / Gemini**、不是 OpenAI / Anthropic）

**i18n keys added** — F32 完整 ~52 keys × 2 locales = ~104 entries（含 F34 retry + F35 upgrade banner）

**Test plan**:
- 4 SettingsLlmPolishSection vitest（provider switch / charCount destructive / toggle disable test button only / dataflow updates reactive）
- 1 providers.ts test（LLM_MODEL_LIST 與 Rust registry length match ≥ 8）
- 1 ProviderPrivacyDialog test（4 provider distinct bodies）
- Playwright vite-shape：
  - 4 dataflow indicator state（4 provider 切換）
  - polish OFF（greyed test button + indicator without polish step）
  - polish ON + no key（yellow banner visible）
  - polish ON + has key（all controls active）
  - custom prompt textarea visible

**Reviewer checklist**:
- [ ] F33 `grep fonts.googleapis.com src/assets/index.css` 空（chunk 4 若新 add shadcn-vue 元件 implementer 必 revert）
- [ ] F29 dataflow indicator 在 polish OFF 也 render（不含 polish step）
- [ ] F30 test sample text 用 i18n key 不 hardcode
- [ ] F31 toggle OFF 時 provider/model/preset/custom 仍 enabled
- [ ] F32 ~100 i18n keys 全到位、zh-TW + en 對齊
- [ ] `grep -rE "get_credential" src/` 空（不 regression）
- [ ] LOC SettingsLlmPolishSection ≤ 360
- [ ] 4 provider key 都 saveable + testable via UI（manual smoke 寫進 session log）

---

### 📚 Chunk 5 — Docs + acceptance + session log（0.5d）

**Deps**：chunks 0–4.

**Files updated**:
- `docs/m6-acceptance.md` (NEW) — manual SOP 14+ 條
- `.claude/PROGRESS.md` — M6 status row
- `.claude/IDEAS.md` — chunk reviewer findings + retro challenger findings 增量 append
- `.claude/sessions/2026-05-XX-m6-llm-polish.md` (NEW、若不同日新檔) — session log mirror M5
- `doc/plans/02-implementation-roadmap.md` — M6 acceptance 打勾 + 「最後更新」bump
- `doc/plans/01-architecture.md` — `polish_text` command + `polish:failed-fallback` event 已 documented (M6 plan 早 fold)、確認 Voice Flow State Machine 圖含 enhancing 行
- `doc/plans/05-data-model.md` — confirm Settings v1 ≧6 LLM fields documented

**Acceptance criteria**:
- ✅ 5 preset modes 各測同 input → 不同 polish 風格
- ✅ 4-provider test connection 全綠（real key dogfood）
- ✅ Polish latency p50：Groq ≤ 2s、其他 ≤ 5s（real measurement）
- ✅ 全 acceptance 6 條（spec line 451-459）打勾

## 預估時間 & dependency

```
chunk 0 (0.5d) → chunk 1 (2d) → chunk 2 (1d) → ┬→ chunk 3 (1d) ──┐
                                                └→ chunk 4 (1.5d) ┴→ chunk 5 (0.5d)

Total: 0.5 + 2 + 1 + max(1, 1.5) + 0.5 = 5.5 days、fits 1.5 週 budget
```

Critical path：chunks 0 → 1 → 2 strictly sequential。Chunks 3 + 4 parallelizable post-chunk-2（不同 file、無 shared edit）。

## 接下來主 session 動作

1. ✅ **本檔 commit 進 git**（plan refinement、reversible documentation only）
2. ✅ **append IDEAS.md** P2 findings（已列在「P2 challenger findings」section）
3. ✅ **bump PROGRESS.md** M6 status 📋 → 🚧 in-progress + 連結本 session log
4. ✅ **8 decisions all confirmed by user 2026-05-06**（包含 Decision #3 改 4 free providers、Decision #5 加 retry toggle、Decision #7 改 tri-state）
5. **dispatch chunk 0 implementer subagent（Opus）** 用本檔 chunk 0 section 為 prompt（next action）
6. **chunk 0 完工 → reviewer subagent → chunk 1 implementer → ...**

## Subagent dispatch 模板（給未來 main session 用）

每 chunk 完成（implementer subagent return）後：
1. Main session 讀 implementer subagent's commit + diff、確認 acceptance criteria
2. Dispatch reviewer subagent（Opus、`general-purpose`）prompt 含本檔對應 chunk 的「Reviewer checklist」+ 「Acceptance criteria」+ 跑 Playwright SOP（chunks 3 + 4 必跑）
3. Reviewer P0 必修（block chunk completion）/ P1 fold 進下 chunk / P2 → IDEAS append
4. Chunk 1 (Rust) 是 critical reviewer：API key invariant 任何 violation 立即 P0
5. Chunk 5 後 dispatch retro challenger（CLAUDE.md item 6）：focus Privacy / API key invariant / Typeless 隱私翻車反思

## Cross-chunk concerns 摘要

1. Error taxonomy 3 enums 不抽（M9 候選、Plan §4 #1）
2. `pub use` Rust re-export 不用：直接 `plugins::llm_polish::polish_text` in `generate_handler!`（IDEAS M3 chunk 3 教訓）
3. Custom prompt validation 雙層：UI (chunk 4) + Rust (chunk 1)
4. Vocabulary cap shared helper `plugins/vocabulary.rs` (F10、chunk 1)
5. polish_busy RAII drop guard (F15、chunk 1)
6. polishWarning ref lifecycle (F25、chunk 2)
7. Settings cache vs authoritative read (chunk 2 註解 race window doc)
8. emitTo("main-window") 對 enhancing payload status 已 forward-compat (F1 chunk 0 type extend)
9. CSP fix in chunk 0 為 future-proof（reqwest Rust-side 不受 CSP 影響、defensive）
10. PolishError::Disabled 防 frontend bypass（F22 雙保險）
11. Rapid double-press during enhancing (F24 + polish_busy RAII)
12. shadcn-vue Google Fonts trap regression watch (F33 + chunk 4 reviewer grep)

## 完整 challenger findings reference

Plan-time challenger output 完整 45 findings 留在 main session memory（Agent tool result）。本檔已 fold P0/P1 進 chunks、列 P2 為 IDEAS append 候選。實際 IDEAS.md append 在 chunk 5 收尾時做（避免 P2 list 與 chunks 1-4 reviewer 找的 chunks-level findings 重複）。

## Plan source agentId 紀錄

- Plan subagent: dispatched 2026-05-06、output 6 chunks + 8 open decisions + cross-chunk concerns
- Plan-time challenger: agentId `a5769f31fdf47c8c7`、output 45 findings A-J 9 區（A=5、B=8、C=7、D=4、E=5、F=4、G=1、H=3、I=3、J=6 = 46 actually、含 J45 刪 LockPoisoned）
