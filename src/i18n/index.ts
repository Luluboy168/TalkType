// vue-i18n setup factory shared by both window entries (HUD + Dashboard).
// Phase 1 keeps every locale loaded for both scopes; we still pass a `scope`
// argument so future milestones can split bundles by window if size matters.
import { createI18n, type I18n } from "vue-i18n";

import enMessages from "./locales/en.json";
import zhTWMessages from "./locales/zh-TW.json";

export type AppLocale = "zh-TW" | "en";
export type I18nScope = "hud" | "dashboard";

const messages = {
  "zh-TW": zhTWMessages,
  en: enMessages,
} as const;

/**
 * Map navigator.language (e.g. "zh-TW", "zh-Hant", "en-US") to one of the
 * locales we ship. Defaults to "en" so unsupported locales still work.
 */
function detectLocale(): AppLocale {
  const navigatorLocale =
    typeof navigator !== "undefined" && typeof navigator.language === "string"
      ? navigator.language
      : "en";
  const normalized = navigatorLocale.toLowerCase();
  if (normalized.startsWith("zh")) {
    return "zh-TW";
  }
  return "en";
}

/**
 * Create a vue-i18n instance for the given window scope. The `scope` is
 * currently unused at runtime but kept in the signature so M1+ can specialize
 * messages per window without changing the entry-point call sites.
 */
export function createAppI18n(_scope: I18nScope): I18n {
  return createI18n({
    legacy: false,
    locale: detectLocale(),
    fallbackLocale: "en",
    messages,
  });
}
