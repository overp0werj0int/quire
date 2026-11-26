//! Shared chrome building blocks: `Control` keyboard activation trait, exact-width `label` and
//! `padded` text helpers, truncated-name tooltips (`name_tip`), brand marks (`MARK`, `MARK_ON_INK`),
//! and platform window dots (`dots`, `DOTS_W`).

use rune::{Color, El, Icon, Key, KeyEvent, Mods, TextStyle, Tx, el, row, text};

use crate::app::App;
use crate::theme::Pal;
use crate::ui::styles::fixed_advance;

macro_rules! svg {
    ($name:ident, $view:expr, $body:expr) => {
        pub static $name: Icon = Icon { name: stringify!($name), view: $view, body: $body };
    };
}

// The logo's mark: three sheets in a stack, the violet one in front (assets/brand).
svg!(MARK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#e1dbcf"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#c9bfae"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#7a4fba"/>"##);
svg!(MARK_ON_INK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#3a352e"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#5f584e"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#b597e8"/>"##);

/// A one-line text. Monospaced text gets its exact CSS width (0.6 em a character), because
/// Rune rounds text boxes up to device pixels and those fractions add up along a row.
pub fn label(s: impl Into<String>, style: TextStyle, c: Color) -> El {
    let s: String = s.into();
    let mono = fixed_advance(style.family);
    let n = s.chars().count();
    let ascii = in_mono_face(&s);
    let e = text(s).font(style).color(c);
    if mono && n > 0 && ascii { e.w(n as f32 * 0.6 * style.size) } else { e }
}

fn in_mono_face(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii() || "↑↓←→↵·…“”‘’".contains(c))
}

/// A label with padding inside its exact width (`extra` adds borders that take room).
pub fn padded(s: impl Into<String>, style: TextStyle, c: Color, px: f32, py: f32, extra: f32) -> El {
    let s: String = s.into();
    let mono = fixed_advance(style.family);
    let n = s.chars().count();
    let ascii = in_mono_face(&s);
    let e = text(s).font(style).color(c).px(px).py(py);
    if mono && n > 0 && ascii { e.w(n as f32 * 0.6 * style.size + 2.0 * px + extra) } else { e }
}

/// Controls the keyboard reaches, as Settings has them.
pub trait Control {
    /// Tab stops on it, and a press, Space or Return runs `run`. Focused from the keyboard, it
    /// shows the view's focus ring (`ui::view`).
    fn control(self, run: impl Fn(&mut App, &mut Tx) + 'static) -> El;
    /// One of a group of choices, as a radio: Tab stops on the chosen one, and the arrow keys
    /// move to the others and choose them. Its container sets `focus_group`.
    fn choice(self, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> El;
}

impl Control for El {
    fn control(self, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
        let run = std::rc::Rc::new(run);
        let on_return = run.clone();
        self.focusable(true)
            .on_press_tx(move |s: &mut App, tx| {
                s.touch_activity();
                run(s, tx);
            })
            .on_key(move |s: &mut App, ev: &KeyEvent, tx| {
                let enter = ev.key == Key::Enter && ev.mods == Mods::default();
                if enter {
                    on_return(s, tx);
                }
                enter
            })
    }

    fn choice(self, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
        self.role("radio").checked(on).control(run).focus_selected(on)
    }
}

/// How wide a tooltip's text grows before it wraps (`ui::view` sets it).
pub const TIP_WRAP: f32 = 360.0;

/// The widest line of a cut name's tooltip, a little inside `TIP_WRAP`.
const TIP_W: f32 = TIP_WRAP - 20.0;

/// The tooltip of a cut note or folder name (C7), in three lines at most.
pub fn name_tip(path: &str) -> String {
    let style = crate::ui::styles::tooltip();
    let size = |t: &str| rune::text::measure(t, &[], style, Some(TIP_W));
    let line = size("A").1;
    let cut = |t: &str, head: bool, fits: &dyn Fn(&str) -> bool| -> String {
        if fits(t) {
            return t.to_string();
        }
        let chars: Vec<char> = t.chars().collect();
        let make = |n: usize| -> String {
            let part: String = if head { chars[chars.len() - n..].iter().collect() } else { chars[..n].iter().collect() };
            if head { format!("…{part}") } else { format!("{}…", part.trim_end()) }
        };
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if fits(&make(mid)) { lo = mid } else { hi = mid - 1 }
        }
        make(lo)
    };
    let one = |t: &str| size(t).1 <= line + 0.5;
    let two = |t: &str| size(t).1 <= 2.0 * line + 0.5;
    let three = |t: &str| size(t).1 <= 3.0 * line + 0.5;
    let name = crate::md::base(path);
    match crate::md::dir(path) {
        dir if !dir.is_empty() && two(name) => format!("{name}\n{}", cut(&format!("in {dir}"), true, &one)),
        _ => cut(name, false, &three),
    }
}

/// Hairline vertical separator between button groups in toolbars and bars.
pub fn bar_sep(p: &Pal, key: impl Into<String>, h: f32) -> El {
    el().key(key.into()).w(1.0).h(h).fill(p.line)
}

/// The width `dots` takes: none where the system or the window manager draws window buttons
/// elsewhere.
pub const DOTS_W: f32 = if cfg!(any(target_os = "windows", target_os = "linux")) { 0.0 } else { 52.0 };

/// Window dots (macOS reserves traffic-light space; Linux/Windows emit an empty placeholder).
pub fn dots(p: &Pal) -> El {
    if cfg!(target_os = "macos") {
        return el().key("dots").size(52.0, 12.0);
    }
    if cfg!(any(target_os = "windows", target_os = "linux")) {
        return el().key("dots");
    }
    let dot = |key: &str, help: &str, run: fn(&mut App, &mut Tx)| {
        el().key(key).size(12.0, 12.0).radius(6.0).fill(p.mark).label(help).help(help).role("button").on_press_tx(move |s: &mut App, tx| run(s, tx))
    };
    row()
        .key("dots")
        .gap(8.0)
        .center_y()
        .child(dot("win-close", "Close", |s, tx| {
            s.flush_all();
            tx.close_window();
        }))
        .child(dot("win-min", "Minimize", |_, tx| tx.minimize()))
        .child(dot("win-max", "Maximize", |_, tx| tx.toggle_maximized()))
}

#[cfg(test)]
mod tests {
    use super::{TIP_W, name_tip};

    /// C7: a cut name's tooltip is the name, not the file path, in three lines at most.
    #[test]
    fn a_cut_names_tooltip_is_three_lines_at_most() {
        let style = crate::ui::styles::tooltip();
        let line = rune::text::measure("A", &[], style, None).1;
        let lines = |t: &str| (rune::text::measure(t, &[], style, Some(TIP_W)).1 / line).round();
        let deep = "Projects/Archive/2025/Quarterly reviews and retrospectives for the platform team/Q3";
        let tip = name_tip(&format!("{deep}/Plan.md"));
        let (name, folder) = tip.split_once('\n').expect("the folder on its own line");
        assert_eq!(name, "Plan");
        assert!(folder.starts_with('…') && folder.ends_with("/Q3"), "{folder}");
        let grow = |n: f32| {
            let mut t = "A moderately long but perfectly normal note title".to_string();
            while lines(&format!("{t} again")) <= n {
                t.push_str(" again");
            }
            t
        };
        let long = grow(2.0);
        assert_eq!(lines(&long), 2.0, "{long}");
        assert_eq!(name_tip(&format!("Drafts/{long}.md")), format!("{long}\nin Drafts"), "a two-line name keeps its folder");
        let long = grow(3.0);
        assert_eq!(lines(&long), 3.0, "{long}");
        assert_eq!(name_tip(&format!("Drafts/{long}.md")), long, "a three-line name shows whole");
        let longer = format!("{long}, and once more with feeling, because some people name their notes with the whole first paragraph");
        let tip = name_tip(&format!("Drafts/{longer}.md"));
        assert!(!tip.contains('\n') && !tip.contains("Drafts") && tip.ends_with('…'), "{tip}");
        assert_eq!(lines(&tip), 3.0, "{tip}");
        assert_eq!(name_tip("Plan.md"), "Plan");
    }
}
