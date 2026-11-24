//! Folders: new notes inside a folder, new folders, moving notes by drag, and the tree's menu.

use rune::{Drag, Phase, TextEdit, Tx};

use crate::app::{App, Sel};
use crate::md;
use crate::vault::Op;

/// A note being dragged in the file tree.
#[derive(Clone, Debug)]
pub struct DragNote {
    pub path: String,
    /// The pointer, in window points.
    pub x: f32,
    pub y: f32,
    /// Past the threshold: a drag, not a click.
    pub active: bool,
    /// The folder under the pointer (`""` is the root), if the note can go there.
    pub target: Option<String>,
}

/// How long a delete's toast offers Undo, in seconds.
pub const UNDO_FOR: f64 = 6.0;

/// What a delete took from the open vault, so Undo can put it back: the notes' texts, the
/// folders, the open tabs and which one was shown. The files themselves wait in `.trash`.
pub struct Deleted {
    /// The note or folder's name, for the toast.
    pub name: String,
    /// When it went.
    pub at: f64,
    rel: String,
    folder: bool,
    notes: Vec<(String, String, Option<u128>)>,
    folders: Vec<String>,
    tabs: Vec<(usize, String)>,
    active: Option<String>,
    expanded: Vec<String>,
}

/// What the tree's menu was opened on.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuOn {
    Folder(String),
    Note(String),
    Root,
}


/// A folder being named in the tree.
pub struct NewFolder {
    pub parent: String,
    pub edit: TextEdit,
}

/// A folder being renamed inline in the tree.
pub struct RenameFolder {
    pub path: String,
    pub edit: TextEdit,
}

/// A folder of the vault's tree (`""` is the root), its rows in the tree's order: its
/// subfolders, then its notes, each by name ignoring case (SB-54).
#[derive(Debug, Default, PartialEq)]
pub struct Folder {
    pub path: String,
    pub folders: Vec<Folder>,
    /// Its notes, by path.
    pub notes: Vec<String>,
    /// The notes in it and in its subfolders (SB-52).
    pub count: usize,
}

impl Folder {
    /// The tree of `notes`, by path, and `folders`, which names the empty ones too.
    pub fn tree<'a>(notes: impl IntoIterator<Item = &'a String>, folders: &[String]) -> Folder {
        #[derive(Default)]
        struct Node {
            folders: std::collections::BTreeMap<String, Node>,
            notes: Vec<String>,
        }
        fn dig<'n>(mut n: &'n mut Node, path: &str) -> &'n mut Node {
            for part in path.split('/') {
                n = n.folders.entry(part.to_string()).or_default();
            }
            n
        }
        fn finish(n: Node, path: String) -> Folder {
            let mut folders: Vec<Folder> = n.folders.into_iter().map(|(name, child)| finish(child, md::join(&path, &name))).collect();
            folders.sort_by(|a, b| natural(a.name(), b.name()));
            let mut notes = n.notes;
            notes.sort_by(|a, b| natural(md::base(a), md::base(b)));
            let count = notes.len() + folders.iter().map(|f| f.count).sum::<usize>();
            Folder { path, folders, notes, count }
        }
        let mut root = Node::default();
        for f in folders.iter().filter(|f| !f.is_empty()) {
            dig(&mut root, f);
        }
        for p in notes {
            let d = md::dir(p);
            let n = if d.is_empty() { &mut root } else { dig(&mut root, d) };
            n.notes.push(p.clone());
        }
        finish(root, String::new())
    }

    /// The folder's own name, the last part of its path.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or_default()
    }
}

/// By name ignoring case, then as written, so two names that differ only in case keep one order.
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// The vault's tree, kept until a note is added or taken out or the folders change, so the
/// view does not sort every note and count the folders each time it is built.
#[derive(Default)]
pub struct Tree {
    at: Option<(u64, u64, Vec<String>)>,
    root: std::rc::Rc<Folder>,
}

impl Tree {
    pub fn of(&mut self, v: &crate::vault::Vault) -> std::rc::Rc<Folder> {
        let fresh = self.at.as_ref().is_some_and(|(id, paths, folders)| *id == v.id() && *paths == v.paths() && *folders == v.folders);
        if !fresh {
            self.root = std::rc::Rc::new(Folder::tree(v.files().keys(), &v.folders));
            self.at = Some((v.id(), v.paths(), v.folders.clone()));
        }
        self.root.clone()
    }
}

/// A tree row key holds its path with `|` for `/`, since `/` separates keys in element paths.
pub fn row_key(kind: &str, path: &str) -> String {
    format!("{kind}:{}", path.replace('/', "|"))
}

/// The folder an element path points into: a folder row is that folder, a note row is its
/// note's folder, and the tree's empty space or the vault's label is the root. The shelf, the
/// top bar, the footer and the results take no drop (SB-116).
pub fn folder_at(path: &str) -> Option<String> {
    for seg in path.split('/').rev() {
        if let Some(p) = seg.strip_prefix("dir:") {
            return Some(p.replace('|', "/"));
        }
        if let Some(p) = seg.strip_prefix("file:") {
            return Some(md::dir(&p.replace('|', "/")).to_string());
        }
    }
    path.split('/').any(|s| s == "vault").then(String::new)
}

/// Splits `Drafts/Ideas/Name` into a clean folder path and a clean note name.
fn split_path(name: &str) -> (String, String) {
    let parts: Vec<String> = name.split('/').map(md::clean_name).filter(|p| !p.is_empty() && p != "." && p != "..").collect();
    match parts.split_last() {
        Some((last, dirs)) => (dirs.join("/"), last.clone()),
        None => (String::new(), String::new()),
    }
}

impl App {
    /// A new note in `folder` (`None`: the active tab's folder, or the root with no tab: SB-12).
    /// A `name` with slashes, such as `Drafts/Idea`, makes the note in that folder from the
    /// root, creating any folders it names.
    pub fn new_note_in(&mut self, folder: Option<&str>, name: Option<&str>, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        if self.vault.is_none() {
            return;
        }
        let mut base_folder = match folder {
            Some(f) => f.to_string(),
            None => self.active.as_deref().filter(|a| self.tabs.iter().any(|t| t == a)).map_or_else(String::new, |a| md::dir(a).to_string()),
        };
        let (sub, bn) = match name {
            Some(n) if n.contains('/') => {
                let (d, b) = split_path(n);
                // A path from the root wins over the open note's folder.
                base_folder = String::new();
                (d, b)
            }
            Some(n) => (String::new(), md::clean_name(n)),
            None => (String::new(), String::new()),
        };
        let folder = if sub.is_empty() { base_folder } else { md::join(&base_folder, &sub) };
        let bn = if bn.is_empty() { "Untitled".to_string() } else { bn };
        let p = self.unique_path(&folder, &bn);
        let content = if name.is_some() { format!("# {bn}\n\n") } else { String::new() };
        if let Some(v) = &mut self.vault {
            v.put(p.clone(), content.clone());
            v.born.insert(p.clone(), crate::app::now_ns());
            let mut d = folder.clone();
            while !d.is_empty() {
                if !v.folders.contains(&d) {
                    v.folders.push(d.clone());
                }
                d = md::dir(&d).to_string();
            }
            v.folders.sort();
        }
        self.rebuild_names_pub();
        self.mark_dirty_pub(&p);
        self.screen = crate::app::Screen::Vault;
        let end = content.len();
        self.open_file(&p, Some(Sel { start: end, end, line: None, force_source: false }), tx);
        if name.is_none() {
            self.mode = crate::app::Mode::Source;
            self.focus = false;
            self.start_rename(tx);
        }
    }

    /// Starts naming a new folder inside `parent` (`""` is the root).
    pub fn start_new_folder(&mut self, parent: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        let mut edit = TextEdit::new("");
        edit.focused = true;
        if !parent.is_empty() {
            self.expanded.insert(parent.to_string());
        }
        self.show_left(tx);
        // The field shows in the tree: search's results give way to it.
        self.query = TextEdit::new("");
        self.new_folder = Some(NewFolder { parent: parent.to_string(), edit });
        self.field_opened(crate::layer::Field::FolderName);
        tx.focus("new-folder", false);
    }

    /// The new folder's name field closes: Enter makes the folder (`keep`), Esc drops it. In
    /// the sidebar's drawer the note cannot take the keyboard, so the tree does, with its
    /// ring: the new folder's row; when no folder was made, the row the field was opened from,
    /// the folder of a New folder here, else the tree's current row.
    pub fn end_new_folder(&mut self, keep: bool, tx: &mut Tx) {
        let parent = self.new_folder.as_ref().map(|nf| nf.parent.clone()).unwrap_or_default();
        let made = if keep { self.commit_new_folder() } else { None };
        self.new_folder = None;
        if self.drawer() == Some(crate::app::Drawer::Sidebar) {
            let row = match made {
                Some(path) => row_key("dir", &path),
                None if !parent.is_empty() => row_key("dir", &parent),
                None => self.tree_stop().unwrap_or_else(|| self.sidebar_stop()),
            };
            tx.focus(row, true);
        }
    }

    /// Makes the folder the name field asks for, and returns its path when it was made.
    fn commit_new_folder(&mut self) -> Option<String> {
        let nf = self.new_folder.take()?;
        let name = md::clean_name(&nf.edit.text);
        if name.is_empty() || name.starts_with('.') {
            return None;
        }
        let path = md::join(&nf.parent, &name);
        let v = self.vault.as_mut()?;
        if v.folders.contains(&path) {
            self.notice = Some(format!("A folder named {name} is already there."));
            return None;
        }
        v.folders.push(path.clone());
        v.folders.sort();
        let root = v.path.clone();
        self.writer.send(Op::Mkdir { root, rel: path.clone() });
        self.expanded.insert(path.clone());
        if !nf.parent.is_empty() {
            self.expanded.insert(nf.parent);
        }
        self.persist_later();
        self.sync_after_save();
        Some(path)
    }

    /// Starts renaming folder `path` inline in the sidebar tree (`F2` or context menu).
    pub fn start_rename_folder(&mut self, path: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() || path.is_empty() {
            return;
        }
        if !self.vault.as_ref().is_some_and(|v| v.folders.iter().any(|f| f == path)) {
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let mut edit = TextEdit::new(name);
        edit.set_selection(0, name.len());
        edit.focused = true;
        self.show_left(tx);
        self.query = TextEdit::new("");
        self.new_folder = None;
        self.renaming_folder = Some(RenameFolder { path: path.to_string(), edit });
        self.field_opened(crate::layer::Field::FolderName);
        tx.focus("rename-folder", false);
    }

    /// Finishes inline folder rename: Enter commits (`keep`), Esc cancels.
    pub fn end_rename_folder(&mut self, keep: bool, tx: &mut Tx) {
        let old = self.renaming_folder.as_ref().map(|rf| rf.path.clone()).unwrap_or_default();
        let renamed = if keep { self.commit_rename_folder() } else { None };
        self.renaming_folder = None;
        let target = renamed.as_deref().unwrap_or(&old);
        if self.drawer() == Some(crate::app::Drawer::Sidebar) {
            let row = if !target.is_empty() { row_key("dir", target) } else { self.tree_stop().unwrap_or_else(|| self.sidebar_stop()) };
            tx.focus(row, true);
        } else {
            self.keys_to_note(tx);
        }
    }

    /// Renames the folder on disk and updates all notes, folders, tabs, maps, expanded state, and
    /// folder-qualified wikilinks inside it.
    pub(crate) fn commit_rename_folder(&mut self) -> Option<String> {
        let rf = self.renaming_folder.take()?;
        let old = rf.path;
        let name = md::clean_name(&rf.edit.text);
        if name.is_empty() || name.starts_with('.') {
            return None;
        }
        let parent = md::dir(&old);
        let new_path = md::join(parent, &name);
        if new_path == old {
            return Some(old);
        }
        let collision = self.vault.as_ref().is_some_and(|v| {
            v.folders.iter().any(|f| f == &new_path) || (!old.eq_ignore_ascii_case(&new_path) && v.path.join(&new_path).exists())
        });
        if collision {
            self.notice = Some(format!("A folder named {name} already exists."));
            self.notice_open = None;
            return None;
        }
        self.flush_notes();
        let update = self.settings().update_links;
        let pre = format!("{old}/");
        let remap = |p: &str| -> Option<String> {
            if p == old {
                Some(new_path.clone())
            } else {
                p.strip_prefix(&pre).map(|rest| format!("{new_path}/{rest}"))
            }
        };
        let Some(v) = &mut self.vault else { return None };
        let root = v.path.clone();
        self.writer.send(Op::Rename { root, from: old.clone(), to: new_path.clone() });
        for f in &mut v.folders {
            if let Some(nf) = remap(f) {
                *f = nf;
            }
        }
        v.folders.sort();
        v.folders.dedup();
        let moved: Vec<(String, String)> = v.files().keys().filter_map(|p| remap(p).map(|np| (p.clone(), np))).collect();
        for (from, to) in &moved {
            let text = v.take(from).unwrap_or_default();
            v.put(to.clone(), text);
            if let Some(b) = v.born.remove(from) {
                v.born.insert(to.clone(), b);
            }
        }
        let mut rewritten = Vec::new();
        if update && !moved.is_empty() {
            let pairs: Vec<(String, String)> = moved.iter().map(|(f, t)| (md::strip_md(f).to_string(), md::strip_md(t).to_string())).collect();
            let changed: Vec<(String, String)> = v
                .files()
                .iter()
                .filter_map(|(p, t)| {
                    let mut cur = t.clone();
                    for (of, nf) in &pairs {
                        cur = md::rewrite_links(&cur, of, nf);
                    }
                    (cur != *t).then(|| (p.clone(), cur))
                })
                .collect();
            for (p, nt) in changed {
                rewritten.push(p.clone());
                v.put(p, nt);
            }
        }
        for (from, to) in &moved {
            if let Some(u) = self.unlocked.remove(from) {
                self.unlocked.insert(to.clone(), u);
            }
            if let Some(e) = self.edits.remove(from) {
                self.edits.insert(to.clone(), e);
            }
            if self.dirty.remove(from) {
                self.dirty.insert(to.clone());
            }
            self.drop_map(from);
        }
        for p in rewritten {
            let t = self.files().get(&p).cloned().unwrap_or_default();
            self.set_text(&p, t);
        }
        for t in self.tabs.iter_mut().chain(self.recent.iter_mut()).chain(self.closed_tabs.iter_mut()) {
            if let Some(nt) = remap(t) {
                *t = nt;
            }
        }
        if let Some(a) = self.active.as_deref().and_then(remap) {
            self.active = Some(a);
        }
        self.expanded = std::mem::take(&mut self.expanded).into_iter().map(|e| remap(&e).unwrap_or(e)).collect();
        self.rebuild_names_pub();
        self.persist_later();
        self.sync_after_save();
        Some(new_path)
    }

    /// Moves a note into `folder` (`""` is the root), keeping its tab, caret and history.
    pub fn move_note(&mut self, from: &str, folder: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        if md::dir(from) == folder || !self.files().contains_key(from) {
            return;
        }
        let name = from.rsplit('/').next().unwrap_or(from).to_string();
        let to = md::join(folder, &name);
        let exists = self.files().contains_key(&to) || self.vault.as_ref().is_some_and(|v| v.path.join(&to).exists());
        if exists {
            self.notice = Some(format!("{} already has a note named {}.", if folder.is_empty() { "The folder root" } else { folder }, md::base(&name)));
            return;
        }
        self.flush_notes();
        let update = self.settings().update_links;
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let text = v.take(from).unwrap_or_default();
        v.put(to.clone(), text);
        if let Some(b) = v.born.remove(from) {
            v.born.insert(to.clone(), b);
        }
        self.writer.send(Op::Rename { root, from: from.to_string(), to: to.clone() });
        // Links by note name still find it. Links that spell out the old folder follow it.
        let mut rewritten = Vec::new();
        if update {
            let (old_full, new_full) = (md::strip_md(from).to_string(), md::strip_md(&to).to_string());
            if let Some(v) = &mut self.vault {
                let changed: Vec<(String, String)> = v.files().iter().filter_map(|(p, t)| {
                    let nt = md::rewrite_links(t, &old_full, &new_full);
                    (nt != *t).then(|| (p.clone(), nt))
                }).collect();
                for (p, nt) in changed {
                    rewritten.push(p.clone());
                    v.put(p, nt);
                }
            }
        }
        if let Some(u) = self.unlocked.remove(from) {
            self.unlocked.insert(to.clone(), u);
        }
        if let Some(e) = self.edits.remove(from) {
            self.edits.insert(to.clone(), e);
        }
        for p in rewritten {
            let t = self.files().get(&p).cloned().unwrap_or_default();
            self.set_text(&p, t);
        }
        for t in self.tabs.iter_mut().chain(self.recent.iter_mut()) {
            if t == from {
                *t = to.clone();
            }
        }
        if self.active.as_deref() == Some(from) {
            self.active = Some(to.clone());
        }
        if !folder.is_empty() {
            self.expanded.insert(folder.to_string());
        }
        self.rebuild_names_pub();
        self.persist_later();
        self.sync_after_save();
        let _ = tx;
    }

    /// A file row's drag: past 6 points it becomes a move, and the folder under the pointer
    /// takes the note when the button comes up.
    pub fn drag_note(&mut self, path: &str, d: &Drag, tx: &mut Tx) {
        if d.cancelled() {
            self.dragging = None;
            return;
        }
        match d.phase {
            Phase::Down => {
                self.dragging = Some(DragNote { path: path.to_string(), x: d.window_point()[0], y: d.window_point()[1], active: false, target: None });
            }
            Phase::Move => {
                let Some(dn) = &mut self.dragging else { return };
                dn.x = d.window_point()[0];
                dn.y = d.window_point()[1];
                if !dn.active && d.dx.abs() + d.dy.abs() > 6.0 {
                    dn.active = true;
                }
                if dn.active {
                    let (x, y) = (dn.x, dn.y);
                    tx.element_at(x, y, |s: &mut App, picked, _| {
                        let target = picked.and_then(|p| folder_at(p.path()));
                        if let Some(dn) = &mut s.dragging {
                            dn.target = target.filter(|f| *f != md::dir(&dn.path));
                        }
                    });
                }
            }
            Phase::Up => {
                if let Some(dn) = self.dragging.take() {
                    if dn.active {
                        if let Some(t) = dn.target {
                            self.move_note(&dn.path, &t, tx);
                        }
                    }
                }
            }
        }
    }


    /// The notes inside `folder`, its subfolders included.
    pub fn notes_in(&self, folder: &str) -> Vec<String> {
        let pre = format!("{folder}/");
        self.files().keys().filter(|p| p.starts_with(&pre)).cloned().collect()
    }

    /// Deletes a folder from the menu: an empty folder goes at once, and a folder with notes
    /// asks first, with the keyboard on Cancel, its ring showing, so Enter never deletes by
    /// surprise.
    pub fn ask_delete_folder(&mut self, folder: &str, at: (f32, f32), tx: &mut Tx) {
        if self.notes_in(folder).is_empty() {
            self.delete_folder(folder);
            self.row_deleted(tx);
        } else {
            self.open_menu(crate::layer::MenuOf::DeleteFolder(folder.to_string()), at, tx);
            tx.focus("tm-cancel", true);
        }
    }

    /// Moves a folder, with every file in it, to `.trash` in the notes folder. Unsaved notes
    /// go to disk first, so the trash holds their last text.
    pub fn delete_folder(&mut self, folder: &str) {
        if self.refuse_in_note_mode() {
            return;
        }
        self.flush_notes();
        let deleted = self.record_delete(folder, true);
        self.remove_folder(folder);
        self.deleted = Some(deleted);
        self.sync_after_save();
    }

    /// `delete_folder` without the sync after it, for a delete that came from a sync.
    pub(crate) fn remove_folder(&mut self, folder: &str) {
        if folder.is_empty() {
            return;
        }
        let pre = format!("{folder}/");
        let inside = |p: &str| p == folder || p.starts_with(&pre);
        self.flush_notes();
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let gone: Vec<String> = v.files().keys().filter(|p| inside(p)).cloned().collect();
        for p in &gone {
            v.take(p);
            v.born.remove(p);
        }
        v.folders.retain(|f| !inside(f));
        self.writer.send(Op::DeleteFolder { root, rel: folder.to_string() });
        for p in &gone {
            self.close_tab(p);
            self.edits.remove(p);
        }
        self.recent.retain(|p| !inside(p));
        self.unlocked.retain(|p, _| !inside(p));
        self.expanded.retain(|p| !inside(p));
        if self.new_folder.as_ref().is_some_and(|nf| inside(&nf.parent)) {
            self.new_folder = None;
        }
        if self.renaming_folder.as_ref().is_some_and(|rf| inside(&rf.path)) {
            self.renaming_folder = None;
        }
        if self.active.as_deref().is_some_and(inside) {
            self.active = None;
        }
        self.rebuild_names_pub();
        self.refresh_trash();
        self.persist_later();
    }

    /// What deleting `rel`, a note or a folder, will take, for Undo. Call it before the delete,
    /// after unsaved notes went to disk.
    pub(crate) fn record_delete(&self, rel: &str, folder: bool) -> Deleted {
        let pre = format!("{rel}/");
        let inside = |p: &str| p == rel || (folder && p.starts_with(&pre));
        let (notes, folders) = self.vault.as_ref().map_or_else(Default::default, |v| {
            let notes = v.files().iter().filter(|(p, _)| inside(p)).map(|(p, t)| (p.clone(), t.clone(), v.born.get(p).copied())).collect();
            (notes, v.folders.iter().filter(|f| inside(f)).cloned().collect())
        });
        let name = if folder { rel.rsplit('/').next().unwrap_or(rel) } else { md::base(rel) };
        Deleted {
            name: name.to_string(),
            at: self.now.get(),
            rel: rel.to_string(),
            folder,
            notes,
            folders,
            tabs: self.tabs.iter().enumerate().filter(|(_, t)| inside(t)).map(|(i, t)| (i, t.clone())).collect(),
            active: self.active.clone().filter(|a| inside(a)),
            expanded: self.expanded.iter().filter(|e| inside(e)).cloned().collect(),
        }
    }

    /// Undo for the last delete: the Writer moves the note or folder back from `.trash`, and
    /// its notes, tabs and folders come back as they were. A sync since then deleted it on the
    /// other devices. Back here it is a new file, so the next sync uploads it again.
    pub fn undo_delete(&mut self, tx: &mut Tx) {
        let Some(d) = self.deleted.take() else { return };
        let Some(v) = &mut self.vault else { return };
        if d.notes.iter().any(|(p, _, _)| v.files().contains_key(p)) || (d.folder && v.folders.contains(&d.rel)) {
            self.notice = Some(format!("Could not put back {}: something else has its name", d.name));
            return;
        }
        self.writer.send(Op::Restore { root: v.path.clone(), rel: d.rel.clone() });
        for (p, text, born) in d.notes {
            if let Some(b) = born {
                v.born.insert(p.clone(), b);
            }
            v.put(p, text);
        }
        for f in d.folders {
            if !v.folders.contains(&f) {
                v.folders.push(f);
            }
        }
        v.folders.sort();
        for (i, t) in d.tabs {
            let i = i.min(self.tabs.len());
            self.tabs.insert(i, t);
        }
        self.expanded.extend(d.expanded);
        self.rebuild_names_pub();
        self.refresh_trash();
        match d.active {
            Some(a) => self.open_file(&a, None, tx),
            None => self.persist_later(),
        }
        self.sync_after_save();
    }

    /// Ends the Undo offer when its time is up, or wakes then. True when it went.
    pub(crate) fn expire_deleted(&mut self, t: f64, tx: &mut Tx) -> bool {
        let Some(d) = &self.deleted else { return false };
        let left = d.at + UNDO_FOR - t;
        if left > 0.0 {
            tx.wake_after(std::time::Duration::from_secs_f64(left));
            return false;
        }
        self.deleted = None;
        true
    }

    /// Moves a note to `.trash` in the folder, open or not.
    pub fn delete_path(&mut self, p: &str) {
        let was = self.active.clone();
        self.active = Some(p.to_string());
        self.delete_note();
        if was.as_deref() != Some(p) {
            // Undo leaves the shown note shown.
            if let Some(d) = &mut self.deleted {
                d.active = None;
            }
            self.active = was.filter(|a| self.files().contains_key(a));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_paths_name_their_folder() {
        assert_eq!(folder_at("root/body/sidebar/lists/vault/side-scroll/#0/tree/dir:Docs|Specs/#2"), Some("Docs/Specs".into()));
        assert_eq!(folder_at("root/body/sidebar/lists/vault/side-scroll/#0/tree/file:Journal|2026.md/#1"), Some("Journal".into()));
        assert_eq!(folder_at("root/body/sidebar/lists/vault/side-scroll/#0"), Some(String::new()), "the tree's empty space");
        assert_eq!(folder_at("root/body/sidebar/lists/vault/label-vault"), Some(String::new()), "the vault's label");
        assert_eq!(folder_at("root/body/sidebar/lists/shelf/shelf-scroll/#0/open-notes/tab:a.md"), None, "the shelf takes no drop");
        assert_eq!(folder_at("root/body/sidebar/side-top/search-box"), None, "nor the top bar");
        assert_eq!(folder_at("root/body/sidebar/results-scroll/#0/results"), None, "nor the results");
        assert_eq!(folder_at("root/body/sidebar/side-foot/foot-tags"), None, "nor the footer");
        assert_eq!(folder_at("root/body/sheet/doc-scroll"), None);
    }

    fn names(f: &Folder) -> (Vec<&str>, Vec<&str>) {
        (f.folders.iter().map(|f| f.name()).collect(), f.notes.iter().map(|n| md::base(n)).collect())
    }

    /// SB-52, SB-54: a folder counts the notes in its subfolders as well, an empty folder
    /// counts none, and each folder lists its subfolders first, then its notes, each by name
    /// ignoring case.
    #[test]
    fn counts_include_subfolders() {
        let notes: Vec<String> = ["b.md", "Zed/one.md", "apple.md", "Zed/Deep/er/two.md", "Zed/Deep/three.md", "alpha/x.md"].map(String::from).into();
        let root = Folder::tree(&notes, &["Empty".into(), "Zed/Deep".into()]);
        assert_eq!(root.count, 6, "the root counts every note");
        assert_eq!(names(&root), (vec!["alpha", "Empty", "Zed"], vec!["apple", "b"]), "folders first, then notes, ignoring case");
        let zed = &root.folders[2];
        assert_eq!((zed.path.as_str(), zed.count), ("Zed", 3), "Zed's one note and the two under it");
        assert_eq!(names(zed), (vec!["Deep"], vec!["one"]));
        assert_eq!((zed.folders[0].count, zed.folders[0].folders[0].path.as_str(), zed.folders[0].folders[0].count), (2, "Zed/Deep/er", 1));
        assert_eq!(root.folders[1].count, 0, "an empty folder counts none");
    }

    /// The tree is built again when a note comes or goes or a folder is made, and not when a
    /// note's text changes, so typing never sorts the vault again.
    #[test]
    fn the_tree_follows_the_notes_not_their_text() {
        let mut v = crate::vault::Vault::for_tests([("a.md".to_string(), "A".to_string())].into());
        let mut tree = Tree::default();
        let first = tree.of(&v);
        assert_eq!(first.count, 1);
        v.put("a.md".into(), "A, edited".into());
        assert!(std::rc::Rc::ptr_eq(&first, &tree.of(&v)), "an edit keeps the tree");
        v.put("Box/b.md".into(), "B".into());
        assert_eq!((tree.of(&v).count, tree.of(&v).folders[0].count), (2, 1), "a new note counts at once");
        v.take("a.md");
        assert_eq!(tree.of(&v).count, 1, "a note taken out stops counting");
        v.folders.push("New".into());
        assert_eq!(tree.of(&v).folders.iter().map(|f| f.name()).collect::<Vec<_>>(), ["Box", "New"], "a new folder shows");
    }

    #[test]
    fn paths_split_into_folder_and_name() {
        assert_eq!(split_path("Drafts/Ideas/A: b"), ("Drafts/Ideas".into(), "A- b".into()));
        assert_eq!(split_path("../x"), (String::new(), "x".into()));
    }

    #[test]
    fn folder_rename_updates_paths_tabs_and_links() {
        let dir = std::env::temp_dir().join(format!("quire-folder-rename-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Archive/Sub")).unwrap();
        std::fs::write(dir.join("Archive/Old.md"), "# Old").unwrap();
        std::fs::write(dir.join("Archive/Sub/Deep.md"), "# Deep").unwrap();
        std::fs::write(dir.join("Index.md"), "Link [[Archive/Old]] and [[Archive/Sub/Deep]]").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Archive/Old.md", None, tx);
            s.expanded.insert("Archive".into());
            s.start_rename_folder("Archive", tx);
            if let Some(rf) = s.renaming_folder.as_mut() {
                rf.edit = TextEdit::new("Vaulted");
            }
            s.end_rename_folder(true, tx);
        });
        rt.update(|s, _| {
            assert_eq!(s.active.as_deref(), Some("Vaulted/Old.md"));
            assert!(s.files().contains_key("Vaulted/Old.md"));
            assert!(s.files().contains_key("Vaulted/Sub/Deep.md"));
            assert!(!s.files().contains_key("Archive/Old.md"));
            assert!(s.expanded.contains("Vaulted"));
            assert_eq!(
                s.files().get("Index.md").map(String::as_str),
                Some("Link [[Vaulted/Old]] and [[Vaulted/Sub/Deep]]")
            );
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
