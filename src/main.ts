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

// M5 chunk 2 vite-only shim — when running `pnpm dev` (no Tauri runtime),
// `window.__TAURI_INTERNALS__` is undefined so any `invoke()` / `listen()`
// throws `Cannot read properties of undefined (reading 'transformCallback')`.
// We inject a no-op shim so the HUD mounts cleanly for Playwright screenshot
// captures. Tree-shaken from production by `import.meta.env.DEV` —
// production never ships this branch.
if (import.meta.env.DEV) {
  const tauriWin = window as unknown as {
    __TAURI_INTERNALS__?: unknown;
  };
  if (!tauriWin.__TAURI_INTERNALS__) {
    let nextCallbackId = 0;
    tauriWin.__TAURI_INTERNALS__ = {
      transformCallback: <T>(_cb: (msg: T) => void): number => {
        // Real Tauri stores cb in window so the IPC bridge can invoke it
        // via id — for vite shim we just return an ID (never used).
        nextCallbackId += 1;
        return nextCallbackId;
      },
      invoke: async (cmd: string): Promise<void> => {
        // Best-effort no-op so positioning / capture / start_recording etc.
        // resolve in vite mode without crashing the store.
        console.debug("[hud-vite-shim] invoke", cmd);
        return undefined;
      },
      ipc: () => {
        /* no-op */
      },
      // Chunk 2 reviewer P1-2: stub `unregisterListener` so the unlisten
      // returned by `listen()` doesn't crash on `useAudioWaveform.stop()`
      // during state transitions in vite-only dev mode.
      unregisterListener: (_event: string, _eventId: number): void => {
        /* no-op */
      },
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main", windowLabel: "main" },
      },
    };
  }

  // M5 chunk 2 reduced-motion screenshot helper — Playwright sets
  // sessionStorage('m5_reduced_motion'='1') then reloads; we intercept
  // matchMedia BEFORE Vue mounts so onMounted reads matches=true.
  // Toggle off by clearing the key + reload.
  if (sessionStorage.getItem("m5_reduced_motion") === "1") {
    const origMatchMedia = window.matchMedia.bind(window);
    window.matchMedia = (query: string): MediaQueryList => {
      if (query.includes("prefers-reduced-motion")) {
        return {
          matches: true,
          media: query,
          onchange: null,
          addEventListener: (() => {}) as MediaQueryList["addEventListener"],
          removeEventListener:
            (() => {}) as MediaQueryList["removeEventListener"],
          addListener: () => {},
          removeListener: () => {},
          dispatchEvent: () => false,
        } as MediaQueryList;
      }
      return origMatchMedia(query);
    };
  }
}

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
// can't attach (M5 will surface this visibly in M9 polish).
//
// **M5 chunk 1**: `init()` is now async (sequential awaits over Tauri's
// `listen()` instead of fire-and-forget `.then(push)`). We use the
// `then().catch()` form rather than top-level await so the rest of the
// module continues evaluating; the cleanup fn lands on `window` after the
// listeners actually finish registering.
const voiceFlow = useVoiceFlowStore();
voiceFlow
  .init()
  .then((cleanup) => {
    // Type-side: store cleanup fn on window for HMR safety. Strict-mode
    // TypeScript doesn't know about ad-hoc window props; cast through unknown.
    (
      window as unknown as { __voiceFlowCleanup?: () => void }
    ).__voiceFlowCleanup = cleanup;
  })
  .catch((err: unknown) => {
    console.error("[hud] voice flow init failed", err);
    // Phase 1: log only; M5 doesn't render an init-failure state. M9 polish.
  });

// M5 chunk 2 dev hook — exposes a small mutator on window so vite-only
// mode (`pnpm dev`, no Tauri runtime) can drive the HUD through its 4
// visual states from Playwright / devtools for screenshot captures.
// Tree-shaken from production builds via `import.meta.env.DEV`.
//
// Usage from devtools / browser_evaluate:
//   window.__hudDev.setStatus('recording', { startedAtMs: Date.now() })
//   window.__hudDev.setStatus('error', { message: 'API key 無效' })
//   window.__hudDev.setStatus('idle')
//
// Implementation: delegates to `__devSetStatus` on the store, which writes
// the inner writable refs directly (the public refs are `readonly()`-wrapped).
if (import.meta.env.DEV) {
  (
    window as unknown as {
      __hudDev?: {
        setStatus: (
          status:
            | "idle"
            | "recording"
            | "transcribing"
            | "enhancing"
            | "success"
            | "error",
          opts?: { message?: string; startedAtMs?: number | null },
        ) => void;
      };
    }
  ).__hudDev = {
    setStatus(status, opts = {}) {
      voiceFlow.__devSetStatus(
        status,
        opts.message ?? "",
        opts.startedAtMs ??
          (status === "recording" ? Date.now() : null),
      );
    },
  };
}
