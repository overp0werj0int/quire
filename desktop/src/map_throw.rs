//! The map's throw (MAP-033): a touchpad swipe over the canvas coasts on when the fingers lift,
//! as a page and the kanban board do (RD-27, decision 0012). Rune throws every bounded scroll
//! viewport natively (`runtime/input/throw.rs`, `runtime/scroll/fling.rs`), including Preview,
//! Source, and Board (`board-scroll`, `board-col-scroll:*`). Because the map is an infinite 2D
//! zoomable canvas driven via `on_wheel`, it runs the exact same Chrome physics here: least-squares
//! velocity over the last three frames, `PhysicsBasedFlingCurve` (with first-step frame clamp),
//! Wayland `touchpad_hold` stop, and `FlingBooster` (a quick second swipe within 50 ms adds the
//! stopped fling's start speed and grows the travel bound by the same factor).

use std::collections::VecDeque;

use rune::{GesturePhase, Mods};

/// Platforms whose trackpads coast by themselves (macOS reports momentum as wheel deltas).
const THROWS: bool = !cfg!(target_os = "macos");
/// Scroll frames the release velocity is fitted to (`kPointerScrollDataSetMaxSize`).
const FRAMES: usize = 3;
/// A lift more than this long after the last frame throws nothing (`kFlingStartTimeoutMs`), s.
const START_TIMEOUT: f64 = 0.2;
/// A second swipe adds to a fling when it begins within this of the stop, and keeps each of its
/// movements within this of the last (`kFlingBoostTimeoutDelay`), seconds.
const BOOST_WINDOW: f64 = 0.05;
/// Both flings move at least this fast to add up (`kMinBoostFlingSpeedSquare`), points a second.
const BOOST_FLING_SPEED: f64 = 350.0;
/// The second swipe moves at least this fast between its movements
/// (`kMinBoostTouchScrollSpeedSquare`), points a second.
const BOOST_SCROLL_SPEED: f64 = 150.0;
/// Chromium's `kDefaultPhysicalDeceleration` at 96 points an inch, points per s².
const DECELERATION: f64 = 2.7559e-5 * 96.0 * 1e6;
/// Deceleration that sets the duration (`kDefaultPixelDeceleration`), points per s².
const DURATION_DECELERATION: f64 = 2300.0;
/// The longest a fling runs (`kMaxCurveDurationForFling`), seconds.
const MAX_DURATION: f64 = 2.0;
/// How many canvas extents a fling may travel at most, before boost (`kDefaultBoundsMultiplier`).
const BOUNDS: f64 = 3.0;
const P1: (f64, f64) = (0.2, 1.0);
const P2: (f64, f64) = (0.55, 1.0);
/// First-step timestamp clamp matching Chromium's `kMaxMicrosecondsFromFlingTimestampToFirstProgress`.
const FRAME: f64 = 1.0 / 60.0;

type V2 = (f64, f64);

fn dot(a: V2, b: V2) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn len2(a: V2) -> f64 {
    dot(a, a)
}

#[derive(Clone, Copy)]
struct Frame {
    d: V2,
    /// Seconds since the frame before.
    dt: f64,
}

/// Whether a fling can still be added to, matching Chromium's `FlingBooster` and Rune's
/// `runtime/input/throw.rs`.
#[derive(Clone, Copy, Default, PartialEq)]
enum Boost {
    #[default]
    Off,
    /// A fling runs that left at `start` (pan points a second) with `mods` held.
    Running { start: V2, mods: Mods },
    /// A fling stopped while moving at `now`: a release in `start`'s direction by `until` adds
    /// `start` to its own. `last` is when the swipe since last moved.
    Open { start: V2, now: V2, mods: Mods, until: f64, last: Option<f64> },
}

impl Boost {
    fn at(self, t: f64) -> Self {
        match self {
            Boost::Open { until, .. } if t > until => Boost::Off,
            boost => boost,
        }
    }

    fn stopped(self, t: f64, now: V2) -> Self {
        match self.at(t) {
            Boost::Running { start, mods } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: None },
            Boost::Open { start, now, mods, .. } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: None },
            Boost::Off => Boost::Off,
        }
    }

    fn began(self, t: f64) -> Self {
        match self.at(t) {
            Boost::Open { start, now, mods, last, .. } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last },
            boost => boost,
        }
    }

    fn moved(self, t: f64, d: V2) -> Self {
        let Boost::Open { start, now, mods, last, .. } = self.at(t) else {
            return self.at(t);
        };
        if dot(start, d) <= 0.0 {
            return Boost::Off;
        }
        if let Some(last) = last {
            let gap = t - last;
            if gap >= 0.001 && len2((d.0 / gap, d.1 / gap)) < BOOST_SCROLL_SPEED * BOOST_SCROLL_SPEED {
                return Boost::Off;
            }
        }
        Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: Some(t) }
    }

    fn released(self, t: f64, v: V2, mods: Mods) -> (V2, f64, Self) {
        let min = BOOST_FLING_SPEED * BOOST_FLING_SPEED;
        let boosted = match self.at(t) {
            Boost::Open { start, now, mods: held, .. } if held == mods && dot(start, v) > 0.0 && len2(now) >= min && len2(v) >= min => {
                (v.0 + start.0, v.1 + start.1)
            }
            _ => v,
        };
        let peak = |v: V2| v.0.abs().max(v.1.abs());
        let boost = if peak(v) > 0.0 { peak(boosted) / peak(v) } else { 1.0 };
        (boosted, boost, if len2(boosted) > 0.0 { Boost::Running { start: boosted, mods } } else { Boost::Off })
    }
}

/// What the map's swipes know between frames, and the coast running now.
#[derive(Default)]
pub struct Throw {
    /// The latest finger frames, newest first.
    frames: VecDeque<Frame>,
    /// Platform time of the last frame, and of the last that moved.
    last: Option<f64>,
    moved: f64,
    boost: Boost,
    coast: Option<Coast>,
}

struct Coast {
    /// The unit direction the camera's pan travels.
    dir: V2,
    fling: Fling,
    /// App time of the last step, app time at start, and platform time at start.
    at: f64,
    started_at: f64,
    started_plat: f64,
}

impl Coast {
    fn velocity(&self) -> V2 {
        let v = self.fling.velocity();
        (self.dir.0 * v, self.dir.1 * v)
    }
}

impl Throw {
    /// A finger frame `d` (pan points) at platform time `t`. It stops any running coast first,
    /// opening the boost window if one was active.
    pub fn moved(&mut self, d: (f32, f32), phase: GesturePhase, t: f64) {
        if !THROWS {
            return;
        }
        if phase == GesturePhase::Began && self.frames.is_empty() {
            self.last = None;
        }
        let dt = self.gap(t);
        if let Some(c) = self.coast.take() {
            self.boost = self.boost.stopped(t, c.velocity());
        } else if matches!(self.boost, Boost::Running { .. }) {
            self.boost = Boost::Off;
        }
        if phase == GesturePhase::Began {
            self.boost = self.boost.began(t);
        }
        let d = (f64::from(d.0), f64::from(d.1));
        if d != (0.0, 0.0) {
            self.boost = self.boost.moved(t, d);
            self.moved = t;
        }
        if self.frames.len() == FRAMES {
            self.frames.pop_back();
        }
        self.frames.push_front(Frame { d, dt });
    }

    /// A wheel step that is no finger (a mouse notch): it throws nothing.
    pub fn stepped(&mut self) {
        self.frames.clear();
        self.last = None;
        self.coast = None;
        self.boost = Boost::Off;
    }

    /// The fingers lifted at platform time `t`: the coast starts at app time `now`, in a canvas
    /// `extent` points along the swipe's longer axis. False when it would not move.
    pub fn released(&mut self, t: f64, now: f64, extent: [f32; 2], mods: Mods) -> bool {
        if !THROWS {
            return false;
        }
        let stop = self.gap(t);
        let raw = self.velocity(stop);
        self.last = None;
        let (v, boost, state) = self.boost.released(t, raw, mods);
        self.boost = state;
        let speed = v.0.hypot(v.1);
        if speed <= 0.0 {
            self.coast = None;
            return false;
        }
        let along = if v.0.abs() > v.1.abs() { extent[0] } else { extent[1] };
        self.coast = Fling::new(speed, f64::from(along), boost, t - self.moved).map(|fling| Coast {
            dir: (v.0 / speed, v.1 / speed),
            fling,
            at: now,
            started_at: now,
            started_plat: t,
        });
        if self.coast.is_none() {
            self.boost = Boost::Off;
        }
        self.coast.is_some()
    }

    /// Fingers landed on the trackpad (Wayland hold gesture) at app time `app_now`: stops a
    /// running coast where it is and opens the 50 ms boost window, matching Rune's `touchpad_hold`.
    pub fn hold(&mut self, app_now: f64) {
        if !THROWS {
            return;
        }
        let Some(c) = self.coast.take() else {
            if matches!(self.boost, Boost::Running { .. }) {
                self.boost = Boost::Off;
            }
            return;
        };
        let t = c.started_plat + (app_now - c.started_at).max(0.0);
        self.frames.clear();
        self.last = None;
        self.boost = self.boost.stopped(t, c.velocity());
    }

    /// Hard stop from a drag, pinch, or camera glide: ends coast and boost state.
    pub fn stop(&mut self) {
        self.coast = None;
        self.boost = Boost::Off;
    }

    /// The pan the coast adds by app time `now`, and whether it runs on; None when none runs.
    pub fn step(&mut self, now: f64) -> Option<([f32; 2], bool)> {
        let c = self.coast.as_mut()?;
        let (d, running) = c.fling.step((now - c.at).max(0.0));
        c.at = now;
        let pan = [(c.dir.0 * d) as f32, (c.dir.1 * d) as f32];
        if !running {
            self.coast = None;
            if matches!(self.boost, Boost::Running { .. }) {
                self.boost = Boost::Off;
            }
        }
        Some((pan, running))
    }

    fn gap(&mut self, t: f64) -> f64 {
        let dt = self.last.map_or(0.0, |last| (t - last).max(0.0));
        self.last = Some(t);
        dt
    }

    /// The fingers' velocity when they left `stop` seconds after the last frame: a least-squares
    /// fit to the last three frames, as Chromium fits it.
    fn velocity(&mut self, stop: f64) -> V2 {
        let frames: Vec<Frame> = self.frames.drain(..).collect();
        if frames.is_empty() || stop > START_TIMEOUT {
            return (0.0, 0.0);
        }
        if let [f] = frames[..] {
            // Chromium's own: a product, not a quotient, so a single frame barely throws.
            let dt = f.dt + stop;
            return (f.d.0 * dt, f.d.1 * dt);
        }
        let (mut time, mut x, mut y) = (stop, 0.0, 0.0);
        let (mut tt, mut ts, mut tx, mut ty, mut xs, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for f in &frames {
            time += f.dt;
            x += f.d.0;
            y += f.d.1;
            tt += time * time;
            ts += time;
            tx += time * x;
            ty += time * y;
            xs += x;
            ys += y;
        }
        let n = frames.len() as f64;
        let det = n * tt - ts * ts;
        if det == 0.0 {
            return if ts != 0.0 { (x / ts, y / ts) } else { (0.0, 0.0) };
        }
        ((n * tx - ts * xs) / det, (n * ty - ts * ys) / det)
    }
}

/// Chromium's `PhysicsBasedFlingCurve` on one axis, matching `rune/src/runtime/scroll/fling.rs`.
struct Fling {
    distance: f64,
    duration: f64,
    ease: Bezier,
    t: f64,
    at: f64,
    stepped: bool,
}

impl Fling {
    /// A fling released at `speed` points a second in an `extent` point window, boosted `boost`
    /// times, whose last movement came `lead` seconds before the lift.
    fn new(speed: f64, extent: f64, boost: f64, lead: f64) -> Option<Self> {
        let bound = (extent * boost * BOUNDS).floor();
        if !(speed > 0.0 && bound > 0.0) {
            return None;
        }
        let to_stop = speed / DECELERATION;
        let reach = ((speed - DECELERATION * to_stop * 0.5) * to_stop).min(bound);
        let duration = MAX_DURATION.min(speed / DURATION_DECELERATION);
        let slope = speed * duration / reach;
        let p1 = if slope * P1.0 < 1.0 { (P1.0, P1.0 * slope) } else { (P1.1 / slope, P1.1) };
        Some(Fling { distance: reach, duration, ease: Bezier::new(p1, P2), t: lead.max(0.0), at: 0.0, stepped: false })
    }

    fn step(&mut self, dt: f64) -> (f64, bool) {
        self.t += dt;
        if !self.stepped {
            self.stepped = true;
            if self.t >= 2.0 * FRAME {
                self.t = FRAME;
            }
        }
        let running = self.t < self.duration;
        let to = if running { self.distance * self.ease.solve(self.t / self.duration) } else { self.distance };
        let d = to - self.at;
        self.at = to;
        (d, running)
    }

    fn velocity(&self) -> f64 {
        if self.t >= self.duration {
            return 0.0;
        }
        self.distance / self.duration * self.ease.slope(self.t / self.duration)
    }
}

/// A CSS cubic Bézier easing from (0, 0) to (1, 1), solved as Chromium's `gfx::CubicBezier`.
struct Bezier {
    a: V2,
    b: V2,
    c: V2,
}

impl Bezier {
    fn new(p1: V2, p2: V2) -> Self {
        let c = (3.0 * p1.0, 3.0 * p1.1);
        let b = (3.0 * (p2.0 - p1.0) - c.0, 3.0 * (p2.1 - p1.1) - c.1);
        Bezier { a: (1.0 - c.0 - b.0, 1.0 - c.1 - b.1), b, c }
    }

    fn x(&self, t: f64) -> f64 {
        ((self.a.0 * t + self.b.0) * t + self.c.0) * t
    }

    fn y(&self, t: f64) -> f64 {
        ((self.a.1 * t + self.b.1) * t + self.c.1) * t
    }

    fn dx(&self, t: f64) -> f64 {
        (3.0 * self.a.0 * t + 2.0 * self.b.0) * t + self.c.0
    }

    fn dy(&self, t: f64) -> f64 {
        (3.0 * self.a.1 * t + 2.0 * self.b.1) * t + self.c.1
    }

    fn t_at(&self, x: f64) -> f64 {
        const EPSILON: f64 = 1e-7;
        let mut t = x;
        for _ in 0..4 {
            let off = self.x(t) - x;
            if off.abs() < EPSILON {
                return t;
            }
            let d = self.dx(t);
            if d.abs() < EPSILON {
                break;
            }
            t -= off / d;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        t = x;
        while lo < hi {
            let at = self.x(t);
            if (at - x).abs() < EPSILON {
                return t;
            }
            if x > at { lo = t } else { hi = t }
            t = (lo + hi) * 0.5;
        }
        t
    }

    fn solve(&self, x: f64) -> f64 {
        self.y(self.t_at(x.clamp(0.0, 1.0)))
    }

    fn slope(&self, x: f64) -> f64 {
        let t = self.t_at(x.clamp(0.0, 1.0));
        let dx = self.dx(t);
        if dx.abs() < 1e-12 { 0.0 } else { self.dy(t) / dx }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quick 3-frame flick after an idle pause must not inherit the idle gap from the previous
    /// gesture as frame 0's `dt` (which otherwise crushes the fitted release speed to near zero).
    #[test]
    fn quick_three_frame_flick_after_idle_pause_keeps_full_speed() {
        let mut throw = Throw::default();
        let mods = Mods::default();
        // First swipe ends at t = 10.9827 s.
        throw.moved((-20.0, 0.0), GesturePhase::Began, 10.950);
        throw.moved((-20.0, 0.0), GesturePhase::Moved, 10.960);
        throw.moved((-20.0, 0.0), GesturePhase::Moved, 10.970);
        assert!(throw.released(10.9827, 1.0, [1200.0, 800.0], mods));
        throw.stop();

        // 1.236 s idle pause, then a fast 3-frame flick (19.8 + 24.0 + 33.2 over ~40 ms).
        throw.moved((19.8, 0.0), GesturePhase::Began, 12.2194);
        throw.moved((24.0, 0.0), GesturePhase::Moved, 12.2345);
        throw.moved((33.2, 0.0), GesturePhase::Moved, 12.2588);
        assert!(throw.released(12.2995, 2.2, [1200.0, 800.0], mods));

        let mut total_x = 0.0f32;
        let mut t = 2.2;
        for _ in 0..180 {
            t += 1.0 / 60.0;
            let Some((pan, running)) = throw.step(t) else { break };
            total_x += pan[0];
            if !running {
                break;
            }
        }
        assert!(total_x > 400.0, "3-frame flick after idle pause should coast cleanly, got {total_x}");
    }
}
