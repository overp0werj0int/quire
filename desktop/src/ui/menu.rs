//! Quire's in-app menus (WR-193), one layer each (`crate::layer::Menu`): the folder's menu
//! under its name, the tree's, the editor's (WR-135, SHEET-032) and the page's block menu
//! (RD-68). Each is a list of items at the point it opened at, kept inside the window. Picking an
//! item closes the menu, which gives focus back, and then runs it.

use rune::{Color, El, col, el, row, text};

use super::V;
use super::layer::Card;
use super::panel;
use super::styles::{keys, literal, nav};
use super::widgets::{floating, label};
use crate::app::App;
use crate::folders::MenuOn;
use crate::keymap;
use crate::layer::{Menu, MenuOf};
use crate::md;
use crate::roles::Roles;

type Run = Box<dyn Fn(&mut App, &mut rune::Tx)>;

/// A row of a menu: its name, its shortcut, and what it does. `key` names the row where a test
/// or a check reaches it; the others are `tm-<index>`.
struct Item {
    key: Option<&'static str>,
    label: &'static str,
    keys: Option<String>,
    enabled: bool,
    run: Run,
}

fn item(label: &'static str, run: Run) -> Item {
    Item { key: None, label, keys: None, enabled: true, run }
}

/// The open menu's card.
pub(super) fn menu(v: V, m: &Menu) -> Card {
    let el = match &m.of {
        MenuOf::Folder => folder(v),
        MenuOf::Tree(on) => list(v, "tree-menu", m.at, 200.0, None, tree(on, m.at)),
        MenuOf::DeleteFolder(f) => confirm_delete_folder(v, f, m.at),
        MenuOf::Edit(_) => list(v, "edit-menu", m.at, 200.0, None, edit(v)),
        MenuOf::Block(line) => list(v, "block-menu", m.at, 200.0, None, block(v, *line)),
    };
    Card { el, plane: None }
}

/// A menu's panel at `at`, at least `min_w` wide, with `head` over its items.
fn list(v: V, key: &str, at: (f32, f32), min_w: f32, head: Option<El>, items: Vec<Item>) -> El {
    let p = v.p;
    // Wide enough for the longest row's name and key, at about 7.2 points a character.
    let need = |it: &Item| 30.0 + 7.2 * (it.label.len() + it.keys.as_ref().map_or(0, |k| k.len() + 2)) as f32;
    let w = items.iter().map(need).fold(min_w, f32::max).ceil();
    let h = 10.0 + 30.0 * items.len() as f32;
    let x = at.0.min(v.vw - w - 8.0).max(8.0);
    let y = at.1.min(v.vh - h - 8.0).max(8.0);
    let mut panel = floating(col(), &Roles::new(p)).key(key).absolute(x, y).w(w).padding(5.0).radius(9.0).role("menu").children(head);
    for (i, it) in items.into_iter().enumerate() {
        let ink = if it.enabled { p.ink } else { p.mute };
        let key = it.key.map_or_else(|| format!("tm-{i}"), str::to_string);
        let mut r = row().key(key).w_full().h(30.0).px(10.0).gap(14.0).center_y().radius(6.0).role("menuitem").label(it.label).child(label(it.label, nav(panel::BODY, false), ink).grow(1.0));
        if let Some(k) = it.keys {
            r = r.child(label(k, keys(panel::BODY_SMALL), p.mute));
        }
        if it.enabled {
            let run = it.run;
            r = r.hover(p.hover).on_press_tx(move |s: &mut App, tx| {
                s.close_layer(tx);
                run(s, tx);
            });
        } else {
            r = r.disabled(true);
        }
        panel = panel.child(r);
    }
    panel
}

/// The folder's menu, under its name below the window corner and the Open list: the folder's
/// path, then Switch folder and Settings.
fn folder(v: V) -> El {
    let p = v.p;
    let path = v.s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    let head = text(path).font(literal(panel::BODY_SMALL).lh(11.0 * 1.45)).color(p.mute).wrap().wrap_anywhere().pt(7.0).px(10.0).pb(8.0);
    let items = vec![
        Item { key: Some("menu-switch"), ..item("Switch folder…", Box::new(|s, tx| s.go_welcome(tx))) },
        Item { key: Some("menu-settings"), ..item("Settings", Box::new(|s, tx| s.open_settings(tx))) },
    ];
    list(v, "vault-menu", (10.0, super::sidebar::folder_menu_y(v)), 228.0, Some(head), items)
}

/// The tree's menu: new notes and folders, rename, move or delete for a note, and delete for a
/// folder.
fn tree(on: &MenuOn, at: (f32, f32)) -> Vec<Item> {
    match on.clone() {
        MenuOn::Folder(f) => {
            let (f2, f3) = (f.clone(), f.clone());
            vec![
                item("New note here", Box::new(move |s, tx| s.new_note_in(Some(&f), None, tx))),
                item("New folder here", Box::new(move |s, tx| s.start_new_folder(&f2, tx))),
                item("Delete folder", Box::new(move |s, tx| s.ask_delete_folder(&f3, at, tx))),
            ]
        }
        MenuOn::Note(n) => {
            let (n1, n2, n3) = (n.clone(), n.clone(), n.clone());
            let mut items = vec![
                item("New note here", Box::new(move |s, tx| s.new_note_in(Some(md::dir(&n1)), None, tx))),
                item("Rename", Box::new(move |s, tx| {
                    s.open_file(&n2, None, tx);
                    s.start_rename(tx);
                })),
            ];
            if !md::dir(&n).is_empty() {
                items.push(item("Move to folder root", Box::new(move |s, tx| s.move_note(&n3, "", tx))));
            }
            items.push(item("Delete note", Box::new(move |s, _| s.delete_path(&n))));
            items
        }
        MenuOn::Root => vec![
            item("New note", Box::new(|s, tx| s.new_note_in(Some(""), None, tx))),
            item("New folder", Box::new(|s, tx| s.start_new_folder("", tx))),
        ],
    }
}

/// The editor's menu (WR-135, SHEET-032): Cut and Copy, with nothing selected greyed out, and
/// Paste, each sent to the field as its key would be. Over the edit sheet as well.
fn edit(v: V) -> Vec<Item> {
    let selected = v.s.edit_menu_selected();
    let send = |c: char| -> Run {
        // As Rune's own field menu sends them.
        let mods = rune::Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..rune::Mods::default() };
        Box::new(move |_, tx| tx.send_key(rune::KeyEvent { key: rune::Key::Command(c), mods }))
    };
    vec![
        Item { keys: Some("Ctrl+X".into()), enabled: selected, ..item("Cut", send('x')) },
        Item { keys: Some("Ctrl+C".into()), enabled: selected, ..item("Copy", send('c')) },
        Item { keys: Some("Ctrl+V".into()), ..item("Paste", send('v')) },
    ]
}

/// The block menu (RD-68): what can be done to the block a right-click landed on. Each row
/// names its key from the active keymap where it has one.
fn block(v: V, line: usize) -> Vec<Item> {
    use crate::sheet::Scope;
    let p = v.s.preset();
    let keys = |id: &str| Some(keymap::label(p, id)).filter(|k| !k.is_empty());
    let with = |label: &'static str, keys: Option<String>, run: Run| Item { keys, ..item(label, run) };
    let mut items = vec![
        with("Copy Markdown", keys("reader.copy-block-under-pointer"), Box::new(move |s, tx| s.copy_block(line, tx))),
        with("Copy text", None, Box::new(move |s, tx| s.copy_block_text(line, tx))),
    ];
    let doc = v.s.doc();
    if let Some(crate::blocks::Kind::Heading { slug, .. }) = doc.blocks.iter().find(|b| b.lines.start == line).map(|b| &b.kind) {
        let slug = slug.clone();
        items.push(with("Copy link", None, Box::new(move |s, tx| s.copy_heading_link(&slug, tx))));
    }
    if v.s.active.as_deref().is_some_and(|a| !v.s.note_locked(a)) {
        items.push(with("Edit block", keys("reader.edit-block"), Box::new(move |s, tx| s.edit_block_at(line, Scope::Block, tx))));
        items.push(with("Edit section", keys("reader.edit-section"), Box::new(move |s, tx| s.edit_block_at(line, Scope::Section, tx))));
    }
    items
}

/// The tree menu's second step for a folder with notes: what moves to the trash, and two buttons.
fn confirm_delete_folder(v: V, folder: &str, at: (f32, f32)) -> El {
    let p = v.p;
    let n = v.s.notes_in(folder).len();
    let name = folder.rsplit('/').next().unwrap_or(folder);
    let msg = format!("Move {name} and its {n} {} to the trash?", if n == 1 { "note" } else { "notes" });
    let w = 260.0;
    let h = 104.0;
    let x = at.0.min(v.vw - w - 8.0).max(8.0);
    let y = at.1.min(v.vh - h - 8.0).max(8.0);
    let f = folder.to_string();
    let button = |key: &str, t: &str, color: Color| {
        row().key(key).h(30.0).px(12.0).center_y().radius(6.0).hover(p.hover).role("button").label(t).child(label(t, nav(panel::BODY, true), color))
    };
    floating(col(), &Roles::new(p))
        .key("tree-menu")
        .absolute(x, y)
        .w(w)
        .padding(12.0)
        .gap(10.0)
        .radius(9.0)
        .role("alertdialog")
        .label(msg.clone())
        .child(label(msg, nav(panel::BODY, false).lh(18.0), p.ink).wrap())
        .child(
            row()
                .gap(4.0)
                .child(el().grow(1.0))
                .child(button("tm-cancel", "Cancel", p.ink2).on_press_tx(|s: &mut App, tx| s.close_layer(tx)))
                .child(button("tm-delete", "Delete", p.err).on_press_tx(move |s: &mut App, tx| {
                    s.close_layer(tx);
                    s.delete_folder(&f);
                })),
        )
}
