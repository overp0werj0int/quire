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

#[derive(Clone, Debug)]
pub struct TreeMenu {
    pub x: f32,
    pub y: f32,
    pub on: MenuOn,
    /// The menu asks before it deletes the folder it was opened on.
    pub confirm: bool,
}

/// A folder being named in the tree.
pub struct NewFolder {
    pub parent: String,
    pub edit: TextEdit,
}

/// A tree row key holds its path with `|` for `/`, since `/` separates keys in element paths.
pub fn row_key(kind: &str, path: &str) -> String {
    format!("{kind}:{}", path.replace('/', "|"))
}

/// The folder an element path points into: a folder row is that folder, a note row is its
/// note's folder, and the tree's empty space or the folder name is the root.
pub fn folder_at(path: &str) -> Option<String> {
    for seg in path.split('/').rev() {
        if let Some(p) = seg.strip_prefix("dir:") {
            return Some(p.replace('|', "/"));
        }
        if let Some(p) = seg.strip_prefix("file:") {
            return Some(md::dir(&p.replace('|', "/")).to_string());
        }
    }
    let segs = || path.split('/');
    (segs().any(|s| matches!(s, "sidebar" | "side-scroll" | "vault-head")) && !segs().any(|s| s == "open-notes")).then(String::new)
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
    /// A new note in `folder` (`None`: the folder from settings). A `name` with slashes, such as
    /// `Drafts/Idea`, makes the note in that folder, creating any folders it names.
    pub fn new_note_in(&mut self, folder: Option<&str>, name: Option<&str>, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        if self.vault.is_none() {
            return;
        }
        let mut base_folder = match folder {
            Some(f) => f.to_string(),
            None => {
                let f = self.settings().new_folder.clone();
                if !f.is_empty() && !self.folder_list().contains(&f) { String::new() } else { f }
            }
        };
        let (sub, bn) = match name {
            Some(n) if n.contains('/') => {
                let (d, b) = split_path(n);
                // A path from the root wins over the settings folder.
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
            v.files.insert(p.clone(), content.clone());
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
        self.tree_menu = None;
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
        self.left_panel = crate::app::Panel::Files;
        self.new_folder = Some(NewFolder { parent: parent.to_string(), edit });
        self.tree_menu = None;
        tx.focus("new-folder", false);
    }

    pub fn commit_new_folder(&mut self) {
        let Some(nf) = self.new_folder.take() else { return };
        let name = md::clean_name(&nf.edit.text);
        if name.is_empty() || name.starts_with('.') {
            return;
        }
        let path = md::join(&nf.parent, &name);
        let Some(v) = &mut self.vault else { return };
        if v.folders.contains(&path) {
            self.notice = Some(format!("A folder named {name} is already there."));
            return;
        }
        v.folders.push(path.clone());
        v.folders.sort();
        let root = v.path.clone();
        self.writer.send(Op::Mkdir { root, rel: path.clone() });
        self.expanded.insert(path);
        if !nf.parent.is_empty() {
            self.expanded.insert(nf.parent);
        }
        self.persist_later();
        self.sync_after_save();
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
        let text = v.files.remove(from).unwrap_or_default();
        v.files.insert(to.clone(), text);
        if let Some(b) = v.born.remove(from) {
            v.born.insert(to.clone(), b);
        }
        self.writer.send(Op::Rename { root, from: from.to_string(), to: to.clone() });
        // Links by note name still find it. Links that spell out the old folder follow it.
        let mut rewritten = Vec::new();
        if update {
            let (old_full, new_full) = (md::strip_md(from).to_string(), md::strip_md(&to).to_string());
            if let Some(v) = &mut self.vault {
                for (p, t) in v.files.iter_mut() {
                    let nt = md::rewrite_links(t, &old_full, &new_full);
                    if nt != *t {
                        *t = nt;
                        rewritten.push(p.clone());
                    }
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

    pub fn open_tree_menu(&mut self, on: MenuOn, x: f32, y: f32) {
        self.tree_menu = Some(TreeMenu { x, y, on, confirm: false });
        self.vault_menu = false;
    }

    /// The notes inside `folder`, its subfolders included.
    pub fn notes_in(&self, folder: &str) -> Vec<String> {
        let pre = format!("{folder}/");
        self.files().keys().filter(|p| p.starts_with(&pre)).cloned().collect()
    }

    /// Deletes a folder from the menu: an empty folder goes at once, and a folder with notes
    /// asks first.
    pub fn ask_delete_folder(&mut self, folder: &str, x: f32, y: f32) {
        if self.notes_in(folder).is_empty() {
            self.tree_menu = None;
            self.delete_folder(folder);
        } else {
            self.tree_menu = Some(TreeMenu { x, y, on: MenuOn::Folder(folder.to_string()), confirm: true });
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
        let gone: Vec<String> = v.files.keys().filter(|p| inside(p)).cloned().collect();
        for p in &gone {
            v.files.remove(p);
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
        if self.active.as_deref().is_some_and(inside) {
            self.active = None;
        }
        self.rebuild_names_pub();
        self.persist_later();
    }

    /// What deleting `rel`, a note or a folder, will take, for Undo. Call it before the delete,
    /// after unsaved notes went to disk.
    pub(crate) fn record_delete(&self, rel: &str, folder: bool) -> Deleted {
        let pre = format!("{rel}/");
        let inside = |p: &str| p == rel || (folder && p.starts_with(&pre));
        let (notes, folders) = self.vault.as_ref().map_or_else(Default::default, |v| {
            let notes = v.files.iter().filter(|(p, _)| inside(p)).map(|(p, t)| (p.clone(), t.clone(), v.born.get(p).copied())).collect();
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
        if d.notes.iter().any(|(p, _, _)| v.files.contains_key(p)) || (d.folder && v.folders.contains(&d.rel)) {
            self.notice = Some(format!("Could not put back {}: something else has its name", d.name));
            return;
        }
        self.writer.send(Op::Restore { root: v.path.clone(), rel: d.rel.clone() });
        for (p, text, born) in d.notes {
            if let Some(b) = born {
                v.born.insert(p.clone(), b);
            }
            v.files.insert(p, text);
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
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list/tree/dir:Docs|Specs/#2"), Some("Docs/Specs".into()));
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list/tree/file:Journal|2026.md/#1"), Some("Journal".into()));
        assert_eq!(folder_at("root/body/sidebar/side-scroll/side-list"), Some(String::new()));
        assert_eq!(folder_at("root/body/sheet/doc-scroll"), None);
    }

    #[test]
    fn paths_split_into_folder_and_name() {
        assert_eq!(split_path("Drafts/Ideas/A: b"), ("Drafts/Ideas".into(), "A- b".into()));
        assert_eq!(split_path("../x"), (String::new(), "x".into()));
    }
}
