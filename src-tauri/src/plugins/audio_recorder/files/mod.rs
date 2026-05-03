// Recording-file management commands.
//
// Recordings live at `app_data_dir/recordings/<id>.wav`. On Windows that's
// `%APPDATA%\com.luluboy168.talktype\recordings\<id>.wav` per
// `doc/plans/05-data-model.md` "Audio File Storage".
//
// Lifecycle in M2:
//   `start_recording` → cpal stream → `Vec<i16>`
//   `stop_recording`  → encode WAV → stash in `state.wav_buffer`
//   `save_recording_file(id?)` → clone bytes from `state.wav_buffer` → write
//       `recordings/<id>.wav` → return relative path string
//   `delete_recording(id)` → remove a single `recordings/<id>.wav` (M3
//       chunk 0, M2 retro: previously only `delete_all_recordings` existed)
//
// The buffer remains in memory after `save_recording_file` (we clone, we
// don't `take()`) so M3's transcribe pipeline can still consume it via
// `consume_wav_buffer` / `clear_recording_buffer`. M3's transcribe is the
// designated last consumer; until then `clear_recording_buffer` is the
// frontend escape hatch (M2 retro #3 — RAM hygiene).

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use tauri::{ipc::Response, AppHandle, Manager, State};
use uuid::Uuid;

use super::error::AudioRecorderError;
use super::AudioRecorderState;

const RECORDINGS_SUBDIR: &str = "recordings";

/// Resolve the `recordings/` directory under the Tauri app data dir, creating
/// it if needed.
pub fn recordings_dir(app: &AppHandle) -> Result<PathBuf, AudioRecorderError> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| AudioRecorderError::FileIo(format!("app_data_dir unavailable: {e}")))?;
    let dir = base.join(RECORDINGS_SUBDIR);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| AudioRecorderError::FileIo(format!("create_dir_all failed: {e}")))?;
    }
    Ok(dir)
}

/// Validate a recording id. Phase 1 enforces "id MUST parse as a UUID" so
/// that no path-traversal payload (`../`, absolute paths, NUL bytes) can ever
/// resolve outside the `recordings/` directory regardless of caller.
fn validate_id(id: &str) -> Result<(), AudioRecorderError> {
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| AudioRecorderError::FileIo(format!("invalid recording id: {id}")))
}

/// Resolve a single recording file path under `recordings/<id>.wav`.
fn recording_path(app: &AppHandle, id: &str) -> Result<PathBuf, AudioRecorderError> {
    validate_id(id)?;
    let dir = recordings_dir(app)?;
    Ok(dir.join(format!("{id}.wav")))
}

// ─── Tauri commands ────────────────────────────────────────────────────────

/// Persist the currently buffered WAV bytes to `recordings/<id>.wav`.
///
/// `id`:
///   * `Some(id)` → use the provided ID (caller-generated, e.g. transcript UUID)
///   * `None`     → generate a fresh UUID v4
///
/// Returns the relative path string `"recordings/<id>.wav"` (forward slashes
/// for IPC consistency).
///
/// **Note**: this clones from `state.wav_buffer` and leaves the bytes in place
/// so that the future `transcribe_*` commands can still `take()` them. If a
/// caller needs a one-shot consumption, M3 will add a dedicated path.
#[tauri::command]
pub async fn save_recording_file(
    app: AppHandle,
    state: State<'_, AudioRecorderState>,
    id: Option<String>,
) -> Result<String, AudioRecorderError> {
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());

    // Snapshot the buffer (clone, do not `take`).
    let bytes = {
        let guard = state
            .wav_buffer
            .lock()
            .map_err(|e| AudioRecorderError::LockPoisoned(e.to_string()))?;
        match guard.as_ref() {
            Some(b) => b.clone(),
            None => {
                return Err(AudioRecorderError::NotRecording);
            }
        }
    };

    let path = recording_path(&app, &id)?;
    fs::write(&path, &bytes).map_err(|e| AudioRecorderError::FileIo(e.to_string()))?;

    // Return forward-slash relative path for cross-platform IPC consistency.
    Ok(format!("{RECORDINGS_SUBDIR}/{id}.wav"))
}

/// Read a previously saved recording. Returns the raw WAV bytes via
/// `tauri::ipc::Response` so the frontend can wrap them in a `Blob`/object URL
/// without a JSON `number[]` round-trip on the slow path.
#[tauri::command]
pub async fn read_recording_file(
    app: AppHandle,
    id: String,
) -> Result<Response, AudioRecorderError> {
    let path = recording_path(&app, &id)?;
    if !path.exists() {
        return Err(AudioRecorderError::RecordingNotFound(id));
    }
    let bytes =
        fs::read(&path).map_err(|e| AudioRecorderError::FileIo(format!("read failed: {e}")))?;
    Ok(Response::new(bytes))
}

/// Delete a single recording by id. Returns `RecordingNotFound(id)` if the
/// file does not exist (rather than silently succeeding) so the UI can show
/// "already deleted" as a distinct outcome from "deleted now".
///
/// `id` MUST parse as a UUID — same path-traversal defense as
/// `read_recording_file`.
#[tauri::command]
pub async fn delete_recording(app: AppHandle, id: String) -> Result<(), AudioRecorderError> {
    let path = recording_path(&app, &id)?;
    if !path.exists() {
        return Err(AudioRecorderError::RecordingNotFound(id));
    }
    fs::remove_file(&path).map_err(|e| AudioRecorderError::FileIo(e.to_string()))?;
    Ok(())
}

/// Delete every `*.wav` file in the recordings directory and return the count
/// of files removed.
#[tauri::command]
pub async fn delete_all_recordings(app: AppHandle) -> Result<u32, AudioRecorderError> {
    let dir = recordings_dir(&app)?;
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(AudioRecorderError::FileIo(e.to_string())),
    };

    let mut count = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("wav") {
            match fs::remove_file(&path) {
                Ok(()) => count += 1,
                Err(e) => eprintln!(
                    "[audio-recorder] delete_all_recordings: failed to remove {}: {e}",
                    path.display()
                ),
            }
        }
    }
    Ok(count)
}

/// Delete WAV files older than `days`. Returns the list of deleted recording
/// IDs (file stems, without the `.wav` extension).
///
/// **Edge case (M2 retro)**: `days == 0` effectively means "delete every
/// file whose mtime is at or before now" — i.e. all of them, since the
/// cutoff equals `SystemTime::now()` and the comparison is `mtime > cutoff`
/// (strictly greater). This is surprising but matches the "older than zero
/// days" reading. If a caller wants explicit "delete all" semantics they
/// should call `delete_all_recordings` (which is independent of mtime).
///
/// FIXME (M9 polish): consider rejecting `days == 0` with `InvalidArg` or
/// requiring `days >= 1` to avoid surprises. For Phase 1 the behavior is
/// documented and the only call site (Settings auto-cleanup) feeds in
/// `auto_cleanup_recordings_days >= 1`.
#[tauri::command]
pub async fn cleanup_old_recordings(
    app: AppHandle,
    days: u32,
) -> Result<Vec<String>, AudioRecorderError> {
    let dir = recordings_dir(&app)?;
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AudioRecorderError::FileIo(e.to_string())),
    };

    let cutoff = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(u64::from(days) * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut deleted = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "[audio-recorder] cleanup_old_recordings: stat failed for {}: {e}",
                    path.display()
                );
                continue;
            }
        };
        if mtime > cutoff {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        match fs::remove_file(&path) {
            Ok(()) => deleted.push(id),
            Err(e) => eprintln!(
                "[audio-recorder] cleanup_old_recordings: remove failed for {}: {e}",
                path.display()
            ),
        }
    }
    Ok(deleted)
}

// ─── Internal helpers exposed for tests ────────────────────────────────────

/// Filesystem-only counterpart to `cleanup_old_recordings` that takes an
/// already-resolved `recordings/` directory. Lifted out so tempfile-based
/// integration tests can exercise the cleanup logic without a Tauri
/// `AppHandle` (which would need a full app context).
#[cfg(test)]
fn cleanup_old_recordings_in_dir(
    dir: &std::path::Path,
    days: u32,
) -> Result<Vec<String>, AudioRecorderError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AudioRecorderError::FileIo(e.to_string())),
    };

    let cutoff = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(u64::from(days) * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut deleted = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if mtime > cutoff {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if fs::remove_file(&path).is_ok() {
            deleted.push(id);
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests;

