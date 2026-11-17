//! The left sidebar: the open notes, then the folder menu, files, search and tags. In Note mode
//! only the open notes.

use std::collections::BTreeMap;

use rune::{Color, El, PointerButton, ScrollSpec, Span, col, el, icon, rich, row, text, text_input};

use super::V;
use super::panel;
use super::widgets::{CHEVRON_DOWN, CHEVRON_RIGHT, CLOSE, FOLDER_PLUS, PLUS, SEARCH, label};
use crate::folders::{MenuOn, row_key};
use crate::app::{App, Panel, Sel};
use crate::md;
use crate::roles::{Roles, mix};
use crate::theme::{self, mono, sans};

pub fn sidebar(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let vault = s.vault.as_ref();
    let name = vault.map_or(String::new(), |v| v.name.clone());

    let c = Roles::panel(p);
    let head = row().key("vault-head").pt(6.0).pb(4.0).w_full().child(
        panel::shade("vault-button", panel::BODY, 0, 0.0, &c, false)
            .grow(1.0)
            .min_w(0.0)
            .gap(6.0)
            .role("button")
            .label(name.clone())
            .on_press(|s: &mut App| s.vault_menu = !s.vault_menu)
            .child(text(name).font(panel::font(panel::BODY, true)).color(c.text).truncate().grow(1.0).shrink().min_w(0.0))
            .child(icon(&CHEVRON_DOWN, 9.0).color(c.faint)),
    );

    // The panel switch is a row of section captions; the open one is in the text colour.
    let mut tabs = row().key("panel-tabs").gap(14.0).pl(panel::TEXT_X).pr(panel::INSET).pt(panel::CAPTION_Y - 4.0).pb(panel::CAPTION_GAP - 4.0).center_y();
    for (id, lbl) in [(Panel::Files, "Files"), (Panel::Search, "Search"), (Panel::Tags, "Tags")] {
        let on = s.left_panel == id;
        tabs = tabs.child(
            row()
                .key(format!("panel-{}", lbl.to_lowercase()))
                .h(20.0)
                .center_y()
                .hover_group()
                .role("tab")
                .on_press_tx(move |s: &mut App, tx| {
                    s.left_panel = id;
                    if id == Panel::Search {
                        tx.focus("search", false);
                    }
                })
                .child(panel::caption(lbl, &c).color(if on { c.text } else { c.faint }).group_color(c.dim)),
        );
    }

    if s.left_panel == Panel::Files {
        tabs = tabs.child(el().grow(1.0)).child(
            row()
                .key("new-folder-button")
                .size(22.0, 22.0)
                .center()
                .radius(3.0)
                .hover(c.hover)
                .hover_group()
                .label("New folder")
                .help("New folder")
                .on_press_tx(|s: &mut App, tx| s.start_new_folder("", tx))
                .child(icon(&FOLDER_PLUS, 13.0).color(c.faint).group_color(c.text)),
        );
    }
    let body = match s.left_panel {
        Panel::Files => files_panel(v),
        Panel::Search => search_panel(v),
        Panel::Tags => tags_panel(v),
    };
    let list = col().key("side-scroll").grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(col().key("side-list").w_full().pb(12.0).child(body));

    let foot_btn = |key: &str, t: &str, kbd: String, run: fn(&mut App, &mut rune::Tx)| {
        panel::shade(key, panel::BODY, 0, 0.0, &c, false)
            .justify_between()
            .hover_group()
            .role("button")
            .on_press_tx(move |s: &mut App, tx| run(s, tx))
            .child(text(t).font(panel::font(panel::BODY, false)).color(c.dim).group_color(c.text))
            .child(text(kbd).font(panel::font(panel::CAPTION, false)).color(c.faint))
    };
    let note_mode = s.note_mode();
    let foot = col()
        .key("side-foot")
        .pt(6.0)
        .pb(8.0)
        .children((!note_mode).then(|| foot_btn("foot-new", "New note", crate::keymap::label(v.s.preset(), "app.new-note"), |s, tx| s.new_note(None, tx))))
        .child(foot_btn("foot-settings", "Settings", crate::keymap::label(v.s.preset(), "app.settings"), |s, _| s.open_settings()));

    // The sidebar runs the window's full height, its edge a line from top to bottom.
    let side = col()
        .key("sidebar")
        .w(248.0)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .border_layout()
        .child(super::titlebar::corner(v));
    // Note mode has no folder: no folder menu, files, search or tags.
    let side = if note_mode { side.child(open_notes(v)).child(el().grow(1.0)) } else { side.child(open_notes(v)).child(head).child(tabs).child(list) };
    let mut side = side.child(foot);
    if s.vault_menu {
        side = side.child(vault_menu(v));
    }
    side
}

/// The open notes, one row each in the order they were opened, under a heading set as the
/// folder's name is. The current note has the tree's accent wash; a row's close button shows on
/// hover and on the current row, and a middle click closes it too.
const OPEN_PAD: f32 = 6.0;
fn open_notes(v: V) -> El {
    let s = v.s;
    let c = Roles::panel(v.p);
    let heading = row()
        .key("open-head")
        .h(panel::row_h(panel::BODY))
        .mx(panel::INSET)
        .pl(panel::TEXT_X - panel::INSET)
        .center_y()
        .child(text("Open").font(panel::font(panel::BODY, true)).color(c.text));
    let mut list = col().key("open-notes").w_full().pt(OPEN_PAD).pb(OPEN_PAD).role("tablist").child(heading);
    if s.note_mode() {
        let shown = s.note_path();
        for file in &s.notes {
            list = list.child(note_row(v, file, shown.as_ref() == Some(file)));
        }
        return list;
    }
    for path in s.tabs.iter().filter(|t| s.files().contains_key(*t)) {
        list = list.child(open_row(v, path));
    }
    list
}

/// A row of Note mode's Open list: a note opened as a file, shown by its file name.
fn note_row(v: V, file: &std::path::Path, on: bool) -> El {
    let c = Roles::panel(v.p);
    let name = file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let (open_file, close_file) = (file.to_path_buf(), file.to_path_buf());
    let close = row()
        .key("x")
        .size(18.0, 18.0)
        .center()
        .radius(3.0)
        .hover(c.hover)
        .label("Close")
        .help("Close")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press(move |s: &mut App| s.close_note(&close_file))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    panel::shade(format!("note:{}", file.display()), panel::BODY, 0, 0.0, &c, on)
        .pr(4.0)
        .hover_group()
        .role("tab")
        .checked(on)
        .label(name.clone())
        .help(crate::store::tilde(file))
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_note(&open_file);
            } else {
                s.open_note(&open_file);
            }
        })
        .child(text(name).font(panel::font(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0))
        .child(close)
}

fn open_row(v: V, path: &str) -> El {
    let c = Roles::panel(v.p);
    let on = v.s.active.as_deref() == Some(path);
    let (open_path, close_path) = (path.to_string(), path.to_string());
    let close = row()
        .key("x")
        .size(18.0, 18.0)
        .center()
        .radius(3.0)
        .hover(c.hover)
        .label("Close")
        .help("Close")
        .group_opacity(if on { 1.0 } else { 0.0 }, 1.0)
        .on_press(move |s: &mut App| s.close_tab(&close_path))
        .child(icon(&CLOSE, 8.0).color(c.faint));
    panel::shade(format!("tab:{path}"), panel::BODY, 0, 0.0, &c, on)
        .pr(4.0)
        .hover_group()
        .role("tab")
        .checked(on)
        .label(md::base(path).to_string())
        .help(path.to_string())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_tab(&open_path);
            } else if s.active.as_deref() != Some(open_path.as_str()) {
                s.open_file(&open_path, None, tx);
            }
        })
        .child(text(md::base(path)).font(panel::font(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0))
        .child(close)
}

fn vault_menu(v: V) -> El {
    let p = v.p;
    let path = v.s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    let item = |key: &str, t: &str, run: fn(&mut App, &mut rune::Tx)| {
        row()
            .key(key)
            .w_full()
            .h(30.0)
            .px(10.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .role("menuitem")
            .on_press_tx(move |s: &mut App, tx| run(s, tx))
            .child(label(t, sans(13.0, 400.0), p.ink))
    };
    let [a, b] = theme::menu_shadows();
    // Under the folder's name, below the corner and the Open list.
    let open = v.s.tabs.iter().filter(|t| v.s.files().contains_key(*t)).count() as f32;
    let y = super::titlebar::BAR_H + OPEN_PAD * 2.0 + panel::row_h(panel::BODY) * (1.0 + open) + 40.0;
    let panel = col()
        .key("vault-menu")
        .absolute(10.0, y)
        .w(228.0)
        .padding(5.0)
        .radius(9.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .role("menu")
        .modal(true, |s: &mut App| s.vault_menu = false)
        .child(text(path).font(mono(11.0, 400.0).lh(11.0 * 1.45)).color(p.mute).wrap().wrap_anywhere().pt(7.0).px(10.0).pb(8.0))
        .child(item("menu-switch", "Switch folder…", |s, tx| s.go_welcome(tx)))
        .child(item("menu-settings", "Settings", |s, _| s.open_settings()));
    el().key("vault-menu-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(31).child(panel)
}

struct Node {
    folders: BTreeMap<String, Node>,
    files: Vec<String>,
    path: String,
}

impl Node {
    fn new(path: String) -> Node {
        Node { folders: BTreeMap::new(), files: Vec::new(), path }
    }
    fn add(&mut self, path: &str) -> &mut Node {
        let mut n = self;
        for part in path.split('/') {
            let child = if n.path.is_empty() { part.to_string() } else { format!("{}/{part}", n.path) };
            n = n.folders.entry(part.to_string()).or_insert_with(|| Node::new(child));
        }
        n
    }
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

fn files_panel(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let Some(vault) = &s.vault else { return col() };
    let mut root = Node::new(String::new());
    for f in &vault.folders {
        root.add(f);
    }
    for f in vault.files.keys() {
        let d = md::dir(f);
        let n = if d.is_empty() { &mut root } else { root.add(d) };
        n.files.push(f.clone());
    }
    let mut rows = Vec::new();
    walk(v, &root, 0, &mut rows);
    let root_target = s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some(""));
    let mut c = col()
        .key("tree")
        .w_full()
        .role("tree")
        .on_secondary_press(|s: &mut App, at, _| s.open_tree_menu(MenuOn::Root, at.x, at.y));
    if root_target {
        c = c.fill(p.soft.alpha(0.5)).radius(3.0);
    }
    if vault.files.is_empty() && s.new_folder.is_none() {
        c = c.child(text("No notes yet.").font(panel::font(panel::BODY_SMALL, false)).color(Roles::panel(p).faint).py(6.0).pl(panel::TEXT_X));
    }
    c.children(rows)
}

fn walk(v: V, n: &Node, depth: usize, rows: &mut Vec<El>) {
    let s = v.s;
    if let Some(nf) = &s.new_folder {
        if nf.parent == n.path {
            rows.push(new_folder_row(v, depth, &nf.edit));
        }
    }
    let mut names: Vec<&String> = n.folders.keys().collect();
    names.sort_by(|a, b| natural(a, b));
    for name in names {
        let f = &n.folders[name];
        let open = s.expanded.contains(&f.path);
        let path = f.path.clone();
        let menu_path = f.path.clone();
        let add_path = f.path.clone();
        let add = row()
            .key("add")
            .size(18.0, 18.0)
            .center()
            .radius(3.0)
            .hover(Roles::panel(v.p).hover)
            .label("New note in this folder")
            .help("New note here")
            .group_opacity(0.0, 1.0)
            .on_press_tx(move |s: &mut App, tx| s.new_note_in(Some(&add_path), None, tx))
            .child(icon(&PLUS, 9.0).color(Roles::panel(v.p).faint));
        rows.push(
            tree_row(v, &row_key("dir", &f.path), name, depth, true, open, false)
                .help(f.path.clone())
                .on_press(move |s: &mut App| s.toggle_folder(&path))
                .on_secondary_press(move |s: &mut App, at, _| s.open_tree_menu(MenuOn::Folder(menu_path.clone()), at.x, at.y))
                .child(add),
        );
        if open {
            walk(v, f, depth + 1, rows);
        }
    }
    let mut files: Vec<&String> = n.files.iter().collect();
    files.sort_by(|a, b| natural(md::base(a), md::base(b)));
    for f in files {
        let on = s.active.as_deref() == Some(f.as_str());
        let path = f.clone();
        let drag_path = f.clone();
        let menu_path = f.clone();
        rows.push(
            tree_row(v, &row_key("file", f), md::base(f), depth, false, false, on)
                .help(f.clone())
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, None, tx))
                .on_drag(move |s: &mut App, d: &mut rune::Drag, tx| s.drag_note(&drag_path, d, tx))
                .on_secondary_press(move |s: &mut App, at, _| s.open_tree_menu(MenuOn::Note(menu_path.clone()), at.x, at.y)),
        );
    }
}

/// Room left of a tree row's name for the folder chevron, so names line up at `TEXT_X`.
const CHEV: f32 = 12.0;

/// A tree row in omarky's outline style: folders bold, notes regular, both 12 px and dim; the
/// open note in the text colour on a tenth of the accent.
fn tree_row(v: V, key: &str, name: &str, depth: usize, folder: bool, open: bool, on: bool) -> El {
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
    let mut r = panel::shade(key, panel::BODY, depth, CHEV, &c, on).pr(if folder { 4.0 } else { panel::RIGHT - panel::INSET }).hover_group().role("treeitem").label(name.to_string()).child(chev);
    if target {
        r = r.fill(p.soft).border(1.0, p.acc.alpha(0.45));
    }
    if dragged {
        r = r.opacity(0.5);
    }
    if folder {
        r = r.expanded(open);
    }
    r.child(text(name).font(panel::font(panel::BODY, folder)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0))
}

/// The row where a new folder gets its name.
fn new_folder_row(v: V, depth: usize, edit: &rune::TextEdit) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    panel::shade("new-folder-row", panel::BODY, depth, CHEV, &c, false)
        .child(el().w(CHEV).h(CHEV))
        .child(
            el().key("new-folder-box").grow(1.0).min_w(0.0).px(4.0).py(1.0).radius(3.0).fill(c.hover).child(
                text_input(edit, panel::font(panel::BODY, true))
                    .key("new-folder")
                    .single_line()
                    .w_full()
                    .color(p.ink)
                    .placeholder("Folder name", theme::hex("#9a9184"))
                    .caret(p.ink, p.sel)
                    .label("New folder name")
                    .on_edit(|s: &mut App| &mut s.new_folder.get_or_insert_with(|| crate::folders::NewFolder { parent: String::new(), edit: rune::TextEdit::new("") }).edit)
                    .on_submit(|s: &mut App, _| s.commit_new_folder())
                    .on_input_key(|s: &mut App, ev: &rune::KeyEvent, _| {
                        if ev.key == rune::Key::Escape {
                            s.new_folder = None;
                            return true;
                        }
                        false
                    }),
            ),
        )
}

/// The tree's menu, at the pointer: new notes and folders, rename or delete for a note, and
/// delete for a folder.
pub fn tree_menu(v: V) -> El {
    let Some(m) = &v.s.tree_menu else { return el() };
    if let (MenuOn::Folder(f), true) = (&m.on, m.confirm) {
        return confirm_delete_folder(v, f, m.x, m.y);
    }
    let mut items: Vec<(&str, Run)> = Vec::new();
    match m.on.clone() {
        MenuOn::Folder(f) => {
            let (f2, f3) = (f.clone(), f.clone());
            let (mx, my) = (m.x, m.y);
            items.push(("New note here", Box::new(move |s, tx| s.new_note_in(Some(&f), None, tx))));
            items.push(("New folder here", Box::new(move |s, tx| s.start_new_folder(&f2, tx))));
            items.push(("Delete folder", Box::new(move |s, _| s.ask_delete_folder(&f3, mx, my))));
        }
        MenuOn::Note(n) => {
            let (n1, n2, n3) = (n.clone(), n.clone(), n.clone());
            items.push(("New note here", Box::new(move |s, tx| s.new_note_in(Some(md::dir(&n1)), None, tx))));
            items.push(("Rename", Box::new(move |s, tx| {
                s.tree_menu = None;
                s.open_file(&n2, None, tx);
                s.start_rename(tx);
            })));
            if !md::dir(&n).is_empty() {
                items.push(("Move to folder root", Box::new(move |s, tx| {
                    s.tree_menu = None;
                    s.move_note(&n3, "", tx);
                })));
            }
            items.push(("Delete note", Box::new(move |s, _| {
                s.tree_menu = None;
                s.delete_path(&n);
            })));
        }
        MenuOn::Root => {
            items.push(("New note", Box::new(|s, tx| s.new_note_in(Some(""), None, tx))));
            items.push(("New folder", Box::new(|s, tx| s.start_new_folder("", tx))));
        }
    }
    let items = items.into_iter().map(|(t, run)| Item { label: t, keys: None, enabled: true, run }).collect();
    menu(v, "tree-menu", (m.x, m.y), items, |s, _| s.tree_menu = None).paint_order(55)
}

type Run = Box<dyn Fn(&mut App, &mut rune::Tx)>;

/// A row of Quire's in-app menu (WR-193): its name, its shortcut, and what it does.
pub struct Item {
    pub label: &'static str,
    pub keys: Option<&'static str>,
    pub enabled: bool,
    pub run: Run,
}

/// Quire's in-app menu at `at`, kept inside the window: the tree's and the editor's. `close`
/// runs before a picked item's action, and on Esc or a click outside.
pub fn menu(v: V, key: &str, at: (f32, f32), items: Vec<Item>, close: fn(&mut App, &mut rune::Tx)) -> El {
    let p = v.p;
    let w = 200.0;
    let h = 10.0 + 30.0 * items.len() as f32;
    let x = at.0.min(v.vw - w - 8.0).max(8.0);
    let y = at.1.min(v.vh - h - 8.0).max(8.0);
    let [a, b] = theme::menu_shadows();
    let mut panel = col().key(key).absolute(x, y).w(w).padding(5.0).radius(9.0).fill(p.bg).box_shadow(a).box_shadow(b).role("menu").modal_tx(true, close);
    for (i, it) in items.into_iter().enumerate() {
        let ink = if it.enabled { p.ink } else { p.mute };
        let mut r = row().key(format!("tm-{i}")).w_full().h(30.0).px(10.0).center_y().radius(6.0).role("menuitem").label(it.label).child(label(it.label, sans(13.0, 400.0), ink).grow(1.0));
        if let Some(k) = it.keys {
            r = r.child(label(k, sans(12.0, 400.0), p.mute));
        }
        if it.enabled {
            let run = it.run;
            r = r.hover(p.hover).on_press_tx(move |s: &mut App, tx| {
                close(s, tx);
                run(s, tx);
            });
        } else {
            r = r.disabled(true);
        }
        panel = panel.child(r);
    }
    el().key(format!("{key}-layer")).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(panel)
}

/// The editor's menu (WR-135, SHEET-032): Cut and Copy, with nothing selected greyed out, and
/// Paste, each sent to the field as its key would be. Over the edit sheet as well.
pub fn edit_menu(v: V) -> El {
    let Some(m) = &v.s.edit_menu else { return el() };
    let selected = v.s.edit_menu_selected();
    let send = |c: char| -> Run {
        // As Rune's own field menu sends them.
        let mods = rune::Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..rune::Mods::default() };
        Box::new(move |_, tx| tx.send_key(rune::KeyEvent { key: rune::Key::Command(c), mods }))
    };
    let items = vec![
        Item { label: "Cut", keys: Some("Ctrl+X"), enabled: selected, run: send('x') },
        Item { label: "Copy", keys: Some("Ctrl+C"), enabled: selected, run: send('c') },
        Item { label: "Paste", keys: Some("Ctrl+V"), enabled: true, run: send('v') },
    ];
    menu(v, "edit-menu", (m.x, m.y), items, |s, tx| s.close_edit_menu(tx)).paint_order(80)
}

/// The tree menu's second step for a folder with notes: what moves to the trash, and two buttons.
fn confirm_delete_folder(v: V, folder: &str, mx: f32, my: f32) -> El {
    let p = v.p;
    let n = v.s.notes_in(folder).len();
    let name = folder.rsplit('/').next().unwrap_or(folder);
    let msg = format!("Move {name} and its {n} {} to the trash?", if n == 1 { "note" } else { "notes" });
    let w = 260.0;
    let h = 104.0;
    let x = mx.min(v.vw - w - 8.0).max(8.0);
    let y = my.min(v.vh - h - 8.0).max(8.0);
    let [a, b] = theme::menu_shadows();
    let f = folder.to_string();
    let button = |key: &str, t: &str, color: Color| {
        row().key(key).h(30.0).px(12.0).center_y().radius(6.0).hover(p.hover).role("button").label(t).child(label(t, sans(13.0, 500.0), color))
    };
    let panel = col()
        .key("tree-menu")
        .absolute(x, y)
        .w(w)
        .padding(12.0)
        .gap(10.0)
        .radius(9.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .role("alertdialog")
        .label(msg.clone())
        .modal(true, |s: &mut App| s.tree_menu = None)
        .child(label(msg, sans(13.0, 400.0).lh(19.0), p.ink).wrap())
        .child(
            row()
                .gap(4.0)
                .child(el().grow(1.0))
                .child(button("tm-cancel", "Cancel", p.ink2).on_press(|s: &mut App| s.tree_menu = None))
                .child(button("tm-delete", "Delete", p.err).on_press(move |s: &mut App| {
                    s.tree_menu = None;
                    s.delete_folder(&f);
                })),
        );
    el().key("tree-menu-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(55).child(panel)
}

/// The note under the pointer while it is dragged to a folder.
pub fn drag_ghost(v: V) -> El {
    let p = v.p;
    let Some(d) = v.s.dragging.as_ref().filter(|d| d.active) else { return el() };
    let [a, b] = theme::menu_shadows();
    let hint = match &d.target {
        Some(t) if t.is_empty() => "to the folder root".to_string(),
        Some(t) => format!("to {t}/"),
        None => String::new(),
    };
    let mut r = row()
        .key("drag-ghost")
        .absolute(d.x + 14.0, d.y + 10.0)
        .gap(8.0)
        .px(10.0)
        .h(28.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .paint_order(70)
        .inert()
        .child(label(md::base(&d.path), sans(13.0, 500.0), p.ink));
    if !hint.is_empty() {
        r = r.child(label(hint, mono(11.0, 400.0), p.acc));
    }
    r
}

fn search_panel(v: V) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let s = v.s;
    let q = s.query.text.trim().to_string();
    let mut box_ = row()
        .key("search-box")
        .h(panel::row_h(panel::BODY))
        .pl(10.0)
        .pr(6.0)
        .gap(8.0)
        .center_y()
        .border(1.0, c.hairline_soft)
        .border_layout()
        .radius(3.0)
        .fill(p.bg)
        .child(icon(&SEARCH, 11.0).color(c.faint))
        .child(
            text_input(&s.query, panel::font(panel::BODY, false))
                .key("search")
                .single_line()
                .px(2.0)
                .py(1.0)
                .grow(1.0)
                .min_w(0.0)
                .color(p.ink)
                .placeholder("Search notes", theme::hex("#9a9184"))
                .caret(p.ink, p.sel)
                .on_edit(|s: &mut App| &mut s.query),
        );
    if !s.query.text.is_empty() {
        box_ = box_.child(
            row()
                .key("search-clear")
                .size(18.0, 18.0)
                .center()
                .radius(3.0)
                .hover(c.hover)
                .label("Clear")
                .help("Clear")
                .on_press_tx(|s: &mut App, tx| {
                    s.query = rune::TextEdit::new("");
                    tx.focus("search", false);
                })
                .child(icon(&CLOSE, 8.0).color(p.mute)),
        );
    }
    let results = if q.is_empty() { Vec::new() } else { md::search(s.files(), &q) };
    let total: usize = results.iter().map(|r| r.hits.len()).sum();
    let summary = if q.is_empty() {
        "Searches every note in this folder".to_string()
    } else if results.is_empty() {
        "No matches".to_string()
    } else {
        format!("{total} {} in {} {}", if total == 1 { "match" } else { "matches" }, results.len(), if results.len() == 1 { "note" } else { "notes" })
    };
    let mut out = col()
        .key("search-panel")
        .w_full()
        .child(col().key("search-top").px(panel::INSET).child(box_).child(text(summary).font(panel::font(panel::BODY_SMALL, false)).color(c.faint).pt(10.0).pl(panel::TEXT_X - panel::INSET).pb(4.0)));
    let ql = q.len();
    for (ri, r) in results.iter().enumerate() {
        let lines: Vec<&str> = s.files().get(&r.path).map(|t| t.split('\n').collect()).unwrap_or_default();
        let first = r.hits.first().map(|h| Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true });
        let path = r.path.clone();
        let name_style = panel::font(panel::BODY, true);
        let small = panel::font(panel::CAPTION, false);
        let shift = super::baseline_shift(name_style, small);
        let mut block = col().key(format!("res-{ri}")).py(4.0).child(
            panel::shade("res-head", panel::BODY, 0, 0.0, &c, false)
                .gap(8.0)
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, first, tx))
                .child(text(md::base(&r.path)).font(name_style).color(c.dim).truncate().shrink())
                .child(text(md::dir(&r.path)).font(small).color(c.faint).truncate().grow(1.0).shrink().mt(shift))
                .child(text(if r.hits.is_empty() { String::new() } else { r.hits.len().to_string() }).font(small).color(c.faint).mt(shift)),
        );
        for (hi, h) in r.hits.iter().take(4).enumerate() {
            let ln = lines.get(h.line).copied().unwrap_or("");
            let a0 = floor_char(ln, h.col.saturating_sub(28));
            let b0 = (h.col + ql).min(ln.len());
            let c1 = floor_char(ln, (b0 + 90).min(ln.len()));
            let pre = format!("{}{}", if a0 > 0 { "…" } else { "" }, ln[a0..h.col].trim_start());
            // `<mark>` has 1 px of padding on each side: a mono space 1 px wide takes it.
            let text_ = format!("{pre} {} {}", &ln[h.col..b0], &ln[b0..c1]);
            let (m0, m1) = (pre.len(), pre.len() + 1 + (b0 - h.col) + 1);
            let pad = Span { mono: true, size: Some(1.0 / 0.6), bg: Some(p.soft), ..Span::default() };
            let spans = vec![
                Span { start: m0 as u32, end: (m0 + 1) as u32, ..pad },
                Span { bg: Some(p.soft), color: Some(p.ink), ..Span::new(m0 + 1, m1 - 1) },
                Span { start: (m1 - 1) as u32, end: m1 as u32, ..pad },
            ];
            let path = r.path.clone();
            let sel = Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true };
            block = block.child(
                el().key(format!("hit-{hi}")).mx(panel::INSET).pl(panel::TEXT_X - panel::INSET).pr(panel::RIGHT - panel::INSET).py(4.0).radius(3.0).hover(c.hover).role("button").on_press_tx(move |s: &mut App, tx| s.open_file(&path, Some(sel), tx)).child(
                    rich(text_, spans).font(panel::font(panel::BODY_SMALL, false).lh(16.0)).color(mix(c.dim, c.faint, 0.5)).wrap().wrap_anywhere().w_full(),
                ),
            );
        }
        if r.hits.len() > 4 {
            block = block.child(text(format!("+ {} more", r.hits.len() - 4)).font(panel::font(panel::CAPTION, false)).color(c.faint).py(2.0).pl(panel::TEXT_X));
        }
        out = out.child(block);
    }
    out
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn tags_panel(v: V) -> El {
    let c = Roles::panel(v.p);
    let tags = md::tags(v.s.files());
    let mut out = col().key("tags").w_full();
    if tags.is_empty() {
        return out.child(text("No tags yet. Write #something in a note.").font(panel::font(panel::BODY_SMALL, false)).color(c.faint).py(6.0).pl(panel::TEXT_X).pr(panel::RIGHT).wrap());
    }
    for (tag, n) in tags {
        let t = tag.clone();
        out = out.child(
            panel::shade(format!("tag-{tag}"), panel::BODY, 0, 0.0, &c, false)
                .justify_between()
                .role("button")
                .on_press(move |s: &mut App| s.search_tag(&t))
                .child(text(format!("#{tag}")).font(panel::font(panel::BODY, false)).color(c.accent))
                .child(text(n.to_string()).font(panel::font(panel::CAPTION, false)).color(c.faint)),
        );
    }
    out
}
