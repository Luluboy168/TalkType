// Audio mic preview path — independent of recording.
//
// `start_audio_preview` lights up a cpal input stream, computes RMS over the
// samples accumulated since the previous tick (~PREVIEW_TICK = 30 ms cadence),
// and emits `audio:preview-level` events while the preview thread is alive.
// `stop_audio_preview` flips an `AtomicBool`,
// joins the thread, and explicitly `pause()`s the cpal stream BEFORE drop
// (mic-safety contract — same SECURITY: log line as the recording path).
//
// The preview is NOT shared with recording state. It's spun up on the
// Settings page when the user opens the mic picker; recording uses its own
// `AudioRecorderState`. If the caller starts a preview while one is already
// running we surface `BuildStream("audio preview already running")` rather
// than auto-stopping — the UI can stop-then-start explicitly when switching
// devices.
//
// Threading: same pattern as recording.
//   * Named thread `"audio-preview"`.
//   * `AtomicBool` stop flag (set by `stop_audio_preview`).
//   * `mpsc::Sender<Result<(), AudioRecorderError>>` single-shot ack so the
//     `start` call returns the same error domain whether the failure was on
//     the command thread or the audio thread.
//   * cpal stream is built and held entirely on the audio thread (cpal's
//     `Stream` is `!Send + !Sync`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::AudioRecorderError;
use super::events::emit_mic_safety_warning;
use super::stream::{
    determine_input_config, dispatch_sample_format_with_callback, select_input_device,
};

/// Source tag used in eprintln + emit helpers so log scrubs find both sides.
const SOURCE_TAG: &str = "[audio-preview]";

/// Tauri event name for the per-30-ms RMS preview level.
const EVENT_PREVIEW_LEVEL: &str = "audio:preview-level";

/// Target preview tick. ~33 fps. SayIt uses 30 ms.
const PREVIEW_TICK: Duration = Duration::from_millis(30);

/// Capacity of the recent-samples ring buffer. Sized so 30 ms of audio at
/// 48 kHz mono (~1440 samples) fits comfortably; lower-rate devices use less.
const PREVIEW_RING_CAPACITY: usize = 4096;

/// Payload for the `audio:preview-level` Tauri event.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AudioPreviewLevelPayload {
    /// RMS amplitude over the samples accumulated since the previous tick
    /// (~PREVIEW_TICK = 30 ms), in `[0.0, 1.0]`.
    pub level: f32,
}

/// Live preview thread handle.
struct PreviewHandle {
    join: JoinHandle<()>,
    should_stop: Arc<AtomicBool>,
}

/// Application-managed state for the preview path. Independent of
/// `AudioRecorderState` — recording and preview are separate flows.
pub struct AudioPreviewState {
    handle: Mutex<Option<PreviewHandle>>,
}

impl AudioPreviewState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
        }
    }
}

impl Default for AudioPreviewState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Start the mic preview. Errors out if a preview is already running.
///
/// Spawns the named `"audio-preview"` thread and blocks on a single-shot
/// startup ack — failures inside the audio thread (no input device, build
/// stream error, play error) propagate to the caller as
/// `AudioRecorderError`.
#[tauri::command]
pub async fn start_audio_preview(
    app: AppHandle,
    state: State<'_, AudioPreviewState>,
    device_name: Option<String>,
) -> Result<(), AudioRecorderError> {
    {
        let guard = state
            .handle
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        if guard.is_some() {
            return Err(AudioRecorderError::BuildStream(
                "audio preview already running".to_string(),
            ));
        }
    }

    let should_stop = Arc::new(AtomicBool::new(false));
    let (ack_tx, ack_rx) = mpsc::channel::<Result<(), AudioRecorderError>>();

    let stop_flag = should_stop.clone();
    let app_for_thread = app.clone();
    let device_name_owned = device_name.clone();

    let thread_handle = thread::Builder::new()
        .name("audio-preview".to_string())
        .spawn(move || {
            run_preview_thread(app_for_thread, device_name_owned, stop_flag, ack_tx);
        })
        .map_err(|e| AudioRecorderError::BuildStream(format!("spawn audio-preview thread: {e}")))?;

    match ack_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            let _ = thread_handle.join();
            return Err(err);
        }
        Err(e) => {
            let _ = thread_handle.join();
            return Err(AudioRecorderError::BuildStream(format!(
                "audio-preview thread closed channel before ack: {e}"
            )));
        }
    }

    let mut guard = state
        .handle
        .lock()
        .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
    *guard = Some(PreviewHandle {
        join: thread_handle,
        should_stop,
    });
    Ok(())
}

/// Stop the running preview. No-op if no preview is active.
#[tauri::command]
pub async fn stop_audio_preview(
    state: State<'_, AudioPreviewState>,
) -> Result<(), AudioRecorderError> {
    let handle = {
        let mut guard = state
            .handle
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        guard.take()
    };

    let Some(handle) = handle else {
        return Ok(());
    };

    handle.should_stop.store(true, Ordering::SeqCst);
    if let Err(e) = handle.join.join() {
        eprintln!("[audio-preview] preview thread panicked: {e:?}");
    }
    Ok(())
}

// ─── Thread body ───────────────────────────────────────────────────────────

fn run_preview_thread(
    app: AppHandle,
    device_name: Option<String>,
    should_stop: Arc<AtomicBool>,
    ack: mpsc::Sender<Result<(), AudioRecorderError>>,
) {
    let host = cpal::default_host();

    let device = match select_input_device(&host, device_name.as_deref()) {
        Ok(d) => d,
        Err(e) => {
            let _ = ack.send(Err(e));
            return;
        }
    };

    let supported = match determine_input_config(&device) {
        Ok(c) => c,
        Err(e) => {
            let _ = ack.send(Err(e));
            return;
        }
    };

    eprintln!(
        "[audio-preview] thread starting: device={:?} channels={} sample_rate={} sample_format={:?}",
        device.name().ok(),
        supported.config().channels,
        supported.config().sample_rate.0,
        supported.sample_format()
    );

    // Shared ring buffer of recent mono `i16` samples. cpal callback pushes
    // here; the tick loop drains it to compute RMS. Bounded so a paused
    // tick loop doesn't grow memory unboundedly.
    let ring: Arc<Mutex<VecDeque<i16>>> = Arc::new(Mutex::new(VecDeque::with_capacity(
        PREVIEW_RING_CAPACITY,
    )));

    let ring_for_callback = ring.clone();
    let cpal_stream = match dispatch_sample_format_with_callback(
        &device,
        &supported,
        move |samples: &[i16]| {
            let Ok(mut guard) = ring_for_callback.lock() else {
                eprintln!("[audio-preview] ring lock poisoned in callback");
                return;
            };
            for &s in samples {
                if guard.len() == PREVIEW_RING_CAPACITY {
                    guard.pop_front();
                }
                guard.push_back(s);
            }
        },
    ) {
        Ok(s) => s,
        Err(e) => {
            let _ = ack.send(Err(e));
            return;
        }
    };

    if let Err(e) = cpal_stream.play() {
        let _ = ack.send(Err(AudioRecorderError::PlayStream(e.to_string())));
        return;
    }

    if ack.send(Ok(())).is_err() {
        // Caller went away before ack — tear down immediately.
        eprintln!("{SOURCE_TAG} caller dropped ack rx; aborting preview");
        if let Err(e) = cpal_stream.pause() {
            emit_mic_safety_warning(&app, SOURCE_TAG, e.to_string());
        }
        drop(cpal_stream);
        return;
    }

    // RMS tick loop.
    let mut next_tick = Instant::now() + PREVIEW_TICK;
    while !should_stop.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now < next_tick {
            let remaining = next_tick - now;
            thread::sleep(remaining.min(PREVIEW_TICK));
            continue;
        }

        // Drain and compute RMS over whatever the callback has accumulated.
        let level = match ring.lock() {
            Ok(mut guard) => {
                let level = compute_rms(guard.iter().copied());
                guard.clear();
                level
            }
            Err(e) => {
                eprintln!("[audio-preview] ring lock poisoned in tick: {e}");
                break;
            }
        };

        let payload = AudioPreviewLevelPayload { level };
        if let Err(e) = app.emit(EVENT_PREVIEW_LEVEL, payload) {
            eprintln!("[audio-preview] failed to emit {EVENT_PREVIEW_LEVEL}: {e}");
        }

        let now2 = Instant::now();
        next_tick = if now2 > next_tick + PREVIEW_TICK {
            now2 + PREVIEW_TICK
        } else {
            next_tick + PREVIEW_TICK
        };
    }

    // Mic-safety contract.
    if let Err(e) = cpal_stream.pause() {
        emit_mic_safety_warning(&app, SOURCE_TAG, e.to_string());
    }
    drop(cpal_stream);
}

/// RMS over an iterator of `i16` samples, normalized to `[0.0, 1.0]`.
///
/// Returns 0.0 when the iterator is empty (e.g. tick happened before the
/// callback delivered any data).
fn compute_rms<I: IntoIterator<Item = i16>>(samples: I) -> f32 {
    let mut count = 0usize;
    let mut sum_squares = 0.0_f32;
    for s in samples {
        let f = (s as f32) / (i16::MAX as f32);
        sum_squares += f * f;
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    (sum_squares / count as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_rms_empty_is_zero() {
        let v: Vec<i16> = Vec::new();
        assert_eq!(compute_rms(v), 0.0);
    }

    #[test]
    fn compute_rms_silence_is_zero() {
        let v = vec![0_i16; 256];
        assert_eq!(compute_rms(v), 0.0);
    }

    #[test]
    fn compute_rms_constant_half_amplitude() {
        // Half-scale → RMS ≈ 0.5
        let half = i16::MAX / 2;
        let v = vec![half; 1024];
        let rms = compute_rms(v);
        assert!((rms - 0.5).abs() < 0.001, "rms = {rms}");
    }

    #[test]
    fn compute_rms_full_scale_is_one() {
        let v = vec![i16::MAX; 256];
        let rms = compute_rms(v);
        assert!((rms - 1.0).abs() < 0.001, "rms = {rms}");
    }

    #[test]
    fn preview_level_payload_serializes_camel_case() {
        let p = AudioPreviewLevelPayload { level: 0.42 };
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "{\"level\":0.42}");
    }

    #[test]
    fn audio_preview_state_starts_empty() {
        let state = AudioPreviewState::new();
        let guard = state.handle.lock().expect("lock");
        assert!(guard.is_none());
    }
}
