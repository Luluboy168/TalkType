// Dashboard window entry — bootstraps the standard decorated window.
// HUD window is at src/main.ts.
import "@fontsource-variable/geist";
import "./assets/index.css";

import { createPinia } from "pinia";
import { createApp } from "vue";

import MainApp from "./MainApp.vue";
import { createAppI18n } from "./i18n";
import { router } from "./router";

// Tag the body so the HUD-only transparent rule in index.css does not apply.
document.body.setAttribute("data-window", "dashboard");

// Disable the browser context menu inside the Tauri webview — feels more like
// a native app and avoids confusing right-click options (Reload, Inspect…).
document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = createApp(MainApp);
app.use(createPinia());
app.use(router);
app.use(createAppI18n("dashboard"));
app.mount("#app");
