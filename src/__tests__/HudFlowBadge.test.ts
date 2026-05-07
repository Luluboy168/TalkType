// Unit tests for HudFlowBadge (M5 chunk 3).
//
// Mocks `@/composables/useTauriEvents` so the component's `listenToEvent`
// call hands us back a captured callback we can drive synchronously, the
// same pattern useVoiceFlowStore.test.ts uses elsewhere. We verify:
//   1. Initial render is empty (status='idle' default → v-if false)
//   2. payload {status:'recording', source:'hud'} → red dot + 「錄音中」 visible
//   3. payload {status:'transcribing'} → badge hides again
//   4. P0-1 echo-loop defense: source !== 'hud' is ignored
//   5. unmount calls the cleanup function returned by listenToEvent
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";

import enMessages from "@/i18n/locales/en.json";
import zhMessages from "@/i18n/locales/zh-TW.json";

// Hoisted shared state — vi.mock factories run before top-level statements
// so we cannot just close over local refs without `vi.hoisted`.
const { listenMock, unlistenMock, callbacks } = vi.hoisted(() => {
  const cbs = new Map<string, (event: { payload: unknown }) => void>();
  const unlisten = vi.fn();
  return {
    listenMock: vi.fn(),
    unlistenMock: unlisten,
    callbacks: cbs,
  };
});

vi.mock("@/composables/useTauriEvents", async () => {
  const actual = await vi.importActual<
    typeof import("@/composables/useTauriEvents")
  >("@/composables/useTauriEvents");
  return {
    ...actual,
    listenToEvent: listenMock,
  };
});

import HudFlowBadge from "@/components/HudFlowBadge.vue";

function createI18nForTest() {
  return createI18n({
    legacy: false,
    locale: "zh-TW",
    fallbackLocale: "en",
    messages: {
      "zh-TW": zhMessages,
      en: enMessages,
    },
  });
}

function mountBadge() {
  return mount(HudFlowBadge, {
    global: {
      plugins: [createI18nForTest()],
    },
  });
}

describe("HudFlowBadge", () => {
  beforeEach(() => {
    callbacks.clear();
    unlistenMock.mockReset();
    listenMock.mockReset();
    listenMock.mockImplementation(
      (
        name: string,
        cb: (event: { payload: unknown }) => void,
      ): Promise<() => void> => {
        callbacks.set(name, cb);
        return Promise.resolve(() => {
          callbacks.delete(name);
          unlistenMock();
        });
      },
    );
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when status starts at idle (no event yet)", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
    expect(wrapper.text()).toBe("");
  });

  it("renders red dot + 「錄音中」 when receiving status='recording' from hud", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    const cb = callbacks.get("voice-flow:state-changed");
    expect(cb).toBeDefined();
    cb?.({
      payload: { status: "recording", message: "", source: "hud" },
    });
    await flushPromises();
    const root = wrapper.find('[role="status"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("aria-live")).toBe("polite");
    // Red pulsing dot rendered.
    expect(wrapper.find("span.bg-red-500").exists()).toBe(true);
    expect(wrapper.find("span.animate-pulse").exists()).toBe(true);
    expect(wrapper.text()).toContain("錄音中");
  });

  it("hides badge when status transitions to transcribing", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    const cb = callbacks.get("voice-flow:state-changed");
    expect(cb).toBeDefined();
    // First go to recording so the badge renders…
    cb?.({
      payload: { status: "recording", message: "", source: "hud" },
    });
    await flushPromises();
    expect(wrapper.find('[role="status"]').exists()).toBe(true);
    // …then transition to transcribing — badge should be removed.
    cb?.({
      payload: { status: "transcribing", message: "轉錄中…", source: "hud" },
    });
    await flushPromises();
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
  });

  it("ignores events with source !== 'hud' (P0-1 echo-loop defense)", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    const cb = callbacks.get("voice-flow:state-changed");
    expect(cb).toBeDefined();
    // A spoofed/echoed event from "dashboard" must NOT flip status.
    cb?.({
      payload: { status: "recording", message: "", source: "dashboard" },
    });
    await flushPromises();
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
  });

  it("calls unlisten on unmount so no leaked listener stays alive", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    expect(callbacks.get("voice-flow:state-changed")).toBeDefined();
    wrapper.unmount();
    await flushPromises();
    expect(unlistenMock).toHaveBeenCalledTimes(1);
    expect(callbacks.get("voice-flow:state-changed")).toBeUndefined();
  });

  // ─── M6 chunk 3 (F26): enhancing badge — amber dot + 「優化中」 ─────────────

  it("renders amber dot + 「優化中」 when receiving status='enhancing' from hud (F26)", async () => {
    const wrapper = mountBadge();
    await flushPromises();
    const cb = callbacks.get("voice-flow:state-changed");
    expect(cb).toBeDefined();
    cb?.({
      payload: { status: "enhancing", message: "", source: "hud" },
    });
    await flushPromises();
    const root = wrapper.find('[role="status"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("aria-live")).toBe("polite");
    // Amber pulsing dot rendered (NOT red — visual differentiation from
    // recording is the whole point of F26).
    expect(wrapper.find("span.bg-amber-500").exists()).toBe(true);
    expect(wrapper.find("span.bg-red-500").exists()).toBe(false);
    expect(wrapper.find("span.animate-pulse").exists()).toBe(true);
    expect(wrapper.text()).toContain("優化中");
    // Regression: the recording label string must NOT appear when in
    // enhancing state.
    expect(wrapper.text()).not.toContain("錄音中");
  });

  it("renders nothing for non-recording-non-enhancing states (F26 hidden states regression)", async () => {
    // F26 spec: idle / transcribing / success / error all render nothing.
    // Sweep through them via the listener callback and confirm the badge
    // root is absent for each.
    const wrapper = mountBadge();
    await flushPromises();
    const cb = callbacks.get("voice-flow:state-changed");
    expect(cb).toBeDefined();
    const hiddenStates = ["idle", "transcribing", "success", "error"] as const;
    for (const status of hiddenStates) {
      cb?.({
        payload: { status, message: "", source: "hud" },
      });
      await flushPromises();
      expect(wrapper.find('[role="status"]').exists()).toBe(false);
    }
  });
});
