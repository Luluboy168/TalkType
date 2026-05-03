// Audio recorder plugin (M2 chunk 1).
//
// Public surface:
//
//   State         AudioRecorderState
//   Commands      start_recording, stop_recording,
//                 list_audio_input_devices, get_default_input_device_name,
//                 save_recording_file, read_recording_file,
//                 delete_all_recordings, cleanup_old_recordings
//
// Submodules:
//
//   error.rs   — `AudioRecorderError` (thiserror + manual flat-string Serialize)
//   stream.rs  — cpal device selection + sample-format dispatch
//   files.rs   — recordings/<id>.wav read/save/cleanup commands
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
//     to signal the recording thread to pause + drop the stream.
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
// so it shows up in any future log scrub. Phase 1 is Windows-only, but the
// pattern stays for Phase 2 macOS.

pub mod error;
pub mod files;
pub mod stream;

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use serde::Serialize;
use tauri::State;

pub use error::AudioRecorderError;
pub use stream::{AudioInputDeviceInfo, StartAck};
// NOTE: file-management Tauri commands live in `files.rs` and must be wired
// via their full module path (`audio_recorder::files::save_recording_file`,
// etc.) in `lib.rs`. Re-exporting them here would lose the auxiliary symbols
// that `#[tauri::command]` generates next to each function.

// ─── State ─────────────────────────────────────────────────────────────────

/// Live recording handle. Held inside `AudioRecorderState::recording` for the
/// lifetime of an active recording. Note that the cpal stream itself does NOT
/// live here — it's held entirely on the named `"audio-recorder"` thread.
pub struct RecordingHandle {
    pub thread: JoinHandle<()>,
    pub should_stop: Arc<AtomicBool>,
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

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Begin a new recording. Errors out if a recording is already in progress.
///
/// Spawns the `"audio-recorder"` thread that owns the cpal stream. Blocks on
/// the startup ack so the caller sees the same error domain regardless of
/// where setup failed.
#[tauri::command]
pub async fn start_recording(
    state: State<'_, AudioRecorderState>,
    device_name: Option<String>,
) -> Result<(), AudioRecorderError> {
    {
        let guard = state
            .recording
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        if guard.is_some() {
            return Err(AudioRecorderError::BuildStream(
                "recording already in progress".to_string(),
            ));
        }
    }

    let should_stop = Arc::new(AtomicBool::new(false));
    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));

    let (ack_tx, ack_rx) = mpsc::channel::<StartAck>();

    let stop_flag = should_stop.clone();
    let samples_for_thread = samples.clone();
    let device_name_owned = device_name.clone();

    let thread_handle = thread::Builder::new()
        .name("audio-recorder".to_string())
        .spawn(move || {
            stream::run_recording_thread(
                device_name_owned,
                samples_for_thread,
                stop_flag,
                ack_tx,
            );
        })
        .map_err(|e| AudioRecorderError::BuildStream(format!("spawn audio-recorder thread: {e}")))?;

    // Wait for the recording thread to either start the stream or error out.
    let sample_rate = match ack_rx.recv() {
        Ok(Ok(rate)) => rate,
        Ok(Err(err)) => {
            // Thread already returned; join to surface any panic.
            let _ = thread_handle.join();
            return Err(err);
        }
        Err(e) => {
            // Sender dropped without sending — thread panicked before play().
            let _ = thread_handle.join();
            return Err(AudioRecorderError::BuildStream(format!(
                "audio-recorder thread closed channel before ack: {e}"
            )));
        }
    };

    // Reserve ~30 s of mono i16 at the negotiated rate. Done from the command
    // thread (rather than in the cpal callback) to keep the audio callback
    // off the allocator hot path. Lock contention is fine here — cpal hasn't
    // produced data yet.
    if let Ok(mut guard) = samples.lock() {
        guard.reserve((sample_rate as usize) * 30);
    }

    let handle = RecordingHandle {
        thread: thread_handle,
        should_stop,
        samples,
        sample_rate,
        started_at: Instant::now(),
    };

    let mut guard = state
        .recording
        .lock()
        .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
    *guard = Some(handle);
    Ok(())
}

/// Stop the current recording, encode the buffered samples to a WAV byte
/// vector stashed in `state.wav_buffer`, and return summary stats.
#[tauri::command]
pub async fn stop_recording(
    state: State<'_, AudioRecorderState>,
) -> Result<StopRecordingResult, AudioRecorderError> {
    // Take ownership of the active recording handle.
    let handle = {
        let mut guard = state
            .recording
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        guard.take().ok_or(AudioRecorderError::NotRecording)?
    };

    // Signal the audio thread to pause + drop the stream.
    handle.should_stop.store(true, Ordering::SeqCst);

    // Wait for the audio thread to finish so the cpal stream is fully torn
    // down before we read the sample buffer.
    if let Err(e) = handle.thread.join() {
        eprintln!("[audio-recorder] audio-recorder thread panicked: {e:?}");
    }

    // Snapshot samples and compute duration.
    let samples = match handle.samples.lock() {
        Ok(g) => g.clone(),
        Err(e) => return Err(AudioRecorderError::LockPoisoned(e.to_string())),
    };
    let duration_ms = handle.started_at.elapsed().as_millis() as u64;

    let peak = compute_peak(&samples);
    let rms = compute_rms(&samples);
    let sample_count = samples.len();

    let wav_bytes = encode_wav(&samples, handle.sample_rate)?;

    {
        let mut guard = state
            .wav_buffer
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        *guard = Some(wav_bytes);
    }

    eprintln!(
        "[audio-recorder] stopped: duration_ms={duration_ms} sample_count={sample_count} peak={peak:.4} rms={rms:.4}"
    );

    Ok(StopRecordingResult {
        duration_ms,
        peak_energy_level: peak,
        rms_energy_level: rms,
        sample_count,
    })
}

/// Enumerate input devices on the default cpal host.
#[tauri::command]
pub async fn list_audio_input_devices() -> Result<Vec<AudioInputDeviceInfo>, AudioRecorderError> {
    stream::list_input_devices()
}

/// Return the default input device name, or `None` if there is no default
/// input device.
#[tauri::command]
pub async fn get_default_input_device_name() -> Result<Option<String>, AudioRecorderError> {
    Ok(stream::default_input_device_name())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Peak-amplitude energy on the buffered samples, in [0.0, 1.0].
fn compute_peak(samples: &[i16]) -> f32 {
    samples
        .iter()
        .map(|s| (*s as f32 / i16::MAX as f32).abs())
        .fold(0.0_f32, f32::max)
}

/// RMS-amplitude energy on the buffered samples, in [0.0, 1.0].
fn compute_rms(samples: &[i16]) -> f32 {
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
fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, AudioRecorderError> {
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
}
