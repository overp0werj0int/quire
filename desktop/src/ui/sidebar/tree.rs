//! The vault's tree of folders and notes, with the new folder row and
//! a note being dragged. The rows come from `App::vault_tree`.

use rune::{El, col, el, icon, row, text, text_input};

use super::super::V;
use super::super::lists::{self, Branch};
use super::super::panel;
use super::super::styles::nav;
use super::super::widgets::{CHEVRON_RIGHT, PLUS, floating, label, name_tip};
use super::BUTTON_END;
use crate::app::App;
use crate::folders::{Folder, MenuOn, row_key};
use crate::layer::MenuOf;
use crate::md;
use crate::roles::Roles;
use crate::theme;

/// The size of a folder's "+": the smallest pointer target. The drawn icon stays small.
const TARGET: f32 = 24.0;

/// The tree: every folder and note, one Tab stop.
pub(super) fn tree(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let Some(root) = s.vault_tree() else { return col() };
    let mut rows = Vec::new();
    walk(v, &root, 0, &mut rows);
    let root_target = s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some(""));
    let mut c = lists::list(col().key("tree").w_full(), "tree")
        .on_secondary_press(|s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Root), (at.x, at.y), tx));
    if root_target {
        c = c.fill(p.soft.alpha(0.5)).radius(3.0);
    }
    if root.count == 0 && s.new_folder.is_none() {
        c = c.child(text("No notes yet.").font(nav(panel::BODY_SMALL, false)).color(Roles::panel(p).faint).py(6.0).pl(panel::TEXT_X));
    }
    c.children(rows)
}

/// The key of a folder's first row inside it, where Right moves to.
fn first_row(f: &Folder) -> Option<String> {
    match f.folders.first() {
        Some(sub) => Some(row_key("dir", &sub.path)),
        None => f.notes.first().map(|n| row_key("file", n)),
    }
}

fn walk(v: V, n: &Folder, depth: usize, rows: &mut Vec<El>) {
    let s = v.s;
    if let Some(nf) = &s.new_folder {
        if nf.parent == n.path {
            rows.push(new_folder_row(v, depth, &nf.edit));
        }
    }
    let parent = (!n.path.is_empty()).then(|| row_key("dir", &n.path));
    for f in &n.folders {
        let name = f.name();
        let open = s.expanded.contains(&f.path);
        let path = f.path.clone();
        let menu_path = f.path.clone();
        let add_path = f.path.clone();
        let add = row()
            .key("add")
            .size(TARGET, TARGET)
            .center()
            .radius(3.0)
            .hover(Roles::panel(v.p).hover)
            .label("New note in this folder")
            .help("New note in this folder")
            .group_opacity(0.0, 1.0)
            .on_press_tx(move |s: &mut App, tx| s.new_note_in(Some(&add_path), None, tx))
            .child(icon(&PLUS, 9.0).color(Roles::panel(v.p).faint));
        let branch = Branch::Folder { path: f.path.clone(), open, first: first_row(f) };
        let count = text(f.count.to_string()).key("count").font(nav(panel::BODY_SMALL, false)).color(Roles::panel(v.p).faint);
        let r = lists::item(tree_row(v, &row_key("dir", &f.path), name, &f.path, depth, true, open, false), v.p, false, move |s: &mut App, _| s.toggle_folder(&path)).child(count);
        rows.push(
            lists::tree_arrows(r, branch, parent.clone())
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Folder(menu_path.clone())), (at.x, at.y), tx))
                .child(add),
        );
        if open {
            walk(v, f, depth + 1, rows);
        }
    }
    for f in &n.notes {
        let on = s.active.as_deref() == Some(f.as_str());
        let path = f.clone();
        let drag_path = f.clone();
        let menu_path = f.clone();
        let r = lists::opener(tree_row(v, &row_key("file", f), md::base(f), f, depth, false, false, on), v.p, on, move |s: &mut App, tx| s.open_file(&path, None, tx));
        rows.push(
            lists::tree_arrows(r, Branch::Note, parent.clone())
                .on_drag(move |s: &mut App, d: &mut rune::Drag, tx| s.drag_note(&drag_path, d, tx))
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Note(menu_path.clone())), (at.x, at.y), tx)),
        );
    }
}

/// The room a cut folder keeps: "…" and its last part, up to 72 pt, so a long folder shows
/// where the note is rather than a bare "…".
pub(super) fn last_part(dir: &str, style: rune::TextStyle) -> f32 {
    match dir.rsplit_once('/') {
        Some((_, last)) => rune::text::measure(&format!("…/{last}"), &[], style, None).0.min(72.0),
        None => rune::text::measure(dir, &[], style, None).0.min(72.0),
    }
}

/// Room left of a tree row's name for the folder chevron, so names line up at `TEXT_X`.
const CHEV: f32 = 12.0;

/// A tree row in omarky's outline style: folders bold, notes regular, both 12 px and dim; the
/// open note in the text colour on a tenth of the accent.
fn tree_row(v: V, key: &str, name: &str, whole: &str, depth: usize, folder: bool, open: bool, on: bool) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let chev = el()
        .key("chev")
        .w(CHEV)
        .h(CHEV)
        .center()
        .opacity(if folder { 1.0 } else { 0.0 })
        .child(icon(&CHEVRON_RIGHT, 7.0).color(c.faint).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }).rotate_motion(rune::Spring::new(0.12, 0.0)));
    let dragged = v.s.dragging.as_ref().is_some_and(|d| d.active && key == row_key("file", &d.path));
    let target = folder && v.s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_ref().is_some_and(|t| key == row_key("dir", t)));
    let mut r = panel::shade(key, panel::BODY, depth, CHEV, &c, on).pr(if folder { BUTTON_END } else { panel::RIGHT - panel::INSET }).hover_group().role("treeitem").label(name.to_string()).child(chev);
    if target {
        r = r.fill(p.soft).border(1.0, p.acc.alpha(0.45));
    }
    if dragged {
        r = r.opacity(0.5);
    }
    if folder {
        r = r.expanded(open);
    }
    // A name that shows in full needs no tooltip; one cut short offers it whole, with its folder (C7).
    r.child(text(name).font(nav(panel::BODY, folder)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(name_tip(whole)))
}

/// The row where a new folder gets its name.
fn new_folder_row(v: V, depth: usize, edit: &rune::TextEdit) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    panel::shade("new-folder-row", panel::BODY, depth, CHEV, &c, false)
        .child(el().w(CHEV).h(CHEV))
        .child(
            el().key("new-folder-box").grow(1.0).min_w(0.0).px(4.0).py(1.0).radius(3.0).fill(c.hover).child(
                super::super::focus::stop(text_input(edit, nav(panel::BODY, true)))
                    .key("new-folder")
                    .single_line()
                    .w_full()
                    .color(p.ink)
                    .placeholder("Folder name", theme::hex("#9a9184"))
                    .caret(p.ink, p.sel)
                    .label("New folder name")
                    .on_edit(|s: &mut App| &mut s.new_folder.get_or_insert_with(|| crate::folders::NewFolder { parent: String::new(), edit: rune::TextEdit::new("") }).edit)
                    .on_submit(|s: &mut App, tx| s.end_new_folder(true, tx))
                    .on_input_key(|s: &mut App, ev: &rune::KeyEvent, tx| {
                        if ev.key == rune::Key::Escape {
                            if !s.esc_takes_notice() {
                                s.end_new_folder(false, tx);
                            }
                            return true;
                        }
                        false
                    }),
            ),
        )
}

/// The note under the pointer while it is dragged to a folder.
pub fn drag_ghost(v: V) -> El {
    let p = v.p;
    let Some(d) = v.s.dragging.as_ref().filter(|d| d.active) else { return el() };
    let hint = match &d.target {
        Some(t) if t.is_empty() => "to the folder root".to_string(),
        Some(t) => format!("to {t}/"),
        None => String::new(),
    };
    let mut r = floating(row(), &Roles::new(p))
        .key("drag-ghost")
        .absolute(d.x + 14.0, d.y + 10.0)
        .gap(8.0)
        .px(10.0)
        .h(28.0)
        .center_y()
        .radius(7.0)
        .paint_order(70)
        .inert()
        .child(label(md::base(&d.path), nav(panel::BODY, true), p.ink));
    if !hint.is_empty() {
        r = r.child(label(hint, nav(panel::BODY_SMALL, false), p.acc));
    }
    r
}
