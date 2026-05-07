// Unit tests for ProviderPrivacyDialog (M3 chunk 1 + M6 chunk 4).
//
// Coverage target for chunk 4: each of the 4 active polish providers
// (groq / openrouter / nvidia / gemini) ships a distinct provider-specific
// body string in zh-TW + en. `openai` / `anthropic` fall through to the
// generic copy until v0.2 (asserted to share that fallback).
//
// We test the i18n surface directly because the reka-ui Dialog renders
// via teleport / portal which jsdom + Vue Test Utils don't always expose
// in the wrapper's text. Asserting body keys is the source-of-truth check
// here — the component itself is just a `te(specificKey) ? ...` lookup.

import { describe, expect, it } from "vitest";

import enMessages from "@/i18n/locales/en.json";
import zhMessages from "@/i18n/locales/zh-TW.json";

const ACTIVE_POLISH_PROVIDERS = ["groq", "openrouter", "nvidia", "gemini"] as const;
const INACTIVE_POLISH_PROVIDERS = ["openai", "anthropic"] as const;

function getBody(
  locale: typeof zhMessages | typeof enMessages,
  providerId: string,
): string | undefined {
  const bodies = locale.views.settings.apiKey.privacyDialog.body as Record<
    string,
    string
  >;
  return bodies[providerId];
}

describe("ProviderPrivacyDialog provider-specific body keys", () => {
  for (const id of ACTIVE_POLISH_PROVIDERS) {
    it(`zh-TW ships dedicated body for ${id}`, () => {
      const body = getBody(zhMessages, id);
      expect(body).toBeDefined();
      expect(body!.length).toBeGreaterThan(40);
    });
    it(`en ships dedicated body for ${id}`, () => {
      const body = getBody(enMessages, id);
      expect(body).toBeDefined();
      expect(body!.length).toBeGreaterThan(40);
    });
  }

  for (const id of INACTIVE_POLISH_PROVIDERS) {
    it(`zh-TW does NOT ship a dedicated body for ${id} (falls back to generic in v0.2)`, () => {
      // The component's `te(specificKey)` short-circuits to generic when
      // missing; chunk 4 only adds bodies for the 4 active providers.
      expect(getBody(zhMessages, id)).toBeUndefined();
      expect(getBody(enMessages, id)).toBeUndefined();
    });
  }

  it("4 active polish providers each ship a distinct body in zh-TW", () => {
    const bodies = new Set<string>();
    for (const id of ACTIVE_POLISH_PROVIDERS) {
      const body = getBody(zhMessages, id);
      expect(body).toBeDefined();
      bodies.add(body!);
    }
    expect(bodies.size).toBe(ACTIVE_POLISH_PROVIDERS.length);
  });

  it("4 active polish providers each ship a distinct body in en", () => {
    const bodies = new Set<string>();
    for (const id of ACTIVE_POLISH_PROVIDERS) {
      const body = getBody(enMessages, id);
      expect(body).toBeDefined();
      bodies.add(body!);
    }
    expect(bodies.size).toBe(ACTIVE_POLISH_PROVIDERS.length);
  });
});
