// Top-level audio_recorder Tauri commands.
//
// Pulled out of `mod.rs` in M3 chunk 0 because M3 will add transcribe* +
// credentials commands that need `AudioRecorderState` access; mod.rs at 418
// lines was already over the 400-line budget. With the split:
//
//   * `mod.rs` keeps state structs, helpers (encode_wav / compute_*), and the
//     module wiring.
//   * `commands.rs` (this file) holds the 4 Tauri commands that don't fit
//     elsewhere: start_recording / stop_recording / list_audio_input_devices
//     / get_default_input_device_name, plus the new clear_recording_buffer
//     (M2 retro #3 — RAM hygiene helper).
//
// Preview commands stay in `preview.rs`; file-management commands stay in
// `files.rs`. Both already had their own modules pre-M3.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

use tauri::{AppHandle, State};

use super::error::AudioRecorderError;
use super::recording_thread;
use super::stream::{self, AudioInputDeviceInfo, StartAck};
use super::{
    compute_peak, compute_rms, encode_wav, AudioRecorderState, RecordingHandle, StopRecordingResult,
};

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Begin a new recording. Errors out if a recording is already in progress.
///
/// Spawns the `"audio-recorder"` thread that owns the cpal stream. Blocks on
/// the startup ack so the caller sees the same error domain regardless of
/// where setup failed.
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
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
    let mic_disconnected = Arc::new(AtomicBool::new(false));
    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));

    let (ack_tx, ack_rx) = mpsc::channel::<StartAck>();

    let stop_flag = should_stop.clone();
    let disconnect_flag = mic_disconnected.clone();
    let samples_for_thread = samples.clone();
    let device_name_owned = device_name.clone();
    let app_for_thread = app.clone();

    let thread_handle = thread::Builder::new()
        .name("audio-recorder".to_string())
        .spawn(move || {
            recording_thread::run_recording_thread(
                app_for_thread,
                device_name_owned,
                samples_for_thread,
                stop_flag,
                disconnect_flag,
                ack_tx,
            );
        })
        .map_err(|e| {
            AudioRecorderError::BuildStream(format!("spawn audio-recorder thread: {e}"))
        })?;

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
        mic_disconnected,
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
///
/// **Note**: works whether the user invoked stop manually OR the recording
/// thread auto-aborted (size cap / mic-unplug). In the auto-abort case the
/// thread has already flipped `should_stop` and torn down its cpal stream;
/// `stop_recording` then just joins, encodes, and returns. M3 chunk 1+
/// surfaces the abort reason via the `audio:recording-aborted` event.
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

    // Signal the audio thread to pause + drop the stream. (Idempotent:
    // recording_thread also sets this on auto-abort.)
    handle.should_stop.store(true, Ordering::SeqCst);

    // Wait for the audio thread to finish so the cpal stream is fully torn
    // down before we read the sample buffer.
    if let Err(e) = handle.thread.join() {
        eprintln!("[audio-recorder] audio-recorder thread panicked: {e:?}");
    }

    // Drain the sample buffer. Prefer `Arc::try_unwrap` (no clone — common
    // case post-join, since the recording thread is the only other Arc
    // holder and it has already returned). Fall back to `mem::take` on the
    // locked guard if a stray Arc reference is somehow still alive (e.g.
    // future plugin holds one).
    let samples = match Arc::try_unwrap(handle.samples) {
        Ok(mutex) => mutex
            .into_inner()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?,
        Err(arc) => {
            let mut guard = arc
                .lock()
                .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
            std::mem::take(&mut *guard)
        }
    };
    let duration_ms = handle.started_at.elapsed().as_millis() as u64;

    let peak = compute_peak(&samples);
    let rms = compute_rms(&samples);
    let sample_count = samples.len();

    // Encode the WAV off the tokio runtime — a 30 min recording is ~115 MB
    // and `hound::WavWriter` is fully synchronous (M2 retro perf finding).
    // Owned `samples` move into the closure so `Send` is satisfied.
    let sample_rate = handle.sample_rate;
    let wav_bytes = tokio::task::spawn_blocking(move || encode_wav(&samples, sample_rate))
        .await
        .map_err(|e| AudioRecorderError::WavEncode(format!("spawn_blocking join error: {e}")))??;

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

/// Drop any buffered WAV bytes from the most recent `stop_recording`. Called
/// by the frontend on `<AudioRecordTest>` unmount to avoid retaining a
/// possibly large recording in RAM after the user navigates away without
/// saving (M2 retro #3).
///
/// Idempotent — calling it when no buffer is present is fine.
#[tauri::command]
pub async fn clear_recording_buffer(
    state: State<'_, AudioRecorderState>,
) -> Result<(), AudioRecorderError> {
    let mut guard = state
        .wav_buffer
        .lock()
        .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
    *guard = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_recorder_state_starts_empty() {
        let state = AudioRecorderState::new();
        let recording = state.recording.lock().expect("lock");
        assert!(recording.is_none());
        let buffer = state.wav_buffer.lock().expect("lock");
        assert!(buffer.is_none());
    }
}
