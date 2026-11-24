//! The vault window's top band: window dots, sidebar toggle, the note's name and view toggles.
//! The open notes are listed on the sidebar's Open shelf (`sidebar::shelf`).

use rune::{El, Tx, el, row};

use super::V;
use super::icons::{CODE, COPY, EYE, FIT, FOCUS, FONT, HELP, LAYOUT_RIGHT, MAP, NARROW, PARAGRAPH, SEARCH, SIDEBAR_LEFT, SIDEBAR_RIGHT, TYPEWRITER, WIDEN, icon_of};
use super::widgets::{Control, dots, icon_button, label, toggle_button};
use crate::app::{App, Mode};
use crate::keymap::tip;
use crate::layer::Kind;
use super::styles::sans;

/// The title bar's height: omarky's header, 40 points. Its buttons are 24 and their icons 15,
/// each about the golden ratio (1.618) of the next size down, all centred 20 points down.
pub const BAR_H: f32 = 40.0;
const BUTTON: f32 = 24.0;
/// Every icon in the bar is drawn at this one size. A glyph that looks heavier or lighter than
/// its neighbours is fixed in its own drawing, not by a size of its own.
const ICON: f32 = 15.0;

/// An icon button sized for the bar, keyboard-focusable with a focus ring while preserving
/// editor/preview focus on pointer clicks via the bar's `preserve_focus_on_press(true)` (`KBDA11Y`).
fn bar_button(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, help: &str, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let (rw, rg, rc) = super::focus_ring(p);
    super::focus::stop(icon_button(p, key, glyph, ICON, help).size(BUTTON, BUTTON))
        .role("button")
        .focus_ring(rw, rg, rc)
        .keeps_text_selection()
        .control(run)
}

fn bar_toggle(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, help: &str, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let (rw, rg, rc) = super::focus_ring(p);
    super::focus::stop(toggle_button(p, key, glyph, ICON, help, on).size(BUTTON, BUTTON))
        .focus_ring(rw, rg, rc)
        .keeps_text_selection()
        .control(run)
}

fn bar_button_disabled(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, help: &str) -> El {
    row()
        .key(key)
        .size(BUTTON, BUTTON)
        .center()
        .radius(6.0)
        .disabled(true)
        .label(help)
        .help(help)
        .child(rune::icon(glyph, ICON).color(p.mute.alpha(0.38)))
}

/// A title bar button's press, which stays live over a drawer: the drawer closes, then the
/// button acts, as a window shortcut does over it (`keys::on_key`). The sidebar and outline
/// toggles answer a drawer through `App::toggle_drawer`.
pub(super) fn act(f: fn(&mut App, &mut Tx)) -> impl Fn(&mut App, &mut Tx) + 'static {
    move |s, tx| {
        s.close_layers(tx);
        f(s, tx);
    }
}

/// The corner's leading padding, gap after the window dots (when shown), and total width.
const CORNER_PL: f32 = if super::widgets::DOTS_W > 0.0 { 14.0 } else { 8.0 };
const CORNER_GAP: f32 = if super::widgets::DOTS_W > 0.0 { 10.0 } else { 0.0 };
const CORNER_W: f32 = CORNER_PL + super::widgets::DOTS_W + CORNER_GAP + BUTTON + 8.0;

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
        .pl(CORNER_PL)
        .pr(8.0)
        .gap(CORNER_GAP)
        .center_y()
        .window_drag()
        .preserve_focus_on_press(true)
        .children((super::widgets::DOTS_W > 0.0).then(|| dots(p)))
        .child(bar_button(p, "toggle-left", &SIDEBAR_LEFT, &tip(v.s.preset(), crate::commands::sidebar_name(v.s), "app.toggle-sidebar"), |s: &mut App, tx| s.toggle_left(tx)).no_window_drag())
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
    // Hide the crumb while the narrow overlay sidebar drawer covers the left half of the bar
    // (`FINDMODEL`), so the drawer edge never slices the note path mid-character.
    if v.has_doc() && !v.left_over() {
        title = title.child(super::editor::crumb(v));
    }

    let right = actions(v, row().key("right-zone").h(BAR_H).pl(6.0).pr(8.0).gap(4.0).center_y());
    let slide = rune::Spring::new(0.16, 0.0);

    // A press on the bar's buttons, or a drag of the window by it, leaves the keyboard where
    // it was: the caret stays in the note. The rename field still takes focus.
    row()
        .key("titlebar")
        .h(BAR_H)
        .w_full()
        .window_drag()
        .preserve_focus_on_press(true)
        .paint_order(3)
        .child(corner_room().w(if left_visible { 0.0 } else { CORNER_W }).motion(slide))
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
        if s.active_locked() {
            r = r.child(bar_button_disabled(p, "find", &SEARCH, "Unlock note to find"));
            r = r.child(bar_button_disabled(p, "copy-all", &COPY, "Unlock note to copy"));
        } else {
            let find = tip("Find in note", "reader.find");
            r = r.child(bar_toggle(p, "find", &SEARCH, &find, s.reader.find.open, act(|s, tx| s.toggle_find(tx))));
            let all = tip("Copy the whole note", "reader.copy-all");
            r = r.child(bar_button(p, "copy-all", &COPY, &all, act(|s, tx| s.copy_all(tx))));
        }
        let wide = s.wide();
        let (glyph, name) = if wide { (&NARROW, "Use readable width") } else { (&WIDEN, "Use full width") };
        let help = tip(name, "reader.toggle-wide");
        r = r.child(bar_toggle(p, "wide", glyph, &help, wide, act(|s, _| s.toggle_wide())));
        let help = tip("Show shortcuts", "reader.help");
        r = r.child(bar_button(p, "help", &HELP, &help, act(|s, tx| s.toggle_help(tx))));
    } else if v.has_doc() && s.mode == Mode::Map {
        // Find, Layout, Fit and Help (MAP-064, MAP-089).
        if s.active_locked() {
            r = r.child(bar_button_disabled(p, "find", &SEARCH, "Unlock note to find"));
            r = r.child(bar_button_disabled(p, "map-layout", &LAYOUT_RIGHT, "Unlock the note to change its layout"));
        } else {
            let find = tip("Find in note", "map.find");
            r = r.child(bar_toggle(p, "find", &SEARCH, &find, s.reader.find.open, act(|s, tx| s.toggle_find(tx))));
            let lay = s.map_tree().opts.layout;
            let help = tip("Layout", "map.layout");
            let layout = bar_toggle(p, "map-layout", icon_of(lay), &help, s.layer_is(Kind::Layouts), act(|s, tx| s.open_layouts(tx)));
            r = r.child(layout.on_layout_tx(|s: &mut App, [x, _, w, _], _| s.map.layout_button_right = x + w));
            let help = tip("Fit the map", "map.fit");
            r = r.child(bar_button(p, "map-fit", &FIT, &help, act(|s, tx| s.map_fit(tx))));
        }
        let help = tip("Show shortcuts", "map.help");
        r = r.child(bar_button(p, "help", &HELP, &help, act(|s, tx| s.toggle_help(tx))));
    } else if v.has_doc() {
        // The writer's view toggles (WR-79, WR-85, WR-125), each on while its feature is.
        let set = s.settings();
        let name = if set.paragraph_focus { "Turn off paragraph focus" } else { "Turn on paragraph focus" };
        let help = tip(name, "writer.paragraph-focus");
        r = r.child(bar_toggle(p, "paragraph-focus", &PARAGRAPH, &help, set.paragraph_focus, act(|s, tx| s.toggle_paragraph_focus(tx))));
        let name = if set.typewriter_scroll { "Turn off typewriter scrolling" } else { "Turn on typewriter scrolling" };
        let help = tip(name, "writer.typewriter");
        r = r.child(bar_toggle(p, "typewriter", &TYPEWRITER, &help, set.typewriter_scroll, act(|s, tx| s.toggle_typewriter(tx))));
        let help = tip("Use next font", "writer.next-font");
        r = r.child(bar_button(p, "next-font", &FONT, &help, act(|s, tx| s.next_font(tx))));
    }
    if v.has_doc() {
        r = r.child(el().key("bar-sep").w(1.0).h(16.0).fill(p.line));
        // Source, Preview and Map, the shown one marked (MAP-064).
        let mut modes = row().key("modes").gap(4.0).center_y().role("radiogroup").label("Mode");
        for (m, key, glyph, name) in [(Mode::Source, "mode-source", &CODE, "Source"), (Mode::Preview, "mode-preview", &EYE, "Preview"), (Mode::Map, "mode-map", &MAP, "Map")] {
            let on = s.mode == m;
            let help = if on { name.to_string() } else { tip(crate::commands::show_name(m), crate::commands::mode_action(s, m)) };
            modes = modes.child(bar_toggle(p, key, glyph, &help, on, act(crate::commands::show(m))).role("radio"));
        }
        r = r.child(modes);
        let help = tip(crate::commands::zen_name(s), if zen { "app.escape" } else { "app.focus-mode" });
        r = r.child(bar_toggle(p, "focus", &FOCUS, &help, zen, act(|s, tx| s.toggle_focus(tx))));
    }
    if !zen && v.has_doc() {
        let outline = tip(crate::commands::outline_name(s), if preview { "reader.toggle-outline" } else { "app.toggle-outline" });
        r = r.child(bar_toggle(p, "toggle-right", &SIDEBAR_RIGHT, &outline, v.right_visible() || v.right_over(), |s: &mut App, tx| s.toggle_right(tx)));
    }
    r
}

/// The width of zen mode's pill in preview, the widest: eight buttons (`actions`), the rule
/// between them, their gaps and its padding.
const PILL_ROOM: f32 = 8.0 * (BUTTON + 4.0) + 1.0 + 8.0;

/// Zen mode's bar: the word count and how to leave, centred in the band above the text, so
/// nothing covers it; the window dots and a button to leave show under the pointer.
fn focus_bar(v: V) -> El {
    let p = v.p;
    let wc = v.s.word_count().words;
    let count = format!("{} {}", super::editor::group(wc), if wc == 1 { "word" } else { "words" });
    // The hint stays 16 pt clear of the pill and the dots, which show under the pointer. It is
    // centred on the window when that leaves the pill its room, else centred in the room left
    // of the pill. A window too narrow even for that shows only the word count. The room is the
    // wider pill's, the preview's, in both modes, so the hint does not move when the mode does.
    // The pill's right margin and the gap before the window's trailing room are 10 and 8.
    let right = PILL_ROOM + 10.0 + 8.0 + v.trailing + 16.0;
    let left = 14.0 + 52.0 + 16.0;
    let full = format!("{count} · Esc to leave zen mode");
    let w = rune::text::measure(&full, &[], sans(12.0, 400.0), None).0;
    let (words, left) = if w <= v.vw - 2.0 * right {
        (full, right)
    } else {
        (if w <= v.vw - left - right { full } else { count }, left)
    };
    let hint = row()
        .key("focus-hint")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .pl(left)
        .pr(right)
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
                    .mr(8.0)
                    .gap(4.0)
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
