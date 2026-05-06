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

import type { LlmPromptMode, LlmProviderId } from "./llm";

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

/**
 * Top-level Settings shape mirroring the Rust `Settings` struct in
 * `src-tauri/src/settings.rs`. The Rust side serializes with
 * `#[serde(rename_all = "camelCase")]` so all field names are camelCase
 * here. v1 was M4 chunk 3 (`schemaVersion` + `hotkey`); M6 chunk 0 adds 7
 * optional LLM polish fields (purely additive, schema_version stays 1 so
 * M5 settings.json forward-loads with all new fields = `undefined`).
 */
export interface Settings {
  /** Persisted schema version. v1 = M4 chunk 3 (M6 chunk 0 stays v1). */
  schemaVersion: number;
  /** Global hotkey configuration. Persisted across launches. */
  hotkey: HotkeyConfig;

  // ─── M6 chunk 0 LLM polish fields (Decisions #5 / #7) ───────────────────
  /**
   * Tri-state polish gate (Decision #7):
   *   * `undefined` — auto-detect via `has_credential(provider)` (M5 → M6
   *                    upgrade default; trust-transitive from existing key)
   *   * `true`      — explicit ON (surfaces ApiKeyMissing if no key)
   *   * `false`     — explicit OFF (skip polish, paste raw transcript)
   */
  llmPolishEnabled?: boolean;
  /** Active polish provider id (Decision #3 4 free providers). */
  llmProvider?: LlmProviderId;
  /** Pinned model id within the chosen provider (e.g. `"llama-3.3-70b-versatile"`). */
  llmModelId?: string;
  /**
   * Escape hatch for model deprecation (F4): if set, this raw string is
   * passed verbatim to the provider, bypassing `llmModelId`. UI hidden in
   * M6 — user edits `settings.json` by hand. Surfaces in v0.2.
   */
  llmModelIdOverride?: string;
  /** Active prompt-mode discriminant (`'default' | 'email' | ... | 'custom'`). */
  llmPromptMode?: LlmPromptMode;
  /**
   * User-provided system prompt when `llmPromptMode === "custom"`. Rust
   * `validate_custom_prompt` enforces `chars().count() ≤ 1000` (CJK 1 char
   * = 3 bytes is fine — counting code points, not bytes).
   */
  llmCustomPrompt?: string;
  /**
   * Tri-state retry toggle (Decision #5):
   *   * `undefined` or `true` — retry once on transient failure (default)
   *   * `false`               — no retry, fall back to raw immediately
   */
  llmPolishRetryEnabled?: boolean;
}

/**
 * Sparse update payload for `update_settings` Tauri command. Every
 * field is optional; missing fields are left untouched on the Rust
 * side. M5-M8 will add further `field?: T` here as Settings grows.
 *
 * M6 chunk 0 mirrors the 7 LLM polish fields from `Settings`. To explicitly
 * "clear" a value back to `undefined` the user goes through a chunk-4
 * dedicated reset flow (a sparse patch with the field absent does NOT clear
 * the stored value — that matches the Rust `apply_patch` semantics).
 */
export interface SettingsPatch {
  hotkey?: HotkeyConfig;
  llmPolishEnabled?: boolean;
  llmProvider?: LlmProviderId;
  llmModelId?: string;
  llmModelIdOverride?: string;
  llmPromptMode?: LlmPromptMode;
  llmCustomPrompt?: string;
  llmPolishRetryEnabled?: boolean;
}
