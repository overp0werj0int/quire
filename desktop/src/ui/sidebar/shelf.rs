//! The Open shelf's tabs on the shelf's darker surface (SB-02, SB-14, SB-21). A tab dragged up
//! or down moves within the shelf. In Folder mode a tab per open note, in Note mode one per
//! note opened as a file. Two-row tabs come next; the rows are today's.

use rune::{Anim, Color, Cursor, El, Phase, PointerButton, Spring, col, icon, row, text};

use super::super::V;
use super::super::lists;
use super::super::panel;
use super::super::styles::nav;
use super::super::widgets::{CLOSE, name_tip};
use super::BUTTON_END;
use crate::app::{App, Shelved};
use crate::md;
use crate::roles::Roles;
use crate::sidebar::Reorder;

/// The size of a row's close button: the smallest pointer target. The drawn icon stays small.
const TARGET: f32 = 24.0;
/// The spring the tabs a lifted tab passes step aside on.
const STEP: Spring = Spring::new(0.18, 0.0);

/// How many tabs the shelf has.
pub(super) fn count(v: V) -> usize {
    v.s.shelf().len()
}

/// The shelf's tabs, one Tab stop. With no note open there are none. Folded, the active tab's
/// alone, which has nowhere to move (SB-44).
pub(super) fn rows(v: V, folded: bool) -> El {
    let s = v.s;
    let shelf = s.shelf();
    let on = s.shelved();
    // A drag the shelf's tabs changed under is let go of (`App::reorder`).
    let drag = s.reorder.filter(|r| !folded && r.len() == shelf.len());
    let mut list = lists::list(col().key("open-notes").w_full(), "list");
    for (i, t) in shelf.iter().enumerate() {
        if folded && on.as_ref() != Some(t) {
            continue;
        }
        let tab = tab(v, t, on.as_ref() == Some(t)).position_in_set(i + 1, shelf.len());
        list = list.child(if folded { tab } else { reorder(v, tab, i, shelf.len(), drag) });
    }
    list
}

/// A tab's row: the note's name, with its close button on hover and on the active tab. A
/// middle click closes it too.
fn tab(v: V, t: &Shelved, on: bool) -> El {
    let c = Roles::panel(v.p);
    let (name, tip) = match t {
        Shelved::Tab(p) => (md::base(p).to_string(), p.clone()),
        Shelved::File(f) => (f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()), crate::store::tilde(f)),
    };
    let close_t = t.clone();
    let close = row()
        .key("x")
        .size(TARGET, TARGET)
        .center()
        .radius(3.0)
        .hover(c.hover)
        .label("Close tab")
        .help("Close tab")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press(move |s: &mut App| s.close_shelved(&close_t))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    let shade = panel::shade(t.key(), panel::BODY, 0, 0.0, &c, on)
        .pr(BUTTON_END)
        .hover_group()
        .role("listitem")
        .checked(on)
        .label(name.clone())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle]);
    let press_t = t.clone();
    lists::opener(shade, v.p, on, move |s: &mut App, tx| {
        if tx.button() == PointerButton::Middle {
            s.close_shelved(&press_t);
        } else if std::mem::take(&mut s.dropped) && !tx.by_keys() {
            // The release of a drag that lifted the tab is no click (SB-21).
        } else if s.shelved().as_ref() != Some(&press_t) {
            match &press_t {
                Shelved::Tab(p) => s.open_file(p, None, tx),
                Shelved::File(f) => {
                    s.open_note(f);
                }
            }
        }
    })
    .child(text(name).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(name_tip(&tip)))
    .child(close)
}

/// A tab the pointer can drag to a new place, `i` of `len` (SB-21). Past a few points it lifts
/// and follows the pointer, over the others, and the tabs it passes step aside on springs. The
/// drop puts it there; Esc, or the window losing focus, puts it back.
fn reorder(v: V, el: El, i: usize, len: usize, drag: Option<Reorder>) -> El {
    let c = Roles::panel(v.p);
    let el = match drag {
        Some(r) if r.lifted() == Some(i) => {
            // Opaque, so the tabs it passes do not show through.
            let fill = if v.s.shelved().is_some_and(|t| v.s.shelf().get(i) == Some(&t)) { c.selected } else { c.hover };
            el.raise()
                .offset(0.0, r.shift(i))
                .offset_motion(Anim::INSTANT)
                .fill(crate::roles::mix(c.shelf, Color { a: 1.0, ..fill }, fill.a))
                .shadow(2.0, 8.0, Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 })
                .cursor(Cursor::Grabbing)
        }
        Some(r) => el.offset(0.0, r.shift(i)).offset_motion(STEP),
        None => el,
    };
    el.on_drag(move |s: &mut App, d: &mut rune::Drag, tx| {
        if d.cancelled() {
            s.dropped = s.reorder.take().is_some_and(|r| r.lifted().is_some());
            return;
        }
        match d.phase {
            Phase::Down => {
                s.dropped = false;
                s.reorder = Some(Reorder::press(i, len, panel::row_h(panel::BODY)));
            }
            Phase::Move => {
                if let Some(r) = &mut s.reorder {
                    r.drag(d.dy);
                    if r.lifted().is_some() {
                        tx.animated();
                    }
                }
            }
            Phase::Up => {
                tx.instant();
                let r = s.reorder.take();
                s.dropped = r.is_some_and(|r| r.lifted().is_some());
                if let Some((from, to)) = r.and_then(Reorder::end) {
                    s.move_shelved(from, to);
                }
            }
        }
    })
}
