// Hotkey listener pure-logic state machine + hot-path atomics.
//
// This module is platform-agnostic and exists so the state-machine tests
// don't need a real `WH_KEYBOARD_LL` hook. Platform integration lives in
// `windows.rs` (`cfg(target_os = "windows")`-gated).
//
// **Hot/cold path split** (per `doc/plans/03-rust-modules.md` "M4 challenger
// findings 落實"):
//
//   `WH_KEYBOARD_LL`'s hook proc runs at OS keyboard-input priority. If the
//   callback exceeds `LowLevelHooksTimeout` (default ~300 ms registry value
//   `HKEY_CURRENT_USER\Control Panel\Desktop`) Windows silently drops the
//   hook AND the user gets a multi-second OS-wide keyboard freeze. So the
//   hook proc cannot afford to acquire a `Mutex` — every field touched in
//   the hot path is an atomic. Cold-path state (Phase 2 custom recording
//   mode) can use `Mutex<Option<...>>` because that's only touched outside
//   the hook proc.
//
// **State-machine output**: the hook proc passes a `KeyEvent` describing
// the raw OS event into `apply_event`, which returns a `Vec<HotkeyEvent>`
// of zero or more semantic events to dispatch. The state machine itself
// owns no IO — all `app.emit(...)` calls happen in the hook proc / worker
// thread on the platform-specific side.
//
// **Why a Vec rather than a single Option<HotkeyEvent>**: the double-tap
// detector may emit *both* a `Released` and the original event in the same
// step in a future iteration; the empty/single-element common case
// allocates a single `Vec` per event so the cost is negligible compared to
// the hook proc's IPC overhead. (Could be swapped for an `ArrayVec` later.)

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

use super::types::{TriggerKey, TriggerMode};

/// 350 ms double-tap window. SayIt uses the same constant — pressing the
/// trigger key twice within this window after a release counts as a
/// "double tap" the future Phase 2 mode-toggle UX hooks into. Phase 1 still
/// emits the regular `Pressed` so the recording flow is unaffected.
pub const DOUBLE_TAP_WINDOW_MS: u64 = 350;

/// Sentinel value for "no prior release recorded" in
/// `double_tap_last_release_ms`. We pick `u64::MAX` because real timestamps
/// are millis-since-some-epoch and will never reach this for centuries.
const NO_PRIOR_RELEASE_MS: u64 = u64::MAX;

/// Hot-path shared state. Every field touched by `hook_proc` is atomic so
/// the OS keyboard-priority callback never blocks on a `Mutex`.
///
/// Field overview:
///
/// * `trigger_key` — `TriggerKey::to_u8()` discriminant; current configured
///   trigger key.
/// * `trigger_mode` — `TriggerMode::to_u8()` discriminant; Hold or Toggle.
/// * `is_pressed` — Hold-mode key-down state. Set on key-down event, cleared
///   on key-up.
/// * `is_toggled_on` — Toggle-mode XOR state. Each Toggle key-down event
///   flips this.
/// * `double_tap_last_release_ms` — Timestamp (ms) of the last key-up, used
///   to detect a release-then-press within `DOUBLE_TAP_WINDOW_MS`.
#[derive(Debug)]
pub struct HotkeySharedState {
    pub trigger_key: AtomicU8,
    pub trigger_mode: AtomicU8,
    pub is_pressed: AtomicBool,
    pub is_toggled_on: AtomicBool,
    pub double_tap_last_release_ms: AtomicU64,
    // Cold path placeholder (Phase 2 custom recording).
    // pub recording_state: Mutex<Option<RecordingMode>>,
}

impl Default for HotkeySharedState {
    fn default() -> Self {
        Self {
            trigger_key: AtomicU8::new(TriggerKey::RightAlt.to_u8()),
            trigger_mode: AtomicU8::new(TriggerMode::Hold.to_u8()),
            is_pressed: AtomicBool::new(false),
            is_toggled_on: AtomicBool::new(false),
            double_tap_last_release_ms: AtomicU64::new(NO_PRIOR_RELEASE_MS),
        }
    }
}

impl HotkeySharedState {
    /// Returns the currently configured trigger key, falling back to the
    /// default (`RightAlt`) on any unknown atomic byte (defensive — the
    /// only writers are `set_trigger_key` and the constructor, both of
    /// which round-trip through `to_u8` so this fallback should be dead
    /// code).
    pub fn current_trigger_key(&self) -> TriggerKey {
        TriggerKey::from_u8(self.trigger_key.load(Ordering::Relaxed))
            .unwrap_or(TriggerKey::RightAlt)
    }

    /// Returns the currently configured trigger mode, falling back to
    /// `Hold` on any unknown atomic byte.
    pub fn current_trigger_mode(&self) -> TriggerMode {
        TriggerMode::from_u8(self.trigger_mode.load(Ordering::Relaxed)).unwrap_or(TriggerMode::Hold)
    }

    /// Hot-swap the trigger key. Called by `update_hotkey_config` on the
    /// command thread; the hook proc picks it up on the next keyboard
    /// event via `current_trigger_key`.
    ///
    /// Also resets transient state (`is_pressed`, `is_toggled_on`,
    /// double-tap timer) so a half-pressed legacy key doesn't leak into
    /// the new trigger's state machine.
    pub fn set_trigger_key(&self, k: TriggerKey) {
        self.trigger_key.store(k.to_u8(), Ordering::Relaxed);
        self.is_pressed.store(false, Ordering::Relaxed);
        self.is_toggled_on.store(false, Ordering::Relaxed);
        self.double_tap_last_release_ms
            .store(NO_PRIOR_RELEASE_MS, Ordering::Relaxed);
    }

    /// Hot-swap the trigger mode. Same reset semantics as `set_trigger_key`.
    pub fn set_trigger_mode(&self, m: TriggerMode) {
        self.trigger_mode.store(m.to_u8(), Ordering::Relaxed);
        self.is_pressed.store(false, Ordering::Relaxed);
        self.is_toggled_on.store(false, Ordering::Relaxed);
        self.double_tap_last_release_ms
            .store(NO_PRIOR_RELEASE_MS, Ordering::Relaxed);
    }

    /// Decide whether the OS-level hook proc should swallow this keystroke
    /// (`LRESULT(1)`) instead of forwarding to the next hook + target window
    /// (`CallNextHookEx`). Suppression makes the configured trigger key
    /// invisible to other apps so it can no longer trigger app-native
    /// shortcuts (e.g. `Right Alt` opening the menu bar in Word / Notepad).
    ///
    /// Decision tree (must align with `apply_event` so the user's mental
    /// model matches):
    ///
    ///   1. AltGr active (LCONTROL + RMENU) → **don't** suppress; we must
    ///      let the OS deliver the AltGr-prefixed character (€, @, # on EU
    ///      keyboards). `apply_event` already drops the dispatch; this only
    ///      ensures the keystroke still reaches the target.
    ///   2. ESC → **don't** suppress; ESC has too many target-app uses
    ///      (close dialog, cancel, etc.) and our use case (cancel recording)
    ///      is additive. We emit the `escape:pressed` event but still let
    ///      ESC propagate.
    ///   3. `vk` matches the configured trigger → **suppress**, regardless
    ///      of mode (Hold / Toggle) and direction (down / up). Symmetric
    ///      suppression of both down and up prevents stray modifier-up
    ///      events from reaching the target.
    ///   4. Anything else → **don't** suppress.
    pub fn should_suppress(&self, ev: &KeyEvent) -> bool {
        if ev.altgr_active {
            return false;
        }
        if ev.vk == VK_ESCAPE {
            return false;
        }
        ev.vk == self.current_trigger_key().virtual_key()
    }

    /// Pure-logic step over a single raw `KeyEvent`. Returns the semantic
    /// `HotkeyEvent`(s) the platform layer should dispatch via `app.emit`.
    ///
    /// Decision tree:
    ///
    ///   1. `vk == VK_ESCAPE` (`0x1B`) on key-down → emit `EscapePressed`
    ///      regardless of mode. The recording flow uses this to cancel
    ///      without paste.
    ///   2. `altgr_active` (RightAlt + LCONTROL both down) → drop the
    ///      event; the user is composing an AltGr character and we should
    ///      not steal the key.
    ///   3. `vk` doesn't match the configured trigger key → drop.
    ///   4. Hold mode:
    ///        * key-down → `is_pressed.store(true)`; emit `Pressed`. Also
    ///          updates the double-tap detector if a recent release exists.
    ///        * key-up   → `is_pressed.store(false)`; emit `Released`;
    ///          stamp `double_tap_last_release_ms = ts_ms`.
    ///   5. Toggle mode:
    ///        * key-down → XOR `is_toggled_on`; emit `ToggledOn` /
    ///          `ToggledOff` based on the new state.
    ///        * key-up   → drop (Toggle mode ignores releases).
    pub fn apply_event(&self, ev: KeyEvent) -> Vec<HotkeyEvent> {
        // 1. ESC key down → emit unconditionally (cancels recording).
        if ev.vk == VK_ESCAPE && ev.is_down {
            return vec![HotkeyEvent::EscapePressed];
        }

        // 2. AltGr suppression — the user is typing an AltGr-prefixed
        //    character (€, @, #, etc. on EU keyboards). Even if the
        //    configured trigger is RightAlt we must NOT steal it; the
        //    target window will receive the regular WM_KEYDOWN and IME
        //    will compose the character.
        if ev.altgr_active {
            return Vec::new();
        }

        // 3. Wrong key → drop.
        let trigger_vk = self.current_trigger_key().virtual_key();
        if ev.vk != trigger_vk {
            return Vec::new();
        }

        // 4 + 5. Mode-specific dispatch.
        match self.current_trigger_mode() {
            TriggerMode::Hold => self.step_hold(ev),
            TriggerMode::Toggle => self.step_toggle(ev),
        }
    }

    fn step_hold(&self, ev: KeyEvent) -> Vec<HotkeyEvent> {
        if ev.is_down {
            // Re-arm idempotently — Windows can repeat key-down events
            // (auto-repeat) but we still emit on every event so the
            // upstream voice-flow handler can debounce if it wants. Update
            // double-tap state from any prior release.
            //
            // Double-tap detection: if there's a stamped release within
            // `DOUBLE_TAP_WINDOW_MS`, this counts as a tap. We don't emit
            // a separate event for it in Phase 1; just clear the stamp so
            // a third press doesn't re-trigger. Phase 2 mode-toggle UX
            // hooks into this.
            let last = self.double_tap_last_release_ms.load(Ordering::Relaxed);
            if last != NO_PRIOR_RELEASE_MS && ev.ts_ms.saturating_sub(last) <= DOUBLE_TAP_WINDOW_MS
            {
                self.double_tap_last_release_ms
                    .store(NO_PRIOR_RELEASE_MS, Ordering::Relaxed);
            }

            self.is_pressed.store(true, Ordering::Relaxed);
            vec![HotkeyEvent::Pressed]
        } else {
            self.is_pressed.store(false, Ordering::Relaxed);
            self.double_tap_last_release_ms
                .store(ev.ts_ms, Ordering::Relaxed);
            vec![HotkeyEvent::Released]
        }
    }

    fn step_toggle(&self, ev: KeyEvent) -> Vec<HotkeyEvent> {
        // Toggle mode: only react to key-down. Releases are ignored so the
        // user can hold the key without spamming events.
        if !ev.is_down {
            return Vec::new();
        }

        // XOR the toggle state. `fetch_xor` returns the *prior* value, so
        // the new state is the negation of the returned value.
        let prior = self.is_toggled_on.fetch_xor(true, Ordering::Relaxed);
        let new_state = !prior;
        if new_state {
            vec![HotkeyEvent::ToggledOn]
        } else {
            vec![HotkeyEvent::ToggledOff]
        }
    }
}

/// `VK_ESCAPE` (0x1B) — pulled into a constant so `apply_event` doesn't
/// depend on the Win32 binding (lets the unit tests run on macOS / Linux
/// CI without the `windows` crate).
const VK_ESCAPE: u32 = 0x1B;

/// Raw OS-level key event passed to the state machine. Built by the hook
/// proc on Windows (`windows.rs`) — the unit tests construct this directly
/// to exercise `apply_event` without a real hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Windows virtual-key code (`VK_*`).
    pub vk: u32,
    /// `true` for key-down, `false` for key-up.
    pub is_down: bool,
    /// Timestamp in milliseconds (any monotonic origin will do — only
    /// used for relative arithmetic against `double_tap_last_release_ms`).
    pub ts_ms: u64,
    /// `true` iff the OS state shows `LCONTROL` already pressed when this
    /// `RightAlt` event arrived. `LCONTROL + RMENU` is the AltGr modifier
    /// sequence on EU keyboards used to type `@`, `€`, `#`, etc. — the hook
    /// proc must NOT steal the key in that case. Set by `windows.rs` via
    /// `GetAsyncKeyState(VK_LCONTROL)`.
    pub altgr_active: bool,
}

/// Semantic events produced by the state machine for the platform layer
/// to dispatch via `app.emit(...)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Hold-mode key-down. Emit `hotkey:pressed` with `triggerMode: hold`,
    /// `action: pressed`.
    Pressed,
    /// Hold-mode key-up. Emit `hotkey:released` with `triggerMode: hold`,
    /// `action: released`.
    Released,
    /// Toggle-mode key-down that flipped state to ON. Emit `hotkey:toggled`
    /// with `triggerMode: toggle`, `action: toggled-on`.
    ToggledOn,
    /// Toggle-mode key-down that flipped state to OFF. Emit `hotkey:toggled`
    /// with `triggerMode: toggle`, `action: toggled-off`.
    ToggledOff,
    /// ESC key-down at any time. Emit `escape:pressed` with no payload.
    /// The voice-flow store cancels the in-progress recording without paste.
    EscapePressed,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a `KeyEvent` for the configured trigger key.
    fn ev_trigger(state: &HotkeySharedState, is_down: bool, ts_ms: u64) -> KeyEvent {
        KeyEvent {
            vk: state.current_trigger_key().virtual_key(),
            is_down,
            ts_ms,
            altgr_active: false,
        }
    }

    #[test]
    fn default_state_is_right_alt_hold_clean() {
        let s = HotkeySharedState::default();
        assert_eq!(s.current_trigger_key(), TriggerKey::RightAlt);
        assert_eq!(s.current_trigger_mode(), TriggerMode::Hold);
        assert!(!s.is_pressed.load(Ordering::Relaxed));
        assert!(!s.is_toggled_on.load(Ordering::Relaxed));
    }

    #[test]
    fn hold_mode_basic_press_release() {
        let s = HotkeySharedState::default();
        // Down → Pressed
        let out = s.apply_event(ev_trigger(&s, true, 100));
        assert_eq!(out, vec![HotkeyEvent::Pressed]);
        assert!(s.is_pressed.load(Ordering::Relaxed));

        // Up → Released
        let out = s.apply_event(ev_trigger(&s, false, 200));
        assert_eq!(out, vec![HotkeyEvent::Released]);
        assert!(!s.is_pressed.load(Ordering::Relaxed));
    }

    /// Mandatory case (a) — 5 down/up cycles within 1s should emit 5
    /// alternating Pressed/Released without leaking state.
    #[test]
    fn five_x_burst_in_one_second_hold_mode() {
        let s = HotkeySharedState::default();
        let mut emitted: Vec<HotkeyEvent> = Vec::new();
        // 5 cycles: down at 0, up at 50; down at 200, up at 250; ...
        for i in 0..5 {
            let down_t = (i * 200) as u64;
            let up_t = down_t + 50;
            emitted.extend(s.apply_event(ev_trigger(&s, true, down_t)));
            emitted.extend(s.apply_event(ev_trigger(&s, false, up_t)));
        }
        assert_eq!(emitted.len(), 10, "expected 10 events");
        for chunk in emitted.chunks(2) {
            assert_eq!(chunk, &[HotkeyEvent::Pressed, HotkeyEvent::Released]);
        }
        // After all events, the press flag must be cleared.
        assert!(!s.is_pressed.load(Ordering::Relaxed));
    }

    /// Mandatory case (b) — double-tap detection at the 350 ms boundary.
    /// We don't emit a separate event for the double-tap in Phase 1, but
    /// we DO clear the stored release timestamp so a third tap starts
    /// fresh. This test inspects the atomic to confirm boundary behaviour.
    #[test]
    fn double_tap_350ms_boundary() {
        // 349 ms — within window → stamp cleared.
        {
            let s = HotkeySharedState::default();
            s.apply_event(ev_trigger(&s, true, 0));
            s.apply_event(ev_trigger(&s, false, 0)); // release at t=0
            assert_eq!(s.double_tap_last_release_ms.load(Ordering::Relaxed), 0);
            // Press at t=349 — within window.
            s.apply_event(ev_trigger(&s, true, 349));
            assert_eq!(
                s.double_tap_last_release_ms.load(Ordering::Relaxed),
                NO_PRIOR_RELEASE_MS,
                "349ms tap should be detected as double-tap and clear stamp"
            );
        }
        // 350 ms — exactly at boundary → cleared (use <=).
        {
            let s = HotkeySharedState::default();
            s.apply_event(ev_trigger(&s, true, 0));
            s.apply_event(ev_trigger(&s, false, 0));
            s.apply_event(ev_trigger(&s, true, 350));
            assert_eq!(
                s.double_tap_last_release_ms.load(Ordering::Relaxed),
                NO_PRIOR_RELEASE_MS,
                "350ms tap should still be detected (inclusive)"
            );
        }
        // 351 ms — outside window → stamp UNCHANGED (still 0).
        {
            let s = HotkeySharedState::default();
            s.apply_event(ev_trigger(&s, true, 0));
            s.apply_event(ev_trigger(&s, false, 0));
            s.apply_event(ev_trigger(&s, true, 351));
            assert_eq!(
                s.double_tap_last_release_ms.load(Ordering::Relaxed),
                0,
                "351ms tap is outside window — stamp should remain at 0"
            );
        }
    }

    /// Mandatory case (c) — AltGr suppression: RightAlt down with
    /// `altgr_active: true` must NOT dispatch a Pressed event. EU keyboard
    /// users typing `@` (LCONTROL + RMENU + Q on some layouts) get the key
    /// passed through.
    #[test]
    fn altgr_suppression_drops_event() {
        let s = HotkeySharedState::default();
        let ev = KeyEvent {
            vk: TriggerKey::RightAlt.virtual_key(),
            is_down: true,
            ts_ms: 100,
            altgr_active: true,
        };
        let out = s.apply_event(ev);
        assert!(out.is_empty(), "AltGr-active event must be dropped");
        // Side-effect check: state must not have flipped.
        assert!(!s.is_pressed.load(Ordering::Relaxed));
    }

    /// Mandatory case (d) — Toggle mode XOR: 4 sequential down events emit
    /// ToggledOn / ToggledOff / ToggledOn / ToggledOff. Releases between
    /// presses are ignored.
    #[test]
    fn toggle_xor_4_presses() {
        let s = HotkeySharedState::default();
        s.set_trigger_mode(TriggerMode::Toggle);

        let mut emitted: Vec<HotkeyEvent> = Vec::new();
        for i in 0..4 {
            let t = (i * 100) as u64;
            emitted.extend(s.apply_event(ev_trigger(&s, true, t)));
            // Release in between — must produce no event in Toggle mode.
            let release_out = s.apply_event(ev_trigger(&s, false, t + 50));
            assert!(
                release_out.is_empty(),
                "Toggle mode must ignore releases (got {release_out:?})"
            );
        }

        assert_eq!(
            emitted,
            vec![
                HotkeyEvent::ToggledOn,
                HotkeyEvent::ToggledOff,
                HotkeyEvent::ToggledOn,
                HotkeyEvent::ToggledOff,
            ]
        );
    }

    #[test]
    fn esc_key_emits_escape_pressed_in_hold_mode() {
        let s = HotkeySharedState::default();
        let ev = KeyEvent {
            vk: VK_ESCAPE,
            is_down: true,
            ts_ms: 0,
            altgr_active: false,
        };
        let out = s.apply_event(ev);
        assert_eq!(out, vec![HotkeyEvent::EscapePressed]);
    }

    #[test]
    fn esc_key_emits_escape_pressed_in_toggle_mode() {
        // ESC must work regardless of mode — toggle mode users still need
        // ESC to cancel an in-progress recording.
        let s = HotkeySharedState::default();
        s.set_trigger_mode(TriggerMode::Toggle);
        let ev = KeyEvent {
            vk: VK_ESCAPE,
            is_down: true,
            ts_ms: 0,
            altgr_active: false,
        };
        let out = s.apply_event(ev);
        assert_eq!(out, vec![HotkeyEvent::EscapePressed]);
    }

    #[test]
    fn esc_key_up_does_not_emit() {
        // Only ESC down emits. Up is silent (avoids double-cancel).
        let s = HotkeySharedState::default();
        let ev = KeyEvent {
            vk: VK_ESCAPE,
            is_down: false,
            ts_ms: 0,
            altgr_active: false,
        };
        let out = s.apply_event(ev);
        assert!(out.is_empty());
    }

    #[test]
    fn unrelated_key_is_dropped() {
        let s = HotkeySharedState::default();
        // 'A' (0x41) is not a trigger key.
        let ev = KeyEvent {
            vk: 0x41,
            is_down: true,
            ts_ms: 0,
            altgr_active: false,
        };
        let out = s.apply_event(ev);
        assert!(out.is_empty());
    }

    #[test]
    fn set_trigger_key_resets_transient_state() {
        let s = HotkeySharedState::default();
        // Press down to build state.
        s.apply_event(ev_trigger(&s, true, 100));
        assert!(s.is_pressed.load(Ordering::Relaxed));

        // Hot-swap to a new trigger key — must reset is_pressed.
        s.set_trigger_key(TriggerKey::LeftAlt);
        assert!(!s.is_pressed.load(Ordering::Relaxed));

        // The previous trigger (RightAlt) must no longer fire.
        let stale = KeyEvent {
            vk: TriggerKey::RightAlt.virtual_key(),
            is_down: true,
            ts_ms: 200,
            altgr_active: false,
        };
        assert!(s.apply_event(stale).is_empty());
    }

    #[test]
    fn set_trigger_mode_resets_transient_state() {
        let s = HotkeySharedState::default();
        s.set_trigger_mode(TriggerMode::Toggle);
        // Toggle one press to ON.
        s.apply_event(ev_trigger(&s, true, 0));
        assert!(s.is_toggled_on.load(Ordering::Relaxed));

        // Hot-swap to Hold mode — toggled flag must clear so we don't
        // start the new mode in a "phantom recording" state.
        s.set_trigger_mode(TriggerMode::Hold);
        assert!(!s.is_toggled_on.load(Ordering::Relaxed));
    }

    #[test]
    fn altgr_suppression_in_toggle_mode_also_drops() {
        // AltGr suppression must apply in Toggle mode too — typing AltGr+@
        // shouldn't toggle the recording state.
        let s = HotkeySharedState::default();
        s.set_trigger_mode(TriggerMode::Toggle);
        let ev = KeyEvent {
            vk: TriggerKey::RightAlt.virtual_key(),
            is_down: true,
            ts_ms: 0,
            altgr_active: true,
        };
        let out = s.apply_event(ev);
        assert!(out.is_empty());
        assert!(!s.is_toggled_on.load(Ordering::Relaxed));
    }

    #[test]
    fn double_tap_ignored_when_no_prior_release() {
        // First press without any prior release — the double-tap branch
        // should NOT fire (NO_PRIOR_RELEASE_MS sentinel).
        let s = HotkeySharedState::default();
        let prior_stamp = s.double_tap_last_release_ms.load(Ordering::Relaxed);
        assert_eq!(prior_stamp, NO_PRIOR_RELEASE_MS);

        s.apply_event(ev_trigger(&s, true, 100));
        // Stamp should remain NO_PRIOR_RELEASE_MS (we only clear on
        // detection; the press itself doesn't write the stamp).
        assert_eq!(
            s.double_tap_last_release_ms.load(Ordering::Relaxed),
            NO_PRIOR_RELEASE_MS
        );
    }

    /// Configured trigger key must be suppressed — both down and up — so
    /// target apps don't see the modifier and can't fire their native
    /// shortcuts (e.g. Right Alt opening Word's menu bar).
    #[test]
    fn should_suppress_trigger_key_down_and_up() {
        let s = HotkeySharedState::default();
        let down = ev_trigger(&s, true, 100);
        let up = ev_trigger(&s, false, 200);
        assert!(s.should_suppress(&down), "trigger down must suppress");
        assert!(s.should_suppress(&up), "trigger up must suppress");
    }

    /// AltGr-active events (LCONTROL + RMENU on EU keyboards) must NOT
    /// be suppressed — the OS still needs to deliver `€ @ #` etc. to the
    /// target window.
    #[test]
    fn should_not_suppress_when_altgr_active() {
        let s = HotkeySharedState::default();
        let altgr = KeyEvent {
            vk: TriggerKey::RightAlt.virtual_key(),
            is_down: true,
            ts_ms: 100,
            altgr_active: true,
        };
        assert!(!s.should_suppress(&altgr));
    }

    /// ESC must pass through to target apps (close dialog, cancel, etc.).
    /// The escape:pressed event we emit is additive, not exclusive.
    #[test]
    fn should_not_suppress_escape() {
        let s = HotkeySharedState::default();
        let esc = KeyEvent {
            vk: VK_ESCAPE,
            is_down: true,
            ts_ms: 100,
            altgr_active: false,
        };
        assert!(!s.should_suppress(&esc));
    }

    /// Non-trigger keys (any other VK) must not be suppressed.
    #[test]
    fn should_not_suppress_unrelated_keys() {
        let s = HotkeySharedState::default();
        let unrelated = KeyEvent {
            vk: 0x41, // 'A'
            is_down: true,
            ts_ms: 100,
            altgr_active: false,
        };
        assert!(!s.should_suppress(&unrelated));
    }

    /// After hot-swap to a different trigger key, the OLD trigger no
    /// longer suppresses; the NEW one does.
    #[test]
    fn should_suppress_follows_trigger_key_hot_swap() {
        let s = HotkeySharedState::default();
        s.set_trigger_key(TriggerKey::RightControl);

        let old_trigger = KeyEvent {
            vk: TriggerKey::RightAlt.virtual_key(),
            is_down: true,
            ts_ms: 100,
            altgr_active: false,
        };
        let new_trigger = KeyEvent {
            vk: TriggerKey::RightControl.virtual_key(),
            is_down: true,
            ts_ms: 100,
            altgr_active: false,
        };
        assert!(!s.should_suppress(&old_trigger));
        assert!(s.should_suppress(&new_trigger));
    }
}
