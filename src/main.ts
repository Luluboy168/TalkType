// HUD window entry — bootstraps the small always-on-top overlay.
// Dashboard window is at src/main-window.ts.
//
// M4 chunk 3 wires `useVoiceFlowStore().init()` here so the HUD subscribes
// to hotkey / ESC / paste events at process boot — before any user input
// could arrive. The Dashboard window does NOT init the voice flow store
// (the voice flow only runs in the HUD; cross-window observation lands in
// M5 via the `voice-flow:state-changed` broadcast).
import "@fontsource-variable/geist";
import "./assets/index.css";

import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";
import { createAppI18n } from "./i18n";
import { useVoiceFlowStore } from "./stores/useVoiceFlowStore";

// Tag the body so the global CSS can opt this window out of the solid
// background (HUD is transparent through to the desktop).
document.body.setAttribute("data-window", "hud");

const app = createApp(App);
app.use(createPinia());
app.use(createAppI18n("hud"));
app.mount("#app");

// Initialize voice flow listeners after Pinia is installed. The cleanup fn
// is stashed on the window so a future hot-reload can call it; in production
// it never fires (the HUD lives for the entire app lifetime). Errors here
// are logged rather than thrown — the HUD must mount even if the listeners
// can't attach (M5 will surface this visibly).
try {
  const voiceFlow = useVoiceFlowStore();
  const cleanup = voiceFlow.init();
  // Type-side: store cleanup fn on window for HMR safety. Strict-mode
  // TypeScript doesn't know about ad-hoc window props; cast through unknown.
  (window as unknown as { __voiceFlowCleanup?: () => void }).__voiceFlowCleanup =
    cleanup;
} catch (err) {
  console.error("[hud] voice flow init failed", err);
}
