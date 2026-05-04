// Settings IPC contract types — single source of truth for the
// `get_settings` / `update_settings` Tauri commands and the `settings:updated`
// event payload that bridges `src-tauri/src/settings.rs` (Rust-owned, M4
// chunk 3) and the Vue Settings UI (`src/views/SettingsView.vue` hotkey
// section, M4 chunk 4).
//
// Naming convention (per `doc/plans/04-frontend-structure.md`):
//   * `*Config` for nested struct shapes inside the larger `Settings` object.
//
// Field names match the Rust `#[serde(rename_all = "camelCase")]` rendering
// of each struct — see `src-tauri/src/settings.rs::HotkeyConfig` for the
// source-of-truth shape (M4 chunk 3 owns the Rust side).

/**
 * Allowed preset trigger keys for the global hotkey listener. Phase 1 ships
 * preset-only — `Custom { keycode }` and `Combo { modifiers, keycode }` are
 * deferred to Phase 2 (see `doc/plans/02-implementation-roadmap.md` M4 task
 * "不做 Custom keycode recording — preset only — 簡化").
 *
 * Values are kebab-case strings matching the Rust enum's
 * `#[serde(rename_all = "kebab-case")]` rendering for `TriggerKey`.
 */
export type TriggerKey =
  | "right-alt"
  | "left-alt"
  | "right-control"
  | "left-control"
  | "right-shift"
  | "left-shift";

/**
 * How the hotkey toggles the recording state.
 *   * `hold`   — push-to-talk: key down starts recording, key up stops.
 *   * `toggle` — tap-to-talk: each press XORs `is_toggled_on`.
 */
export type TriggerMode = "hold" | "toggle";

/**
 * Hotkey configuration as stored in `Settings.hotkey` (Rust-owned).
 *
 * Defaults (set by `settings.rs::Settings::default()` in M4 chunk 3):
 *   * `triggerKey:  "right-alt"`
 *   * `triggerMode: "hold"`
 */
export interface HotkeyConfig {
  /** Active trigger key (preset). */
  triggerKey: TriggerKey;
  /** Push-to-talk vs tap-to-talk. */
  triggerMode: TriggerMode;
}
