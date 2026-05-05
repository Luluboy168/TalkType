// Unit tests for HudWaveform (M5 chunk 2).
//
// Mocks `useAudioWaveform` to return a stable smoothedLevels ref so we can
// assert the rendered DOM without dealing with the listenToEvent / RAF
// machinery. We just need to confirm the prop-driven render switching:
// reducedMotion=false → 6 bars; reducedMotion=true → 1 dot.
import { mount } from "@vue/test-utils";
import { ref } from "vue";
import { describe, expect, it, vi } from "vitest";

const { startMock, stopMock, smoothedLevelsRef } = vi.hoisted(() => ({
  startMock: vi.fn().mockResolvedValue(undefined),
  stopMock: vi.fn(),
  smoothedLevelsRef: { value: [0, 0, 0, 0, 0, 0] as number[] },
}));

vi.mock("@/composables/useAudioWaveform", () => ({
  useAudioWaveform: () => ({
    smoothedLevels: ref(smoothedLevelsRef.value),
    start: startMock,
    stop: stopMock,
  }),
}));

import HudWaveform from "@/components/HudWaveform.vue";

describe("HudWaveform", () => {
  it("renders 6 bar elements when reducedMotion is false", () => {
    const wrapper = mount(HudWaveform, { props: { reducedMotion: false } });
    const bars = wrapper.findAll("span.w-1");
    expect(bars).toHaveLength(6);
    // Sanity: the dot fallback is NOT rendered.
    expect(wrapper.findAll("span.size-2")).toHaveLength(0);
  });

  it("renders single dot when reducedMotion is true", () => {
    const wrapper = mount(HudWaveform, { props: { reducedMotion: true } });
    const dot = wrapper.find("span.size-2");
    expect(dot.exists()).toBe(true);
    // Bars NOT rendered.
    expect(wrapper.findAll("span.w-1")).toHaveLength(0);
  });

  it("calls start() on mount and stop() on unmount (animation lifecycle)", () => {
    startMock.mockClear();
    stopMock.mockClear();
    const wrapper = mount(HudWaveform, { props: { reducedMotion: false } });
    expect(startMock).toHaveBeenCalledTimes(1);
    wrapper.unmount();
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it("does NOT call start() when mounted with reducedMotion=true (P2-1)", () => {
    // Energy-conscious: spec §5.2 says reduced motion should not run RAF.
    startMock.mockClear();
    stopMock.mockClear();
    const wrapper = mount(HudWaveform, { props: { reducedMotion: true } });
    expect(startMock).not.toHaveBeenCalled();
    wrapper.unmount();
    // stop() still runs at unmount (idempotent — useAudioWaveform.stop guards)
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it("toggles between start() and stop() when reducedMotion prop changes", async () => {
    startMock.mockClear();
    stopMock.mockClear();
    const wrapper = mount(HudWaveform, { props: { reducedMotion: false } });
    expect(startMock).toHaveBeenCalledTimes(1);
    await wrapper.setProps({ reducedMotion: true });
    expect(stopMock).toHaveBeenCalled();
    startMock.mockClear();
    await wrapper.setProps({ reducedMotion: false });
    expect(startMock).toHaveBeenCalledTimes(1);
  });
});
