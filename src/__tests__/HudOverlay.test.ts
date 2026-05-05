// Unit tests for HudOverlay (M5 chunk 2).
//
// Covers:
//   * status='idle' → bubble container not rendered
//   * status='recording' → HudWaveform + HudTimer mounted, ariaMessage matches
//   * status='success' → CheckCircle2 + label rendered
//   * status='error' → click root → store.dismissError() called once
//   * status transitions → setIgnoreCursorEvents called with the right toggle
//
// Mocks (via vi.hoisted so factories see them in the same scope):
//   * `getCurrentWindow().setIgnoreCursorEvents` — captured to assert calls
//   * `useAudioWaveform` (HudWaveform's dep) — stub start/stop with no-op refs
//   * `useVoiceFlowStore` — stub a reactive shape so watcher fires correctly
//   * `window.matchMedia` — defaults to reducedMotion=false
import { setActivePinia, createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";

import enMessages from "@/i18n/locales/en.json";
import zhMessages from "@/i18n/locales/zh-TW.json";

const { setIgnoreCursorEventsMock } = vi.hoisted(() => ({
  setIgnoreCursorEventsMock: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setIgnoreCursorEvents: setIgnoreCursorEventsMock,
  }),
}));

const { startMock, stopMock } = vi.hoisted(() => ({
  startMock: vi.fn().mockResolvedValue(undefined),
  stopMock: vi.fn(),
}));

vi.mock("@/composables/useAudioWaveform", async () => {
  const { ref: vueRef } = await vi.importActual<typeof import("vue")>("vue");
  return {
    useAudioWaveform: () => ({
      smoothedLevels: vueRef([0, 0, 0, 0, 0, 0]),
      start: startMock,
      stop: stopMock,
    }),
  };
});

// We expose three Vue refs through a hoisted module-singleton object so each
// test can flip the status / message reactively and the watcher in HudOverlay
// (and its template re-render) reacts as in production. The factory is async
// so it can `vi.importActual('vue')` to grab `ref` from the real Vue runtime.
type MockStatus =
  | "idle"
  | "recording"
  | "transcribing"
  | "success"
  | "error";

const { storeRefHolder, dismissErrorMock } = vi.hoisted(() => ({
  storeRefHolder: {} as {
    status: { value: MockStatus };
    message: { value: string };
    recordingStartedAtMs: { value: number | null };
  },
  dismissErrorMock: vi.fn(),
}));

vi.mock("@/stores/useVoiceFlowStore", async () => {
  const { ref: vueRef } = await vi.importActual<typeof import("vue")>("vue");
  storeRefHolder.status = vueRef<MockStatus>("idle");
  storeRefHolder.message = vueRef<string>("");
  storeRefHolder.recordingStartedAtMs = vueRef<number | null>(null);
  return {
    useVoiceFlowStore: () => ({
      get status() {
        return storeRefHolder.status.value;
      },
      get message() {
        return storeRefHolder.message.value;
      },
      get recordingStartedAtMs() {
        return storeRefHolder.recordingStartedAtMs.value;
      },
      dismissError: dismissErrorMock,
    }),
  };
});

import HudOverlay from "@/components/HudOverlay.vue";

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

function mountOverlay() {
  return mount(HudOverlay, {
    global: {
      plugins: [createI18nForTest()],
    },
  });
}

describe("HudOverlay", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    setIgnoreCursorEventsMock.mockClear();
    dismissErrorMock.mockClear();
    // Reset reactive store state via the refs (so reactivity fires).
    storeRefHolder.status.value = "idle";
    storeRefHolder.message.value = "";
    storeRefHolder.recordingStartedAtMs.value = null;

    Object.defineProperty(window, "matchMedia", {
      writable: true,
      configurable: true,
      value: vi.fn().mockImplementation(() => ({
        matches: false,
        media: "(prefers-reduced-motion: reduce)",
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("does not render the bubble when status is idle", () => {
    storeRefHolder.status.value = "idle";
    const wrapper = mountOverlay();
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
  });

  it("renders waveform + timer when status is recording", async () => {
    storeRefHolder.status.value = "recording";
    storeRefHolder.recordingStartedAtMs.value = Date.now();
    const wrapper = mountOverlay();
    await flushPromises();
    const root = wrapper.find('[role="status"]');
    expect(root.exists()).toBe(true);
    // ARIA label uses the recording message.
    expect(root.attributes("aria-label")).toBe("錄音中");
    // Waveform component renders 6 bars by default.
    expect(wrapper.findAll("span.w-1")).toHaveLength(6);
    // Timer renders mm:ss text — at least one font-mono span.
    expect(wrapper.find("span.font-mono").exists()).toBe(true);
  });

  it("renders success icon + label when status is success", async () => {
    storeRefHolder.status.value = "success";
    const wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[aria-label="完成"]').exists()).toBe(true);
    // CheckCircle2 from lucide-vue-next renders an SVG with the green class.
    expect(wrapper.find("svg.text-green-600").exists()).toBe(true);
    expect(wrapper.text()).toContain("完成");
  });

  it("renders error icon + message when status is error", async () => {
    storeRefHolder.status.value = "error";
    storeRefHolder.message.value = "API key 無效";
    const wrapper = mountOverlay();
    await flushPromises();
    const root = wrapper.find('[role="status"]');
    expect(root.attributes("aria-label")).toBe("錯誤：API key 無效");
    expect(wrapper.find("svg.text-red-600").exists()).toBe(true);
    expect(wrapper.text()).toContain("API key 無效");
  });

  it("calls dismissError once when error bubble is clicked", async () => {
    storeRefHolder.status.value = "error";
    storeRefHolder.message.value = "fail";
    const wrapper = mountOverlay();
    await flushPromises();
    await wrapper.find('[role="status"]').trigger("click");
    expect(dismissErrorMock).toHaveBeenCalledTimes(1);
  });

  it("does NOT call dismissError when clicking outside error state", async () => {
    storeRefHolder.status.value = "recording";
    storeRefHolder.recordingStartedAtMs.value = Date.now();
    const wrapper = mountOverlay();
    await flushPromises();
    await wrapper.find('[role="status"]').trigger("click");
    expect(dismissErrorMock).not.toHaveBeenCalled();
  });

  it("toggles setIgnoreCursorEvents based on status (idle → true, error → false)", async () => {
    // Initial mount with idle — onMounted calls setIgnoreCursorEvents(true).
    storeRefHolder.status.value = "idle";
    const wrapper = mountOverlay();
    await flushPromises();
    expect(setIgnoreCursorEventsMock).toHaveBeenCalledWith(true);

    // Move to recording — watcher fires with status='recording' → through.
    setIgnoreCursorEventsMock.mockClear();
    storeRefHolder.status.value = "recording";
    storeRefHolder.recordingStartedAtMs.value = Date.now();
    await flushPromises();
    expect(setIgnoreCursorEventsMock).toHaveBeenCalledWith(true);

    // Move to error — should call with false (so user can click).
    setIgnoreCursorEventsMock.mockClear();
    storeRefHolder.status.value = "error";
    storeRefHolder.message.value = "x";
    await flushPromises();
    expect(setIgnoreCursorEventsMock).toHaveBeenCalledWith(false);

    wrapper.unmount();
  });
});
