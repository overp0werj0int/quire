//! The Open shelf: the open notes on the shelf's darker surface (SB-02, SB-14). In Folder mode
//! one row per tab, in Note mode one per note opened as a file. Two-row tabs come with stage 2
//! of the sidebar's plan; the rows are today's.

use rune::{El, PointerButton, col, icon, row, text};

use super::super::V;
use super::super::lists;
use super::super::panel;
use super::super::styles::nav;
use super::super::widgets::{CLOSE, name_tip};
use super::BUTTON_END;
use crate::app::App;
use crate::md;
use crate::roles::Roles;

/// The size of a row's close button: the smallest pointer target. The drawn icon stays small.
const TARGET: f32 = 24.0;

/// How many rows the shelf has: the tabs whose notes exist, or Note mode's notes.
pub(super) fn count(v: V) -> usize {
    if v.s.note_mode() { v.s.notes.len() } else { v.s.tabs.iter().filter(|t| v.s.files().contains_key(*t)).count() }
}

/// The shelf's rows, one Tab stop. With no note open there are none. Folded, the active tab's
/// alone (SB-44).
pub(super) fn rows(v: V, folded: bool) -> El {
    let s = v.s;
    let mut list = lists::list(col().key("open-notes").w_full(), "list");
    if s.note_mode() {
        let shown = s.note_path();
        for file in &s.notes {
            list = list.child(note_row(v, file, shown.as_ref() == Some(file)));
        }
        return list;
    }
    for path in s.tabs.iter().filter(|t| s.files().contains_key(*t) && (!folded || s.active.as_ref() == Some(*t))) {
        list = list.child(open_row(v, path));
    }
    list
}

/// A row of Note mode's shelf: a note opened as a file, shown by its file name.
fn note_row(v: V, file: &std::path::Path, on: bool) -> El {
    let c = Roles::panel(v.p);
    let name = file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let (open_file, close_file) = (file.to_path_buf(), file.to_path_buf());
    let close = row()
        .key("x")
        .size(TARGET, TARGET)
        .center()
        .radius(3.0)
        .hover(c.hover)
        .label("Close note")
        .help("Close note")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press(move |s: &mut App| s.close_note(&close_file))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    let shade = panel::shade(format!("note:{}", file.display()), panel::BODY, 0, 0.0, &c, on)
        .pr(BUTTON_END)
        .hover_group()
        .role("listitem")
        .checked(on)
        .label(name.clone())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle]);
    lists::opener(shade, v.p, on, move |s: &mut App, tx| {
        if tx.button() == PointerButton::Middle {
            s.close_note(&open_file);
        } else {
            s.open_note(&open_file);
        }
    })
    .child(text(name).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(name_tip(&crate::store::tilde(file))))
    .child(close)
}

/// A tab's row: the note's name, with its close button on hover and on the active tab. A
/// middle click closes it too.
fn open_row(v: V, path: &str) -> El {
    let c = Roles::panel(v.p);
    let on = v.s.active.as_deref() == Some(path);
    let (open_path, close_path) = (path.to_string(), path.to_string());
    let close = row()
        .key("x")
        .size(TARGET, TARGET)
        .center()
        .radius(3.0)
        .hover(c.hover)
        .label("Close tab")
        .help("Close tab")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press(move |s: &mut App| s.close_tab(&close_path))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    let shade = panel::shade(format!("tab:{path}"), panel::BODY, 0, 0.0, &c, on)
        .pr(BUTTON_END)
        .hover_group()
        .role("listitem")
        .checked(on)
        .label(md::base(path).to_string())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle]);
    lists::opener(shade, v.p, on, move |s: &mut App, tx| {
        if tx.button() == PointerButton::Middle {
            s.close_tab(&open_path);
        } else if s.active.as_deref() != Some(open_path.as_str()) {
            s.open_file(&open_path, None, tx);
        }
    })
    .child(text(md::base(path)).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(name_tip(path)))
    .child(close)
}
