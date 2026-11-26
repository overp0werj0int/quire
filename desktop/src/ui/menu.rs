//! Quire's in-app menus (WR-193), one layer each (`crate::layer::Menu`): the folder's menu
//! under its name, the tree's, the editor's (WR-135, SHEET-032) and the page's block menu
//! (RD-68). Each is a list of items at the point it opened at, kept inside the window. Picking an
//! item closes the menu, which gives focus back, and then runs it; an item that opens a field
//! gives it the keyboard (`App::pick`).
//!
//! A menu works from the keyboard as a native one does (Rune's `menu` focus group): Up and
//! Down move through its items and wrap, a letter jumps to the next item it starts, Enter or
//! Space picks, Esc closes. Opened from the keyboard (Enter on the folder's name, the Menu key
//! or Shift+F10 in the editor), its first item shows marked; the pointer and the
//! keys share one highlight.

use rune::{El, col, el, row, text};

use super::V;
use super::layer::Card;
use super::panel;
use super::styles::{keys, literal, nav};
use super::components::{label, popup_frame};
use super::tokens::{radius, size, space};
use crate::app::App;
use crate::folders::MenuOn;
use crate::keymap;
use crate::layer::{Menu, MenuOf};
use crate::md;

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
        MenuOf::Folder => folder(v, m.at),
        MenuOf::Tree(on) => list(v, "tree-menu", m.at, 200.0, None, 0.0, tree(v, on, m.at)),
        MenuOf::DeleteFolder(f) => confirm_delete_folder(v, f, m.at),
        MenuOf::Edit(_) => list(v, "edit-menu", m.at, 200.0, None, 0.0, edit(v)),
        MenuOf::Block(line) => list(v, "block-menu", m.at, 200.0, None, 0.0, block(v, *line)),
        MenuOf::Node(id) => list(v, "node-menu", m.at, 200.0, None, 0.0, node(v, *id)),
        MenuOf::RecentVault(path) => list(v, "recent-menu", m.at, 180.0, None, 0.0, recent_vault(path)),
        MenuOf::Tab(t) => list(v, "tab-menu", m.at, 180.0, None, 0.0, tab_menu(v, t)),
    };
    Card { el, plane: None }
}

/// Clamps a menu's top-left corner inside the window with an 8 pt edge margin.
pub(crate) fn menu_pos(vw: f32, vh: f32, at: (f32, f32), w: f32, h: f32) -> (f32, f32) {
    let x = at.0.min(vw - w - 8.0).max(8.0);
    let y = at.1.min(vh - h - 8.0).max(8.0);
    (x, y)
}

/// A menu's panel at `at`, at least `min_w` wide, with `head` (of height `head_h`) over its items.
fn list(v: V, key: &str, at: (f32, f32), min_w: f32, head: Option<El>, head_h: f32, items: Vec<Item>) -> El {
    let p = v.p;
    // Wide enough for the longest row's name and key, at about 7.2 points a character.
    let need = |it: &Item| 30.0 + 7.2 * (it.label.len() + it.keys.as_ref().map_or(0, |k| k.len() + 2)) as f32;
    let w = items.iter().map(need).fold(min_w, f32::max).ceil();
    let h = 10.0 + head_h + size::MENU_ROW * items.len() as f32;
    let (x, y) = menu_pos(v.vw, v.vh, at, w, h);
    let mut panel = popup_frame(col(), p).key(key).absolute(x, y).w(w).padding(5.0).radius(radius::FIELD).role("menu").focus_group().children(head);
    for (i, it) in items.into_iter().enumerate() {
        let ink = if it.enabled { p.ink } else { p.mute };
        let key = it.key.map_or_else(|| format!("tm-{i}"), str::to_string);
        let mut r = row().key(key).w_full().h(size::MENU_ROW).px(space::PAD_INPUT_X).gap(14.0).center_y().radius(radius::CONTROL).role("menuitem").label(it.label).child(label(it.label, nav(panel::BODY, false), ink).grow(1.0));
        if let Some(k) = it.keys {
            r = r.child(label(k, keys(panel::BODY_SMALL), p.mute));
        }
        if it.enabled {
            let run = std::rc::Rc::new(it.run);
            let on_enter = run.clone();
            r = r
                .focusable(true)
                .focus_highlight(p.hover)
                .on_press_tx(move |s: &mut App, tx| s.pick(&*run, tx))
                .on_key(move |s: &mut App, ev: &rune::KeyEvent, tx| {
                    if ev.key == rune::Key::Enter && ev.mods == rune::Mods::default() && tx.focus_visible() {
                        s.pick(&*on_enter, tx);
                        return true;
                    }
                    false
                });
        } else {
            r = r.disabled(true);
        }
        panel = panel.child(r);
    }
    panel
}

/// The vault label's menu, where it was opened: the folder's path, then Switch folder and New
/// folder, which names a folder at the root and unfolds a folded vault first (SB-110, SB-111).
fn folder(v: V, at: (f32, f32)) -> El {
    let p = v.p;
    let path = v.s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    let min_w: f32 = 228.0;
    let head_style = literal(panel::BODY_SMALL).lh(11.0 * 1.45);
    let head_text_h = if path.is_empty() { 0.0 } else { rune::text::measure(&path, &[], head_style, Some((min_w - 30.0).max(40.0))).1 };
    let head_h = (head_text_h + 15.0).ceil();
    let head = text(path).font(head_style).color(p.mute).wrap().wrap_anywhere().pt(7.0).px(10.0).pb(8.0);
    let items = vec![
        Item { key: Some("menu-switch"), ..item("Switch folder…", Box::new(|s, tx| s.go_welcome(tx))) },
        Item {
            key: Some("menu-new-folder"),
            ..item(
                "New folder",
                Box::new(|s, tx| {
                    s.persist.sidebar.unfold(crate::sidebar::Section::Vault);
                    s.start_new_folder("", tx);
                }),
            )
        },
    ];
    list(v, "vault-menu", at, min_w, Some(head), head_h, items)
}

/// A recent folder row's context menu on the welcome screen.
fn recent_vault(path: &std::path::Path) -> Vec<Item> {
    let p = path.to_path_buf();
    vec![Item { key: Some("menu-forget"), ..item("Remove from recent", Box::new(move |s, tx| s.forget_vault(&p, tx))) }]
}

/// Shared lock actions for a note path in the file tree or Open shelf tab menu.
fn lock_menu_items(v: V, path: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let lock_key = Some(v.s.keymap.label("note.lock")).filter(|k| !k.is_empty());
    if v.s.note_unlocked(path) {
        let (p1, p2, p3) = (path.to_string(), path.to_string(), path.to_string());
        items.push(Item {
            key: Some("menu-lock-now"),
            keys: lock_key,
            ..item("Lock now", Box::new(move |s, tx| s.relock_path(&p1, tx)))
        });
        items.push(Item {
            key: Some("menu-change-pass"),
            ..item("Change password…", Box::new(move |s, tx| {
                if s.active.as_deref() != Some(p2.as_str()) {
                    s.open_file(&p2, None, tx);
                }
                s.ask_change_password(tx);
            }))
        });
        items.push(Item {
            key: Some("menu-remove-pass"),
            ..item("Remove password", Box::new(move |s, tx| {
                if s.active.as_deref() != Some(p3.as_str()) {
                    s.open_file(&p3, None, tx);
                }
                s.ask_remove_password(tx);
            }))
        });
    } else if !v.s.note_locked(path) {
        let p1 = path.to_string();
        items.push(Item {
            key: Some("menu-lock"),
            keys: lock_key,
            ..item("Lock with password…", Box::new(move |s, tx| {
                if s.active.as_deref() != Some(p1.as_str()) {
                    s.open_file(&p1, None, tx);
                }
                s.ask_lock(tx);
            }))
        });
    }
    items
}

/// An Open shelf tab's context menu: Rename (folder mode), Copy path, Lock actions, Close, Close others.
fn tab_menu(v: V, t: &crate::app::Shelved) -> Vec<Item> {
    let mut items = Vec::new();
    if let crate::app::Shelved::Tab(p) = t {
        let (p1, p2) = (p.clone(), p.clone());
        items.push(Item {
            key: Some("menu-tab-rename"),
            ..item("Rename", Box::new(move |s, tx| s.rename_note_path(&p1, tx)))
        });
        items.push(Item {
            key: Some("menu-tab-move"),
            ..item("Move to folder…", Box::new(move |s, tx| s.open_move_picker(&p2, tx)))
        });
    }
    let (t1, t2, t3) = (t.clone(), t.clone(), t.clone());
    items.push(Item {
        key: Some("menu-tab-copy-path"),
        ..item("Copy path", Box::new(move |s, tx| s.copy_shelved_path(&t1, tx)))
    });
    match t {
        crate::app::Shelved::Tab(p) => items.extend(lock_menu_items(v, p)),
        crate::app::Shelved::File(f) => {
            if v.s.note_path().as_deref() == Some(f.as_path())
                && let Some(a) = v.s.active.as_deref()
            {
                items.extend(lock_menu_items(v, a));
            }
        }
    }
    let close_key = Some(v.s.keymap.label("app.close-tab")).filter(|k| !k.is_empty());
    items.push(Item {
        key: Some("menu-tab-close"),
        keys: close_key,
        ..item("Close", Box::new(move |s, tx| {
            s.close_shelved(&t2);
            s.keys_to_note(tx);
        }))
    });
    items.push(Item {
        key: Some("menu-tab-close-others"),
        enabled: v.s.shelf().len() > 1,
        ..item("Close others", Box::new(move |s, tx| s.close_other_shelved(&t3, tx)))
    });
    items.push(Item {
        key: Some("menu-tab-close-all"),
        enabled: !v.s.shelf().is_empty(),
        ..item("Close all tabs", Box::new(|s, tx| s.close_all_tabs(tx)))
    });
    items
}

/// The tree's menu: new notes and folders, rename, move, lock or delete for a note, and rename or
/// delete for a folder.
fn tree(v: V, on: &MenuOn, at: (f32, f32)) -> Vec<Item> {
    match on.clone() {
        MenuOn::Folder(f) => {
            let (f2, f3, f4) = (f.clone(), f.clone(), f.clone());
            vec![
                item("New note here", Box::new(move |s, tx| s.new_note_in(Some(&f), None, tx))),
                item("New folder here", Box::new(move |s, tx| s.start_new_folder(&f2, tx))),
                Item {
                    key: Some("menu-rename-folder"),
                    ..item("Rename folder", Box::new(move |s, tx| s.start_rename_folder(&f3, tx)))
                },
                item("Delete folder", Box::new(move |s, tx| s.ask_delete_folder(&f4, at, tx))),
            ]
        }
        MenuOn::Note(n) => {
            let (n1, n2, n3, n4) = (n.clone(), n.clone(), n.clone(), n.clone());
            let mut items = vec![
                item("New note here", Box::new(move |s, tx| s.new_note_in(Some(md::dir(&n1)), None, tx))),
                item("Rename", Box::new(move |s, tx| {
                    s.open_file(&n2, None, tx);
                    s.start_rename(tx);
                })),
                Item {
                    key: Some("menu-move-folder"),
                    ..item("Move to folder…", Box::new(move |s, tx| s.open_move_picker(&n4, tx)))
                },
            ];
            if !md::dir(&n).is_empty() {
                items.push(item("Move to folder root", Box::new(move |s, tx| s.move_note(&n3, "", tx))));
            }
            items.extend(lock_menu_items(v, &n));
            items.push(item("Delete note", Box::new(move |s, tx| {
                s.delete_path(&n);
                s.row_deleted(tx);
            })));
            items
        }
        MenuOn::Root => vec![
            item("New note", Box::new(|s, tx| s.new_note_in(Some(""), None, tx))),
            item("New folder", Box::new(|s, tx| s.start_new_folder("", tx))),
        ],
    }
}

/// The editor's menu (WR-135, SHEET-032): Cut, Copy, Paste, Undo, Redo, Select all, and Follow
/// link when over a link in Source. Cut/Copy/Paste stay first so Paste remains `tm-2`.
fn edit(v: V) -> Vec<Item> {
    let selected = v.s.edit_menu_selected();
    let send = |c: char, shift: bool| -> Run {
        // As Rune's own field menu sends them.
        let mods = rune::Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), shift, ..rune::Mods::default() };
        Box::new(move |_, tx| tx.send_key(rune::KeyEvent { key: rune::Key::Command(c), mods }))
    };
    let mut items = vec![
        Item { keys: Some(keymap::cmd_label('x')), enabled: selected, ..item("Cut", send('x', false)) },
        Item { keys: Some(keymap::cmd_label('c')), enabled: selected, ..item("Copy", send('c', false)) },
        Item { keys: Some(keymap::cmd_label('v')), ..item("Paste", send('v', false)) },
        Item { keys: Some(keymap::cmd_label('z')), ..item("Undo", send('z', false)) },
        Item { keys: Some(keymap::label_of("Ctrl+Shift+Z")), ..item("Redo", send('z', true)) },
        Item { keys: Some(keymap::cmd_label('a')), ..item("Select all", send('a', false)) },
    ];
    let has_link = v.s.sheet().is_none()
        && v.s.mode == crate::app::Mode::Source
        && v.s.active.as_deref().and_then(|a| v.s.edits.get(a)).is_some_and(|e| {
            crate::writer::link_at_caret(&e.text, e.focus.min(e.text.len())).is_some()
        });
    if has_link {
        let follow_key = Some(v.s.keymap.label("writer.follow-link")).filter(|k| !k.is_empty());
        items.push(Item {
            key: Some("menu-follow-link"),
            keys: follow_key,
            ..item("Follow link", Box::new(|s, tx| super::editor::follow_link_at_caret(s, tx)))
        });
    }
    items
}

/// The block menu (RD-68): what can be done to the block a right-click landed on. Each row
/// names its key from the active keymap where it has one.
fn block(v: V, line: usize) -> Vec<Item> {
    use crate::sheet::Scope;
    let keys = |id: &str| Some(v.s.keymap.label(id)).filter(|k| !k.is_empty());
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

/// A map node's menu (MAP-062): what the node can do, each item on the node it was opened on.
/// The footnotes group folds and copies; a footnote edits and copies.
fn node(v: V, id: crate::map::NodeId) -> Vec<Item> {
    use crate::map::NodeKind;
    use crate::map_app::Place;
    let t = v.s.map_tree();
    let Some(i) = t.find(id) else { return Vec::new() };
    let n = &t.nodes[i];
    let on = |key: &'static str, label: &'static str, keymap_id: &str, enabled: bool, run: fn(&mut App, usize, &mut rune::Tx)| Item {
        key: Some(key),
        keys: Some(v.s.keymap.label(keymap_id)).filter(|k| !k.is_empty()),
        enabled,
        ..item(label, Box::new(move |s, tx| if let Some(i) = s.map_pick(id, tx) { run(s, i, tx) }))
    };
    let folded = v.s.map_view().is_some_and(|m| m.folds.whole(id));
    let fold = (!n.children.is_empty()).then(|| on("menu-fold", if folded { "Unfold" } else { "Fold" }, "map.toggle-fold", true, |s, i, tx| s.map_toggle(i, None, false, tx)));
    let copy = on("menu-copy", "Copy Markdown", "map.copy-node", true, |s, _, tx| s.map_copy(false, tx));
    let is_media = matches!(&n.body, crate::map::Body::Card(bl) if matches!(bl.kind, crate::blocks::Kind::Image { .. } | crate::blocks::Kind::Gallery { .. }));
    let edit = if is_media {
        on("menu-edit", "Edit", "map.edit", true, |s, _, tx| s.map_edit_sheet(tx))
    } else {
        on("menu-edit", "Edit", "map.edit", true, |s, _, tx| s.map_edit(tx))
    };
    match n.kind {
        NodeKind::Footnotes => fold.into_iter().chain([copy]).collect(),
        NodeKind::Footnote { .. } => vec![edit, copy],
        _ => {
            let root = i == crate::map::ROOT;
            let virtual_root = n.kind == NodeKind::Root;
            let mut items = vec![on("menu-add-child", "Add child", "map.add-child", crate::map::takes_children(&t, i), |s, _, tx| s.map_add(Place::Child, tx))];
            items.extend((!root).then(|| on("menu-add-sibling", "Add sibling", "map.add-sibling", true, |s, _, tx| s.map_add(Place::After, tx))));
            items.push(edit);
            items.extend((!virtual_root).then(|| on("menu-edit-branch", "Edit branch in sheet", "map.edit-subtree", true, |s, _, tx| s.map_edit_branch(tx))));
            items.extend((root && t.props > 0).then(|| Item {
                key: Some("menu-properties"),
                keys: None,
                enabled: true,
                ..item("Edit properties", Box::new(|s, tx| s.map_props(tx)))
            }));
            items.extend(fold);
            if !root && !v.s.active_locked() && crate::map::can_convert(&t, i) {
                use crate::blocks::Task;
                use crate::map::{BlockKind, NewKind};
                if !matches!(n.kind, NodeKind::Block(BlockKind::Paragraph)) {
                    items.push(on("menu-convert-para", "Convert to paragraph", "map.paragraph", true, |s, _, tx| s.map_convert(NewKind::Paragraph, tx)));
                }
                if !matches!(n.kind, NodeKind::Heading { .. }) {
                    items.push(on("menu-convert-heading", "Convert to heading", "map.heading-2", true, |s, _, tx| s.map_convert(NewKind::Heading(2), tx)));
                }
                if !matches!(n.kind, NodeKind::Item { ordered: false, task, .. } if task != Task::None) {
                    items.push(on("menu-convert-task", "Convert to task", "map.task", true, |s, _, tx| s.map_convert(NewKind::Task, tx)));
                }
                if !matches!(n.kind, NodeKind::Item { ordered: false, task: Task::None, .. }) {
                    items.push(on("menu-convert-bullet", "Convert to bulleted list", "map.bulleted", true, |s, _, tx| s.map_convert(NewKind::Bullet, tx)));
                }
                if !matches!(n.kind, NodeKind::Item { ordered: true, .. }) {
                    items.push(on("menu-convert-number", "Convert to numbered list", "map.numbered", true, |s, _, tx| s.map_convert(NewKind::Number, tx)));
                }
            }
            items.extend((!root && !v.s.active_locked()).then(|| on("menu-cut", "Cut", "map.cut", true, |s, _, tx| s.map_cut(tx))));
            items.push(copy);
            items.push(on("menu-copy-branch", "Copy branch", "map.copy-subtree", true, |s, _, tx| s.map_copy(true, tx)));
            items.extend((!v.s.active_locked()).then(|| {
                on("menu-paste", "Paste", "map.paste", true, |s, _, tx| s.map_paste_menu(tx))
            }));
            items.extend((!root).then(|| on("menu-delete", "Delete", "map.delete", true, |s, _, tx| s.map_delete(tx))));
            items
        }
    }
}

/// The tree menu's second step for a folder with notes: what moves to the trash, and two
/// buttons. Tab moves between them, Enter or Space presses the focused one, Esc cancels.
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
    popup_frame(col(), p)
        .key("tree-menu")
        .absolute(x, y)
        .w(w)
        .padding(space::PX_12)
        .gap(space::PX_10)
        .radius(radius::FIELD)
        .role("alertdialog")
        .label(msg.clone())
        .child(label(msg, nav(panel::BODY, false).lh(18.0), p.ink).wrap())
        .child(
            row()
                .gap(space::GAP_CONTROL)
                .child(el().grow(1.0))
                .child(super::components::Button::outline(p, "tm-cancel", "Cancel").sm().on_press(|s: &mut App, tx| s.close_layer(tx)))
                .child(super::components::Button::destructive(p, "tm-delete", "Delete").sm().on_press(move |s: &mut App, tx| {
                    s.close_layer(tx);
                    s.delete_folder(&f);
                    s.row_deleted(tx);
                })),
        )
}
