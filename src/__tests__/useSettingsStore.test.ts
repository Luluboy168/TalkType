// Unit tests for the settings Pinia store (M4 chunk 4).
//
// Covers: initial state, `load()` happy + error paths, `update()` invokes
// Rust + lets the `settings:updated` listener refresh the cached snapshot,
// `subscribe()` is idempotent.
import { setActivePinia, createPinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock Tauri's invoke + event listener BEFORE importing the store — the
// store imports both at module load time, so the mock must be registered
// first. The `listen` mock collects callbacks into a side-channel so a test
// can fire a fake `settings:updated` event manually and inspect what the
// store does with it. We use `vi.hoisted` so the side-channel array +
// `listen` mock are defined in the same hoisted block as the `vi.mock`
// factory (Vitest hoists `vi.mock` to the top of the file; without
// `vi.hoisted`, in-scope variables aren't visible to the factory).
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const { settingsUpdatedCallbacks, listenMock } = vi.hoisted(() => {
  const settingsUpdatedCallbacks: Array<
    (event: { payload: unknown }) => void
  > = [];
  const listenMock = vi.fn(
    async (
      _eventName: string,
      callback: (event: { payload: unknown }) => void,
    ) => {
      settingsUpdatedCallbacks.push(callback);
      return () => {
        const idx = settingsUpdatedCallbacks.indexOf(callback);
        if (idx >= 0) settingsUpdatedCallbacks.splice(idx, 1);
      };
    },
  );
  return { settingsUpdatedCallbacks, listenMock };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
  emit: vi.fn().mockResolvedValue(undefined),
  emitTo: vi.fn().mockResolvedValue(undefined),
}));

import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "@/stores/useSettingsStore";
import type { Settings } from "@/types/settings";

const mockInvoke = vi.mocked(invoke);

const SAMPLE_SETTINGS: Settings = {
  schemaVersion: 1,
  hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
};

describe("useSettingsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
    listenMock.mockClear();
    settingsUpdatedCallbacks.length = 0;
  });

  afterEach(() => {
    settingsUpdatedCallbacks.length = 0;
  });

  it("starts with null settings, loading=true, no error", () => {
    const store = useSettingsStore();
    expect(store.settings).toBeNull();
    expect(store.loading).toBe(true);
    expect(store.error).toBeNull();
  });

  it("load() populates settings from invoke and clears loading flag", async () => {
    mockInvoke.mockResolvedValueOnce(SAMPLE_SETTINGS);
    const store = useSettingsStore();
    await store.load();
    expect(mockInvoke).toHaveBeenCalledWith("get_settings");
    expect(store.settings).toEqual(SAMPLE_SETTINGS);
    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
  });

  it("load() surfaces invoke error and resets settings to null", async () => {
    mockInvoke.mockRejectedValueOnce("Settings store unavailable: disk full");
    const store = useSettingsStore();
    await store.load();
    expect(store.settings).toBeNull();
    expect(store.loading).toBe(false);
    expect(store.error).toBe("Settings store unavailable: disk full");
  });

  it("update() invokes update_settings with the patch and re-throws on failure", async () => {
    mockInvoke.mockResolvedValueOnce(SAMPLE_SETTINGS);
    const store = useSettingsStore();
    const patch = {
      hotkey: { triggerKey: "left-control" as const, triggerMode: "toggle" as const },
    };
    await store.update(patch);
    expect(mockInvoke).toHaveBeenCalledWith("update_settings", { patch });
    expect(store.error).toBeNull();

    mockInvoke.mockRejectedValueOnce("Settings state lock poisoned");
    await expect(store.update(patch)).rejects.toBeDefined();
    expect(store.error).toBe("Settings state lock poisoned");
  });

  it("subscribe() registers a settings:updated listener and the callback refreshes cached snapshot", async () => {
    const store = useSettingsStore();
    await store.subscribe();
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock.mock.calls[0]?.[0]).toBe("settings:updated");

    // Simulate Rust emitting the event with the new merged Settings.
    const updated: Settings = {
      schemaVersion: 1,
      hotkey: { triggerKey: "right-shift", triggerMode: "toggle" },
    };
    expect(settingsUpdatedCallbacks.length).toBe(1);
    settingsUpdatedCallbacks[0]?.({ payload: updated });
    expect(store.settings).toEqual(updated);
  });

  it("subscribe() is idempotent — calling twice does not stack listeners", async () => {
    const store = useSettingsStore();
    await store.subscribe();
    await store.subscribe();
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(settingsUpdatedCallbacks.length).toBe(1);
  });

  it("unsubscribe() detaches the listener so settings:updated no longer mutates state", async () => {
    const store = useSettingsStore();
    await store.subscribe();
    store.unsubscribe();
    expect(settingsUpdatedCallbacks.length).toBe(0);
    // Re-subscribing after unsubscribe should re-register a fresh listener.
    await store.subscribe();
    expect(settingsUpdatedCallbacks.length).toBe(1);
  });
});
