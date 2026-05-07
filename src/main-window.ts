// Dashboard window entry — bootstraps the standard decorated window.
// HUD window is at src/main.ts.
import "@fontsource-variable/geist";
import "./assets/index.css";

import { createPinia } from "pinia";
import { createApp } from "vue";

import MainApp from "./MainApp.vue";
import { createAppI18n } from "./i18n";
import { router } from "./router";

// M5 chunk 3 vite-only shim — when running `pnpm dev` (no Tauri runtime),
// `window.__TAURI_INTERNALS__` is undefined so `listen()` throws on the
// `transformCallback` deref. We inject a no-op shim that ALSO exposes a
// `__dashboardDev.emitFlow(...)` helper so Playwright can drive the
// `voice-flow:state-changed` listener for HudFlowBadge screenshots.
//
// Tree-shaken from production by `import.meta.env.DEV`.
if (import.meta.env.DEV) {
  type EventCallback = (e: { id: number; event: string; payload: unknown }) => void;
  const callbackRegistry = new Map<number, EventCallback>();
  // Track which callback id belongs to which event name. Tauri's `listen()`
  // calls `transformCallback(handler)` first (returning an id) then invokes
  // `plugin:event|listen` with `{event, handler: <id>}`, so we wire the two
  // together via the second step.
  const eventToCallbackIds = new Map<string, Set<number>>();

  const tauriWin = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (!tauriWin.__TAURI_INTERNALS__) {
    let nextCallbackId = 0;
    tauriWin.__TAURI_INTERNALS__ = {
      transformCallback: <T>(cb: (msg: T) => void): number => {
        nextCallbackId += 1;
        const id = nextCallbackId;
        callbackRegistry.set(id, cb as unknown as EventCallback);
        return id;
      },
      invoke: async (cmd: string, args?: Record<string, unknown>): Promise<number | void> => {
        // Mirror the bits of the Tauri runtime we actually need for vite mode.
        if (cmd === "plugin:event|listen" && args) {
          const event = args.event as string;
          const handlerId = args.handler as number;
          if (!eventToCallbackIds.has(event)) {
            eventToCallbackIds.set(event, new Set());
          }
          eventToCallbackIds.get(event)?.add(handlerId);
          return handlerId;
        }
        if (cmd === "plugin:event|unlisten" && args) {
          const event = args.event as string;
          const eventId = args.eventId as number;
          eventToCallbackIds.get(event)?.delete(eventId);
          callbackRegistry.delete(eventId);
          return undefined;
        }
        // Best-effort no-op for other invokes (positioning / settings / …).
        console.debug("[dashboard-vite-shim] invoke", cmd);
        return undefined;
      },
      ipc: () => {
        /* no-op */
      },
      unregisterListener: (event: string, eventId: number): void => {
        eventToCallbackIds.get(event)?.delete(eventId);
        callbackRegistry.delete(eventId);
      },
      metadata: {
        currentWindow: { label: "main-window" },
        currentWebview: { label: "main-window", windowLabel: "main-window" },
      },
    };
  }

  // Dispatcher exposed for Playwright / devtools so we can simulate the
  // cross-window `voice-flow:state-changed` event without spinning up a
  // Tauri runtime. Defaults to source='hud' so the badge defense passes.
  (
    window as unknown as {
      __dashboardDev?: {
        emitFlow: (
          status:
            | "idle"
            | "recording"
            | "transcribing"
            | "enhancing"
            | "success"
            | "error",
          opts?: { message?: string; source?: "hud" | "dashboard" },
        ) => void;
      };
    }
  ).__dashboardDev = {
    emitFlow(status, opts = {}) {
      const ids = eventToCallbackIds.get("voice-flow:state-changed");
      if (!ids || ids.size === 0) return;
      for (const id of ids) {
        const cb = callbackRegistry.get(id);
        cb?.({
          id,
          event: "voice-flow:state-changed",
          payload: {
            status,
            message: opts.message ?? "",
            source: opts.source ?? "hud",
          },
        });
      }
    },
  };
}

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
