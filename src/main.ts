// HUD window entry — bootstraps the small always-on-top overlay.
// Dashboard window is at src/main-window.ts.
import "@fontsource-variable/geist";
import "./assets/index.css";

import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";
import { createAppI18n } from "./i18n";

// Tag the body so the global CSS can opt this window out of the solid
// background (HUD is transparent through to the desktop).
document.body.setAttribute("data-window", "hud");

const app = createApp(App);
app.use(createPinia());
app.use(createAppI18n("hud"));
app.mount("#app");
