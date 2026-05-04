// Settings Pinia store — read-through cache over the Rust-owned `Settings`
// state (M4 chunk 4).
//
// **Why a thin read-through cache (vs SayIt's frontend-side store)**:
//   * Settings live in `src-tauri/src/settings.rs` (M4 chunk 3) wrapped in
//     `Arc<RwLock<Settings>>`. The HUD + Dashboard share that one source of
//     truth; this store just caches it for reactive Vue consumption.
//   * `update()` does NOT optimistically mutate — it invokes
//     `update_settings` and lets the Rust side broadcast `settings:updated`
//     so every window converges on the persisted-on-disk shape. No local
//     state can drift from the keyring / JSON store.
//   * Subscribing once at bootstrap is idempotent (`subscribe()` short-
//     circuits if a listener is already attached).
//
// **Lifecycle**: typical consumer is `<SettingsHotkeySection>` which calls
// `load()` + `subscribe()` on mount. `unsubscribe()` is provided for tests
// and for the (currently unused) explicit teardown path; the store is
// shared across the Dashboard so components don't normally tear down the
// Rust event subscription on unmount — the listener leaks intentionally
// across navigations within a single window's lifetime.
//
// **Out of scope** (deferred to later milestones):
//   * Optimistic UI (M5+ if HUD-side responsiveness needs it)
//   * Cross-window broadcast (Rust already covers via `app.emit`)
//   * Migration UI (M8 — schema bumps)
import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { readonly, ref } from "vue";

import { listenToEvent, SETTINGS_UPDATED } from "@/composables/useTauriEvents";
import type { Settings, SettingsPatch } from "@/types/settings";

export const useSettingsStore = defineStore("settings", () => {
  /** Cached snapshot of the Rust-owned `Settings`. `null` until the first
   * `load()` resolves. Components should guard on `null` (typical pattern:
   * `v-if="settings"` in templates). */
  const settings = ref<Settings | null>(null);
  /** True from store creation until the first `load()` resolves (success or
   * failure). Lets components show a spinner / skeleton on first paint. */
  const loading = ref<boolean>(true);
  /** Last error message from `load()` or `update()`. Cleared on successful
   * subsequent calls. Components surface this in their own UI; the store
   * doesn't auto-revert it. */
  const error = ref<string | null>(null);
  /**
   * Cleanup function returned by `listenToEvent(SETTINGS_UPDATED, ...)`.
   * `null` until `subscribe()` is called. Held in a closure rather than a
   * ref because Vue reactivity isn't needed and would just add noise to the
   * dev tools. The closure is per-store-instance so multiple Pinia
   * instances (e.g. across test cases via `setActivePinia`) don't share.
   */
  let unlistenSettingsUpdated: (() => void) | null = null;

  /**
   * Initial fetch from Rust. Call once at component mount; idempotent if
   * called again (just re-fetches and updates the cached snapshot).
   *
   * On failure, `settings` is set to `null` so consumers fall back to their
   * "no settings yet" branch rather than rendering stale data. The error
   * message surfaces on `error` for inline display.
   */
  async function load(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      settings.value = await invoke<Settings>("get_settings");
    } catch (err) {
      error.value = formatError(err);
      settings.value = null;
    } finally {
      loading.value = false;
    }
  }

  /**
   * Send a sparse patch to Rust. The store does NOT optimistically update
   * `settings.value` — it waits for the `settings:updated` event so the UI
   * always reflects what's persisted on disk. Re-throws on invoke failure
   * so callers can surface inline errors next to the form button (matching
   * the `<SettingsApiKeySection>` pattern).
   */
  async function update(patch: SettingsPatch): Promise<void> {
    error.value = null;
    try {
      await invoke<Settings>("update_settings", { patch });
      // settings:updated listener will refresh `settings.value`.
    } catch (err) {
      error.value = formatError(err);
      throw err;
    }
  }

  /**
   * Subscribe to Rust `settings:updated` events. Idempotent — guarded
   * against double-init so calling `subscribe()` from multiple components
   * doesn't stack listeners.
   *
   * The listener writes the event payload (the new merged `Settings`)
   * directly into `settings.value`, fanning the change out to every
   * reactive consumer in the window.
   */
  async function subscribe(): Promise<void> {
    if (unlistenSettingsUpdated) return;
    unlistenSettingsUpdated = await listenToEvent<Settings>(
      SETTINGS_UPDATED,
      (event) => {
        settings.value = event.payload;
      },
    );
  }

  /**
   * Tear down the `settings:updated` listener. Mainly used by tests so a
   * fresh `setActivePinia` cycle doesn't leak listeners across cases.
   * Production code typically lets the subscription live for the lifetime
   * of the window — see header comment.
   */
  function unsubscribe(): void {
    if (unlistenSettingsUpdated) {
      unlistenSettingsUpdated();
      unlistenSettingsUpdated = null;
    }
  }

  /**
   * Best-effort error normalization. Rust commands serialize their
   * `thiserror` enums as flat strings (per CLAUDE.md "Error enum 手動
   * implement Serialize 為 string"), so the typical input is already a
   * string. We still handle the `Error` instance + arbitrary object shapes
   * defensively, mirroring `useVoiceFlowStore.formatError`.
   */
  function formatError(err: unknown): string {
    if (typeof err === "string") return err;
    if (err instanceof Error) return err.message;
    if (err && typeof err === "object" && "message" in err) {
      const msg = (err as { message: unknown }).message;
      if (typeof msg === "string") return msg;
    }
    return "Unknown error";
  }

  return {
    // Read-only refs for consumers — mutations only via `load`/`update`.
    settings: readonly(settings),
    loading: readonly(loading),
    error: readonly(error),
    load,
    update,
    subscribe,
    unsubscribe,
  };
});
