// cpal stream construction for the audio recorder.
//
// Responsibilities:
//   * Enumerate input devices (`list_input_devices`).
//   * Resolve the user-selected device, with a workaround that prefers
//     `default_input_device()` whenever its name matches the requested device
//     (defends against the cpal 0.15.x macOS Arc-cycle bug — phase 1 ships on
//     Windows so it's defensive only, but we want the pattern in place for
//     Phase 2).
//   * Pick a stream config that prefers 16 kHz mono and falls back to the
//     device's `default_input_config()` when no 16 kHz path exists.
//   * Build a typed `cpal::Stream` for any of the 10 cpal sample formats
//     (`I8/I16/I32/I64/U8/U16/U32/U64/F32/F64`) and convert each callback's
//     samples into mono `i16` pushed onto a shared buffer.
//   * `run_recording_thread` — the named `"audio-recorder"` thread body that
//     owns a single cpal `Stream` for the lifetime of one recording.
//
// The cpal callback runs on cpal's internal audio thread (cpal 0.15 spawns
// one per stream). Because `cpal::Stream: !Send + !Sync`, we cannot stash the
// stream in a `tauri::State` slot — instead the stream lives entirely on the
// `"audio-recorder"` thread that this module owns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, SupportedStreamConfig};
use serde::Serialize;

use super::error::AudioRecorderError;

/// Single-shot ack from the recording thread reporting either a negotiated
/// sample rate (success) or an `AudioRecorderError` (setup failure).
pub type StartAck = Result<u32, AudioRecorderError>;

/// Lightweight description of an input device for the frontend device picker.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

// ─── Device enumeration ────────────────────────────────────────────────────

/// Return all input devices on the default cpal host. The default device
/// (when present) is flagged with `is_default = true`.
pub fn list_input_devices() -> Result<Vec<AudioInputDeviceInfo>, AudioRecorderError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let devices = host
        .input_devices()
        .map_err(|e| AudioRecorderError::InputConfig(e.to_string()))?;

    let mut out = Vec::new();
    for dev in devices {
        let name = match dev.name() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[audio-recorder] skipping device with unreadable name: {e}");
                continue;
            }
        };
        let is_default = default_name.as_deref() == Some(name.as_str());
        out.push(AudioInputDeviceInfo { name, is_default });
    }
    Ok(out)
}

/// Convenience helper for the `get_default_input_device_name` Tauri command.
pub fn default_input_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}

// ─── Device selection ──────────────────────────────────────────────────────

/// Pick a device for recording.
///
/// `requested_name = None` → use the default input device.
///
/// `requested_name = Some(name)` → if the default input device's name matches,
/// reuse the default device handle. Otherwise iterate `host.input_devices()`
/// looking for a name match. Fall back to the default device if no match is
/// found (so a stale settings entry doesn't permanently break recording).
///
/// **SayIt note**: cpal 0.15.x has a known Arc-cycle bug on macOS where
/// re-creating a `Device` for a name that matches the default leaks the
/// previous CoreAudio device. Prefer the default-device handle whenever
/// possible. Phase 1 is Windows-only so this is defensive code, but the
/// pattern is required for Phase 2 macOS.
pub fn select_input_device(
    host: &cpal::Host,
    requested_name: Option<&str>,
) -> Result<cpal::Device, AudioRecorderError> {
    let default_device = host.default_input_device();

    let Some(requested) = requested_name else {
        // No preference → use default.
        return default_device.ok_or(AudioRecorderError::NoInputDevice);
    };

    // If the requested name matches the default, reuse the default handle to
    // sidestep the macOS Arc-cycle bug.
    if let Some(default) = &default_device {
        if let Ok(default_name) = default.name() {
            if default_name == requested {
                return Ok(default.clone());
            }
        }
    }

    // Otherwise look the device up by name.
    let mut devices = host
        .input_devices()
        .map_err(|e| AudioRecorderError::InputConfig(e.to_string()))?;

    if let Some(found) = devices.find(|d| d.name().map(|n| n == requested).unwrap_or(false)) {
        return Ok(found);
    }

    // Fall back to the default if the named device isn't present (e.g. user
    // unplugged the mic since saving the setting).
    eprintln!(
        "[audio-recorder] requested input device '{requested}' not found; falling back to default"
    );
    default_device.ok_or(AudioRecorderError::NoInputDevice)
}

// ─── Stream config selection ───────────────────────────────────────────────

/// Pick a `SupportedStreamConfig` for the given input device.
///
/// Preference order (per SayIt + `doc/plans/05-data-model.md` 16 kHz mono):
///   1. A supported config range that contains 16 kHz mono → instantiate at
///      16 kHz mono.
///   2. Whichever config range scores best on (mono_penalty, channels) where
///      mono_penalty = 0 for 1 channel, 1 otherwise. Pick min-channel config
///      and clamp the sample rate to its `min_sample_rate()`.
///   3. Fall back to `default_input_config()`.
pub fn determine_input_config(
    device: &cpal::Device,
) -> Result<SupportedStreamConfig, AudioRecorderError> {
    let supported_iter = match device.supported_input_configs() {
        Ok(iter) => iter,
        Err(e) => {
            eprintln!(
                "[audio-recorder] supported_input_configs failed ({e}); falling back to default"
            );
            return device
                .default_input_config()
                .map_err(|e| AudioRecorderError::InputConfig(e.to_string()));
        }
    };
    let supported: Vec<_> = supported_iter.collect();

    // Pass 1: look for a range that contains 16 kHz mono.
    const TARGET_RATE: cpal::SampleRate = cpal::SampleRate(16_000);
    for cfg in &supported {
        if cfg.channels() == 1
            && cfg.min_sample_rate() <= TARGET_RATE
            && cfg.max_sample_rate() >= TARGET_RATE
        {
            return Ok((*cfg).with_sample_rate(TARGET_RATE));
        }
    }

    // Pass 2: prefer mono, fewest channels, then min sample rate of that range.
    if let Some(best) = supported
        .iter()
        .min_by_key(|cfg| (if cfg.channels() == 1 { 0u16 } else { 1 }, cfg.channels()))
    {
        let chosen_rate = best.min_sample_rate();
        return Ok((*best).with_sample_rate(chosen_rate));
    }

    // Pass 3: fall back to the device's default.
    device
        .default_input_config()
        .map_err(|e| AudioRecorderError::InputConfig(e.to_string()))
}

// ─── Stream construction ───────────────────────────────────────────────────

/// Build a typed cpal input stream that converts every callback into mono
/// `i16` and appends to `samples`.
///
/// This is monomorphized for each `cpal::SizedSample` type via
/// `dispatch_sample_format` below.
fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
) -> Result<cpal::Stream, AudioRecorderError>
where
    T: SizedSample + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("[audio-recorder] cpal stream error: {err}");

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _info: &cpal::InputCallbackInfo| {
                if data.is_empty() || channels == 0 {
                    return;
                }
                // Convert every frame to a mono f32, clamp, and push as i16.
                // Branching on `channels` is fine — cpal callbacks are not on
                // the hot path for a 16 kHz mic in the way SIMD audio DSP is.
                let mut guard = match samples.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("[audio-recorder] samples lock poisoned in callback: {e}");
                        return;
                    }
                };
                guard.reserve(data.len() / channels);
                for frame in data.chunks_exact(channels) {
                    let mut sum = 0.0_f32;
                    for sample in frame {
                        sum += f32::from_sample(*sample);
                    }
                    let mono = sum / channels as f32;
                    // Clamp into [-1.0, 1.0] before scaling.
                    let clamped = mono.clamp(-1.0, 1.0);
                    let scaled = (clamped * i16::MAX as f32) as i16;
                    guard.push(scaled);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| AudioRecorderError::BuildStream(e.to_string()))?;

    Ok(stream)
}

/// Build an input stream regardless of which `cpal::SampleFormat` the device
/// chose. Covers all 10 sample formats cpal currently supports.
pub fn dispatch_sample_format(
    device: &cpal::Device,
    supported: &SupportedStreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
) -> Result<cpal::Stream, AudioRecorderError> {
    let stream_config = supported.config();

    match supported.sample_format() {
        SampleFormat::I8 => build_input_stream::<i8>(device, &stream_config, samples),
        SampleFormat::I16 => build_input_stream::<i16>(device, &stream_config, samples),
        SampleFormat::I32 => build_input_stream::<i32>(device, &stream_config, samples),
        SampleFormat::I64 => build_input_stream::<i64>(device, &stream_config, samples),
        SampleFormat::U8 => build_input_stream::<u8>(device, &stream_config, samples),
        SampleFormat::U16 => build_input_stream::<u16>(device, &stream_config, samples),
        SampleFormat::U32 => build_input_stream::<u32>(device, &stream_config, samples),
        SampleFormat::U64 => build_input_stream::<u64>(device, &stream_config, samples),
        SampleFormat::F32 => build_input_stream::<f32>(device, &stream_config, samples),
        SampleFormat::F64 => build_input_stream::<f64>(device, &stream_config, samples),
        other => Err(AudioRecorderError::BuildStream(format!(
            "unsupported sample format: {other:?}"
        ))),
    }
}

// ─── Recording thread ──────────────────────────────────────────────────────

/// Body of the named `"audio-recorder"` thread. Builds the cpal stream on
/// this thread, plays it, parks until `should_stop` flips, then explicitly
/// `pause()`s the stream (mic-safety contract) and drops it.
///
/// `ack` is a single-shot mpsc sender used to report startup status back to
/// the Tauri command thread:
///   * `Ok(sample_rate)` once the stream is playing.
///   * `Err(AudioRecorderError)` if any setup step failed.
pub fn run_recording_thread(
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

    let cpal_stream = match dispatch_sample_format(&device, &supported, samples) {
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

    // Park until stop_recording flips the flag. cpal feeds the callback on
    // its own audio thread, so this thread just needs to keep `cpal_stream`
    // alive — we don't park forever, we busy-wait with a sleep so we react
    // to should_stop in time. 16 ms is enough for M2; chunk 2 will use a
    // tighter waveform-driven loop.
    while !should_stop.load(Ordering::SeqCst) {
        thread::sleep(std::time::Duration::from_millis(16));
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
