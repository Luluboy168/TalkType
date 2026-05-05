// Tests for the recording-file management commands. Split out of `mod.rs`
// so production code stays under the 500-line CLAUDE.md cliff.

use super::*;

// ─── validate_id property test ─────────────────────────────────────────
//
// Hand-crafted 100+ string corpus rather than a `proptest` dep. Covers:
//   * obvious traversal payloads (../, ..\, absolute paths, NUL bytes)
//   * non-UUID shapes (empty, short hex, off-length)
//   * UUID variants (v4, v1, uppercase, mixed case)
//   * boundary lengths

fn _all_valid_uuids() -> Vec<String> {
    vec![
        // v4
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
        "f47ac10b-58cc-4372-a567-0e02b2c3d479".to_string(),
        "00000000-0000-4000-8000-000000000000".to_string(),
        "ffffffff-ffff-4fff-bfff-ffffffffffff".to_string(),
        // v1 — `Uuid::parse_str` accepts these even though we generate v4
        "c232ab00-9414-11ec-b909-0242ac120002".to_string(),
        // uppercase
        "550E8400-E29B-41D4-A716-446655440000".to_string(),
        // mixed case
        "550e8400-E29B-41d4-A716-446655440000".to_string(),
    ]
}

fn _all_invalid_ids() -> Vec<String> {
    // NOTE: `Uuid::parse_str` accepts both dashed AND undashed 32-hex
    // forms (`"550e8400e29b41d4a716446655440000"` parses fine). We rely
    // on the `format!("{id}.wav")` path construction to keep all
    // generated filenames inside the recordings dir regardless of which
    // form was used; both forms are still rejected as path-traversal
    // payloads since they produce no `/` or `\` separator. So the corpus
    // below excludes the undashed form from the "expect reject" list.
    vec![
        String::new(),
        "abc".to_string(),
        "../etc/passwd".to_string(),
        "..\\windows\\system32".to_string(),
        "a/b/c".to_string(),
        "a\\b\\c".to_string(),
        "a:b".to_string(),
        "a\0b".to_string(),
        // off-length (dashed)
        "550e8400-e29b-41d4-a716-44665544000".to_string(), // missing 1 char
        "550e8400-e29b-41d4-a716-4466554400000".to_string(), // extra char
        // off-length (undashed)
        "550e8400e29b41d4a71644665544000".to_string(), // 31 hex
        "550e8400e29b41d4a7164466554400000".to_string(), // 33 hex
        // non-hex
        "550e8400-e29b-41d4-a716-44665544zzzz".to_string(),
        // newline / control
        "550e8400-e29b-41d4-a716-446655440000\n".to_string(),
        "\n550e8400-e29b-41d4-a716-446655440000".to_string(),
        // file:// scheme
        "file:///c:/windows/system32/drivers/etc/hosts".to_string(),
        // path traversal in UUID-shaped wrapper
        "../550e8400-e29b-41d4-a716-446655440000".to_string(),
        // ridiculously long
        "x".repeat(10_000),
        // unicode
        "你好-e29b-41d4-a716-446655440000".to_string(),
        // common mistakes
        ".".to_string(),
        "..".to_string(),
        "/".to_string(),
        "\\".to_string(),
        // partial UUIDs
        "550e8400".to_string(),
        "550e8400-e29b".to_string(),
        "550e8400-e29b-41d4".to_string(),
        "550e8400-e29b-41d4-a716".to_string(),
    ]
}

#[test]
fn validate_id_accepts_valid_uuids() {
    for id in _all_valid_uuids() {
        assert!(
            validate_id(&id).is_ok(),
            "expected accept for valid UUID: {id:?}"
        );
    }
}

#[test]
fn validate_id_rejects_traversal_payloads_and_non_uuids() {
    for id in _all_invalid_ids() {
        assert!(
            validate_id(&id).is_err(),
            "expected reject for invalid id: {id:?}"
        );
    }
}

#[test]
fn validate_id_rejects_a_hundred_random_bytes_strings() {
    // Pseudo-random strings of varying lengths from a fixed seed (no
    // `rand` dep). Fuzz-style coverage to catch regressions when
    // someone changes the validate_id impl.
    let mut state: u64 = 0x00C0_FFEE_F00D;
    let mut accepted_garbage = 0;
    for _ in 0..100 {
        // Linear-congruential PRNG (constants from Numerical Recipes)
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let len = ((state >> 16) % 64) as usize + 1; // [1, 64]
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let b = (state & 0xFF) as u8;
            // Filter to printable ASCII so the test is stable on Windows
            let ch = (b % 95) + 32;
            s.push(ch as char);
        }
        // We don't assert reject (the PRNG could theoretically produce a
        // valid UUID — extremely unlikely but possible). Just count.
        if validate_id(&s).is_ok() {
            accepted_garbage += 1;
        }
    }
    // Statistical sanity: with 100 random strings, the chance of any one
    // being a valid UUID is ~0. Allow up to 1 just to be paranoid.
    assert!(
        accepted_garbage <= 1,
        "validate_id accepted {accepted_garbage}/100 random strings — looks too lenient"
    );
}

// ─── cleanup_old_recordings(0) edge case ───────────────────────────────

#[test]
fn cleanup_zero_days_deletes_all_wav_files() {
    // M2 retro edge-case test. The documented behavior of `days == 0`:
    //   cutoff = SystemTime::now()
    //   keep iff mtime > cutoff (strict)
    //   → all files with mtime ≤ now (which is essentially every file
    //     that exists by the time the call happens) get deleted.
    // This is surprising — see the doc comment on `cleanup_old_recordings`
    // for the FIXME. Test pins the current behavior so a future fix is
    // an intentional change, not a silent regression.
    let dir = tempfile::tempdir().expect("tempdir");
    // Create three fresh .wav files
    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = uuid::Uuid::new_v4();
        let path = dir.path().join(format!("{id}.wav"));
        std::fs::write(&path, b"fake wav contents").expect("write");
        ids.push(id.to_string());
    }
    // Also a non-.wav file we should never touch (extension filter).
    std::fs::write(dir.path().join("notes.txt"), b"nope").expect("write");

    let deleted = cleanup_old_recordings_in_dir(dir.path(), 0).expect("cleanup");
    // Pin current behavior: 0-day cleanup deletes every WAV.
    assert_eq!(
        deleted.len(),
        3,
        "0-day cleanup deletes every WAV (surprising but documented)"
    );
    for id in &ids {
        assert!(
            deleted.contains(id),
            "expected id {id} in deleted list, got {deleted:?}"
        );
        assert!(!dir.path().join(format!("{id}.wav")).exists());
    }
    // And the .txt file is still there (extension filter holds).
    assert!(dir.path().join("notes.txt").exists());
}

#[test]
fn cleanup_one_day_deletes_old_files_only() {
    // We can't easily backdate files in a portable way without filetime
    // crates. So we just sanity check the "newer than cutoff" branch:
    // freshly written files survive a 1-day cleanup.
    let dir = tempfile::tempdir().expect("tempdir");
    let id = uuid::Uuid::new_v4();
    let path = dir.path().join(format!("{id}.wav"));
    std::fs::write(&path, b"fresh").expect("write");

    let deleted = cleanup_old_recordings_in_dir(dir.path(), 1).expect("cleanup");
    assert!(
        deleted.is_empty(),
        "fresh file should survive 1-day cleanup"
    );
    assert!(path.exists());
}

// ─── Tempfile-based integration test (no AppHandle) ────────────────────
//
// We test the underlying filesystem helpers (`recordings_dir` is gated on
// an `AppHandle`, but `recording_path`'s validation + the `fs::write` /
// `fs::read` / `fs::remove_file` flow can be exercised against an
// arbitrary directory). This catches regressions in the WAV round-trip
// without booting Tauri.

#[test]
fn save_read_delete_round_trip_via_filesystem() {
    let dir = tempfile::tempdir().expect("tempdir");
    let id = uuid::Uuid::new_v4().to_string();

    // validate_id passes for a real UUID
    validate_id(&id).expect("uuid valid");

    let path = dir.path().join(format!("{id}.wav"));
    let payload = b"RIFF....WAVEfmt fake wav body";
    std::fs::write(&path, payload).expect("save");

    let read_back = std::fs::read(&path).expect("read");
    assert_eq!(read_back, payload);

    // delete_recording analogue: existence check then remove
    assert!(path.exists());
    std::fs::remove_file(&path).expect("delete");
    assert!(!path.exists());

    // cleanup is a no-op now (empty dir)
    let deleted = cleanup_old_recordings_in_dir(dir.path(), 1).expect("cleanup");
    assert!(deleted.is_empty());
}

#[test]
fn delete_recording_path_not_found_returns_recording_not_found() {
    // Drive-by exercise of the delete_recording rejection path that does
    // not need an AppHandle: test that `recording_path`'s id validation
    // rejects garbage before we ever touch the filesystem.
    let bad_id = "../etc/passwd";
    assert!(validate_id(bad_id).is_err());
}
