//! The vault window's top band: window dots, sidebar toggle, the note's name and view toggles.
//! The open notes are listed at the top of the sidebar (`sidebar::open_notes`).

use rune::{El, Tx, el, row};

use super::V;
use super::widgets::{CODE, COPY_ALL, EYE, FOCUS, FONT, HELP, NARROW, PARAGRAPH, SEARCH, SIDEBAR_LEFT, SIDEBAR_RIGHT, TYPEWRITER, WIDEN, dots, icon_button, label, toggle_button};
use crate::app::{App, Mode};
use crate::keymap::tip;
use super::styles::sans;

/// The title bar's height: omarky's header, 40 points. Its buttons are 24 and their icons 15,
/// each about the golden ratio (1.618) of the next size down, all centred 20 points down.
pub const BAR_H: f32 = 40.0;
const BUTTON: f32 = 24.0;
/// Every icon in the bar is drawn at this one size. A glyph that looks heavier or lighter than
/// its neighbours is fixed in its own drawing, not by a size of its own.
const ICON: f32 = 15.0;

/// An icon button sized for the bar.
fn bar_button(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, help: &str) -> El {
    icon_button(p, key, glyph, ICON, help).size(BUTTON, BUTTON)
}

fn bar_toggle(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, help: &str, on: bool) -> El {
    toggle_button(p, key, glyph, ICON, help, on).size(BUTTON, BUTTON)
}

/// A title bar button's press: it closes an open drawer, then acts, as the sidebar and outline
/// toggles do through `App::toggle_drawer`. So every title bar button answers a drawer alike.
pub(super) fn act(f: fn(&mut App, &mut Tx)) -> impl Fn(&mut App, &mut Tx) + 'static {
    move |s, tx| {
        s.drawer = None;
        f(s, tx);
    }
}

/// The corner's width: its padding, the dots, the gap and the toggle.
const CORNER_W: f32 = 14.0 + super::widgets::DOTS_W + 10.0 + BUTTON + 8.0;

/// The window's top-left corner: the window dots and the sidebar toggle. It lies over the
/// window, outside the sidebar, which leaves room for it at its top, as the title bar does when
/// the sidebar is closed. So there is one toggle, and it stays under the pointer while the
/// sidebar comes and goes. Its icons centre on the bar's middle.
pub fn corner(v: V) -> El {
    let p = v.p;
    row()
        .key("left-zone")
        .pin(Some(0.0), Some(0.0), None, None)
        .w(CORNER_W)
        .paint_order(37)
        .h(BAR_H)
        .pl(14.0)
        .pr(8.0)
        .gap(10.0)
        .center_y()
        .window_drag()
        .preserve_focus_on_press(true)
        .child(dots(p))
        .child(bar_button(p, "toggle-left", &SIDEBAR_LEFT, &tip(v.s.preset(), crate::commands::sidebar_name(v.s), "app.toggle-sidebar")).no_window_drag().on_press_tx(|s: &mut App, tx| s.toggle_left(tx)))
}

/// The room under the corner, at the sidebar's top or the title bar's start.
pub fn corner_room() -> El {
    el().key("corner-room").h(BAR_H)
}

/// The title bar, or zen mode's bar. Entering or leaving zen mode, the old bar goes at once and
/// the new one fades in where it stays.
pub fn titlebar(v: V) -> El {
    if v.focus() { focus_bar(v) } else { bar(v) }.transition(super::swap())
}

fn bar(v: V) -> El {
    let left_visible = v.left_visible();
    // The note's folder and name start where the page does: after the sidebar, or after the
    // window corner's buttons when it is closed.
    let mut title = row().key("title").grow(1.0).min_w(0.0).h(BAR_H).pl(if left_visible { 20.0 } else { 6.0 }).pr(8.0).center_y().clip_rounded(0.0);
    if v.has_doc() {
        title = title.child(super::editor::crumb(v));
    }

    let right = actions(v, row().key("right-zone").h(BAR_H).pl(6.0).pr(10.0).gap(6.0).center_y());

    // A press on the bar's buttons, or a drag of the window by it, leaves the keyboard where
    // it was: the caret stays in the note. The rename field still takes focus.
    row()
        .key("titlebar")
        .h(BAR_H)
        .w_full()
        .window_drag()
        .preserve_focus_on_press(true)
        .paint_order(3)
        .children((!left_visible).then(|| corner_room().w(CORNER_W)))
        .child(title)
        .child(right)
        .child(el().w(v.trailing))
}

/// The bar's buttons for the shown note, into `r`. The buttons both modes have (mode, zen and
/// outline) hold the bar's right end in that order, so none moves when the mode changes. The
/// mode's own buttons stand left of them, after a hairline. In zen mode (the floating pill) the
/// outline toggle is left out, as zen mode hides the outline, and the zen button shows as on.
fn actions(v: V, mut r: El) -> El {
    let p = v.p;
    let s = v.s;
    let zen = v.focus();
    let tip = |name: &str, id: &str| tip(s.preset(), name, id);
    // In preview, omarky's header chips: find, copy all, the measure and the shortcuts (RD-34).
    // Each shows as on while its feature is (RD-35).
    let preview = v.has_doc() && s.mode == Mode::Preview;
    if preview {
        let find = tip("Find in note", "reader.find");
        r = r.child(bar_toggle(p, "find", &SEARCH, &find, s.reader.find.open).on_press_tx(act(|s, tx| s.toggle_find(tx))));
        let all = tip("Copy the whole note", "reader.copy-all");
        r = r.child(bar_button(p, "copy-all", &COPY_ALL, &all).role("button").on_press_tx(act(|s, tx| s.copy_all(tx))));
        let wide = s.wide();
        let (glyph, name) = if wide { (&NARROW, "Use readable width") } else { (&WIDEN, "Use full width") };
        let help = tip(name, "reader.toggle-wide");
        r = r.child(bar_toggle(p, "wide", glyph, &help, wide).on_press_tx(act(|s, _| s.toggle_wide())));
        let help = tip("Show shortcuts", "reader.help");
        r = r.child(bar_button(p, "help", &HELP, &help).role("button").on_press_tx(act(|s, tx| s.toggle_help(tx))));
    } else if v.has_doc() {
        // The writer's view toggles (WR-79, WR-85, WR-125), each on while its feature is.
        let set = s.settings();
        let name = if set.paragraph_focus { "Turn off paragraph focus" } else { "Turn on paragraph focus" };
        let help = tip(name, "writer.paragraph-focus");
        r = r.child(bar_toggle(p, "paragraph-focus", &PARAGRAPH, &help, set.paragraph_focus).on_press_tx(act(|s, tx| s.toggle_paragraph_focus(tx))));
        let name = if set.typewriter_scroll { "Turn off typewriter scrolling" } else { "Turn on typewriter scrolling" };
        let help = tip(name, "writer.typewriter");
        r = r.child(bar_toggle(p, "typewriter", &TYPEWRITER, &help, set.typewriter_scroll).on_press_tx(act(|s, tx| s.toggle_typewriter(tx))));
        let help = tip("Use next font", "writer.next-font");
        r = r.child(bar_button(p, "next-font", &FONT, &help).role("button").on_press_tx(act(|s, tx| s.next_font(tx))));
    }
    if v.has_doc() {
        r = r.child(el().key("bar-sep").w(1.0).h(16.0).mx(2.0).fill(p.line));
        // One button flips between source and preview. Its icon shows where it goes.
        let glyph = if s.mode == Mode::Source { &EYE } else { &CODE };
        let help = tip(crate::commands::mode_name(s), "app.toggle-mode");
        r = r.child(bar_button(p, "mode", glyph, &help).role("switch").checked(s.mode == Mode::Preview).on_press_tx(act(|s, tx| s.toggle_mode(tx))));
        let help = tip(crate::commands::zen_name(s), if zen { "app.escape" } else { "app.focus-mode" });
        r = r.child(bar_toggle(p, "focus", &FOCUS, &help, zen).on_press_tx(act(|s, tx| s.toggle_focus(tx))));
    }
    if !zen {
        let outline = tip(crate::commands::outline_name(s), if preview { "reader.toggle-outline" } else { "app.toggle-outline" });
        r = r.child(bar_toggle(p, "toggle-right", &SIDEBAR_RIGHT, &outline, v.right_visible() || v.right_over()).on_press_tx(|s: &mut App, tx| s.toggle_right(tx)));
    }
    r
}

/// Zen mode's bar: the word count and how to leave, centred in the band above the text, so
/// nothing covers it; the window dots and a button to leave show under the pointer.
fn focus_bar(v: V) -> El {
    let p = v.p;
    let wc = v.s.word_count().words;
    let words = format!("{} {} · esc to leave zen mode", super::editor::group(wc), if wc == 1 { "word" } else { "words" });
    let hint = row()
        .key("focus-hint")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .justify_center()
        .center_y()
        .inert()
        .child(label(words, sans(12.0, 400.0), p.mute));
    let inner = row()
        .key("focus-inner")
        .h(BAR_H)
        .w_full()
        .pl(14.0)
        .gap(8.0)
        .center_y()
        .group_opacity(0.0, 1.0)
        .child(dots(p))
        .child(el().grow(1.0))
        .child(
            // The title bar's buttons on a floating pill, 85% opaque, in the top-right corner.
            actions(
                v,
                row()
                    .key("zen-actions")
                    .h(BUTTON + 8.0)
                    .px(4.0)
                    .mr(10.0)
                    .gap(6.0)
                    .center_y()
                    .radius(10.0)
                    .fill(p.bg.alpha(0.85))
                    .border(1.0, p.line)
                    .backdrop_blur(12.0)
                    .box_shadow(rune::Shadow { x: 0.0, y: 6.0, blur: 22.0, spread: -4.0, color: rune::Color::BLACK.alpha(if p.dark { 0.5 } else { 0.12 }), inset: false })
                    .no_window_drag(),
            ),
        )
        .child(el().w(v.trailing));
    el().key("titlebar").h(BAR_H).w_full().window_drag().preserve_focus_on_press(true).hover_group().hover_motion(rune::Spring::new(0.25, 0.0), rune::Spring::new(0.25, 0.0)).child(hint).child(inner)
}
