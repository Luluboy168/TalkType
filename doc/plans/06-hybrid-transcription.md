# Hybrid Transcription 設計

> **狀態**：Draft v1
> **最後更新**：2026-05-02

如何在 Cloud (Groq) 與 Local (whisper.cpp) 之間切換、fallback、以及給使用者選擇。

## 設計目標

1. **使用者體驗**：「就 work」 — 大多數時候不用想 cloud vs local
2. **彈性**：使用者明確需要時可強制 cloud 或 local
3. **隱私**：可選擇 100% 本地、永不送音頻外網
4. **成本**：cloud free-tier 預設、超過時 graceful 提示
5. **離線可用**：無網路時自動 fallback 到 local（如有模型）

## 三種使用模式

### Mode 1: Cloud-only

> **「我隨時有網、想要最快、不在意隱私」**

- 使用者設 `whisperProvider = "groq"` + 有 Groq API key
- 無 local model 下載
- 無網路 → 顯示錯誤 "No internet, please connect or download a local model"

### Mode 2: Local-only（Privacy mode）

> **「我要完全離線、不送任何音頻到 cloud」**

- 使用者設 `whisperProvider = "local"` + 有下載 local model
- 無 API key 也 ok
- 完全本地推論（即使有網也不上 cloud）

### Mode 3: Hybrid (Auto-fallback)

> **「優先 cloud (快+品質好)、無網用 local」**

- 使用者設 `whisperProvider = "auto"`
- 同時設 Groq API key 與下載 local model
- 邏輯：
  1. 有網 + 有 API key → cloud
  2. 無網 → fallback 到 local（如有 model）
  3. 都無 → 錯誤訊息

## Settings 設計

### Phase 1 簡化版

只 ship Mode 1 + Mode 2，不做 Auto。理由：

- Mode 3 邏輯複雜（何時偵測無網？timeout 多少？）
- 使用者明確切換已足夠
- Phase 2 加 Auto 模式

```typescript
interface Settings {
  whisperProvider: 'groq' | 'local';
  whisperModelId: string;  // Cloud: 'whisper-large-v3-turbo'
                           // Local: 'ggml-base-q5_1'
  // ...
}
```

### Phase 2 升級為 Auto

```typescript
interface SettingsV2 {
  whisperProvider: 'groq' | 'local' | 'auto';
  whisperModelIdCloud: string;
  whisperModelIdLocal: string;
  // ...
}
```

## Rust 端 dispatch logic

### `src-tauri/src/plugins/transcription_dispatcher.rs`（新增）

```rust
pub async fn transcribe_audio(
    settings_state: State<'_, SettingsState>,
    cloud_state: State<'_, CloudTranscriptionState>,
    local_state: State<'_, LocalTranscriptionState>,
    audio_state: State<'_, AudioRecorderState>,
    credentials_state: State<'_, CredentialsState>,
    vocabulary: Option<Vec<String>>,
    language: Option<String>,
) -> Result<TranscriptionResult, TranscriptionError> {
    let settings = settings_state.get().await;
    
    match settings.whisper_provider.as_str() {
        "groq" => {
            transcription_cloud::transcribe(
                cloud_state, audio_state, credentials_state,
                &settings, vocabulary, language,
            ).await
        }
        "local" => {
            transcription_local::transcribe(
                local_state, audio_state, &settings, vocabulary, language,
            ).await
        }
        // Phase 2: "auto" branch
        _ => Err(TranscriptionError::UnknownProvider(settings.whisper_provider)),
    }
}
```

Frontend 只呼叫一個 command：`transcribe_audio`。Rust 內部 dispatch。

## Cloud Path：Groq Whisper

### 流程

```
WAV bytes (in AudioRecorderState::wav_buffer)
    ↓ take()
驗證 size: 1KB ≤ size ≤ 25MB
    ↓
從 keyring 讀 Groq API key
    ↓ 無 key → ApiKeyMissing error
建 multipart form:
  - file (audio/wav)
  - model: settings.whisperModelId
  - response_format: verbose_json
  - language: settings.languageTranscription (None = auto)
  - prompt: vocabulary terms (top 50)
    ↓
POST https://api.groq.com/openai/v1/audio/transcriptions
  Authorization: Bearer <key>
  120s timeout
    ↓ 4xx/5xx → ApiError
解析 verbose_json:
  - text
  - segments[].no_speech_prob → min() = noSpeechProbability
    ↓
TranscriptionResult { rawText, transcriptionDurationMs, noSpeechProbability }
```

### Phase 2：多 cloud provider

新加：

- OpenAI Whisper（同 OpenAI compat API）
- Deepgram（不同 API 形狀）
- AssemblyAI

抽象 mirror SayIt 的 `llmProvider.ts` pattern：

```typescript
interface WhisperProviderConfig {
  id: WhisperProviderId;
  baseUrl: string;
  consoleUrl: string;
  apiKeyHeaderStyle: 'bearer' | 'x-api-key' | 'query';
  buildRequestBody: (audio: Blob, opts: WhisperRequestOpts) => FormData | Blob;
  parseResponse: (resp: any) => TranscriptionResult;
}
```

## Local Path：whisper.cpp

### Spike 階段（M7 開始時）

選 Rust binding：

#### 候選 1: `whisper-rs`

```toml
whisper-rs = "0.14"
```

優：

- 較成熟、社群活躍
- API 穩
- 自動 build whisper.cpp

缺：

- 需要 CMake、C++ toolchain
- Windows MSVC 配置可能踩雷

#### 候選 2: `whisper-cpp-2`

較新版本、可能更靈活但 less proven。

#### 候選 3: 直接 FFI

最大控制，最大工作量。

**Phase 1 嘗試順序**：先 `whisper-rs` → 失敗就 `whisper-cpp-2` → 都不行就 FFI。

### 流程

```
WAV bytes (in AudioRecorderState::wav_buffer)
    ↓ take()
驗證 model loaded:
    if let None = ctx { load_model(model_path)? }
    ↓
WAV bytes → f32 samples (16kHz mono):
    let samples = parse_wav(&wav_bytes)?;
    ↓
WhisperContext.full(params, &samples):
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(num_cpus::get_physical() as i32);
    params.set_language(Some(language.unwrap_or("auto")));
    params.set_translate(false);
    params.set_print_progress(false);
    params.set_initial_prompt(&vocabulary_prompt);
    ctx.full(params, &samples)?;
    ↓
集合 segments → 完整 text:
    let mut text = String::new();
    for i in 0..ctx.full_n_segments()? {
        text.push_str(&ctx.full_get_segment_text(i)?);
    }
    ↓
TranscriptionResult { rawText: text, transcriptionDurationMs, noSpeechProbability: None }
```

### Lazy load 模型

```rust
pub struct LocalTranscriptionState {
    context: Arc<Mutex<Option<WhisperContext>>>,
    current_model_id: Arc<Mutex<Option<String>>>,
}

fn ensure_model_loaded(state: &State<LocalTranscriptionState>, model_id: &str) -> Result<()> {
    let mut ctx = state.context.lock().unwrap();
    let mut current = state.current_model_id.lock().unwrap();
    
    if current.as_deref() == Some(model_id) {
        return Ok(()); // Already loaded
    }
    
    // Unload old
    *ctx = None;
    
    // Load new
    let path = model_path(model_id)?;
    let new_ctx = WhisperContext::new_with_params(path.to_str().unwrap(), WhisperContextParameters::default())?;
    
    *ctx = Some(new_ctx);
    *current = Some(model_id.to_string());
    
    Ok(())
}
```

### 進度 event

```rust
ctx.set_progress_callback(|progress: i32| {
    app.emit("transcription:progress", progress).ok();
});
```

Frontend listen 顯示 spinner percentage。

## 模型下載 UX

### Settings 頁面

```
[ Whisper provider ]  [▼ Cloud (Groq)            ]
                      [  Cloud (Groq)             ]
                      [  Local (offline)          ]

(如選 Local:)
[ Available models ]
  ☑ ggml-base-q5_1 (60 MB) — Downloaded ✓     [Delete]
  ☐ ggml-tiny-q5_1 (30 MB) — Not downloaded   [Download]
  ☐ ggml-small-q5_1 (180 MB) — Not downloaded [Download]

[ Currently using ]   [▼ ggml-base-q5_1          ]
```

### Download 流程

```rust
#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    let info = MODEL_REGISTRY.get(&model_id).ok_or("Unknown model")?;
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        info.file_name
    );
    
    let dest = models_dir(&app)?.join(&info.file_name);
    let dest_tmp = dest.with_extension("tmp");
    
    let client = reqwest::Client::new();
    let mut resp = client.get(&url).send().await?;
    let total = resp.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    
    let mut file = tokio::fs::File::create(&dest_tmp).await?;
    let mut hasher = Sha256::new();
    
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        
        // Throttle progress events to ~10/sec
        app.emit("model:download-progress", DownloadProgress {
            model_id: model_id.clone(),
            downloaded,
            total,
        }).ok();
    }
    
    // Verify SHA-256
    let hash = format!("{:x}", hasher.finalize());
    if hash != info.sha256 {
        tokio::fs::remove_file(&dest_tmp).await?;
        return Err(format!("SHA-256 mismatch: expected {}, got {}", info.sha256, hash));
    }
    
    // Atomic rename
    tokio::fs::rename(&dest_tmp, &dest).await?;
    
    // Update local_models table (Phase 2)
    
    Ok(())
}
```

### Resume support（Phase 2）

用 HTTP `Range` header：

```rust
let existing_size = if dest_tmp.exists() {
    tokio::fs::metadata(&dest_tmp).await?.len()
} else {
    0
};

let mut req = client.get(&url);
if existing_size > 0 {
    req = req.header("Range", format!("bytes={}-", existing_size));
}
```

## Model registry

### `src/lib/whisperModelRegistry.ts`

```typescript
export interface WhisperModelInfo {
  id: string;
  displayName: string;
  fileName: string;
  fileSizeMb: number;
  sha256: string;
  url: string;
  recommendedFor: 'fast' | 'balanced' | 'high-quality';
  estimatedSpeed: string;  // e.g., "1x realtime on CPU"
}

export const WHISPER_MODELS: Record<string, WhisperModelInfo> = {
  'ggml-base-q5_1': {
    id: 'ggml-base-q5_1',
    displayName: 'Base (quantized)',
    fileName: 'ggml-base-q5_1.bin',
    fileSizeMb: 60,
    sha256: '...',  // 真實 hash 在實作時填
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin',
    recommendedFor: 'balanced',
    estimatedSpeed: '~1.5x realtime on modern CPU',
  },
  // Phase 2 加更多
};
```

## Cloud vs Local 比較表

| 維度 | Cloud (Groq Whisper) | Local (whisper.cpp base-q5_1) |
|---|---|---|
| **延遲（10s audio）** | ~600ms | ~3000ms (CPU) / ~1000ms (GPU、未實作) |
| **正確率** | 高（large-v3 model） | 中（base model） |
| **網路** | 必要 | 不需要 |
| **API key** | 必要 | 不需要 |
| **隱私** | 音頻送到 Groq US server | 完全本地 |
| **成本** | 免費 tier 有限制 | 免費（一次下載） |
| **Binary 大小** | +0 MB | +60 MB（model 檔） |
| **多語言** | 高品質 99 種 | 中品質 99 種 |
| **特殊術語** | 透過 vocabulary prompt | 同 |

## 跨 provider 一致的 vocabulary biasing

兩種 provider 都接 `vocabulary` 參數（top-N 詞彙）：

- **Cloud (Groq)**：放 `prompt` field：「Important Vocabulary: t1, t2, ...」
- **Local (whisper.cpp)**：放 `initial_prompt` 同樣格式

Format 一致 → 程式碼好維護。

## Error handling

### Cloud-specific errors

| Error | 觸發 | UI 顯示 | 自動 action |
|---|---|---|---|
| `ApiKeyMissing` | Keyring 讀不到 | "Please set Groq API key in Settings" | Open settings |
| `ApiError(401)` | Key 無效 | "Invalid API key" | Open settings |
| `ApiError(429)` | Rate limit | "Rate limited, please wait" | Show wait time |
| `RequestFailed` | Network error | "Network error, check connection" | Suggest local fallback (Phase 2) |
| `FileTooLarge` | > 25 MB | "Audio too long, max 25 MB" | — |

### Local-specific errors

| Error | 觸發 | UI 顯示 | 自動 action |
|---|---|---|---|
| `ModelNotFound` | model 沒下載 | "Model not downloaded" | Open settings → download |
| `ModelLoadError` | 模型檔損毀 | "Model corrupted, please re-download" | — |
| `WhisperRuntimeError` | whisper.cpp 推論錯 | "Local transcription failed" | Suggest cloud fallback |

## Phase 2 enhancement：Auto mode + intelligent fallback

```rust
// Phase 2 logic
async fn transcribe_auto(...) -> Result<...> {
    let prefer_cloud = has_credential("groq")? && has_internet().await;
    
    if prefer_cloud {
        match transcription_cloud::transcribe(...).await {
            Ok(result) => Ok(result),
            Err(TranscriptionError::RequestFailed(_)) if has_local_model() => {
                // Network failed mid-call, try local
                emit_event("transcription:fallback", "cloud→local").ok();
                transcription_local::transcribe(...).await
            }
            Err(e) => Err(e),
        }
    } else if has_local_model() {
        transcription_local::transcribe(...).await
    } else {
        Err(TranscriptionError::NoProviderAvailable)
    }
}
```

## 對 SayIt 改進總結

| SayIt | TalkType |
|---|---|
| Groq-only transcription | Hybrid Cloud + Local |
| 無 offline 故事 | Local-only privacy mode |
| 無 model download UX | Built-in download with progress + SHA verify |
| API key per-call from frontend | Rust-side keyring read |
| 單一 transcribe command | Dispatcher pattern (cloud/local 透明) |

## 連結

- 架構 → [`01-architecture.md`](01-architecture.md)
- Rust modules → [`03-rust-modules.md`](03-rust-modules.md)
- Data model → [`05-data-model.md`](05-data-model.md)
- Implementation roadmap M7 → [`02-implementation-roadmap.md#m7-local-whispercpp-整合`](02-implementation-roadmap.md)
- SayIt transcription reference → [`../reference/sayit-backend-analysis.md#48-transcriptionrs-groq-whisper`](../reference/sayit-backend-analysis.md)
