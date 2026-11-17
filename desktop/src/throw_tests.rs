//! RD-27: a touchpad swipe on the reader throws the page as Chrome on Linux throws it. The
//! fingers' last frames set the release speed, the page coasts v²/2a on Chrome's fling curve,
//! fingers landing (the Wayland hold) or a new swipe stop it, and a second swipe the same way
//! soon after a stop adds the first fling's speed to its own.
//!
//! Each frame carries its platform time, as winit gives it on Wayland, so frames sent together
//! still throw by their own spacing.

use rune::{GesturePhase, Runtime, WheelEvent};

use crate::app::{App, Mode};
use crate::bench::{DT, LONG, open};
use crate::scroll_tests::ENV;

/// A frame of a swipe every 1/120 s.
const STEP: f64 = 1.0 / 120.0;
/// How far a fling at 2,400 points a second carries the page: v² / 2a, with Chrome's
/// deceleration of 2.7559e-5 inches per ms² at 96 points per inch.
const THROW_2400: f32 = 1088.6;

fn offset(rt: &mut Runtime<App>) -> f32 {
    let mut at = 0.0;
    rt.update_if(|s, _| {
        at = s.reader.scroll.get()[0];
        false
    });
    at
}

fn frame(x: f32, y: f32, d: (f32, f32), phase: GesturePhase, t: f64) -> WheelEvent {
    WheelEvent::new(d.0, d.1).at(x, y).phase(phase).precise(true).time(t)
}

/// A swipe of `n` frames moving `d` each, from platform time `t`, with a display frame after
/// each when `paced`, and the fingers lifting `lift` seconds after the last. Returns the lift's
/// time.
fn swipe(rt: &mut Runtime<App>, at: (f32, f32), d: (f32, f32), n: usize, t: f64, lift: f64, paced: bool) -> f64 {
    for i in 0..n {
        let phase = if i == 0 { GesturePhase::Began } else { GesturePhase::Moved };
        rt.wheel_event(frame(at.0, at.1, d, phase, t + i as f64 * STEP));
        if paced {
            rt.frame(STEP);
        }
    }
    let end = t + (n - 1) as f64 * STEP + lift;
    rt.wheel_event(frame(at.0, at.1, (0.0, 0.0), GesturePhase::Ended, end));
    end
}

/// Frames until the page stops moving, at most 3 s of them.
fn settle(rt: &mut Runtime<App>) -> f32 {
    let mut last = offset(rt);
    let mut still = 0;
    for _ in 0..180 {
        rt.frame(DT);
        let now = offset(rt);
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still == 5 {
            break;
        }
    }
    last
}

fn reader(name: &str, text: &str) -> (Runtime<App>, (f32, f32), std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("quire-throw-{name}-{}", std::process::id()));
    let (mut rt, _) = open(&dir, text, Mode::Preview);
    let at = rt.find(crate::reader::SCROLL).expect("the page is drawn");
    rt.pointer_move(at.0, at.1);
    rt.frame(DT);
    (rt, at, dir)
}

#[test]
fn a_swipe_throws_the_page_its_release_speed_squared_over_twice_the_deceleration() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (mut rt, at, dir) = reader("throw", LONG);
    swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, true);
    let lifted = offset(&mut rt);
    assert!((lifted - 160.0).abs() < 0.5, "the swipe itself moves the page 8 × 20: {lifted}");
    rt.frame(DT);
    rt.frame(DT);
    assert!(offset(&mut rt) > lifted + 40.0, "it leaves at about 2,400 points a second: {}", offset(&mut rt));
    let travel = settle(&mut rt) - lifted;
    assert!((travel - THROW_2400).abs() < 1.0, "it coasts v²/2a: {travel}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frames_that_arrive_together_throw_by_their_own_times() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (mut rt, at, dir) = reader("bunched", LONG);
    swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, false);
    rt.frame(DT);
    let travel = settle(&mut rt) - 160.0;
    assert!((travel - THROW_2400).abs() < 1.0, "as if paced: {travel}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_lift_long_after_the_last_movement_throws_nothing() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (mut rt, at, dir) = reader("late", LONG);
    swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.25, true);
    let lifted = offset(&mut rt);
    assert_eq!(settle(&mut rt), lifted, "250 ms still: past Chrome's 200 ms");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fingers_landing_stop_the_page() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (mut rt, at, dir) = reader("hold", LONG);
    let lift = swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, true);
    for _ in 0..10 {
        rt.frame(DT);
    }
    rt.touchpad_hold(Some(lift + 10.0 * DT));
    let held = offset(&mut rt);
    assert!(held > 160.0 + 100.0, "it was coasting: {held}");
    assert_eq!(settle(&mut rt), held, "it stops where the fingers landed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_swipe_stops_the_page() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (mut rt, at, dir) = reader("reswipe", LONG);
    let lift = swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, true);
    for _ in 0..10 {
        rt.frame(DT);
    }
    let before = offset(&mut rt);
    rt.wheel_event(frame(at.0, at.1, (0.0, 4.0), GesturePhase::Began, lift + 10.0 * DT));
    rt.frame(DT);
    let after = offset(&mut rt);
    assert!((after - (before - 4.0)).abs() < 0.5, "the fling stops and the fingers move it: {before} to {after}");
    assert_eq!(settle(&mut rt), after, "nothing coasts while the fingers rest");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Chrome's fling booster: a swipe that begins within 50 ms of a stop, keeps heading the same
/// way and is released fast adds the stopped fling's start speed to its own. Two 2,400 point a
/// second swipes make a 4,800 point a second throw, which carries four times as far.
#[test]
fn a_quick_second_swipe_the_same_way_adds_to_the_throw() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let throw = |name: &str, after: f64| {
        let (mut rt, at, dir) = reader(name, LONG);
        let lift = swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, true);
        for _ in 0..6 {
            rt.frame(DT);
        }
        let hold = lift + 6.0 * DT;
        rt.touchpad_hold(Some(hold));
        swipe(&mut rt, at, (0.0, -20.0), 8, hold + after, 0.008, true);
        let lifted = offset(&mut rt);
        let travel = settle(&mut rt) - lifted;
        let _ = std::fs::remove_dir_all(&dir);
        travel
    };
    let boosted = throw("boost", 0.03);
    assert!((boosted - 4.0 * THROW_2400).abs() < 4.0, "30 ms after the stop it adds up: {boosted}");
    let alone = throw("noboost", 0.08);
    assert!((alone - THROW_2400).abs() < 1.0, "80 ms after, it throws alone: {alone}");
}

#[test]
fn a_throw_stops_at_the_end_of_the_page() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let text = (1..=40).map(|i| format!("Paragraph {i} of a short note.\n\n")).collect::<String>();
    let (mut rt, at, dir) = reader("end", &text);
    let max = || {
        let mut max = 0.0;
        move |rt: &mut Runtime<App>| {
            rt.update_if(|s, _| {
                max = s.reader.scroll.get()[1];
                false
            });
            max
        }
    };
    let end = max()(&mut rt);
    assert!(end > 400.0 && end < 160.0 + THROW_2400 - 100.0, "the throw would carry past the end: {end}");
    swipe(&mut rt, at, (0.0, -20.0), 8, 10.0, 0.008, true);
    assert_eq!(settle(&mut rt), end, "it lands on the end, not past it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sideways_swipe_throws_a_wide_code_block() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let text = format!("# Wide\n\n```\n{}\n```\n", "wide ".repeat(800));
    let (mut rt, _, dir) = reader("sideways", &text);
    let at = rt.find("scroll").expect("the code block is drawn");
    let code_x = |rt: &Runtime<App>| rt.presented("code").unwrap().0[0];
    let start = code_x(&rt);
    swipe(&mut rt, at, (-20.0, 0.0), 8, 10.0, 0.008, true);
    let lifted = start - code_x(&rt);
    assert!((lifted - 160.0).abs() < 0.5, "the swipe moves the code 8 × 20: {lifted}");
    let mut last = code_x(&rt);
    for _ in 0..180 {
        rt.frame(DT);
        last = code_x(&rt);
    }
    let travel = start - last - 160.0;
    assert!((travel - THROW_2400).abs() < 1.0, "it coasts v²/2a sideways: {travel}");
    let _ = std::fs::remove_dir_all(&dir);
}
