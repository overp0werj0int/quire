//! The two lists: the Open shelf over the vault, each under its label, with the handle between
//! them (SB-14, SB-33 to SB-48). A label folds its section; the handle moves the split. Both
//! are kept once for the app in `state.json` (`crate::sidebar::Layout`).

use rune::{El, Phase, ScrollSpec, col, el, icon};

use super::super::V;
use super::super::panel;
use super::super::widgets::{CHEVRON_DOWN, Control};
use super::parts::{LABEL_H, label_row};
use super::{BAR_PAD, BUTTON, shelf, tree};
use crate::app::App;
use crate::roles::Roles;
use crate::sidebar::{Section, split};

/// The room above a label.
const LABEL_TOP: f32 = 2.0;
/// The handle: 9 pt, its two lines inside (SB-33).
const HANDLE: f32 = 9.0;
/// The room under the shelf's last row.
const SHELF_END: f32 = 6.0;

/// The height the shelf and the vault share: the window less its corner, the top bar and the
/// footer.
fn room(v: V) -> f32 {
    let bars = 2.0 * (BUTTON + 1.0) + 2.0 * BAR_PAD + 2.0 * 6.0;
    (v.vh - super::super::titlebar::BAR_H - bars).max(0.0)
}

/// The shelf's height for rows `rows` tall.
fn shelf_h(rows: f32) -> f32 {
    LABEL_TOP + LABEL_H + panel::RING_ROOM + rows + SHELF_END
}

/// The vault's least height: its label and two rows (SB-36).
fn vault_min() -> f32 {
    LABEL_TOP + LABEL_H + panel::RING_ROOM + 2.0 * panel::row_h(panel::BODY)
}

/// The shelf over the vault. With no note open the vault takes the whole room and its label
/// does not fold, since there is nothing to give the room to (SB-125). In Note mode the shelf
/// alone, its label with no chevron (SB-119).
pub(super) fn lists(v: V) -> El {
    let s = v.s;
    let n = shelf::count(v);
    let out = col().key("lists").w_full().grow(1.0).min_h(0.0);
    if s.note_mode() {
        // The shelf takes the room down to the footer, its label fixed (SB-119).
        return out.child(shelf_el(v, n, None, false).grow(1.0));
    }
    if n == 0 {
        return out.child(vault_el(v, false, false));
    }
    match s.persist.sidebar.folded {
        Some(Section::Vault) => out.child(shelf_el(v, n, None, true).grow(1.0)).child(vault_el(v, true, true)),
        Some(Section::Shelf) => out.child(shelf_el(v, n, None, true)).child(vault_el(v, true, false)),
        None => {
            let room = room(v) - HANDLE;
            let least = shelf_h(shelf::height(1));
            let h = split(s.persist.sidebar.shelf_h, room, shelf_h(shelf::fit(v)), least, vault_min());
            let (lo, hi) = (least, (room - vault_min()).max(least));
            out.child(shelf_el(v, n, Some(h), true)).child(handle(v, h, lo, hi)).child(vault_el(v, true, false))
        }
    }
}

/// The shelf on its darker surface. `h` is its height under the handle; None lets it fit its
/// rows or grow. Folded, it shows only the active tab, with a line under it (SB-44).
fn shelf_el(v: V, n: usize, h: Option<f32>, folds: bool) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let folded = folds && v.s.persist.sidebar.folded == Some(Section::Shelf);
    let count = if folded { format!("{n} {}", if n == 1 { "tab" } else { "tabs" }) } else { n.to_string() };
    let head = label(v, Section::Shelf, "Open", count, folds.then_some(folded));
    let out = col().key("shelf").w_full().min_h(0.0).fill(c.shelf).pt(LABEL_TOP).child(head);
    if folded {
        return out.pb(SHELF_END).border_sides([0.0, 0.0, 1.0, 0.0], p.line).border_layout().child(shelf::rows(v, true));
    }
    let rows = col().key("shelf-scroll").grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).scroll_margin(panel::RING_ROOM).child(col().w_full().pt(panel::RING_ROOM).pb(SHELF_END).child(shelf::rows(v, false)));
    match h {
        Some(h) => out.h(h).child(rows),
        None => out.child(rows),
    }
}

/// The vault: its label, then its tree, or the label alone on one row above the footer when it
/// is folded (SB-42).
fn vault_el(v: V, folds: bool, folded: bool) -> El {
    let s = v.s;
    let name = s.vault.as_ref().map_or(String::new(), |v| v.name.clone());
    let n = s.files().len();
    let count = if folded { format!("{n} {}", if n == 1 { "note" } else { "notes" }) } else { n.to_string() };
    // A right-click, the Menu key or Shift+F10 on the label opens the vault's menu (SB-110).
    let mut head = label(v, Section::Vault, &name, count, folds.then_some(folded)).on_secondary_press(|s: &mut App, at, tx| s.open_menu(crate::layer::MenuOf::Folder, (at.x, at.y), tx));
    // A note dragged onto the label, or the tree's empty space, goes to the root, which the
    // label stands for: it wears the drop target's mark, as a folder's row does (SB-116).
    if s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some("")) {
        head = head.fill(v.p.soft).border(1.0, v.p.acc.alpha(0.45));
    }
    let out = col().key("vault").w_full().pt(LABEL_TOP).child(head);
    if folded {
        return out.pb(LABEL_TOP).border_sides([1.0, 0.0, 0.0, 0.0], v.p.line).border_layout();
    }
    out.grow(1.0).min_h(0.0).child(super::scroll("side-scroll", tree::tree(v)))
}

/// A section's label: a chevron, the name in small capitals and a count (SB-05). `folded` is
/// None for a label that does not fold: no chevron and no Tab stop. One that folds is a
/// button, expanded while its section shows; Enter or Space folds it and the keyboard stays
/// (SB-39, SB-41).
fn label(v: V, of: Section, name: &str, count: String, folded: Option<bool>) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let key = match of {
        Section::Shelf => "label-shelf",
        Section::Vault => "label-vault",
    };
    let Some(folded) = folded else {
        return label_row(p, key, None, name, count);
    };
    let (rw, rg, rc) = super::super::focus_ring(p);
    let turn = if folded { -std::f32::consts::FRAC_PI_2 } else { 0.0 };
    let chevron = icon(&CHEVRON_DOWN, 10.0).color(c.faint).rotate(turn).rotate_motion(rune::Spring::new(0.12, 0.0));
    super::super::focus::stop(label_row(p, key, Some(chevron), name, count))
        .hover(c.hover)
        .role("button")
        .expanded(!folded)
        .label(name.to_string())
        .focus_ring(rw, rg, rc)
        .keeps_text_selection()
        .control(move |s: &mut App, _| {
            s.persist.sidebar.toggle(of);
            s.persist_later();
        })
}

/// The handle between the shelf and the vault, under a shelf `h` tall: a drag moves the split
/// between `lo` and `hi`, the shelf's limits (SB-33, SB-34, SB-36). A drag that is called off puts it back. Not a Tab
/// stop: from the keyboard a section folds instead (SB-37).
fn handle(v: V, h: f32, lo: f32, hi: f32) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    el().key("handle")
        .w_full()
        .h(HANDLE)
        .center()
        .border_sides([1.0, 0.0, 1.0, 0.0], p.line)
        .border_layout()
        .cursor(rune::Cursor::ResizeRow)
        .on_drag(move |s: &mut App, d: &mut rune::Drag, _| {
            if d.cancelled() {
                if let Some(was) = s.handle.take() {
                    s.persist.sidebar.shelf_h = was;
                }
                return;
            }
            match d.phase {
                Phase::Down => {
                    // The shelf's height as drawn, which a drag moves from.
                    d.data = h;
                    s.handle = Some(s.persist.sidebar.shelf_h);
                }
                Phase::Move => s.persist.sidebar.shelf_h = Some((d.data + d.dy).clamp(lo, hi)),
                Phase::Up => {
                    s.handle = None;
                    s.persist_later();
                }
            }
        })
        .child(el().key("grip").size(22.0, 2.0).radius(1.0).fill(c.faint))
}
