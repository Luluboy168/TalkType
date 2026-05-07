// Unit tests for the voice flow Pinia store (M4 chunk 3 + M5 chunk 1).
//
// Covers the state machine transitions through `handleStart` / `handleStop` /
// `handleCancel` / `dismissError` with `invoke` mocked. M5 chunk 1 adds:
//   * async `init()` (was sync) — tests now `await store.init()`
//   * `audio:recording-aborted` listener wiring (size cap / mic unplug)
//   * `dismissError()` user-driven recovery
//   * `transitionTo()` emits `voice-flow:state-changed` to Dashboard
//   * `formatError` pattern matches Rust `FocusRestoreFailed` → friendly hint
//
// Listener tests work by capturing the Tauri `listen()` callback for each
// event and invoking it directly — that's the same pattern useTauriEvents
// uses everywhere else and avoids fragile real-OS event setup.
import { setActivePinia, createPinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock Tauri's invoke BEFORE importing the store — the store imports invoke
// at module load time, so the mock must be registered first.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Capture listen callbacks so tests can invoke them synchronously without
// needing real OS event plumbing. `emitTo` is also captured so tests can
// assert cross-window broadcasts. `vi.hoisted` is required because vi.mock
// factories run BEFORE the file's top-level statements.
const { listenMock, emitToMock, listenCallbacks } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  emitToMock: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
  emit: vi.fn().mockResolvedValue(undefined),
  emitTo: emitToMock,
}));

import { invoke } from "@tauri-apps/api/core";
import { useVoiceFlowStore } from "@/stores/useVoiceFlowStore";

const mockInvoke = vi.mocked(invoke);

/**
 * Drive the store through a full pipeline up to the recording state by
 * stubbing out the trio of invokes `handleStart` issues
 * (position_hud_for_active_monitor + capture_target_window + start_recording).
 *
 * Centralized here so the M5 chunk 1 addition of positioning doesn't force
 * every test to know the new invoke order.
 */
function mockHandleStartInvokes(): void {
  mockInvoke.mockResolvedValueOnce(undefined); // position_hud_for_active_monitor
  mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
  mockInvoke.mockResolvedValueOnce(undefined); // start_recording
}

describe("useVoiceFlowStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
    listenMock.mockReset();
    emitToMock.mockReset();
    listenCallbacks.clear();
    // Default: each `listen(name, cb)` stashes the callback and returns a
    // no-op unlisten fn. Tests that need to drive a specific event invoke
    // `listenCallbacks.get(name)` after `await store.init()`.
    listenMock.mockImplementation(
      (
        name: string,
        cb: (event: { payload: unknown }) => void,
      ): Promise<() => void> => {
        listenCallbacks.set(name, cb);
        return Promise.resolve(() => {
          listenCallbacks.delete(name);
        });
      },
    );
    emitToMock.mockResolvedValue(undefined);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts in idle state with empty message and null timestamp", () => {
    const store = useVoiceFlowStore();
    expect(store.status).toBe("idle");
    expect(store.message).toBe("");
    expect(store.recordingStartedAtMs).toBeNull();
  });

  it("handleStart from idle invokes position + capture + start_recording and transitions to recording", async () => {
    mockHandleStartInvokes();

    const store = useVoiceFlowStore();
    await store.handleStart();

    expect(mockInvoke).toHaveBeenNthCalledWith(
      1,
      "position_hud_for_active_monitor",
    );
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "capture_target_window");
    expect(mockInvoke).toHaveBeenNthCalledWith(3, "start_recording", {
      deviceName: null,
    });
    expect(store.status).toBe("recording");
    expect(store.recordingStartedAtMs).toBeTypeOf("number");
  });

  it("handleStart is a no-op when already recording", async () => {
    // Simulate the store already being in "recording" — handleStart should
    // bail before any invoke call.
    const store = useVoiceFlowStore();
    mockInvoke.mockResolvedValue(undefined);
    await store.handleStart(); // idle → recording
    mockInvoke.mockClear();
    await store.handleStart(); // recording → no-op
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(store.status).toBe("recording");
  });

  it("handleStart from success state re-triggers (rapid press fix)", async () => {
    // M4 acceptance fix: after release the flow goes through transcribing →
    // success (1s linger) → idle. Old code only allowed handleStart from
    // idle, dropping fresh presses during the 2-3s gap. The relaxed guard
    // lets the user re-trigger as soon as paste completes (success state).
    const store = useVoiceFlowStore();

    // Drive through a full pipeline to land in "success".
    mockHandleStartInvokes();
    await store.handleStart();
    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "first",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text
    await store.handleStop();
    expect(store.status).toBe("success");

    // Press again BEFORE the 1s success linger expires. Old behavior was
    // a silent no-op; new behavior is a fresh recording.
    mockInvoke.mockClear();
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording");
    expect(mockInvoke).toHaveBeenNthCalledWith(
      1,
      "position_hud_for_active_monitor",
    );
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "capture_target_window");
    expect(mockInvoke).toHaveBeenNthCalledWith(3, "start_recording", {
      deviceName: null,
    });
  });

  it("handleStart during transcribing starts a new recording in parallel (rapid press v2)", async () => {
    // M4 acceptance fix v2: previous version blocked handleStart during
    // transcribing, but that meant pressing within ~1s of release (still
    // in the Groq round-trip window) silently no-op'd. Now we allow start;
    // the old transcribe + paste finish in the background via the paste
    // serialization chain.
    const store = useVoiceFlowStore();

    // Get into recording.
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording");

    // Stop, let transcribe_audio hang so we stay in "transcribing".
    let resolveTranscribe1: (value: unknown) => void = () => {};
    const transcribe1 = new Promise((resolve) => {
      resolveTranscribe1 = resolve;
    });
    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording 1
    mockInvoke.mockReturnValueOnce(transcribe1); // transcribe_audio 1 (pending)
    const stopPromise = store.handleStop();
    // Drain microtasks until handleStop reaches the awaiting transcribe1
    // step. M5 chunk 1's async transitionTo (with internal await emitTo)
    // adds extra microtask ticks vs M4, so we drain a few more rounds.
    for (let i = 0; i < 8; i++) await Promise.resolve();
    expect(store.status).toBe("transcribing");

    // Press during transcribing — should start a new recording, NOT
    // no-op like the previous (overly-conservative) guard did.
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording"); // session 2 took over

    // Now resolve the OLD transcribe — its paste runs through the chain
    // and its terminal transitions should NOT clobber session 2's
    // "recording" status.
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text 1
    resolveTranscribe1({
      rawText: "first",
      transcriptionDurationMs: 50,
      noSpeechProbability: null,
    });
    await stopPromise;
    // Old session's terminal transition is gated by currentSession check,
    // so status stays "recording" for session 2.
    expect(store.status).toBe("recording");
  });

  it("handleStop from recording transitions through transcribing → success → idle and pastes raw text", async () => {
    const store = useVoiceFlowStore();

    // Set up handleStart first so we're in recording state.
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording");

    // Now mock the stop → transcribe → paste sequence.
    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "你好世界",
      transcriptionDurationMs: 250,
      noSpeechProbability: 0.05,
    }); // transcribe_audio
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    expect(mockInvoke).toHaveBeenCalledWith("stop_recording");
    expect(mockInvoke).toHaveBeenCalledWith("transcribe_audio", {
      vocabulary: undefined,
    });
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", { text: "你好世界" });
    expect(store.recordingStartedAtMs).toBeNull();

    // Advance the auto-revert timer → idle. M6 chunk 2 bumped
    // SUCCESS_LINGER_MS from 1000 → 1500 ms (Decision #8).
    vi.advanceTimersByTime(1500);
    expect(store.status).toBe("idle");
  });

  it("handleCancel from recording returns to idle without invoking paste_text", async () => {
    const store = useVoiceFlowStore();

    // Enter recording state.
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording");

    // Set up cancel cleanup invokes.
    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce(undefined); // clear_recording_buffer

    await store.handleCancel();
    expect(store.status).toBe("idle");
    expect(store.recordingStartedAtMs).toBeNull();

    // Crucially: paste_text was NOT called (cancel path skips paste).
    const pasteCall = mockInvoke.mock.calls.find(
      ([cmd]) => cmd === "paste_text",
    );
    expect(pasteCall).toBeUndefined();

    // The cleanup invokes WERE called.
    expect(mockInvoke).toHaveBeenCalledWith("stop_recording");
    expect(mockInvoke).toHaveBeenCalledWith("clear_recording_buffer");
  });

  it("transitions to error on invoke failure and auto-reverts to idle after 3s", async () => {
    const store = useVoiceFlowStore();
    // Reject the very first invoke (position_hud_for_active_monitor) — but
    // because positioning is wrapped in its own try/catch (P1-8), it does
    // NOT trigger error path. We need to fail capture_target_window instead.
    mockInvoke.mockResolvedValueOnce(undefined); // position_hud_for_active_monitor (succeeds, decorative)
    mockInvoke.mockRejectedValueOnce("API key missing for provider groq"); // capture_target_window (proxy for any core failure)
    await store.handleStart();
    expect(store.status).toBe("error");
    expect(store.message).toContain("API key missing");

    vi.advanceTimersByTime(3000);
    expect(store.status).toBe("idle");
  });

  // ─── M5 chunk 1: positioning failure does NOT block recording (P1-8) ────

  it("positioning failure is non-fatal and recording still starts (P1-8)", async () => {
    const store = useVoiceFlowStore();

    // Simulate Rust position_hud_for_active_monitor failing (e.g. monitor
    // probe error). Recording should still proceed because positioning is
    // decorative (HUD stays at last position).
    mockInvoke.mockRejectedValueOnce("monitor probe failed"); // position
    mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
    mockInvoke.mockResolvedValueOnce(undefined); // start_recording
    await store.handleStart();
    expect(store.status).toBe("recording");
    expect(store.recordingStartedAtMs).toBeTypeOf("number");
  });

  // ─── M5 chunk 1: dismissError ────────────────────────────────────────────

  it("dismissError() transitions error → idle", async () => {
    const store = useVoiceFlowStore();
    // Drive into error state by failing capture_target_window.
    mockInvoke.mockResolvedValueOnce(undefined); // position
    mockInvoke.mockRejectedValueOnce("transcribe failed");
    await store.handleStart();
    expect(store.status).toBe("error");

    store.dismissError();
    expect(store.status).toBe("idle");
    expect(store.message).toBe("");
  });

  it("dismissError() is a no-op when status is not error", async () => {
    const store = useVoiceFlowStore();
    // status starts as 'idle'
    expect(store.status).toBe("idle");
    store.dismissError();
    expect(store.status).toBe("idle"); // unchanged

    // Drive to recording — dismissError must NOT clobber it.
    mockHandleStartInvokes();
    await store.handleStart();
    expect(store.status).toBe("recording");
    store.dismissError();
    expect(store.status).toBe("recording"); // unchanged
  });

  // ─── M5 chunk 1: audio:recording-aborted listener ────────────────────────

  it("audio:recording-aborted with reason='max_size' triggers error path", async () => {
    const store = useVoiceFlowStore();
    const cleanup = await store.init();

    const abortCb = listenCallbacks.get("audio:recording-aborted");
    expect(abortCb).toBeDefined();

    abortCb!({
      payload: { reason: "max_size", bytesRecorded: 25_000_000 },
    });

    expect(store.status).toBe("error");
    expect(store.message).toContain("錄音超過上限");

    // Auto-revert after 3s.
    vi.advanceTimersByTime(3000);
    expect(store.status).toBe("idle");

    cleanup();
  });

  it("audio:recording-aborted with reason='mic_unplug' triggers error path", async () => {
    const store = useVoiceFlowStore();
    const cleanup = await store.init();

    const abortCb = listenCallbacks.get("audio:recording-aborted");
    expect(abortCb).toBeDefined();

    abortCb!({
      payload: { reason: "mic_unplug", bytesRecorded: 0 },
    });

    expect(store.status).toBe("error");
    expect(store.message).toBe("麥克風已拔除");

    cleanup();
  });

  // ─── M5 chunk 1: transitionTo emits voice-flow:state-changed (P0-1) ─────

  it("transitionTo emits voice-flow:state-changed via emitTo with source='hud'", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartInvokes();
    await store.handleStart();
    // After handleStart, transitionTo("recording", "") should have fired.

    // Find the recording-state emit. emitTo is (target, event, payload).
    const recordingEmit = emitToMock.mock.calls.find(
      ([_target, _event, payload]) => {
        const p = payload as { status?: string } | undefined;
        return p?.status === "recording";
      },
    );
    expect(recordingEmit).toBeDefined();
    const [target, event, payload] = recordingEmit!;
    expect(target).toBe("main-window");
    expect(event).toBe("voice-flow:state-changed");
    expect(payload).toMatchObject({
      status: "recording",
      message: "",
      source: "hud",
    });
  });

  it("emitTo failure does not break the state machine (best-effort)", async () => {
    emitToMock.mockRejectedValue(new Error("cross-window IPC offline"));

    const store = useVoiceFlowStore();
    mockHandleStartInvokes();
    // handleStart should NOT throw even when emitTo fails internally.
    await expect(store.handleStart()).resolves.toBeUndefined();
    expect(store.status).toBe("recording");
  });

  // ─── M5 chunk 1: formatError P0-3 regression ─────────────────────────────

  it("formatError appends '請手動 Ctrl+V' for actual Rust Display string", async () => {
    // The REAL Rust ClipboardError::FocusRestoreFailed Display format —
    // mirrors src-tauri/src/plugins/clipboard_paste/mod.rs:96.
    // Chunk 1 reviewer P0 found that original test used a fictional pattern
    // ("Focus restore failed") that never matches actual Rust output
    // ("Failed to restore focus to ..."). This is the regression test.
    const store = useVoiceFlowStore();
    mockInvoke.mockResolvedValueOnce(undefined); // position
    mockInvoke.mockRejectedValueOnce(
      "Failed to restore focus to target HWND 0x3039: GetLastError=0",
    );
    await store.handleStart();
    expect(store.status).toBe("error");
    expect(store.message).toContain("請手動 Ctrl+V");
    // Retro challenger P1: hint must be PREPENDED so it survives the HUD
    // bubble's text-overflow: ellipsis truncation (chunk 2 max-width 280px).
    expect(store.message.startsWith("請手動 Ctrl+V")).toBe(true);
  });

  it("formatError matches legacy 'Focus restore failed' string too (defensive)", async () => {
    // Belt-and-suspenders for any future Rust rename.
    const store = useVoiceFlowStore();
    mockInvoke.mockResolvedValueOnce(undefined); // position
    mockInvoke.mockRejectedValueOnce(
      "Focus restore failed (HWND=12345, GLE=0)",
    );
    await store.handleStart();
    expect(store.status).toBe("error");
    expect(store.message).toContain("請手動 Ctrl+V");
    // Retro challenger P1: hint must be PREPENDED so it survives the HUD
    // bubble's text-overflow: ellipsis truncation (chunk 2 max-width 280px).
    expect(store.message.startsWith("請手動 Ctrl+V")).toBe(true);
  });

  it("formatError matches the Rust enum variant name 'FocusRestoreFailed' too", async () => {
    // Defensive — covers debug-format paths or future Tauri serializer changes.
    const store = useVoiceFlowStore();
    mockInvoke.mockResolvedValueOnce(undefined); // position
    mockInvoke.mockRejectedValueOnce("ClipboardError::FocusRestoreFailed");
    await store.handleStart();
    expect(store.status).toBe("error");
    expect(store.message).toContain("請手動 Ctrl+V");
    // Retro challenger P1: hint must be PREPENDED so it survives the HUD
    // bubble's text-overflow: ellipsis truncation (chunk 2 max-width 280px).
    expect(store.message.startsWith("請手動 Ctrl+V")).toBe(true);
  });

  // ─── M6 chunk 2: polish branch (F21 + F22 + F25 + Decision #5) ──────────

  /**
   * Helper: drive handleStart through to recording with the F21 polish
   * snapshot resolved per the supplied tri-state inputs. Mirrors
   * `mockHandleStartInvokes` but adds the get_settings + has_credential
   * mocks that the M6 chunk 2 polish-enabled-at-start snapshot consumes.
   *
   * `polishEnabled` argument:
   *   * `true`      — Settings.llmPolishEnabled = true (explicit ON)
   *   * `false`     — Settings.llmPolishEnabled = false (explicit OFF)
   *   * `undefined` — Settings.llmPolishEnabled = undefined (auto-detect);
   *                    `hasCredential` controls the resolved value
   *
   * `retryEnabled`: Settings.llmPolishRetryEnabled value used by
   * runPolishWithRetry's get_settings (chunk 2 reads it live during
   * handleStop, not snapshot at handleStart).
   */
  function mockHandleStartWithPolishSnapshot(opts: {
    polishEnabled?: boolean | undefined;
    hasCredential?: boolean;
    provider?: string;
  }): void {
    mockInvoke.mockResolvedValueOnce(undefined); // position_hud_for_active_monitor
    mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
    mockInvoke.mockResolvedValueOnce(undefined); // start_recording
    // get_settings for polish snapshot
    mockInvoke.mockResolvedValueOnce({
      schemaVersion: 1,
      hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
      llmPolishEnabled: opts.polishEnabled,
      llmProvider: opts.provider ?? "groq",
    });
    // has_credential is only invoked on the auto-detect path (undefined).
    if (opts.polishEnabled === undefined) {
      mockInvoke.mockResolvedValueOnce(opts.hasCredential ?? false);
    }
  }

  /** Helper: mock the get_settings + polish_text path that runPolishWithRetry
   * calls. `settings` is the live settings the retry branch reads;
   * `polishOutcomes` is the queue of polish_text resolutions/rejections to
   * consume in order (1 entry = no retry; 2 entries = retry). */
  function mockPolishPipeline(opts: {
    retryEnabled?: boolean;
    polishOutcomes: Array<
      { kind: "ok"; polishedText: string } | { kind: "err"; error: unknown }
    >;
  }): void {
    mockInvoke.mockResolvedValueOnce({
      schemaVersion: 1,
      hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
      llmPolishRetryEnabled: opts.retryEnabled,
    }); // get_settings inside runPolishWithRetry
    for (const outcome of opts.polishOutcomes) {
      if (outcome.kind === "ok") {
        mockInvoke.mockResolvedValueOnce({
          polishedText: outcome.polishedText,
          durationMs: 100,
          inputTokens: null,
          outputTokens: null,
        });
      } else {
        mockInvoke.mockRejectedValueOnce(outcome.error);
      }
    }
  }

  it("M6 chunk 2: polish ON explicit Some(true) + invoke succeeds → enhancing transition fires + polished text pasted", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartWithPolishSnapshot({ polishEnabled: true });
    await store.handleStart();
    expect(store.status).toBe("recording");

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      polishOutcomes: [{ kind: "ok", polishedText: "polished output" }],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    // Capture the enhancing emit — assertion easier than trying to peek
    // the brief enhancing state synchronously (transitionTo is async).
    await store.handleStop();
    expect(store.status).toBe("success");
    expect(store.polishWarning).toBe(false);

    const enhancingEmit = emitToMock.mock.calls.find(
      ([, , payload]) => (payload as { status?: string } | undefined)?.status === "enhancing",
    );
    expect(enhancingEmit).toBeDefined();

    // paste_text was invoked with POLISHED text, not raw.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "polished output",
    });
    const rawPaste = mockInvoke.mock.calls.find(
      ([cmd, args]) =>
        cmd === "paste_text" &&
        (args as { text?: string } | undefined)?.text === "raw transcription",
    );
    expect(rawPaste).toBeUndefined();
  });

  it("M6 chunk 2: polish ON explicit Some(true) but ApiKeyMissing → fallback to raw + polishWarning=true", async () => {
    const store = useVoiceFlowStore();
    // Decision #7 Some(true) attempts polish even without a key — Rust
    // surfaces ApiKeyMissing → fallback. Helper passes hasCredential=false
    // for completeness even though the snapshot path doesn't query it.
    mockHandleStartWithPolishSnapshot({
      polishEnabled: true,
      hasCredential: false,
    });
    await store.handleStart();
    expect(store.status).toBe("recording");

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      polishOutcomes: [
        {
          kind: "err",
          error: "API key missing for provider groq — set it in Settings",
        },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    // F25: warning set on fallback so HUD chunk 3 renders amber bubble.
    expect(store.polishWarning).toBe(true);
    // Pasted RAW (fallback), not polished.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "raw transcription",
    });
    // Enhancing transition still fired (we entered the polish path).
    const enhancingEmit = emitToMock.mock.calls.find(
      ([, , payload]) => (payload as { status?: string } | undefined)?.status === "enhancing",
    );
    expect(enhancingEmit).toBeDefined();
    // ApiKeyMissing is non-retryable → exactly one polish_text invoke.
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(1);
  });

  it("M6 chunk 2: polish None auto-detect + has_credential=false → silent skip (no enhancing, no warning)", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartWithPolishSnapshot({
      polishEnabled: undefined,
      hasCredential: false,
    });
    await store.handleStart();
    expect(store.status).toBe("recording");

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text (no polish path)

    await store.handleStop();
    expect(store.status).toBe("success");
    // No enhancing transition, no warning.
    expect(store.polishWarning).toBe(false);
    const enhancingEmit = emitToMock.mock.calls.find(
      ([, , payload]) => (payload as { status?: string } | undefined)?.status === "enhancing",
    );
    expect(enhancingEmit).toBeUndefined();
    // polish_text never invoked.
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(0);
    // Pasted raw.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "raw transcription",
    });
  });

  it("M6 chunk 2: polish OFF explicit Some(false) → skips polish entirely even with key stored", async () => {
    const store = useVoiceFlowStore();
    // hasCredential=true is irrelevant when user explicitly turned it off.
    // The has_credential helper isn't actually invoked on the explicit OFF
    // path (we short-circuit before auto-detect), so the option is just
    // noise here — included to document intent.
    mockHandleStartWithPolishSnapshot({
      polishEnabled: false,
      hasCredential: true,
    });
    await store.handleStart();
    expect(store.status).toBe("recording");

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    expect(store.polishWarning).toBe(false);
    // polish_text never invoked.
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(0);
    // Pasted raw.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "raw transcription",
    });
    // No has_credential invoke either — the explicit OFF path skips it.
    const hasCredCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "has_credential",
    );
    expect(hasCredCalls).toHaveLength(0);
  });

  it("M6 chunk 2: polish failure with retry ON + first transient error → retries with attempt:2 and second succeeds", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartWithPolishSnapshot({ polishEnabled: true });
    await store.handleStart();

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      retryEnabled: true,
      polishOutcomes: [
        // Connection refused matches the retryable PolishError::ConnectionRefused
        // Display string ("Connection refused by server").
        { kind: "err", error: "Connection refused by server" },
        { kind: "ok", polishedText: "polished after retry" },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    // Success on retry → no warning.
    expect(store.polishWarning).toBe(false);
    // Pasted polished text from second attempt.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "polished after retry",
    });
    // Two polish_text invokes — attempt 1 then attempt 2.
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(2);
    // Verify attempt numbers AND that the outer `args` envelope is present.
    // Optional chaining (?.args?.attempt) would mask a wrapper-shape mismatch
    // — the M6 retro caught a P0 in SettingsLlmPolishSection where flat args
    // were passed because the test happily evaluated `undefined?.attempt`.
    // Strict assertions below: outer `args` envelope must exist, no flat
    // top-level fields allowed.
    const firstAttempt = polishCalls[0]?.[1] as
      | { args?: { attempt?: number; rawText?: string }; rawText?: unknown; attempt?: unknown }
      | undefined;
    const secondAttempt = polishCalls[1]?.[1] as
      | { args?: { attempt?: number; rawText?: string }; rawText?: unknown; attempt?: unknown }
      | undefined;
    expect(firstAttempt).toBeDefined();
    expect(firstAttempt!.args).toBeDefined();
    expect(firstAttempt!.args!.attempt).toBe(1);
    expect(firstAttempt!.args!.rawText).toBeDefined();
    expect(secondAttempt).toBeDefined();
    expect(secondAttempt!.args).toBeDefined();
    expect(secondAttempt!.args!.attempt).toBe(2);
    expect(secondAttempt!.args!.rawText).toBeDefined();
    // Defense against future drift: NO flat fields at top level. If any
    // future change accidentally sends `{ rawText, attempt }` instead of
    // `{ args: { rawText, attempt } }`, these assertions fail loudly
    // (Tauri's deserializer would reject the IPC call at runtime).
    expect(firstAttempt).not.toHaveProperty("rawText");
    expect(firstAttempt).not.toHaveProperty("attempt");
    expect(secondAttempt).not.toHaveProperty("rawText");
    expect(secondAttempt).not.toHaveProperty("attempt");
  });

  it("M6 chunk 2: polish failure with retry ON + non-retryable error → no retry, fallback to raw + warning", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartWithPolishSnapshot({ polishEnabled: true });
    await store.handleStart();

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      retryEnabled: true,
      polishOutcomes: [
        // SafetyBlocked is non-retryable — Display string
        // "Provider blocked the response: SAFETY".
        { kind: "err", error: "Provider blocked the response: SAFETY" },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    expect(store.polishWarning).toBe(true);
    // Exactly one polish_text invoke (no retry).
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(1);
    // Pasted raw.
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "raw transcription",
    });
  });

  it("M6 chunk 2: polish failure with retry OFF → single attempt + fallback even on transient error", async () => {
    const store = useVoiceFlowStore();
    mockHandleStartWithPolishSnapshot({ polishEnabled: true });
    await store.handleStart();

    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "raw transcription",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      retryEnabled: false,
      polishOutcomes: [
        // Timeout is normally retryable, but retry is disabled.
        { kind: "err", error: "Request timed out after 3s" },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text

    await store.handleStop();
    expect(store.status).toBe("success");
    expect(store.polishWarning).toBe(true);
    // Exactly one polish_text invoke even though Timeout is retryable —
    // user disabled retry.
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(1);
    expect(mockInvoke).toHaveBeenCalledWith("paste_text", {
      text: "raw transcription",
    });
  });

  it("M6 chunk 2: polishWarning clears on next recording transition (F25 lifecycle)", async () => {
    const store = useVoiceFlowStore();

    // Drive a polish-failed cycle to set polishWarning = true.
    mockHandleStartWithPolishSnapshot({ polishEnabled: true });
    await store.handleStart();
    mockInvoke.mockResolvedValueOnce(undefined); // stop_recording
    mockInvoke.mockResolvedValueOnce({
      rawText: "first",
      transcriptionDurationMs: 100,
      noSpeechProbability: null,
    }); // transcribe_audio
    mockPolishPipeline({
      polishOutcomes: [
        { kind: "err", error: "API key missing for provider groq" },
      ],
    });
    mockInvoke.mockResolvedValueOnce(undefined); // paste_text
    await store.handleStop();
    expect(store.status).toBe("success");
    expect(store.polishWarning).toBe(true);

    // Press hotkey to start a new recording. transitionTo('recording')
    // must clear polishWarning per F25.
    mockHandleStartWithPolishSnapshot({ polishEnabled: false });
    await store.handleStart();
    expect(store.status).toBe("recording");
    expect(store.polishWarning).toBe(false);
  });

  // ─── M6 chunk 2 (P2-3 fold-in): ESC during enhancing is a no-op (F23) ────

  it("M6 chunk 3 P2-3 fold-in: ESC during enhancing is a no-op + console.warn (F23)", async () => {
    // F23 spec: ESC during `enhancing` does NOT cancel the in-flight LLM
    // HTTP request. Cancelling reqwest mid-flight requires a `cancel_polish`
    // Tauri command or a tokio::select! cancel channel — both deferred to
    // v0.2. Polish's bounded timeouts (3 s Groq / 15 s others) cap the user
    // inconvenience. The handler emits a console.warn breadcrumb so a future
    // dev grepping for the M6 limitation can find the swap site.
    //
    // Test: drive the store to `enhancing` via the DEV-only `__devSetStatus`
    // helper, capture the ESC listener via `init()`, invoke it, and assert:
    //   * status remains 'enhancing' (not flipped to idle by handleCancel)
    //   * console.warn was called with the expected breadcrumb
    //   * stop_recording / clear_recording_buffer were NOT invoked (those
    //     are the recording-cancel cleanup, not relevant here)
    const store = useVoiceFlowStore();

    // Wire listeners so the captured ESC callback is available in
    // listenCallbacks. init issues 4 listen() calls (hotkey pressed/
    // released/toggled + escape/audio-aborted) — we only need ESC.
    const cleanup = await store.init();

    // Drive into enhancing via the DEV mutator (exposed on the store
    // surface; tree-shaken in production).
    store.__devSetStatus("enhancing", "", null);
    expect(store.status).toBe("enhancing");

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {
      /* swallow log noise — we only care that it fires */
    });

    const escCb = listenCallbacks.get("escape:pressed");
    expect(escCb).toBeDefined();
    // ESC listener fires handleCancel; with status='enhancing' it should
    // hit the F23 no-op branch.
    escCb!({ payload: undefined });
    // handleCancel awaits internally; flush the microtask queue so the
    // sync console.warn fires before our assertion.
    await Promise.resolve();
    await Promise.resolve();

    // Status unchanged — F23 no-op branch took over.
    expect(store.status).toBe("enhancing");

    // The breadcrumb mentioning the M6 limitation should have fired.
    expect(warnSpy).toHaveBeenCalled();
    const warnedAboutEnhancing = warnSpy.mock.calls.some((call) =>
      call.some(
        (arg) => typeof arg === "string" && arg.includes("ESC during enhancing"),
      ),
    );
    expect(warnedAboutEnhancing).toBe(true);

    // Recording-cancel cleanup invokes must NOT have fired — those are
    // for the `recording` ESC path, not enhancing.
    const stopRecordingCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "stop_recording",
    );
    const clearBufferCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "clear_recording_buffer",
    );
    expect(stopRecordingCalls).toHaveLength(0);
    expect(clearBufferCalls).toHaveLength(0);

    warnSpy.mockRestore();
    cleanup();
  });
});
