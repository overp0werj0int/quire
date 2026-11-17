//! The vault window's top band: window dots, sidebar toggle, the note's name and view toggles.
//! The open notes are listed at the top of the sidebar (`sidebar::open_notes`).

use rune::{El, el, row};

use super::V;
use super::widgets::{CODE, EYE, FOCUS, FONT, HELP, NARROW, PARAGRAPH, SEARCH, SIDEBAR_LEFT, SIDEBAR_RIGHT, TYPEWRITER, WIDEN, dots, icon_button, label, toggle_button};
use super::octicons::COPY;
use crate::app::{App, Mode};
use crate::keymap::tip;
use crate::theme::mono;

/// The title bar's height: omarky's header, 40 points. Its buttons are 24 and their icons 15,
/// each about the golden ratio (1.618) of the next size down, all centred 20 points down.
pub const BAR_H: f32 = 40.0;
const BUTTON: f32 = 24.0;

/// An icon button sized for the bar.
fn bar_button(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, size: f32, help: &str) -> El {
    icon_button(p, key, glyph, size, help).size(BUTTON, BUTTON)
}

fn bar_toggle(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, size: f32, help: &str, on: bool) -> El {
    toggle_button(p, key, glyph, size, help, on).size(BUTTON, BUTTON)
}

/// The window's top-left corner: the window dots and the sidebar toggle. It heads the sidebar
/// when that is open and the title bar when it is closed, at the same place, so the toggle
/// never moves. Its icons centre on the bar's middle.
pub fn corner(v: V) -> El {
    let p = v.p;
    row()
        .key("left-zone")
        .h(BAR_H)
        .pl(14.0)
        .pr(8.0)
        .gap(10.0)
        .center_y()
        .window_drag()
        .preserve_focus_on_press(true)
        .child(dots(p))
        .child(bar_button(p, "toggle-left", &SIDEBAR_LEFT, 15.0, &tip(v.s.preset(), "Sidebar", "app.toggle-sidebar")).no_window_drag().on_press(|s: &mut App| s.toggle_left()))
}

pub fn titlebar(v: V) -> El {
    if v.focus() { focus_bar(v) } else { bar(v) }
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
        .children((!left_visible).then(|| corner(v)))
        .child(title)
        .child(right)
        .child(el().w(v.trailing))
}

/// The bar's buttons for the shown note, into `r`. In zen mode (the floating pill) the outline
/// toggle is left out, as zen mode hides the outline, and the zen button shows as on.
fn actions(v: V, mut r: El) -> El {
    let p = v.p;
    let s = v.s;
    let zen = v.focus();
    let tip = |name: &str, id: &str| tip(s.preset(), name, id);
    if v.has_doc() {
        // One button flips between source and preview. Its icon shows where it goes.
        let (glyph, name) = if s.mode == Mode::Source { (&EYE, "Show preview") } else { (&CODE, "Show source") };
        let help = tip(name, "app.toggle-mode");
        r = r.child(bar_button(p, "mode", glyph, 15.0, &help).role("switch").checked(s.mode == Mode::Preview).on_press_tx(|s: &mut App, tx| s.toggle_mode(tx)));
        let help = if zen { tip("Leave zen mode", "app.escape") } else { tip("Zen mode", "app.focus-mode") };
        r = r.child(bar_toggle(p, "focus", &FOCUS, 15.0, &help, zen).on_press_tx(|s: &mut App, tx| s.toggle_focus(tx)));
    }
    // In preview, omarky's header chips: find, copy all and the measure, then the outline and
    // the shortcuts (RD-34). Each shows as on while its feature is (RD-35).
    let preview = v.has_doc() && s.mode == Mode::Preview;
    if preview {
        let find = tip("Find", "reader.find");
        r = r.child(bar_toggle(p, "find", &SEARCH, 13.0, &find, s.reader.find.open).on_press_tx(|s: &mut App, tx| s.toggle_find(tx)));
        let all = tip("Copy all", "reader.copy-all");
        r = r.child(bar_button(p, "copy-all", &COPY, 12.0, &all).role("button").on_press_tx(|s: &mut App, tx| s.copy_all(tx)));
        let wide = s.wide();
        let (glyph, name) = if wide { (&NARROW, "Readable width") } else { (&WIDEN, "Full width") };
        let help = tip(name, "reader.toggle-wide");
        r = r.child(bar_toggle(p, "wide", glyph, 15.0, &help, wide).on_press(|s: &mut App| s.toggle_wide()));
    } else if v.has_doc() {
        // The writer's view toggles (WR-79, WR-85, WR-125), each on while its feature is.
        let set = s.settings();
        let help = tip("Paragraph focus", "writer.paragraph-focus");
        r = r.child(bar_toggle(p, "paragraph-focus", &PARAGRAPH, 15.0, &help, set.paragraph_focus).on_press_tx(|s: &mut App, tx| s.toggle_paragraph_focus(tx)));
        let help = tip("Typewriter scrolling", "writer.typewriter");
        r = r.child(bar_toggle(p, "typewriter", &TYPEWRITER, 15.0, &help, set.typewriter_scroll).on_press_tx(|s: &mut App, tx| s.toggle_typewriter(tx)));
        let help = tip("Next font", "writer.next-font");
        r = r.child(bar_button(p, "next-font", &FONT, 15.0, &help).role("button").on_press_tx(|s: &mut App, tx| s.next_font(tx)));
    }
    if !zen {
        let outline = tip("Outline", if preview { "reader.toggle-outline" } else { "app.toggle-outline" });
        r = r.child(bar_toggle(p, "toggle-right", &SIDEBAR_RIGHT, 15.0, &outline, v.right_visible() || v.right_over()).on_press_tx(|s: &mut App, tx| s.toggle_right(tx)));
    }
    if preview {
        let help = tip("Shortcuts", "reader.help");
        r = r.child(bar_button(p, "help", &HELP, 14.0, &help).role("button").on_press(|s: &mut App| s.reader.help = !s.reader.help));
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
        .child(label(words, mono(11.5, 400.0), p.mute));
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
