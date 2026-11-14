//! Reduced motion: one gate for every animation (PAGE-84). Rune snaps its own springs, flights
//! and scroll glides when the view asks; Quire's own durations go through `ms`. Timers (toast
//! hold, chord timeout, debounces) are not animations and never pass through here.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Quire's own reasons, read once: `QUIRE_REDUCED_MOTION=1`, or Hyprland with
/// `animations:enabled` off. The system setting comes from Rune, per frame.
pub fn forced() -> bool {
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| env_says(std::env::var("QUIRE_REDUCED_MOTION").ok().as_deref()) || hyprland_off())
}

fn env_says(v: Option<&str>) -> bool {
    matches!(v.map(str::trim), Some("1" | "true" | "yes" | "on"))
}

fn hyprland_off() -> bool {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return false;
    }
    std::process::Command::new("hyprctl")
        .args(["getoption", "animations:enabled", "-j"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .is_some_and(|s| hyprland_json_off(&s))
}

/// `hyprctl getoption -j` prints `{"option": "animations:enabled", "int": 0, ...}`.
fn hyprland_json_off(s: &str) -> bool {
    s.split_once("\"int\"").and_then(|(_, r)| r.trim_start().strip_prefix(':')).is_some_and(|r| r.trim_start().starts_with('0'))
}

/// Records this frame's answer, which the view takes from Rune and `forced`.
pub fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn reduced() -> bool {
    ON.load(Ordering::Relaxed)
}

/// An animation's duration in ms, or 0 under reduced motion.
#[cfg_attr(not(test), allow(dead_code))]
pub fn ms(d: f32) -> f32 {
    if reduced() { 0.0 } else { d }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_84_sources_of_reduced_motion() {
        assert!(env_says(Some("1")) && env_says(Some(" true")));
        assert!(!env_says(Some("0")) && !env_says(None) && !env_says(Some("")));
        assert!(hyprland_json_off(r#"{"option": "animations:enabled", "int": 0, "set": true}"#));
        assert!(!hyprland_json_off(r#"{"option": "animations:enabled", "int": 1, "set": true}"#));
        assert!(!hyprland_json_off("hyprctl: no instance"));
        set(true);
        assert_eq!(ms(160.0), 0.0);
        set(false);
        assert_eq!(ms(160.0), 160.0);
    }
}
