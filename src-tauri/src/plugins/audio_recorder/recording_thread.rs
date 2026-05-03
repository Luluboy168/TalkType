// Body of the named `"audio-recorder"` thread.
//
// Responsibilities (chunk 2):
//   * Build the cpal stream on this thread (it's `!Send + !Sync`).
//   * Ack startup back to the Tauri command thread via a single-shot mpsc.
//   * Drive a 16 ms FFT tick loop that emits `audio:waveform` events.
//   * Honour the mic-safety contract: `pause()` BEFORE drop and log
//     `SECURITY:` if the pause fails.
//
// **Single-buffer + cursor design**
//
// The cpal callback pushes every mono `i16` sample to the recording
// `samples` buffer. On each tick this thread acquires the `samples` lock
// briefly, copies the slice from `last_processed_idx..`, advances the
// cursor, and feeds it to a local `WaveformProcessor`. Once the FFT ring is
// full the tick emits an `audio:waveform` event.
//
// Why single-buffer instead of a side ring: the cpal callback already
// holds the `samples` lock to push data; adding a second mutex would
// double the contention on the audio-critical path. With a cursor we add
// zero extra locks to the callback, and the FFT tick only needs the lock
// long enough to do a slice copy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use tauri::{AppHandle, Emitter};

use super::error::AudioRecorderError;
use super::stream::{
    determine_input_config, dispatch_sample_format, select_input_device, StartAck,
};
use super::waveform::{WaveformPayload, WaveformProcessor};

/// Tauri event name for the FFT 6-band waveform payload.
const EVENT_WAVEFORM: &str = "audio:waveform";

/// Target FFT tick interval. ~60 fps. Tighter would over-emit; SayIt uses 16 ms.
const WAVEFORM_TICK: Duration = Duration::from_millis(16);

/// Body of the named `"audio-recorder"` thread.
///
/// `ack` is a single-shot mpsc sender used to report startup status back to
/// the Tauri command thread:
///   * `Ok(sample_rate)` once the stream is playing.
///   * `Err(AudioRecorderError)` if any setup step failed.
pub fn run_recording_thread(
    app: AppHandle,
    device_name: Option<String>,
    samples: Arc<Mutex<Vec<i16>>>,
    should_stop: Arc<AtomicBool>,
    ack: mpsc::Sender<StartAck>,
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
    let stream_config = supported.config();
    let sample_rate = stream_config.sample_rate.0;

    eprintln!(
        "[audio-recorder] thread starting: device={:?} channels={} sample_rate={} sample_format={:?}",
        device.name().ok(),
        stream_config.channels,
        sample_rate,
        supported.sample_format()
    );

    let cpal_stream = match dispatch_sample_format(&device, &supported, samples.clone()) {
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

    // Stream is live — ack the caller with the negotiated sample rate.
    if ack.send(Ok(sample_rate)).is_err() {
        // Caller went away; tear the stream down immediately.
        eprintln!("[audio-recorder] caller dropped ack rx; aborting recording");
        if let Err(e) = cpal_stream.pause() {
            eprintln!(
                "[audio-recorder] SECURITY: stream.pause() failed — mic may still be active: {e}"
            );
        }
        drop(cpal_stream);
        return;
    }

    // FFT driver loop. See module-level comment for the single-buffer + cursor
    // design rationale.
    let mut processor = WaveformProcessor::new();
    let mut last_processed_idx: usize = 0;
    // Reusable scratch — sized for one tick worth at 48 kHz mono = ~768 samples.
    let mut new_samples: Vec<i16> = Vec::with_capacity(2048);
    let mut next_tick = Instant::now() + WAVEFORM_TICK;

    while !should_stop.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now < next_tick {
            // Sleep up to next tick. Cap individual sleeps so we react to
            // `should_stop` within ~16 ms even if next_tick is much further out.
            let remaining = next_tick - now;
            thread::sleep(remaining.min(WAVEFORM_TICK));
            continue;
        }

        // Grab any new samples the cpal callback has pushed since the last tick.
        new_samples.clear();
        match samples.lock() {
            Ok(guard) => {
                if guard.len() > last_processed_idx {
                    new_samples.extend_from_slice(&guard[last_processed_idx..]);
                    last_processed_idx = guard.len();
                }
            }
            Err(e) => {
                // Lock poisoned — sample buffer is in an unknown state. We
                // can't recover here without panicking the recording, so log
                // and break the tick loop. The stop teardown still runs.
                eprintln!("[audio-recorder] FFT tick: samples lock poisoned: {e}");
                break;
            }
        }

        if !new_samples.is_empty() {
            processor.push_samples(&new_samples);
            if let Some(levels) = processor.compute_levels() {
                let payload = WaveformPayload { levels };
                if let Err(e) = app.emit(EVENT_WAVEFORM, payload) {
                    // Don't kill the recording over an emit failure; just log.
                    eprintln!("[audio-recorder] failed to emit {EVENT_WAVEFORM}: {e}");
                }
            }
        }

        // Schedule next tick. If the tick itself overran (e.g. the lock was
        // contended) just emit at the next 16 ms boundary instead of trying
        // to "catch up" by double-ticking.
        let now2 = Instant::now();
        next_tick = if now2 > next_tick + WAVEFORM_TICK {
            now2 + WAVEFORM_TICK
        } else {
            next_tick + WAVEFORM_TICK
        };
    }

    // Mic-safety contract: pause BEFORE drop. cpal 0.15.x has an Arc-cycle
    // bug on macOS where dropping the stream alone may not call
    // `AudioOutputUnitStop`. If pause fails we continue (log SECURITY:) —
    // dropping is still better than panicking.
    if let Err(e) = cpal_stream.pause() {
        eprintln!(
            "[audio-recorder] SECURITY: stream.pause() failed — mic may still be active: {e}"
        );
    }
    drop(cpal_stream);
}
