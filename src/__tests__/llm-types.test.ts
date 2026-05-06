// M6 chunk 0 — IPC contract type-surface unit tests.
// Locks the runtime-iterable shapes (LLM_PROMPT_MODES tuple, POLISH_FAILURE_REASONS
// closed enum) and verifies the additive Settings extension keeps M5 → M6
// upgrades forward-compatible (new LLM fields default to undefined).
import { describe, expect, it } from "vitest";

import {
  LLM_PROMPT_MODES,
  POLISH_FAILURE_REASONS,
  type PolishFallbackPayload,
} from "@/types/llm";
import type { Settings } from "@/types/settings";

describe("M6 chunk 0 type surface", () => {
  it("LLM_PROMPT_MODES contains exactly 5 modes in expected order", () => {
    // Decision #2 — unit-only enum + separate `llmCustomPrompt` field. The
    // five-mode order matters for the chunk-4 RadioGroup (UX consistency)
    // and snapshot tests in chunk 1's `prompts.rs`.
    expect(LLM_PROMPT_MODES).toEqual([
      "default",
      "email",
      "chat",
      "code",
      "custom",
    ]);
  });

  it("POLISH_FAILURE_REASONS is closed enum with 12 entries", () => {
    // F2 — closed taxonomy. Adding a new reason requires a deliberate
    // change to both this tuple and the Rust `PolishFailureReason` enum;
    // the length assertion catches accidental drift.
    expect(POLISH_FAILURE_REASONS).toHaveLength(12);
    // Type narrowing: `reason` must accept only members of the const tuple.
    const sample: PolishFallbackPayload = {
      reason: "network",
      providerId: "groq",
    };
    expect(sample.reason).toBe("network");
    expect(sample.providerId).toBe("groq");
  });

  it("Settings accepts new optional LLM fields as undefined (M5 → M6 forward-compat)", () => {
    // M5 settings.json on disk only has `schemaVersion` + `hotkey`. After
    // upgrading to M6 chunk 0 the loaded Settings must still satisfy the
    // type without any LLM fields populated; chunk 2's tri-state default
    // handles the runtime semantics. This locks the additive contract.
    const s: Settings = {
      schemaVersion: 1,
      hotkey: { triggerKey: "right-alt", triggerMode: "hold" },
    };
    expect(s.llmPolishEnabled).toBeUndefined();
    expect(s.llmProvider).toBeUndefined();
    expect(s.llmModelId).toBeUndefined();
    expect(s.llmModelIdOverride).toBeUndefined();
    expect(s.llmPromptMode).toBeUndefined();
    expect(s.llmCustomPrompt).toBeUndefined();
    expect(s.llmPolishRetryEnabled).toBeUndefined();
  });
});
