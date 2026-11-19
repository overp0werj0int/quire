//! The vault's tree of folders and notes, with the new folder row and
//! a note being dragged. The rows come from `App::vault_tree`.

use rune::{El, col, el, icon, row, text, text_input};

use super::super::V;
use super::super::lists::{self, Branch};
use super::super::panel;
use super::super::styles::{medium, nav};
use super::super::widgets::{FOLDER, FOLDER_OPEN, PLUS, floating, label, name_tip};
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
    let rows = guides(v, rows);
    let root_target = s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some(""));
    let mut c = lists::list(col().key("tree").w_full().gap(GAP), "tree")
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

/// The rows of `n`'s folders and notes, each with its depth, the open folders' rows under them.
fn walk(v: V, n: &Folder, depth: usize, rows: &mut Vec<(usize, El)>) {
    let s = v.s;
    if let Some(nf) = &s.new_folder {
        if nf.parent == n.path {
            rows.push((depth, new_folder_row(v, depth, &nf.edit)));
        }
    }
    let parent = (!n.path.is_empty()).then(|| row_key("dir", &n.path));
    for f in &n.folders {
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
        let r = lists::item(folder_row(v, f, depth, open), v.p, false, move |s: &mut App, _| s.toggle_folder(&path)).child(count);
        rows.push((
            depth,
            lists::tree_arrows(r, branch, parent.clone())
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Folder(menu_path.clone())), (at.x, at.y), tx))
                .child(add),
        ));
        if open {
            walk(v, f, depth + 1, rows);
        }
    }
    for f in &n.notes {
        let on = s.active.as_deref() == Some(f.as_str());
        let path = f.clone();
        let drag_path = f.clone();
        let menu_path = f.clone();
        let r = lists::opener(note_row(v, f, depth, on), v.p, on, move |s: &mut App, tx| s.open_file(&path, None, tx));
        rows.push((
            depth,
            lists::tree_arrows(r, Branch::Note, parent.clone())
                .on_drag(move |s: &mut App, d: &mut rune::Drag, tx| s.drag_note(&drag_path, d, tx))
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Note(menu_path.clone())), (at.x, at.y), tx)),
        ));
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

/// A row's height (SB-07), the room between rows, and its corners, as the shelf's tabs have.
const ROW_H: f32 = 28.0;
const GAP: f32 = 1.0;
const RADIUS: f32 = 6.0;
/// One level's indent: an icon and the gap after it (SB-50).
const STEP: f32 = 21.0;
/// A folder's icon, and the room from it to the name.
const ICON: f32 = 14.0;
const ICON_GAP: f32 = STEP - ICON;

/// The left edge of a row's first icon or name at `depth`: the rows' text edge at the root,
/// one step in for each level (SB-50).
fn lead(depth: usize) -> f32 {
    panel::TEXT_X + depth as f32 * STEP
}

/// A tree row keyed `key`, its content from `lead(depth)`: the hover fill under the pointer,
/// cut names offering their whole path, and the drag's marks.
fn shade(v: V, key: &str, depth: usize) -> El {
    let c = Roles::panel(v.p);
    row().key(key.to_string()).h(ROW_H).mx(panel::INSET).pl(lead(depth) - panel::INSET).pr(panel::RIGHT - panel::INSET).center_y().radius(RADIUS).hover(c.hover)
}

/// Each row's guide lines: one for each folder above it, at the centre of that folder's icon,
/// 1 pt wide over the row's height and the gap under it, so the line of an open folder runs
/// from under its own row down to its last row (SB-50, SB-129).
fn guides(v: V, rows: Vec<(usize, El)>) -> Vec<El> {
    let line = v.p.line;
    let depths: Vec<usize> = rows.iter().map(|(d, _)| *d).collect();
    rows.into_iter()
        .enumerate()
        .map(|(i, (depth, r))| {
            let next = depths.get(i + 1).copied().unwrap_or(0);
            r.children((0..depth).map(|k| {
                // The pixel left of the icon's centre, as the drawing puts it.
                let x = lead(k) + ICON / 2.0 - 1.0 - panel::INSET;
                el().key("guide").pin(Some(x), Some(0.0), None, Some(if next > k { -GAP } else { 0.0 })).w(1.0).fill(line).inert()
            }))
        })
        .collect()
}

/// A folder's row: its icon in the accent, open or closed, and its name in strong at weight
/// 500 (SB-49, SB-55).
fn folder_row(v: V, f: &Folder, depth: usize, open: bool) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let key = row_key("dir", &f.path);
    let target = v.s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some(f.path.as_str()));
    let mut r = shade(v, &key, depth).pr(BUTTON_END).gap(ICON_GAP).hover_group().role("treeitem").label(f.name().to_string()).expanded(open);
    if target {
        r = r.fill(p.soft).border(1.0, p.acc.alpha(0.45));
    }
    let (glyph, icon_key) = if open { (&FOLDER_OPEN, "folder-open") } else { (&FOLDER, "folder-shut") };
    r.child(icon(glyph, ICON).key(icon_key).color(p.acc)).child(name(f.name(), &f.path, medium(panel::BODY), c.text))
}

/// A note's row: its name, where a folder's icon starts at its level, in Text; the active
/// tab's note in strong.
fn note_row(v: V, path: &str, depth: usize, on: bool) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let key = row_key("file", path);
    let mut r = shade(v, &key, depth).fill(panel::shade_fill(&c, on)).role("treeitem").label(md::base(path).to_string());
    if v.s.dragging.as_ref().is_some_and(|d| d.active && d.path == path) {
        r = r.opacity(0.5);
    }
    r.child(name(md::base(path), path, nav(panel::BODY, false), if on { c.text } else { c.dim }))
}

/// A row's name, cut at its end. One that shows in full needs no tooltip; one cut short offers
/// it whole, with its folder (C7, SB-117).
fn name(name: &str, whole: &str, font: rune::TextStyle, color: rune::Color) -> El {
    text(name).font(font).color(color).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(name_tip(whole))
}

/// The room inside the new folder's field before its text.
const FIELD_PAD: f32 = 4.0;

/// The row where a new folder gets its name.
fn new_folder_row(v: V, depth: usize, edit: &rune::TextEdit) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    // The field's text starts where a folder's name starts, after a closed folder's icon.
    shade(v, "new-folder-row", depth)
        .gap(ICON_GAP - FIELD_PAD)
        .child(icon(&FOLDER, ICON).color(p.acc))
        .child(
            el().key("new-folder-box").grow(1.0).min_w(0.0).px(FIELD_PAD).py(1.0).radius(3.0).fill(c.hover).child(
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
