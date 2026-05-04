// Settings module — Rust-owned source of truth for the persisted user
// configuration (M4 chunk 3).
//
// **Why Rust-owned (over SayIt's frontend-Pinia + tauri-plugin-store)**:
//   * Two windows (HUD + Dashboard) share the same Settings — putting it in
//     a frontend store causes race conditions on writes.
//   * `update_settings` is the single chokepoint; the patch is applied
//     atomically inside the `RwLock`, persisted to JSON, then broadcast as a
//     `settings:updated` event so every subscriber sees the same snapshot.
//   * Hotkey changes must hot-swap the live `HotkeyListenerState` atomics in
//     the same transaction — keeping that glue here rather than in the
//     frontend prevents a window from believing the hotkey is one thing
//     while the OS hook treats it as another.
//
// **v1 schema (M4 chunk 3 minimal)**:
//   * `schema_version: u32`     — reserved for migration on destructive
//                                  changes; bumped per
//                                  `doc/plans/05-data-model.md` migration
//                                  policy.
//   * `hotkey: HotkeyConfig`    — global hotkey trigger key + mode.
//
// M5–M8 will extend `Settings` in place (HUD options, LLM polish provider,
// audio device, history retention, etc.). The JSON store keeps unknown fields
// across versions so a user mid-upgrade does not lose state.
//
// **Persistence**: `tauri-plugin-store` writes `app_data_dir/settings.json`
// atomically. Auto-save is enabled via `Store::set` + `Store::save` so the
// JSON on disk reflects the in-memory `Arc<RwLock<Settings>>` after each
// `update`. Defense in depth: malformed JSON in the store falls back to
// defaults rather than crashing the app.
//
// **Error contract**: `SettingsError` is a `thiserror` enum with manual
// `Serialize` to a flat string, mirroring `CredentialsError` /
// `TranscriptionError`. The frontend always receives a `string` from
// `invoke<T>()` failure paths.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize, Serializer};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use thiserror::Error;

use crate::plugins::hotkey_listener::{HotkeyConfig, HotkeyListenerState};

/// File name (relative to `app_data_dir`) used by `tauri-plugin-store`. Kept
/// `.json` for export/import round-trips with a text editor.
const SETTINGS_STORE_PATH: &str = "settings.json";

/// Top-level key inside the JSON store. The store is a `HashMap<String,
/// Value>` so we wrap the whole `Settings` struct under one key — that way
/// a future migration can read the previous shape from the same store
/// without reshuffling other future top-level keys (e.g. `last_used_at`).
const SETTINGS_KEY: &str = "settings";

/// Current persisted schema version. Bump when a destructive change requires
/// migration code. M4 chunk 3 ships v1 with only the `hotkey` field.
const SCHEMA_VERSION: u32 = 1;

// ─── Schema ────────────────────────────────────────────────────────────────

/// Source-of-truth Settings struct. Frontend reads via `get_settings` command
/// and listens to `settings:updated` for hot updates.
///
/// `#[serde(rename_all = "camelCase")]` matches the TypeScript `Settings`
/// interface in `src/types/settings.ts`. M5+ milestones extend this struct
/// in place (no rename to break existing JSON store contents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Schema version. Bumped when a destructive change requires migration.
    /// Persistent; `serde(default)` so older JSON without this field still
    /// loads and gets normalized to the current `SCHEMA_VERSION`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Global hotkey configuration. Default per `HotkeyConfig::default()`
    /// (RightAlt + Hold). M5+ may add other top-level fields here.
    #[serde(default)]
    pub hotkey: HotkeyConfig,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            hotkey: HotkeyConfig::default(),
        }
    }
}

/// Partial settings update — frontend sends this in `update_settings`. Every
/// field is `Option<>` so the patch is sparse; missing fields are left
/// untouched. The match-and-merge happens in `apply_patch`.
///
/// The `#[serde(default)]` on each field plus `#[serde(default)]` on the
/// struct itself means an empty `{}` body deserializes to a no-op patch
/// (useful for validation flows that just want to round-trip the current
/// state).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    /// Replace the entire `hotkey` config when present.
    pub hotkey: Option<HotkeyConfig>,
    // M5+ adds further `Option<...>` fields here. Each new field must
    // appear in `apply_patch` below.
}

// ─── Error ─────────────────────────────────────────────────────────────────

/// Errors returned by the settings Tauri commands and `SettingsState` API.
///
/// Variants split coarsely:
///   * Storage layer (StoreUnavailable / JsonMalformed)
///   * Concurrency (LockPoisoned)
///   * Side effects (EventEmitFailed / HotkeyApplyFailed)
#[derive(Error, Debug)]
pub enum SettingsError {
    /// `tauri-plugin-store` could not be acquired or saved (disk full,
    /// permission, plugin not registered).
    #[error("Settings store unavailable: {0}")]
    StoreUnavailable(String),

    /// JSON in the store could not be deserialized into `Settings`. The
    /// caller logs the error and falls back to `Settings::default()` rather
    /// than blocking startup. Variant kept so the manual smoke / future
    /// CLI repair path can surface "your settings.json was corrupt and got
    /// reset" to the user.
    #[error("Settings JSON malformed: {0}")]
    JsonMalformed(String),

    /// `RwLock` poisoned by a prior panic in another thread. Should be
    /// effectively impossible in practice but we never `unwrap()` so the
    /// app survives.
    #[error("Settings state lock poisoned")]
    LockPoisoned,

    /// `Emitter::emit("settings:updated", ...)` failed. We continue (the
    /// in-memory + on-disk state is already updated) but surface this so
    /// the manual smoke can investigate why a window didn't refresh.
    #[error("Failed to broadcast settings:updated event: {0}")]
    EventEmitFailed(String),

    /// Hotkey listener hot-swap failed. Currently never returned because
    /// `apply_config` is infallible — kept here so M4 chunk 3 has a slot
    /// for if hot-swap ever grows fallible side effects.
    #[allow(dead_code)]
    #[error("Hotkey apply failed: {0}")]
    HotkeyApplyFailed(String),
}

// Manual `Serialize` so the frontend receives a flat string, mirroring the
// pattern in `CredentialsError` / `TranscriptionError`.
impl Serialize for SettingsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ─── State ─────────────────────────────────────────────────────────────────

/// Tauri-managed state slot for settings. `Arc<RwLock<Settings>>` so reads
/// (which fire from multiple commands on multiple threads) don't block each
/// other; writes are exclusive.
pub struct SettingsState {
    inner: Arc<RwLock<Settings>>,
}

impl SettingsState {
    /// Build initial state from disk. Loads from
    /// `app_data_dir/settings.json` if present + valid; otherwise initializes
    /// with `Settings::default()` and persists. Called from `lib.rs` setup
    /// hook.
    ///
    /// Malformed JSON in the store is logged + replaced with defaults rather
    /// than blocking startup — corrupt settings shouldn't make the app
    /// unstartable. The manual smoke covers this path; tests cover the
    /// pure deserialization fallback in `load_from_value`.
    pub fn load_or_default(app: &AppHandle) -> Result<Self, SettingsError> {
        let store = app
            .store(SETTINGS_STORE_PATH)
            .map_err(|e| SettingsError::StoreUnavailable(e.to_string()))?;

        let initial = match store.get(SETTINGS_KEY) {
            Some(json) => match load_from_value(json) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[settings] malformed settings.json — falling back to default: {e}");
                    let d = Settings::default();
                    if let Ok(value) = serde_json::to_value(&d) {
                        store.set(SETTINGS_KEY, value);
                        if let Err(save_err) = store.save() {
                            eprintln!("[settings] failed to persist default after malformed load: {save_err}");
                        }
                    }
                    d
                }
            },
            None => {
                let d = Settings::default();
                let value = serde_json::to_value(&d)
                    .map_err(|e| SettingsError::JsonMalformed(e.to_string()))?;
                store.set(SETTINGS_KEY, value);
                store
                    .save()
                    .map_err(|e| SettingsError::StoreUnavailable(e.to_string()))?;
                d
            }
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(initial)),
        })
    }

    /// In-memory snapshot getter for `get_settings` command. Cheap clone —
    /// `Settings` is small (`schema_version: u32` + 2 enum bytes via
    /// `HotkeyConfig`).
    pub fn snapshot(&self) -> Result<Settings, SettingsError> {
        self.inner
            .read()
            .map_err(|_| SettingsError::LockPoisoned)
            .map(|s| s.clone())
    }

    /// Apply a `SettingsPatch`:
    ///   1. Merge the patch into the in-memory `Settings` under the write
    ///      lock (atomic).
    ///   2. Persist the merged result to the JSON store (`store.save()`).
    ///   3. If the patch touched `hotkey`, hot-swap the live
    ///      `HotkeyListenerState` atomics so the OS hook sees the new
    ///      config on the very next keystroke.
    ///   4. Broadcast `settings:updated` to all windows.
    ///
    /// The whole flow returns the new merged `Settings` so the calling
    /// command body can return it to the frontend without an extra
    /// snapshot read.
    pub fn update(&self, app: &AppHandle, patch: SettingsPatch) -> Result<Settings, SettingsError> {
        // 1. Merge inside the write lock.
        let next = {
            let mut guard = self
                .inner
                .write()
                .map_err(|_| SettingsError::LockPoisoned)?;
            apply_patch(&mut guard, &patch);
            guard.clone()
        };

        // 2. Persist. Store handle is cheap (Arc<...>) so we re-acquire
        // here rather than holding it across the lock above.
        let store = app
            .store(SETTINGS_STORE_PATH)
            .map_err(|e| SettingsError::StoreUnavailable(e.to_string()))?;
        let value =
            serde_json::to_value(&next).map_err(|e| SettingsError::JsonMalformed(e.to_string()))?;
        store.set(SETTINGS_KEY, value);
        store
            .save()
            .map_err(|e| SettingsError::StoreUnavailable(e.to_string()))?;

        // 3. Hot-swap the live hotkey listener if the patch touched it.
        // `try_state` rather than `state` so an early-error path during
        // setup doesn't panic if the listener wasn't installed yet (unit
        // test flows + non-Windows builds).
        if patch.hotkey.is_some() {
            if let Some(hotkey_state) = app.try_state::<HotkeyListenerState>() {
                hotkey_state.apply_config(next.hotkey);
            }
        }

        // 4. Broadcast. We surface emit failure as an error — the on-disk
        // + in-memory state already updated, but a missed broadcast means
        // the Settings UI in the other window won't refresh until restart,
        // which is bad UX worth flagging.
        app.emit("settings:updated", &next)
            .map_err(|e| SettingsError::EventEmitFailed(e.to_string()))?;

        Ok(next)
    }
}

// ─── Pure helpers (testable without a Tauri AppHandle) ────────────────────

/// Apply a `SettingsPatch` to a mutable `Settings`. Pure logic so unit tests
/// can exercise the merge without spinning up a Tauri runtime.
///
/// Currently only one optional field; M5+ adds more. Keep this fn the single
/// chokepoint that knows the patch shape so reviewers can grep for `apply_patch`
/// to verify every new field is wired.
fn apply_patch(settings: &mut Settings, patch: &SettingsPatch) {
    if let Some(hotkey) = patch.hotkey {
        settings.hotkey = hotkey;
    }
    // M5+ adds further patch.field merges here.
}

/// Deserialize a JSON value into `Settings`. Returns
/// `Err(SettingsError::JsonMalformed)` on shape mismatch. Pure so unit
/// tests can exercise both the success and the malformed-fallback paths
/// without touching a real `Store`.
fn load_from_value(value: serde_json::Value) -> Result<Settings, SettingsError> {
    serde_json::from_value::<Settings>(value)
        .map_err(|e| SettingsError::JsonMalformed(e.to_string()))
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Snapshot of the current settings. The HUD / Dashboard call this once on
/// boot, then subscribe to `settings:updated` for live changes.
#[tauri::command]
pub async fn get_settings(
    state: tauri::State<'_, SettingsState>,
) -> Result<Settings, SettingsError> {
    state.snapshot()
}

/// Apply a partial patch and broadcast the new state. Returns the merged
/// `Settings` so the caller (typically a Settings sub-component) can
/// optimistically render the new state without waiting for the
/// `settings:updated` event round trip.
#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
    patch: SettingsPatch,
) -> Result<Settings, SettingsError> {
    state.update(&app, patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::hotkey_listener::{TriggerKey, TriggerMode};

    #[test]
    fn default_settings_serializes_to_expected_camel_case_shape() {
        let settings = Settings::default();
        let value = serde_json::to_value(&settings).expect("serialize");
        // Field name + nested shape both camelCase, schema version 1, hotkey
        // defaults RightAlt + Hold per M4 chunk 1 contract.
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["hotkey"]["triggerKey"], "right-alt");
        assert_eq!(value["hotkey"]["triggerMode"], "hold");
    }

    #[test]
    fn default_settings_round_trips_through_json() {
        let settings = Settings::default();
        let value = serde_json::to_value(&settings).expect("serialize");
        let parsed: Settings = serde_json::from_value(value).expect("deserialize");
        assert_eq!(parsed, settings);
    }

    #[test]
    fn settings_loads_with_legacy_missing_schema_version() {
        // Defense: future migrations might write the file before this
        // schema_version field existed. `serde(default)` should fall back to
        // current SCHEMA_VERSION rather than failing.
        let value = serde_json::json!({
            "hotkey": { "triggerKey": "right-alt", "triggerMode": "hold" }
        });
        let parsed = load_from_value(value).expect("legacy without schemaVersion still parses");
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn settings_loads_with_legacy_missing_hotkey() {
        // Defense: a partial / hand-edited JSON without `hotkey` should
        // fall back to the default hotkey rather than failing the load.
        let value = serde_json::json!({ "schemaVersion": 1 });
        let parsed = load_from_value(value).expect("missing hotkey falls back to default");
        assert_eq!(parsed.hotkey, HotkeyConfig::default());
    }

    #[test]
    fn malformed_json_returns_json_malformed_error() {
        // A non-object JSON value (string instead of struct) should produce
        // JsonMalformed. The `load_or_default` caller logs + falls back to
        // defaults; this test confirms the inner classification.
        let value = serde_json::Value::String("not a settings object".to_string());
        let err = load_from_value(value).unwrap_err();
        assert!(matches!(err, SettingsError::JsonMalformed(_)));
    }

    #[test]
    fn apply_patch_merges_only_supplied_fields() {
        let mut settings = Settings::default();
        let patch = SettingsPatch {
            hotkey: Some(HotkeyConfig {
                trigger_key: TriggerKey::LeftControl,
                trigger_mode: TriggerMode::Toggle,
            }),
        };
        apply_patch(&mut settings, &patch);
        assert_eq!(settings.hotkey.trigger_key, TriggerKey::LeftControl);
        assert_eq!(settings.hotkey.trigger_mode, TriggerMode::Toggle);
        // Untouched fields stay at default.
        assert_eq!(settings.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn apply_empty_patch_is_noop() {
        let mut settings = Settings::default();
        let original = settings.clone();
        apply_patch(&mut settings, &SettingsPatch::default());
        assert_eq!(settings, original);
    }

    #[test]
    fn snapshot_returns_clone_of_inner() {
        // Build a SettingsState directly without going through Tauri so we
        // can exercise the lock paths in unit-test isolation.
        let state = SettingsState {
            inner: Arc::new(RwLock::new(Settings::default())),
        };
        let snap = state.snapshot().expect("snapshot");
        assert_eq!(snap, Settings::default());
    }

    #[test]
    fn snapshot_returns_lock_poisoned_when_inner_is_poisoned() {
        let inner = Arc::new(RwLock::new(Settings::default()));
        // Poison the lock by panicking inside a write guard.
        let cloned = inner.clone();
        let _ = std::thread::spawn(move || {
            let _guard = cloned.write().expect("write");
            panic!("simulated poison");
        })
        .join();
        let state = SettingsState { inner };
        let err = state.snapshot().unwrap_err();
        assert!(matches!(err, SettingsError::LockPoisoned));
    }

    #[test]
    fn settings_error_serializes_as_flat_string() {
        let err = SettingsError::LockPoisoned;
        let json = serde_json::to_string(&err).expect("serialize");
        assert_eq!(json, "\"Settings state lock poisoned\"");

        let err = SettingsError::JsonMalformed("expected object".to_string());
        let json = serde_json::to_string(&err).expect("serialize");
        assert!(json.starts_with('"') && json.ends_with('"'));
        assert!(json.contains("Settings JSON malformed"));
        assert!(json.contains("expected object"));
    }

    #[test]
    fn settings_patch_deserializes_empty_object_as_default() {
        // The frontend may invoke `update_settings({ patch: {} })` to no-op
        // round-trip; serde(default) on the struct + each field makes that
        // safe.
        let patch: SettingsPatch = serde_json::from_str("{}").expect("empty object");
        assert!(patch.hotkey.is_none());
    }

    #[test]
    fn settings_patch_deserializes_with_camel_case_hotkey() {
        // Mirrors the wire format the frontend sends.
        let json = r#"{"hotkey":{"triggerKey":"left-shift","triggerMode":"toggle"}}"#;
        let patch: SettingsPatch = serde_json::from_str(json).expect("hotkey patch");
        assert_eq!(patch.hotkey.unwrap().trigger_key, TriggerKey::LeftShift);
    }
}
