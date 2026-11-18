//! Small pieces the screens share: the design's icons, segmented controls and switches.

use rune::{Color, El, Icon, Key, KeyEvent, Mods, Shadow, TextStyle, Tx, el, icon, row, text};

use crate::app::App;
use super::styles::{fixed_advance, literal, sans};
use crate::roles::Roles;
use crate::theme::{self, Pal};

macro_rules! svg {
    ($name:ident, $view:expr, $body:expr) => {
        pub static $name: Icon = Icon { name: stringify!($name), view: $view, body: $body };
    };
}

svg!(SIDEBAR_LEFT, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M6 3v10" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
svg!(SIDEBAR_RIGHT, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M10 3v10" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
svg!(CLOSE, 8.0, r#"<path d="M1 1l6 6M7 1L1 7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(CLOSE_THIN, 8.0, r#"<path d="M1 1l6 6M7 1L1 7" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
svg!(PLUS, 12.0, r#"<path d="M6 1.5v9M1.5 6h9" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(CHEVRON_DOWN, 10.0, r#"<path d="M2 3.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(CHEVRON_UP, 10.0, r#"<path d="M2 6.5l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(CHEVRON_RIGHT, 8.0, r#"<path d="M2.5 1l3 3-3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(FOLDER_PLUS, 16.0, r#"<path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h4.9A1.5 1.5 0 0 1 14 6.1v5.4a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M8 7v4M6 9h4" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#);
svg!(SEARCH, 16.0, r#"<circle cx="7" cy="7" r="4.4" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M10.3 10.3L13.6 13.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(EYE, 16.0, r#"<path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><circle cx="8" cy="8" r="2" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
// The writer's view toggles in the title bar: paragraph focus (the middle line at full ink),
// typewriter scrolling (the line held in the middle) and the next font.
svg!(PARAGRAPH, 16.0, r#"<path d="M2.5 3.5h11M2.5 12.5h8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-opacity=".4"/><path d="M2.5 8h11" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#);
svg!(TYPEWRITER, 16.0, r#"<path d="M2 8h12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M5.5 2.5L8 5l2.5-2.5M5.5 13.5L8 11l2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(FONT, 16.0, r#"<path d="M1.5 13L5 3l3.5 10M2.8 9.6h4.4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/><circle cx="11.6" cy="10.6" r="2.3" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M13.9 8v5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
// The formatting toolbar.
svg!(BOLD, 16.0, r#"<path d="M4.8 2.8h3.9a2.6 2.6 0 0 1 0 5.2H4.8zM4.8 8h4.6a2.6 2.6 0 0 1 0 5.2H4.8z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#);
svg!(ITALIC, 16.0, r#"<path d="M7 2.8h5.2M3.8 13.2H9M9.6 2.8L6.4 13.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(STRIKE, 16.0, r#"<path d="M2.8 8h10.4M11 4.4c-.5-.9-1.6-1.6-3-1.6-1.8 0-3 1-3 2.3 0 .9.5 1.5 1.4 1.9M5 11.6c.5.9 1.6 1.6 3 1.6 1.8 0 3.1-1 3.1-2.4 0-.5-.1-.9-.4-1.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(HEADING, 16.0, r#"<path d="M4 3v10M12 3v10M4 8h8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>"#);
svg!(LINK, 16.0, r#"<path d="M6.9 9.1a2.6 2.6 0 0 0 3.7 0l2.1-2.1a2.6 2.6 0 0 0-3.7-3.7l-.7.7M9.1 6.9a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.7-.7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(WIKI, 16.0, r#"<path d="M4.6 3H2.6v10h2M7.2 3H5.4v10h1.8M8.8 3h1.8v10H8.8M11.4 3h2v10h-2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(BULLETS, 16.0, r#"<circle cx="3.2" cy="4" r="1" fill="currentColor"/><circle cx="3.2" cy="8" r="1" fill="currentColor"/><circle cx="3.2" cy="12" r="1" fill="currentColor"/><path d="M6.2 4h7.6M6.2 8h7.6M6.2 12h7.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(NUMBERS, 16.0, r#"<path d="M2.5 2.9l1-.6v3.4M2.2 7.5c.2-.5.6-.8 1.1-.8.6 0 1 .4 1 .9 0 .8-2.1 1.5-2.1 2.4h2.2M6.6 4h7.2M6.6 8h7.2M6.6 12h7.2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(TASK, 16.0, r#"<rect x="2.6" y="2.6" width="10.8" height="10.8" rx="2.4" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.3 8.2l1.9 1.9 3.6-4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(QUOTE, 16.0, r#"<path d="M3.2 3v10M6.4 4.6h7M6.4 8h7M6.4 11.4h4.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
// A padlock: lock a note with a password.
svg!(LOCK, 16.0, r#"<rect x="3.4" y="7" width="9.2" height="6.6" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.4 7V5.2a2.6 2.6 0 0 1 5.2 0V7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
// A picture with a hill and a sun: insert an image.
svg!(IMAGE, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="10.5" cy="6.2" r="1.2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M2.6 11.6l3.6-3.7 2.6 2.6 1.6-1.5 3 2.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"/>"#);
// Four corners of a frame: focus mode.
svg!(FOCUS, 16.0, r#"<path d="M2.5 6V4a1.5 1.5 0 0 1 1.5-1.5h2M10 2.5h2A1.5 1.5 0 0 1 13.5 4v2M13.5 10v2a1.5 1.5 0 0 1-1.5 1.5h-2M6 13.5H4A1.5 1.5 0 0 1 2.5 12v-2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
// Arrows out and in: the wide measure and the reading measure (RD-34).
svg!(WIDEN, 16.0, r#"<path d="M1.8 8h12.4M4.3 5.5L1.8 8l2.5 2.5M11.7 5.5L14.2 8l-2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(NARROW, 16.0, r#"<path d="M1.5 8h4.3M10.2 8h4.3M3.5 5.5L6 8l-2.5 2.5M12.5 5.5L10 8l2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
// A question mark in a ring: the shortcuts sheet.
svg!(HELP, 16.0, r#"<circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M6.2 6.3a1.9 1.9 0 0 1 3.7.4c0 1.3-1.9 1.5-1.9 2.7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".8" fill="currentColor"/>"#);
// The title bar's Copy all: two sheets in the bar's line, unlike the filled octicon that code
// blocks use.
svg!(COPY_ALL, 16.0, r#"<rect x="6" y="6" width="7.5" height="7.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M9.5 6V4A1.5 1.5 0 0 0 8 2.5H4A1.5 1.5 0 0 0 2.5 4v4A1.5 1.5 0 0 0 4 9.5h2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(CODE, 16.0, r#"<path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
// The logo's mark: three sheets in a stack, the violet one in front (assets/brand).
svg!(MARK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#e1dbcf"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#c9bfae"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#7a4fba"/>"##);
svg!(MARK_ON_INK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#3a352e"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#5f584e"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#b597e8"/>"##);
svg!(CHECK, 10.0, r##"<path d="M1.5 5.2l2.4 2.4L8.5 2.6" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"##);

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

/// A switch drawn 36 by 20 that takes a press over 36 by 28.
pub fn switch(p: &Pal, key: &str, on: bool, label_text: &str, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    // The drawn track is 36 × 20. The control around it is 28 high, so it takes a press 4 pt
    // above and below the track.
    let track = el()
        .key("track")
        .size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.button } else { p.mark })
        .child(el().key("knob").size(16.0, 16.0).radius(8.0).fill(Color::WHITE).box_shadow(theme::knob_shadow()).absolute(if on { 18.0 } else { 2.0 }, 2.0));
    let e = row().key(key).size(36.0, 28.0).radius(14.0).center_y().role("switch").checked(on).label(label_text);
    e.control(run).child(track)
}

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
}
