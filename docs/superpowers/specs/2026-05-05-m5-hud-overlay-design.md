# M5 — HUD Overlay 設計 Spec

> **狀態**：Approved v1（brainstorming 完、user 通過、進 writing-plans）
> **最後更新**：2026-05-05
> **前提**：M0–M4 全部完工 merged。`useVoiceFlowStore` 已 emit 4 logical states；`useAudioWaveform` 已輸出 6-band smoothed levels；HUD `WS_EX_NOACTIVATE` 已套（M4 chunk 2）。
> **後續**：`docs/superpowers/plans/2026-05-05-m5-hud-overlay-plan.md`（writing-plans skill 產出）

## TL;DR

完成 M1 留下的 HUD overlay placeholder。HUD 顯示 4 個 visual states（recording / transcribing / success / error）配 6-bar waveform、spinner、icon、auto-hide、accessibility（ARIA + `prefers-reduced-motion`）、active-monitor 定位、dev visibility tooling。Dashboard sidebar 加最小 voice flow indicator。

5 chunks（sequential），實作期間 plan-time challenger 平行 dispatch。Reviewer + retro challenger 都跑 Playwright 截 interactive states 比對 spec。

## 1. Scope

### 1.1 In scope

- **HUD components**（4 個 .vue 檔，每檔 < 200 LOC）
  - `HudOverlay.vue` — orchestrator（state machine、ARIA、reduced-motion、click-through 控制、click-to-dismiss）
  - `HudWaveform.vue` — 6-bar visualizer（接 `useAudioWaveform`）
  - `HudSpinner.vue` — transcribing 旋轉 icon
  - `HudTimer.vue` — `mm:ss` + cap warning color
- **`useVoiceFlowStore` 微調**
  - `init()` 改 `async`、await 全部 listener registration
  - 移除 `paste:focus-restore-failed` listener（redundant；handleStop catch 已處理）
  - 加 `dismissError()` method
  - 加 `audio:recording-aborted` listener
  - `transitionTo()` emit cross-window event `voice-flow:state-changed`
- **HUD positioning**
  - 新 Rust command `position_hud_for_active_monitor()` — cursor 所在 monitor center-x、y=50 logical
  - 在 `handleStart` 之前 invoke
- **Dev tooling**
  - 新 Rust command `set_hud_visible_for_dev(visible: bool)`，`#[cfg(debug_assertions)]` only
- **Dashboard sidebar indicator**
  - 新 component `HudFlowBadge.vue`、`status === 'recording'` 顯示紅點 + 「錄音中」、否則 hidden
  - Listen `voice-flow:state-changed` cross-window event
- **HUD window 大小調整**
  - `tauri.conf.json` HUD window 固定 380×56 logical（解決 click-through-off 時「點透明區也算點 HUD」問題）
- **i18n**
  - 新 keys：`hud.aria.*`、`hud.transcribing`、`hud.recording`、`hud.success`、`hud.error.*`、`sidebar.recordingBadge`
- **Accessibility**
  - `<div role="status" aria-live="polite" aria-label="...">` 在 HudOverlay 根
  - state 切換 announce 對應 message（i18n key）
  - `prefers-reduced-motion: reduce` 完整 fallback（見 §5.2）
- **Documentation**
  - `docs/m5-acceptance.md`、含 ≥ 12 acceptance conditions

### 1.2 Out of scope（明確 deferred）

| Item | Defer to | 理由 |
|---|---|---|
| `mute_on_recording` setting + 系統音訊 mute/restore | M8 | M8 Settings 整體拓展時順手做；user 確認 deferred |
| LLM polish wiring（`enhancing` state） | M6 | M6 owns LLM polish |
| History persist（`add_history` SQLite） | M8 | M8 owns DB |
| Sound effects | Phase 2 | Phase 2 polish |
| Per-app preset hooks | v0.2+ | Wispr Flow / Typeless 招牌、不在 Phase 1 |
| Mount-time state sync（Dashboard 開啟後當前 status 同步） | Phase 2 | 需 cross-window request/reply 機制；M5 接受「Dashboard 從下個 event 開始顯示」 |
| Settings UI dev visibility toggle | M9 polish | dev tool 可從 console invoke、不需 UI |

## 2. State → Visual mapping

| `useVoiceFlowStore.status` | HUD visible? | 顯示內容 | Click-through | Auto-hide timer |
|---|---|---|---|---|
| `idle` | hidden（HUD window `setIgnoreCursorEvents(true)` + Vue `v-if` 不 render bubble） | — | through | — |
| `recording` | visible | `<HudWaveform>` + `<HudTimer>` | through | persists（直到放熱鍵 / toggle off / ESC / cap abort） |
| `transcribing` | visible | `<HudSpinner>` + 「轉錄中…」label | through | persists（直到 transcribe 結果回） |
| `success` | visible | `<CheckCircle2>` (lucide-vue-next) + 「完成」label | through | 1s linger → fade idle |
| `error` | visible | `<XCircle>` + `store.message` text | **NOT through**（user 可點 dismiss） | 6s linger → fade idle、或 user click → 立即 idle |

### 2.1 Transition timing

- 各 state 進場 / 出場：`transition: opacity 200ms ease, transform 200ms ease`（fade + 輕微 scale 0.95→1）
- Vue 用 `<Transition mode="out-in" name="fade">` 包 inner state-bubble switch
- Reduced-motion ON：`transition: none`、scale 不套；fade-only via JS class swap

## 3. Position

### 3.1 Active-monitor logic

`handleStart` 流程：
1. `await invoke("position_hud_for_active_monitor")` （新 Rust command）
2. `await invoke("capture_target_window")`
3. `await invoke("start_recording", { deviceName: null })`
4. `transitionTo("recording", "")`

### 3.2 Rust 實作策略

優先用 Tauri 抽象、無法滿足才下沉 raw Win32：

```rust
// Pseudo-code outline; 實作細節由 implementer 決定
#[tauri::command]
async fn position_hud_for_active_monitor(app: AppHandle) -> Result<(), HudError> {
    // 1. 取 cursor 位置
    let cursor = get_cursor_position()?;  // raw Win32 GetCursorPos OR Tauri API
    // 2. 找 cursor 所在 monitor（available_monitors() 列舉、判斷 cursor 在哪個 rect）
    let hud = app.get_webview_window("main").ok_or(HudError::WindowMissing)?;
    let monitors = hud.available_monitors()?;
    let target = monitors.iter().find(|m| rect_contains(m, cursor)).unwrap_or(&monitors[0]);
    // 3. 算 HUD position：center-x of target monitor、y = 50 logical
    let pos = compute_position(target, hud_logical_size = (380, 56));
    hud.set_position(pos)?;
    Ok(())
}
```

### 3.3 DPI handling

Tauri webview 用 logical pixels、OS 自動 scale。Implementer 確保：
- `set_position` 用 logical position、Tauri 內部轉 physical
- HUD `tauri.conf.json` size 用 logical（`width: 380, height: 56`）
- Acceptance SOP 加 100% / 150% / 200% DPI 三組 + mixed-DPI 多螢幕手測

## 4. HUD window 配置

### 4.1 `tauri.conf.json` 變更

```json
{
  "label": "main",
  "width": 380,
  "height": 56,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "visible": false,
  "resizable": false,
  "center": false,         // 改：手動 position via Rust command
  "x": 0, "y": 0           // 改：實際位置由 position_hud_for_active_monitor 設
}
```

### 4.2 click-through 策略

- HUD window 預設 `setIgnoreCursorEvents(true)`（M2 baseline 已是 click-through）
- HudOverlay `watch(status, ...)`：
  - `status === 'error'` → `await getCurrentWindow().setIgnoreCursorEvents(false)`
  - 其他 → `await getCurrentWindow().setIgnoreCursorEvents(true)`
- HUD window 大小（380×56）小到「click-through-off 時點透明邊緣」幾乎不擋使用者；點 HUD 任何位置都觸發 `dismissError`

### 4.3 Visibility 策略

- HUD window 整體 `setVisible(true/false)`：M5 不主動 show/hide（讓 Vue `v-if="store.status !== 'idle'"` 控制 bubble、window 一直 mounted）
- 替代方案：手動 `hide()`/`show()` window — 更乾淨但需 IPC 來回；defer 到 M9 polish 看 RAF 用量
- Dev mode `set_hud_visible_for_dev(true)` 強制 window `setVisible(true)`、bypass status 條件、方便 visual debug

## 5. Accessibility

### 5.1 ARIA

```vue
<div
  v-if="visible"
  class="hud-root"
  role="status"
  aria-live="polite"
  :aria-label="ariaMessage"
  @click="handleHudClick"
>
  ...
</div>
```

`ariaMessage` computed：
- `recording` → i18n `hud.aria.recording`（"Recording"）
- `transcribing` → i18n `hud.aria.transcribing`（"Transcribing"）
- `success` → i18n `hud.aria.success`（"Done"）
- `error` → i18n `hud.aria.error`（"Error: {message}", message=store.message）

所有 inner icon `aria-hidden="true"`（避免 SR 念兩次）。

### 5.2 `prefers-reduced-motion: reduce` fallback

- HudOverlay `<Transition>` `name`：`reducedMotion ? 'instant' : 'fade'`、`fade` 200ms、`instant` 0ms
- HudWaveform：reduced-motion → 改顯示單一灰色 dot + 「錄音中」label；不跑 RAF lerp
- HudSpinner：reduced-motion → 改顯示「…」三點靜態文字；CSS `animation: none`
- HudIcon (success/error)：reduced-motion → 純 fade-in 取代 scale

`reducedMotion` ref via `window.matchMedia('(prefers-reduced-motion: reduce)')`、`addEventListener('change', ...)` 動態反應系統設定變更。

## 6. Cross-window event：`voice-flow:state-changed`

### 6.1 Emit 端（HUD）

`useVoiceFlowStore.transitionTo(next, msg)`：
```typescript
async function transitionTo(next: VoiceFlowStatus, msg: string): Promise<void> {
  status.value = next;
  message.value = msg;
  await emit("voice-flow:state-changed", { status: next, message: msg });
}
```

### 6.2 Listen 端（Dashboard）

`MainApp.vue` 或 sidebar component：
```typescript
const status = ref<VoiceFlowStatus>("idle");
const unlisten = await listen<{ status: VoiceFlowStatus; message: string }>(
  "voice-flow:state-changed",
  (e) => {
    status.value = e.payload.status;
  },
);
onUnmounted(() => unlisten());
```

`HudFlowBadge.vue` 接 `status` prop、`v-if="status === 'recording'"` 顯示。

### 6.3 Phase 1 限制

Dashboard 在 voice flow 中途開啟時，不會立刻反映「正在錄音」（要等下次 transition）。Phase 2 加 mount-time state request 解決。

## 7. Store changes（`useVoiceFlowStore.ts`）

### 7.1 `init()` async refactor

```typescript
async function init(): Promise<() => void> {
  const unlistenFns: Array<() => void> = [];

  // Sequential await — events 在 await 期間不會 lost（Tauri queue），
  // 且 await 結束後保證 listener 已註冊
  unlistenFns.push(
    await listenToEvent<HotkeyEventPayload>(HOTKEY_PRESSED, () => { void handleStart(); }),
  );
  unlistenFns.push(
    await listenToEvent<HotkeyEventPayload>(HOTKEY_RELEASED, () => { void handleStop(); }),
  );
  unlistenFns.push(
    await listenToEvent<HotkeyEventPayload>(HOTKEY_TOGGLED, (event) => {
      const action = event.payload.action;
      if (action === "toggled-on") void handleStart();
      else if (action === "toggled-off") void handleStop();
    }),
  );
  unlistenFns.push(
    await listenToEvent<void>(ESCAPE_PRESSED, () => { void handleCancel(); }),
  );
  unlistenFns.push(
    await listenToEvent<RecordingAbortedPayload>(AUDIO_RECORDING_ABORTED, (event) => {
      handleError(new Error(`錄音超過上限：${event.payload.reason}`));
    }),
  );

  return () => unlistenFns.forEach((fn) => fn());
}
```

HUD entry `main.ts`：
```typescript
const store = useVoiceFlowStore();
store.init().then((cleanup) => {
  (window as { __voiceFlowCleanup?: () => void }).__voiceFlowCleanup = cleanup;
});
```

### 7.2 移除 `paste:focus-restore-failed` listener

```typescript
// REMOVED in M5:
// void listenToEvent<PasteFocusRestoreFailedPayload>(PASTE_FOCUS_RESTORE_FAILED, ...);
```

理由：
- Rust paste pipeline 失敗時 emit 此 event **and** return `FocusRestoreFailed` error
- `handleStop` 的 try/catch 已捕 error → 走 `handleError` → transition 到 error
- listener 是重複觸發（已 transition 一次再 transition 一次）
- 移除 listener 順便解決 IDEAS 標的「state collision in transcribing」（無 listener 就無 collision）
- Rust 端 event 繼續 emit、為未來 Dashboard tooltip 用

### 7.3 加 `dismissError()`

```typescript
function dismissError(): void {
  if (status.value === "error") {
    transitionTo("idle", "");
  }
}
```

HudOverlay click handler：
```typescript
function handleHudClick(): void {
  if (store.status === "error") {
    store.dismissError();
  }
}
```

### 7.4 `audio:recording-aborted` listener

**已驗證**：`AUDIO_RECORDING_ABORTED` event constant 已在 `src/composables/useTauriEvents.ts` export；`RecordingAbortedPayload` 已在 `src/types/events.ts:110` 完整 typed 為 `{ reason: 'max_size' | 'mic_unplug', bytesRecorded: number }`、可直接使用。Chunk 1 只需新增 listener、不必改 type 或 constant。

### 7.5 Cross-window emit on transition

見 §6.1。

## 8. Components 詳細結構

### 8.1 `HudOverlay.vue` (~150 LOC)

```vue
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckCircle2, XCircle } from "lucide-vue-next";

import { useVoiceFlowStore } from "@/stores/useVoiceFlowStore";
import HudWaveform from "@/components/HudWaveform.vue";
import HudSpinner from "@/components/HudSpinner.vue";
import HudTimer from "@/components/HudTimer.vue";

const store = useVoiceFlowStore();
const { t } = useI18n();
const reducedMotion = ref(false);
let mediaQuery: MediaQueryList | null = null;

const visible = computed(() => store.status !== "idle");
const ariaMessage = computed(() => {
  switch (store.status) {
    case "recording": return t("hud.aria.recording");
    case "transcribing": return t("hud.aria.transcribing");
    case "success": return t("hud.aria.success");
    case "error": return t("hud.aria.error", { message: store.message });
    default: return "";
  }
});
const transitionName = computed(() => (reducedMotion.value ? "instant" : "fade"));

async function syncClickThrough(status: string): Promise<void> {
  await getCurrentWindow().setIgnoreCursorEvents(status !== "error");
}

watch(() => store.status, (newStatus) => { void syncClickThrough(newStatus); });

function handleHudClick(): void {
  if (store.status === "error") store.dismissError();
}

function handleMediaChange(e: MediaQueryListEvent): void {
  reducedMotion.value = e.matches;
}

onMounted(async () => {
  mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  reducedMotion.value = mediaQuery.matches;
  mediaQuery.addEventListener("change", handleMediaChange);
  await getCurrentWindow().setIgnoreCursorEvents(true);
});

onUnmounted(() => {
  mediaQuery?.removeEventListener("change", handleMediaChange);
  mediaQuery = null;
});
</script>

<template>
  <div
    v-if="visible"
    class="flex h-screen w-screen items-center justify-center"
    role="status"
    aria-live="polite"
    :aria-label="ariaMessage"
    @click="handleHudClick"
  >
    <Transition :name="transitionName" mode="out-in">
      <div
        v-if="store.status === 'recording'"
        key="recording"
        class="bubble bubble-recording"
      >
        <HudWaveform :reduced-motion="reducedMotion" />
        <HudTimer :started-at-ms="store.recordingStartedAtMs" />
      </div>
      <div
        v-else-if="store.status === 'transcribing'"
        key="transcribing"
        class="bubble bubble-transcribing"
      >
        <HudSpinner :reduced-motion="reducedMotion" />
        <span class="bubble-label">{{ t("hud.transcribing") }}</span>
      </div>
      <div
        v-else-if="store.status === 'success'"
        key="success"
        class="bubble bubble-success"
      >
        <CheckCircle2 :size="20" aria-hidden="true" class="text-green-600" />
        <span class="bubble-label">{{ t("hud.success") }}</span>
      </div>
      <div
        v-else-if="store.status === 'error'"
        key="error"
        class="bubble bubble-error"
      >
        <XCircle :size="20" aria-hidden="true" class="text-red-600" />
        <span class="bubble-label">{{ store.message }}</span>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.bubble {
  /* Tailwind utility classes via @apply OR rely on inline class strings */
  display: flex;
  align-items: center;
  gap: 0.5rem;
  border-radius: 9999px;
  background: var(--card);
  color: var(--card-foreground);
  border: 1px solid var(--border);
  padding: 0.5rem 1rem;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}
.bubble-label { font-size: 0.875rem; font-weight: 500; }

/* fade transition (200ms) */
.fade-enter-active, .fade-leave-active { transition: opacity 200ms ease, transform 200ms ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; transform: scale(0.95); }
.fade-enter-to, .fade-leave-from { opacity: 1; transform: scale(1); }

/* instant transition (reduced-motion) */
.instant-enter-active, .instant-leave-active { transition: none; }
</style>
```

### 8.2 `HudWaveform.vue` (~50 LOC)

```vue
<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useAudioWaveform } from "@/composables/useAudioWaveform";

defineProps<{ reducedMotion: boolean }>();
const { smoothedLevels, start, stop } = useAudioWaveform();

onMounted(() => { void start(); });
onUnmounted(stop);
</script>

<template>
  <div v-if="!reducedMotion" class="flex items-center gap-1" aria-hidden="true">
    <span
      v-for="(level, i) in smoothedLevels"
      :key="i"
      class="h-4 w-1 rounded-full bg-primary"
      :style="{ height: `${4 + level * 16}px` }"
    />
  </div>
  <div v-else aria-hidden="true" class="flex items-center gap-2">
    <span class="size-2 rounded-full bg-primary" />
  </div>
</template>
```

### 8.3 `HudSpinner.vue` (~30 LOC)

```vue
<script setup lang="ts">
defineProps<{ reducedMotion: boolean }>();
</script>

<template>
  <span
    v-if="!reducedMotion"
    aria-hidden="true"
    class="inline-block size-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
  />
  <span v-else aria-hidden="true" class="text-sm font-mono">…</span>
</template>
```

### 8.4 `HudTimer.vue` (~60 LOC)

```vue
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";

const props = defineProps<{ startedAtMs: number | null }>();
const now = ref(Date.now());
let interval: ReturnType<typeof setInterval> | null = null;

const elapsed = computed(() => {
  if (!props.startedAtMs) return 0;
  return Math.max(0, Math.floor((now.value - props.startedAtMs) / 1000));
});
const formatted = computed(() => {
  const m = Math.floor(elapsed.value / 60);
  const s = elapsed.value % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
});
const colorClass = computed(() => {
  if (elapsed.value >= 720) return "text-red-600";       // ≥ 12 分鐘
  if (elapsed.value >= 540) return "text-yellow-600";    // ≥ 9 分鐘
  return "text-muted-foreground";
});

onMounted(() => { interval = setInterval(() => { now.value = Date.now(); }, 1000); });
onUnmounted(() => { if (interval) clearInterval(interval); });
</script>

<template>
  <span class="font-mono text-sm" :class="colorClass">{{ formatted }}</span>
</template>
```

### 8.5 `HudFlowBadge.vue` (~40 LOC, Dashboard side)

```vue
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { listenToEvent, VOICE_FLOW_STATE_CHANGED } from "@/composables/useTauriEvents";
import type { VoiceFlowStateChangedPayload } from "@/types/events";

const { t } = useI18n();
const status = ref<string>("idle");
let unlisten: (() => void) | null = null;

onMounted(async () => {
  unlisten = await listenToEvent<VoiceFlowStateChangedPayload>(
    VOICE_FLOW_STATE_CHANGED,
    (e) => { status.value = e.payload.status; },
  );
});
onUnmounted(() => { unlisten?.(); });
</script>

<template>
  <div
    v-if="status === 'recording'"
    class="flex items-center gap-2 px-3 py-2"
    role="status"
    aria-live="polite"
  >
    <span class="size-2 animate-pulse rounded-full bg-red-500" aria-hidden="true" />
    <span class="text-xs">{{ t("sidebar.recordingBadge") }}</span>
  </div>
</template>
```

## 9. i18n keys

新加（zh-TW + en 各一份）：

| Key | zh-TW | en |
|---|---|---|
| `hud.aria.recording` | 錄音中 | Recording |
| `hud.aria.transcribing` | 轉錄中 | Transcribing |
| `hud.aria.success` | 完成 | Done |
| `hud.aria.error` | 錯誤：{message} | Error: {message} |
| `hud.transcribing` | 轉錄中… | Transcribing… |
| `hud.success` | 完成 | Done |
| `sidebar.recordingBadge` | 錄音中 | Recording |

（既有 `hud.title` / `hud.pongCount` 在 M5 chunk 2 隨 placeholder 移除）

## 10. Rust changes

### 10.1 `position_hud_for_active_monitor()` command

```rust
#[tauri::command]
pub async fn position_hud_for_active_monitor(
    app: AppHandle,
) -> Result<(), HudError> {
    // 詳細實作由 implementer 決定；優先用 Tauri API，fallback raw Win32
    let hud = app.get_webview_window("main")
        .ok_or(HudError::WindowMissing)?;
    let cursor_pos = get_cursor_position()?;  // raw Win32 GetCursorPos
    let monitors = hud.available_monitors()?;
    let target = pick_monitor(&monitors, cursor_pos);
    let position = compute_centered_position(target, /* hud size */ (380.0, 56.0), /* y */ 50.0);
    hud.set_position(position)?;
    Ok(())
}
```

註冊在 `tauri::generate_handler!`。Capabilities 影響：HUD 既有 `src-tauri/capabilities/hud.json` 只 grant `core:default + core:event:default`。Tauri 2 自製 commands 預設 allowed（不需顯式 permission），但 `WebviewWindow::set_position()` 是 core API、可能需要 `core:webview:allow-set-position`。Chunk 0 implementer 必須：(1) cargo build 試跑、(2) 若編譯 OK 但 runtime allow-list 拒絕、加 permission；(3) 兩 capability files（hud.json + dashboard.json）若需 set-position 都加。

### 10.2 `set_hud_visible_for_dev(visible: bool)` command

```rust
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn set_hud_visible_for_dev(
    app: AppHandle,
    visible: bool,
) -> Result<(), HudError> {
    let hud = app.get_webview_window("main")
        .ok_or(HudError::WindowMissing)?;
    if visible { hud.show()?; } else { hud.hide()?; }
    Ok(())
}
```

註冊在 `tauri::generate_handler!` 但用 `#[cfg(debug_assertions)]` block 包；release build 不暴露此 command。

### 10.3 新 file：`src-tauri/src/plugins/hud.rs`

擺 `HudError`、上述兩 command、helper `pick_monitor`、`compute_centered_position`、`get_cursor_position`。檔案目標 < 200 LOC。

## 11. Acceptance criteria（≥ 12 conditions）

| # | 條件 | 自動驗證 | 手動驗證 |
|---|---|---|---|
| 1 | recording state — 6-bar waveform 動、timer 0:01→0:02 | ✅ Vitest HudOverlay state machine | ✅ user dogfood |
| 2 | transcribing state — spinner 旋、「轉錄中…」 label | ✅ Vitest | ✅ user dogfood |
| 3 | success state — ✓ + 「完成」、1s 後 fade idle | ✅ Vitest auto-hide timer | ✅ user dogfood |
| 4 | error state — ✗ + message、6s 後 fade idle、可點 dismiss 立即 idle | ✅ Vitest auto-hide + dismiss method | ✅ user dogfood + click |
| 5 | cap warning：timer ≥ 9 min 黃、≥ 12 min 紅；達 25 MB cap 自動 abort 進 error | ✅ Vitest color computed + abort listener | ✅ user 連錄 ≥ 13 min 驗 error |
| 6 | active-monitor positioning — primary monitor 工作、HUD 在 primary | ✅ Rust unit test pick_monitor | ✅ user 在 secondary monitor 工作、按熱鍵、確認 HUD 出現在 secondary |
| 7 | DPI 100% / 150% / 200% 三組 + mixed-DPI（4K 主 + 1080p 副）— HUD 不糊 | ❌ 純手動 | ✅ user 在 Display Settings 切換 DPI 驗 |
| 8 | `prefers-reduced-motion: reduce` ON — waveform 改 dot、spinner 改「…」、無 fade scale | ✅ Vitest reduced-motion fallback | ✅ user OS Settings 開啟「動畫減少」驗 |
| 9 | ARIA — screen reader 念出 state 變化 | ❌ 純手動 | ✅ user NVDA / Narrator 驗 |
| 10 | Dashboard sidebar — recording 期間紅點 + 「錄音中」、其他狀態 hidden | ✅ Vitest HudFlowBadge | ✅ user dogfood |
| 11 | click-through — recording / transcribing / success state 滑鼠可穿透 HUD 點到下層 app；error state 不能 | ❌ 純手動 | ✅ user 試在 HUD 範圍內點 Notepad / Word |
| 12 | dev visibility — `set_hud_visible_for_dev(true)` 強制顯示 idle 狀態 HUD | ✅ Rust unit test | ✅ user 在 dev console invoke |
| 13 | `paste:focus-restore-failed` 後 — clipboard 仍有文字、HUD 顯示「請手動 Ctrl+V」 message、6s linger | ✅ Vitest handleStop catch path | ✅ user 試在 UAC elevated app（Task Manager）driving paste 失敗 |
| 14 | 移除 `paste:focus-restore-failed` listener — 確認快速兩次熱鍵期間舊 paste 失敗 event 不影響新 flow | ✅ Vitest（無對應 listener 註冊） | ✅ user 試 burst press during transcribing |

### 11.1 Static checks（all green required）

- `vue-tsc --noEmit` → 0 errors
- `eslint .` → 0 errors / 0 warnings
- `vitest run` → vitest pass（M4 = 16；M5 預估 +6-8 = ~22-24）
- `cargo check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test --lib` → cargo test pass（M4 = 157；M5 預估 +5-8 = ~162-165）

### 11.2 Playwright screenshot SOP（implementer + reviewer + challenger 各自跑）

**Vite-only mode**（`pnpm dev` + 手動觸發 status 改變 via vue devtools 或 mock）：

| # | Screenshot | 觸發 | 驗證 |
|---|---|---|---|
| 1 | `m5-hud-idle.png` | 預設 | bubble 不 render、HUD window 透明 |
| 2 | `m5-hud-recording-empty.png` | mock `status='recording'`、無 waveform data | 6 bars 起點 + timer 0:00 |
| 3 | `m5-hud-recording-active.png` | mock `status='recording'` + mock waveform levels | 6 bars 高低不一 + timer mm:ss |
| 4 | `m5-hud-recording-warn-yellow.png` | mock `recordingStartedAtMs` 9 min 前 | timer 文字黃色 |
| 5 | `m5-hud-recording-warn-red.png` | mock `recordingStartedAtMs` 12 min 前 | timer 文字紅色 |
| 6 | `m5-hud-transcribing.png` | mock `status='transcribing'` | spinner + 「轉錄中…」 |
| 7 | `m5-hud-success.png` | mock `status='success'` | ✓ + 「完成」 |
| 8 | `m5-hud-error.png` | mock `status='error'`、`message='API key 無效'` | ✗ + message |
| 9 | `m5-hud-error-after-click.png` | screenshot 8 之後 click HUD root | bubble fade out → idle |
| 10 | `m5-hud-reduced-motion-recording.png` | `emulateMedia({reducedMotion: 'reduce'})` + mock recording | dot + label 取代 6 bars |
| 11 | `m5-hud-reduced-motion-transcribing.png` | reduced-motion + transcribing | 「…」取代 spinner |
| 12 | `m5-dashboard-sidebar-badge.png` | mock voice-flow:state-changed event status='recording' on Dashboard | sidebar 紅點 + 「錄音中」 |
| 13 | `m5-dashboard-sidebar-idle.png` | status='idle' | badge 不 render |

每張 screenshot 之後 **Read 工具讀回**目視確認（CLAUDE.md UI verification SOP）。

## 12. Testing strategy

### 12.1 Vitest（HUD logic）

新 test files：
- `src/__tests__/HudOverlay.test.ts` — state → visual class 對應、auto-hide timer 與 fake timers、click-to-dismiss、reduced-motion fallback
- `src/__tests__/HudTimer.test.ts` — mm:ss 格式、9/12 min 警告色
- `src/__tests__/HudWaveform.test.ts` — reduced-motion 切換 render
- `src/__tests__/useVoiceFlowStore.test.ts` 改寫 — async init、新 listeners、dismissError

### 12.2 Cargo tests

新 test cases：
- `pick_monitor` 對 cursor 在不同 monitor rect 的選取邏輯
- `compute_centered_position` 對不同 monitor size + DPI 的 position 計算
- `set_hud_visible_for_dev` mock app handle assertions

### 12.3 Manual

- 多螢幕 + 多 DPI handheld（acceptance criteria #6, #7）
- Screen reader（acceptance criteria #9）
- click-through 物理測試（acceptance criteria #11）

## 13. Chunking（5 chunks，sequential）

### Chunk 0：Rust + types + tauri.conf
- `src-tauri/src/plugins/hud.rs`（new）：`position_hud_for_active_monitor` + `set_hud_visible_for_dev`（debug only）+ `HudError` + helpers + tests
- `tauri.conf.json` HUD window：380×56、`center: false`、`x/y: 0`（實際位置由 command 設）
- `src/types/events.ts`（增）：`VoiceFlowStateChangedPayload` 取代既有 `type ... = unknown` placeholder（line 167）為 `interface { status: VoiceFlowStatus; message: string }`、import `VoiceFlowStatus` from store。`RecordingAbortedPayload` M2 已 export、無需動。
- `src/composables/useTauriEvents.ts`（增）：`VOICE_FLOW_STATE_CHANGED` constant
- `lib.rs` register handler、`AppHandle` plumbing
- Acceptance：cargo check + clippy + test + vue-tsc

### Chunk 1：useVoiceFlowStore refinements
- `init()` async + sequential await
- 移除 `paste:focus-restore-failed` listener（保留 Rust emit）
- 加 `dismissError()`
- 加 `audio:recording-aborted` listener
- `transitionTo()` emit `voice-flow:state-changed`
- `useVoiceFlowStore.test.ts` 改寫（保留 6 既有 test、加 ~3 新 test）
- HUD entry `main.ts`：`store.init().then(...)`
- Acceptance：vitest + vue-tsc + eslint

### Chunk 2：HUD components + i18n
- 移除 `HudOverlay.vue` 既有 placeholder（pong counter）
- 新 `HudOverlay.vue`、`HudWaveform.vue`、`HudSpinner.vue`、`HudTimer.vue`
- i18n `hud.aria.*`、`hud.transcribing`、`hud.success`（zh-TW + en）
- 新 vitest files（HudOverlay / HudTimer / HudWaveform）
- Acceptance：vitest + vue-tsc + eslint
- 每張 vite-shape screenshot 1-11 驗

### Chunk 3：Dashboard sidebar + dev tooling
- `HudFlowBadge.vue`（new）
- `MainApp.vue` 或 sidebar 加 `<HudFlowBadge />`
- i18n `sidebar.recordingBadge`
- vitest `HudFlowBadge.test.ts`
- Acceptance：vitest + vue-tsc + eslint
- vite-shape screenshot 12-13

### Chunk 4：Acceptance docs + screenshots compare + update PROGRESS
- `docs/m5-acceptance.md`（含 14 conditions、每條「設定 / 步驟 / 預期」）
- Implementer 完成 13 張 vite-shape screenshots in `docs/screenshots/m5/`
- `.claude/PROGRESS.md` + `doc/plans/02-implementation-roadmap.md` dashboard 更新（M5 → ✅ Implementation done）
- 新 session log `.claude/sessions/2026-05-05-m5-hud-overlay.md`（topic + What changed + Key decisions + Surprises + Follow-ups）

### Reviewer + retro challenger（chunk 4 完成後）

- **Reviewer subagent（Opus 4.7）**：read code + run Playwright + 驗證 spec §11.1 / §11.2、output P0/P1/P2 findings。**MUST 跑 Playwright 截 13 張 screenshots 自己驗、不只看 implementer screenshots。**
- **Retro challenger subagent（Opus 4.7、`general-purpose`）**：read 完整 chunk 0-4 commits + session log、跑 Playwright 截 6+ states 比對 spec、提出 latent UX / perf / accessibility / 邊界條件問題、append `.claude/IDEAS.md`。**MUST 跑 Playwright 截圖比對 spec、不只 read code。**

## 14. Risks + 緩解

| Risk | 影響 | 緩解 |
|---|---|---|
| Tauri `setIgnoreCursorEvents` 在 Win11 anti-flash 下行為與 Win10 不一致 | error state 點不到 HUD dismiss、user 卡 6s | acceptance #11 手測 + Win10/11 都驗 |
| `prefers-reduced-motion` JS API 在 Tauri webview 行為與 browser 不一致 | reduced-motion fallback 不觸發 | chunk 2 implementer 開 `pnpm tauri dev` 驗、不只 vite-only |
| HUD window 380×56 在 4K 螢幕看不清 | 文字小、難讀 | acceptance #7 多 DPI 手測；如 fail 改用 `1.5x` scaling |
| `voice-flow:state-changed` event 廣播太頻繁影響 perf | Dashboard 重 render | M5 4 個 transitions / flow → 4 emits、trivial |
| Vue `<Transition mode="out-in">` 與 reactivity update 順序撞 click-through `setIgnoreCursorEvents` await | race：transition 進場時 click-through 還未切換 | chunk 2 implementer 在 `watch` callback 同步 await、確認 await sequence 對 |
| `available_monitors()` Tauri API 在 multi-monitor 處理 DPI scaling 不對 | HUD 在 secondary monitor 位置歪 | chunk 0 implementer Win32 raw fallback 留 escape hatch |

## 15. 與 SayIt 對比

| 主題 | SayIt | TalkType M5 |
|---|---|---|
| HUD LOC | 861（NotchHud.vue 一檔肥） | < 600（4 components 加總） |
| Visual modes | 10（含 morphing / collapsing / learned / mode-switch） | 4（recording / transcribing / success / error）+ Phase 2 加 |
| Animation | CSS clip-path 動態 SVG path morphing | Vue `<Transition>` opacity + scale |
| ARIA | **0** | role="status" + aria-live + aria-label + state announce |
| `prefers-reduced-motion` | **無** | 完整 fallback（waveform / spinner / icon 三層） |
| Multi-monitor | primary only | active-monitor positioning |
| Dev visibility | manual `tauri.conf.json` 改 visible | `set_hud_visible_for_dev` debug-only command |
| 跨視窗 voice flow | localStorage poll | Tauri event `voice-flow:state-changed` |

## 16. 驗收 + 收尾

- 全部 14 acceptance conditions 自動驗證項全綠（vitest + cargo test + vue-tsc + clippy + eslint）
- Implementer 跑 13 張 vite-shape screenshots、Read 確認
- Reviewer subagent 自跑 13 張 Playwright screenshots + report
- Retro challenger subagent 自跑 6+ Playwright screenshots + IDEAS append
- User 跑 manual acceptance #6 #7 #9 #11（multi-monitor + DPI + screen reader + click-through 物理測）
- M4 模式重複：開 PR、user merge、PROGRESS dashboard `M5 → ✅ Done`

## 17. Open questions（implementer 解或下次 session）

- HUD bubble background 用 `bg-card` (語意 token) 還是 `bg-background/80` (semi-transparent)? Implementer 試兩種看 dogfood feedback
- `compute_centered_position` 對 ultrawide monitor 的 logical pixel 假設是否成立? Chunk 0 implementer 跑 ultrawide test
- `<Transition mode="out-in">` 對 同 status reactive update（e.g. recording 期間 timer 變化）是否會誤觸 transition? Implementer 確認 transition key 設對
