# Frontend 結構

> **狀態**：Draft v1
> **最後更新**：2026-05-02

Vue 3 + TypeScript + shadcn-vue 專案結構與 conventions。

## 目錄結構

```
src/
├── App.vue                        HUD root
├── MainApp.vue                    Dashboard root
├── main.ts                        HUD entry
├── main-window.ts                 Dashboard entry
├── router.ts                      Dashboard router (hash mode)
├── style.css                      Tailwind v4 + theme tokens
├── assets/
│   ├── logo.svg
│   └── ...
├── components/
│   ├── ui/                        shadcn-vue primitives (auto-generated)
│   ├── HudOverlay.vue             HUD root component
│   ├── HudWaveform.vue            6-bar waveform
│   ├── HudStateIcon.vue           idle/recording/transcribing/success/error icons
│   ├── Sidebar.vue                Dashboard sidebar
│   ├── DashboardHeader.vue
│   └── ApiKeyInput.vue            通用 component（masked input）
├── composables/
│   ├── useTauriEvents.ts          ★ 唯一 import @tauri-apps/api/event 的地方
│   ├── useFeedbackMessage.ts      Toast-like feedback
│   ├── useAudioPreview.ts         Mic preview RAF + lerp
│   ├── useAudioWaveform.ts        Waveform RAF + lerp
│   └── useSettings.ts             便利 wrapper：呼叫 invoke 與 listen settings:updated
├── i18n/
│   ├── index.ts                   createI18n config
│   ├── detectLocale.ts            navigator.languages → BCP 47
│   ├── prompts.ts                 LLM polish system prompts (per language)
│   └── locales/
│       ├── zh-TW.json
│       └── en.json
├── lib/
│   ├── utils.ts                   cn() class merger
│   ├── enhancer.ts                LLM polish 多 provider abstraction
│   ├── llmProvider.ts             4 provider 配置 + buildFetchParams
│   ├── modelRegistry.ts           static catalog of LLM + Whisper models
│   ├── apiPricing.ts              cost ceiling 計算（Phase 1 簡單版）
│   ├── errorUtils.ts              i18n-aware error message extractor
│   ├── formatUtils.ts             timestamp / duration / number 格式化
│   ├── keycodeMap.ts              DOM event.code → Windows VK
│   └── (Phase 2) sentry.ts        @sentry/vue 設定
├── stores/                        Pinia stores
│   ├── useVoiceFlowStore.ts       Voice flow state machine + orchestration
│   ├── useSettingsStore.ts        Frontend mirror of Rust settings + invoke wrappers
│   ├── useHistoryStore.ts         Read-through cache from Rust
│   └── useVocabularyStore.ts      Read-through cache from Rust
├── types/
│   ├── index.ts                   re-export everything
│   ├── settings.ts                Settings、HotkeyConfig、TriggerKey、TriggerMode
│   ├── transcription.ts           TranscriptionResult、TranscriptionRecord
│   ├── vocabulary.ts              VocabularyEntry
│   ├── audio.ts                   StopRecordingResult、AudioInputDeviceInfo
│   ├── events.ts                  All Tauri event payload types
│   └── llm.ts                     LlmProviderId、LlmModelConfig、PolishOptions
└── views/                         Route-level components
    ├── DashboardView.vue          / (簡單概覽)
    ├── HistoryView.vue            /history
    ├── DictionaryView.vue         /dictionary
    ├── SettingsView.vue           /settings (拆 sub-components 避免肥大)
    │   └── components/            (相對於 SettingsView)
    │       ├── SettingsHotkey.vue
    │       ├── SettingsTranscription.vue
    │       ├── SettingsLlm.vue
    │       ├── SettingsAudio.vue
    │       ├── SettingsApiKeys.vue
    │       ├── SettingsAppearance.vue
    │       └── SettingsAdvanced.vue
    └── FeatureGuideView.vue       /guide
```

## 雙 entry 設計

### `index.html` + `main.ts` (HUD)

```typescript
// src/main.ts
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import { initI18n } from './i18n';
import { initVoiceFlow } from './stores/useVoiceFlowStore';
import './style.css';

async function bootstrap() {
  const app = createApp(App);
  app.use(createPinia());
  app.use(await initI18n('hud'));  // 只載必要的 keys
  await initVoiceFlow();
  app.mount('#app');
}

bootstrap().catch(console.error);
```

### `main-window.html` + `main-window.ts` (Dashboard)

```typescript
// src/main-window.ts
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import MainApp from './MainApp.vue';
import { router } from './router';
import { initI18n } from './i18n';

async function bootstrap() {
  const app = createApp(MainApp);
  app.use(createPinia());
  app.use(router);
  app.use(await initI18n('dashboard'));
  app.mount('#app');
  
  // 停用瀏覽器 right-click context menu
  document.addEventListener('contextmenu', e => e.preventDefault());
}

bootstrap().catch(console.error);
```

### Vite multi-entry config

```typescript
// vite.config.ts
export default defineConfig({
  plugins: [vue(), tailwindcss()],
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        'main-window': resolve(__dirname, 'main-window.html'),
      },
    },
  },
  resolve: {
    alias: { '@': resolve(__dirname, './src') },
  },
  server: { port: 1420, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_'],
  define: {
    __APP_VERSION__: JSON.stringify(version),
  },
});
```

## State Management Strategy

### Settings Store — 對 SayIt 的關鍵改進

SayIt 把 settings 全持有在 Pinia + `tauri-plugin-store`。TalkType 改為：**Rust 是 source of truth、Pinia 是 read-through cache**。

```typescript
// src/stores/useSettingsStore.ts
export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings | null>(null);
  const loading = ref(true);
  
  async function load() {
    settings.value = await invoke<Settings>('get_settings');
    loading.value = false;
  }
  
  async function update(patch: Partial<Settings>) {
    await invoke('update_settings', { patch });
    // Don't optimistically update — wait for settings:updated event
  }
  
  // Subscribe to Rust event
  listenToEvent(SETTINGS_UPDATED, (newSettings: Settings) => {
    settings.value = newSettings;
  });
  
  return { settings: readonly(settings), loading: readonly(loading), load, update };
});
```

效益：

- ✅ HUD 與 Dashboard 自動同步（Rust event 廣播）
- ✅ 沒有 race condition
- ✅ Pinia store 變超薄（< 100 行 vs SayIt 1395）
- ✅ 不用 emit cross-window event

### History / Vocabulary Stores

同樣 pattern：read-through cache。

```typescript
export const useHistoryStore = defineStore('history', () => {
  const transcriptionList = ref<TranscriptionRecord[]>([]);
  const hasMore = ref(true);
  let offset = 0;
  
  async function loadNext(pageSize = 20, search?: string) {
    const page = await invoke<TranscriptionRecord[]>('get_history_paged', {
      offset, limit: pageSize, search,
    });
    transcriptionList.value.push(...page);
    offset += page.length;
    hasMore.value = page.length === pageSize;
  }
  
  async function reset(search?: string) {
    transcriptionList.value = [];
    offset = 0;
    hasMore.value = true;
    await loadNext(20, search);
  }
  
  // Auto-refresh on new history
  listenToEvent(HISTORY_ADDED, (record: TranscriptionRecord) => {
    transcriptionList.value.unshift(record);
  });
  
  return { transcriptionList, hasMore, loadNext, reset };
});
```

### Voice Flow Store — 業務邏輯主力

唯一比 SayIt 小不了多少的 store（核心 orchestration）。Phase 1 預算 ~600 行。

```typescript
export const useVoiceFlowStore = defineStore('voiceFlow', () => {
  const status = ref<HudStatus>('idle');
  const message = ref('');
  const recordingElapsed = ref(0);
  let recordingTimer: NodeJS.Timeout | null = null;
  
  async function handleStartRecording() {
    if (status.value !== 'idle') return;
    
    try {
      // 1. Capture target window (Windows-only)
      await invoke('capture_target_window');
      
      // 2. Mute system audio if enabled
      const settings = await getSettings();
      if (settings.muteOnRecording) {
        await invoke('mute_system_audio');
      }
      
      // 3. Start recording
      await invoke('start_recording', { deviceName: settings.audioInputDevice });
      
      // 4. Play start sound
      if (settings.soundEffectsEnabled) {
        await invoke('play_start_sound');
      }
      
      transitionTo('recording', '');
      startTimer();
    } catch (err) {
      handleError(err);
    }
  }
  
  async function handleStopRecording() {
    if (status.value !== 'recording') return;
    stopTimer();
    
    transitionTo('transcribing');
    
    try {
      const settings = await getSettings();
      const stopResult = await invoke('stop_recording');
      
      // Restore audio
      if (settings.muteOnRecording) {
        await invoke('restore_system_audio');
      }
      
      if (settings.soundEffectsEnabled) {
        await invoke('play_stop_sound');
      }
      
      // Transcribe
      const vocabulary = await getTopVocabularyTerms();
      const command = settings.whisperProvider === 'cloud' ? 'transcribe_cloud' : 'transcribe_local';
      const result = await invoke<TranscriptionResult>(command, { vocabulary });
      
      let finalText = result.rawText;
      
      // LLM polish
      if (settings.llmPolishEnabled) {
        transitionTo('enhancing');
        try {
          finalText = await enhanceText(result.rawText, settings, vocabulary);
        } catch (err) {
          // Fallback to raw
          console.warn('Polish failed, using raw text', err);
        }
      }
      
      // Paste
      await invoke('paste_text', { text: finalText });
      
      // Save history
      await invoke('add_transcription', {
        record: { rawText: result.rawText, processedText: finalText, ... },
      });
      
      transitionTo('success');
      setTimeout(() => transitionTo('idle'), 1000);
      
    } catch (err) {
      handleError(err);
    }
  }
  
  function transitionTo(newStatus: HudStatus, msg?: string) {
    status.value = newStatus;
    if (msg !== undefined) message.value = msg;
    emitToWindow('main-window', VOICE_FLOW_STATE_CHANGED, { status: newStatus });
  }
  
  // Subscribe to hotkey events
  listenToEvent(HOTKEY_PRESSED, handleStartRecording);
  listenToEvent(HOTKEY_RELEASED, handleStopRecording);
  listenToEvent(HOTKEY_TOGGLED, payload => {
    payload.action === 'Start' ? handleStartRecording() : handleStopRecording();
  });
  listenToEvent(ESCAPE_PRESSED, () => {
    if (status.value === 'recording') {
      handleCancel();
    }
  });
  
  return { status, message, recordingElapsed };
});
```

## Composables

### `useTauriEvents.ts` — 集中化 event API

```typescript
import { listen, emit, emitTo } from '@tauri-apps/api/event';

export const listenToEvent = listen;
export const emitEvent = emit;
export const emitToWindow = emitTo;

// Event constants — 對應 Rust 端
export const HOTKEY_PRESSED = 'hotkey:pressed' as const;
export const HOTKEY_RELEASED = 'hotkey:released' as const;
export const HOTKEY_TOGGLED = 'hotkey:toggled' as const;
export const HOTKEY_ERROR = 'hotkey:error' as const;
export const HOTKEY_RECORDING_CAPTURED = 'hotkey:recording-captured' as const;
export const HOTKEY_RECORDING_REJECTED = 'hotkey:recording-rejected' as const;
export const ESCAPE_PRESSED = 'escape:pressed' as const;
export const AUDIO_WAVEFORM = 'audio:waveform' as const;
export const AUDIO_PREVIEW_LEVEL = 'audio:preview-level' as const;
export const TRANSCRIPTION_PROGRESS = 'transcription:progress' as const;
export const MODEL_DOWNLOAD_PROGRESS = 'model:download-progress' as const;
export const SETTINGS_UPDATED = 'settings:updated' as const;
export const HISTORY_ADDED = 'history:added' as const;
export const VOCABULARY_CHANGED = 'vocabulary:changed' as const;

// Frontend-only
export const VOICE_FLOW_STATE_CHANGED = 'voice-flow:state-changed' as const;
```

**規則**：所有 `listen` / `emit` 必須透過這個 file。如果有人直接 import `@tauri-apps/api/event`，ESLint rule 該 fail（Phase 2 加 custom rule）。

### `useAudioWaveform.ts` & `useAudioPreview.ts`

學 SayIt 的 RAF + lerp pattern，每秒 60 / 30 fps interpolate value to target。

## shadcn-vue Conventions

學 SayIt CLAUDE.md 的「禁忌」：

| 需求 | ❌ 禁止 | ✅ 必須 |
|---|---|---|
| 側邊欄 | 手寫 `<nav>` | `SidebarProvider` + `Sidebar` + `SidebarMenu` |
| 按鈕 | 原生 `<button>` + 手寫樣式 | `<Button>` + variant prop |
| 表單 | 原生 input/select | `Input` / `Select` / `Textarea` |
| 表格 | 原生 `<table>` | `Table` 系列 |
| 開關 | 原生 checkbox | `Switch` |
| Radio group | 原生 radio | `RadioGroup` + `RadioGroupItem` |

### 樣式規則

- 語意色彩優先：`bg-card`、`text-foreground`、`border-border`
- 禁止硬編碼：`bg-zinc-900`、`text-white`、`border-zinc-700`
- 覆蓋元件樣式時只微調（padding、size），不改色彩

### 元件 API

- variant 優先：`variant="destructive"` 而非 `class="text-destructive border-destructive"`
- Switch 綁定：`:model-value` + `@update:model-value`（不是 `:checked`）
- Select 綁定：`:model-value` + `@update:model-value`
- Label 必須加 `for`、控制項加 `id`
- 用 `lucide-vue-next` 不用 `@tabler/icons-vue`

## Type 命名慣例

| Suffix | 用途 | 範例 |
|---|---|---|
| `*Payload` | Tauri Event payload | `WaveformPayload`、`HotkeyEventPayload` |
| `*Record` | Database row | `TranscriptionRecord`、`VocabularyEntry` (這個是 *Entry) |
| `*Config` | Settings 物件 | `HotkeyConfig` |
| `*Entry` | Dictionary item | `VocabularyEntry` |
| `*Result` | API/IPC return | `TranscriptionResult`、`StopRecordingResult` |
| `*Patch` | Partial update | `SettingsPatch` |

### 禁止 `any` / `unknown` 在 IPC boundary

```typescript
// ❌ 禁止
const result = await invoke('get_settings');

// ✅ 必須
const result = await invoke<Settings>('get_settings');
```

## i18n Strategy

### 範圍

- Phase 1：zh-TW + en
- Phase 2：+ zh-CN、ja、ko

### Pattern

```json
// src/i18n/locales/zh-TW.json
{
  "settings": {
    "hotkey": {
      "title": "熱鍵",
      "trigger_key": "觸發按鍵",
      "trigger_mode_hold": "按住說話",
      "trigger_mode_toggle": "按一下開始 / 結束"
    }
  }
}
```

### UI vs Transcription 語言分離

學 SayIt：

- `language_ui` — i18n locale
- `language_transcription` — Whisper 轉錄語言（可獨立、預設 auto）

## Vite 設定

```typescript
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';
import { readFileSync } from 'fs';

const { version } = JSON.parse(readFileSync('./package.json', 'utf-8'));

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        'main-window': resolve(__dirname, 'main-window.html'),
      },
    },
    sourcemap: process.env.VITE_SENTRY_SOURCEMAPS_ENABLED === 'true',  // Phase 2
  },
  resolve: {
    alias: { '@': resolve(__dirname, './src') },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || false,
  },
  watch: {
    ignored: ['**/src-tauri/**'],
  },
  envPrefix: ['VITE_', 'TAURI_'],
  define: {
    __APP_VERSION__: JSON.stringify(version),
  },
});
```

## TypeScript 設定

```json
{
  "compilerOptions": {
    "target": "ES2021",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "esModuleInterop": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "resolveJsonModule": true,
    "noEmit": true,
    "jsx": "preserve",
    "lib": ["ES2021", "DOM", "DOM.Iterable"],
    "types": ["vite/client", "vitest"],
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src/**/*", "tests/**/*"],
  "exclude": ["node_modules", "dist", "src-tauri"]
}
```

## ESLint 設定

學 SayIt 的 flat config：

```javascript
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import pluginVue from 'eslint-plugin-vue';

export default [
  { ignores: ['src/components/ui/**', 'dist/**', 'src-tauri/**'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],
  {
    rules: {
      'vue/multi-word-component-names': 'off',
      '@typescript-eslint/no-unused-vars': 'off',  // 交給 vue-tsc
      '@typescript-eslint/no-explicit-any': 'warn',
      'no-undef': 'off',
    },
  },
];
```

## Vitest 設定

```typescript
import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'path';

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['tests/unit/**/*.test.ts', 'tests/component/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.ts', 'src/**/*.vue'],
      exclude: ['src/main.ts', 'src/main-window.ts', '**/*.d.ts'],
    },
  },
  resolve: {
    alias: { '@': resolve(__dirname, './src') },
  },
});
```

## Pinia Pattern

只用 composition style：

```typescript
export const useFooStore = defineStore('foo', () => {
  const state = ref(...);
  function action() { ... }
  return { state, action };
});
```

不用 Options style。

## 連結

- 架構 → [`01-architecture.md`](01-architecture.md)
- Rust modules → [`03-rust-modules.md`](03-rust-modules.md)
- Data model → [`05-data-model.md`](05-data-model.md)
- SayIt frontend reference → [`../reference/sayit-frontend-analysis.md`](../reference/sayit-frontend-analysis.md)
