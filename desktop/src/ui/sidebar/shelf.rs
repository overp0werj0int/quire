//! The Open shelf's tabs on the shelf's darker surface (SB-02, SB-14 to SB-24). Each tab is two
//! rows, the note's name over its folder, with a close button on hover and always on the
//! active tab. A tab dragged up or down moves within the shelf; Alt+Up and Alt+Down move the
//! focused one, Delete and Backspace close it. In Folder mode a tab per open note, in Note mode
//! one per note opened as a file.

use rune::{Anim, Color, Cursor, El, Key, KeyEvent, Mods, Phase, PointerButton, Spring, col, icon, row, text};

use super::super::V;
use super::super::lists;
use super::super::panel;
use super::super::styles::nav;
use super::super::widgets::{CLOSE, name_tip};
use super::BUTTON_END;
use super::parts::{TWO_ROWS_H, caption_style, two_rows};
use super::tree::last_part;
use crate::app::{App, Shelved};
use crate::md;
use crate::roles::Roles;
use crate::sidebar::Reorder;

/// The size of a tab's close button: the smallest pointer target. The drawn icon stays small.
const TARGET: f32 = 24.0;
/// A tab's height.
const TAB_H: f32 = TWO_ROWS_H;
/// The room between two tabs.
const GAP: f32 = 1.0;
/// The spring the tabs a lifted tab passes step aside on.
const STEP: Spring = Spring::new(0.18, 0.0);

/// The active note's H1 headings, in order: the outline under its tab (SB-17, SB-27). Setext
/// headings and headings in code blocks are left out, as the outline panel leaves them out. A
/// locked note has none (SB-133).
pub(super) fn sections(v: V) -> Vec<md::Heading> {
    md::headings(v.s.active_text()).into_iter().filter(|h| h.level == 1).collect()
}

/// " · N sections", which the active tab's caption adds; nothing for a note with no H1 (SB-17).
fn count_text(n: usize) -> Option<String> {
    match n {
        0 => None,
        1 => Some(" · 1 section".into()),
        n => Some(format!(" · {n} sections")),
    }
}

/// How many tabs the shelf has.
pub(super) fn count(v: V) -> usize {
    v.s.shelf().len()
}

/// The height of `n` tabs.
pub(super) fn height(n: usize) -> f32 {
    n as f32 * TAB_H + n.saturating_sub(1) as f32 * GAP
}

/// The shelf's tabs, one Tab stop that enters at the active tab. With no note open there are
/// none. Folded, the active tab's alone, which has nowhere to move (SB-44).
pub(super) fn rows(v: V, folded: bool) -> El {
    let s = v.s;
    let shelf = s.shelf();
    let on = s.shelved();
    // A drag the shelf's tabs changed under is let go of (`App::reorder`).
    let drag = s.reorder.filter(|r| !folded && r.len() == shelf.len());
    let heads = sections(v);
    let mut list = lists::list(col().key("open-notes").w_full().gap(GAP), "list");
    for (i, t) in shelf.iter().enumerate() {
        if folded && on.as_ref() != Some(t) {
            continue;
        }
        let active = on.as_ref() == Some(t);
        let tab = tab(v, t, active, if active { heads.len() } else { 0 }).position_in_set(i + 1, shelf.len());
        list = list.child(if folded { tab } else { reorder(v, tab, i, shelf.len(), drag) });
    }
    list
}

/// A tab's two rows: the note's name, cut at its end, over its folder's path, cut at its start
/// (SB-15, SB-128). The active tab has the selected fill and its close button always shows;
/// another shows its button under the pointer (SB-16). The active tab's caption ends with the
/// count of its note's `sections`, shown whole (SB-17, SB-128). A click on another tab switches
/// to it, a click on the active tab does nothing yet, and a middle click closes either (SB-19).
fn tab(v: V, t: &Shelved, on: bool, sections: usize) -> El {
    let s = v.s;
    let c = Roles::panel(v.p);
    let (title, folder, tip) = match t {
        Shelved::Tab(p) => {
            let dir = md::dir(p);
            let folder = if dir.is_empty() { s.vault.as_ref().map_or_else(String::new, |v| v.name.clone()) } else { dir.to_string() };
            (md::base(p).to_string(), folder, p.clone())
        }
        Shelved::File(f) => {
            let name = f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let folder = f.parent().map_or_else(String::new, crate::store::tilde);
            (md::base(&name).to_string(), folder, crate::store::tilde(f))
        }
    };
    let small = caption_style();
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
        .on_press_tx(move |s: &mut App, tx| close_tab(s, &close_t, tx))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    let title_el = text(title.clone()).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().w_full().help_when_truncated(name_tip(&tip));
    let folder_el = text(folder.clone()).font(small).color(c.faint).truncate_head().shrink().min_w(last_part(&folder, small));
    let caption = row().key("caption").shrink().min_w(0.0).child(folder_el).children(count_text(sections).map(|n| text(n).key("sections").font(small).color(c.faint)));
    let mut el = two_rows(t.key(), title_el, caption)
        .pr(BUTTON_END)
        .hover_group()
        .role("listitem")
        .checked(on)
        .label(title)
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle]);
    el = if on { el.fill(c.selected) } else { el.hover(c.hover) };
    let (key_t, press_t) = (t.clone(), t.clone());
    let el = lists::item(el, v.p, on, move |s: &mut App, tx| {
        if tx.button() == PointerButton::Middle {
            close_tab(s, &press_t, tx);
            return;
        }
        // The release of a drag that lifted the tab is no click (SB-21), and a click on the
        // active tab folds its outline out (SB-26), in a later stage.
        if (std::mem::take(&mut s.dropped) && !tx.by_keys()) || s.shelved().as_ref() == Some(&press_t) {
            return;
        }
        match &press_t {
            Shelved::Tab(p) => s.open_file(p, None, tx),
            Shelved::File(f) => {
                s.open_note(f);
            }
        }
        s.close_drawer();
        s.keys_to_note(tx);
    });
    el.on_key(move |s: &mut App, ev: &KeyEvent, tx| tab_key(s, &key_t, ev, tx)).child(close)
}

/// Closes `t` by its button or a middle click. The note shown goes with its editor, so a
/// keyboard that was in it goes to the note shown now (SB-20); a keyboard elsewhere stays.
fn close_tab(s: &mut App, t: &Shelved, tx: &mut rune::Tx) {
    let in_note = s.shelved().as_ref() == Some(t) && tx.focused().is_some_and(|k| crate::focus::stop(k).is_none());
    s.close_shelved(t);
    if in_note {
        s.keys_to_note(tx);
    }
}

/// A focused tab's own keys: Alt+Up and Alt+Down move it a place and the keyboard stays on it
/// (SB-85); Delete and Backspace close it (SB-86).
fn tab_key(s: &mut App, t: &Shelved, ev: &KeyEvent, tx: &mut rune::Tx) -> bool {
    let alt = Mods { alt: true, ..Mods::default() };
    match (&ev.key, ev.mods) {
        (Key::Up | Key::Down, m) if m == alt => {
            let shelf = s.shelf();
            if let Some(i) = shelf.iter().position(|x| x == t) {
                let to = if ev.key == Key::Up { i.checked_sub(1) } else { Some(i + 1).filter(|&n| n < shelf.len()) };
                if let Some(to) = to {
                    s.move_shelved(i, to);
                }
            }
            true
        }
        (Key::Delete | Key::Backspace, m) if m == Mods::default() => {
            s.close_focused_tab(t, tx);
            true
        }
        _ => false,
    }
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
                s.reorder = Some(Reorder::press(i, len, TAB_H + GAP));
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
