//! The Open shelf's tabs on the shelf's darker surface (SB-02, SB-14 to SB-24). Each tab is two
//! rows, the note's name over its folder, with a close button on hover and always on the
//! active tab. A click on the active tab folds its note's outline out under it, its H1s one
//! entry each, and the next folds it back. A tab dragged up or down moves within the shelf;
//! Alt+Up and Alt+Down move the focused one, Delete and Backspace close it. In Folder mode a
//! tab per open note, in Note mode one per note opened as a file.

use rune::{Anim, Color, Cursor, El, Key, KeyEvent, Live, Mods, Phase, PointerButton, Spring, Tx, col, icon, row, text};

use super::super::V;
use super::super::lists;
use super::super::styles::nav;
use super::super::icons::{CHEVRON_DOWN, CHEVRON_RIGHT, CLOSE, LOCK, WARNING};
use super::super::widgets::name_tip;
use super::{BUTTON_END, ICON};
use super::parts::{GAP, INNER_RADIUS, INSET, OUT_TEXT, RADIUS, ROW_TEXT, TAB_TITLE_LH, TWO_ROWS_H, caption_style, two_rows};
use super::tree::last_part;
use crate::app::{App, Shelved};
use crate::md;
use crate::roles::Roles;
use crate::sidebar::Reorder;

/// The size of a tab's close button: the smallest pointer target. The drawn icon stays 14 pt.
const TARGET: f32 = 24.0;
/// A tab's height.
const TAB_H: f32 = TWO_ROWS_H;
/// The spring the tabs a lifted tab passes step aside on.
const STEP: Spring = Spring::new(0.18, 0.0);
/// An outline entry's height: its 11.5 pt line and the room above and below it. The drawing's
/// is about 22, but an entry has no fill to show where it ends, so it takes the least pointer
/// target (SB-08, SB-30).
const ENTRY_H: f32 = 24.0;
/// The outline's guide line, in from the tab's edge (SB-30).
const GUIDE_X: f32 = 16.0;
/// An entry's edge in from the guide line (7 pt clears the 5 pt focus ring by 2 pt) and its
/// text padding (5 pt), keeping the text at the spec's 12 pt past the guide line.
const ENTRY_IN: f32 = 7.0;
const ENTRY_PAD: f32 = 5.0;

/// The active note's H1 headings, in order: the outline under its tab (SB-17, SB-27). Setext
/// headings and headings in code blocks are left out, as the outline panel leaves them out. A
/// locked note has none (SB-133). Uses the cached heading index (`LIVEPACE`).
pub(super) fn sections(v: V) -> std::rc::Rc<Vec<md::Heading>> {
    v.s.headings_cached().1
}

/// "N sections", which the active tab's caption adds after its folder and a middle dot; nothing
/// for a note with no H1 (SB-17).
fn count_text(n: usize) -> Option<String> {
    match n {
        0 => None,
        1 => Some("1 section".into()),
        n => Some(format!("{n} sections")),
    }
}

/// Symmetric horizontal gap on each side of the caption's middle dot `·`.
const DOT_GAP: f32 = 4.0;

/// How many tabs the shelf has.
pub(super) fn count(v: V) -> usize {
    v.s.shelf().len()
}

/// The height of `n` tabs.
pub(super) fn height(n: usize) -> f32 {
    n as f32 * TAB_H + n.saturating_sub(1) as f32 * GAP
}

/// The height of the shelf's rows: its tabs, and the active tab's outline while it is out
/// (SB-35).
pub(super) fn fit(v: V) -> f32 {
    let n = sections(v).len();
    height(count(v)) + if is_out(v, n) { outline_h(n) } else { 0.0 }
}

/// The height an outline of `n` entries adds under its tab.
fn outline_h(n: usize) -> f32 {
    GAP + n as f32 * ENTRY_H + n.saturating_sub(1) as f32 * GAP
}

/// Whether the active tab's outline, of `n` entries, is out (SB-26, SB-27).
fn is_out(v: V, n: usize) -> bool {
    n > 0 && v.s.outline_out.is_some() && v.s.outline_out == v.s.shelved()
}

/// The shelf's tabs, one Tab stop that enters at the active tab, with its outline's entries
/// after it while it is out (SB-82). With no note open there are none. Folded, the active tab's
/// alone, which has nowhere to move, and still folds its outline out (SB-44).
pub(super) fn rows(v: V, folded: bool) -> El {
    let s = v.s;
    let shelf = s.shelf();
    let on = s.shelved();
    // A drag the shelf's tabs changed under is let go of (`App::reorder`).
    let drag = s.reorder.filter(|r| !folded && r.len() == shelf.len());
    let heads = sections(v);
    let out = is_out(v, heads.len());
    let at = shelf.iter().position(|t| on.as_ref() == Some(t));
    let mut list = lists::list(col().key("open-notes").w_full().gap(GAP), "list");
    for (i, t) in shelf.iter().enumerate() {
        if folded && on.as_ref() != Some(t) {
            continue;
        }
        let active = on.as_ref() == Some(t);
        let tab = tab(v, t, active, if active { heads.len() } else { 0 }, active && out).position_in_set(i + 1, shelf.len());
        // A tab under the outline moves up by it when a lift folds it (SB-22).
        let lead = if out && at.is_some_and(|a| i > a) { outline_h(heads.len()) } else { 0.0 };
        list = list.child(if folded { tab } else { reorder(v, tab, i, shelf.len(), drag, lead) });
        if active && out {
            list = list.child(outline(v, t, &heads));
        }
    }
    list
}

/// A tab's two rows: the note's name, cut at its end, over its folder's path, cut at its start
/// (SB-15, SB-128). The active tab has the selected fill and its close button always shows;
/// another shows its button under the pointer (SB-16). The active tab's caption ends with the
/// count of its note's `sections`, shown whole (SB-17, SB-128). A click on another tab switches
/// to it, a click on the active tab folds its outline out or back, `out` while it is, and a
/// middle click closes either (SB-19, SB-26). A note with no H1 has no outline to fold
/// (SB-27).
fn tab(v: V, t: &Shelved, on: bool, sections: usize, out: bool) -> El {
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
        .radius(INNER_RADIUS)
        .hover(c.hover)
        .label("Close tab")
        .help("Close tab")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press_tx(move |s: &mut App, tx| close_tab(s, &close_t, tx))
        .child(icon(&CLOSE, ICON).color(if on { c.dim } else { c.faint }));
    let lock_state = match t {
        Shelved::Tab(p) if s.is_locked(p) => Some((p.clone(), s.note_unlocked(p))),
        Shelved::File(_) if on => s.active.as_deref().filter(|a| s.is_locked(a)).map(|a| (a.to_string(), s.note_unlocked(a))),
        _ => None,
    };
    let conflict_path = match t {
        Shelved::Tab(p) if md::conflict_origin(p).is_some() => Some(p.clone()),
        Shelved::File(_) if on => s.active.as_deref().filter(|a| md::conflict_origin(a).is_some()).map(str::to_string),
        _ => None,
    };
    let title_text = text(title.clone()).font(nav(ROW_TEXT, false).lh(TAB_TITLE_LH)).color(if on { c.text } else { c.dim }).truncate().help_when_truncated(name_tip(&tip));
    let title_el = if lock_state.is_some() || conflict_path.is_some() {
        let mut r = row().w_full().gap(4.0).center_y().child(title_text.shrink().min_w(0.0));
        if let Some(cp) = conflict_path {
            r = r.child(
                icon(&WARNING, 12.0)
                    .key(format!("conflict-tab:{cp}"))
                    .color(v.p.err)
                    .label("Sync conflict copy")
                    .help("Sync conflict copy"),
            );
        }
        if let Some((lp, unlocked)) = lock_state {
            let tip = if unlocked { "Unlocked in this session" } else { "Locked note" };
            r = r.child(
                icon(&LOCK, 12.0)
                    .key(format!("lock-tab:{lp}"))
                    .color(if unlocked { v.p.acc } else { v.p.mute })
                    .label(tip)
                    .help(tip),
            );
        }
        r
    } else {
        title_text.w_full()
    };
    let chev_id = match t {
        Shelved::Tab(p) => p.clone(),
        Shelved::File(f) => f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
    };
    let folder_el = text(folder.clone()).font(small).color(c.faint).truncate_head().shrink().min_w(last_part(&folder, small)).help_when_truncated(folder);
    let caption = row().key("caption").shrink().min_w(0.0).child(folder_el).children(count_text(sections).map(|n| {
        let mut sec = row()
            .key("sections")
            .ml(DOT_GAP)
            .gap(DOT_GAP)
            .center_y()
            .child(text("·").key("sections-dot").font(small).color(c.faint))
            .child(text(n).key("sections-count").font(small).color(c.faint));
        if on {
            let tip_text = if out { "Collapse outline" } else { "Expand outline" };
            sec = sec.child(
                icon(if out { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 10.0)
                    .key(format!("tab-chev:{chev_id}"))
                    .color(c.dim)
                    .label(tip_text)
                    .help(tip_text),
            );
        }
        sec
    }));
    let mut el = two_rows(t.key(), title_el, caption)
        .pr(BUTTON_END)
        .hover_group()
        .role("listitem")
        .checked(on)
        .label(title)
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle]);
    if sections > 0 {
        el = el.expanded(out);
    }
    el = if on { el.fill(c.selected) } else { el.hover(c.hover) };
    let (key_t, press_t, menu_t) = (t.clone(), t.clone(), t.clone());
    let el = lists::item(el, v.p, on, move |s: &mut App, tx| {
        if tx.button() == PointerButton::Middle {
            close_tab(s, &press_t, tx);
            return;
        }
        // The release of a drag that lifted the tab is no click (SB-21).
        if std::mem::take(&mut s.dropped) && !tx.by_keys() {
            return;
        }
        // On the active tab a click, Enter or Space folds its outline, and the keyboard stays
        // (SB-26, SB-83), including inside a narrow overlay sidebar drawer when the note has
        // sections to expand. When there are no sections, activating the active tab in a narrow
        // drawer closes the drawer and returns to the note.
        if s.shelved().as_ref() == Some(&press_t) {
            if sections > 0 {
                fold(s, &press_t, !out, tx);
                return;
            }
            if s.drawer() == Some(crate::app::Drawer::Sidebar) {
                s.close_drawer(tx);
                s.keys_to_note(tx);
            }
            return;
        }
        match &press_t {
            Shelved::Tab(p) => s.open_file(p, None, tx),
            Shelved::File(f) => {
                s.open_note(f);
            }
        }
        s.close_drawer(tx);
        s.keys_to_note(tx);
    });
    el.on_key(move |s: &mut App, ev: &KeyEvent, tx| tab_key(s, &key_t, sections, ev, tx))
        .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(crate::layer::MenuOf::Tab(menu_t.clone()), (at.x, at.y), tx))
        .child(close)
}

/// Folds the active tab `t`'s outline out, or back. Out, the shelf scrolls to show it, and its
/// tab, should both not fit (SB-24, SB-26).
fn fold(s: &mut App, t: &Shelved, out: bool, tx: &mut Tx) {
    s.outline_out = out.then(|| t.clone());
    if out {
        tx.scroll_into_view_in("shelf-scroll", "toc");
        tx.scroll_into_view_in("shelf-scroll", t.key());
    }
}

/// The active tab `t`'s outline: its note's H1s, one entry each, indented under the tab beside
/// a guide line, in faint and never filled; the entry of the section in view in the accent
/// (SB-27, SB-29, SB-30). In the writer that is the last H1 at or before the caret's line. In
/// the reader it is the last H1 at or above the view's top, by the outline panel's rule, and
/// the page's scroll handler moves it through live colours with no view run
/// (`reader::PageMap::mark`). Above the first H1 no entry is marked.
fn outline(v: V, t: &Shelved, heads: &[md::Heading]) -> El {
    let s = v.s;
    let c = Roles::panel(v.p);
    let mut map = s.reader.map.borrow_mut();
    let live = map.reading || map.writing;
    let cur = if live {
        if map.reading {
            let blocks = s.doc();
            map.shelf.heads = heads
                .iter()
                .enumerate()
                .filter_map(|(n, h)| {
                    let k = blocks.blocks.partition_point(|b| b.lines.start < h.line);
                    blocks.blocks.get(k).is_some_and(|b| b.lines.start == h.line).then_some((k, Some(n)))
                })
                .collect();
        } else {
            map.shelf.heads = heads.iter().enumerate().map(|(n, h)| (h.line, Some(n))).collect();
        }
        map.shelf.first_row = None;
        let cur = if map.writing && map.source_from_caret {
            let doc = s.active_text();
            let caret = s.active.as_ref().and_then(|a| s.edits.get(a)).map_or(0, |e| e.focus.min(doc.len()));
            let line = md::line_col(doc, caret).0;
            crate::reader::PageMap::head_row_line(&map.shelf, line)
        } else {
            map.head_row(&map.shelf, s.reader.scroll.get())
        };
        map.shelf.marked = cur;
        cur
    } else if crate::ui::mapping(s) {
        // On the map the H1 holding the selected node is marked (MAP-067).
        s.map_heading_line(1).and_then(|l| heads.iter().position(|h| h.line == l))
    } else {
        None
    };
    let mut out = col().key("toc").ml(INSET + GUIDE_X).mr(INSET).gap(GAP).border_sides([0.0, 0.0, 0.0, 1.0], v.p.line).border_layout();
    for (n, h) in heads.iter().enumerate() {
        let on = cur == Some(n);
        let ink = |on: bool| if on { c.accent_text } else { c.faint };
        let mut name = text(h.text.clone()).key(format!("shelf-h:{n}")).font(nav(OUT_TEXT, false)).color(ink(on)).truncate().shrink().min_w(0.0).help_when_truncated(h.text.clone());
        if live {
            // The ink alone: an entry is never filled (SB-30).
            let mark = crate::reader::Mark { on: [Color::TRANSPARENT, ink(true)], off: [Color::TRANSPARENT, ink(false)], fill: Live::new(Color::TRANSPARENT), ink: Live::new(ink(on)) };
            name = name.live_text_color(&mark.ink);
            map.shelf.marks.push(mark);
        }
        out = out.child(entry(v, t, n, h, name));
    }
    out
}

/// Entry `n` of tab `t`'s outline, for heading `h`, its text `name`. A row of the shelf's list:
/// a click scrolls the note to its heading, as the outline panel's rows do, and Enter or Space
/// does too and gives the note the keyboard (SB-28, SB-83). Left moves the keyboard to its tab
/// (SB-84). It has no hover mark (SB-31).
fn entry(v: V, t: &Shelved, n: usize, h: &md::Heading, name: El) -> El {
    let line = h.line;
    let el = row().key(format!("toc:{n}")).h(ENTRY_H).ml(ENTRY_IN).px(ENTRY_PAD).center_y().radius(RADIUS).role("listitem").label(h.text.clone());
    let tab = t.key();
    lists::item(el, v.p, false, move |s: &mut App, tx| {
        s.jump(line, tx);
        if s.drawer() == Some(crate::app::Drawer::Sidebar) || tx.by_keys() {
            s.close_drawer(tx);
            s.keys_to_note(tx);
        }
    })
    .on_key(move |_: &mut App, ev: &KeyEvent, tx| {
        if ev.key != Key::Left || ev.mods != Mods::default() {
            return false;
        }
        tx.focus(tab.clone(), true);
        true
    })
    .child(name)
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

/// A focused tab's own keys: on the active tab, with `sections` to list, Right folds its
/// outline out and Left folds it back, and on another they do nothing (SB-84); Alt+Up and
/// Alt+Down move it a place and the keyboard stays on it (SB-85); Delete and Backspace close it
/// (SB-86).
fn tab_key(s: &mut App, t: &Shelved, sections: usize, ev: &KeyEvent, tx: &mut rune::Tx) -> bool {
    let alt = Mods { alt: true, ..Mods::default() };
    match (&ev.key, ev.mods) {
        (Key::Right | Key::Left, m) if m == Mods::default() => {
            let out = ev.key == Key::Right;
            if sections > 0 && s.shelved().as_ref() == Some(t) && (s.outline_out.as_ref() == Some(t)) != out {
                fold(s, t, out, tx);
            }
            true
        }
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
/// A lift folds the active tab's outline back, so every tab is one height while it moves; a tab
/// under it takes `lead`, how far that moves it up (SB-22).
fn reorder(v: V, el: El, i: usize, len: usize, drag: Option<Reorder>, lead: f32) -> El {
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
                    let held = r.lifted().is_none();
                    r.drag(d.dy);
                    if r.lifted().is_some() {
                        if held && s.outline_out.take().is_some() {
                            r.lead(lead);
                        }
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

#[cfg(test)]
mod tests {
    #[test]
    fn narrow_drawer_active_tab_chevron_expands_outline_and_entry_closes_drawer() {
        let dir = std::env::temp_dir().join(format!("quire-drawer-chev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Alpha.md"), "# First\n\nBody.\n\n# Second\n\nMore.\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (640.0, 720.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Alpha.md", None, tx);
            s.mode = crate::app::Mode::Source;
            s.open_layer(crate::layer::Layer::Drawer(crate::app::Drawer::Sidebar), tx);
        });
        for _ in 0..10 {
            rt.frame(1.0 / 60.0);
        }
        let chev = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("tab-chev:Alpha.md")).expect("tab-chev in drawer");
        let (cx, cy) = (chev.rect[0] + chev.rect[2] / 2.0, chev.rect[1] + chev.rect[3] / 2.0);
        rt.pointer_move(cx, cy);
        rt.pointer_down(cx, cy);
        rt.pointer_up(cx, cy);
        for _ in 0..10 {
            rt.frame(1.0 / 60.0);
        }
        assert_eq!(rt.state.drawer(), Some(crate::app::Drawer::Sidebar), "tapping chevron keeps drawer open");
        let els = rt.elements();
        assert!(els.iter().any(|e| e.key.as_deref() == Some("shelf-h:0")), "expanded outline exposes shelf-h:0");
        let toc1 = els.iter().find(|e| e.key.as_deref() == Some("toc:1")).expect("toc:1 entry");
        let (tx, ty) = (toc1.rect[0] + 20.0, toc1.rect[1] + toc1.rect[3] / 2.0);
        rt.pointer_move(tx, ty);
        rt.pointer_down(tx, ty);
        rt.pointer_up(tx, ty);
        for _ in 0..10 {
            rt.frame(1.0 / 60.0);
        }
        assert_eq!(rt.state.drawer(), None, "clicking an outline heading closes the drawer");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
