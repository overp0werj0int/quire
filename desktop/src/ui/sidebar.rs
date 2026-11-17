//! The left sidebar: the open notes, then the folder menu, files, search and tags. In Note mode
//! only the open notes.

use std::collections::BTreeMap;

use rune::{El, PointerButton, ScrollSpec, Span, col, el, icon, rich, row, text, text_input};

use super::V;
use super::panel;
use super::widgets::{CHEVRON_DOWN, CHEVRON_RIGHT, CLOSE, FOLDER_PLUS, PLUS, SEARCH, floating, label};
use crate::folders::{MenuOn, row_key};
use crate::layer::MenuOf;
use crate::app::{App, Panel, Sel};
use crate::md;
use crate::roles::{Roles, mix};
use super::styles::{keys, nav};
use crate::theme;

/// The size of a sidebar button and of a panel tab: the smallest pointer target. The drawn
/// icons stay small inside it.
const TARGET: f32 = 24.0;

/// A row's room after its button, so the button's icon sits 13 points from the row's end.
const BUTTON_END: f32 = 1.0;

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
            .on_press_tx(|s: &mut App, tx| s.open_menu(MenuOf::Folder, (0.0, 0.0), tx))
            .child(text(name).font(nav(panel::BODY, true)).color(c.text).truncate().grow(1.0).shrink().min_w(0.0))
            .child(icon(&CHEVRON_DOWN, 9.0).color(c.faint)),
    );

    // The panel switch is a row of tabs in the caption's type, so they read as tabs, not as
    // labels: the others in the rows' colour, brighter under the pointer, and the open one in
    // the text colour over a mark of the accent. The new folder button lines up with the rows'
    // buttons below it.
    let mut tabs = row().key("panel-tabs").gap(14.0).pl(panel::TEXT_X).pr(panel::INSET + BUTTON_END).pt(panel::CAPTION_Y - 5.0).pb(panel::CAPTION_GAP - 5.0).center_y();
    for (id, lbl) in [(Panel::Files, "Files"), (Panel::Search, "Search"), (Panel::Tags, "Tags")] {
        let on = s.left_panel == id;
        tabs = tabs.child(
            row()
                .key(format!("panel-{}", lbl.to_lowercase()))
                .h(TARGET)
                .center_y()
                .hover_group()
                .role("tab")
                .on_press_tx(move |s: &mut App, tx| {
                    s.left_panel = id;
                    if id == Panel::Search {
                        tx.focus("search", false);
                    }
                })
                .child(super::styles::caps(lbl, true).color(if on { c.text } else { c.dim }).group_color(c.text))
                .children(on.then(|| {
                    el().key("tab-mark").pin(Some(0.0), None, Some(0.0), Some(1.0)).h(2.0).radius(1.0).fill(c.accent).transition(super::swap())
                })),
        );
    }

    if s.left_panel == Panel::Files {
        tabs = tabs.child(el().grow(1.0)).child(
            row()
                .key("new-folder-button")
                .size(TARGET, TARGET)
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
            .child(text(t).font(nav(panel::BODY, false)).color(c.dim).group_color(c.text))
            .child(text(kbd).font(keys(panel::CAPTION)).color(c.faint))
    };
    let note_mode = s.note_mode();
    let foot = col()
        .key("side-foot")
        .pt(6.0)
        .pb(8.0)
        .children((!note_mode).then(|| foot_btn("foot-new", "New note", crate::keymap::label(v.s.preset(), "app.new-note"), |s, tx| s.new_note(None, tx))))
        .child(foot_btn("foot-settings", "Settings", crate::keymap::label(v.s.preset(), "app.settings"), |s, tx| s.open_settings(tx)));

    // The sidebar runs the window's full height, its edge a line from top to bottom.
    let side = col()
        .key("sidebar")
        .w(248.0)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .border_layout()
        .child(super::titlebar::corner_room());
    // Note mode has no folder: no folder menu, files, search or tags.
    let side = if note_mode { side.children(open_notes(v)).child(el().grow(1.0)) } else { side.children(open_notes(v)).child(head).child(tabs).child(list) };
    side.child(foot)
}

/// The open notes, one row each in the order they were opened, under a section label. The
/// current note has the tree's accent wash; a row's close button shows on hover and on the
/// current row, and a middle click closes it too. With no note open, label and list are gone.
const OPEN_PAD: f32 = 6.0;
fn open_notes(v: V) -> Option<El> {
    let s = v.s;
    if open_count(v) == 0 {
        return None;
    }
    let heading = row()
        .key("open-head")
        .h(panel::row_h(panel::BODY))
        .mx(panel::INSET)
        .pl(panel::TEXT_X - panel::INSET)
        .center_y()
        .child(super::styles::caption("Open", true, v.p));
    let mut list = col().key("open-notes").w_full().pt(OPEN_PAD).pb(OPEN_PAD).role("tablist").child(heading);
    if s.note_mode() {
        let shown = s.note_path();
        for file in &s.notes {
            list = list.child(note_row(v, file, shown.as_ref() == Some(file)));
        }
        return Some(list);
    }
    for path in s.tabs.iter().filter(|t| s.files().contains_key(*t)) {
        list = list.child(open_row(v, path));
    }
    Some(list)
}

/// How many rows the Open list has.
fn open_count(v: V) -> usize {
    if v.s.note_mode() { v.s.notes.len() } else { v.s.tabs.iter().filter(|t| v.s.files().contains_key(*t)).count() }
}

/// The Open list's height: its label, its rows and its padding, or nothing when it is gone.
/// Where the folder's menu opens: under the folder's name, below the window corner and the
/// Open list.
pub(super) fn folder_menu_y(v: V) -> f32 {
    super::titlebar::BAR_H + open_h(v) + 40.0
}

fn open_h(v: V) -> f32 {
    match open_count(v) {
        0 => 0.0,
        n => OPEN_PAD * 2.0 + panel::row_h(panel::BODY) * (1 + n) as f32,
    }
}

/// A row of Note mode's Open list: a note opened as a file, shown by its file name.
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
    panel::shade(format!("note:{}", file.display()), panel::BODY, 0, 0.0, &c, on)
        .pr(BUTTON_END)
        .hover_group()
        .role("tab")
        .checked(on)
        .label(name.clone())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_note(&open_file);
            } else {
                s.open_note(&open_file);
            }
        })
        .child(text(name).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(crate::store::tilde(file)))
        .child(close)
}

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
    panel::shade(format!("tab:{path}"), panel::BODY, 0, 0.0, &c, on)
        .pr(BUTTON_END)
        .hover_group()
        .role("tab")
        .checked(on)
        .label(md::base(path).to_string())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_tab(&open_path);
            } else if s.active.as_deref() != Some(open_path.as_str()) {
                s.open_file(&open_path, None, tx);
            }
        })
        .child(text(md::base(path)).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(path))
        .child(close)
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
        .on_secondary_press(|s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Root), (at.x, at.y), tx));
    if root_target {
        c = c.fill(p.soft.alpha(0.5)).radius(3.0);
    }
    if vault.files.is_empty() && s.new_folder.is_none() {
        c = c.child(text("No notes yet.").font(nav(panel::BODY_SMALL, false)).color(Roles::panel(p).faint).py(6.0).pl(panel::TEXT_X));
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
            .size(TARGET, TARGET)
            .center()
            .radius(3.0)
            .hover(Roles::panel(v.p).hover)
            .label("New note in this folder")
            .help("New note in this folder")
            .group_opacity(0.0, 1.0)
            .on_press_tx(move |s: &mut App, tx| s.new_note_in(Some(&add_path), None, tx))
            .child(icon(&PLUS, 9.0).color(Roles::panel(v.p).faint));
        rows.push(
            tree_row(v, &row_key("dir", &f.path), name, &f.path, depth, true, open, false)
                .on_press(move |s: &mut App| s.toggle_folder(&path))
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Folder(menu_path.clone())), (at.x, at.y), tx))
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
            tree_row(v, &row_key("file", f), md::base(f), f, depth, false, false, on)
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, None, tx))
                .on_drag(move |s: &mut App, d: &mut rune::Drag, tx| s.drag_note(&drag_path, d, tx))
                .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(MenuOf::Tree(MenuOn::Note(menu_path.clone())), (at.x, at.y), tx)),
        );
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
    // A name that shows in full needs no tooltip; one cut short offers its whole path (C7).
    r.child(text(name).font(nav(panel::BODY, folder)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(whole))
}

/// The row where a new folder gets its name.
fn new_folder_row(v: V, depth: usize, edit: &rune::TextEdit) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    panel::shade("new-folder-row", panel::BODY, depth, CHEV, &c, false)
        .child(el().w(CHEV).h(CHEV))
        .child(
            el().key("new-folder-box").grow(1.0).min_w(0.0).px(4.0).py(1.0).radius(3.0).fill(c.hover).child(
                text_input(edit, nav(panel::BODY, true))
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
            text_input(&s.query, nav(panel::BODY, false))
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
                .label("Clear search")
                .help("Clear search")
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
        .child(col().key("search-top").px(panel::INSET).child(box_).child(text(summary).font(nav(panel::BODY_SMALL, false)).color(c.faint).wrap().w_full().pt(10.0).pl(panel::TEXT_X - panel::INSET).pr(panel::TEXT_X - panel::INSET).pb(4.0)));
    let ql = q.len();
    for (ri, r) in results.iter().enumerate() {
        let lines: Vec<&str> = s.files().get(&r.path).map(|t| t.split('\n').collect()).unwrap_or_default();
        let first = r.hits.first().map(|h| Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true });
        let path = r.path.clone();
        let name_style = nav(panel::BODY, true);
        let small = nav(panel::CAPTION, false);
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
            // The line as text, cut at words to two of the panel's lines (C17).
            let (snip, m) = md::snippet(ln, h.col, ql, SNIP_BEFORE, SNIP_COLS, 2);
            let (text_, spans) = if m.is_empty() {
                (snip, Vec::new())
            } else {
                // `<mark>` has 1 px of padding on each side: a mono space 1 px wide takes it.
                let text_ = format!("{} {} {}", &snip[..m.start], &snip[m.clone()], &snip[m.end..]);
                let (m0, m1) = (m.start, m.end + 2);
                let pad = Span { mono: true, size: Some(1.0 / 0.6), bg: Some(p.soft), ..Span::default() };
                let spans = vec![
                    Span { start: m0 as u32, end: (m0 + 1) as u32, ..pad },
                    Span { bg: Some(p.soft), color: Some(p.ink), ..Span::new(m0 + 1, m1 - 1) },
                    Span { start: (m1 - 1) as u32, end: m1 as u32, ..pad },
                ];
                (text_, spans)
            };
            let path = r.path.clone();
            let sel = Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true };
            block = block.child(
                el().key(format!("hit-{hi}")).mx(panel::INSET).pl(panel::TEXT_X - panel::INSET).pr(panel::RIGHT - panel::INSET).py(4.0).radius(3.0).hover(c.hover).role("button").on_press_tx(move |s: &mut App, tx| s.open_file(&path, Some(sel), tx)).child(
                    rich(text_, spans).font(nav(panel::BODY_SMALL, false).lh(16.0)).color(mix(c.dim, c.faint, 0.5)).max_lines(2).w_full(),
                ),
            );
        }
        if r.hits.len() > 4 {
            block = block.child(text(format!("+ {} more", r.hits.len() - 4)).font(nav(panel::CAPTION, false)).color(c.faint).py(2.0).pl(panel::TEXT_X));
        }
        out = out.child(block);
    }
    out
}

/// A search snippet's room: at most this many characters ahead of the match, and the characters
/// a line of the panel holds, its family being monospaced at 0.6 em.
const SNIP_BEFORE: usize = 16;
const SNIP_COLS: usize = ((248.0 - panel::TEXT_X - panel::RIGHT) / (panel::BODY_SMALL * 0.6)) as usize - 1;

fn tags_panel(v: V) -> El {
    let c = Roles::panel(v.p);
    let tags = md::tags(v.s.files());
    let mut out = col().key("tags").w_full();
    if tags.is_empty() {
        return out.child(text("No tags yet. Write #something in a note.").font(nav(panel::BODY_SMALL, false)).color(c.faint).py(6.0).pl(panel::TEXT_X).pr(panel::RIGHT).wrap());
    }
    for (tag, n) in tags {
        let t = tag.clone();
        out = out.child(
            panel::shade(format!("tag-{tag}"), panel::BODY, 0, 0.0, &c, false)
                .justify_between()
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.search_tag(&t, tx))
                .child(text(format!("#{tag}")).font(nav(panel::BODY, false)).color(c.accent))
                .child(text(n.to_string()).font(nav(panel::CAPTION, false)).color(c.faint)),
        );
    }
    out
}
