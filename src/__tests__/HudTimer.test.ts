// Unit tests for HudTimer (M5 chunk 2).
//
// Covers:
//   * mm:ss formatting at 0 / 59 / 60 / 599 / 720 elapsed seconds
//   * colorClass thresholds (< 540 → muted, ≥ 540 → yellow, ≥ 720 → red)
//   * P1-5 regression: startedAtMs=null renders "0:00" with no errors
//   * setInterval reactivity (fake timers advance 1s → reactive update)
import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import HudTimer from "@/components/HudTimer.vue";

describe("HudTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders 0:00 when elapsed is 0", () => {
    // startedAtMs = now → elapsed = 0
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const wrapper = mount(HudTimer, { props: { startedAtMs: Date.now() } });
    expect(wrapper.text()).toBe("0:00");
  });

  it("renders 0:59 at 59 seconds elapsed", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 59_000;
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.text()).toBe("0:59");
  });

  it("renders 1:00 at 60 seconds elapsed", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 60_000;
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.text()).toBe("1:00");
  });

  it("renders 9:59 at 599 seconds elapsed (just under yellow threshold)", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 599_000;
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.text()).toBe("9:59");
  });

  it("renders 12:00 at 720 seconds elapsed (red threshold)", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 720_000;
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.text()).toBe("12:00");
  });

  it("color is muted-foreground for elapsed < 540s", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 100_000; // 1:40
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.classes()).toContain("text-muted-foreground");
    expect(wrapper.classes()).not.toContain("text-yellow-600");
    expect(wrapper.classes()).not.toContain("text-red-600");
  });

  it("color is yellow-600 for elapsed in [540, 720) seconds", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 600_000; // 10:00
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.classes()).toContain("text-yellow-600");
    expect(wrapper.classes()).not.toContain("text-red-600");
  });

  it("color is red-600 for elapsed ≥ 720 seconds", () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now() - 720_000; // 12:00
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.classes()).toContain("text-red-600");
    expect(wrapper.classes()).not.toContain("text-yellow-600");
  });

  // P1-5 regression — chunk 2 fold-in
  it("renders 0:00 when startedAtMs is null without errors", () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const wrapper = mount(HudTimer, { props: { startedAtMs: null } });
    expect(wrapper.text()).toBe("0:00");
    expect(wrapper.classes()).toContain("text-muted-foreground");

    expect(errSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled();

    errSpy.mockRestore();
    warnSpy.mockRestore();
  });

  it("reactively updates when fake timer advances 1s", async () => {
    vi.setSystemTime(new Date("2026-05-05T00:00:00Z"));
    const startedAtMs = Date.now();
    const wrapper = mount(HudTimer, { props: { startedAtMs } });
    expect(wrapper.text()).toBe("0:00");

    // Advance fake timer 1s — setInterval fires once, now ref updates,
    // computed elapsed re-evaluates to 1.
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.text()).toBe("0:01");

    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.text()).toBe("0:02");
  });
});
