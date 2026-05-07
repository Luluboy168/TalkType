// Unit tests for src/lib/providers.ts (M6 chunk 4 — Decision #3 4 free
// providers active + LLM_MODEL_LIST mirroring Rust registry).
//
// Coverage:
//   1. Exactly 4 active providers (groq / openrouter / nvidia / gemini)
//   2. OpenAI + Anthropic remain inactive (defer v0.2)
//   3. LLM_MODEL_LIST has >= 8 models, each active provider has >= 2
//   4. getModelsByProvider filters correctly
//   5. getDefaultModelId returns the registry's first match per provider
//
// These assertions catch drift if either side of the IPC boundary
// (Rust `LLM_MODEL_LIST` vs TS `LLM_MODEL_LIST`) is updated without the
// other.

import { describe, expect, it } from "vitest";

import {
  LLM_MODEL_LIST,
  LLM_PROVIDERS,
  getDefaultModelId,
  getModelsByProvider,
} from "@/lib/providers";

describe("LLM_PROVIDERS", () => {
  it("ships exactly 4 active providers per Decision #3", () => {
    const active = LLM_PROVIDERS.filter((p) => p.active);
    expect(active).toHaveLength(4);
    const ids = active.map((p) => p.id).sort();
    expect(ids).toEqual(["gemini", "groq", "nvidia", "openrouter"]);
  });

  it("keeps OpenAI and Anthropic inactive (defer v0.2)", () => {
    const inactive = LLM_PROVIDERS.filter((p) => !p.active);
    const ids = inactive.map((p) => p.id).sort();
    expect(ids).toEqual(["anthropic", "openai"]);
  });
});

describe("LLM_MODEL_LIST", () => {
  it("has at least 8 models (4 providers × 2 models)", () => {
    expect(LLM_MODEL_LIST.length).toBeGreaterThanOrEqual(8);
  });

  it("each active provider ships at least 2 models", () => {
    const activeProviders = ["groq", "openrouter", "nvidia", "gemini"] as const;
    for (const provider of activeProviders) {
      const models = LLM_MODEL_LIST.filter((m) => m.provider === provider);
      expect(models.length).toBeGreaterThanOrEqual(2);
    }
  });

  it("getModelsByProvider returns provider-scoped subset", () => {
    const groqModels = getModelsByProvider("groq");
    expect(groqModels.length).toBeGreaterThanOrEqual(2);
    for (const m of groqModels) {
      expect(m.provider).toBe("groq");
    }
  });

  it("getDefaultModelId picks the first matching entry per provider", () => {
    expect(getDefaultModelId("groq")).toBe("llama-3.3-70b-versatile");
    expect(getDefaultModelId("gemini")).toBe("gemini-2.5-flash");
    expect(getDefaultModelId("openrouter")).toBe(
      "meta-llama/llama-3.3-70b-instruct:free",
    );
    expect(getDefaultModelId("nvidia")).toBe("meta/llama-3.3-70b-instruct");
  });
});
