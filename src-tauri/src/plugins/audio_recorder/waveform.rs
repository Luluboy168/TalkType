// FFT-based 6-bin waveform processor.
//
// Pulls the SayIt pattern (see `doc/reference/sayit-backend-analysis.md`):
//
//   * 64-sample ring buffer of normalized [-1.0, 1.0] f32 values fed by the
//     cpal callback / recording thread.
//   * `rustfft` forward FFT every ~16 ms (60 fps), windowed with a Hann
//     window pre-computed once at construction.
//   * 6 bins picked at hand-tuned indices `[9, 4, 1, 2, 6, 12]` for visual
//     spread (matches what the frontend HUD wants — NOT linear, NOT log).
//   * Magnitudes converted to dB via `20 * log10(.)` and mapped from
//     `[-100 dB, -20 dB]` to `[0.0, 1.0]` by `normalize_db`.
//
// The `WaveformProcessor` is single-threaded — the recording thread owns one
// of these and pushes samples + reads levels per tick. No Mutex, no channels.
//
// Public surface:
//   * `WAVEFORM_BIN_COUNT` / `FFT_WINDOW_SIZE` / `WAVEFORM_BIN_INDICES`
//   * `WaveformPayload { levels: [f32; 6] }` — serialized as `{ levels: [...] }`
//   * `WaveformProcessor::new()`
//   * `WaveformProcessor::push_samples(&[i16])`
//   * `WaveformProcessor::compute_levels() -> Option<[f32; 6]>`
//   * `normalize_db(magnitude_db, min_db, max_db) -> f32`

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;

use rustfft::{num_complex::Complex, Fft, FftPlanner};
use serde::Serialize;

/// Number of bins emitted per `audio:waveform` payload.
pub const WAVEFORM_BIN_COUNT: usize = 6;

/// Size of the FFT window. Drives the ring buffer capacity. 64 is what SayIt
/// uses — keeps FFT cheap (< 1 ms on modern hardware) so the 16 ms recording
/// tick can hit 60 fps comfortably.
pub const FFT_WINDOW_SIZE: usize = 64;

/// Indices into the FFT magnitude output that we pick for the 6 visual bins.
/// Hand-tuned for visual spread (not linear, not log). See SayIt reference.
pub const WAVEFORM_BIN_INDICES: [usize; WAVEFORM_BIN_COUNT] = [9, 4, 1, 2, 6, 12];

/// Lower edge of the dB normalization range. Magnitudes at or below this map
/// to `0.0`.
const DEFAULT_MIN_DB: f32 = -100.0;
/// Upper edge of the dB normalization range. Magnitudes at or above this map
/// to `1.0`.
const DEFAULT_MAX_DB: f32 = -20.0;

/// Floor used to avoid `log10(0.0)` returning `-inf`.
const MIN_MAGNITUDE: f32 = 1e-9;

/// Payload for the `audio:waveform` Tauri event. `[f32; 6]` serializes as a
/// JSON array, which TypeScript receives as `number[]` (we re-narrow to a
/// 6-tuple on the frontend type side).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WaveformPayload {
    pub levels: [f32; WAVEFORM_BIN_COUNT],
}

/// Stateful FFT processor.
///
/// Owns the rustfft plan + scratch buffer + ring buffer of recent samples.
/// Push i16 samples in via `push_samples`, then call `compute_levels` to get
/// the latest 6-bin normalized vector. `compute_levels` returns `None` until
/// the ring buffer has at least `FFT_WINDOW_SIZE` samples — early callers
/// (first ~4 ms of a fresh recording at 16 kHz) will see `None` and should
/// just skip emitting the event.
pub struct WaveformProcessor {
    plan: Arc<dyn Fft<f32>>,
    /// Pre-computed Hann window of length `FFT_WINDOW_SIZE`.
    window: [f32; FFT_WINDOW_SIZE],
    /// Recent normalized samples in [-1.0, 1.0]. Capacity = `FFT_WINDOW_SIZE`.
    /// Oldest sample drops off the front when a new sample is pushed beyond
    /// capacity.
    ring: VecDeque<f32>,
    /// Reusable buffer for the FFT input/output. `rustfft` does in-place FFT
    /// (`process`) so this slot holds both the windowed input AND the
    /// frequency-domain output across a single `compute_levels` call.
    scratch: Vec<Complex<f32>>,
}

impl WaveformProcessor {
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let plan = planner.plan_fft_forward(FFT_WINDOW_SIZE);

        // Pre-compute the Hann window: w[n] = 0.5 * (1 - cos(2π n / (N-1))).
        let mut window = [0.0_f32; FFT_WINDOW_SIZE];
        for (n, w) in window.iter_mut().enumerate() {
            *w = 0.5 * (1.0 - (2.0 * PI * (n as f32) / ((FFT_WINDOW_SIZE - 1) as f32)).cos());
        }

        Self {
            plan,
            window,
            ring: VecDeque::with_capacity(FFT_WINDOW_SIZE),
            scratch: vec![Complex::new(0.0, 0.0); FFT_WINDOW_SIZE],
        }
    }

    /// Push a slice of `i16` samples (already mono per the cpal callback) into
    /// the ring buffer. Each sample is normalized to `[-1.0, 1.0]`.
    ///
    /// If the ring is at capacity the oldest values fall off the front, so
    /// after the first `FFT_WINDOW_SIZE` samples are received the buffer always
    /// holds the most recent `FFT_WINDOW_SIZE` samples.
    pub fn push_samples(&mut self, samples: &[i16]) {
        for &s in samples {
            let normalized = (s as f32) / (i16::MAX as f32);
            if self.ring.len() == FFT_WINDOW_SIZE {
                self.ring.pop_front();
            }
            self.ring.push_back(normalized);
        }
    }

    /// Compute the 6-bin normalized magnitude vector from the latest
    /// `FFT_WINDOW_SIZE` samples in the ring buffer. Returns `None` if the
    /// ring buffer hasn't been filled yet.
    pub fn compute_levels(&mut self) -> Option<[f32; WAVEFORM_BIN_COUNT]> {
        if self.ring.len() < FFT_WINDOW_SIZE {
            return None;
        }

        // Copy ring → scratch with Hann window applied.
        // `make_contiguous` is the cheap path on a VecDeque whose buffer
        // hasn't wrapped — it does at most one memmove.
        let contiguous = self.ring.make_contiguous();
        debug_assert_eq!(contiguous.len(), FFT_WINDOW_SIZE);

        for (i, &sample) in contiguous.iter().enumerate().take(FFT_WINDOW_SIZE) {
            let windowed = sample * self.window[i];
            self.scratch[i] = Complex::new(windowed, 0.0);
        }

        // In-place FFT.
        self.plan.process(&mut self.scratch);

        let mut levels = [0.0_f32; WAVEFORM_BIN_COUNT];
        for (out_idx, &bin_idx) in WAVEFORM_BIN_INDICES.iter().enumerate() {
            // Defensive bound: the indices are constants ≤ 12, FFT_WINDOW_SIZE = 64.
            debug_assert!(bin_idx < FFT_WINDOW_SIZE);
            let magnitude = self.scratch[bin_idx].norm() / (FFT_WINDOW_SIZE as f32);
            let magnitude_db = 20.0 * magnitude.max(MIN_MAGNITUDE).log10();
            levels[out_idx] = normalize_db(magnitude_db, DEFAULT_MIN_DB, DEFAULT_MAX_DB);
        }
        Some(levels)
    }
}

impl Default for WaveformProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a dB magnitude into `[0.0, 1.0]`.
///
/// `magnitude_db <= min_db` → `0.0`
/// `magnitude_db >= max_db` → `1.0`
/// otherwise → linear interpolation across `[min_db, max_db]`
///
/// `min_db` MUST be < `max_db`. If they are equal or inverted the function
/// returns 0.0 (degenerate input — better than NaN/inf).
pub fn normalize_db(magnitude_db: f32, min_db: f32, max_db: f32) -> f32 {
    if min_db >= max_db {
        return 0.0;
    }
    if magnitude_db <= min_db {
        return 0.0;
    }
    if magnitude_db >= max_db {
        return 1.0;
    }
    (magnitude_db - min_db) / (max_db - min_db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_db_below_min_is_zero() {
        assert_eq!(normalize_db(-150.0, -100.0, -20.0), 0.0);
        assert_eq!(normalize_db(-100.0, -100.0, -20.0), 0.0);
    }

    #[test]
    fn normalize_db_above_max_is_one() {
        assert_eq!(normalize_db(-10.0, -100.0, -20.0), 1.0);
        assert_eq!(normalize_db(-20.0, -100.0, -20.0), 1.0);
    }

    #[test]
    fn normalize_db_midpoint_is_half() {
        // -60 dB is the midpoint of [-100, -20]
        let v = normalize_db(-60.0, -100.0, -20.0);
        assert!((v - 0.5).abs() < f32::EPSILON, "got {v}");
    }

    #[test]
    fn normalize_db_inverted_range_returns_zero() {
        // min >= max is degenerate; we return 0.0 rather than blowing up.
        assert_eq!(normalize_db(-50.0, -20.0, -100.0), 0.0);
        assert_eq!(normalize_db(-50.0, -50.0, -50.0), 0.0);
    }

    #[test]
    fn compute_levels_empty_ring_returns_none() {
        let mut proc = WaveformProcessor::new();
        assert!(proc.compute_levels().is_none());
    }

    #[test]
    fn compute_levels_partial_ring_returns_none() {
        let mut proc = WaveformProcessor::new();
        // Only push half the window; should still be None.
        let half = vec![100_i16; FFT_WINDOW_SIZE / 2];
        proc.push_samples(&half);
        assert!(proc.compute_levels().is_none());
    }

    #[test]
    fn push_samples_caps_at_window_size() {
        let mut proc = WaveformProcessor::new();
        // Push twice the window worth of samples; ring should still cap.
        let two_windows = vec![123_i16; FFT_WINDOW_SIZE * 2];
        proc.push_samples(&two_windows);
        assert_eq!(proc.ring.len(), FFT_WINDOW_SIZE);
    }

    #[test]
    fn compute_levels_full_ring_returns_some() {
        let mut proc = WaveformProcessor::new();
        // Fill ring with constant samples (DC). compute_levels returns Some;
        // values are valid floats in [0.0, 1.0].
        let full = vec![1000_i16; FFT_WINDOW_SIZE];
        proc.push_samples(&full);
        let levels = proc.compute_levels().expect("levels");
        for &l in &levels {
            assert!((0.0..=1.0).contains(&l), "level out of range: {l}");
            assert!(l.is_finite(), "level not finite: {l}");
        }
    }

    #[test]
    fn compute_levels_silent_input_is_near_zero() {
        // Pure silence: all bins should normalize to 0.0 (mag = 0 → -inf dB,
        // clamped to MIN_MAGNITUDE → ~-180 dB, well below -100).
        let mut proc = WaveformProcessor::new();
        let silence = vec![0_i16; FFT_WINDOW_SIZE];
        proc.push_samples(&silence);
        let levels = proc.compute_levels().expect("levels");
        for &l in &levels {
            assert_eq!(l, 0.0, "silence should be 0.0, got {l}");
        }
    }

    #[test]
    fn compute_levels_loud_sine_wave_lights_up_some_bin() {
        // Fill the ring with a sine wave. With FFT_WINDOW_SIZE = 64 and a
        // 16 kHz sample rate, frequency f maps to bin index k = f * 64 / 16000.
        // We don't tie to a specific bin index — just verify the loud signal
        // produces at least one bin that's substantially > 0.0.
        let mut proc = WaveformProcessor::new();
        let mut sine = Vec::with_capacity(FFT_WINDOW_SIZE);
        // Pick frequency such that the bin index is one of WAVEFORM_BIN_INDICES.
        // bin 4 → f = 4 * 16000 / 64 = 1000 Hz. With sample rate 16 kHz that's
        // a period of 16 samples, fits 4 cycles in 64 samples.
        for n in 0..FFT_WINDOW_SIZE {
            let phase = 2.0 * PI * 4.0 * (n as f32) / (FFT_WINDOW_SIZE as f32);
            let v = (phase.sin() * (i16::MAX as f32 * 0.8)) as i16;
            sine.push(v);
        }
        proc.push_samples(&sine);
        let levels = proc.compute_levels().expect("levels");
        let max_level = levels.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            max_level > 0.5,
            "expected a loud sine wave to light up some bin > 0.5, got levels = {levels:?}"
        );
    }

    #[test]
    fn waveform_payload_serializes_camel_case_array() {
        let payload = WaveformPayload {
            levels: [0.0, 0.25, 0.5, 0.75, 1.0, 0.5],
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        // `levels` is already lowercase so camelCase has no effect, but we
        // verify the payload shape the frontend type expects.
        assert_eq!(json, "{\"levels\":[0.0,0.25,0.5,0.75,1.0,0.5]}");
    }
}
