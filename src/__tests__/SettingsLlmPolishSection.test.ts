// Unit tests for SettingsLlmPolishSection (M6 chunk 4).
//
// Coverage targets:
//   1. Provider switch updates the model dropdown's filtered list (chunk 4)
//   2. Custom prompt char count flips destructive class at 1001 chars (F11/F32)
//   3. Polish toggle OFF disables ONLY the test polish button (F31)
//   4. Data-flow indicator updates reactively when provider changes (F29)
//   5. Data-flow indicator hides ✨ Polish step when polish OFF (F29)
//   6. M5→M6 upgrade banner shows on first render, hides after dismiss + flag (F35)
//   7. No-key warning banner shows when polish=Some(true) + no credential (F22)
//
// Mock surface (see useSettingsStore pattern):
//   * `@tauri-apps/api/core` — invoke; covers has_credential + polish_text +
//     update_settings + get_settings
//   * `@tauri-apps/api/event` — listen / emit / emitTo
//   * useSettingsStore — exercised end-to-end through the pinia + invoke mocks
//
// localStorage is jsdom-native (no mock needed).

import { setActivePinia, createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";

import enMessages from "@/i18n/locales/en.json";
import zhMessages from "@/i18n/locales/zh-TW.json";
import type { Settings } from "@/types/settings";

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
import SettingsLlmPolishSection from "@/components/SettingsLlmPolishSection.vue";

const mockInvoke = vi.mocked(invoke);

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

/**
 * Shape the invoke mock to handle all the calls the component makes:
 *   * `get_settings` returns the supplied snapshot
 *   * `has_credential` returns the supplied boolean
 *   * `update_settings` succeeds with the same snapshot (settings:updated
 *     would normally fan out via listener)
 *   * `polish_text` returns the supplied polish result
 */
function setupInvokeMock(opts: {
  settings: Settings;
  hasCredential: boolean;
  polishText?: { polishedText: string };
}): void {
  mockInvoke.mockImplementation(((command: string, _args?: unknown) => {
    if (command === "get_settings") return Promise.resolve(opts.settings);
    if (command === "has_credential")
      return Promise.resolve(opts.hasCredential);
    if (command === "update_settings") return Promise.resolve(opts.settings);
    if (command === "polish_text") {
      return Promise.resolve(
        opts.polishText ?? { polishedText: "polished output" },
      );
    }
    return Promise.resolve(undefined);
  }) as never);
}

function mountSection() {
  return mount(SettingsLlmPolishSection, {
    global: {
      plugins: [createPinia(), createI18nForTest()],
      stubs: {
        // Stub the entire reka-ui Select tree so SelectItem doesn't try to
        // inject SelectContentContext (which is only set up when the real
        // SelectContent mounts via teleport — incompatible with jsdom +
        // synchronous test flows).
        Select: { template: "<div><slot /></div>" },
        SelectTrigger: { template: "<div><slot /></div>" },
        SelectValue: { template: "<span><slot /></span>" },
        SelectContent: { template: "<div><slot /></div>" },
        SelectItem: {
          template: '<div data-slot="select-item"><slot /></div>',
          props: ["value"],
        },
        // Switch is a button with `data-state` — stub it to a plain
        // button that forwards `update:modelValue` so handler tests work.
        Switch: {
          template:
            '<button :data-testid="$attrs[\'data-testid\']" :disabled="disabled" :data-model-value="modelValue" @click="$emit(\'update:modelValue\', !modelValue)"><slot /></button>',
          props: ["modelValue", "disabled"],
          emits: ["update:modelValue"],
        },
        RadioGroup: { template: "<div><slot /></div>" },
        RadioGroupItem: {
          template: '<input type="radio" :value="value" />',
          props: ["value"],
        },
      },
    },
  });
}

describe("SettingsLlmPolishSection", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
    listenMock.mockClear();
    settingsUpdatedCallbacks.length = 0;
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
    settingsUpdatedCallbacks.length = 0;
  });

  it("renders the data-flow indicator with provider name (F29)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "groq",
      },
      hasCredential: true,
    });
    const wrapper = mountSection();
    await flushPromises();
    const indicator = wrapper.find('[data-testid="dataflow-indicator"]');
    expect(indicator.exists()).toBe(true);
    // Polish step rendered with provider name "Groq"
    expect(indicator.text()).toContain("Groq");
    // dataFlow.audio + paste tokens present
    expect(indicator.text()).toContain(zhMessages.views.settings.llmPolish.dataFlow.audio);
  });

  it("data-flow indicator hides Polish step when polish OFF (F29)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: false,
        llmProvider: "groq",
      },
      hasCredential: true,
    });
    const wrapper = mountSection();
    await flushPromises();
    const indicator = wrapper.find('[data-testid="dataflow-indicator"]');
    expect(indicator.exists()).toBe(true);
    // "(未啟用潤飾)" / "(polish disabled)" present when polish OFF
    expect(indicator.text()).toContain(
      zhMessages.views.settings.llmPolish.dataFlow.withoutPolish,
    );
  });

  it("provider switch updates model filtered list (and indicator)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "groq",
        llmModelId: "llama-3.3-70b-versatile",
      },
      hasCredential: true,
    });
    const wrapper = mountSection();
    await flushPromises();
    // Initially groq → SelectItems list 2 Groq models
    const initialItems = wrapper
      .findAll('[data-slot="select-item"]')
      .map((n) => n.text());
    expect(initialItems).toContain("Llama 3.3 70B (Versatile)");
    expect(initialItems).toContain("Llama 3.1 8B (Instant)");
    // Provider Groq → indicator says Groq
    expect(
      wrapper.find('[data-testid="dataflow-indicator"]').text(),
    ).toContain("Groq");

    // Switch settings to openrouter
    settingsUpdatedCallbacks[0]?.({
      payload: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "openrouter",
        llmModelId: "meta-llama/llama-3.3-70b-instruct:free",
      } satisfies Settings,
    });
    await flushPromises();
    // Indicator now reflects OpenRouter
    expect(
      wrapper.find('[data-testid="dataflow-indicator"]').text(),
    ).toContain("OpenRouter");
    // Model dropdown now lists OpenRouter models
    const updatedItems = wrapper
      .findAll('[data-slot="select-item"]')
      .map((n) => n.text());
    expect(updatedItems).toContain("Llama 3.3 70B (OpenRouter free)");
    expect(updatedItems).toContain("Qwen 2.5 72B (OpenRouter free)");
    expect(updatedItems).not.toContain("Llama 3.1 8B (Instant)");
  });

  it("custom prompt char count flips destructive at 1001 chars (F32)", async () => {
    const longPrompt = "a".repeat(1001);
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "groq",
        llmPromptMode: "custom",
        llmCustomPrompt: longPrompt,
      },
      hasCredential: true,
    });
    const wrapper = mountSection();
    await flushPromises();
    const count = wrapper.find('[data-testid="custom-prompt-count"]');
    expect(count.exists()).toBe(true);
    // 1001 chars → destructive class
    expect(count.classes()).toContain("text-destructive");
    expect(count.text()).toContain("1001");
  });

  it("polish toggle OFF disables ONLY the test polish button (F31)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: false,
        llmProvider: "groq",
      },
      hasCredential: true,
    });
    const wrapper = mountSection();
    await flushPromises();
    // Test button: disabled
    const testButton = wrapper.find('[data-testid="test-polish-button"]');
    expect(testButton.exists()).toBe(true);
    expect(testButton.attributes("disabled")).toBeDefined();
    // Provider select & retry toggle: NOT disabled (i.e. attribute absent)
    const retryToggle = wrapper.find('[data-testid="retry-toggle"]');
    expect(retryToggle.exists()).toBe(true);
    // The Switch component renders a button with data-state attribute, not
    // a disabled attribute, when not explicitly disabled. We assert the
    // disabled attribute is absent on the retry toggle's underlying button.
    expect(retryToggle.attributes("disabled")).toBeUndefined();
  });

  it("test polish button invokes polish_text with wrapped { args: { ... } } envelope (M6 P0 ship-blocker fix)", async () => {
    // M6 retro found a P0: handleTestPolish was passing flat args
    // `{ rawText, vocabulary, attempt }` instead of the wrapped envelope
    // `{ args: { rawText, vocabulary, attempt } }`. The Rust command
    // signature `polish_text(args: PolishTextArgs)` requires the wrapper
    // per Tauri 2 parameter binding; flat args produce a deserialize
    // error like "missing field `args`" at runtime — which would have
    // failed acceptance condition #10 across all 4 free providers.
    //
    // This test exercises the click path of `handleTestPolish` and
    // asserts the IPC arg-shape is wrapped. A flat-args regression
    // would fail this test loudly (positive + negative assertions).
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "groq",
      },
      hasCredential: true,
      polishText: { polishedText: "polished output" },
    });
    const wrapper = mountSection();
    await flushPromises();

    const testButton = wrapper.find('[data-testid="test-polish-button"]');
    expect(testButton.exists()).toBe(true);
    // Sanity: must NOT be disabled in this setup.
    expect(testButton.attributes("disabled")).toBeUndefined();

    await testButton.trigger("click");
    await flushPromises();

    // Find the polish_text invoke call (must have happened exactly once).
    const polishCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "polish_text",
    );
    expect(polishCalls).toHaveLength(1);

    // Assert outer { args: { ... } } envelope shape.
    const polishCall = polishCalls[0]!;
    const callArgs = polishCall[1] as
      | {
          args?: {
            rawText?: string;
            vocabulary?: unknown[];
            attempt?: number;
          };
          rawText?: unknown;
          vocabulary?: unknown;
          attempt?: unknown;
        }
      | undefined;
    expect(callArgs).toBeDefined();
    expect(callArgs!.args).toBeDefined();
    expect(callArgs!.args!.rawText).toEqual(expect.any(String));
    expect(callArgs!.args!.vocabulary).toEqual(expect.any(Array));
    expect(callArgs!.args!.attempt).toBe(1);
    // Critical defense against future drift: NO flat fields at top level.
    // If any future refactor accidentally re-introduces flat args, these
    // assertions fail loudly.
    expect(callArgs).not.toHaveProperty("rawText");
    expect(callArgs).not.toHaveProperty("vocabulary");
    expect(callArgs).not.toHaveProperty("attempt");
  });

  it("M5→M6 upgrade banner shows on first render, hides after dismiss + flag set (F35)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
      },
      hasCredential: false,
    });
    expect(window.localStorage.getItem("talktype:m6_upgrade_seen")).toBeNull();
    const wrapper = mountSection();
    await flushPromises();
    const banner = wrapper.find('[data-testid="m6-upgrade-banner"]');
    expect(banner.exists()).toBe(true);
    // Dismiss
    const dismiss = wrapper.find('[data-testid="m6-upgrade-banner-dismiss"]');
    await dismiss.trigger("click");
    await flushPromises();
    expect(window.localStorage.getItem("talktype:m6_upgrade_seen")).toBe("1");
    expect(
      wrapper.find('[data-testid="m6-upgrade-banner"]').exists(),
    ).toBe(false);
  });

  it("upgrade banner stays hidden when localStorage flag is already set (F35 persistence)", async () => {
    window.localStorage.setItem("talktype:m6_upgrade_seen", "1");
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
      },
      hasCredential: false,
    });
    const wrapper = mountSection();
    await flushPromises();
    expect(
      wrapper.find('[data-testid="m6-upgrade-banner"]').exists(),
    ).toBe(false);
  });

  it("no-key warning banner shows when polish=Some(true) + no credential (F22)", async () => {
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmPolishEnabled: true,
        llmProvider: "groq",
      },
      hasCredential: false,
    });
    const wrapper = mountSection();
    await flushPromises();
    expect(wrapper.find('[data-testid="no-key-banner"]').exists()).toBe(true);
  });

  it("no-key warning banner does NOT show when polish=None (auto-detect) + no credential (F22)", async () => {
    // polish_enabled = undefined (None) → auto-detect, no warning even without key
    setupInvokeMock({
      settings: {
        schemaVersion: 1,
        hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
        llmProvider: "groq",
        // NOTE: llmPolishEnabled intentionally absent (undefined)
      },
      hasCredential: false,
    });
    const wrapper = mountSection();
    await flushPromises();
    expect(wrapper.find('[data-testid="no-key-banner"]').exists()).toBe(false);
  });
});
