//! The vault window's top band: window dots, sidebar toggle, tabs and view toggles.

use rune::{Color, El, PointerButton, el, icon, row, text};

use super::V;
use super::widgets::{CLOSE, CODE, EYE, FOCUS, HELP, NARROW, PLUS, SEARCH, SIDEBAR_LEFT, SIDEBAR_RIGHT, WIDEN, dots, icon_button, label, toggle_button};
use super::octicons::COPY;
use crate::app::{App, Mode};
use crate::md;
use crate::theme::{mono, sans};

/// The title bar's height: omarky's header, 40 points. Its buttons are 24 and their icons 15,
/// each about the golden ratio (1.618) of the next size down, all centred 20 points down.
pub const BAR_H: f32 = 40.0;
const BUTTON: f32 = 24.0;
/// A tab hangs one point below the bar, leaving 5 points above it.
const TAB_H: f32 = 36.0;

/// An icon button sized for the bar.
fn bar_button(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, size: f32, help: &str) -> El {
    icon_button(p, key, glyph, size, help).size(BUTTON, BUTTON)
}

fn bar_toggle(p: &crate::theme::Pal, key: &str, glyph: &'static rune::Icon, size: f32, help: &str, on: bool) -> El {
    toggle_button(p, key, glyph, size, help, on).size(BUTTON, BUTTON)
}

pub fn titlebar(v: V) -> El {
    if v.focus() { focus_bar(v) } else { bar(v) }
}

fn bar(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let left_visible = v.left_visible();
    // The icons on both sides, the window dots and the new-tab button centre on the bar's middle.
    let mut left = row().key("left-zone").h(BAR_H).pl(14.0).pr(8.0).gap(10.0).center_y();
    if left_visible {
        left = left.w(248.0);
    }
    left = left
        .child(dots(p))
        .child(el().grow(1.0))
        .child(bar_button(p, "toggle-left", &SIDEBAR_LEFT, 15.0, "Toggle sidebar").on_press(|s: &mut App| s.toggle_left()));

    // With the sidebar open, the first tab's left border sits on the line where the content starts.
    let mut tabs = row().key("tabs").grow(1.0).min_w(0.0).h(BAR_H).items_end().gap(2.0).pl(if left_visible { 0.0 } else { 6.0 }).pr(4.0).clip_rounded(0.0).offset(0.0, 1.0);
    for p_ in s.tabs.iter().filter(|t| s.files().contains_key(*t)) {
        tabs = tabs.child(tab(v, p_));
    }
    tabs = tabs.child(bar_button(p, "new-tab", &PLUS, 11.0, "New note").mb(BAR_H + 1.0 - BAR_H / 2.0 - BUTTON / 2.0).ml(2.0).on_press_tx(|s: &mut App, tx| s.new_note(None, tx)));

    let mut right = row().key("right-zone").h(BAR_H).pl(6.0).pr(10.0).gap(6.0).center_y();
    if v.has_doc() {
        // One button flips between source and preview. Its icon shows where it goes.
        let (glyph, help) = if s.mode == Mode::Source {
            (&EYE, format!("Show preview ({})", crate::keymap::label(s.preset(), "app.toggle-mode")))
        } else {
            (&CODE, format!("Show source ({})", crate::keymap::label(s.preset(), "app.toggle-mode")))
        };
        right = right.child(bar_button(p, "mode", glyph, 15.0, &help).role("switch").checked(s.mode == Mode::Preview).on_press_tx(|s: &mut App, tx| s.toggle_mode(tx)));
        let help = format!("Zen mode ({})", crate::keymap::label(s.preset(), "app.focus-mode"));
        right = right.child(bar_button(p, "focus", &FOCUS, 15.0, &help).on_press_tx(|s: &mut App, tx| s.toggle_focus(tx)));
    }
    // In preview, omarky's header chips: find, copy all and the measure, then the outline and
    // the shortcuts (RD-34). Each shows as on while its feature is (RD-35).
    let preview = v.has_doc() && s.mode == Mode::Preview;
    let tip = |name: &str, id: &str| format!("{name}  {}", crate::keymap::label(s.preset(), id));
    if preview {
        let find = tip("Find", "reader.find");
        right = right.child(bar_toggle(p, "find", &SEARCH, 13.0, &find, s.reader.find.open).on_press_tx(|s: &mut App, tx| s.toggle_find(tx)));
        let all = tip("Copy all", "reader.copy-all");
        right = right.child(bar_button(p, "copy-all", &COPY, 12.0, &all).role("button").on_press_tx(|s: &mut App, tx| s.copy_all(tx)));
        let wide = s.wide();
        let (glyph, name) = if wide { (&NARROW, "Reading width") } else { (&WIDEN, "Wide width") };
        let help = tip(name, "reader.toggle-wide");
        right = right.child(bar_toggle(p, "wide", glyph, 15.0, &help, wide).on_press(|s: &mut App| s.toggle_wide()));
    }
    let outline = if preview { tip("Outline", "reader.toggle-outline") } else { "Toggle outline".to_string() };
    right = right.child(bar_toggle(p, "toggle-right", &SIDEBAR_RIGHT, 15.0, &outline, preview && v.right_visible()).on_press(|s: &mut App| s.toggle_right()));
    if preview {
        let help = tip("Shortcuts", "reader.help");
        right = right.child(bar_button(p, "help", &HELP, 14.0, &help).role("button").on_press(|s: &mut App| s.reader.help = !s.reader.help));
    }

    row()
        .key("titlebar")
        .h(BAR_H)
        .w_full()
        .window_drag()
        .paint_order(3)
        .child(left)
        .child(tabs)
        .child(right)
        .child(el().w(v.trailing))
}

fn tab(v: V, path: &str) -> El {
    let p = v.p;
    let on = v.s.active.as_deref() == Some(path);
    let open_path = path.to_string();
    let close_path = path.to_string();
    let key = format!("tab:{path}");
    let mut t = row()
        .key(key)
        .h(TAB_H)
        .w(190.0)
        .shrink()
        .min_w(72.0)
        .pl(14.0)
        .pr(6.0)
        // The label and close button centre on the bar's middle, with the icons, not the tab's.
        .pb(BAR_H + 2.0 - TAB_H)
        .gap(6.0)
        .center_y()
        .corners([9.0, 9.0, 0.0, 0.0])
        .fill(if on { p.bg } else { Color::TRANSPARENT })
        .hover_group()
        .role("tab")
        .label(md::base(path).to_string())
        .help(path.to_string())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .no_window_drag()
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_tab(&open_path);
            } else if s.active.as_deref() != Some(open_path.as_str()) {
                s.open_file(&open_path, None, tx);
            }
        });
    // Every tab has a 1 px border that takes room, transparent unless active, as in CSS.
    t = t.border_sides([1.0, 1.0, 0.0, 1.0], if on { p.line } else { Color::TRANSPARENT }).border_layout();
    let name = text(md::base(path)).font(sans(13.0, 400.0)).color(if on { p.ink } else { p.mute }).group_color(p.ink).truncate().grow(1.0).shrink().min_w(0.0);
    let close = row()
        .key("x")
        .size(20.0, 20.0)
        .center()
        .radius(5.0)
        .hover(p.hover)
        .hover_group()
        .label("Close")
        .help("Close")
        .on_press(move |s: &mut App| s.close_tab(&close_path))
        .child(icon(&CLOSE, 8.0).color(p.mute).group_opacity(if on { 1.0 } else { 0.55 }, 1.0));
    t.child(name).child(close)
}

fn focus_bar(v: V) -> El {
    let p = v.p;
    let inner = row()
        .key("focus-inner")
        .h(BAR_H)
        .w_full()
        .pl(14.0)
        .gap(8.0)
        .center_y()
        .fill(p.bg)
        .group_opacity(0.0, 1.0)
        .child(dots(p))
        .child(el().grow(1.0))
        .child(
            row()
                .key("leave-focus")
                .h(BUTTON)
                .px(10.0)
                .mr(10.0)
                .center()
                .radius(6.0)
                .hover(p.hover)
                .hover_group()
                .role("button")
                .on_press_tx(|s: &mut App, tx| s.toggle_focus(tx))
                .child(label("leave zen mode · esc", mono(11.5, 400.0), p.mute).group_color(p.ink)),
        )
        .child(el().w(v.trailing));
    el().key("titlebar").h(BAR_H).w_full().window_drag().hover_group().hover_motion(rune::Spring::new(0.25, 0.0), rune::Spring::new(0.25, 0.0)).child(inner)
}
