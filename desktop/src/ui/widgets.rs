//! Small pieces the screens share: brand marks, icon buttons, segmented controls and switches.

use rune::{Color, El, Icon, Key, KeyEvent, Mods, Shadow, TextStyle, Tx, el, icon, row, text};

use crate::app::App;
use super::V;
use super::styles::{fixed_advance, literal, sans};
use crate::roles::Roles;
use crate::theme::{self, Pal};

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
    // Only ASCII is sure to come from the mono face; arrows fall back to other fonts.
    if mono && n > 0 && ascii { e.w(n as f32 * 0.6 * style.size) } else { e }
}

/// Whether every character is in the mono face, so it is exactly 0.6 em wide.
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

/// A square icon button: 28 by 28, radius 6, muted, ink on hover.
/// A menu or popup over the window (D4): the window's colour, the hairline edge the palette,
/// Settings and the lock dialog have, and the menu's shadow, whose own 1 point ring is too faint
/// to show on a dark theme. Every menu, popup, toast and floating notice takes it.
/// How far a scrolling sheet's page fades out at its edges (`faded`). Its scroll area takes it
/// as its scroll margin, so a row scrolled into view clears the fade.
pub const SCROLL_FADE: f32 = 28.0;

/// A scrolling sheet's page with its fades, for Settings, help and the palette's list alike:
/// at the bottom edge while more of the page is below, and at the top edge once it is scrolled.
/// Each fade holds to its edge and shows as far as it is held, so a page that fits shows
/// none, and the first 12 points of scroll bring one in whole. `name` keys them:
/// `{name}-page`, `{name}-fade` and `{name}-fade-top`, each fade in a `-hold` box.
pub fn faded(name: &str, bg: Color, page: El) -> El {
    let fade = |key: String, top: bool| {
        let stops = if top { [(0.0, bg), (1.0, bg.alpha(0.0))] } else { [(0.0, bg.alpha(0.0)), (1.0, bg)] };
        el().key(key).pin(Some(0.0), Some(if top { 0.0 } else { -SCROLL_FADE }), Some(0.0), None).h(SCROLL_FADE).linear_gradient(std::f32::consts::PI, stops).inert().stuck_reveal(12.0)
    };
    let top = el().key(format!("{name}-fade-top-hold")).w_full().h(0.0).sticky_top(0.0).paint_order(1).child(fade(format!("{name}-fade-top"), true));
    let bottom = el().key(format!("{name}-fade-hold")).w_full().h(0.0).sticky_bottom(0.0).child(fade(format!("{name}-fade"), false));
    rune::col().key(format!("{name}-page")).w_full().child(top).child(page).child(bottom)
}

/// The corner of a bar floating over the note: the formatting toolbar and the find bar, which
/// share a row. omarky's find bar has it (RD-53, WR-122).
pub const BAR_RADIUS: f32 = 6.0;

/// A floating bar's corner and edge, one for the formatting toolbar and the find bar: the
/// radius and a 1 point hairline.
pub fn bar_edge(e: El, c: &Roles) -> El {
    e.radius(BAR_RADIUS).border(1.0, c.hairline)
}

pub fn floating(e: El, c: &Roles) -> El {
    let [a, b] = theme::menu_shadows();
    e.fill(c.bg).border(1.0, c.hairline).box_shadow(a).box_shadow(b)
}

pub fn icon_button(p: &Pal, key: &str, glyph: &'static Icon, size: f32, help: &str) -> El {
    row()
        .key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .hover(p.hover)
        .hover_group()
        .label(help)
        .help(help)
        .child(icon(glyph, size).color(p.mute).group_color(p.ink))
}

/// [`icon_button`] that shows as active, with the hover fill, a hairline and ink, while its
/// feature is on (RD-35).
pub fn toggle_button(p: &Pal, key: &str, glyph: &'static Icon, size: f32, help: &str, on: bool) -> El {
    if !on {
        return icon_button(p, key, glyph, size, help).role("switch").checked(false);
    }
    row()
        .key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .fill(p.hover)
        .border(1.0, p.line)
        .role("switch")
        .checked(true)
        .label(help)
        .help(help)
        .child(icon(glyph, size).color(p.ink))
}

fn seg_shadow(on: bool) -> Vec<Shadow> {
    if on { theme::seg_shadows().to_vec() } else { Vec::new() }
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
        self.focusable(true).on_press_tx(move |s: &mut App, tx| run(s, tx)).on_key(move |s: &mut App, ev: &KeyEvent, tx| {
            // Rune presses a focused control on Space. Return would go to a default button, so
            // it is taken here.
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

/// One button of a segmented control.
pub struct Seg {
    pub label: String,
    pub on: bool,
    pub run: Box<dyn Fn(&mut App, &mut Tx)>,
}

pub fn seg(label: impl Into<String>, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> Seg {
    Seg { label: label.into(), on, run: Box::new(run) }
}

/// The design's segmented control: a tinted track with raised buttons.
pub fn segmented(p: &Pal, key: &str, segs: Vec<Seg>, h: f32, pad_x: f32, radius: f32, style: TextStyle, track: Color, track_radius: f32) -> El {
    let mut r = row().key(key).gap(2.0).padding(2.0).radius(track_radius).fill(track).role("radiogroup").focus_group();
    for (i, sg) in segs.into_iter().enumerate() {
        let on = sg.on;
        let run = sg.run;
        let mut b = row()
            .key(format!("{key}-{i}"))
            .h(h)
            .px(pad_x)
            .center()
            .shrink()
            .radius(radius)
            .fill(if on { p.bg } else { Color::TRANSPARENT })
            .label(sg.label.clone())
            .child(label(sg.label, style, if on { p.ink } else { p.mute }).truncate());
        for sh in seg_shadow(on) {
            b = b.box_shadow(sh);
        }
        r = r.child(b.choice(on, move |s, tx| run(s, tx)));
    }
    r
}

/// The settings segmented control: 26 tall, 12.5 px Instrument Sans, on the side colour.
pub fn settings_seg(v: super::V, key: &str, segs: Vec<Seg>) -> El {
    segmented(v.p, key, segs, 26.0, settings_seg_pad(v), 6.0, sans(12.5, 400.0), v.p.side, 8.0)
}

/// A settings segment's side padding: less when the page is narrow and its rows stack, so a
/// control of three long names still fits. A segment that cannot fit shortens its name.
fn settings_seg_pad(v: super::V) -> f32 {
    if super::settings::stacked(v) { 7.0 } else { 12.0 }
}

/// The settings segmented control for literal names (MCP transport).
pub fn settings_seg_literal(v: super::V, key: &str, segs: Vec<Seg>) -> El {
    segmented(v.p, key, segs, 26.0, settings_seg_pad(v), 6.0, literal(12.0), v.p.side, 8.0)
}

/// How wide a tooltip's text grows before it wraps (`ui::view` sets it).
pub const TIP_WRAP: f32 = 360.0;

/// The widest line of a cut name's tooltip, a little inside `TIP_WRAP`.
const TIP_W: f32 = TIP_WRAP - 20.0;

/// The tooltip of a cut note or folder name (C7), in three lines at most. A name that fits two
/// lines has its folder on the next, cut at its start; a longer name takes all three, cut with
/// "…" at its end.
pub fn name_tip(path: &str) -> String {
    let style = super::styles::tooltip();
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

/// A switch drawn 50 by 28, as big as the target it takes a press over, its knob 24 across.
/// The pointer anywhere on it gives the track its hover fill, which lands at once and fades
/// over 100 ms as a hover does; Tab rings the track, as every ring sits around what is drawn.
pub fn switch(v: V, key: &str, on: bool, label_text: &str, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let p = v.p;
    let hovered = v.s.hover_switch.as_deref() == Some(key);
    let fill = match (on, hovered) {
        (true, false) => p.button,
        (true, true) => p.button_hover,
        (false, false) => p.mark,
        (false, true) => p.mark.mix(p.ink, SWITCH_HOVER),
    };
    let (rw, rg, rc) = super::focus_ring(p);
    let track = el()
        .key("track")
        .size(SWITCH_W, SWITCH_H)
        .radius(SWITCH_H / 2.0)
        .fill(fill)
        .focus_ring(rw, rg, rc)
        .child(el().key("knob").size(KNOB, KNOB).radius(KNOB / 2.0).fill(Color::WHITE).box_shadow(theme::knob_shadow()).absolute(if on { SWITCH_W - KNOB - KNOB_INSET } else { KNOB_INSET }, KNOB_INSET));
    let own = key.to_string();
    let e = row().key(key).size(SWITCH_W, SWITCH_H).center_y().role("switch").checked(on).label(label_text).on_hover_change(move |s: &mut App, inside, tx| {
        if inside {
            s.hover_switch = Some(own.clone());
            tx.instant();
        } else if s.hover_switch.as_deref() == Some(own.as_str()) {
            s.hover_switch = None;
            tx.color_motion(rune::Spring::new(0.1, 0.0));
        }
    });
    e.control(run).child(track)
}

/// How far an off switch's track moves toward the ink under the pointer.
const SWITCH_HOVER: f32 = 0.2;
/// A switch's track, which is its whole target: as tall as a row's control, wide enough that
/// the knob travels most of its own width.
const SWITCH_W: f32 = 50.0;
const SWITCH_H: f32 = 28.0;
/// The knob, and the track it shows round it.
const KNOB_INSET: f32 = 2.0;
const KNOB: f32 = SWITCH_H - 2.0 * KNOB_INSET;

/// A bordered button such as "Switch folder…" or "Restore".
pub fn outline_button(p: &Pal, key: &str, text_: &str, h: f32, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let e = row()
        .key(key)
        .h(h)
        .px(12.0)
        .center()
        .radius(7.0)
        .border(1.0, p.line)
        .border_layout()
        .hover(p.hover)
        .role("button")
        .label(text_)
        .child(label(text_, sans(12.5, 400.0), p.ink));
    e.control(run)
}

/// The solid purple button.
pub fn primary_button(p: &Pal, key: &str, text_: &str, h: f32, px: f32, size: f32, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let e = row()
        .key(key)
        .h(h)
        .px(px)
        .center()
        .radius(8.0)
        .fill(p.button)
        .hover(p.button_hover)
        .role("button")
        .label(text_)
        .child(label(text_, sans(size, 500.0), p.on_acc));
    e.control(run)
}

/// The width `dots` takes: none where the system or the window manager draws the window's
/// buttons elsewhere.
pub const DOTS_W: f32 = if cfg!(any(target_os = "windows", target_os = "linux")) { 0.0 } else { 52.0 };

/// The design's window dots. On Linux they are the window's close, minimize and maximize
/// buttons. macOS draws its own traffic lights in the same place, so there they leave room.
pub fn dots(p: &Pal) -> El {
    if cfg!(target_os = "macos") {
        return el().key("dots").size(52.0, 12.0);
    }
    // Windows draws its own buttons at the other end of the title bar. On Linux the window
    // manager closes and places the window, and `Ctrl+Q` quits.
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
    use crate::roles::Roles;
    use super::{TIP_W, name_tip};
    use crate::theme::{THEMES, contrast, theme_pal};

    /// D4: the menu's shadow appears only in `floating`, so no menu or popup can take the
    /// shadow without the edge.
    #[test]
    fn menus_take_the_floating_edge() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&ui).unwrap() {
            let path = entry.unwrap().path();
            let Ok(src) = std::fs::read_to_string(&path) else { continue };
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            for (i, line) in src.lines().enumerate().take_while(|(_, l)| !l.starts_with("#[cfg(test)]")) {
                if line.contains("menu_shadows") && !(name == "widgets.rs" && line.contains("let [a, b] = theme::menu_shadows();")) {
                    found.push(format!("ui/{name}:{}", i + 1));
                }
            }
        }
        assert!(found.is_empty(), "the menu's shadow outside widgets::floating:\n{}", found.join("\n"));
    }

    /// D4: the edge shows against the window in every theme, as the modal panels' edge does, and
    /// more than the shadow's own ring, which all but vanished on dark themes.
    #[test]
    fn floating_edge_shows_on_every_theme() {
        for t in THEMES {
            let c = Roles::new(theme_pal(t.id, t.dark));
            let edge = c.bg.mix(c.hairline.alpha(1.0), c.hairline.a);
            assert!(contrast(edge, c.bg) >= 1.1, "{}: menu edge {:.2}:1", t.id, contrast(edge, c.bg));
            let ring = crate::theme::menu_shadows()[0].color;
            let old = c.bg.mix(ring.alpha(1.0), ring.a);
            assert!(contrast(edge, c.bg) > contrast(old, c.bg), "{}: the edge shows less than the shadow's ring", t.id);
        }
    }

    /// C7: a cut name's tooltip is the name, not the file path, in three lines at most: a name
    /// that fits two has its folder under it, cut at its start; a longer one takes all three and
    /// ends "…".
    #[test]
    fn a_cut_names_tooltip_is_three_lines_at_most() {
        let style = super::super::styles::tooltip();
        let line = rune::text::measure("A", &[], style, None).1;
        let lines = |t: &str| (rune::text::measure(t, &[], style, Some(TIP_W)).1 / line).round();
        let deep = "Projects/Archive/2025/Quarterly reviews and retrospectives for the platform team/Q3";
        let tip = name_tip(&format!("{deep}/Plan.md"));
        let (name, folder) = tip.split_once('\n').expect("the folder on its own line");
        assert_eq!(name, "Plan");
        assert!(folder.starts_with('…') && folder.ends_with("/Q3"), "{folder}");
        // Names grown to two and three lines in the tooltip's family, whichever `panel_font` is.
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
