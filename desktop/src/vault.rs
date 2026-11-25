//! A notes folder on disk: reading it, and a writer thread that keeps disk work off the UI thread.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Vault {
    pub name: String,
    pub path: PathBuf,
    /// Folders relative to the root, `/`-separated, including empty ones.
    pub folders: Vec<String>,
    /// Note path relative to the root (`Drafts/On slow software.md`) to its text. Private, so
    /// every write goes through `put` or `take`, which count it for search's index.
    files: BTreeMap<String, String>,
    /// Counts the writes to `files`: one for each `put` and `take`.
    writes: u64,
    /// Counts the changes to which notes there are: a `put` of a new path, a `take` of one.
    paths: u64,
    /// Tells this vault from any other read in this run, so a cache of its notes is never
    /// taken for another's.
    id: u64,
    /// The `writes` count at the last write to each note.
    ver: std::collections::HashMap<String, u64>,
    /// How many times each note path was written (`put`), so `writes - write_count(active)` stays
    /// invariant while editing only `active`.
    note_writes: std::collections::HashMap<String, u64>,
    /// Creation order of each note (file birth time, else modification time), for lists the
    /// design keeps in the order notes were made, such as backlinks.
    pub born: std::collections::HashMap<String, u128>,
    pub scope: Scope,
}

/// What an open vault holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// A notes folder in the list: every note under the root, remembered and synced.
    Folder,
    /// Note mode: one note opened as a file from outside the folders in the list (a file
    /// double-clicked in a file manager), rooted at its own folder. Only that note is read and
    /// watched; the folder is not scanned, listed, remembered or synced, and no note is added
    /// to it, renamed, moved or deleted.
    Note,
}

fn skip(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

/// Reads every `.md` file under `root`. Hidden folders (`.git`, `.trash`) stay out.
pub fn load(root: &Path) -> std::io::Result<Vault> {
    let mut files = BTreeMap::new();
    let mut born = std::collections::HashMap::new();
    let mut folders = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    let mut count = 0usize;
    while let Some((dir, rel)) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if rel.is_empty() => return Err(e),
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() {
                folders.push(child.clone());
                stack.push((entry.path(), child));
            } else if ft.is_file() && name.to_lowercase().ends_with(".md") {
                count += 1;
                if count > 20_000 {
                    continue;
                }
                if let Ok(t) = std::fs::read_to_string(entry.path()) {
                    let meta = entry.metadata().ok();
                    let when = meta.and_then(|m| m.created().or_else(|_| m.modified()).ok());
                    let ns = when.and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
                    born.insert(child.clone(), ns);
                    files.insert(child, t.replace("\r\n", "\n"));
                }
            }
        }
    }
    folders.sort();
    let name = root.file_name().map_or_else(|| root.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
    Ok(Vault { name, path: root.to_path_buf(), folders, files, born, scope: Scope::Folder, id: next_id(), writes: 0, paths: 0, ver: Default::default(), note_writes: Default::default() })
}

/// Reads one note for Note mode (`Scope::Note`): the vault's root is the note's folder and its
/// only note is the file.
pub fn load_one(file: &Path) -> std::io::Result<Vault> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a note file");
    let root = file.parent().ok_or_else(bad)?;
    let rel = file.file_name().ok_or_else(bad)?.to_string_lossy().into_owned();
    let text = std::fs::read_to_string(file)?.replace("\r\n", "\n");
    let meta = std::fs::metadata(file).ok();
    let when = meta.and_then(|m| m.created().or_else(|_| m.modified()).ok());
    let ns = when.and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let name = root.file_name().map_or_else(|| root.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
    Ok(Vault {
        name,
        path: root.to_path_buf(),
        folders: Vec::new(),
        born: [(rel.clone(), ns)].into(),
        files: [(rel, text)].into(),
        scope: Scope::Note,
        id: next_id(),
        writes: 0,
        paths: 0,
        ver: Default::default(),
        note_writes: Default::default(),
    })
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Vault {
    /// The notes, by path relative to the root, with their text.
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    /// The notes, taken out of the vault.
    pub fn into_files(self) -> BTreeMap<String, String> {
        self.files
    }

    /// Writes the note at `path`.
    pub fn put(&mut self, path: String, text: String) {
        self.replace(path, text);
    }

    /// Replaces `path`'s text in the vault and returns its previous text without cloning.
    pub fn replace(&mut self, path: String, text: String) -> Option<String> {
        self.writes += 1;
        self.ver.insert(path.clone(), self.writes);
        *self.note_writes.entry(path.clone()).or_insert(0) += 1;
        let prev = self.files.insert(path, text);
        if prev.is_none() {
            self.paths += 1;
        }
        prev
    }

    /// Takes the note at `path` out, with its text.
    pub fn take(&mut self, path: &str) -> Option<String> {
        self.writes += 1;
        self.ver.remove(path);
        self.note_writes.remove(path);
        let text = self.files.remove(path);
        if text.is_some() {
            self.paths += 1;
        }
        text
    }

    /// This vault's own number: no other vault read in this run has it.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Changes with every write to the notes.
    pub fn writes(&self) -> u64 {
        self.writes
    }

    /// Changes whenever any note other than `path` is written, added, or removed; stays constant
    /// when only `path` is edited in place.
    pub fn other_writes(&self, path: &str) -> u64 {
        self.writes.saturating_sub(self.note_writes.get(path).copied().unwrap_or(0))
    }

    /// Changes when a note is added or taken out, and not when a note's text changes.
    pub fn paths(&self) -> u64 {
        self.paths
    }

    /// The `writes` count at the last write to the note at `path`: 0 for one read from disk and not
    /// written since.
    pub fn ver(&self, path: &str) -> u64 {
        self.ver.get(path).copied().unwrap_or(0)
    }

    /// A vault of `files` with no folder on disk, for tests.
    #[cfg(test)]
    pub fn for_tests(files: BTreeMap<String, String>) -> Vault {
        Vault { name: "test".into(), path: PathBuf::new(), folders: Vec::new(), files, born: Default::default(), scope: Scope::Folder, id: next_id(), writes: 0, paths: 0, ver: Default::default(), note_writes: Default::default() }
    }
}

pub fn count_notes(root: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() && name.to_lowercase().ends_with(".md") => n += 1,
                _ => {}
            }
        }
        if n > 99_999 {
            break;
        }
    }
    n
}

/// A trashed note or folder inside `<vault>/.trash/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashEntry {
    /// Entry filename inside `.trash/`.
    pub name: String,
    /// Relative path inside the vault where restoring places it (before deduplication).
    pub original_rel: String,
    /// Whether this entry is a trashed directory.
    pub is_dir: bool,
    /// Last modification time (nanoseconds since epoch), for newest-first order.
    pub deleted_at: u128,
    /// Short content preview for trashed notes (empty for folders).
    pub snippet: String,
}

/// Completed trash scan or recovery event sent from the `Writer` thread back to `App`.
#[derive(Clone, Debug)]
pub enum TrashEvent {
    Listed {
        root: PathBuf,
        entries: Vec<TrashEntry>,
    },
    Restored {
        root: PathBuf,
        original_rel: String,
        label: String,
        files: Vec<(String, String, u128)>,
        folders: Vec<String>,
        open_rel: Option<String>,
        entries: Vec<TrashEntry>,
    },
    ItemDeleted {
        root: PathBuf,
        entries: Vec<TrashEntry>,
    },
    Emptied {
        root: PathBuf,
    },
}

const TRASH_META: &str = ".meta.json";

/// Sanitizes a vault-relative path so restore never escapes the vault root.
fn sanitize_rel(raw: &str) -> String {
    let norm = raw.replace('\\', "/");
    let parts: Vec<&str> = norm
        .split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .collect();
    if parts.is_empty() {
        "Restored.md".to_string()
    } else {
        parts.join("/")
    }
}

fn read_trash_meta(root: &Path) -> BTreeMap<String, String> {
    let path = root.join(".trash").join(TRASH_META);
    let Ok(bytes) = std::fs::read(&path) else { return BTreeMap::new() };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn write_trash_meta(root: &Path, meta: &BTreeMap<String, String>) {
    let path = root.join(".trash").join(TRASH_META);
    if meta.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(meta) {
        let _ = write_atomic(&path, &bytes);
    }
}

fn record_trash_meta(root: &Path, trash_name: &str, original_rel: &str) {
    let mut meta = read_trash_meta(root);
    meta.insert(trash_name.to_string(), sanitize_rel(original_rel));
    write_trash_meta(root, &meta);
}

fn remove_trash_meta(root: &Path, trash_name: &str) {
    let mut meta = read_trash_meta(root);
    if meta.remove(trash_name).is_some() {
        write_trash_meta(root, &meta);
    }
}

/// Resolves the vault-relative restore path for a `.trash` entry: exact manifest lookup first,
/// else legacy fallback that only splits `" - "` prefixes matching an existing vault folder.
pub fn original_rel_from_trash(root: &Path, meta: &BTreeMap<String, String>, name: &str) -> String {
    if let Some(rel) = meta.get(name) {
        return sanitize_rel(rel);
    }
    let parts: Vec<&str> = name.split(" - ").collect();
    if parts.len() > 1 {
        for split_at in (1..parts.len()).rev() {
            let dir_rel = sanitize_rel(&parts[..split_at].join("/"));
            if !dir_rel.is_empty() && root.join(&dir_rel).is_dir() {
                let tail = parts[split_at..].join(" - ");
                return sanitize_rel(&format!("{dir_rel}/{tail}"));
            }
        }
    }
    sanitize_rel(name)
}

/// Picks a non-colliding vault-relative destination path for restoring `original_rel` into `root`.
fn safe_restore_rel(root: &Path, original_rel: &str, is_dir: bool) -> String {
    let base = sanitize_rel(original_rel);
    if !root.join(&base).exists() {
        return base;
    }
    let (stem, ext) = if is_dir {
        (base.as_str(), None)
    } else {
        match base.rsplit_once('.') {
            Some((s, e)) if !e.contains('/') => (s, Some(e)),
            _ => (base.as_str(), None),
        }
    };
    let mut n = 2u32;
    loop {
        let next = match ext {
            Some(e) => format!("{stem} {n}.{e}"),
            None => format!("{stem} {n}"),
        };
        if !root.join(&next).exists() {
            return next;
        }
        n += 1;
    }
}

/// Lists trashed files and folders inside `<root>/.trash/`, newest first.
pub fn list_trash(root: &Path) -> Vec<TrashEntry> {
    let dir = root.join(".trash");
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let meta = read_trash_meta(root);
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        let is_dir = ft.is_dir();
        if !is_dir && !ft.is_file() {
            continue;
        }
        let deleted_at = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().or_else(|_| m.created()).ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let original_rel = original_rel_from_trash(root, &meta, &name);
        let snippet = if !is_dir && name.to_lowercase().ends_with(".md") {
            trash_note_snippet(&entry.path())
        } else {
            String::new()
        };
        out.push(TrashEntry { name, original_rel, is_dir, deleted_at, snippet });
    }
    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

fn trash_note_snippet(path: &Path) -> String {
    let Ok(raw) = std::fs::read_to_string(path) else { return String::new() };
    if crate::lock::is_locked(&raw) {
        return "Locked note".to_string();
    }
    let mut lines = raw.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim() == "---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim() == "---" {
                break;
            }
        }
    }
    for l in lines {
        let t = l.trim().trim_start_matches(['#', '-', '*', '>', ' ']).trim();
        if !t.is_empty() {
            let mut s: String = t.chars().take(72).collect();
            if t.chars().count() > 72 {
                s.push('…');
            }
            return s;
        }
    }
    String::new()
}

/// Reads restored `.md` files and subfolders rooted at `root/dest_rel`.
fn collect_restored(root: &Path, dest_rel: &str, is_dir: bool) -> (Vec<(String, String, u128)>, Vec<String>) {
    let mut files = Vec::new();
    let mut folders = Vec::new();
    let dst = root.join(dest_rel);
    if !is_dir {
        let mut d = crate::md::dir(dest_rel).to_string();
        while !d.is_empty() {
            folders.push(d.clone());
            d = crate::md::dir(&d).to_string();
        }
        if dest_rel.to_lowercase().ends_with(".md")
            && let Ok(t) = std::fs::read_to_string(&dst)
        {
            let ns = std::fs::metadata(&dst)
                .ok()
                .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                .and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            files.push((dest_rel.to_string(), t.replace("\r\n", "\n"), ns));
        }
        return (files, folders);
    }
    let mut d = dest_rel.to_string();
    while !d.is_empty() {
        folders.push(d.clone());
        d = crate::md::dir(&d).to_string();
    }
    let mut stack = vec![(dst, dest_rel.to_string())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            let child = format!("{rel}/{name}");
            if ft.is_dir() {
                folders.push(child.clone());
                stack.push((entry.path(), child));
            } else if ft.is_file() && name.to_lowercase().ends_with(".md") {
                if let Ok(t) = std::fs::read_to_string(entry.path()) {
                    let ns = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                        .and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_nanos());
                    files.push((child, t.replace("\r\n", "\n"), ns));
                }
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    folders.sort();
    folders.dedup();
    (files, folders)
}

#[derive(Debug)]
pub enum Op {
    Write { root: PathBuf, rel: String, text: String },
    Rename { root: PathBuf, from: String, to: String },
    /// Moves a note to `.trash`.
    Delete { root: PathBuf, rel: String },
    /// Writes a file that is not a note, such as an image from sync.
    WriteBytes { root: PathBuf, rel: String, bytes: Vec<u8> },
    /// Moves a folder, with every file in it, to `.trash`.
    DeleteFolder { root: PathBuf, rel: String },
    /// Moves the note or folder that the last `Delete` or `DeleteFolder` of `rel` put in
    /// `.trash` back to `rel`. It fails if something else is at `rel` by then.
    Restore { root: PathBuf, rel: String },
    /// Scans `<root>/.trash/` off the UI thread and posts `TrashEvent::Listed`.
    ListTrash { root: PathBuf },
    /// Restores a named entry from `<root>/.trash/` back into the vault and posts `TrashEvent::Restored`.
    RestoreTrash { root: PathBuf, trash_name: String },
    /// Permanently removes a single named entry inside `<root>/.trash/` and posts `TrashEvent::ItemDeleted`.
    DeleteTrashItem { root: PathBuf, trash_name: String },
    /// Permanently removes every entry inside `<root>/.trash/` and posts `TrashEvent::Emptied`.
    EmptyTrash { root: PathBuf },
    Mkdir { root: PathBuf, rel: String },
    SaveState(Box<crate::store::Persist>),
    /// Answers once every operation sent before it is done.
    Barrier(std::sync::mpsc::Sender<()>),
    /// Saves a sync's base record after the sync's writes. If any operation failed after
    /// `failures_before`, it deletes the record, so the next sync keeps both texts of a note
    /// instead of taking the old one for an edit.
    SaveSyncBase { path: PathBuf, bytes: Vec<u8>, failures_before: usize },
}

/// Disk work runs on this thread in the order it was sent. Errors land in `errors`.
#[derive(Clone)]
pub struct Writer {
    tx: Sender<Op>,
    pub errors: Arc<Mutex<Vec<String>>>,
    pub trash_events: Arc<Mutex<Vec<TrashEvent>>>,
    waker: Arc<Mutex<Option<rune::Waker>>>,
    /// How many operations failed since the start. It only goes up.
    pub failed: Arc<std::sync::atomic::AtomicUsize>,
}

impl Writer {
    pub fn start(waker: Option<rune::Waker>) -> Writer {
        let (tx, rx) = channel::<Op>();
        let errors: Arc<Mutex<Vec<String>>> = Arc::default();
        let errs = errors.clone();
        let trash_events: Arc<Mutex<Vec<TrashEvent>>> = Arc::default();
        let tev = trash_events.clone();
        let waker_slot: Arc<Mutex<Option<rune::Waker>>> = Arc::new(Mutex::new(waker));
        let waker_bg = waker_slot.clone();
        let failed: Arc<std::sync::atomic::AtomicUsize> = Arc::default();
        let failed2 = failed.clone();
        let wake = move || {
            if let Ok(guard) = waker_bg.lock()
                && let Some(w) = guard.as_ref()
            {
                w();
            }
        };
        std::thread::Builder::new()
            .name("quire-writer".into())
            .spawn(move || {
                let mut trash = Trash::new();
                while let Ok(op) = rx.recv() {
                    if let Op::Barrier(done) = &op {
                        let _ = done.send(());
                        continue;
                    }
                    if let Op::SaveSyncBase { path, bytes, failures_before } = &op {
                        let r = if failed2.load(std::sync::atomic::Ordering::SeqCst) > *failures_before {
                            std::fs::remove_file(path).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
                        } else {
                            write_atomic(path, bytes)
                        };
                        if let Err(e) = r {
                            eprintln!("quire: could not save the sync record: {e}");
                        }
                        continue;
                    }
                    if let Op::ListTrash { root } = &op {
                        let entries = list_trash(root);
                        if let Ok(mut q) = tev.lock() {
                            q.push(TrashEvent::Listed { root: root.clone(), entries });
                        }
                        wake();
                        continue;
                    }
                    if let Op::RestoreTrash { root, trash_name } = &op {
                        match restore_from_trash(root, trash_name, &mut trash) {
                            Ok(ev) => {
                                if let Ok(mut q) = tev.lock() {
                                    q.push(ev);
                                }
                                wake();
                            }
                            Err(e) => {
                                failed2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                if let Ok(mut v) = errs.lock() {
                                    v.push(format!("Could not restore {trash_name}: {e}"));
                                }
                                wake();
                            }
                        }
                        continue;
                    }
                    if let Op::DeleteTrashItem { root, trash_name } = &op {
                        match delete_trash_item(root, trash_name, &mut trash) {
                            Ok(entries) => {
                                if let Ok(mut q) = tev.lock() {
                                    q.push(TrashEvent::ItemDeleted { root: root.clone(), entries });
                                }
                                wake();
                            }
                            Err(e) => {
                                failed2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                if let Ok(mut v) = errs.lock() {
                                    v.push(format!("Could not delete {trash_name}: {e}"));
                                }
                                wake();
                            }
                        }
                        continue;
                    }
                    if let Op::EmptyTrash { root } = &op {
                        match empty_trash_dir(root, &mut trash) {
                            Ok(()) => {
                                if let Ok(mut q) = tev.lock() {
                                    q.push(TrashEvent::Emptied { root: root.clone() });
                                }
                                wake();
                            }
                            Err(e) => {
                                failed2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                if let Ok(mut v) = errs.lock() {
                                    v.push(format!("Could not empty trash: {e}"));
                                }
                                wake();
                            }
                        }
                        continue;
                    }
                    if let Err(e) = apply(&op, &mut trash) {
                        failed2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let what = match &op {
                            Op::Write { rel, .. } | Op::WriteBytes { rel, .. } | Op::Delete { rel, .. } | Op::DeleteFolder { rel, .. } | Op::Mkdir { rel, .. } => format!("save {rel}"),
                            Op::Restore { rel, .. } => format!("put back {rel}"),
                            Op::Rename { from, .. } => format!("save {from}"),
                            Op::SaveState(_) => "save settings".into(),
                            Op::Barrier(_) | Op::SaveSyncBase { .. } | Op::ListTrash { .. } | Op::RestoreTrash { .. } | Op::DeleteTrashItem { .. } | Op::EmptyTrash { .. } => "save".into(),
                        };
                        if let Ok(mut v) = errs.lock() {
                            v.push(format!("Could not {what}: {e}"));
                        }
                        wake();
                    }
                }
            })
            .ok();
        Writer { tx, errors, trash_events, waker: waker_slot, failed }
    }

    pub fn set_waker(&self, w: rune::Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(w);
        }
    }

    pub fn send(&self, op: Op) {
        let _ = self.tx.send(op);
    }
}

fn restore_from_trash(root: &Path, trash_name: &str, trash: &mut Trash) -> std::io::Result<TrashEvent> {
    if trash_name.is_empty() || trash_name.contains('/') || trash_name.contains('\\') || trash_name == "." || trash_name == ".." {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid trash entry"));
    }
    let src = root.join(".trash").join(trash_name);
    if !src.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "it is no longer in .trash"));
    }
    let is_dir = src.is_dir();
    let meta = read_trash_meta(root);
    let original_rel = original_rel_from_trash(root, &meta, trash_name);
    let dest_rel = safe_restore_rel(root, &original_rel, is_dir);
    let dst = root.join(&dest_rel);
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::rename(&src, &dst)?;
    remove_trash_meta(root, trash_name);
    trash.retain(|_, v| v != &src);
    let (files, folders) = collect_restored(root, &dest_rel, is_dir);
    let open_rel = files.first().map(|(p, _, _)| p.clone());
    let label = if is_dir {
        dest_rel.rsplit('/').next().unwrap_or(&dest_rel).to_string()
    } else {
        crate::md::base(&dest_rel).to_string()
    };
    let entries = list_trash(root);
    Ok(TrashEvent::Restored { root: root.to_path_buf(), original_rel, label, files, folders, open_rel, entries })
}

fn delete_trash_item(root: &Path, trash_name: &str, trash: &mut Trash) -> std::io::Result<Vec<TrashEntry>> {
    if trash_name.is_empty() || trash_name.contains('/') || trash_name.contains('\\') || trash_name == "." || trash_name == ".." {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid trash entry"));
    }
    let target = root.join(".trash").join(trash_name);
    if target.is_dir() {
        std::fs::remove_dir_all(&target)?;
    } else if target.exists() {
        std::fs::remove_file(&target)?;
    }
    remove_trash_meta(root, trash_name);
    trash.retain(|_, v| v != &target);
    Ok(list_trash(root))
}

fn empty_trash_dir(root: &Path, trash: &mut Trash) -> std::io::Result<()> {
    let dir = root.join(".trash");
    if dir.exists() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
        }
    }
    trash.retain(|_, v| !v.starts_with(&dir));
    Ok(())
}

/// Writes a file through a temporary file next to it and a rename, so a reader never sees half.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Writes a note atomically: a temporary file next to it, then a rename.
pub fn write_note(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    apply(&Op::Write { root: root.to_path_buf(), rel: rel.to_string(), text: text.to_string() }, &mut Trash::new())
}

impl Writer {
    /// Waits, up to `max`, until every operation sent so far is done.
    pub fn wait(&self, max: std::time::Duration) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Op::Barrier(tx));
        let _ = rx.recv_timeout(max);
    }

    /// A barrier for another thread to wait on.
    pub fn barrier(&self) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Op::Barrier(tx));
        rx
    }
}

/// Where each deleted path went in `.trash`, by its old full path, for `Op::Restore`.
type Trash = std::collections::HashMap<PathBuf, PathBuf>;

fn apply(op: &Op, trash: &mut Trash) -> std::io::Result<()> {
    match op {
        Op::Write { root, rel, text } => {
            let p = root.join(rel);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            let tmp = p.with_file_name(format!(".{}.quire-tmp", p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())));
            std::fs::write(&tmp, text)?;
            std::fs::rename(&tmp, &p)
        }
        Op::Rename { root, from, to } => {
            let dst = root.join(to);
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::rename(root.join(from), dst)
        }
        Op::WriteBytes { root, rel, bytes } => write_atomic(&root.join(rel), bytes),
        Op::Delete { root, rel } => {
            // Move the file to `.trash` inside the folder, so it can come back.
            let src = root.join(rel);
            let dst = trash_file(root, rel)?;
            trash.insert(src, dst);
            Ok(())
        }
        Op::DeleteFolder { root, rel } => {
            let src = root.join(rel);
            if !src.is_dir() {
                return Ok(());
            }
            let name = rel.replace('/', " - ");
            let dst = free(&root.join(".trash"), &name, |n| format!("{name} {n}"))?;
            std::fs::rename(&src, &dst)?;
            if let Some(tn) = dst.file_name().map(|n| n.to_string_lossy().into_owned()) {
                record_trash_meta(root, &tn, rel);
            }
            trash.insert(src, dst);
            Ok(())
        }
        Op::Restore { root, rel } => {
            let dst = root.join(rel);
            let Some(src) = trash.remove(&dst) else {
                return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "it is not in .trash"));
            };
            if dst.exists() {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "something else has its name"));
            }
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::rename(&src, dst)?;
            if let Some(tn) = src.file_name().map(|n| n.to_string_lossy().into_owned()) {
                remove_trash_meta(root, &tn);
            }
            Ok(())
        }
        Op::Mkdir { root, rel } => std::fs::create_dir_all(root.join(rel)),
        Op::SaveState(p) => crate::store::save(p),
        Op::Barrier(_) | Op::SaveSyncBase { .. } | Op::ListTrash { .. } | Op::RestoreTrash { .. } | Op::DeleteTrashItem { .. } | Op::EmptyTrash { .. } => Ok(()),
    }
}

/// Moves a note file `rel` inside `root` into `<root>/.trash/` with deduplication and
/// `.trash/.meta.json` path recording so Trash restore puts it back in its original folder.
pub(crate) fn trash_file(root: &Path, rel: &str) -> std::io::Result<PathBuf> {
    let src = root.join(rel);
    let name = rel.replace('/', " - ");
    let (stem, ext) = name.rsplit_once('.').map_or((name.as_str(), "md"), |(s, e)| (s, e));
    let dst = free(&root.join(".trash"), &name, |n| format!("{stem} {n}.{ext}"))?;
    std::fs::rename(&src, &dst)?;
    if let Some(tn) = dst.file_name().map(|n| n.to_string_lossy().into_owned()) {
        record_trash_meta(root, &tn, rel);
    }
    Ok(dst)
}

/// A path in `.trash` (made if missing) named `name`, or `numbered(2)`, `numbered(3)` and on
/// while the name is taken.
fn free(trash: &Path, name: &str, numbered: impl Fn(u32) -> String) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(trash)?;
    let mut dst = trash.join(name);
    let mut n = 2;
    while dst.exists() {
        dst = trash.join(numbered(n));
        n += 1;
    }
    Ok(dst)
}

/// Writes the sample notes into `root`, replacing files with the same names.
pub fn write_sample(root: &Path) -> std::io::Result<()> {
    for f in crate::sample::FOLDERS {
        std::fs::create_dir_all(root.join(f))?;
    }
    // Files keep the order they were made in (some lists follow it). File times tick in
    // steps of a few milliseconds, so leave a gap between the notes.
    for (rel, text) in crate::sample::files() {
        let p = root.join(rel);
        let _ = std::fs::remove_file(&p);
        std::fs::write(&p, text)?;
        std::thread::sleep(std::time::Duration::from_millis(6));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deleted_folder_moves_to_the_trash_with_its_files() {
        let root = std::env::temp_dir().join(format!("quire-delfolder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for p in ["Drafts/Old/A.md", "Drafts/Old/img.png"] {
            std::fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
            std::fs::write(root.join(p), "x").unwrap();
        }
        let mut trash = Trash::new();
        let mut del = |rel: &str| apply(&Op::DeleteFolder { root: root.clone(), rel: rel.into() }, &mut trash).unwrap();
        del("Drafts/Old");
        assert!(root.join(".trash/Drafts - Old/A.md").is_file());
        assert!(root.join(".trash/Drafts - Old/img.png").is_file(), "files that are not notes move too");
        assert!(!root.join("Drafts/Old").exists());
        // A second folder with the same path gets a number.
        std::fs::create_dir_all(root.join("Drafts/Old")).unwrap();
        del("Drafts/Old");
        assert!(root.join(".trash/Drafts - Old 2").is_dir());
        // A folder that is not on disk is no error.
        del("Missing");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_restore_puts_back_what_the_last_delete_moved() {
        let root = std::env::temp_dir().join(format!("quire-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut trash = Trash::new();
        let mut run = |op: Op| apply(&op, &mut trash);
        let (r, rel) = (root.clone(), String::from("Drafts/A.md"));
        // Two notes of the same name were deleted. The second went to "Drafts - A 2.md".
        for text in ["one", "two"] {
            std::fs::create_dir_all(root.join("Drafts")).unwrap();
            std::fs::write(root.join(&rel), text).unwrap();
            run(Op::Delete { root: r.clone(), rel: rel.clone() }).unwrap();
        }
        std::fs::remove_dir(root.join("Drafts")).unwrap();
        run(Op::Restore { root: r.clone(), rel: rel.clone() }).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(&rel)).unwrap(), "two", "the last delete comes back, folder and all");
        assert!(root.join(".trash/Drafts - A.md").is_file(), "the older one stays in the trash");
        // A restore never replaces a file that took the name, and asks only once.
        run(Op::Delete { root: r.clone(), rel: rel.clone() }).unwrap();
        std::fs::write(root.join(&rel), "new").unwrap();
        assert!(run(Op::Restore { root: r.clone(), rel: rel.clone() }).is_err());
        assert_eq!(std::fs::read_to_string(root.join(&rel)).unwrap(), "new");
        assert!(run(Op::Restore { root: r.clone(), rel: rel.clone() }).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_list_restore_and_empty_round_trip() {
        let root = std::env::temp_dir().join(format!("quire-trash-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Drafts")).unwrap();
        std::fs::write(root.join("Drafts/Idea.md"), "# Idea\n").unwrap();
        let mut trash = Trash::new();
        apply(&Op::Delete { root: root.clone(), rel: "Drafts/Idea.md".into() }, &mut trash).unwrap();
        let entries = list_trash(&root);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Drafts - Idea.md");
        assert_eq!(entries[0].original_rel, "Drafts/Idea.md");
        assert!(!entries[0].is_dir);

        // Restoring into an empty slot restores the original path; colliding path deduplicates.
        std::fs::write(root.join("Drafts/Idea.md"), "# New\n").unwrap();
        let ev = restore_from_trash(&root, "Drafts - Idea.md", &mut trash).unwrap();
        match ev {
            TrashEvent::Restored { files, entries, .. } => {
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].0, "Drafts/Idea 2.md");
                assert!(entries.is_empty());
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(root.join("Drafts/Idea 2.md")).unwrap(), "# Idea\n");

        // Deleting again and emptying trash clears `.trash`.
        apply(&Op::Delete { root: root.clone(), rel: "Drafts/Idea 2.md".into() }, &mut trash).unwrap();
        assert_eq!(list_trash(&root).len(), 1);
        empty_trash_dir(&root, &mut trash).unwrap();
        assert!(list_trash(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_preserves_dash_filenames_and_nested_paths() {
        let root = std::env::temp_dir().join(format!("quire-trash-dash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Sub")).unwrap();
        std::fs::create_dir_all(root.join("Folder/Sub")).unwrap();
        std::fs::write(root.join("2026 - Plan.md"), "# Root Plan\n").unwrap();
        std::fs::write(root.join("Sub/2026 - Plan.md"), "# Sub Plan\n").unwrap();
        std::fs::write(root.join("Folder/Sub/Note.md"), "# Deep Note\n").unwrap();

        let mut trash = Trash::new();
        apply(&Op::Delete { root: root.clone(), rel: "2026 - Plan.md".into() }, &mut trash).unwrap();
        apply(&Op::Delete { root: root.clone(), rel: "Sub/2026 - Plan.md".into() }, &mut trash).unwrap();
        apply(&Op::Delete { root: root.clone(), rel: "Folder/Sub/Note.md".into() }, &mut trash).unwrap();

        let entries = list_trash(&root);
        assert_eq!(entries.len(), 3, ".meta.json must stay hidden from trash listing");
        let root_e = entries.iter().find(|e| e.original_rel == "2026 - Plan.md").expect("root dash entry");
        let sub_e = entries.iter().find(|e| e.original_rel == "Sub/2026 - Plan.md").expect("nested dash entry");
        let deep_e = entries.iter().find(|e| e.original_rel == "Folder/Sub/Note.md").expect("deep nested entry");

        // Restore root dash note -> stays at root, never creates `2026/Plan.md`.
        let ev1 = restore_from_trash(&root, &root_e.name, &mut trash).unwrap();
        assert!(matches!(ev1, TrashEvent::Restored { ref original_rel, .. } if original_rel == "2026 - Plan.md"));
        assert!(root.join("2026 - Plan.md").is_file());
        assert!(!root.join("2026/Plan.md").exists());

        // Restore nested dash note with collision -> `Sub/2026 - Plan 2.md`.
        std::fs::write(root.join("Sub/2026 - Plan.md"), "# Occupied\n").unwrap();
        let ev2 = restore_from_trash(&root, &sub_e.name, &mut trash).unwrap();
        match ev2 {
            TrashEvent::Restored { original_rel, files, .. } => {
                assert_eq!(original_rel, "Sub/2026 - Plan.md");
                assert_eq!(files[0].0, "Sub/2026 - Plan 2.md");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(root.join("Sub/2026 - Plan 2.md")).unwrap(), "# Sub Plan\n");

        // Restore deep nested note even after its parent folder was removed.
        std::fs::remove_dir_all(root.join("Folder")).unwrap();
        restore_from_trash(&root, &deep_e.name, &mut trash).unwrap();
        assert_eq!(std::fs::read_to_string(root.join("Folder/Sub/Note.md")).unwrap(), "# Deep Note\n");
        assert!(!root.join(".trash/.meta.json").exists(), "empty manifest is removed");

        // Legacy entry without manifest: keep root ' - ' intact if prefix folder does not exist.
        std::fs::write(root.join(".trash/2026 - Legacy.md"), "# Legacy\n").unwrap();
        let legacy = list_trash(&root);
        assert_eq!(legacy[0].original_rel, "2026 - Legacy.md");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_restore_clears_stale_undo_toast() {
        let root = std::env::temp_dir().join(format!("quire-trash-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("2026 - Plan.md"), "# Plan\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(root.clone(), tx);
            s.open_file("2026 - Plan.md", None, tx);
            s.delete_note();
            assert!(s.deleted.is_some(), "delete sets 6s Undo toast");
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        rt.update(|s, tx| {
            s.poll(1.0, tx);
            let trash_name = s.trash_entries.first().expect("entry in trash").name.clone();
            s.restore_trash(&trash_name, tx);
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        rt.update(|s, tx| {
            s.poll(1.5, tx);
            assert!(s.deleted.is_none(), "Trash restore must clear stale Undo toast");
            assert!(s.files().contains_key("2026 - Plan.md"));
        });
        assert!(root.join("2026 - Plan.md").is_file());
        assert!(!root.join("2026/Plan.md").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nav_back_and_forward_history() {
        let root = std::env::temp_dir().join(format!("quire-nav-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Alpha.md"), "# Alpha\n").unwrap();
        std::fs::write(root.join("Beta.md"), "# Beta\n").unwrap();
        std::fs::write(root.join("Gamma.md"), "# Gamma\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(root.clone(), tx);
            s.open_file("Alpha.md", None, tx);
            s.open_file("Beta.md", None, tx);
            s.open_file("Gamma.md", None, tx);
            assert_eq!(s.active.as_deref(), Some("Gamma.md"));
            assert_eq!(s.nav_back, vec!["Alpha.md", "Beta.md"]);
            assert!(s.nav_fwd.is_empty());

            s.nav_back(tx);
            assert_eq!(s.active.as_deref(), Some("Beta.md"));
            assert_eq!(s.nav_fwd, vec!["Gamma.md"]);

            s.nav_back(tx);
            assert_eq!(s.active.as_deref(), Some("Alpha.md"));
            assert_eq!(s.nav_fwd, vec!["Gamma.md", "Beta.md"]);

            s.nav_forward(tx);
            assert_eq!(s.active.as_deref(), Some("Beta.md"));

            // Navigating to a fresh note clears forward history.
            s.open_file("Alpha.md", None, tx);
            assert!(s.nav_fwd.is_empty());
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trash_per_item_delete_and_multi_restore_stays_open() {
        let root = std::env::temp_dir().join(format!("quire-trash-c45-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("One.md"), "# First Heading\nBody preview line.\n").unwrap();
        std::fs::write(root.join("Two.md"), "# Second Heading\nAnother preview.\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(root.clone(), tx);
            s.open_file("One.md", None, tx);
            s.delete_note();
            s.open_file("Two.md", None, tx);
            s.delete_note();
            s.open_trash(tx);
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        rt.update(|s, tx| {
            s.poll(1.0, tx);
            assert_eq!(s.trash_entries.len(), 2);
            let one = s.trash_entries.iter().find(|e| e.original_rel == "One.md").expect("One in trash");
            assert!(one.snippet.contains("First Heading"), "snippet: {:?}", one.snippet);
            assert!(one.deleted_at > 0);
            let one_name = one.name.clone();
            s.restore_trash(&one_name, tx);
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        rt.update(|s, tx| {
            s.poll(1.5, tx);
            assert!(s.files().contains_key("One.md"));
            assert_eq!(s.trash_entries.len(), 1);
            assert!(matches!(s.layer(), Some(crate::layer::Layer::Trash(_))), "modal stays open while items remain");

            // 2-click per-item permanent delete of Two.md.
            let two_name = s.trash_entries[0].name.clone();
            s.delete_trash_item_press(&two_name, tx);
            assert!(matches!(s.layer(), Some(crate::layer::Layer::Trash(m)) if m.armed_delete.as_deref() == Some(two_name.as_str())));
            s.delete_trash_item_press(&two_name, tx);
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        rt.update(|s, tx| {
            s.poll(2.0, tx);
            assert!(s.trash_entries.is_empty());
        });
        assert!(!root.join(".trash/.meta.json").exists(), "manifest cleaned up after last item deleted");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn keep_conflict_version_supports_full_undo_recovery() {
        let root = std::env::temp_dir().join(format!("quire-conflict-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Beta.md"), "# Original Beta\nOriginal body.\n").unwrap();
        std::fs::write(root.join("Beta (conflict Laptop).md"), "# Laptop Beta\nConflict body.\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(root.clone(), tx);
            s.open_file("Beta (conflict Laptop).md", None, tx);
            s.keep_conflict_version("Beta (conflict Laptop).md", "Beta.md", tx);
            assert_eq!(s.note_text("Beta.md"), Some("# Laptop Beta\nConflict body.\n"));
            assert!(!s.files().contains_key("Beta (conflict Laptop).md"));
            assert!(s.deleted.is_some(), "Undo toast must remain active after Keep this version");
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        assert_eq!(std::fs::read_to_string(root.join("Beta.md")).unwrap(), "# Laptop Beta\nConflict body.\n");
        assert!(!root.join("Beta (conflict Laptop).md").exists());

        rt.update(|s, tx| {
            s.undo_delete(tx);
            assert_eq!(s.note_text("Beta.md"), Some("# Original Beta\nOriginal body.\n"));
            assert_eq!(s.note_text("Beta (conflict Laptop).md"), Some("# Laptop Beta\nConflict body.\n"));
        });
        rt.state.writer.wait(std::time::Duration::from_secs(2));
        assert_eq!(std::fs::read_to_string(root.join("Beta.md")).unwrap(), "# Original Beta\nOriginal body.\n");
        assert_eq!(std::fs::read_to_string(root.join("Beta (conflict Laptop).md")).unwrap(), "# Laptop Beta\nConflict body.\n");
        let _ = std::fs::remove_dir_all(&root);
    }
}
