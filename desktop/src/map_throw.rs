//! The map's throw (MAP-033): a touchpad swipe over the canvas coasts on when the fingers lift,
//! as a page does (RD-27, decision 0012). Rune throws its own scroll views, but the map pans
//! its camera in an `on_wheel` handler, which takes the swipe from Rune, so the map throws it
//! here with Rune's numbers: the release speed fitted to the swipe's last three frames by their
//! platform times (`runtime/input/throw.rs`), and Chromium's fling curve
//! (`runtime/scroll/fling.rs`). The coast runs along the swipe's direction, at most three
//! canvas extents, and a new swipe, a drag, a pinch or a camera glide stops it.

use std::collections::VecDeque;

/// Scroll frames the release velocity is fitted to (`kPointerScrollDataSetMaxSize`).
const FRAMES: usize = 3;
/// A lift more than this long after the last frame throws nothing (`kFlingStartTimeoutMs`), s.
const START_TIMEOUT: f64 = 0.2;
/// Chromium's `kDefaultPhysicalDeceleration` at 96 points an inch, points per s².
const DECELERATION: f64 = 2.7559e-5 * 96.0 * 1e6;
/// Deceleration that sets the duration (`kDefaultPixelDeceleration`), points per s².
const DURATION_DECELERATION: f64 = 2300.0;
/// The longest a fling runs (`kMaxCurveDurationForFling`), seconds.
const MAX_DURATION: f64 = 2.0;
/// How many canvas extents a fling may travel at most (`kDefaultBoundsMultiplier`).
const BOUNDS: f64 = 3.0;
const P1: (f64, f64) = (0.2, 1.0);
const P2: (f64, f64) = (0.55, 1.0);

#[derive(Clone, Copy)]
struct Frame {
    d: (f64, f64),
    /// Seconds since the frame before.
    dt: f64,
}

/// What the map's swipes know between frames, and the coast running now.
#[derive(Default)]
pub struct Throw {
    /// The latest finger frames, newest first.
    frames: VecDeque<Frame>,
    /// Platform time of the last frame, and of the last that moved.
    last: Option<f64>,
    moved: f64,
    coast: Option<Coast>,
}

struct Coast {
    /// The unit direction the camera's pan travels.
    dir: (f64, f64),
    fling: Fling,
    /// App time of the last step.
    at: f64,
}

impl Throw {
    /// A finger frame `d` (pan points) at platform time `t`. It stops a running coast.
    pub fn moved(&mut self, d: (f32, f32), t: f64) {
        let dt = self.gap(t);
        self.coast = None;
        let d = (f64::from(d.0), f64::from(d.1));
        if d != (0.0, 0.0) {
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
    }

    /// The fingers lifted at platform time `t`: the coast starts at app time `now`, in a canvas
    /// `extent` points along the swipe's longer axis. False when it would not move.
    pub fn released(&mut self, t: f64, now: f64, extent: [f32; 2]) -> bool {
        let stop = self.gap(t);
        let v = self.velocity(stop);
        let speed = v.0.hypot(v.1);
        let along = if v.0.abs() > v.1.abs() { extent[0] } else { extent[1] };
        self.coast = Fling::new(speed, f64::from(along), t - self.moved).map(|fling| Coast { dir: (v.0 / speed, v.1 / speed), fling, at: now });
        self.coast.is_some()
    }

    pub fn stop(&mut self) {
        self.coast = None;
    }

    /// The pan the coast adds by app time `now`, and whether it runs on; None when none runs.
    pub fn step(&mut self, now: f64) -> Option<([f32; 2], bool)> {
        let c = self.coast.as_mut()?;
        let (d, running) = c.fling.step((now - c.at).max(0.0));
        c.at = now;
        let pan = [(c.dir.0 * d) as f32, (c.dir.1 * d) as f32];
        if !running {
            self.coast = None;
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
    fn velocity(&mut self, stop: f64) -> (f64, f64) {
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

/// Chromium's `PhysicsBasedFlingCurve` on one axis, as `rune/src/runtime/scroll/fling.rs` has it.
struct Fling {
    distance: f64,
    duration: f64,
    ease: Bezier,
    t: f64,
    at: f64,
}

impl Fling {
    /// A fling released at `speed` points a second in an `extent` point window, whose last
    /// movement came `lead` seconds before the lift.
    fn new(speed: f64, extent: f64, lead: f64) -> Option<Self> {
        let bound = (extent * BOUNDS).floor();
        if !(speed > 0.0 && bound > 0.0) {
            return None;
        }
        let to_stop = speed / DECELERATION;
        let reach = ((speed - DECELERATION * to_stop * 0.5) * to_stop).min(bound);
        let duration = MAX_DURATION.min(speed / DURATION_DECELERATION);
        let slope = speed * duration / reach;
        let p1 = if slope * P1.0 < 1.0 { (P1.0, P1.0 * slope) } else { (P1.1 / slope, P1.1) };
        Some(Fling { distance: reach, duration, ease: Bezier::new(p1, P2), t: lead.max(0.0), at: 0.0 })
    }

    fn step(&mut self, dt: f64) -> (f64, bool) {
        self.t += dt;
        let running = self.t < self.duration;
        let to = if running { self.distance * self.ease.solve(self.t / self.duration) } else { self.distance };
        let d = to - self.at;
        self.at = to;
        (d, running)
    }
}

/// A CSS cubic Bézier easing from (0, 0) to (1, 1), solved as Chromium's `gfx::CubicBezier`.
struct Bezier {
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
}

impl Bezier {
    fn new(p1: (f64, f64), p2: (f64, f64)) -> Self {
        let c = (3.0 * p1.0, 3.0 * p1.1);
        let b = (3.0 * (p2.0 - p1.0) - c.0, 3.0 * (p2.1 - p1.1) - c.1);
        Bezier { a: (1.0 - c.0 - b.0, 1.0 - c.1 - b.1), b, c }
    }

    fn x(&self, t: f64) -> f64 {
        ((self.a.0 * t + self.b.0) * t + self.c.0) * t
    }

    fn dx(&self, t: f64) -> f64 {
        (3.0 * self.a.0 * t + 2.0 * self.b.0) * t + self.c.0
    }

    fn solve(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..4 {
            let off = self.x(t) - x;
            if off.abs() < 1e-7 {
                return self.y(t);
            }
            let d = self.dx(t);
            if d.abs() < 1e-7 {
                break;
            }
            t -= off / d;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        t = x;
        while lo < hi {
            let at = self.x(t);
            if (at - x).abs() < 1e-7 {
                break;
            }
            if x > at { lo = t } else { hi = t }
            t = (lo + hi) * 0.5;
        }
        self.y(t)
    }

    fn y(&self, t: f64) -> f64 {
        ((self.a.1 * t + self.b.1) * t + self.c.1) * t
    }
}
