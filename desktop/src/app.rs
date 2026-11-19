//! Quire's state and the actions that change it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rune::{FileDialogOptions, TextEdit, Tx};

use crate::layer::Layer;
use crate::md::{self, Names};
use crate::store::{self, Persist, Session, Settings, VaultRef};
use crate::vault::{self, Op, Scope, Vault, Writer};

/// The window width from which the outline docks beside the page; below it, it opens over
/// the page (RD-40, WR-96).
pub const OUTLINE_DOCK_W: f32 = 1100.0;
/// The window width from which the sidebar docks beside the page; below it, its button opens it
/// over the page.
pub const SIDEBAR_DOCK_W: f32 = 680.0;

/// A panel that opens over the page as a drawer in a window too narrow to show it beside the
/// page. One at a time, never with the find bar, and for this session only, so a narrow window
/// never opens with the page covered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    Outline,
    Sidebar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Vault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Source,
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalMode {
    Files,
    Commands,
}

pub struct Palette {
    pub mode: PalMode,
    pub q: TextEdit,
    pub idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sec {
    Appearance,
    Editor,
    Files,
    Sync,
    Mcp,
    Keys,
}

pub const SECS: [(Sec, &str); 6] = [
    (Sec::Appearance, "Appearance"),
    (Sec::Editor, "Editor"),
    (Sec::Files, "Files & links"),
    (Sec::Sync, "Sync & backup"),
    (Sec::Mcp, "MCP"),
    (Sec::Keys, "Shortcuts"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
    Idle,
    Testing { msg: String },
    Error { msg: String, missing: Vec<String> },
    Ok { msg: String },
}

/// Where to put the caret after opening a note.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sel {
    pub start: usize,
    pub end: usize,
    pub line: Option<usize>,
    pub force_source: bool,
}

/// A tab of the Open shelf: a note of the open folder, by its path, or in Note mode a note
/// opened as a file.
#[derive(Clone, Debug, PartialEq)]
pub enum Shelved {
    Tab(String),
    File(PathBuf),
}

impl Shelved {
    /// The tab's element key, with `|` for `/` as the tree's rows have it.
    pub fn key(&self) -> String {
        match self {
            Shelved::Tab(p) => crate::folders::row_key("tab", p),
            Shelved::File(f) => crate::folders::row_key("note", &f.to_string_lossy()),
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub persist: Persist,
    pub vault: Option<Vault>,
    pub names: Names,
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub recent: Vec<String>,
    pub expanded: BTreeSet<String>,
    pub mode: Mode,
    /// The tags list shows in the sidebar in place of the Open shelf and the vault (SB-74).
    pub show_tags: bool,
    pub query: TextEdit,
    /// The layers open over the page, bottom first (`crate::layer`).
    pub(crate) layers: Vec<crate::layer::Open>,
    /// Where focus goes back to, from a layer that closed in this change, for a layer that
    /// opens in its place (`crate::layer`).
    pub(crate) handed: Option<Option<String>>,
    /// A layer replaced another in this change: the one leaving goes at once (`ui::view`).
    pub(crate) layer_swap: bool,
    pub set_sec: Sec,
    pub focus: bool,
    pub renaming: Option<TextEdit>,
    pub creating: Option<TextEdit>,
    /// One editor per open note, so each keeps its caret and undo history.
    pub edits: HashMap<String, TextEdit>,
    /// Single-line fields in settings, by key.
    pub fields: HashMap<String, TextEdit>,
    pub reveal: BTreeSet<String>,
    pub sync_status: SyncStatus,
    pub mcp_copied: bool,
    pub notice: Option<String>,
    /// The last delete, while its toast offers Undo.
    pub deleted: Option<crate::folders::Deleted>,
    pub writer: Writer,
    pub(crate) dirty: BTreeSet<String>,
    save_at: Option<f64>,
    state_at: Option<f64>,
    /// The app clock in seconds, from `poll` (virtual in headless runs).
    pub(crate) now: std::cell::Cell<f64>,
    /// Note counts for the welcome screen's recent list, read once per visit.
    pub counts: HashMap<PathBuf, usize>,
    /// Set by a view that wants the editor focused once it exists.
    pub want_editor_focus: bool,
    /// The system appearance as the last view saw it, for commands run from keys.
    pub sys_dark: std::cell::Cell<bool>,
    /// The window height as the last view saw it.
    pub view_h: std::cell::Cell<f32>,
    /// The window width as the last view saw it.
    pub view_w: std::cell::Cell<f32>,
    pub mcp_copied_at: Option<Instant>,
    pub mcp_http: Option<crate::mcp::HttpServer>,
    pub mcp_http_error: Option<String>,
    mcp_http_failed_port: Option<u16>,
    /// The editor's caret as the last frame saw it: note, anchor, focus, text length.
    last_caret: Option<(String, usize, usize, usize)>,
    /// A key reached the editor since the last frame.
    pub key_moved: bool,
    pub(crate) waker: Option<rune::Waker>,
    pub(crate) watcher: Option<crate::watch::Watcher>,
    /// How often the watcher looks at the folder: 1.5 s, shorter in tests (VER-34).
    pub(crate) watch_every: Duration,
    /// Counts name lookups rebuilt, so cached lines redraw when a link's target appears.
    pub names_gen: u64,
    /// Set by the theme watcher when Omarchy switches themes.
    theme_changed: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub dragging: Option<crate::folders::DragNote>,
    /// While the sidebar's handle is dragged, the shelf's stored height before the drag, which
    /// a drag called off puts back.
    pub handle: Option<Option<f32>>,
    /// A tab held by the pointer on the Open shelf, by its place among the tabs it shows
    /// (SB-21). Any change to the tabs lets go of it.
    pub reorder: Option<crate::sidebar::Reorder>,
    /// A tab drag that lifted has just been let go of: the press its release sends after it,
    /// with the tab still under the pointer, is no click (SB-21). The next press down clears it.
    pub dropped: bool,
    /// The tab the shelf last scrolled to, by its element key (SB-24).
    shelf_shown: Option<String>,
    pub new_folder: Option<crate::folders::NewFolder>,
    pub sync: crate::sync::app::SyncState,
    pub images: crate::images::Images,
    /// Files are dragged over the note.
    pub drop_over: bool,
    /// The Settings switch whose target the pointer is in, by key (`widgets::switch`).
    pub hover_switch: Option<String>,
    /// Locked notes open for reading and editing, by path. The plain text lives only here and
    /// in the note's editor. The folder copy in `vault.files` stays sealed.
    pub unlocked: HashMap<String, Unlocked>,
    /// Why the last unlock failed.
    pub unlock_error: Option<String>,
    /// The reader's state: find, help, toasts, scrolling (`crate::reader`).
    pub reader: crate::reader::Reader,
    /// The writer's state: typewriter scrolling (`crate::writer_app`).
    pub wstate: crate::writer_app::WriterState,
    /// Note mode's Open list: the notes opened as files, by full path, in order. Each is a
    /// vault of its own, rooted at its folder; the one shown is `vault`, read again from disk
    /// each time it is shown.
    pub notes: Vec<PathBuf>,
    /// Note mode's notes in Open but not shown, by full path: what each keeps until it shows again.
    parked: HashMap<PathBuf, Parked>,
    /// Folder mode's sidebar setting, kept while Note mode collapses the sidebar, so Note mode
    /// never changes what Folder mode opens with.
    folder_left: Option<bool>,
}

/// A locked note while it is open.
pub struct Unlocked {
    pub key: crate::lock::Key,
    pub text: String,
}

/// A note in Note mode's Open list while another shows: its editor, with the caret and undo
/// history, and its plain text if it was unlocked. `text` is what the file held, so a note
/// changed outside comes back from disk instead, locked again if it was locked.
struct Parked {
    text: String,
    edit: Option<TextEdit>,
    unlocked: Option<Unlocked>,
}

pub fn init() -> App {
    let mut persist = store::load();
    crate::ui::styles::set_panel_font(&persist.settings.panel_font);
    let writer = Writer::start(None);
    seed_sample(&mut persist);
    let mut app = App {
        screen: Screen::Welcome,
        persist,
        vault: None,
        names: Names::default(),
        tabs: Vec::new(),
        active: None,
        recent: Vec::new(),
        expanded: BTreeSet::new(),
        mode: Mode::Source,
        show_tags: false,
        query: TextEdit::new(""),
        layers: Vec::new(),
        handed: None,
        layer_swap: false,
        set_sec: Sec::Appearance,
        focus: false,
        renaming: None,
        creating: None,
        edits: HashMap::new(),
        fields: HashMap::new(),
        reveal: BTreeSet::new(),
        sync_status: SyncStatus::Idle,
        mcp_copied: false,
        notice: None,
        deleted: None,
        writer,
        dirty: BTreeSet::new(),
        save_at: None,
        state_at: None,
        now: std::cell::Cell::new(0.0),
        counts: HashMap::new(),
        want_editor_focus: false,
        sys_dark: std::cell::Cell::new(false),
        view_h: std::cell::Cell::new(800.0),
        view_w: std::cell::Cell::new(1280.0),
        mcp_copied_at: None,
        mcp_http: None,
        mcp_http_error: None,
        mcp_http_failed_port: None,
        last_caret: None,
        key_moved: false,
        waker: None,
        watcher: None,
        watch_every: crate::watch::EVERY,
        names_gen: 0,
        theme_changed: None,
        dragging: None,
        handle: None,
        reorder: None,
        dropped: false,
        shelf_shown: None,
        new_folder: None,
        sync: Default::default(),
        images: Default::default(),
        drop_over: false,
        hover_switch: None,
        unlocked: HashMap::new(),
        notes: Vec::new(),
        parked: HashMap::new(),
        folder_left: None,
        unlock_error: None,
        reader: Default::default(),
        wstate: Default::default(),
    };
    crate::theme::reload_omarchy();
    // The remote folder used to be one value for every notes folder. Each folder has its own now.
    for c in app.persist.settings.sync.cfg.values_mut() {
        c.remove("prefix");
    }
    app.mode = if app.persist.mode == "preview" { Mode::Preview } else { Mode::Source };
    let known: Vec<PathBuf> = app.persist.vaults.iter().map(|v| v.path.clone()).collect();
    let (start, note) = match crate::cli::start_target(&known) {
        Some(crate::cli::Start::Note(file)) => {
            if !app.start_note(&file) {
                app.screen = Screen::Welcome;
            }
            (None, None)
        }
        Some(crate::cli::Start::Folder(dir, note)) => (Some(dir), note),
        None => (app.persist.last.clone(), None),
    };
    if let Some(p) = start {
        if app.load_vault(&p).is_err() {
            app.screen = Screen::Welcome;
        } else if let Some(n) = note.filter(|n| app.files().contains_key(n)) {
            if !app.tabs.contains(&n) {
                app.tabs.push(n.clone());
            }
            app.active = Some(n);
            app.ensure_edit();
        }
    }
    if app.screen == Screen::Welcome {
        app.refresh_counts();
    }
    app
}

/// The sample folder: `~/Documents/Field Notes`, as the design has it. If a folder with that
/// name already exists and is not Quire's sample, the sample goes to Quire's data folder.
pub fn sample_path() -> PathBuf {
    let docs = store::documents_dir().join(crate::sample::NAME);
    let fallback = store::dirs().map(|d| d.data().join(crate::sample::NAME));
    let p = store::load();
    if let Some(known) = p.vaults.iter().find(|v| v.name == crate::sample::NAME && (v.path == docs || Some(&v.path) == fallback.as_ref())) {
        return known.path.clone();
    }
    if !docs.exists() {
        return docs;
    }
    fallback.unwrap_or(docs)
}

fn seed_sample(p: &mut Persist) {
    if p.seeded {
        return;
    }
    let path = sample_path();
    if !path.exists() && vault::write_sample(&path).is_err() {
        return;
    }
    p.seeded = true;
    if !p.vaults.iter().any(|v| v.path == path) {
        p.vaults.push(VaultRef { name: crate::sample::NAME.into(), path: path.clone() });
    }
    let tabs: Vec<String> = crate::sample::TABS.iter().map(|s| s.to_string()).collect();
    p.sessions.insert(
        path.to_string_lossy().into_owned(),
        Session { tabs: tabs.clone(), active: Some(tabs[0].clone()), recent: tabs, expanded: vec!["Drafts".into(), "Journal".into()] },
    );
    p.last = Some(path);
    let _ = store::save(p);
}

impl App {
    /// The keymap preset the settings pick.
    pub fn preset(&self) -> crate::keymap::Preset {
        crate::keymap::Preset::from_id(&self.settings().keymap)
    }

    pub fn settings(&self) -> &Settings {
        &self.persist.settings
    }

    pub fn files(&self) -> &BTreeMap<String, String> {
        static EMPTY: BTreeMap<String, String> = BTreeMap::new();
        self.vault.as_ref().map_or(&EMPTY, |v| v.files())
    }

    /// The active note's text. An unlocked note gives its plain text, and a locked note none.
    pub fn active_text(&self) -> &str {
        let Some(a) = &self.active else { return "" };
        if let Some(u) = self.unlocked.get(a) {
            return &u.text;
        }
        self.files().get(a).map_or("", |t| if crate::lock::is_locked(t) { "" } else { t.as_str() })
    }

    /// The note at `p` is locked and not open: its text needs the password.
    pub fn note_locked(&self, p: &str) -> bool {
        !self.unlocked.contains_key(p) && self.files().get(p).is_some_and(|t| crate::lock::is_locked(t))
    }

    /// The note at `p` is locked and open now.
    pub fn note_unlocked(&self, p: &str) -> bool {
        self.unlocked.contains_key(p)
    }

    pub fn is_dark(&self, system_dark: bool) -> bool {
        self.palette(system_dark).dark
    }

    /// Light or dark mode now. On Omarchy, "Match system" follows the Omarchy theme's mode.
    pub fn dark_mode(&self, system_dark: bool) -> bool {
        match self.settings().theme.as_str() {
            "dark" => true,
            "system" => crate::theme::omarchy().map_or(system_dark, |o| o.dark),
            _ => false,
        }
    }

    /// The theme picked for dark or light mode. Without a pick it is Paper or Ink, or the
    /// Omarchy theme under "Match system" when its mode matches.
    pub fn theme_id(&self, dark: bool) -> String {
        let set = self.settings();
        if let Some(id) = if dark { &set.dark_theme } else { &set.light_theme } {
            return id.clone();
        }
        if set.theme == "system" && crate::theme::omarchy().is_some_and(|o| o.dark == dark) {
            "omarchy".into()
        } else if dark {
            "ink".into()
        } else {
            "paper".into()
        }
    }

    /// The colours to draw with: the theme of the current mode, with the accent from the settings.
    pub fn palette(&self, system_dark: bool) -> &'static crate::theme::Pal {
        let dark = self.dark_mode(system_dark);
        crate::theme::themed(&self.theme_id(dark), dark, &self.settings().accent)
    }

    pub(crate) fn rebuild_names_pub(&mut self) {
        self.rebuild_names();
    }

    pub(crate) fn mark_dirty_pub(&mut self, rel: &str) {
        self.mark_dirty(rel);
    }

    fn rebuild_names(&mut self) {
        self.names = Names::build(self.files());
        self.names_gen += 1;
    }

    pub fn folder_list(&self) -> Vec<String> {
        let Some(v) = &self.vault else { return Vec::new() };
        let mut set: BTreeSet<String> = v.folders.iter().cloned().collect();
        for p in v.files().keys() {
            let d = md::dir(p);
            if !d.is_empty() {
                set.insert(d.to_string());
            }
        }
        set.into_iter().collect()
    }

    // ---------- Persistence ----------

    pub fn persist_later(&mut self) {
        self.state_at = Some(self.now.get() + 0.4);
    }

    fn session(&self) -> Session {
        Session {
            tabs: self.tabs.clone(),
            active: self.active.clone(),
            recent: self.recent.iter().take(30).cloned().collect(),
            expanded: self.expanded.iter().cloned().collect(),
        }
    }

    fn store_session(&mut self) {
        if let Some(v) = self.vault.as_ref().filter(|v| v.scope == Scope::Folder) {
            let key = v.path.to_string_lossy().into_owned();
            let s = self.session();
            self.persist.sessions.insert(key, s);
        }
        if !self.note_mode() {
            self.persist.mode = if self.mode == Mode::Preview { "preview" } else { "source" }.into();
        }
    }

    /// What goes to `state.json`: Folder mode's sidebar setting, not Note mode's collapsed one.
    fn saved(&self) -> Persist {
        let mut p = self.persist.clone();
        if let Some(left) = self.folder_left {
            p.left_open = left;
        }
        p
    }

    /// Runs before each frame: writes due notes and settings, and picks up writer errors.
    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        self.now.set(t);
        let now = Instant::now();
        let mut changed = false;
        if self.save_at.is_some_and(|due| t >= due) {
            self.flush_notes();
        } else if let Some(due) = self.save_at {
            tx.wake_after(Duration::from_secs_f64(due - t));
        }
        if self.state_at.is_some_and(|due| t >= due) {
            self.flush_state();
        } else if let Some(due) = self.state_at {
            tx.wake_after(Duration::from_secs_f64(due - t));
        }
        if self.mcp_copied_at.is_some_and(|t| now >= t + Duration::from_millis(1600)) {
            self.mcp_copied_at = None;
            self.mcp_copied = false;
            changed = true;
        }
        changed |= self.sync_mcp_http();
        changed |= self.check_link_click(tx);
        changed |= self.apply_outside_changes();
        changed |= self.poll_sync(t, tx);
        changed |= self.images.poll();
        changed |= self.expire_reader(t);
        self.lift_over_band(tx);
        self.reveal_tab(tx);
        changed |= self.release_room(tx);
        changed |= self.expire_sheet(t);
        changed |= self.drop_stale_layers(tx);
        changed |= self.expire_deleted(t, tx);
        self.poll_writer(t, tx);
        changed |= self.search_due(t, tx);
        if self.theme_changed.as_ref().is_some_and(|f| f.swap(false, std::sync::atomic::Ordering::SeqCst)) {
            changed |= crate::theme::reload_omarchy();
        }
        if let Ok(mut e) = self.writer.errors.lock() {
            if let Some(m) = e.pop() {
                self.notice = Some(m);
                e.clear();
                changed = true;
            }
        }
        changed
    }

    pub fn set_waker(&mut self, w: rune::Waker) {
        self.images.start(w.clone());
        self.theme_changed = Some(crate::watch::watch_theme(w.clone()));
        self.waker = Some(w);
        self.watch_vault();
    }

    fn watch_vault(&mut self) {
        self.watcher = self.vault.as_ref().map(|v| {
            let only = (v.scope == Scope::Note).then(|| v.files().keys().cloned().collect());
            crate::watch::start(v.path.clone(), only, self.waker.clone(), self.watch_every)
        });
    }

    /// Takes changes other apps made to the folder. A note with edits not yet saved keeps
    /// the edits; they overwrite the outside change when they save.
    fn apply_outside_changes(&mut self) -> bool {
        let Some(w) = &self.watcher else { return false };
        let c = match w.inbox.lock() {
            Ok(mut b) => std::mem::take(&mut *b),
            Err(_) => return false,
        };
        if c.changed.is_empty() && c.removed.is_empty() {
            return false;
        }
        let mut any = false;
        for (rel, text) in c.changed {
            if self.dirty.contains(&rel) || self.files().get(&rel) == Some(&text) {
                continue;
            }
            self.note_reloaded(&rel);
            if self.unlocked.contains_key(&rel) {
                self.outside_locked_text(&rel, text);
                any = true;
                continue;
            }
            let new = !self.files().contains_key(&rel);
            if let Some(e) = self.edits.get_mut(&rel) {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            if let Some(v) = &mut self.vault {
                if new {
                    v.born.insert(rel.clone(), now_ns());
                }
                v.put(rel, text);
            }
            any = true;
        }
        for rel in c.removed {
            if self.dirty.contains(&rel) {
                continue;
            }
            if let Some(v) = &mut self.vault {
                if v.take(&rel).is_some() {
                    any = true;
                }
            }
            if self.tabs.contains(&rel) {
                self.close_tab(&rel);
            }
            self.recent.retain(|x| x != &rel);
        }
        if any {
            self.rebuild_names();
        }
        any
    }

    /// A text field gives no click event, so a click shows as a caret that moved with no key
    /// and no text change. With Ctrl (⌘ on macOS) held, that click follows the link under it.
    fn check_link_click(&mut self, tx: &mut Tx) -> bool {
        let Some(a) = self.active.clone() else {
            self.last_caret = None;
            return false;
        };
        let Some(e) = self.edits.get(&a) else { return false };
        let cur = (a.clone(), e.anchor, e.focus, e.text.len());
        let clicked = self.last_caret.as_ref().is_some_and(|l| l.0 == cur.0 && l.3 == cur.3 && (l.1, l.2) != (cur.1, cur.2))
            && !self.key_moved
            && cur.1 == cur.2
            && e.focused
            && self.mode == Mode::Source
            && tx.mods().cmd;
        self.last_caret = Some(cur);
        self.key_moved = false;
        if clicked {
            crate::ui::follow_link_at_caret(self, tx);
            return true;
        }
        false
    }

    /// Starts, updates or stops the HTTP MCP server to match the settings.
    fn sync_mcp_http(&mut self) -> bool {
        let m = &self.persist.settings.mcp;
        let want = m.enabled && m.transport == "http" && self.folder_open();
        let port: Option<u16> = m.port.parse().ok().filter(|p| *p > 0);
        if !want || port.is_none() {
            let had = self.mcp_http.take().is_some() || self.mcp_http_error.is_some();
            if want && port.is_none() {
                let msg = Some("not running · the port must be a number from 1 to 65535".to_string());
                let changed = self.mcp_http_error != msg;
                self.mcp_http_error = msg;
                return changed;
            }
            self.mcp_http_error = None;
            return had;
        }
        let port = port.unwrap_or(3917);
        let cfg = crate::mcp::HttpCfg {
            root: self.vault.as_ref().map(|v| v.path.clone()).unwrap_or_default(),
            mcp: m.clone(),
            update_links: self.persist.settings.update_links,
        };
        if let Some(srv) = &self.mcp_http {
            if srv.port == port {
                if let Ok(mut c) = srv.cfg.lock() {
                    if *c != cfg {
                        *c = cfg;
                    }
                }
                return false;
            }
            self.mcp_http = None;
        }
        if self.mcp_http_error.is_some() && self.mcp_http_failed_port == Some(port) {
            return false;
        }
        match crate::mcp::serve_http(port, cfg) {
            Ok(srv) => {
                self.mcp_http = Some(srv);
                self.mcp_http_error = None;
                self.mcp_http_failed_port = None;
            }
            Err(e) => {
                self.mcp_http_error = Some(format!("not running · port {port}: {e}"));
                self.mcp_http_failed_port = Some(port);
            }
        }
        true
    }

    pub fn flush_notes(&mut self) {
        self.save_at = None;
        let Some(v) = &self.vault else { return };
        let wrote = !self.dirty.is_empty();
        for rel in std::mem::take(&mut self.dirty) {
            if let Some(text) = v.files().get(&rel) {
                self.writer.send(Op::Write { root: v.path.clone(), rel, text: text.clone() });
            }
        }
        if wrote {
            self.sync_after_save();
        }
    }

    pub fn flush_state(&mut self) {
        self.state_at = None;
        self.store_session();
        self.writer.send(Op::SaveState(Box::new(self.saved())));
    }

    /// Saves everything now, for quitting: queued disk work finishes, then unsaved notes are
    /// written on this thread, so nothing is lost when the process ends.
    pub fn flush_all(&mut self) {
        self.save_at = None;
        self.writer.wait(Duration::from_secs(5));
        if let Some(v) = &self.vault {
            for rel in std::mem::take(&mut self.dirty) {
                if let Some(text) = v.files().get(&rel) {
                    if let Err(e) = vault::write_note(&v.path, &rel, text) {
                        eprintln!("quire: could not save {rel}: {e}");
                    }
                }
            }
        }
        self.store_session();
        if let Some(v) = self.vault.as_ref().filter(|v| v.scope == Scope::Folder) {
            self.persist.last = Some(v.path.clone());
        }
        let _ = store::save(&self.saved());
    }

    fn mark_dirty(&mut self, rel: &str) {
        self.dirty.insert(rel.to_string());
        if self.save_at.is_none() {
            self.save_at = Some(self.now.get() + 0.5);
            // `poll` writes the note and only runs before a frame, so wake it: an edit that
            // brings no frame of its own (an undo) would otherwise wait for the next one.
            if let Some(w) = &self.waker {
                w();
            }
        }
    }

    /// Changes a note's text from outside the editor (a task box, a link rewrite).
    pub fn set_text(&mut self, rel: &str, text: String) {
        if self.unlocked.contains_key(rel) {
            if let Some(e) = self.edits.get_mut(rel)
                && e.text != text
            {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            self.store_unlocked(rel, text);
            return;
        }
        if let Some(e) = self.edits.get_mut(rel) {
            if e.text != text {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
        }
        if let Some(v) = &mut self.vault {
            v.put(rel.to_string(), text);
        }
        self.mark_dirty(rel);
    }

    // ---------- Folders ----------

    pub fn refresh_counts(&mut self) {
        self.counts = self.persist.vaults.iter().map(|v| (v.path.clone(), vault::count_notes(&v.path))).collect();
    }

    pub fn load_vault(&mut self, path: &Path) -> std::io::Result<()> {
        let v = vault::load(path)?;
        self.enter(v);
        Ok(())
    }

    /// Opens `file` in Note mode, as a vault of its own rooted at its folder, and adds it to
    /// the Open list. A note new to the list opens in preview. False, with a notice, when it
    /// can't be read.
    pub fn open_note(&mut self, file: &Path) -> bool {
        let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        if self.note_path().as_ref() == Some(&file) {
            return true;
        }
        self.park();
        let parked = self.parked.remove(&file);
        if parked.is_some() {
            // Its last edits are queued for the writer: read the file after them.
            self.writer.wait(Duration::from_secs(2));
        }
        match vault::load_one(&file) {
            Ok(v) => {
                let new = !self.notes.contains(&file);
                if new {
                    self.notes.push(file);
                }
                // After `enter`, which keeps Folder mode's mode when Note mode starts.
                self.enter(v);
                if new {
                    self.mode = Mode::Preview;
                    self.want_editor_focus = false;
                }
                let rel = self.active.clone().unwrap_or_default();
                if let Some(p) = parked.filter(|p| self.files().get(&rel) == Some(&p.text)) {
                    if let Some(u) = p.unlocked {
                        self.unlocked.insert(rel.clone(), u);
                    }
                    if let Some(mut e) = p.edit {
                        e.focused = self.mode == Mode::Source;
                        self.edits.insert(rel, e);
                    }
                }
                true
            }
            Err(e) => {
                self.notice = Some(format!("Could not open {}: {e}", store::tilde(&file)));
                false
            }
        }
    }

    /// Keeps the shown note's editor and unlocked text while another note shows in Note mode.
    fn park(&mut self) {
        let Some(file) = self.note_path() else { return };
        let Some(rel) = self.active.clone() else { return };
        let Some(text) = self.files().get(&rel).cloned() else { return };
        let (edit, unlocked) = (self.edits.remove(&rel), self.unlocked.remove(&rel));
        self.parked.insert(file, Parked { text, edit, unlocked });
    }

    /// Starts the app in Note mode on `file`, in zen; leaving zen shows the page with the
    /// sidebar collapsed.
    pub fn start_note(&mut self, file: &Path) -> bool {
        self.focus = self.open_note(file);
        self.focus
    }

    /// Takes `file` off Note mode's Open list. Closing the note shown shows the one above it,
    /// or the one below when it was first (SB-20).
    pub fn close_note(&mut self, file: &Path) {
        let Some(i) = self.notes.iter().position(|n| n == file) else { return };
        let shown = self.note_path().as_deref() == Some(file);
        self.notes.remove(i);
        self.reorder = None;
        // A closed note forgets its undo history and locks again.
        self.parked.remove(file);
        if !shown {
            return;
        }
        while let Some(next) = crate::sidebar::next_active(i, self.notes.len()).map(|n| self.notes[n].clone()) {
            if self.open_note(&next) {
                // Showing the neighbour parked the closed note.
                self.parked.remove(file);
                return;
            }
            self.notes.retain(|n| *n != next);
        }
        self.flush_notes();
        self.tabs.clear();
        self.edits.clear();
        self.unlocked.clear();
        self.active = None;
        self.focus = false;
    }

    /// The full path of the note shown in Note mode.
    pub fn note_path(&self) -> Option<PathBuf> {
        let v = self.vault.as_ref().filter(|v| v.scope == Scope::Note)?;
        Some(v.path.join(self.active.as_ref()?))
    }

    /// Ctrl+O: the quick switcher, or in Note mode the system's file picker.
    pub fn quick_open(&mut self, tx: &mut Tx) {
        if !self.note_mode() {
            return self.open_palette(PalMode::Files, tx);
        }
        let directory = self.vault.as_ref().map(|v| v.path.clone());
        let options = FileDialogOptions { title: "Open a note".into(), directory, ..Default::default() }.filter("Markdown", &["md", "markdown"]);
        tx.open_file(options, |s: &mut App, result, tx| match result {
            Ok(Some(path)) => {
                // A new note opens in preview, where the editor leaves with the keyboard.
                s.open_note(&path);
                s.keys_to_note(tx);
            }
            Ok(None) => {}
            Err(e) => s.notice = Some(format!("Could not show the file picker: {e}")),
        });
    }

    /// Makes `v` the open vault. A folder takes the session it was left with and joins the list;
    /// a note in Note mode is its only tab, with the sidebar collapsed.
    fn enter(&mut self, v: Vault) {
        self.flush_notes();
        self.store_session();
        match v.scope {
            Scope::Folder => {
                self.notes.clear();
                self.parked.clear();
                if let Some(left) = self.folder_left.take() {
                    self.persist.left_open = left;
                    self.mode = if self.persist.mode == "preview" { Mode::Preview } else { Mode::Source };
                }
                self.focus = false;
            }
            Scope::Note => {
                if self.folder_left.is_none() {
                    self.folder_left = Some(self.persist.left_open);
                    self.persist.left_open = false;
                }
            }
        }
        let path = v.path.clone();
        let ses = match v.scope {
            Scope::Folder => self.persist.sessions.get(&*path.to_string_lossy()).cloned().unwrap_or_default(),
            Scope::Note => {
                let only: Vec<String> = v.files().keys().cloned().collect();
                Session { active: only.first().cloned(), tabs: only, ..Default::default() }
            }
        };
        self.tabs = ses.tabs.into_iter().filter(|p| v.files().contains_key(p)).collect();
        self.active = ses.active.filter(|a| v.files().contains_key(a)).or_else(|| self.tabs.first().cloned());
        self.recent = ses.recent.into_iter().filter(|p| v.files().contains_key(p)).collect();
        if self.recent.is_empty() {
            self.recent = self.tabs.clone();
        }
        self.expanded = ses.expanded.into_iter().collect();
        if v.scope == Scope::Folder {
            if !self.persist.vaults.iter().any(|r| r.path == path) {
                self.persist.vaults.push(VaultRef { name: v.name.clone(), path: path.clone() });
            }
            self.persist.last = Some(path);
        }
        self.vault = Some(v);
        self.edits.clear();
        // An Undo offer puts a note back into the folder it left, so it ends with that folder.
        self.deleted = None;
        // Paths name notes only within one vault: a note unlocked in the last one locks again.
        self.unlocked.clear();
        self.rebuild_names();
        if self.waker.is_some() {
            self.watch_vault();
        }
        self.screen = Screen::Vault;
        self.creating = None;
        self.query = TextEdit::new("");
        self.show_tags = false;
        self.layers.clear();
        self.renaming = None;
        self.ensure_edit();
        if self.mode == Mode::Source {
            if let Some(e) = self.cur_edit_opt() {
                e.focused = true;
            }
        }
        self.want_editor_focus = self.mode == Mode::Source;
        self.persist_later();
        self.sync_auto(0.5);
    }

    /// A folder from the list is open, not a note on its own.
    pub fn folder_open(&self) -> bool {
        self.vault.as_ref().is_some_and(|v| v.scope == Scope::Folder)
    }

    /// Note mode: notes opened as files, each in a vault of its own.
    pub fn note_mode(&self) -> bool {
        self.vault.as_ref().is_some_and(|v| v.scope == Scope::Note)
    }

    /// Note mode has no folder to add notes or folders to, rename, move or delete them in:
    /// says so and returns true.
    pub fn refuse_in_note_mode(&mut self) -> bool {
        let note = self.note_mode();
        if note {
            self.notice = Some("This note is open as a file. Open its folder to add, rename, move or delete notes.".into());
        }
        note
    }

    pub fn open_vault(&mut self, path: PathBuf, tx: &mut Tx) {
        match self.load_vault(&path) {
            Ok(()) => {
                if let Some(v) = &self.vault {
                    tx.set_title(format!("{} · Quire", v.name));
                }
                self.focus_editor(tx);
            }
            Err(e) => self.notice = Some(format!("Could not open {}: {e}", store::tilde(&path))),
        }
    }

    pub fn open_folder_dialog(&mut self, tx: &mut Tx) {
        let options = FileDialogOptions { title: "Open a folder of notes".into(), directory: Some(store::documents_dir()), ..Default::default() };
        tx.open_folder(options, |s: &mut App, result, tx| match result {
            Ok(Some(path)) => s.open_vault(path, tx),
            Ok(None) => {}
            Err(e) => s.notice = Some(format!("Could not show the folder picker: {e}")),
        });
    }

    pub fn go_welcome(&mut self, tx: &mut Tx) {
        self.flush_notes();
        self.store_session();
        self.screen = Screen::Welcome;
        self.layers.clear();
        self.focus = false;
        self.renaming = None;
        if self.folder_open() {
            self.persist.last = None;
        }
        self.watcher = None;
        self.refresh_counts();
        self.persist_later();
        tx.set_title("Quire");
    }

    pub fn new_vault_path(&self) -> PathBuf {
        let name = self.creating.as_ref().map_or("", |e| e.text.trim());
        let name = md::clean_name(if name.is_empty() { "Untitled notebook" } else { name });
        store::documents_dir().join(name)
    }

    pub fn create_vault(&mut self, tx: &mut Tx) {
        let path = self.new_vault_path();
        if let Err(e) = std::fs::create_dir_all(&path) {
            self.notice = Some(format!("Could not create {}: {e}", store::tilde(&path)));
            return;
        }
        self.open_vault(path, tx);
    }

    pub fn forget_vault(&mut self, path: &Path) {
        self.persist.vaults.retain(|v| v.path != path);
        self.persist_later();
    }

    pub fn reset_sample(&mut self, tx: &mut Tx) {
        let path = sample_path();
        if let Err(e) = vault::write_sample(&path) {
            self.notice = Some(format!("Could not restore the sample notes: {e}"));
            return;
        }
        let tabs: Vec<String> = crate::sample::TABS.iter().map(|s| s.to_string()).collect();
        self.persist.sessions.insert(
            path.to_string_lossy().into_owned(),
            Session { tabs: tabs.clone(), active: Some(tabs[0].clone()), recent: tabs, expanded: vec!["Drafts".into(), "Journal".into()] },
        );
        // Load without storing the current session over the one just written.
        self.vault = None;
        self.open_vault(path, tx);
        if let Some(e) = self.cur_edit_opt() {
            e.set_selection(0, 0);
        }
    }

    // ---------- Notes ----------

    pub fn ensure_edit(&mut self) {
        if let Some(a) = self.active.clone() {
            let text = match self.unlocked.get(&a) {
                Some(u) => u.text.clone(),
                None => self.files().get(&a).filter(|t| !crate::lock::is_locked(t)).cloned().unwrap_or_default(),
            };
            self.edits.entry(a).or_insert_with(|| {
                let mut e = TextEdit::new(text);
                e.set_selection(0, 0);
                e
            });
        }
    }

    pub fn cur_edit_opt(&mut self) -> Option<&mut TextEdit> {
        let a = self.active.clone()?;
        self.edits.get_mut(&a)
    }

    /// The active note's editor. The view only builds the editor while a note is active.
    pub fn cur_edit(&mut self) -> &mut TextEdit {
        self.ensure_edit();
        let key = self.active.clone().unwrap_or_default();
        self.edits.entry(key).or_insert_with(|| TextEdit::new(""))
    }

    /// The editor's text changed: keep the folder copy in step and save soon.
    pub fn after_edit(&mut self) {
        let Some(a) = self.active.clone() else { return };
        let Some(text) = self.edits.get(&a).map(|e| e.text.clone()) else { return };
        if self.unlocked.contains_key(&a) {
            self.store_unlocked(&a, text);
            return;
        }
        if self.note_locked(&a) {
            return;
        }
        if let Some(v) = &mut self.vault {
            if v.files().get(&a) != Some(&text) {
                v.put(a.clone(), text);
                self.mark_dirty(&a);
            }
        }
    }

    pub fn focus_editor(&mut self, tx: &mut Tx) {
        if self.active.is_some() && self.mode == Mode::Source {
            tx.focus("editor", false);
            self.want_editor_focus = true;
        }
    }

    pub fn open_file(&mut self, path: &str, sel: Option<Sel>, tx: &mut Tx) {
        if self.files().get(path).is_none() {
            return;
        }
        if !self.tabs.iter().any(|t| t == path) {
            self.tabs.push(path.to_string());
            self.reorder = None;
        }
        let changed = self.active.as_deref() != Some(path);
        let from = self.active.replace(path.to_string());
        let mut d = md::dir(path).to_string();
        while !d.is_empty() {
            self.expanded.insert(d.clone());
            d = md::dir(&d).to_string();
        }
        self.recent.retain(|x| x != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(30);
        self.renaming = None;
        if sel.is_some_and(|s| s.force_source) {
            self.mode = Mode::Source;
        }
        self.ensure_edit();
        if let Some(s) = sel {
            if let Some(e) = self.cur_edit_opt() {
                let n = e.text.len();
                e.set_selection(s.start.min(n), s.end.min(n));
                e.break_history_group();
            }
            if let Some(line) = s.line.filter(|_| self.mode == Mode::Source) {
                self.place_line(line, tx);
            }
        } else if changed {
            self.switch_reading(from.as_deref(), path, tx);
        }
        self.focus_editor(tx);
        self.persist_later();
    }

    pub fn close_tab(&mut self, p: &str) {
        if let Some(file) = self.note_path().filter(|_| self.active.as_deref() == Some(p)) {
            return self.close_note(&file);
        }
        let Some(i) = self.tabs.iter().position(|x| x == p) else { return };
        self.tabs.remove(i);
        self.reorder = None;
        self.edits.remove(p);
        // A closed note locks again.
        self.unlocked.remove(p);
        if self.active.as_deref() == Some(p) {
            self.active = crate::sidebar::next_active(i, self.tabs.len()).map(|n| self.tabs[n].clone());
            if self.active.is_none() {
                self.focus = false;
            }
            // The note that shows now has its editor, so its text shows at once.
            self.ensure_edit();
        }
        self.renaming = None;
        self.persist_later();
    }

    /// The Open shelf's tabs in their order: in Folder mode the tabs whose notes exist, in Note
    /// mode the notes opened as files.
    pub fn shelf(&self) -> Vec<Shelved> {
        if self.note_mode() {
            return self.notes.iter().cloned().map(Shelved::File).collect();
        }
        self.tabs.iter().filter(|t| self.files().contains_key(*t)).cloned().map(Shelved::Tab).collect()
    }

    /// The shelf's active tab.
    pub fn shelved(&self) -> Option<Shelved> {
        if self.note_mode() {
            return self.note_path().map(Shelved::File);
        }
        self.active.clone().filter(|a| self.tabs.contains(a)).map(Shelved::Tab)
    }

    /// Closes a tab of the shelf (SB-20).
    pub fn close_shelved(&mut self, t: &Shelved) {
        match t {
            Shelved::Tab(p) => self.close_tab(p),
            Shelved::File(f) => self.close_note(f),
        }
    }

    /// Moves the shelf's tab at `from` to `to`, both places among the tabs it shows (SB-21,
    /// SB-85). The order is kept with the folder's session.
    pub fn move_shelved(&mut self, from: usize, to: usize) {
        let shelf = self.shelf();
        let (Some(a), Some(b)) = (shelf.get(from), shelf.get(to)) else { return };
        match (a, b) {
            (Shelved::Tab(a), Shelved::Tab(b)) => {
                let at = |p: &str| self.tabs.iter().position(|t| t == p);
                if let (Some(i), Some(j)) = (at(a), at(b)) {
                    crate::sidebar::move_tab(&mut self.tabs, i, j);
                }
            }
            _ => crate::sidebar::move_tab(&mut self.notes, from, to),
        }
        self.reorder = None;
        self.persist_later();
    }

    /// Delete or Backspace on a focused tab: closes it, and the keyboard goes to the tab that
    /// takes its place by SB-20's rule, or to the note with no tab left (SB-86).
    pub fn close_focused_tab(&mut self, t: &Shelved, tx: &mut Tx) {
        let Some(i) = self.shelf().iter().position(|x| x == t) else { return };
        self.close_shelved(t);
        let shelf = self.shelf();
        match crate::sidebar::next_active(i, shelf.len()) {
            Some(n) => tx.focus(shelf[n].key(), true),
            None => self.keys_to_note(tx),
        }
    }

    /// Before each frame: when the active tab changed, by a click, a key or a note opened
    /// elsewhere, the shelf scrolls just far enough to show it (SB-24).
    fn reveal_tab(&mut self, tx: &mut Tx) {
        let key = self.shelved().map(|t| t.key());
        if key != self.shelf_shown {
            if let Some(k) = &key {
                tx.scroll_into_view_in("shelf-scroll", k.clone());
            }
            self.shelf_shown = key;
        }
    }

    pub fn toggle_folder(&mut self, p: &str) {
        if !self.expanded.remove(p) {
            self.expanded.insert(p.to_string());
        }
        self.persist_later();
    }

    pub(crate) fn unique_path(&self, folder: &str, base: &str) -> String {
        let files = self.files();
        let mut p = md::join(folder, &format!("{base}.md"));
        let mut n = 2;
        while files.contains_key(&p) || self.vault.as_ref().is_some_and(|v| v.path.join(&p).exists()) {
            p = md::join(folder, &format!("{base} {n}.md"));
            n += 1;
        }
        p
    }

    pub fn new_note(&mut self, name: Option<&str>, tx: &mut Tx) {
        self.new_note_in(None, name, tx);
    }

    pub fn delete_note(&mut self) {
        if self.refuse_in_note_mode() {
            return;
        }
        let Some(p) = self.active.clone() else { return };
        // Unsaved text goes to disk first, so the trash and Undo have it.
        self.flush_notes();
        self.deleted = Some(self.record_delete(&p, false));
        self.unlocked.remove(&p);
        let Some(v) = &mut self.vault else { return };
        v.take(&p);
        v.born.remove(&p);
        let root = v.path.clone();
        self.writer.send(Op::Delete { root, rel: p.clone() });
        self.recent.retain(|x| x != &p);
        self.close_tab(&p);
        self.focus = false;
        self.rebuild_names();
        self.sync_after_save();
    }

    pub fn open_today(&mut self, tx: &mut Tx) {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let now = rune::LocalTime::at(ms);
        let p = format!("Journal/{:04}-{:02}-{:02}.md", now.year, now.month, now.day);
        let existing = self.files().get(&p).cloned();
        let text = match existing {
            Some(t) => t,
            None => {
                let c = format!("# {}\n\n#journal\n\n", long_date(now.year, now.month, now.day));
                if let Some(v) = &mut self.vault {
                    v.put(p.clone(), c.clone());
                    v.born.insert(p.clone(), now_ns());
                    if !v.folders.iter().any(|f| f == "Journal") {
                        v.folders.push("Journal".into());
                        v.folders.sort();
                    }
                }
                self.rebuild_names();
                self.mark_dirty(&p);
                c
            }
        };
        let end = text.len();
        self.open_file(&p, Some(Sel { start: end, end, line: None, force_source: true }), tx);
    }

    /// A wiki link: opens the note it names, or creates it. A `#Heading` after the name
    /// travels to that heading in the reader once the note is shown (RD-102, RD-04).
    pub fn open_link(&mut self, name: &str, tx: &mut Tx) {
        let (t, frag) = md::wiki_parts(name);
        if t.is_empty() {
            if let Some(f) = frag {
                self.go_anchor(&f, tx);
            }
            return;
        }
        match self.names.resolve(&t).map(str::to_string) {
            Some(p) => {
                self.open_file(&p, None, tx);
                if let Some(f) = frag.filter(|_| self.mode == Mode::Preview) {
                    self.go_anchor(&f, tx);
                }
            }
            None => self.new_note(Some(&t), tx),
        }
    }

    /// Ticks (`done`) or clears the task box on 0-based `line`.
    pub fn toggle_task(&mut self, line: usize, done: bool) {
        let Some(a) = self.active.clone() else { return };
        if let Some(t) = crate::blocks::toggle_task(self.active_text(), line, done) {
            self.set_text(&a, t);
        }
    }

    pub fn start_rename(&mut self, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        let Some(a) = &self.active else { return };
        let mut e = TextEdit::new(md::base(a));
        let n = e.text.len();
        e.set_selection(0, n);
        e.focused = true;
        self.renaming = Some(e);
        self.field_opened(crate::layer::Field::NoteName);
        tx.focus("rename", false);
    }

    pub fn cancel_rename(&mut self, tx: &mut Tx) {
        self.renaming = None;
        self.focus_editor(tx);
    }

    pub fn commit_rename(&mut self, tx: &mut Tx) {
        let Some(e) = self.renaming.take() else { return };
        let Some(old) = self.active.clone() else { return };
        let val = md::clean_name(&e.text);
        let np = md::join(md::dir(&old), &format!("{val}.md"));
        let exists = self.files().contains_key(&np);
        if val.is_empty() || np == old || exists {
            self.focus_editor(tx);
            return;
        }
        // Pending writes go first, so the file to rename exists on disk.
        self.flush_notes();
        let old_base = md::base(&old).to_string();
        let update = self.settings().update_links;
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let text = v.take(&old).unwrap_or_default();
        v.put(np.clone(), text);
        if let Some(b) = v.born.remove(&old) {
            v.born.insert(np.clone(), b);
        }
        let mut rewritten = Vec::new();
        if update {
            let changed: Vec<(String, String)> = v.files().iter().filter_map(|(p, t)| {
                let nt = md::rewrite_links(t, &old_base, &val);
                (nt != *t).then(|| (p.clone(), nt))
            }).collect();
            for (p, nt) in changed {
                rewritten.push(p.clone());
                v.put(p, nt);
            }
        }
        // The rename goes to disk first, then any rewritten notes, the renamed one included.
        self.writer.send(Op::Rename { root, from: old.clone(), to: np.clone() });
        if let Some(u) = self.unlocked.remove(&old) {
            self.unlocked.insert(np.clone(), u);
        }
        if let Some(ed) = self.edits.remove(&old) {
            self.edits.insert(np.clone(), ed);
        }
        for p in rewritten {
            let t = self.files().get(&p).cloned().unwrap_or_default();
            self.set_text(&p, t);
        }
        for t in self.tabs.iter_mut().chain(self.recent.iter_mut()) {
            if *t == old {
                *t = np.clone();
            }
        }
        self.active = Some(np);
        self.rebuild_names();
        self.focus_editor(tx);
        self.persist_later();
        self.sync_after_save();
    }

    // ---------- Modes and panels ----------

    pub fn toggle_mode(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        self.switch_mode(tx);
        self.persist_later();
    }


    /// The zoom keys: a step up or down omarky's ladder, or `None` back to 14 px.
    pub fn zoom(&mut self, up: Option<bool>) {
        let size = self.settings().size;
        self.settings_mut().size = up.map_or(crate::store::SIZE_DEFAULT, |up| crate::store::zoom_step(size, up));
    }

    pub fn toggle_focus(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        self.focus = !self.focus;
        self.focus_editor(tx);
    }

    /// The sidebar button and keys: the docked sidebar's setting, or in a narrow window the
    /// sidebar's drawer.
    pub fn toggle_left(&mut self, tx: &mut Tx) {
        if self.view_w.get() >= SIDEBAR_DOCK_W {
            self.persist.left_open = !self.persist.left_open;
            self.persist_later();
        } else {
            self.toggle_drawer(Drawer::Sidebar, tx);
        }
    }

    /// Whether the sidebar button's sidebar is open, docked or as a drawer.
    pub fn sidebar_open(&self) -> bool {
        if self.view_w.get() >= SIDEBAR_DOCK_W { self.persist.left_open } else { self.drawer() == Some(Drawer::Sidebar) }
    }

    /// Shows the sidebar for search, a tag or a new folder: docked, or as the drawer.
    pub(crate) fn show_left(&mut self, tx: &mut Tx) {
        self.focus = false;
        if self.view_w.get() >= SIDEBAR_DOCK_W {
            self.persist.left_open = true;
        } else if self.drawer() != Some(Drawer::Sidebar) {
            self.open_layer(Layer::Drawer(Drawer::Sidebar), tx);
        }
    }

    /// The outline button and keys: the docked outline's setting, or in a narrow window the
    /// outline's drawer.
    pub fn toggle_right(&mut self, tx: &mut Tx) {
        if self.view_w.get() >= OUTLINE_DOCK_W {
            self.persist.right_open = !self.persist.right_open;
            self.persist_later();
        } else {
            self.toggle_drawer(Drawer::Outline, tx);
        }
    }

    /// Whether the outline button's outline is open, docked or as a drawer.
    pub fn outline_open(&self) -> bool {
        if self.view_w.get() >= OUTLINE_DOCK_W { self.persist.right_open } else { self.drawer() == Some(Drawer::Outline) }
    }

    /// Opens drawer `d`, or closes it when it is open.
    pub fn toggle_drawer(&mut self, d: Drawer, tx: &mut Tx) {
        if self.drawer() == Some(d) {
            self.close_layers(tx);
        } else {
            self.open_layer(Layer::Drawer(d), tx);
        }
    }

    /// Whether drawer `d` has its reason to be: zen mode is off and the window is too narrow to
    /// show its panel beside the page. The outline's also needs a note. A drawer that loses it
    /// closes (`crate::layer`).
    pub(crate) fn drawer_fits(&self, d: Drawer) -> bool {
        let narrow = match d {
            Drawer::Outline => self.view_w.get() < OUTLINE_DOCK_W && self.active.is_some(),
            Drawer::Sidebar => self.view_w.get() < SIDEBAR_DOCK_W,
        };
        narrow && !self.focus && self.screen == Screen::Vault
    }

    /// Settings' key: opens Settings, or closes it when it is open.
    pub fn toggle_settings(&mut self, tx: &mut Tx) {
        if self.layer_is(crate::layer::Kind::Settings) {
            self.close_layer(tx);
        } else {
            self.open_settings(tx);
        }
    }

    /// A toolbar action on the active note's selection, as one undo step.
    pub fn format(&mut self, f: crate::format::Fmt, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let e = self.target_edit();
        let ed = crate::format::apply(&e.text, e.anchor, e.focus, f);
        e.undo_group(|e| {
            e.set_selection(ed.start, ed.end);
            e.insert(&ed.insert);
            e.set_selection(ed.sel.0, ed.sel.1);
        });
        self.target_changed(tx);
    }

    /// Saves everything, then closes the window.
    pub fn quit(&mut self, tx: &mut Tx) {
        self.flush_all();
        tx.close_window();
    }

    pub fn open_settings(&mut self, tx: &mut Tx) {
        self.open_layer(crate::layer::Layer::Settings, tx);
    }

    pub fn open_search(&mut self, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        self.show_left(tx);
        let n = self.query.text.len();
        self.query.set_selection(0, n);
        tx.focus("search", false);
    }

    pub fn search_tag(&mut self, tag: &str, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        self.show_left(tx);
        self.show_tags = false;
        self.query = TextEdit::new(format!("#{tag}"));
        let n = self.query.text.len();
        self.query.set_selection(n, n);
    }

    pub fn open_palette(&mut self, mode: PalMode, tx: &mut Tx) {
        self.open_layer(crate::layer::Layer::Palette(Palette { mode, q: TextEdit::new(""), idx: 0 }), tx);
        tx.focus("pal-input", false);
    }

    /// Esc with the keyboard on a stop or in a field of the page, or anywhere in a drawer: a
    /// showing notice goes first, as it does from the note, and the keyboard stays where it was;
    /// the next Esc closes the drawer. Any other open layer (the palette, Settings, help, the
    /// lock dialog, the edit sheet, a menu, a menu over a drawer) takes Esc before the notice,
    /// so under one this does nothing.
    pub(crate) fn esc_takes_notice(&mut self) -> bool {
        matches!(self.layer().map(Layer::kind), None | Some(crate::layer::Kind::Drawer(_))) && self.notice.take().is_some()
    }

    /// Esc on the page, with no layer open (an open layer takes Esc first, `keys::on_key`):
    /// the notice goes, then new folder, rename and the create prompt close, then zen mode ends.
    /// The keyboard stays where it was.
    pub fn escape(&mut self, tx: &mut Tx) -> bool {
        if self.notice.take().is_some() {
            return true;
        }
        if self.new_folder.is_some() {
            self.new_folder = None;
        } else if self.renaming.is_some() {
            self.cancel_rename(tx);
        } else if self.creating.is_some() {
            self.creating = None;
        } else if self.focus {
            self.focus = false;
        } else {
            return false;
        }
        true
    }

    /// Moves the caret, or the preview, to line `line`.
    pub fn jump(&mut self, line: usize, tx: &mut Tx) {
        // From the outline's drawer: the page moves, and the drawer gives it back.
        self.close_layers(tx);
        if self.mode == Mode::Source {
            self.source_jump(line, tx);
        } else {
            self.jump_block(line, tx);
        }
    }

    pub fn set_theme(&mut self, theme: &str) {
        self.persist.settings.theme = theme.into();
        self.persist_later();
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        self.persist_later();
        &mut self.persist.settings
    }

    /// A settings text field, made on first use with `init`.
    pub fn field(&mut self, key: &str, init: impl FnOnce() -> String) -> &mut TextEdit {
        self.fields.entry(key.to_string()).or_insert_with(|| TextEdit::new(init()))
    }
}

pub(crate) fn now_ns() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

/// "Thursday 12 November", as `toLocaleDateString('en-GB')` writes it.
fn long_date(year: i32, month: u32, day: u32) -> String {
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    // Sakamoto's method: 0 is Sunday.
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let w = (y + y / 4 - y / 100 + y / 400 + T[(month as usize + 11) % 12] + day as i32).rem_euclid(7);
    format!("{} {} {}", DAYS[w as usize], day, MONTHS[(month as usize + 11) % 12])
}
