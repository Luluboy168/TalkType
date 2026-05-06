# M5 — HUD Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **Spec**: [`docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md`](../specs/2026-05-05-m5-hud-overlay-design.md) (commit `44b025f`)
> **Project conventions**: [`CLAUDE.md`](../../../CLAUDE.md) — `model: opus` for impl + reviewer + challenger; UI 必截 interactive states；commit per chunk + HEREDOC + Co-Authored-By。
> **最後更新**：2026-05-05

**Goal:** Complete the M1 HUD placeholder into a fully functional HUD overlay with 4 visual states (recording / transcribing / success / error), 6-bar waveform, ARIA + reduced-motion accessibility, active-monitor positioning, dev visibility tooling, Dashboard sidebar indicator.

**Architecture:** 4 HUD .vue components driven by `useVoiceFlowStore`'s logical state machine. Rust adds `position_hud_for_active_monitor` + `set_hud_visible_for_dev` (debug only) commands. Cross-window event `voice-flow:state-changed` syncs HUD → Dashboard sidebar badge.

**Tech Stack:** Vue 3.5 (`<Transition>`, `<script setup>`), Tauri 2 (`getCurrentWindow().setIgnoreCursorEvents`, `available_monitors()`), `lucide-vue-next` (CheckCircle2, XCircle), `useAudioWaveform` (M2 emit, ready), Pinia readonly refs.

---

## File structure delta

### Created
- `src-tauri/src/plugins/hud.rs` — `HudError`, `position_hud_for_active_monitor` cmd, `set_hud_visible_for_dev` cmd（debug only）, helpers `pick_monitor` + `compute_centered_position` + `get_cursor_position`, unit tests
- `src/components/HudWaveform.vue` — 6-bar visualizer + reduced-motion fallback
- `src/components/HudSpinner.vue` — transcribing spinner + reduced-motion fallback
- `src/components/HudTimer.vue` — mm:ss elapsed + 9/12 min warning colors
- `src/components/HudFlowBadge.vue` — Dashboard sidebar recording indicator
- `src/__tests__/HudOverlay.test.ts` — state machine + transitions + click handler
- `src/__tests__/HudTimer.test.ts` — mm:ss + warning colors
- `src/__tests__/HudWaveform.test.ts` — reduced-motion fallback
- `src/__tests__/HudFlowBadge.test.ts` — visibility binding
- `docs/m5-acceptance.md` — 14 acceptance conditions
- `docs/screenshots/m5/` — 13 vite-shape screenshots
- `.claude/sessions/2026-05-05-m5-hud-overlay.md` — session log

### Modified
- `src/components/HudOverlay.vue` — replace M1 placeholder with state machine + ARIA + reduced-motion + click handler + 4 sub-component composition
- `src/stores/useVoiceFlowStore.ts` — async `init()`, remove `paste:focus-restore-failed` listener, add `dismissError()` + `audio:recording-aborted` listener, emit `voice-flow:state-changed` on transitionTo
- `src/__tests__/useVoiceFlowStore.test.ts` — async init mocks, new listeners, dismissError tests
- `src/main.ts` (HUD entry) — async `store.init().then(...)`
- `src/types/events.ts` — replace `VoiceFlowStateChangedPayload = unknown` with concrete interface
- `src/composables/useTauriEvents.ts` — add `VOICE_FLOW_STATE_CHANGED` constant
- `src/i18n/locales/zh-TW.json` + `en.json` — `hud.aria.*`, `hud.transcribing`, `hud.success`, `sidebar.recordingBadge` keys
- `src/main-window.ts` (Dashboard entry) or `src/MainApp.vue` — `<HudFlowBadge />` integration
- `src-tauri/tauri.conf.json` — HUD window 380×56 logical, `center: false`, `x/y: 0`
- `src-tauri/src/plugins/mod.rs` — `pub mod hud;`
- `src-tauri/src/lib.rs` — register 2 new commands in `generate_handler!`, possibly `.manage(...)` for Hud state if needed
- `src-tauri/capabilities/hud.json` — add `core:webview:allow-set-position` if Tauri 2 requires it (verify at compile time)
- `.claude/PROGRESS.md` — M5 → ✅ Done line + session link
- `doc/plans/02-implementation-roadmap.md` — dashboard `M5 → ✅ Done`

### Deleted (M1 placeholder)
- `src/components/HudOverlay.vue` 內 pong counter（chunk 2 replacement）

---

## Chunking overview（5 chunks，sequential）

| Chunk | 內容 | 依賴 | 估時 |
|---|---|---|---|
| 0 | Rust commands + types + tauri.conf | M4 done | 1.5 hr |
| 1 | useVoiceFlowStore async refactor + new listeners | Chunk 0 (event constant) | 1 hr |
| 2 | HUD components (4 files) + i18n + main.ts | Chunk 1 (async init) | 2 hr |
| 3 | Dashboard sidebar HudFlowBadge | Chunk 1 (cross-window emit) | 1 hr |
| 4 | Acceptance docs + screenshots + retro | Chunks 0-3 | 1 hr |

---

## Refinements from plan-time challenger

Challenger（agentId `abe35088f7a648bf9`）2026-05-05 回 3 P0 + 8 P1 + 3 P2。Findings 處置：

### P0（已折進 spec + plan task list）

| ID | Issue | Fix location |
|---|---|---|
| P0-1 | `emit("voice-flow:state-changed")` 廣播到 HUD 自身 webview、未來若 HUD 也 listen 會 echo loop | Spec §6.1 改用 `emitTo("main-window", ...)` + payload 加 `source: "hud"`；Chunk 0 Task 0.5 update `VoiceFlowStateChangedPayload` interface；Chunk 1 Task 1.5 對應更新 |
| P0-2 | Spec §10.1 假 API `Manager::cursor_position()`、Tauri 2 stable 不存在、implementer 會撞牆；pure helpers 沒強制 extract | Spec §10.1 改 raw Win32 直接路徑、強制 extract `MonitorRect` + `pick_monitor` + `compute_centered_position` 為 testable helpers；Chunk 0 Task 0.1 更新 |
| P0-3 | 移除 `paste:focus-restore-failed` listener 同時失去「請手動 Ctrl+V」友善 hint，user 看 raw Rust enum Display | Spec §7.2 改成 `formatError` pattern-match `FocusRestoreFailed` Rust error → 加上 hint；Chunk 1 Task 1.2 + 新 Task 1.2.5 |

### P1（折進 spec、無 task 結構大改）

| ID | Issue | Fix location |
|---|---|---|
| P1-1 | 380×56 HUD bubble 不 truncate 長 error 訊息、會醜 | Spec §8.1 styles 加 `text-overflow: ellipsis`；Chunk 2 Task 2.4 採用 |
| P1-3 | rapid hotkey press → HudWaveform mount/unmount race in `useAudioWaveform.start()` | Spec §14 Risks 增條目；Chunk 2 加 sub-task：`useAudioWaveform.ts` 加 `starting` flag |
| P1-4 | cpal stream sleep 行為未定 | Spec §14 Risks 增條目；M5 acceptance #15 加「sleep mid-recording」條件 |
| P1-6 | `#[cfg(debug_assertions)]` 在 `generate_handler!` macro 展開可能撞 toolchain；release 編譯失敗 | Spec §10.2 改成 `cfg!`-branched two-path generate_handler；Chunk 0 Task 0.2 + 新 Task 0.2.5 |
| P1-7 | `voice-flow:state-changed` payload 沒 source/version | 同 P0-1 fix（`source: "hud"` 已加） |
| P1-8 | `position_hud_for_active_monitor` 失敗 → `handleStart` throw → 阻擋 recording | Spec §3.1 加 try/catch fallback；Chunk 1 Task 1.1.5（new sub-task） |

### P1（評估後推 IDEAS、不阻擋 M5 ship）

| ID | Issue | 處置 |
|---|---|---|
| P1-2 | Vitest 測 `<Transition mode="out-in">` 在 jsdom timing 不可靠 | 不需 spec 改、是 implementer 認知 — append IDEAS 提醒「vitest 測 state→class 用 nextTick 即可、transition timing 靠 Playwright」 |
| P1-5 | HudTimer null `startedAtMs` 已正確處理 (return 0)、缺 regression test | append IDEAS、chunk 2 implementer 順手加 vitest 即可 |

### P2（全部推 IDEAS）

| ID | Issue | 處置 |
|---|---|---|
| P2-1 | Dashboard sidebar 只顯示 `recording`、不顯示 transcribing / error | IDEAS Phase 2 |
| P2-2 | aria-live="polite" 連續同訊息 SR 不 announce | IDEAS Phase 2 a11y polish |
| P2-3 | HudSpinner 30 LOC borderline | IDEAS — chunk 2 implementer 自行決定要不要 inline |

---

## Chunk 0 — Rust commands + types + tauri.conf

**Goal**: Lay the Rust foundation + frontend type contract + HUD window resize. No Vue components yet.

### Files
- **Create**: `src-tauri/src/plugins/hud.rs`
- **Modify**: `src-tauri/src/plugins/mod.rs` (`pub mod hud;`)
- **Modify**: `src-tauri/src/lib.rs` (register 2 commands)
- **Modify**: `src-tauri/tauri.conf.json` (HUD window 380×56)
- **Modify**: `src-tauri/capabilities/hud.json` (`core:webview:allow-set-position` if needed; verify at build time)
- **Modify**: `src/types/events.ts` (replace `VoiceFlowStateChangedPayload = unknown` placeholder at line 167 with concrete interface)
- **Modify**: `src/composables/useTauriEvents.ts` (add `VOICE_FLOW_STATE_CHANGED = "voice-flow:state-changed"`)

### Tasks for the implementer subagent

- [ ] **Task 0.1: Create `src-tauri/src/plugins/hud.rs` with `HudError` + helpers + `position_hud_for_active_monitor` command** (P0-2 fold-in)
  - **Strict implementation per refined spec §10.1** (challenger flagged Tauri 2 stable API mistake)
  - **MUST extract pure helpers as cargo-testable functions**:
    - `MonitorRect { position, size, scale_factor }` plain struct + `From<&tauri::Monitor>` impl
    - `pick_monitor(monitors: &[MonitorRect], cursor: (i32, i32)) -> &MonitorRect` — find rect containing cursor; fallback first
    - `compute_centered_position(monitor: &MonitorRect, hud_logical: (f64, f64), y_offset_logical: f64) -> PhysicalPosition<f64>` — DPI-aware centering
    - `get_cursor_position() -> Result<(i32, i32), HudError>` — **raw Win32 `GetCursorPos` directly**, do NOT try Tauri abstraction (spec §10.1 explicitly mandates this — Tauri 2 stable does not expose cursor_position)
  - Public command `position_hud_for_active_monitor(app: AppHandle) -> Result<(), HudError>` orchestrates helpers + `hud.set_position(...)`
  - **Error variants** (thiserror, manual `Serialize` to flat string per CLAUDE.md):
    - `WindowMissing`、`MonitorEnumerationFailed(String)`、`SetPositionFailed(String)`、`CursorQueryFailed(String)`、`WindowOpFailed(String)`（後者用於 `set_hud_visible_for_dev`）

- [ ] **Task 0.2: `set_hud_visible_for_dev(visible: bool)` command (debug-only)** (P1-6 fold-in)
  - `#[cfg(debug_assertions)]` on the entire fn block in `hud.rs`
  - **In `lib.rs`, use the two-path `generate_handler!` pattern per spec §10.2**：
    ```rust
    #[cfg(debug_assertions)]
    let invoke_handler = tauri::generate_handler![
        /* ... 既有 commands ... */
        plugins::hud::position_hud_for_active_monitor,
        plugins::hud::set_hud_visible_for_dev,  // debug-only
    ];
    #[cfg(not(debug_assertions))]
    let invoke_handler = tauri::generate_handler![
        /* ... 既有 commands ... */
        plugins::hud::position_hud_for_active_monitor,
    ];
    // ... 後續 .invoke_handler(invoke_handler)
    ```
  - **Verify cfg-strip**: 跑 `cargo build --release`、確認 release build 編譯成功（symbol 完全不存在）；跑 `cargo build` (debug)、確認 debug build 仍 expose

- [ ] **Task 0.3: Unit tests in `hud.rs`**
  - `#[test] fn pick_monitor_returns_monitor_containing_cursor()` — synthesize 2 monitors with mock `Monitor` struct; cursor in monitor B → returns B
  - `#[test] fn pick_monitor_falls_back_to_first_when_cursor_outside_all()` — cursor at (-100, -100) → first monitor
  - `#[test] fn compute_centered_position_centers_on_monitor()` — monitor 1920×1080 at (0,0), HUD 380×56, y=50 → expect x = (1920-380)/2 = 770, y = 50
  - `#[test] fn compute_centered_position_handles_dpi_scale()` — monitor scale_factor 2.0 → physical position is logical × 2
  - **Note**: Tauri's `Monitor` struct is hard to construct in tests; create a `MonitorRect` plain struct {position, size, scale_factor} for `pick_monitor` + `compute_centered_position` to take, then `position_hud_for_active_monitor` does `monitors.iter().map(MonitorRect::from).collect()` adapter

- [ ] **Task 0.4: tauri.conf.json HUD window resize**
  - Read `src-tauri/tauri.conf.json` `app.windows` array, find `label: "main"` HUD entry
  - Set `width: 380`, `height: 56`, `center: false`, `x: 0`, `y: 0`, keep `decorations: false`, `transparent: true`, `alwaysOnTop: true`, `skipTaskbar: true`, `visible: false`, `resizable: false`

- [ ] **Task 0.5: TypeScript event types** (P0-1 + P1-7 fold-in)
  - In `src/types/events.ts:167`, replace:
    ```typescript
    export type VoiceFlowStateChangedPayload = unknown;
    ```
    with:
    ```typescript
    /**
     * Cross-window event `voice-flow:state-changed` payload (HUD → Dashboard).
     * Mirrors `useVoiceFlowStore.status` + `message` snapshot at transition time.
     * M5 introduces; future: M6 will extend with `enhancing` status.
     *
     * `source` field defends against echo loops if HUD ever adds its own listener
     * (challenger P0-1 / P1-7). Dashboard listener should filter `source !== 'hud'`
     * to ignore self-emitted events.
     */
    export interface VoiceFlowStateChangedPayload {
      status: "idle" | "recording" | "transcribing" | "success" | "error";
      message: string;
      source: "hud" | "dashboard";
    }
    ```
  - In `src/composables/useTauriEvents.ts`, add export constant `VOICE_FLOW_STATE_CHANGED = "voice-flow:state-changed"`

- [ ] **Task 0.6: Capabilities verification**
  - Run `pnpm tauri build` (or `cargo build` for src-tauri only) once
  - If build complains about `core:webview:allow-set-position` permission missing, add it to BOTH `src-tauri/capabilities/hud.json` and `dashboard.json` permissions arrays
  - If build passes without changes, document in commit message that `core:default` already covers `set_position`

- [ ] **Task 0.7: Static checks + commit**
  - `cd src-tauri && cargo check && cargo clippy --all-targets -- -D warnings && cargo test --lib hud::`
  - `cd .. && pnpm exec vue-tsc --noEmit && pnpm exec eslint src/types/events.ts src/composables/useTauriEvents.ts`
  - Commit with HEREDOC + Co-Authored-By per CLAUDE.md convention:
    ```
    feat(m5): chunk 0 — hud.rs + tauri.conf 380x56 + state-changed types

    - position_hud_for_active_monitor cmd (active-monitor centering, DPI-aware)
    - set_hud_visible_for_dev cmd (debug-only via cfg(debug_assertions))
    - HudError thiserror enum, pick_monitor + compute_centered_position helpers
    - tauri.conf HUD window 380x56 logical, center:false (manual position)
    - VoiceFlowStateChangedPayload concrete type (replaces unknown placeholder)
    - VOICE_FLOW_STATE_CHANGED event constant
    - 4 unit tests for monitor picking + position centering

    Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
    ```

### Acceptance for Chunk 0

- ✅ `cargo test --lib hud::` → 4 tests pass
- ✅ `cargo clippy --all-targets -- -D warnings` → clean
- ✅ `pnpm exec vue-tsc --noEmit` → 0 errors
- ✅ `pnpm tauri dev` 啟動成功（Rust commands 已註冊、HUD window 380×56 但 `visible: false`）

### Reviewer dispatch（Chunk 0 完工後）

Subagent type `general-purpose`, model `opus`, foreground (need findings before chunk 1).

**Reviewer prompt**:
```
You are a code reviewer for TalkType M5 chunk 0 (Rust + types + tauri.conf). Read commit `<chunk0_sha>` diff. Verify:

1. **Spec compliance** with §10 and §13 chunk 0 of `docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md`
2. **CLAUDE.md compliance**:
   - File size budget (hud.rs < 200 LOC?)
   - Rust style: thiserror + manual Serialize for HudError; Result<T, HudError> not Result<T, String>
   - cfg(debug_assertions) on set_hud_visible_for_dev — verify it's compile-time gated, not runtime
3. **Cross-platform**: any `#[cfg(target_os = "windows")]` needed for Win32 calls?
4. **Test coverage**:
   - pick_monitor: rect-containment + fall-back paths covered?
   - compute_centered_position: DPI scaling math correct?
   - Edge cases: zero-monitor list, cursor exactly on monitor boundary
5. **Capability**: does hud.json now grant correct permission? Or is core:default already sufficient?
6. **TypeScript types**: VoiceFlowStateChangedPayload status union exhaustive (5 values)? Matches Rust voice flow state if it lived in Rust (M5 keeps it Vue-side, but worth flagging)?

Output: P0 / P1 / P2 findings, each with file:line reference + concrete suggestion. Limit to 800 words. End with: "Recommendation: PROCEED to chunk 1 / FIX P0 first / NEEDS DISCUSSION".
```

---

## Chunk 1 — useVoiceFlowStore async refactor + new listeners

**Goal**: Refactor `useVoiceFlowStore` for async listener registration; remove redundant `paste:focus-restore-failed` listener; add `dismissError()` + `audio:recording-aborted` listener; emit `voice-flow:state-changed` on transitions.

### Files
- **Modify**: `src/stores/useVoiceFlowStore.ts`
- **Modify**: `src/__tests__/useVoiceFlowStore.test.ts`
- **Modify**: `src/main.ts` (HUD entry — await async init)

### Tasks for the implementer subagent

- [ ] **Task 1.1: Refactor `init()` to async with sequential awaits**
  - Reference spec §7.1 for exact pattern
  - Change signature: `function init(): () => void` → `async function init(): Promise<() => void>`
  - Remove all `void listenToEvent(...).then((unlisten) => unlistenFns.push(unlisten))` patterns; replace with `unlistenFns.push(await listenToEvent(...))`
  - Order matters: register HOTKEY_PRESSED, HOTKEY_RELEASED, HOTKEY_TOGGLED, ESCAPE_PRESSED first (user-driven); then AUDIO_RECORDING_ABORTED (cap event); finally cleanup return

- [ ] **Task 1.2: Remove redundant `paste:focus-restore-failed` listener**
  - Delete the `void listenToEvent<PasteFocusRestoreFailedPayload>(PASTE_FOCUS_RESTORE_FAILED, ...)` block entirely (current lines ~301-316)
  - Add comment block above where it WAS: `// REMOVED in M5: paste:focus-restore-failed listener was redundant — handleStop's catch path already handles via FocusRestoreFailed Rust error → handleError. Removing also avoids the state collision in IDEAS.md (chunk 3 reviewer P2). Rust continues to emit the event for future Dashboard tooltip use. Friendly "請手動 Ctrl+V" hint moved into formatError pattern match (Task 1.2.5).`
  - Remove unused `PasteFocusRestoreFailedPayload` import if no longer referenced (vue-tsc will catch)

- [ ] **Task 1.2.5: P0-3 fix — `formatError` pattern-match for `FocusRestoreFailed` to preserve friendly hint**
  - Update `formatError` in `useVoiceFlowStore.ts`:
    ```typescript
    function formatError(err: unknown): string {
      const raw = (() => {
        if (typeof err === "string") return err;
        if (err instanceof Error) return err.message;
        if (err && typeof err === "object" && "message" in err) {
          const msg = (err as { message: unknown }).message;
          if (typeof msg === "string") return msg;
        }
        return "Unknown error";
      })();

      // P0-3: preserve M4's friendly "請手動 Ctrl+V" hint that was previously
      // appended by the (now-removed) paste:focus-restore-failed listener.
      // Pattern match Rust ClipboardError::FocusRestoreFailed Display string.
      if (raw.startsWith("Focus restore failed") || raw.includes("FocusRestoreFailed")) {
        return `${raw}（請手動 Ctrl+V）`;
      }

      return raw;
    }
    ```
  - Add vitest test: `formatError` for FocusRestoreFailed-like input returns string ending with `（請手動 Ctrl+V）`

- [ ] **Task 1.3: Add `dismissError()` method**
  - Reference spec §7.3
  - Add to `useVoiceFlowStore` setup:
    ```typescript
    function dismissError(): void {
      if (status.value === "error") {
        transitionTo("idle", "");
      }
    }
    ```
  - Export in return block alongside `init`, `handleStart`, `handleStop`, `handleCancel`

- [ ] **Task 1.4: Add `audio:recording-aborted` listener**
  - Reference spec §7.4
  - Use existing `RecordingAbortedPayload` type from `@/types/events` (already at line 110)
  - Use existing `AUDIO_RECORDING_ABORTED` constant from `@/composables/useTauriEvents`
  - Map reason to user-facing message (i18n later — for now, hard-coded zh-TW string OK):
    - `'max_size'` → "錄音超過上限（~13 分鐘 @ 16 kHz）"
    - `'mic_unplug'` → "麥克風已拔除"
  - Listener body:
    ```typescript
    unlistenFns.push(
      await listenToEvent<RecordingAbortedPayload>(AUDIO_RECORDING_ABORTED, (event) => {
        const reasonMsg = event.payload.reason === 'max_size'
          ? "錄音超過上限（~13 分鐘 @ 16 kHz）"
          : "麥克風已拔除";
        // Reuse existing handleError to ensure auto-revert + state collision logic
        handleError(new Error(reasonMsg));
      })
    );
    ```

- [ ] **Task 1.5: Emit `voice-flow:state-changed` in `transitionTo()`** (P0-1 fold-in: emitTo + source)
  - Reference spec §6.1 + §7.5
  - Import `emitTo` from `@tauri-apps/api/event` (NOT `emit` — explicit Dashboard-only target prevents echo loop)
  - Refactor `transitionTo` from sync to async:
    ```typescript
    import { emitTo } from "@tauri-apps/api/event";

    async function transitionTo(next: VoiceFlowStatus, msg: string): Promise<void> {
      status.value = next;
      message.value = msg;
      // Best-effort emit; failure here doesn't break the state machine.
      try {
        await emitTo("main-window", "voice-flow:state-changed", {
          status: next,
          message: msg,
          source: "hud",
        });
      } catch (err) {
        console.warn("[voice-flow] cross-window emit failed", err);
      }
    }
    ```
  - All callers (`handleStart`, `handleStop`, `handleCancel`, `handleError`, `dismissError`, etc.) currently `transitionTo(...)` — they're already in async functions or void contexts; review each call site:
    - `handleStart`/`handleStop`/`handleCancel`: change `transitionTo(...)` to `await transitionTo(...)` (these are already `async`)
    - `handleError`: not async — change to `void transitionTo(...)` (don't block error-path; emit is best-effort)
    - `setTimeout` callbacks: `void transitionTo("idle", "")` (best-effort)

- [ ] **Task 1.5.1: P1-8 — wrap `position_hud_for_active_monitor` invoke in try/catch in `handleStart`**
  ```typescript
  async function handleStart(): Promise<void> {
    if (status.value === "recording") return;
    const mySession = ++currentSession;
    try {
      // P1-8: positioning failure must NOT block recording (decorative vs core)
      try {
        await invoke<void>("position_hud_for_active_monitor");
      } catch (err) {
        console.warn("[voice-flow] HUD positioning failed, using fallback", err);
      }
      await invoke<void>("capture_target_window");
      await invoke<void>("start_recording", { deviceName: null });
      // ... rest unchanged
    } catch (err) {
      if (mySession === currentSession) handleError(err);
    }
  }
  ```

- [ ] **Task 1.6: Update `useVoiceFlowStore.test.ts`**
  - Async init: tests now await `await store.init()` and call `cleanup()` to teardown
  - Mock `emit` from `@tauri-apps/api/event` via `vi.hoisted` pattern (already in use for `listenToEvent`):
    ```typescript
    const { emitMock, callbacks } = vi.hoisted(() => ({
      emitMock: vi.fn(),
      callbacks: new Map<string, (event: unknown) => void>(),
    }));
    vi.mock("@tauri-apps/api/event", () => ({
      emit: emitMock,
    }));
    ```
  - 6 existing tests still pass (idle → recording → transcribing → success → idle path; ESC cancel; rapid press session counter)
  - **New tests** (≥ 3):
    - `dismissError() transitions error → idle`
    - `dismissError() noop when status is not error`
    - `audio:recording-aborted with reason='max_size' triggers error path`
    - `transitionTo emits voice-flow:state-changed with correct payload`

- [ ] **Task 1.7: Update HUD entry `src/main.ts`**
  - Find current `useVoiceFlowStore().init()` call (likely after Pinia setup)
  - Refactor to:
    ```typescript
    const store = useVoiceFlowStore();
    store.init().then((cleanup) => {
      (window as { __voiceFlowCleanup?: () => void }).__voiceFlowCleanup = cleanup;
    }).catch((err) => {
      console.error("[hud] voice flow init failed", err);
      // Phase 1: log only; M5 doesn't render an init-failure state. M9 polish.
    });
    ```

- [ ] **Task 1.8: Static checks + commit**
  - `pnpm exec vue-tsc --noEmit` → 0 errors
  - `pnpm exec eslint src/stores/useVoiceFlowStore.ts src/__tests__/useVoiceFlowStore.test.ts src/main.ts` → 0 / 0
  - `pnpm test --run useVoiceFlowStore` → all pass (existing 6 + new 3 = 9)
  - Commit:
    ```
    feat(m5): chunk 1 — useVoiceFlowStore async + dismissError + abort listener

    - init() async refactor: sequential awaits, no Promise<.then(push)> race
    - removed paste:focus-restore-failed listener (redundant w/ handleStop catch)
    - added dismissError() store method (used by HUD click-to-dismiss in chunk 2)
    - added audio:recording-aborted listener: max_size + mic_unplug → error
    - transitionTo() emits voice-flow:state-changed for Dashboard sidebar
    - 3 new vitest tests, total 9 store tests pass

    Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
    ```

### Acceptance for Chunk 1

- ✅ `pnpm test --run useVoiceFlowStore` → 9 tests pass
- ✅ `pnpm exec vue-tsc --noEmit` → 0 errors
- ✅ `pnpm exec eslint src/` → 0 / 0
- ✅ HUD entry main.ts compiles, no top-level await issues
- ✅ Manual: `pnpm tauri dev` 啟動 HUD 顯示 placeholder（M1 pong-counter 仍在、會在 chunk 2 移除）、按熱鍵 trigger 流程成功

### Reviewer dispatch（Chunk 1 完工後）

Subagent type `general-purpose`, model `opus`, foreground.

**Reviewer prompt** (full):
```
You are a code reviewer for TalkType M5 chunk 1 (useVoiceFlowStore async refactor). Read commit `<chunk1_sha>` diff + `docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md` §6, §7. Verify:

1. **Async init correctness**: Sequential awaits — events between init() call and resolution should NOT be lost. Tauri's listen() registers atomically; verify ordering is correct.
2. **paste:focus-restore-failed removal**: Confirm the FocusRestoreFailed Rust error path → handleStop catch → handleError still surfaces the error correctly. Check that no orphaned import remains. Check IDEAS.md collision concern is addressed.
3. **dismissError**: Status guard correct (only acts when status === 'error'). Idempotent.
4. **audio:recording-aborted handler**: i18n strategy reasonable for M5 (hardcoded zh-TW now, M5 chunk 2 should consider i18n key)? Reason mapping covers both M2 emit values?
5. **transitionTo emit**: Best-effort try/catch; emit failure doesn't break state machine. Cross-window event reaches Dashboard? (Manual test in chunk 3, but verify pattern.)
6. **Test coverage**: existing 6 tests still pass after async refactor? New tests cover happy + edge?

P0 / P1 / P2 findings with file:line. End with PROCEED / FIX P0 / NEEDS DISCUSSION.
```

---

## Chunk 2 — HUD components + i18n

**Goal**: Replace M1 placeholder `HudOverlay.vue` with real 4-state machine; create 3 sub-components (HudWaveform, HudSpinner, HudTimer); add i18n keys; HUD-side initial Playwright screenshot pass.

### Files
- **Modify**: `src/components/HudOverlay.vue` (full replacement)
- **Create**: `src/components/HudWaveform.vue`
- **Create**: `src/components/HudSpinner.vue`
- **Create**: `src/components/HudTimer.vue`
- **Create**: `src/__tests__/HudOverlay.test.ts`
- **Create**: `src/__tests__/HudTimer.test.ts`
- **Create**: `src/__tests__/HudWaveform.test.ts`
- **Modify**: `src/i18n/locales/zh-TW.json`
- **Modify**: `src/i18n/locales/en.json`

### Tasks for the implementer subagent

- [ ] **Task 2.0.5: P1-3 — harden `src/composables/useAudioWaveform.ts` against rapid mount/unmount race**
  - Add `starting` flag to short-circuit re-entry while `await listenToEvent` in flight：
    ```typescript
    let starting = false;
    async function start() {
      if (unlisten || starting) return;
      starting = true;
      try {
        unlisten = await listenToEvent<WaveformPayload>(AUDIO_WAVEFORM, (event) => {
          const { levels } = event.payload;
          for (let i = 0; i < BAR_COUNT; i++) targetLevels[i] = levels[i] ?? 0;
        });
        raf = requestAnimationFrame(tick);
      } finally {
        starting = false;
      }
    }
    ```
  - 不修 stop（既有實作 OK）

- [ ] **Task 2.1: Create `HudWaveform.vue`** — reference spec §8.2 for full code

- [ ] **Task 2.2: Create `HudSpinner.vue`** — reference spec §8.3 for full code

- [ ] **Task 2.3: Create `HudTimer.vue`** — reference spec §8.4 for full code; **note**: setInterval cleanup correct in onUnmounted

- [ ] **Task 2.4: Replace `HudOverlay.vue`** — reference spec §8.1 for full code (P1-1 fold-in)
  - Delete entire M1 placeholder body (pong counter)
  - Implement state machine with `<Transition mode="out-in">` + sub-components
  - `setIgnoreCursorEvents` toggle in `watch(() => store.status, ...)`
  - Click handler routes to `dismissError` only when status === 'error'
  - `prefers-reduced-motion` reactive ref via `window.matchMedia(...)` + `addEventListener('change', ...)`
  - ARIA: root `role="status" aria-live="polite" :aria-label="ariaMessage"`; sub-icons `aria-hidden="true"`
  - **P1-1 ellipsis CSS**：spec §8.1 styles 已加 `text-overflow: ellipsis` + `max-width: 280px` 在 `.bubble-label` — 確保 implement 時複製這 4 行 CSS

- [ ] **Task 2.5: i18n keys** — reference spec §9 for full table
  - Add to both `zh-TW.json` + `en.json`: 7 keys
  - Update `hud.title` / `hud.pongCount` keys: REMOVE (no longer used after placeholder gone)

- [ ] **Task 2.6: Vitest tests**
  - **`src/__tests__/HudOverlay.test.ts`** (estimate ~120 LOC, 5 tests):
    - mount with mocked Pinia store status='idle' → bubble not rendered
    - status='recording' → HudWaveform + HudTimer mounted
    - status='success' → CheckCircle2 + label rendered
    - status='error' → click root → store.dismissError() called once
    - status='error' → setIgnoreCursorEvents called with `false`; status='recording' → with `true`
  - Mock `getCurrentWindow` from `@tauri-apps/api/window`:
    ```typescript
    const { setIgnoreCursorEventsMock } = vi.hoisted(() => ({
      setIgnoreCursorEventsMock: vi.fn(),
    }));
    vi.mock("@tauri-apps/api/window", () => ({
      getCurrentWindow: () => ({ setIgnoreCursorEvents: setIgnoreCursorEventsMock }),
    }));
    ```
  - Mock `window.matchMedia`:
    ```typescript
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    });
    ```
  - **`src/__tests__/HudTimer.test.ts`** (~80 LOC, 5 tests — P1-5 add null regression test):
    - mm:ss formatting at various elapsed seconds (0, 59, 60, 599, 720)
    - colorClass: < 540s → muted; ≥ 540s → yellow; ≥ 720s → red
    - startedAtMs=null → renders 0:00 with no interval running (P1-5 regression test)
    - startedAtMs=null → no errors / no warnings emitted to console (P1-5)
    - vi.useFakeTimers() advance 1s → reactive update
  - **`src/__tests__/HudWaveform.test.ts`** (~60 LOC, 2 tests):
    - reducedMotion=false → 6 bar elements rendered
    - reducedMotion=true → 1 dot element rendered
    - Mock `useAudioWaveform` to return stable smoothedLevels

- [ ] **Task 2.7: Playwright vite-shape screenshots**
  - Run `pnpm dev` in vite-only mode (no Tauri runtime; mock approach for status changes)
  - Implementer uses Playwright MCP tools (`mcp__plugin_playwright_playwright__browser_*`) to:
    - Navigate to `http://localhost:1420` (HUD entry)
    - Use `browser_evaluate` to set `useVoiceFlowStore().status` directly via `window` access — OR temporarily expose a dev mutation method in HudOverlay for testing (gated by `import.meta.env.DEV`)
    - Take screenshot per state → save under `docs/screenshots/m5/`
  - **13 screenshots** per spec §11.2: idle, recording-empty, recording-active, recording-warn-yellow, recording-warn-red, transcribing, success, error, error-after-click, reduced-motion-recording, reduced-motion-transcribing, dashboard-sidebar-badge (chunk 3 will add 12-13), dashboard-sidebar-idle (chunk 3)
  - **Read each screenshot** with `Read` tool to visually confirm
  - **Note**: Vite-only mode `useAudioWaveform` won't have real Rust events; mock by setting `smoothedLevels` directly via dev hook OR by directly editing the component for screenshot session (revert before commit)

- [ ] **Task 2.8: Static checks + commit**
  - `pnpm exec vue-tsc --noEmit && pnpm exec eslint src/` → 0 / 0
  - `pnpm test --run` → all tests pass (M4 baseline ≥16 + chunk 1 +3 + chunk 2 +11 ≈ 30)
  - Commit:
    ```
    feat(m5): chunk 2 — HUD components + ARIA + reduced-motion + i18n

    - HudOverlay.vue rewrite: 4 visual states, <Transition mode=out-in>,
      ARIA (role=status + aria-live + aria-label), prefers-reduced-motion
      via matchMedia, click-through toggle on error state, dismiss handler
    - HudWaveform.vue: 6-bar visualizer + reduced-motion fallback to dot
    - HudSpinner.vue: CSS spin + reduced-motion fallback to "..." text
    - HudTimer.vue: mm:ss + 9/12 min warning colors via setInterval
    - i18n: hud.aria.*, hud.transcribing, hud.success (zh-TW + en)
    - 11 new vitest tests across 3 files
    - 11 vite-shape screenshots in docs/screenshots/m5/

    Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
    ```

### Acceptance for Chunk 2

- ✅ All 11 new vitest tests pass
- ✅ vue-tsc + eslint clean
- ✅ 11 Playwright screenshots taken + visually confirmed by Read
- ✅ Manual: `pnpm tauri dev` 按熱鍵 → HUD 顯示 6-bar waveform + timer mm:ss + transitions 平滑

### Reviewer dispatch（Chunk 2 完工後）

Subagent type `general-purpose`, model `opus`, foreground. **MUST 跑自己的 Playwright screenshots**（user 強調）。

**Reviewer prompt** (full):
```
You are a code reviewer for TalkType M5 chunk 2 (HUD components). Read commit `<chunk2_sha>` diff + `docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md` §2, §5, §8, §11.2 sections, and `.claude/PROGRESS.md` for context.

**MANDATE: You must run your OWN Playwright screenshot pass — DO NOT just look at the implementer's screenshots. Per CLAUDE.md feedback memory `feedback_ui_screenshots.md`, independent passes catch different bugs (e.g. focus rings, ::-ms-reveal native UI, transition timing).**

Steps:
1. `cd <worktree path>; pnpm dev` (vite-only mode), wait for vite ready on port 1420
2. Use Playwright MCP `mcp__plugin_playwright_playwright__browser_*` tools:
   - browser_navigate to http://localhost:1420
   - For each of 4 states + 2 reduced-motion variants:
     - Use `browser_evaluate` to mutate the Pinia store status (the component should expose a dev hook OR use `window.__pinia__` accessor)
     - browser_take_screenshot
3. Read each screenshot via Read tool (multimodal) — DO NOT skip this step
4. Compare visually against `docs/screenshots/m5/` (implementer's pass)
5. Differences = potential bug

Then verify code:
- **Spec §2 state mapping**: each status' visual / click-through / auto-hide behavior matches spec exactly?
- **Spec §5 ARIA**: role, aria-live, aria-label all present? aria-hidden on icons? Screen reader would announce 4 distinct messages?
- **Spec §5.2 reduced-motion**: 3-layer fallback (waveform / spinner / fade) all implemented? Static / animated paths verified by your screenshots?
- **Spec §4.2 click-through**: setIgnoreCursorEvents called on EVERY status change? watcher correctly placed?
- **Spec §8 component sizes**: each component file < 200 LOC?
- **CLAUDE.md compliance**:
  - shadcn-vue Google Fonts trap: grep `fonts.googleapis.com` in src/assets/index.css → must NOT exist
  - lucide-vue-next imports correct (CheckCircle2 + XCircle)?
  - No emojis in code unless user requested
  - Comments minimal (only WHY non-obvious)
- **Test coverage**: state transitions, click handler, reduced-motion fallback, setIgnoreCursorEvents calls all tested?

Output: P0 / P1 / P2 findings with file:line. **Include screenshots taken (paths) in report.** End with PROCEED / FIX P0 / NEEDS DISCUSSION.

Cap response 1500 words.
```

---

## Chunk 3 — Dashboard sidebar HudFlowBadge + dev tooling

**Goal**: Add `HudFlowBadge.vue` to Dashboard sidebar listening to `voice-flow:state-changed`; verify dev visibility tooling end-to-end.

### Files
- **Create**: `src/components/HudFlowBadge.vue` (reference spec §8.5)
- **Create**: `src/__tests__/HudFlowBadge.test.ts`
- **Modify**: `src/MainApp.vue` or sidebar layout (mount `<HudFlowBadge />`)
- **Modify**: `src/i18n/locales/zh-TW.json` + `en.json` (`sidebar.recordingBadge`)

### Tasks for the implementer subagent

- [ ] **Task 3.1: Create `HudFlowBadge.vue`** — reference spec §8.5 for full code
  - **Important**: Use `listenToEvent` (existing helper at `@/composables/useTauriEvents`) — DO NOT directly import `listen` from `@tauri-apps/api/event` per CLAUDE.md "唯一 import @tauri-apps/api/event 的地方是 src/composables/useTauriEvents.ts"

- [ ] **Task 3.2: Mount `<HudFlowBadge />` in Dashboard**
  - Read `src/MainApp.vue` (Dashboard root) — find sidebar slot
  - Add `<HudFlowBadge />` at sidebar bottom (above any footer if present)
  - Import HudFlowBadge

- [ ] **Task 3.3: i18n key**
  - Add to both locale files:
    - zh-TW: `"sidebar": { "recordingBadge": "錄音中" }`
    - en: `"sidebar": { "recordingBadge": "Recording" }`

- [ ] **Task 3.4: Vitest test `HudFlowBadge.test.ts`** (~50 LOC, 2 tests)
  - mount → triggers listenToEvent registration → simulated event payload {status:'recording'} → badge visible
  - simulated event payload {status:'idle'} → badge hidden
  - Mock `listenToEvent` via vi.hoisted pattern (mirror useVoiceFlowStore.test.ts pattern)

- [ ] **Task 3.5: Manual integration check**
  - Run `pnpm tauri dev`
  - Press hotkey → recording starts → Dashboard sidebar shows red dot + "錄音中"
  - Release hotkey → transcribing starts → sidebar hides badge
  - Verify: cross-window event reaches Dashboard correctly

- [ ] **Task 3.6: Playwright screenshots (sidebar variants)**
  - 2 screenshots: badge visible (mocked status='recording') + badge hidden (status='idle')
  - Save as `docs/screenshots/m5/m5-dashboard-sidebar-badge.png` + `m5-dashboard-sidebar-idle.png`
  - Read both for visual confirmation

- [ ] **Task 3.7: Static checks + commit**
  - vue-tsc + eslint + vitest clean
  - Commit:
    ```
    feat(m5): chunk 3 — HudFlowBadge sidebar + voice-flow:state-changed listener

    - HudFlowBadge.vue: red pulsing dot + "錄音中" on status='recording', hidden otherwise
    - listens to voice-flow:state-changed cross-window event from HUD
    - mounted in Dashboard sidebar
    - i18n: sidebar.recordingBadge (zh-TW + en)
    - 2 vitest tests
    - 2 Playwright screenshots (badge + idle states)

    Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
    ```

### Acceptance for Chunk 3

- ✅ Vitest 2 new tests pass
- ✅ Manual: `pnpm tauri dev` press hotkey → Dashboard sidebar badge visible during recording
- ✅ 2 screenshots taken + Read confirmed

### Reviewer dispatch（Chunk 3 完工後）

Subagent type `general-purpose`, model `opus`, foreground.

**Reviewer prompt** (key points):
- Verify `listenToEvent` (NOT direct `listen`) used per CLAUDE.md
- Cross-window event payload type matches `VoiceFlowStateChangedPayload` (chunk 0)
- Badge accessibility (`role="status" aria-live="polite"` per spec §8.5)
- Reviewer must run own Playwright pass on Dashboard (`pnpm dev` + simulate event via `browser_evaluate('emit(...)')`)
- 5-state coverage: badge visible / hidden across `recording` / `transcribing` / `success` / `error` / `idle`
- Output P0/P1/P2 + PROCEED/FIX/NEEDS DISCUSSION

---

## Chunk 4 — Acceptance docs + screenshots compare + retro

**Goal**: Document manual acceptance SOP; consolidate all screenshots; update PROGRESS dashboard; create session log.

### Files
- **Create**: `docs/m5-acceptance.md` (14 conditions per spec §11)
- **Verify**: `docs/screenshots/m5/` complete (13 .png files)
- **Modify**: `.claude/PROGRESS.md`
- **Modify**: `doc/plans/02-implementation-roadmap.md` (dashboard `M5 → ✅ Done`, 最後更新 date bump)
- **Create**: `.claude/sessions/2026-05-05-m5-hud-overlay.md` (session log)

### Tasks for the implementer subagent

- [ ] **Task 4.1: `docs/m5-acceptance.md`**
  - Mirror M4's `docs/m4-acceptance.md` structure: introduction + table of conditions + per-condition section with "設定 / 步驟 / 預期" subsections
  - 14 conditions copied from spec §11
  - Add screenshots referenced (e.g. "預期見 [m5-hud-recording-active.png](screenshots/m5/m5-hud-recording-active.png)")

- [ ] **Task 4.2: Verify all 13 screenshots present**
  - `ls docs/screenshots/m5/*.png | wc -l` → 13
  - If any missing, re-take via Playwright

- [ ] **Task 4.3: Update PROGRESS.md**
  - Update "現在在哪" line: M5 ✅ Done @ 2026-05-05
  - Add row to "最近的 session" table linking to new session log
  - Update "下個 session 接手 SOP" to point to M6 (LLM polish 多 provider)

- [ ] **Task 4.4: Update roadmap dashboard**
  - In `doc/plans/02-implementation-roadmap.md`, change M5 row to:
    `| M5：HUD overlay | ✅ Done | 2026-05-05 | 2026-05-05 |`
  - Bump 「最後更新」 to 2026-05-05
  - Mark M5 task list checkboxes `[x]` (or add note "see `docs/superpowers/plans/2026-05-05-m5-hud-overlay-plan.md` for chunk-level tracking")

- [ ] **Task 4.5: Session log `.claude/sessions/2026-05-05-m5-hud-overlay.md`**
  - Sections: topic, outcome, what changed (per chunk + commits), key decisions, surprises / 踩雷, follow-ups, acceptance criteria checklist
  - Include all chunk SHAs for traceability
  - Append "M5 retro challenger findings" section once challenger returns

- [ ] **Task 4.6: Commit**
  ```
  docs(m5): acceptance SOP + session log + dashboard update

  - docs/m5-acceptance.md with 14 conditions (auto + manual)
  - docs/screenshots/m5/ verified 13 vite-shape PNGs
  - PROGRESS.md M5 done + M6 next-session SOP
  - roadmap dashboard M5 -> ✅ Done
  - session log .claude/sessions/2026-05-05-m5-hud-overlay.md

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  ```

### Acceptance for Chunk 4

- ✅ `docs/m5-acceptance.md` exists, ≥ 14 conditions
- ✅ 13 screenshots in `docs/screenshots/m5/`
- ✅ PROGRESS.md + roadmap reflect M5 done
- ✅ Session log includes all key info

---

## Retro challenger（Chunk 4 完工後 — M5 完整 ship 前最後關卡）

Subagent type `general-purpose`, model `opus`, foreground.

**Mandate**: read full chunk 0-4 commits + session log + spec; run own Playwright pass on the integrated HUD + Dashboard; find latent UX / perf / accessibility / 邊界條件 issues that chunk-level reviewers may have missed; append findings to `.claude/IDEAS.md` for M6 / M9 / Phase 2 consideration.

**Retro challenger prompt** (full):
```
You are a retro challenger subagent for TalkType M5 (HUD Overlay) — final independent review BEFORE shipping.

CLAUDE.md item 6 mandates this retro pass: chunk-level reviewers see commits in isolation; YOU look at the integrated whole and find latent issues they couldn't catch alone.

Read in this order:
1. `docs/superpowers/specs/2026-05-05-m5-hud-overlay-design.md` (the design contract)
2. `docs/superpowers/plans/2026-05-05-m5-hud-overlay-plan.md` (this plan)
3. `git log --oneline -20` and review commits matching `feat(m5):` and `docs(m5):`
4. `.claude/sessions/2026-05-05-m5-hud-overlay.md` (session log)
5. Final state of:
   - `src/components/HudOverlay.vue` + `Hud{Waveform,Spinner,Timer,FlowBadge}.vue`
   - `src/stores/useVoiceFlowStore.ts`
   - `src-tauri/src/plugins/hud.rs`
   - `src-tauri/tauri.conf.json`

**MANDATE: You must run your OWN Playwright screenshot pass — DO NOT just trust the implementer's or chunk reviewer's screenshots.**

Steps:
1. `pnpm dev` (vite-only mode for HUD), wait for ready
2. Take ≥ 6 screenshots:
   - HUD all 4 visual states (recording / transcribing / success / error)
   - HUD reduced-motion ON (recording + transcribing variants)
   - Dashboard sidebar badge (recording + idle)
3. Read each via Read tool, compare against spec §2 state mapping table

Then look for:

**Integration cracks**:
- Does Vue Transition mode="out-in" + setIgnoreCursorEvents await sequence have a race?
- Cross-window emit on transitionTo + Dashboard listener: timing OK in real Tauri runtime?
- prefers-reduced-motion change mid-state — does animation cancel correctly?
- HUD bubble width 380px vs longest possible error message — truncation policy?
- HudTimer setInterval drift over long recording (toggle mode 30+ min)?
- audio:recording-aborted received during transcribing state — handled correctly?

**UX latent**:
- 200ms transition feels right or sluggish?
- Click-anywhere-dismiss on error state actually clear UX?
- Toggle mode user has no visual cue they're in toggle mode (vs hold) — Phase 2?
- Multi-DPI: HUD 380x56 logical readable on 4K @ 100% scale?
- success state 1s linger — too short to register?

**Accessibility audit**:
- aria-live="polite" appropriate vs "assertive" for errors? NVDA / JAWS verify needed?
- aria-label on root vs aria-labelledby with separate label element — convention?
- Dashboard sidebar badge `role="status"` semantics correct vs role="alert"?
- focus management: HUD opens, does focus shift? Should it not?

**Performance latent**:
- audio:waveform 60fps + Vue reactivity + lerp RAF + setInterval timer + transitionTo emit — total frame budget?
- HudOverlay re-renders on every store mutation — memoize? Read-only refs already.
- Dashboard sidebar badge listener never unsubscribes if Dashboard window stays open across multiple voice flows — leak?

**Architecture concerns**:
- Single-source-of-truth question: HUD state lives in Vue store, but Dashboard reads via cross-window event. Phase 2 may need unified state — design this exposure right?
- M5 introduces window-level position state (380x56 fixed) — Phase 2 (LLM polish, longer error messages) may need dynamic sizing — coupling cost?

**Out-of-scope concerns flagged by spec but worth confirming**:
- mute_on_recording deferred to M8 — verify no leakage
- LLM polish enhancing state stub — should HUD partially support for forward compat?

**Output**:
- P1/P2 findings (P0 should not exist — chunks already passed reviewers)
- Each finding: priority + file:line + concrete suggestion + which session/file to track to (M6 / M8 / M9 / Phase 2 / IDEAS)
- ≥ 6 findings expected; if fewer, justify
- Aim for 10-15 findings
- Append to `.claude/IDEAS.md` under section "## M5 retro challenger findings (2026-05-05)"
- Final assessment: M5 SHIP READY / DO NOT SHIP — concrete blocker if not ready
- Cap 2000 words

Verify Playwright passes complete by listing screenshot paths in report.
```

After challenger:
- Main session reads challenger output
- P1 findings: discuss with user; if user wants fixes in M5, dispatch a hotfix chunk; if not, append to `.claude/IDEAS.md`
- P2 findings: append to `.claude/IDEAS.md` for M6+ consideration
- M5 ship: open PR `claude/m5-hud-overlay` → user merge after CI passes

---

## Summary checklist (entire M5)

- [ ] Chunk 0: Rust + types + tauri.conf — implementer + reviewer
- [ ] Chunk 1: useVoiceFlowStore async — implementer + reviewer
- [ ] Chunk 2: HUD components + i18n + screenshots — implementer + reviewer (with Playwright)
- [ ] Chunk 3: Dashboard sidebar — implementer + reviewer (with Playwright)
- [ ] Chunk 4: Docs + dashboard + session log — implementer
- [ ] Retro challenger (with Playwright)
- [ ] User manual acceptance: multi-monitor, multi-DPI, screen reader, click-through (acceptance criteria #6, #7, #9, #11)
- [ ] Open PR + user merge
- [ ] Update `.claude/PROGRESS.md` to M6 entry

---

## Self-review (writing-plans skill checklist)

**1. Spec coverage:** Each spec section has at least one task:
- §1 Scope — covered across chunks
- §2 State mapping — Task 2.4 (HudOverlay state machine)
- §3 Position — Task 0.1 + 0.4
- §4 HUD window — Task 0.4 + 2.4 (click-through toggle)
- §5 Accessibility — Task 2.4 (ARIA + reduced-motion)
- §6 Cross-window event — Task 1.5 + 3.1 + 3.2
- §7 Store changes — Tasks 1.1-1.5
- §8 Components — Tasks 2.1-2.4 + 3.1
- §9 i18n — Task 2.5 + 3.3
- §10 Rust — Tasks 0.1-0.3
- §11 Acceptance — Chunk 4 (Task 4.1) + Playwright in chunks 2/3 + retro
- §12 Testing — Tasks 0.3, 1.6, 2.6, 3.4
- §13 Chunking — this plan IS the implementation
- §14 Risks — covered in reviewer prompts (race, DPI, multi-monitor)

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" except the explicit refinements section (waiting for challenger). All other steps have concrete file paths + code references to spec sections.

**3. Type consistency:**
- `VoiceFlowStateChangedPayload` defined in chunk 0 (interface), used in chunk 1 (emit) + chunk 3 (listen).
- `VoiceFlowStatus` from `useVoiceFlowStore` import path consistent (`@/stores/useVoiceFlowStore`).
- `RecordingAbortedPayload` reused from `@/types/events:110` (M2 already exported).

---

## Execution choice

This plan is the implementation contract. Per `CLAUDE.md` item 1-3 + user explicit instruction (「實作的部分盡量請subagent」):

**Subagent-driven execution** is the only path for M5. Each chunk = 1 implementer subagent (`model: opus`). Each chunk + 1 reviewer subagent (`model: opus`). M5 finale = 1 retro challenger subagent (`model: opus`). Main session orchestrates only.

After plan-time challenger returns and findings folded in, main session dispatches Chunk 0 implementer.
