// Unit tests for the voice flow Pinia store (M4 chunk 3).
//
// Covers the state machine transitions through `handleStart` / `handleStop` /
// `handleCancel` with `invoke` mocked. The Tauri event listener wiring
// (`init`) is left for M4 chunk 4 manual smoke + M5 HUD integration tests
// because vi.mock on `@tauri-apps/api/event` would mostly assert "the call
// happened" rather than catching real bugs.
import { setActivePinia, createPinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock Tauri's invoke BEFORE importing the store — the store imports invoke
// at module load time, so the mock must be registered first.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Also mock `@tauri-apps/api/event` because `useTauriEvents` re-exports
// `listen` from there, and `init()` would otherwise try to attach real OS
// event listeners during module import.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
  emitTo: vi.fn().mockResolvedValue(undefined),
}));

import { invoke } from "@tauri-apps/api/core";
import { useVoiceFlowStore } from "@/stores/useVoiceFlowStore";

const mockInvoke = vi.mocked(invoke);

describe("useVoiceFlowStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
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

  it("handleStart from idle invokes capture_target_window + start_recording and transitions to recording", async () => {
    mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
    mockInvoke.mockResolvedValueOnce(undefined); // start_recording

    const store = useVoiceFlowStore();
    await store.handleStart();

    expect(mockInvoke).toHaveBeenNthCalledWith(1, "capture_target_window");
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "start_recording", {
      deviceName: null,
    });
    expect(store.status).toBe("recording");
    expect(store.recordingStartedAtMs).toBeTypeOf("number");
  });

  it("handleStart is a no-op when not in idle state", async () => {
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

  it("handleStop from recording transitions through transcribing → success → idle and pastes raw text", async () => {
    const store = useVoiceFlowStore();

    // Set up handleStart first so we're in recording state.
    mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
    mockInvoke.mockResolvedValueOnce(undefined); // start_recording
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

    // Advance the auto-revert timer (1000 ms) → idle.
    vi.advanceTimersByTime(1000);
    expect(store.status).toBe("idle");
  });

  it("handleCancel from recording returns to idle without invoking paste_text", async () => {
    const store = useVoiceFlowStore();

    // Enter recording state.
    mockInvoke.mockResolvedValueOnce(undefined); // capture_target_window
    mockInvoke.mockResolvedValueOnce(undefined); // start_recording
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
    mockInvoke.mockRejectedValueOnce("API key missing for provider groq");
    await store.handleStart();
    expect(store.status).toBe("error");
    expect(store.message).toContain("API key missing");

    vi.advanceTimersByTime(3000);
    expect(store.status).toBe("idle");
  });
});
