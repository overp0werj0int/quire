//! The left sidebar (`docs/reader-writer/behavior/sidebar.md`): the window corner's room, the
//! top bar with the search field and "+", the Open shelf over the vault (or search's results,
//! or the tags), and the footer with the tags and settings buttons. In Note mode the shelf and
//! a footer with the settings button alone (SB-119). One builder serves the docked sidebar and
//! the drawer.

mod parts;
mod results;
mod sections;
mod shelf;
mod tree;

pub use tree::drag_ghost;

use rune::{El, ScrollSpec, col, el, icon, row, text, text_input};

use super::V;
use super::panel;
use super::styles::{keys, nav};
use super::icons::{CLOSE, GEAR, PLUS, SEARCH, TAG};
use super::widgets::Control;
use crate::app::App;
use crate::keymap;
use crate::roles::Roles;

/// The sidebar's width, docked or in its drawer.
pub(super) const W: f32 = 248.0;
/// What the docked sidebar takes from the page: its width and the line on its right edge.
pub(super) const DOCKED_W: f32 = W + 1.0;

/// A row's room after its 24 pt button so the 14 pt icon inside ends 8 pt from the row's end.
const BUTTON_END: f32 = 3.0;
/// The top bar's and the footer's buttons, and the search field's height.
const BUTTON: f32 = 28.0;
/// The top bar's and the footer's padding.
const BAR_PAD: f32 = 8.0;
/// The sidebar's icons (SB-07).
pub(super) const ICON: f32 = 14.0;
/// The longest key hint the search field shows, in characters, as `Ctrl+/` (SB-10).
const HINT_CHARS: usize = 6;

pub fn sidebar(v: V) -> El {
    let p = v.p;
    let s = v.s;
    // The sidebar runs the window's full height, its edge a line from top to bottom.
    let side = super::focus::pane(col(), crate::focus::Pane::Sidebar)
        .key("sidebar")
        .w(W)
        .h_full()
        .min_h(0.0)
        .fill(p.side)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .border_layout()
        .child(super::titlebar::corner_room());
    if s.note_mode() {
        // Note mode has no folder: no search, "+", vault or tags.
        return side.child(sections::lists(v)).child(foot(v));
    }
    // While the field holds text other than spaces, the results take the room of both lists
    // (SB-57). The lists' folds, split, scroll and open folders wait as they were.
    let middle = match s.results() {
        Some(r) => scroll("results-scroll", results::results(v, &r)),
        None => sections::lists(v),
    };
    side.child(top_bar(v)).child(middle).child(foot(v))
}

/// A list that scrolls in the room left to it. A focused row's ring reaches RING_ROOM past the
/// row: a row that takes focus scrolls in far enough to show it, and the list leaves that room
/// at its ends (RING).
pub(super) fn scroll(key: &str, body: El) -> El {
    col().key(key).grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).scroll_margin(panel::RING_ROOM).child(col().w_full().pt(panel::RING_ROOM).pb(12.0).child(body))
}

/// The top bar: the search field and the "+" button (SB-09).
fn top_bar(v: V) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let (rw, rg, rc) = super::focus_ring(p);
    let new_note = super::focus::stop(row())
        .key("new-note")
        .size(BUTTON, BUTTON)
        .center()
        .radius(6.0)
        .fill(c.hover)
        .hover(c.selected)
        .border(1.0, p.line)
        .border_layout()
        .role("button")
        .label("New note")
        .help(keymap::tip(v.s.preset(), "New note", "app.new-note"))
        .focus_ring(rw, rg, rc)
        .keeps_text_selection()
        .control(|s: &mut App, tx| s.new_note(None, tx))
        .child(icon(&PLUS, ICON).color(c.text));
    row()
        .key("side-top")
        .w_full()
        .padding(BAR_PAD)
        .gap(6.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .border_layout()
        .child(search_field(v))
        .child(new_note)
}

/// The search field: a magnifier, the field, then the key of search while it is empty or a
/// clear button while it holds text (SB-10).
fn search_field(v: V) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let s = v.s;
    let first = s.results().and_then(|r| r.first().map(|row| row.key()));
    let (rw, rg, rc) = super::focus_ring(p);
    let field = super::focus::stop(text_input(&s.query, nav(parts::ROW_TEXT, false)))
        .key("search")
        .single_line()
        .py(1.0)
        .grow(1.0)
        .min_w(0.0)
        .color(c.text)
        .placeholder("Search notes", c.faint)
        .caret(p.ink, p.sel)
        .label("Search notes")
        .help(keymap::tip(s.preset(), "Search notes", "app.search-notes"))
        .on_edit(|s: &mut App| &mut s.query)
        // Down moves into the results and Enter chooses the filled row, the first (SB-66);
        // with no match Enter does nothing (SB-69). Esc goes by `App::search_escape`, then
        // gives the note the keys, which the field would otherwise drop; in the drawer that
        // Esc is the drawer's, as from a row (SB-92). Esc with Shift, Ctrl or Alt goes round
        // again as Esc (`keys::esc_again`).
        .on_input_key(move |s: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx| {
            if crate::keys::esc_again(ev, tx) {
                return true;
            }
            if ev.mods != rune::Mods::default() {
                return false;
            }
            match (&ev.key, &first) {
                (rune::Key::Escape, _) if s.search_escape(tx) => {}
                (rune::Key::Escape, _) if s.layer().is_none() => s.keys_to_note(tx),
                (rune::Key::Down, Some(k)) => tx.focus(k.clone(), true),
                (rune::Key::Enter, Some(k)) => s.choose_result(k, tx),
                (rune::Key::Enter, None) => {}
                _ => return false,
            }
            true
        });
    // The hint shows only when it is short enough to leave "Search notes" whole: Ctrl+/ fits
    // (SB-100), Ctrl+Shift+G does not, and the field's tooltip names it instead.
    let k = keymap::label(s.preset(), "app.search-notes");
    let end = if s.query.text.is_empty() {
        (!k.is_empty() && k.chars().count() <= HINT_CHARS).then(|| kbd(v, k).mr(5.0))
    } else {
        Some(
            super::focus::stop(row())
                .key("search-clear")
                .size(24.0, 24.0)
                .center()
                .radius(5.0)
                .hover(c.hover)
                .label("Clear search")
                .help("Clear search")
                .role("button")
                .focus_ring(rw, rg, rc)
                .control(|s: &mut App, tx| {
                    s.query = rune::TextEdit::new("");
                    tx.focus("search", false);
                })
                .child(icon(&CLOSE, ICON).color(c.faint)),
        )
    };
    super::widgets::input_focus_ring(
        row()
            .key("search-box")
            .grow(1.0)
            .min_w(0.0)
            .h(BUTTON)
            .pl(8.0)
            .pr(2.0)
            .gap(7.0)
            .center_y()
            .radius(7.0)
            .fill(c.shelf)
            .border(1.0, p.line)
            .border_layout(),
        p,
    )
    .child(icon(&SEARCH, ICON).color(c.faint))
        .child(field)
        .children(end)
}

/// A key's name in a chip, as the field's hint (SB-10) and the filled result's (SB-65).
pub(super) fn kbd(v: V, k: String) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    el().key("kbd").px(4.0).py(2.0).radius(4.0).border(1.0, p.line).border_layout().child(text(k).font(keys(super::styles::CAPTION).lh(12.0)).color(c.faint))
}

/// The footer: the tags button on the left, the settings button on the right (SB-78). Note
/// mode has no tags.
fn foot(v: V) -> El {
    let p = v.p;
    let s = v.s;
    // The tags show as the search for "#" (SB-74, SB-75).
    let tags = (!s.note_mode()).then(|| {
        let on = s.query.text.trim_start().starts_with('#');
        foot_button(v, "foot-tags", &TAG, "Tags".to_string(), on, |s: &mut App, tx| s.toggle_tags(tx)).role("switch").checked(on)
    });
    let settings = foot_button(v, "foot-settings", &GEAR, keymap::tip(s.preset(), "Settings", "app.settings"), false, |s: &mut App, tx| s.open_settings(tx)).role("button");
    row()
        .key("side-foot")
        .w_full()
        .px(BAR_PAD)
        .py(6.0)
        .gap(4.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .border_layout()
        .children(tags)
        .child(el().grow(1.0))
        .child(settings)
}

/// A footer button: an icon in a 28 pt target, the hover fill under the pointer, the selected
/// fill while `on`.
fn foot_button(v: V, key: &str, glyph: &'static rune::Icon, help: String, on: bool, run: impl Fn(&mut App, &mut rune::Tx) + 'static) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let (rw, rg, rc) = super::focus_ring(p);
    let name = help.split(" (").next().unwrap_or_default().to_string();
    super::focus::stop(row())
        .key(key)
        .size(BUTTON, BUTTON)
        .center()
        .radius(6.0)
        .fill(if on { c.selected } else { rune::Color::TRANSPARENT })
        .hover(c.hover)
        .label(name)
        .help(help)
        .focus_ring(rw, rg, rc)
        .keeps_text_selection()
        .control(run)
        .child(icon(glyph, ICON).color(if on { c.text } else { c.dim }))
}
