// ARIA wording differentiation test (F28, M6 chunk 3).
//
// Background: when a user transitions from `transcribing` to `enhancing`,
// the screen reader should announce both transitions. ARIA-live regions only
// re-announce when the message text actually changes — if both states
// resolved to the same string ("Loading…" or similar) the SR would skip the
// second. F28 mandates the two strings differ by more than punctuation /
// whitespace so the SR experience is clear.
//
// **Naive comparison trap (chunk 2 reviewer P2-6)**: the original kickoff
// log spec sketched `t1.replace(/\W/g, '') !== t2.replace(/\W/g, '')` to
// strip "non-word" characters before comparison. JavaScript's `\W` regex
// without the `u` flag treats EVERY CJK character as non-word, so zh-TW
// 「轉錄中」 and 「優化中」 both strip to the empty string and the
// assertion passes trivially without actually verifying differentiation.
//
// **Fix**: use Unicode property classes `\p{L}` (any letter, Unicode-aware,
// includes CJK) and `\p{N}` (any digit) with the `u` flag so CJK characters
// survive the strip. This catches the bug we'd miss with `\W`.
import { describe, expect, it } from "vitest";
import { createI18n } from "vue-i18n";

import enMessages from "@/i18n/locales/en.json";
import zhMessages from "@/i18n/locales/zh-TW.json";

/**
 * Strip everything except Unicode letters and numbers, leaving the
 * "alphabetic body" of a message. CJK characters count as letters under
 * `\p{L}` (verified via test in this file). With this definition, two
 * messages compare equal only if they're identical aside from punctuation
 * / whitespace differences — which is exactly the property F28 forbids
 * for the transcribing/enhancing pair.
 */
function alphanumericChars(s: string): string {
  return s.replace(/[^\p{L}\p{N}]/gu, "");
}

function makeI18n(locale: "zh-TW" | "en") {
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: "en",
    messages: {
      "zh-TW": zhMessages,
      en: enMessages,
    },
  });
}

describe("F28 ARIA wording differentiation (transcribing vs enhancing)", () => {
  it("hud.aria.transcribing and hud.aria.enhancing differ by more than punctuation/whitespace in zh-TW + en", () => {
    // Run both locales in a single it() to keep the chunk-3 test count
    // aligned with the kickoff log's +8 target. Each locale is asserted
    // independently below.
    for (const locale of ["zh-TW", "en"] as const) {
      const i18n = makeI18n(locale);
      const t = i18n.global.t;
      const transcribing = t("hud.aria.transcribing");
      const enhancing = t("hud.aria.enhancing");
      // Sanity: both keys resolved (non-empty, not the literal key name).
      expect(transcribing, `${locale} transcribing resolved`).toBeTruthy();
      expect(enhancing, `${locale} enhancing resolved`).toBeTruthy();
      expect(transcribing).not.toBe("hud.aria.transcribing");
      expect(enhancing).not.toBe("hud.aria.enhancing");
      // The actual F28 assertion — Unicode-aware so CJK characters count.
      const t1Body = alphanumericChars(transcribing);
      const t2Body = alphanumericChars(enhancing);
      // Self-check: the alphanumeric bodies must be non-empty (otherwise
      // the trap would let the assertion pass trivially — \p{L} catches
      // CJK so this confirms the regex is doing what we think).
      expect(
        t1Body.length,
        `${locale} transcribing alphanum body non-empty`,
      ).toBeGreaterThan(0);
      expect(
        t2Body.length,
        `${locale} enhancing alphanum body non-empty`,
      ).toBeGreaterThan(0);
      expect(t1Body, `${locale} transcribing vs enhancing differ`).not.toBe(
        t2Body,
      );
    }
  });
});
