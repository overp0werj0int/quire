//! Unified document reveal controller (`REVEALMOD`).
//!
//! One explicit state machine owns every automatic document scroll reveal on `doc-scroll`:
//! - toast/notice band appearance and growth lifts (caret or current find match),
//! - hand-control detection and immediate cancellation of pending lifts on pointer/wheel/touchpad
//!   scrolls, distinguishing genuine hand scrolls from editor typing/caret `follow_caret()` moves,
//! - smooth deferred caret reveals on the next editor keystroke (respecting reduced motion),
//! - gradual release of the band's extra end room, and
//! - find-match placement between the top roof and the foot band.

use std::cell::Cell;
use std::rc::Rc;

use rune::Spring;

/// Critically damped glide used for toast-band lifts and deferred caret reveals:
/// for a ~104 pt deferred reveal, peak step at 60 Hz is ~16.7 pt (<= 24 pt) and settles in ~0.25 s.
pub const REVEAL_SPRING: Spring = Spring::new(0.24, 0.0);

/// How long after a genuine user hand scroll the page still counts as hand-controlled:
/// a wheel/swipe glide ticks every frame while active.
pub const HAND_STILL: f64 = 0.1;

/// Frames `PendingLift` waits before starting its glide (~17 ms at 60 Hz) so a wheel/swipe
/// gesture arriving on the band's layout frame steps its `WheelCurve` (whose first frame is
/// `fresh` with 0 delta) before `poll` decides whether to lift, cancelling with zero reversal.
const LIFT_SETTLE_FRAMES: u8 = 1;

/// How long a find placement follows its match while surrounding virtual rows measure.
const PLACE_FOR: f64 = 0.6;

/// The page's extra end room closes in on this time constant, in seconds (RD-104)...
const ROOM_EASE: f32 = 0.08;
/// ...and never faster than this, in points per second.
const ROOM_SPEED: f32 = 240.0;

/// Per-frame scroll telemetry shared between `doc-scroll`'s `on_scroll_offset` callback and
/// `Reveal`. Updated directly inside the scroll handler with no view rebuild.
#[derive(Clone, Copy, Debug)]
pub struct ScrollSignal {
    /// Clock time of the most recent genuine user hand scroll (wheel, swipe, momentum, drag,
    /// reader key scroll). Never stamped by synchronous editor `follow_caret()` scrolls.
    pub hand_at: f64,
    /// Monotonic counter incremented on every genuine user hand scroll.
    pub hand_gen: u64,
    /// Set when an editor typing/caret key or edit runs so Rune's synchronous `follow_caret()`
    /// scroll on the same frame is classified as editor-driven rather than a user hand swipe.
    pub editor_key_frame: bool,
}

impl Default for ScrollSignal {
    fn default() -> Self {
        Self { hand_at: f64::NEG_INFINITY, hand_gen: 0, editor_key_frame: false }
    }
}

impl ScrollSignal {
    /// Records one `doc-scroll` offset callback. Returns `true` when this callback was a genuine
    /// user hand scroll.
    pub fn on_scroll_offset(&mut self, moved: bool, by_hand: bool, t: f64) -> bool {
        if !by_hand || !moved {
            return false;
        }
        if self.editor_key_frame {
            self.editor_key_frame = false;
            return false;
        }
        self.hand_at = t;
        self.hand_gen = self.hand_gen.wrapping_add(1);
        true
    }
}

/// Explicit state machine governing band lifts and deferred caret reveals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BandState {
    /// No lift pending and no deferred reveal awaiting a keystroke.
    Idle,
    /// A foot band appeared or grew and asked to lift the caret or find match to `target`.
    /// Waits `settle_frames` frames; any genuine hand scroll cancels it immediately to `Deferred`.
    PendingLift { target: f32, armed_gen: u64, settle_frames: u8 },
    /// A band lift was suppressed by user hand scrolling. The page stays where the user put it
    /// until the next editor key/edit or until the band disappears.
    Deferred,
    /// An editor key/edit ran while a foot band is visible; waiting for layout to report the
    /// fresh caret rect (`seq`) so we can glide it above the band if it sits underneath.
    AwaitCaret { seq: u32, armed_gen: u64, woken: bool },
    /// A `REVEAL_SPRING` glide toward `target` is in flight. Further editor keys during the glide
    /// do not re-sample the mid-glide caret (which lags `scroll.offset` by one spring tick and
    /// would overshoot); once the glide settles, `recheck` triggers one steady check so newly
    /// added lines (e.g. `Enter` or wrap) are also lifted (`REVEALWEL`).
    Gliding { target: f32, armed_gen: u64, started: f64, recheck: bool },
}

/// Scroll action emitted by the band/caret state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RevealAction {
    None,
    /// Keep polling on the next frame while `PendingLift` settles.
    Wake,
    /// Smoothly glide `doc-scroll` to `target` with `REVEAL_SPRING`.
    Glide(f32),
    /// Jump `doc-scroll` immediately to `target` (reduced motion).
    Instant(f32),
}

/// A find match being brought into view once its highlight lays out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub n: u32,
    at: f64,
    glide: bool,
    to: Option<f32>,
    anchored: bool,
    expected_h: f32,
}

/// Scroll action emitted when a find match's highlight reports its laid-out rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MatchAction {
    None,
    Glide(f32),
    Jump(f32),
}

/// Where a find match `h` tall goes (its top below the view's top) in a view `vh` tall whose top
/// band reaches `roof` down and whose foot band is `band` tall: the middle of the room between
/// them, or of the view with no band or when it cannot fit there (RD-60, WR-118, RD-104).
pub fn match_to(vh: f32, h: f32, roof: f32, band: f32) -> f32 {
    let room = vh - band;
    if band <= 0.0 || room - roof < h { ((vh - h) / 2.0).max(0.0) } else { roof + (room - roof - h) / 2.0 }
}

/// Single controller for all document reveal decisions on `doc-scroll`.
pub struct Reveal {
    pub signal: Rc<Cell<ScrollSignal>>,
    pub state: BandState,
    /// How tall the toast/notice band last laid out, 0 while no pill shows (RD-104).
    pub band: f32,
    /// Extra end room held after the page's last line for the band, released gradually.
    pub room: f32,
    room_at: f64,
    caret_seq: u32,
    /// Active find-match placement awaiting layout.
    pub place: Option<Place>,
    placed: u32,
    /// Last laid-out line y of the current find match (from the page's top).
    pub find_line: Option<f32>,
}

impl Default for Reveal {
    fn default() -> Self {
        Self {
            signal: Rc::new(Cell::new(ScrollSignal::default())),
            state: BandState::Idle,
            band: 0.0,
            room: 0.0,
            room_at: 0.0,
            caret_seq: 0,
            place: None,
            placed: 0,
            find_line: None,
        }
    }
}

impl Reveal {
    /// Whether an active find placement is waiting for layout, and its serial.
    pub fn place_n(&self) -> Option<u32> {
        self.place.map(|p| p.n)
    }

    /// Whether an editor key/edit is waiting for the view to report the caret rect.
    pub fn awaiting_caret(&self) -> Option<u32> {
        match self.state {
            BandState::AwaitCaret { seq, .. } => Some(seq),
            _ => None,
        }
    }

    /// Records an editor typing/navigation keystroke. Classifies any synchronous `follow_caret()`
    /// on this frame as editor-driven (not a hand swipe), and when a foot band is visible arms a
    /// smooth caret reveal once layout reports the caret rect. Returns `true` if a next-frame wake
    /// should be scheduled.
    pub fn on_editor_key(&mut self, writing_with_foot: bool, typewriter: bool) -> bool {
        let mut sig = self.signal.get();
        sig.editor_key_frame = true;
        self.signal.set(sig);
        if !writing_with_foot || typewriter {
            return false;
        }
        if let BandState::Gliding { recheck, .. } = &mut self.state {
            *recheck = true;
            return false;
        }
        self.caret_seq = self.caret_seq.wrapping_add(1);
        self.state = BandState::AwaitCaret { seq: self.caret_seq, armed_gen: sig.hand_gen, woken: false };
        true
    }

    /// Records an editor mutation from `after_edit()` (e.g. paste, toolbar format, undo/redo).
    pub fn on_editor_edit(&mut self, writing_with_foot: bool, typewriter: bool) {
        let mut sig = self.signal.get();
        sig.editor_key_frame = true;
        self.signal.set(sig);
        if !writing_with_foot || typewriter {
            return;
        }
        if let BandState::Gliding { recheck, .. } = &mut self.state {
            *recheck = true;
            return;
        }
        if !matches!(self.state, BandState::AwaitCaret { .. }) {
            self.caret_seq = self.caret_seq.wrapping_add(1);
            // `after_edit()` already runs inside a committing event, so no extra poll wake is needed.
            self.state = BandState::AwaitCaret { seq: self.caret_seq, armed_gen: sig.hand_gen, woken: true };
        }
    }

    /// Called when `toast-band` lays out at window rect `[x, y, w, h]` (RD-104).
    /// Returns `true` when `PendingLift` was armed and needs a next-frame wake.
    #[allow(clippy::too_many_arguments)]
    pub fn on_band_layout(
        &mut self,
        [_, y, _, h]: [f32; 4],
        scroll: [f32; 4],
        caret: Option<[f32; 4]>,
        find_at: Option<[f32; 4]>,
        roof: f32,
        now: f64,
        typewriter: bool,
        has_find_query: bool,
    ) -> bool {
        let grew = h > self.band + 0.5;
        self.band = h;
        self.room = self.room.max(h);
        if !grew {
            return false;
        }
        let sig = self.signal.get();
        if now - sig.hand_at < HAND_STILL {
            self.state = BandState::Deferred;
            return false;
        }
        if matches!(self.state, BandState::Deferred) {
            return false;
        }
        if let BandState::Gliding { recheck, .. } = &mut self.state {
            *recheck = true;
            return false;
        }
        let [offset, _, vh, _] = scroll;
        let view = y + h - vh;
        let under = |top: f32, height: f32| top < y + h && top + height > view && top + height > y;
        let to = if let Some([_, top, _, height]) = caret {
            if typewriter || !under(top, height) {
                return false;
            }
            offset + top + height - y
        } else {
            let Some([_, top, _, height]) = find_at.filter(|_| has_find_query) else { return false };
            let top = top - offset;
            if !under(top, height) {
                return false;
            }
            if y - (view + roof) >= height {
                offset + top + height - y
            } else {
                offset + top - view - ((vh - height) / 2.0).max(0.0)
            }
        };
        self.state = BandState::PendingLift { target: to, armed_gen: sig.hand_gen, settle_frames: LIFT_SETTLE_FRAMES };
        true
    }

    /// Called when `caret-reveal-{seq}` reports the fresh caret rect after an editor key/edit.
    #[allow(clippy::too_many_arguments)]
    pub fn on_caret_layout(
        &mut self,
        seq: u32,
        [_, top, _, height]: [f32; 4],
        [_, vy, _, vh]: [f32; 4],
        band: f32,
        [offset, max, ..]: [f32; 4],
        now: f64,
        typewriter: bool,
        reduced: bool,
    ) -> RevealAction {
        let BandState::AwaitCaret { seq: want, armed_gen, .. } = self.state else {
            return RevealAction::None;
        };
        if want != seq {
            return RevealAction::None;
        }
        if self.signal.get().hand_gen != armed_gen {
            self.state = BandState::Deferred;
            return RevealAction::None;
        }
        self.state = BandState::Idle;
        if typewriter || band <= 0.0 {
            return RevealAction::None;
        }
        let view_bot = vy + vh;
        let band_top = view_bot - band;
        let caret_bot = top + height;
        // Only reveal when the caret is inside the view and overlaps the foot band.
        if top >= view_bot || caret_bot <= vy || caret_bot <= band_top + 0.5 {
            return RevealAction::None;
        }
        let target = (offset + caret_bot - band_top).clamp(0.0, max.max(0.0));
        if (target - offset).abs() <= 0.5 {
            return RevealAction::None;
        }
        if reduced {
            RevealAction::Instant(target)
        } else {
            self.state = BandState::Gliding { target, armed_gen, started: now, recheck: false };
            RevealAction::Glide(target)
        }
    }

    /// Advances the reveal controller once per frame in `App::poll`:
    /// - clears the previous frame's `editor_key_frame` flag,
    /// - ticks `PendingLift` (immediately cancelling on any hand scroll),
    /// - wakes `view()` once if `AwaitCaret` has not yet been measured,
    /// - watches active `Gliding` settles (re-checking once after settle if edits arrived mid-glide), and
    /// - gradually releases the band's extra end room when the band shrinks or disappears.
    ///
    /// Returns `(action, needs_view_rebuild, wake_next_frame)`.
    pub fn poll(&mut self, foot_shows: bool, now: f64, base_room: f32, scroll_offset: f32, reduced: bool) -> (RevealAction, bool, bool) {
        let mut sig = self.signal.get();
        sig.editor_key_frame = false;
        self.signal.set(sig);

        if !foot_shows {
            self.band = 0.0;
            self.state = BandState::Idle;
        }

        let mut action = RevealAction::None;
        let mut rebuild = false;
        let mut wake = false;

        match self.state {
            BandState::PendingLift { target, armed_gen, settle_frames } => {
                if sig.hand_gen != armed_gen || now - sig.hand_at < HAND_STILL {
                    self.state = BandState::Deferred;
                } else if settle_frames > 0 {
                    self.state = BandState::PendingLift { target, armed_gen, settle_frames: settle_frames - 1 };
                    action = RevealAction::Wake;
                    wake = true;
                } else if reduced {
                    self.state = BandState::Idle;
                    action = RevealAction::Instant(target);
                } else {
                    self.state = BandState::Gliding { target, armed_gen, started: now, recheck: false };
                    action = RevealAction::Glide(target);
                }
            }
            BandState::AwaitCaret { seq, armed_gen, woken } => {
                if sig.hand_gen != armed_gen {
                    self.state = BandState::Deferred;
                } else if !woken {
                    self.state = BandState::AwaitCaret { seq, armed_gen, woken: true };
                    rebuild = true;
                }
            }
            BandState::Gliding { target, armed_gen, started, recheck } => {
                if sig.hand_gen != armed_gen || now - sig.hand_at < HAND_STILL {
                    self.state = BandState::Deferred;
                } else if (scroll_offset - target).abs() <= 0.25 || now - started >= 0.45 {
                    if recheck {
                        self.caret_seq = self.caret_seq.wrapping_add(1);
                        self.state = BandState::AwaitCaret { seq: self.caret_seq, armed_gen, woken: true };
                        rebuild = true;
                    } else {
                        self.state = BandState::Idle;
                    }
                } else if recheck {
                    wake = true;
                }
            }
            BandState::Idle | BandState::Deferred => {}
        }

        let dt = (now - std::mem::replace(&mut self.room_at, now)).clamp(0.0, 0.05) as f32;
        let (room, want) = (self.room.max(base_room), self.band.max(base_room));
        if room <= want + 0.25 || reduced {
            self.room = self.band;
            rebuild |= room > want;
        } else {
            let step = ((room - want) * (1.0 - (-dt / ROOM_EASE).exp())).min(ROOM_SPEED * dt);
            self.room = room - step;
            rebuild = true;
            wake = true;
        }

        (action, rebuild, wake)
    }

    /// Starts tracking a find match placement awaiting highlight layout.
    pub fn begin_match(&mut self, now: f64, glide: bool, anchored: bool, expected_h: f32) -> u32 {
        self.placed = self.placed.wrapping_add(1);
        let n = self.placed;
        self.place = Some(Place { n, at: now, glide, to: None, anchored, expected_h });
        n
    }

    /// Clears find placement state when find closes.
    pub fn clear_find(&mut self) {
        self.place = None;
        self.find_line = None;
    }

    /// Resolves where find match `n` should scroll when its highlight lays out at `[_, y, _, h]`
    /// inside viewport `view`.
    #[allow(clippy::too_many_arguments)]
    pub fn on_match_layout(
        &mut self,
        n: u32,
        [_, y, _, h]: [f32; 4],
        view: [f32; 4],
        roof: f32,
        band: f32,
        scroll_offset: f32,
        now: f64,
    ) -> MatchAction {
        let Some(mut p) = self.place.filter(|p| p.n == n) else { return MatchAction::None };
        if now - p.at > PLACE_FOR {
            self.place = None;
            return MatchAction::None;
        }
        let (y, vh) = (y - view[1], view[3]);
        let line = self.find_line.replace(y);
        if p.anchored {
            self.place = None;
            // When `page::match_line` predicted the highlight's height, Rune's anchor landed
            // in one move; correct only when the actual rendered highlight height differed
            // materially (e.g. a wrapped multi-line highlight).
            if (h - p.expected_h).abs() > 1.0 {
                let to = match_to(vh, h, roof, band);
                let target = y - to;
                if (target - scroll_offset).abs() > 0.5 {
                    return MatchAction::Jump(target);
                }
            }
            return MatchAction::None;
        }
        let to = match p.to {
            Some(to) => to,
            None => {
                let shown = y - scroll_offset;
                let room = vh - band;
                if line.is_some_and(|l| (l - y).abs() < 0.5) && shown >= roof && shown + h <= room {
                    self.place = None;
                    return MatchAction::None;
                }
                let to = match_to(vh, h, roof, band);
                p.to = Some(to);
                self.place = Some(p);
                to
            }
        };
        if p.glide { MatchAction::Glide(y - to) } else { MatchAction::Jump(y - to) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2: when a toast band appears right before a wheel/swipe gesture, the first hand scroll
    /// frame immediately cancels `PendingLift` into `Deferred` with zero scroll action.
    #[test]
    fn m2_hand_scroll_cancels_pending_lift_without_reversal() {
        let mut r = Reveal::default();
        let scroll = [564.0, 1200.0, 759.0, 1032.0];
        // Caret sits at y=707..738 (44 pt under toast band y=694..800).
        let caret = Some([300.0, 707.0, 1.5, 31.0]);
        assert!(r.on_band_layout([248.0, 694.0, 1032.0, 106.0], scroll, caret, None, 62.0, 0.800, false, false));
        assert!(matches!(r.state, BandState::PendingLift { settle_frames: LIFT_SETTLE_FRAMES, .. }));

        // Frame 1 (816.7ms): poll waits one settle frame before firing.
        let (act1, _, wake1) = r.poll(true, 0.8167, 400.0, scroll[0], false);
        assert_eq!(act1, RevealAction::Wake);
        assert!(wake1);

        // End of frame 1: wheel curve steps and triggers a genuine hand scroll.
        let mut sig = r.signal.get();
        assert!(sig.on_scroll_offset(true, true, 0.8167));
        r.signal.set(sig);

        // Frame 2 (833.3ms): poll sees bumped hand_gen and cancels lift to Deferred.
        let (act2, _, _) = r.poll(true, 0.8333, 400.0, scroll[0], false);
        assert_eq!(act2, RevealAction::None);
        assert_eq!(r.state, BandState::Deferred);
    }

    /// M3: when a toast lift was deferred by a swipe, the next editor key smoothly glides the
    /// caret above the band (and jumps immediately under reduced motion).
    #[test]
    fn m3_deferred_caret_glides_on_next_key_and_respects_reduced_motion() {
        for reduced in [false, true] {
            let mut r = Reveal::default();
            // Swipe happened at t=0.8167s.
            let mut sig = r.signal.get();
            sig.on_scroll_offset(true, true, 0.8167);
            r.signal.set(sig);

            // Toast arrives during swipe at t=0.85s -> deferred.
            let scroll = [504.0, 1200.0, 759.0, 1032.0];
            assert!(!r.on_band_layout([248.0, 694.0, 1032.0, 106.0], scroll, Some([300.0, 767.0, 1.5, 31.0]), None, 62.0, 0.85, false, false));
            assert_eq!(r.state, BandState::Deferred);

            // Later at t=2.0s, Right key runs in editor.
            assert!(r.on_editor_key(true, false));
            let seq = r.awaiting_caret().expect("awaiting caret measurement");
            let (_, rebuild, _) = r.poll(true, 2.0167, 400.0, scroll[0], reduced);
            assert!(rebuild, "first poll forces view rebuild even for no-op Right at EOF");

            // Layout reports caret at y=767..798 (104 pt below band top 694).
            let act = r.on_caret_layout(seq, [300.0, 767.0, 1.5, 31.0], [248.0, 41.0, 1032.0, 759.0], 106.0, scroll, 2.0167, false, reduced);
            let want = if reduced { RevealAction::Instant(608.0) } else { RevealAction::Glide(608.0) };
            assert_eq!(act, want);
            if reduced {
                assert_eq!(r.state, BandState::Idle);
            } else {
                assert!(matches!(r.state, BandState::Gliding { target, recheck: false, .. } if (target - 608.0).abs() < 0.01));
            }
        }
    }

    /// REVEALWEL: rapid keystrokes arriving mid-glide do not re-trigger AwaitCaret against a
    /// stale caret rect (preventing the 4.72 pt overshoot), and re-check cleanly once settled.
    #[test]
    fn rapid_typing_burst_during_glide_never_overshoots() {
        let mut r = Reveal::default();
        let scroll = [504.0, 1200.0, 759.0, 1032.0];
        assert!(r.on_editor_key(true, false));
        let seq = r.awaiting_caret().unwrap();
        let act = r.on_caret_layout(seq, [300.0, 767.0, 1.5, 31.0], [248.0, 41.0, 1032.0, 759.0], 106.0, scroll, 1.30, false, false);
        assert_eq!(act, RevealAction::Glide(608.0));

        // 1 frame later, spring moved scroll by 4.72 pt while further characters arrive.
        assert!(!r.on_editor_key(true, false), "mid-glide key must not re-arm AwaitCaret");
        assert!(r.awaiting_caret().is_none());
        let (act2, rebuild2, wake2) = r.poll(true, 1.3167, 400.0, 508.72, false);
        assert_eq!(act2, RevealAction::None);
        assert!(!rebuild2 && wake2);

        // Once scroll settles at exact 608.0, poll schedules one steady recheck.
        let (act3, rebuild3, _) = r.poll(true, 1.60, 400.0, 608.0, false);
        assert_eq!(act3, RevealAction::None);
        assert!(rebuild3);
        let seq2 = r.awaiting_caret().expect("steady post-glide recheck");
        // Same-line caret is now at 663..694 (right at band_top 694) -> no extra movement.
        let act4 = r.on_caret_layout(seq2, [300.0, 663.0, 1.5, 31.0], [248.0, 41.0, 1032.0, 759.0], 106.0, [608.0, 1200.0, 759.0, 1032.0], 1.60, false, false);
        assert_eq!(act4, RevealAction::None);
        assert_eq!(r.state, BandState::Idle);
    }

    /// Post-typing Save: `Enter` at the bottom triggers `follow_caret()`, which must NOT be
    /// misclassified as a hand swipe, so `Ctrl+S` 0.06s later lifts the caret cleanly above the band.
    #[test]
    fn post_typing_save_is_not_blocked_by_follow_caret() {
        let mut r = Reveal::default();
        // t=0.55s: Enter runs in editor -> marks editor_key_frame.
        assert!(!r.on_editor_key(false, false));
        // Same frame: Rune's follow_caret() moves doc-scroll with by_hand=true.
        let mut sig = r.signal.get();
        assert!(!sig.on_scroll_offset(true, true, 0.55), "editor follow_caret is not a hand swipe");
        r.signal.set(sig);

        // t=0.627s (0.077s after Enter, < HAND_STILL): Ctrl+S shows toast band.
        let scroll = [564.0, 1200.0, 759.0, 1032.0];
        let caret = Some([300.0, 707.0, 1.5, 31.0]);
        assert!(r.on_band_layout([248.0, 694.0, 1032.0, 106.0], scroll, caret, None, 62.0, 0.627, false, false));

        // After LIFT_SETTLE_FRAMES polls with no hand scroll, glide fires to 608.0.
        let mut fired = RevealAction::None;
        for i in 1..=4 {
            let (act, _, _) = r.poll(true, 0.627 + i as f64 * 0.0167, 400.0, scroll[0], false);
            if matches!(act, RevealAction::Glide(_)) {
                fired = act;
                break;
            }
        }
        assert_eq!(fired, RevealAction::Glide(608.0));
    }

    /// Invariant: nothing moves when the caret is already above the band (or out of view),
    /// or when the band shrinks.
    #[test]
    fn invariant_caret_above_band_never_scrolls() {
        let mut r = Reveal::default();
        let scroll = [564.0, 1200.0, 759.0, 1032.0];
        // Caret at y=300..331 is well above band top 694.
        assert!(!r.on_band_layout([248.0, 694.0, 1032.0, 106.0], scroll, Some([300.0, 300.0, 1.5, 31.0]), None, 62.0, 1.0, false, false));
        assert_eq!(r.state, BandState::Idle);

        // Typing while caret is above band produces RevealAction::None.
        assert!(r.on_editor_key(true, false));
        let seq = r.awaiting_caret().unwrap();
        let act = r.on_caret_layout(seq, [300.0, 300.0, 1.5, 31.0], [248.0, 41.0, 1032.0, 759.0], 106.0, scroll, 1.0, false, false);
        assert_eq!(act, RevealAction::None);

        // Caret out of view below (y=850..881) produces no band lift either.
        let mut r2 = Reveal::default();
        assert!(!r2.on_band_layout([248.0, 694.0, 1032.0, 106.0], scroll, Some([300.0, 850.0, 1.5, 31.0]), None, 62.0, 1.0, false, false));
        assert_eq!(r2.state, BandState::Idle);
    }
}
