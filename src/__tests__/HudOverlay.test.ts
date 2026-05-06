// Unit tests for HudOverlay (M5 chunk 2, M6 chunk 3).
//
// Covers (M5 baseline + M6 chunk 3 additions):
//   * status='idle' → bubble container not rendered
//   * status='recording' → HudWaveform + HudTimer mounted, ariaMessage matches
//   * status='enhancing' → HudSpinner + 「優化中…」 label (M6 chunk 3 NEW)
//   * status='success' + !polishWarning → CheckCircle2 + 「完成」 (regression)
//   * status='success' + polishWarning → AlertTriangle + warning label (M6 NEW)
//   * status='error' → click root → store.dismissError() called once
//   * status transitions → setIgnoreCursorEvents called with the right toggle
//   * ariaMessage covers all 5 visible states (M6 chunk 3 NEW)
//
// Mocks (via vi.hoisted so factories see them in the same scope):
//   * `getCurrentWindow().setIgnoreCursorEvents` — captured to assert calls
//   * `useAudioWaveform` (HudWaveform's dep) — stub start/stop with no-op refs
//   * `useVoiceFlowStore` — stub a reactive shape so watcher fires correctly;
//     M6 chunk 3 adds `polishWarning` ref to the mock surface
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

// We expose four Vue refs through a hoisted module-singleton object so each
// test can flip the status / message / polishWarning reactively and the
// watcher in HudOverlay (and its template re-render) reacts as in production.
// The factory is async so it can `vi.importActual('vue')` to grab `ref` from
// the real Vue runtime.
type MockStatus =
  | "idle"
  | "recording"
  | "transcribing"
  | "enhancing"
  | "success"
  | "error";

const { storeRefHolder, dismissErrorMock } = vi.hoisted(() => ({
  storeRefHolder: {} as {
    status: { value: MockStatus };
    message: { value: string };
    recordingStartedAtMs: { value: number | null };
    polishWarning: { value: boolean };
  },
  dismissErrorMock: vi.fn(),
}));

vi.mock("@/stores/useVoiceFlowStore", async () => {
  const { ref: vueRef } = await vi.importActual<typeof import("vue")>("vue");
  storeRefHolder.status = vueRef<MockStatus>("idle");
  storeRefHolder.message = vueRef<string>("");
  storeRefHolder.recordingStartedAtMs = vueRef<number | null>(null);
  // M6 chunk 3: polishWarning drives the success bubble dual-mode (amber
  // AlertTriangle vs green CheckCircle2). Defaults to false so success
  // tests that don't touch this ref still see the M5 green-checkmark path.
  storeRefHolder.polishWarning = vueRef<boolean>(false);
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
      get polishWarning() {
        return storeRefHolder.polishWarning.value;
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
    // M6 chunk 3: ensure tests start with the green CheckCircle2 path so
    // each test that wants the amber AlertTriangle has to opt in by setting
    // polishWarning.value = true explicitly.
    storeRefHolder.polishWarning.value = false;

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

  // ─── M6 chunk 3: enhancing + success-warning + ariaMessage coverage ───────

  it("renders HudSpinner + 「優化中…」 label when status is enhancing (M6 chunk 3)", async () => {
    storeRefHolder.status.value = "enhancing";
    const wrapper = mountOverlay();
    await flushPromises();
    const root = wrapper.find('[role="status"]');
    expect(root.exists()).toBe(true);
    // F28 ARIA: enhancing announces 「優化中」 not 「轉錄中」.
    expect(root.attributes("aria-label")).toBe("優化中");
    // Label uses the punctuated 「優化中…」 form (matching transcribing's …).
    expect(wrapper.text()).toContain("優化中…");
    // HudSpinner is a <span class="animate-spin ..."/> in non-reduced-motion
    // mode (or a "…" glyph span when reduced motion is on). Default test
    // matchMedia mock returns matches=false, so the animated form mounts.
    expect(wrapper.find("span.animate-spin").exists()).toBe(true);
  });

  it("renders amber AlertTriangle + warning label when status is success and polishWarning is true (M6 chunk 3 Decision #5)", async () => {
    storeRefHolder.status.value = "success";
    storeRefHolder.polishWarning.value = true;
    const wrapper = mountOverlay();
    await flushPromises();
    // The amber AlertTriangle should render (text-amber-500 class) and the
    // green CheckCircle2 (text-green-600) should NOT.
    expect(wrapper.find("svg.text-amber-500").exists()).toBe(true);
    expect(wrapper.find("svg.text-green-600").exists()).toBe(false);
    // The warning label appears in the bubble text.
    expect(wrapper.text()).toContain("優化失敗、已貼上原始轉錄");
    // ariaMessage swaps to the warning form so SR matches the visual.
    const root = wrapper.find('[role="status"]');
    expect(root.attributes("aria-label")).toBe("優化失敗、已貼上原始轉錄");
  });

  it("renders green CheckCircle2 + 「完成」 when status is success and polishWarning is false (M5 regression)", async () => {
    // Regression check: the M5 green-checkmark path must still work after
    // chunk 3 introduces the amber dual-mode branch.
    storeRefHolder.status.value = "success";
    storeRefHolder.polishWarning.value = false;
    const wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find("svg.text-green-600").exists()).toBe(true);
    expect(wrapper.find("svg.text-amber-500").exists()).toBe(false);
    expect(wrapper.text()).toContain("完成");
    expect(wrapper.text()).not.toContain("優化失敗");
  });

  it("ariaMessage covers all 5 visible voice-flow states (M6 chunk 3 5-state matrix)", async () => {
    // recording — uses the noun ARIA so a SR doesn't read 「錄音中…」 with
    // the punctuation that's in the visible label.
    storeRefHolder.status.value = "recording";
    storeRefHolder.recordingStartedAtMs.value = Date.now();
    let wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[role="status"]').attributes("aria-label")).toBe(
      "錄音中",
    );
    wrapper.unmount();

    // transcribing
    storeRefHolder.status.value = "transcribing";
    wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[role="status"]').attributes("aria-label")).toBe(
      "轉錄中",
    );
    wrapper.unmount();

    // enhancing — F28 wording differentiation
    storeRefHolder.status.value = "enhancing";
    wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[role="status"]').attributes("aria-label")).toBe(
      "優化中",
    );
    wrapper.unmount();

    // success (no warning) — green-path message
    storeRefHolder.status.value = "success";
    storeRefHolder.polishWarning.value = false;
    wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[role="status"]').attributes("aria-label")).toBe(
      "完成",
    );
    wrapper.unmount();

    // error — message templated into the ARIA label
    storeRefHolder.status.value = "error";
    storeRefHolder.message.value = "test failure";
    wrapper = mountOverlay();
    await flushPromises();
    expect(wrapper.find('[role="status"]').attributes("aria-label")).toBe(
      "錯誤：test failure",
    );
    wrapper.unmount();
  });
});
