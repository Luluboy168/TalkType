// Audio recorder IPC contract types — single source of truth for command
// argument / return shapes that bridge `src-tauri/src/plugins/audio_recorder/`
// and the Vue / composable consumers in `src/composables/useAudio*.ts` plus
// the Settings mic picker / Dashboard record-test card.
//
// Naming convention (per `doc/plans/04-frontend-structure.md`):
//   * `*Info` for Rust struct payloads emitted from `list_*` commands.
//   * `*Result` for `Result<T, E>` `Ok` payloads of action commands.
//
// Keep field names matching the Rust `#[serde(rename_all = "camelCase")]`
// rendering of each struct — see `audio_recorder/stream.rs::AudioInputDeviceInfo`
// and `audio_recorder/mod.rs::StopRecordingResult` for the source-of-truth shapes.

/**
 * Lightweight description of a single cpal input device, returned by the
 * `list_audio_input_devices` Tauri command. `isDefault` flags the host's
 * default input device (when any) — the Settings UI uses it to preselect a
 * sensible entry.
 */
export interface AudioInputDeviceInfo {
  /** Device name as reported by `cpal::Device::name()`. */
  name: string;
  /** True for the host's default input device; at most one entry per host. */
  isDefault: boolean;
}

/**
 * Result returned by the `stop_recording` Tauri command. Mirrors
 * `audio_recorder::StopRecordingResult` in Rust. `peakEnergyLevel` and
 * `rmsEnergyLevel` are normalized to `[0.0, 1.0]`; UI consumers can use them
 * to detect silent / clipped takes before invoking transcription.
 */
export interface StopRecordingResult {
  /** Total recording duration in milliseconds. */
  durationMs: number;
  /** Peak amplitude over the full recording, in `[0.0, 1.0]`. */
  peakEnergyLevel: number;
  /** RMS amplitude over the full recording, in `[0.0, 1.0]`. */
  rmsEnergyLevel: number;
  /** Total number of mono `i16` samples buffered. */
  sampleCount: number;
}
