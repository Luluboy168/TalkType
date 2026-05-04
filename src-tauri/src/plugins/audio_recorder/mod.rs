// Audio recorder plugin (M2 + M3 chunk-0).
//
// Public surface:
//
//   State         AudioRecorderState
//   Commands      start_recording, stop_recording, clear_recording_buffer
//                 list_audio_input_devices, get_default_input_device_name,
//                 save_recording_file, read_recording_file, delete_recording,
//                 delete_all_recordings, cleanup_old_recordings
//
// Submodules:
//
//   error.rs            — `AudioRecorderError` (thiserror + manual flat-string Serialize)
//   events.rs           — `RecordingAbortedPayload` / `MicSafetyPayload` + emit helpers
//   stream.rs           — cpal device selection + sample-format dispatch
//   recording_thread.rs — body of the named "audio-recorder" thread
//   preview.rs          — independent mic preview path
//   waveform.rs         — FFT 6-band processor
//   files.rs            — recordings/<id>.wav read/save/cleanup commands
//   commands.rs         — top-level Tauri commands (start/stop/list/default)
//
// **Threading model**
//
// `cpal::Stream` is `!Send + !Sync` (cpal 0.15 marks every stream with
// `NotSendSyncAcrossAllPlatforms`), so we cannot stash it inside a
// `tauri::State` slot — `State<T>` requires `T: Send + Sync`. Instead, the
// stream is built and held entirely on a dedicated `"audio-recorder"`
// `std::thread`, the same pattern SayIt uses.
//
// Communication between the Tauri command thread and the recording thread:
//
//   * `should_stop: Arc<AtomicBool>` — flipped to true by `stop_recording`
//     to signal the recording thread to pause + drop the stream. Also flipped
//     by the recording thread itself when it aborts on `MAX_WAV_BYTES` or a
//     mic-disconnect (M3 chunk 0).
//   * `mic_disconnected: Arc<AtomicBool>` — set by the cpal `err_fn` when a
//     stream-level error fires. The recording thread polls this alongside
//     `should_stop` and emits `audio:recording-aborted { reason: 'mic_unplug' }`
//     before tearing down (M2 retro: mic-unplug detection).
//   * `samples: Arc<Mutex<Vec<i16>>>` — the cpal callback pushes mono i16
//     samples here; `stop_recording` snapshots them after thread join.
//   * `mpsc::channel<StartAck>` — a single-shot ack from the recording
//     thread reporting either successful stream startup (with the negotiated
//     sample rate) or a setup error. `start_recording` blocks on this so
//     callers see the same error-domain whether the failure is on the
//     command thread or the audio thread.
//
// **Mic safety contract**: the recording thread ALWAYS calls
// `stream.pause()` before dropping the cpal `Stream`. cpal 0.15.x has an
// Arc-cycle bug on macOS where dropping the stream alone may not call
// `AudioOutputUnitStop` — leaving the mic active after the user thinks
// recording stopped. If `pause()` returns `Err`, we log a `SECURITY:` line
// AND emit `audio:mic-safety-warning` to the frontend (eprintln! is
// invisible in release builds — M2 retro #2). Phase 1 is Windows-only, but
// the pattern stays for Phase 2 macOS.
//
// **WAV size cap**: the recording thread monitors the i16 buffer size and
// auto-aborts when it would produce a WAV larger than `MAX_WAV_BYTES`. The
// constant doubles as M3's Groq upload cap (matches OpenAI Whisper API
// limit) and as an OOM defense (M2 retro #1) so a forgotten Toggle-mode
// session can't grow unbounded.

pub mod commands;
pub mod error;
pub mod events;
pub mod files;
pub mod preview;
pub mod recording_thread;
pub mod stream;
pub mod waveform;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;

pub use error::AudioRecorderError;
pub use preview::AudioPreviewState;
pub use stream::{AudioInputDeviceInfo, StartAck};
// NOTE: Tauri commands are NOT `pub use`d up — `#[tauri::command]` generates
// auxiliary symbols (`__cmd__*`, `__tauri_command_name_*`) sibling to each
// command function that a `pub use` does NOT pull along. `lib.rs` wires
// each command via its full module path (`audio_recorder::commands::*`,
// `audio_recorder::files::*`, `audio_recorder::preview::*`).

// ─── Constants ─────────────────────────────────────────────────────────────

/// Hard cap on the encoded WAV size for a single recording. 25 MB matches
/// Groq's `audio.transcriptions` upload cap (and OpenAI Whisper API), and
/// also serves as an OOM defense if the user forgets a Toggle-mode session
/// (M2 retro #1).
///
/// Computed against the i16 sample buffer (`samples.len() * 2`) — the WAV
/// header is a tiny ~44-byte fixed cost and we always abort *before* the
/// cap so the encoded WAV never exceeds it.
///
/// At 16 kHz mono i16 this is ~13 minutes of audio. The recording thread
/// monitors `samples.len() * 2` against this and emits
/// `audio:recording-aborted { reason: 'max_size' }` when reached.
pub const MAX_WAV_BYTES: usize = 25_000_000;

/// Bytes per i16 sample. Used by the recording-thread size monitor.
pub(crate) const BYTES_PER_SAMPLE: usize = 2;

// ─── State ─────────────────────────────────────────────────────────────────

/// Live recording handle. Held inside `AudioRecorderState::recording` for the
/// lifetime of an active recording. Note that the cpal stream itself does NOT
/// live here — it's held entirely on the named `"audio-recorder"` thread.
pub struct RecordingHandle {
    pub thread: JoinHandle<()>,
    pub should_stop: Arc<AtomicBool>,
    pub mic_disconnected: Arc<AtomicBool>,
    pub samples: Arc<Mutex<Vec<i16>>>,
    /// Negotiated stream sample rate (after `determine_input_config`). Drives
    /// the WAV header on stop.
    pub sample_rate: u32,
    /// `Instant` when recording started; used to compute duration on stop.
    pub started_at: Instant,
}

/// Application-managed recorder state. Single `Mutex<Option<...>>` for the
/// live recording handle plus a separate buffer for the encoded WAV bytes
/// that survives `stop_recording` until the next consumer (transcribe in M3,
/// or `save_recording_file` here in M2) reads it.
pub struct AudioRecorderState {
    pub recording: Mutex<Option<RecordingHandle>>,
    pub wav_buffer: Mutex<Option<Vec<u8>>>,
}

impl AudioRecorderState {
    pub fn new() -> Self {
        Self {
            recording: Mutex::new(None),
            wav_buffer: Mutex::new(None),
        }
    }

    /// Take ownership of the buffered WAV bytes, leaving `None` behind.
    /// Designed for M3's `transcribe_cloud` which is the WAV's last consumer
    /// — `save_recording_file` (which may run before transcribe) still
    /// `clone()`s instead.
    ///
    /// Returns `Ok(None)` if no WAV is buffered.
    pub fn consume_wav_buffer(&self) -> Result<Option<Vec<u8>>, AudioRecorderError> {
        self.wav_buffer
            .lock()
            .map(|mut guard| guard.take())
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))
    }
}

impl Default for AudioRecorderState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result returned by `stop_recording`. Mirrors the shape SayIt uses so M3
/// transcribe + M5 HUD UX can plug in without churn.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StopRecordingResult {
    pub duration_ms: u64,
    pub peak_energy_level: f32,
    pub rms_energy_level: f32,
    pub sample_count: usize,
}

// ─── Helpers (used by `commands.rs`) ───────────────────────────────────────

/// Peak-amplitude energy on the buffered samples, in [0.0, 1.0].
pub(crate) fn compute_peak(samples: &[i16]) -> f32 {
    samples
        .iter()
        .map(|s| (*s as f32 / i16::MAX as f32).abs())
        .fold(0.0_f32, f32::max)
}

/// RMS-amplitude energy on the buffered samples, in [0.0, 1.0].
pub(crate) fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = samples
        .iter()
        .map(|s| {
            let f = *s as f32 / i16::MAX as f32;
            f * f
        })
        .sum();
    (sum_squares / samples.len() as f32).sqrt()
}

/// Encode a buffered mono `i16` sample slice into a WAV byte vector.
///
/// WAV header is 1 channel, 16-bit signed PCM, sample rate as supplied. We
/// always write a mono file because the cpal callback already averages
/// multi-channel input down to a single channel before pushing onto the
/// shared buffer.
///
/// **Note**: synchronous; callers run this inside `tokio::task::spawn_blocking`
/// so a 30 min / ~115 MB recording doesn't block the tokio runtime (M2 retro
/// perf finding).
pub(crate) fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, AudioRecorderError> {
    use std::io::Cursor;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| AudioRecorderError::WavEncode(e.to_string()))?;
        for &sample in samples {
            writer
                .write_sample(sample)
                .map_err(|e| AudioRecorderError::WavEncode(e.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|e| AudioRecorderError::WavEncode(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_peak_zero_samples_is_zero() {
        assert_eq!(compute_peak(&[]), 0.0);
    }

    #[test]
    fn compute_peak_max_amplitude_is_one() {
        let samples = [0i16, i16::MAX, -i16::MAX / 2, 100];
        let peak = compute_peak(&samples);
        assert!((peak - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_peak_negative_only_still_returns_positive() {
        let samples = [-10_000i16, -20_000, -i16::MAX];
        let peak = compute_peak(&samples);
        assert!((peak - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_rms_zero_samples_is_zero() {
        assert_eq!(compute_rms(&[]), 0.0);
    }

    #[test]
    fn compute_rms_constant_amplitude_matches_amplitude() {
        // All samples at half-scale → RMS = 0.5.
        let half = i16::MAX / 2;
        let samples = vec![half; 1024];
        let rms = compute_rms(&samples);
        // Slight off-by-one because i16::MAX / 2 isn't exactly half of i16::MAX.
        assert!((rms - 0.5).abs() < 0.001, "rms = {rms}");
    }

    #[test]
    fn compute_rms_is_lower_than_or_equal_to_peak() {
        let samples = [0i16, i16::MAX, 0, -i16::MAX, 0, i16::MAX, 0, -i16::MAX];
        let peak = compute_peak(&samples);
        let rms = compute_rms(&samples);
        assert!(rms <= peak + f32::EPSILON);
    }

    #[test]
    fn encode_wav_writes_riff_wave_header() {
        let samples = vec![0i16; 16_000]; // 1 second of silence at 16 kHz
        let bytes = encode_wav(&samples, 16_000).expect("encode");
        // WAV header is 44 bytes; followed by sample data.
        assert!(bytes.len() >= 44, "header too short: {}", bytes.len());
        // RIFF
        assert_eq!(&bytes[0..4], b"RIFF");
        // WAVE
        assert_eq!(&bytes[8..12], b"WAVE");
        // fmt subchunk header
        assert_eq!(&bytes[12..16], b"fmt ");
        // 1 channel (bytes 22..24, little-endian u16)
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1);
        // 16 kHz sample rate (bytes 24..28, little-endian u32)
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            16_000
        );
        // 16 bits per sample (bytes 34..36, little-endian u16)
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16);
        // data subchunk header
        assert_eq!(&bytes[36..40], b"data");
        // Sample bytes = 2 bytes per sample × 16_000 samples = 32_000 bytes
        let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(data_size, 32_000);
    }

    #[test]
    fn encode_wav_empty_produces_minimum_header() {
        let bytes = encode_wav(&[], 16_000).expect("encode");
        assert!(bytes.len() >= 44);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        // data size is 0 for an empty recording.
        let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(data_size, 0);
    }

    #[test]
    fn consume_wav_buffer_takes_ownership() {
        let state = AudioRecorderState::new();
        {
            let mut guard = state.wav_buffer.lock().expect("lock");
            *guard = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        }
        let taken = state.consume_wav_buffer().expect("consume");
        assert_eq!(taken, Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        // Second consume returns None.
        let again = state.consume_wav_buffer().expect("consume");
        assert_eq!(again, None);
    }

    #[test]
    fn consume_wav_buffer_when_empty_is_none() {
        let state = AudioRecorderState::new();
        let taken = state.consume_wav_buffer().expect("consume");
        assert_eq!(taken, None);
    }

    #[test]
    fn max_wav_bytes_matches_groq_cap() {
        // Sanity: a 16 kHz mono i16 recording at MAX_WAV_BYTES is ~13 min.
        // Documented in module comment + 02-implementation-roadmap.md M3.
        let max_samples = MAX_WAV_BYTES / BYTES_PER_SAMPLE;
        let max_seconds = max_samples / 16_000;
        assert_eq!(MAX_WAV_BYTES, 25_000_000);
        assert!(max_seconds >= 13 * 60);
    }
}
