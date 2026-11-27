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
    /// The note as a mind map (decision 0014).
    Map,
    /// The note as a kanban board (decision 0015).
    Board,
}

impl Mode {
    /// The mode `state.json` names; anything unknown is the source (MAP-064, BRD-029).
    pub fn from_id(id: &str) -> Mode {
        match id {
            "preview" => Mode::Preview,
            "map" => Mode::Map,
            "board" => Mode::Board,
            _ => Mode::Source,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Mode::Source => "source",
            Mode::Preview => "preview",
            Mode::Map => "map",
            Mode::Board => "board",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalMode {
    Files,
    Commands,
    MoveFolder,
}

pub struct Palette {
    pub mode: PalMode,
    pub q: TextEdit,
    pub idx: usize,
    /// Target note path when `mode == PalMode::MoveFolder`.
    pub move_path: Option<String>,
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
    pub query: TextEdit,
    /// Search's results for `query`, kept until the query or the notes change (`results`).
    pub search: std::cell::RefCell<crate::search::Cache>,
    /// The vault's tree with its folders' counts, kept until its notes or folders change.
    pub tree: std::cell::RefCell<crate::folders::Tree>,
    /// The result row that has the keyboard, by its key, as the last frame saw it: the row
    /// the list fills (SB-65).
    pub res_focus: Option<String>,
    /// The layers open over the page, bottom first (`crate::layer`).
    pub(crate) layers: Vec<crate::layer::Open>,
    /// Where focus goes back to, from a layer that closed in this change, for a layer that
    /// opens in its place (`crate::layer`).
    pub(crate) handed: Option<Option<String>>,
    /// A layer replaced another in this change: the one leaving goes at once (`ui::view`).
    pub(crate) layer_swap: bool,
    pub set_sec: Sec,
    /// Incremented whenever the active settings section changes so the body slides vertically.
    pub(crate) set_gen: u64,
    /// Vertical slide direction for settings section switches (`+1` down, `-1` up, `0` none).
    pub(crate) set_slide: i8,
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
    /// Optional note path to open directly from the active notice (e.g. a sync conflict copy).
    pub notice_open: Option<String>,
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
    /// Most recent MCP HTTP tool/client activity `(label, unix_ms)` and last seen call count.
    pub mcp_last_activity: Option<(String, i64)>,
    mcp_seen_calls: u64,
    /// The editor's caret as the last frame saw it: note, anchor, focus, text length.
    last_caret: Option<(String, usize, usize, usize)>,
    /// A key reached the editor or window since the last frame.
    pub key_moved: bool,
    /// Rune accessed the editor's `TextEdit` since the last frame (e.g. a pointer click on the
    /// field, even when the caret stayed at the same offset).
    pub editor_accessed: bool,
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
    /// Active note path waiting to be scrolled into view in `side-scroll` once the sidebar shows.
    pub(crate) tree_reveal: Option<String>,
    /// The tab whose outline is folded out under it. It shows only while that tab is active,
    /// and goes once another is (SB-25, SB-26). Not kept between sessions.
    pub outline_out: Option<Shelved>,
    pub new_folder: Option<crate::folders::NewFolder>,
    /// A folder being renamed inline in the sidebar tree.
    pub renaming_folder: Option<crate::folders::RenameFolder>,
    pub sync: crate::sync::app::SyncState,
    pub images: crate::images::Images,
    /// Files are dragged over the note.
    pub drop_over: bool,
    /// The Settings switch whose target the pointer is in, by key (`components::switch`).
    pub hover_switch: Option<String>,
    /// Locked notes open for reading and editing, by path. The plain text lives only here and
    /// in the note's editor. The folder copy in `vault.files` stays sealed.
    pub unlocked: HashMap<String, Unlocked>,
    /// In-flight background PBKDF2 key-derivation job (`lock_app::LockJob`).
    pub lock_job: Option<crate::lock_app::LockJob>,
    /// App-clock time of the most recent user interaction, for idle auto-lock.
    pub last_activity: std::rc::Rc<std::cell::Cell<f64>>,
    /// Monotonic sequence number for `lock_job`, bumped on spawn and cancel so late results are ignored.
    pub(crate) lock_job_seq: u64,
    /// Why the last unlock failed.
    pub unlock_error: Option<String>,
    /// The reader's state: find, help, toasts, scrolling (`crate::reader`).
    pub reader: crate::reader::Reader,
    /// The writer's state: typewriter scrolling (`crate::writer_app`).
    pub wstate: crate::writer_app::WriterState,
    /// Map mode's state: each tab's camera, folds and selection (`crate::map_app`).
    pub map: crate::map_app::MapState,
    /// Board mode's state: hold/drag, layout rects, scroll offsets (`crate::board_app`).
    pub board: crate::board_app::BoardState,
    /// Note mode's Open list: the notes opened as files, by full path, in order. Each is a
    /// vault of its own, rooted at its folder; the one shown is `vault`, read again from disk
    /// each time it is shown.
    pub notes: Vec<PathBuf>,
    /// Note mode's notes in Open but not shown, by full path: what each keeps until it shows again.
    parked: HashMap<PathBuf, Parked>,
    /// Window size has been measured by at least one layout pass (`COLDPACE`).
    pub laid_out: std::cell::Cell<bool>,
    /// Idle deadline for scheduling `Tx::prepare_text(ui::prepare_cold)` after open/switch (`COLDPACE`).
    prepare_due: Option<f64>,
    /// Snapshot of note/viewport/theme inputs last pre-warmed via `Tx::prepare_text`.
    prepared_key: Option<PrepareKey>,
    /// Folder mode's sidebar setting, kept while Note mode collapses the sidebar, so Note mode
    /// never changes what Folder mode opens with.
    folder_left: Option<bool>,
    /// Monotonic revision bumped whenever the active tab changes to a different note, keying the
    /// document stage (`tab-page:{tab_gen}`) so tab switches slide cleanly from the side without
    /// triggering on mode switches, renames, or edits.
    pub(crate) tab_gen: u64,
    /// Horizontal slide direction for the latest tab switch: `+1` enters from the trailing
    /// (right) side, `-1` enters from the leading (left) side, `0` disables the slide.
    pub(crate) tab_slide: i8,
    /// True after an in-tab mode switch (`Source`/`Preview`/`Map`) so mode content plays `WR-109`
    /// `swap()` fade, and false on a tab switch so the sliding page mounts at full 100% opacity.
    pub(crate) mode_swapped: bool,
    /// User keybinding overrides loaded from `~/.config/quire/keybindings.json`.
    pub keymap: crate::keymap::Overrides,
    /// Active keybinding capture row in Settings → Shortcuts.
    pub key_edit: Option<KeyEdit>,
    /// Live search filter in Settings → Shortcuts.
    pub keys_query: TextEdit,
    /// Recently closed tabs in this folder, for `app.reopen-tab` (`Ctrl+Shift+T`).
    pub closed_tabs: Vec<String>,
    /// Back/forward navigation history within the open folder (`Alt+Left` / `Alt+Right`).
    pub nav_back: Vec<String>,
    pub nav_fwd: Vec<String>,
    nav_jumping: bool,
    /// Trashed notes and folders inside `<vault>/.trash/`, scanned on the `Writer` thread.
    pub trash_entries: Vec<crate::vault::TrashEntry>,
    /// Conflict copy path whose inline diff view is currently expanded under `conflict-banner`.
    pub conflict_diff: Option<String>,
}

/// Recording state for one action row in Settings → Shortcuts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEdit {
    pub action_id: &'static str,
    pub error: Option<String>,
}

/// Inputs that determine whether Source and Command Palette text layouts are already pre-warmed.
#[derive(Clone, PartialEq, Eq)]
struct PrepareKey {
    source: Option<(TextKey, u32, u32, String, bool, bool, bool, bool)>,
    palette: (bool, u64, bool, bool, String),
}

/// A locked note while it is open.
pub struct Unlocked {
    pub key: crate::lock::Key,
    pub text: String,
    /// Monotonic revision bumped whenever `text` is created or updated, so caches keyed on
    /// `App::active_text_key` invalidate without hashing the note's text.
    pub ver: u64,
}

impl Unlocked {
    pub fn new(key: crate::lock::Key, text: String) -> Self {
        Unlocked { key, text, ver: next_unlocked_ver() }
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.ver = next_unlocked_ver();
    }
}

fn next_unlocked_ver() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Owned cache key identifying `App::active_text()` in O(1) without hashing the note body.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextKey {
    vault_id: u64,
    path: String,
    ver: u64,
    unlocked: bool,
    len: usize,
}

/// Borrowed view of `TextKey` for allocation-free cache hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextKeyRef<'a> {
    vault_id: u64,
    path: &'a str,
    ver: u64,
    unlocked: bool,
    len: usize,
}

impl TextKey {
    pub fn matches(&self, r: TextKeyRef<'_>) -> bool {
        self.vault_id == r.vault_id
            && self.ver == r.ver
            && self.unlocked == r.unlocked
            && self.len == r.len
            && self.path == r.path
    }
}

impl TextKeyRef<'_> {
    pub fn to_owned(self) -> TextKey {
        TextKey { vault_id: self.vault_id, path: self.path.to_owned(), ver: self.ver, unlocked: self.unlocked, len: self.len }
    }

    pub fn hash_val(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.vault_id, self.path, self.ver, self.unlocked, self.len).hash(&mut h);
        h.finish()
    }
}

/// A note in Note mode's Open list while another shows: its editor, with the caret and undo
/// history, and its plain text if it was unlocked. `text` is what the file held, so a note
/// changed outside comes back from disk instead, locked again if it was locked.
struct Parked {
    text: String,
    edit: Option<TextEdit>,
    unlocked: Option<Unlocked>,
    map: Option<crate::map_app::MapView>,
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
        query: TextEdit::new(""),
        search: Default::default(),
        tree: Default::default(),
        res_focus: None,
        layers: Vec::new(),
        handed: None,
        layer_swap: false,
        set_sec: Sec::Appearance,
        set_gen: 0,
        set_slide: 0,
        focus: false,
        renaming: None,
        creating: None,
        edits: HashMap::new(),
        fields: HashMap::new(),
        reveal: BTreeSet::new(),
        sync_status: SyncStatus::Idle,
        mcp_copied: false,
        notice: None,
        notice_open: None,
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
        mcp_last_activity: None,
        mcp_seen_calls: 0,
        last_caret: None,
        key_moved: false,
        editor_accessed: false,
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
        tree_reveal: None,
        outline_out: None,
        new_folder: None,
        renaming_folder: None,
        sync: Default::default(),
        images: Default::default(),
        drop_over: false,
        hover_switch: None,
        unlocked: HashMap::new(),
        lock_job: None,
        last_activity: std::rc::Rc::new(std::cell::Cell::new(0.0)),
        lock_job_seq: 0,
        notes: Vec::new(),
        parked: HashMap::new(),
        laid_out: std::cell::Cell::new(false),
        prepare_due: None,
        prepared_key: None,
        folder_left: None,
        unlock_error: None,
        reader: Default::default(),
        wstate: Default::default(),
        map: Default::default(),
        board: Default::default(),
        tab_gen: 0,
        tab_slide: 0,
        mode_swapped: false,
        keymap: crate::keymap::Overrides::load(),
        key_edit: None,
        keys_query: TextEdit::new(""),
        closed_tabs: Vec::new(),
        nav_back: Vec::new(),
        nav_fwd: Vec::new(),
        nav_jumping: false,
        trash_entries: Vec::new(),
        conflict_diff: None,
    };
    crate::theme::reload_omarchy();
    // The remote folder used to be one value for every notes folder. Each folder has its own now.
    for c in app.persist.settings.sync.cfg.values_mut() {
        c.remove("prefix");
    }
    app.mode = Mode::from_id(&app.persist.mode);
    app.map.back = Mode::from_id(&app.persist.back);
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
        app.open_startup(&p, note.as_deref());
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
    pub fn settings(&self) -> &Settings {
        &self.persist.settings
    }

    pub fn start_key_edit(&mut self, action_id: &'static str, tx: &mut Tx) {
        self.key_edit = Some(KeyEdit { action_id, error: None });
        tx.focus(format!("key-rec-{action_id}"), true);
    }

    pub fn cancel_key_edit(&mut self) {
        self.key_edit = None;
    }

    pub fn clear_key_binding(&mut self, action_id: &'static str) {
        self.keymap.set(action_id, Vec::new());
        let _ = self.keymap.save();
        self.key_edit = None;
    }

    pub fn reset_key_binding(&mut self, action_id: &str) {
        self.keymap.reset(action_id);
        let _ = self.keymap.save();
        if self.key_edit.as_ref().is_some_and(|e| e.action_id == action_id) {
            self.key_edit = None;
        }
    }

    pub fn reset_all_key_bindings(&mut self) {
        self.keymap.reset_all();
        let _ = self.keymap.save();
        self.key_edit = None;
    }

    /// Captures a chord while recording an action in Settings → Shortcuts.
    pub fn handle_key_edit(&mut self, ev: &rune::KeyEvent) -> bool {
        let Some(edit) = self.key_edit.clone() else { return false };
        if matches!(ev.key, rune::Key::Tab) && !ev.mods.ctrl && !ev.mods.cmd && !ev.mods.alt && !ev.mods.logo {
            return false;
        }
        if matches!(ev.key, rune::Key::Escape) {
            self.key_edit = None;
            return true;
        }
        let Some(spec) = crate::keymap::format_event(ev) else {
            return true;
        };
        if let Some(act) = crate::keymap::action(edit.action_id)
            && act.ctx.iter().any(|&c| crate::keymap::is_reserved_field_key(c, &spec))
            && !matches!(act.id, "palette.next" | "palette.previous" | "palette.back-to-notes")
        {
            self.key_edit = Some(KeyEdit { action_id: edit.action_id, error: Some(format!("{} is reserved for text editing", crate::keymap::label_of(&spec))) });
            return true;
        }
        if let Some(other) = self.keymap.conflict(edit.action_id, &spec) {
            self.key_edit = Some(KeyEdit { action_id: edit.action_id, error: Some(format!("Already used by {}", other.name)) });
            return true;
        }
        self.keymap.set(edit.action_id, vec![spec]);
        let _ = self.keymap.save();
        self.key_edit = None;
        true
    }

    pub fn files(&self) -> &BTreeMap<String, String> {
        static EMPTY: BTreeMap<String, String> = BTreeMap::new();
        self.vault.as_ref().map_or(&EMPTY, |v| v.files())
    }

    /// The active note's text. An unlocked note gives its plain text, and a locked note none.
    pub fn active_text(&self) -> &str {
        self.active.as_deref().and_then(|a| self.note_text(a)).unwrap_or("")
    }

    /// Note `rel`'s plain text: None when it is gone, or locked and not unlocked.
    pub fn note_text(&self, rel: &str) -> Option<&str> {
        if let Some(u) = self.unlocked.get(rel) {
            return Some(&u.text);
        }
        self.files().get(rel).filter(|t| !crate::lock::is_locked(t)).map(String::as_str)
    }

    /// O(1) revision key for `active_text()`: changes whenever the vault, active note path, lock
    /// state, or note text changes, avoiding per-frame full-text hashing.
    pub fn active_text_key(&self) -> TextKeyRef<'_> {
        let vault_id = self.vault.as_ref().map_or(0, |v| v.id());
        let Some(a) = self.active.as_deref() else {
            return TextKeyRef { vault_id, path: "", ver: 0, unlocked: false, len: 0 };
        };
        if let Some(u) = self.unlocked.get(a) {
            return TextKeyRef { vault_id, path: a, ver: u.ver, unlocked: true, len: u.text.len() };
        }
        match self.vault.as_ref().and_then(|v| v.files().get(a).map(|t| (v, t))) {
            Some((v, t)) if !crate::lock::is_locked(t) => TextKeyRef { vault_id, path: a, ver: v.ver(a), unlocked: false, len: t.len() },
            _ => TextKeyRef { vault_id, path: "", ver: 0, unlocked: false, len: 0 },
        }
    }

    /// The note at `p` is locked and not open: its text needs the password.
    pub fn note_locked(&self, p: &str) -> bool {
        !self.unlocked.contains_key(p) && self.files().get(p).is_some_and(|t| crate::lock::is_locked(t))
    }

    /// The active note is locked and needs its password before it can be read or edited.
    pub fn active_locked(&self) -> bool {
        self.active.as_deref().is_some_and(|a| self.note_locked(a))
    }

    /// The note at `p` is locked and open now.
    pub fn note_unlocked(&self, p: &str) -> bool {
        self.unlocked.contains_key(p)
    }

    /// The note at `p` is password-protected, whether sealed or unlocked in this session.
    pub fn is_locked(&self, p: &str) -> bool {
        self.unlocked.contains_key(p) || self.files().get(p).is_some_and(|t| crate::lock::is_locked(t))
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
            self.persist.mode = self.mode.id().into();
            self.persist.back = self.map.back.id().into();
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
        changed |= self.sync_mcp_http(tx);
        changed |= self.check_link_click(tx);
        changed |= self.apply_outside_changes();
        changed |= self.poll_lock(t, tx);
        changed |= self.poll_map_paste(t, tx);
        changed |= self.poll_auto_lock(t, tx);
        changed |= self.poll_trash(tx);
        changed |= self.poll_sync(t, tx);
        changed |= self.images.poll();
        changed |= self.expire_reader(t);
        self.reveal_tab(tx);
        self.reveal_tree(tx);
        changed |= self.poll_reveal(tx);
        changed |= self.expire_sheet(t);
        changed |= self.drop_stale_layers(tx);
        changed |= self.expire_deleted(t, tx);
        self.poll_writer(t, tx);
        changed |= self.poll_map(t, tx);
        changed |= self.poll_board(t, tx);
        if self.want_editor_focus && self.laid_out.get() {
            self.want_editor_focus = false;
            if self.layers.is_empty() && tx.focused().is_none() {
                if self.writing_note() {
                    tx.focus("editor", false);
                } else if self.active_locked() {
                    tx.focus(crate::ui::lock::PASS, false);
                }
            }
        }
        self.poll_prepare(t, tx);
        changed |= self.search_due(t, tx);
        let res = tx.focused().filter(|k| k.starts_with("res-")).map(str::to_string);
        if res != self.res_focus {
            self.res_focus = res;
            changed = true;
        }
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

    /// Schedules idle text pre-warming (`Tx::prepare_text(ui::prepare_cold)`) 250 ms after opening
    /// a vault note or changing viewport/theme so initial frames finish undisturbed at 60 Hz while
    /// cold 1st `Ctrl+P` (Preview->Source) and 1st `Ctrl+Shift+K` (Command Palette) hit warm caches.
    fn poll_prepare(&mut self, t: f64, tx: &mut Tx) {
        if self.screen != Screen::Vault || !self.laid_out.get() || self.active_locked() {
            return;
        }
        let vw = self.view_w.get();
        let dark = self.dark_mode(self.sys_dark.get());
        let set = self.settings();
        let left_vis = self.persist.left_open && !self.focus && vw >= SIDEBAR_DOCK_W;
        let right_vis = self.persist.right_open && !self.focus && self.active.is_some() && vw >= OUTLINE_DOCK_W;
        let source = (matches!(self.mode, Mode::Preview | Mode::Map | Mode::Board) && self.active.is_some() && self.active_text().len() <= 2_000_000).then(|| {
            (self.active_text_key().to_owned(), vw.to_bits(), set.size, set.font.clone(), self.wide(), left_vis, right_vis, dark)
        });
        let palette = (dark, self.keymap.rev, self.note_mode(), self.active.is_some(), set.panel_font.clone());
        let cur = PrepareKey { source, palette };
        if self.prepared_key.as_ref() == Some(&cur)
            || (cur.source.is_none() && self.prepared_key.as_ref().is_some_and(|p| p.palette == cur.palette))
        {
            self.prepare_due = None;
            return;
        }
        if self.mode == Mode::Map && (self.map.glide.is_some() || self.map.relayout.is_some()) {
            return;
        }
        match self.prepare_due {
            None => {
                self.prepare_due = Some(t + 0.25);
                tx.wake_after(Duration::from_millis(250));
            }
            Some(due) if t >= due => {
                self.prepare_due = None;
                self.prepared_key = Some(cur);
                tx.prepare_text(crate::ui::prepare_cold, None);
            }
            Some(due) => {
                tx.wake_after(Duration::from_secs_f64((due - t).max(0.01)));
            }
        }
    }

    pub fn set_waker(&mut self, w: rune::Waker) {
        self.writer.set_waker(w.clone());
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
            let old = self.files().get(&rel).cloned();
            if let Some(e) = self.edits.get_mut(&rel) {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            if let Some(v) = &mut self.vault {
                if old.is_none() {
                    v.born.insert(rel.clone(), now_ns());
                }
                v.put(rel.clone(), text);
            }
            if let Some(old) = old {
                self.map_reloaded(&rel, &old);
                if self.active.as_deref() == Some(rel.as_str()) {
                    self.board_cancel_hold();
                }
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

    /// A text field gives no click event, so a click shows as a caret move or fresh `on_edit`
    /// access with no key and no text change. With Ctrl (⌘ on macOS) held, that click follows the
    /// link under the caret, even when the caret was already resting on the link.
    fn check_link_click(&mut self, tx: &mut Tx) -> bool {
        let accessed = std::mem::take(&mut self.editor_accessed);
        let key_moved = std::mem::take(&mut self.key_moved);
        if self.mode != Mode::Source {
            self.last_caret = None;
            return false;
        }
        let Some(a) = self.active.clone() else {
            self.last_caret = None;
            return false;
        };
        let Some(e) = self.edits.get(&a) else { return false };
        let cur = (a.clone(), e.anchor, e.focus, e.text.len());
        let same_doc = self.last_caret.as_ref().is_some_and(|l| l.0 == cur.0 && l.3 == cur.3);
        let moved = same_doc && self.last_caret.as_ref().is_some_and(|l| (l.1, l.2) != (cur.1, cur.2));
        let clicked = (moved || (same_doc && accessed)) && !key_moved && cur.1 == cur.2 && e.focused && tx.mods().cmd;
        self.last_caret = Some(cur);
        if clicked {
            crate::ui::follow_link_at_caret(self, tx);
            return true;
        }
        false
    }

    /// Clears the failed MCP HTTP port lock so `sync_mcp_http` tries binding again.
    pub fn retry_mcp_http(&mut self) {
        self.mcp_http_failed_port = None;
        self.mcp_http_error = None;
    }

    /// Starts, updates or stops the HTTP MCP server to match the settings, and surfaces recent
    /// MCP client tool activity.
    fn sync_mcp_http(&mut self, tx: &mut Tx) -> bool {
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
                let calls = srv.activity.calls.load(std::sync::atomic::Ordering::SeqCst);
                if calls != self.mcp_seen_calls {
                    self.mcp_seen_calls = calls;
                    let last = srv.activity.last.lock().ok().and_then(|g| g.clone());
                    if let Some((label, ts)) = last {
                        self.mcp_last_activity = Some((label.clone(), ts));
                        self.toast(&format!("MCP · {label}"), tx);
                    }
                    return true;
                }
                return false;
            }
            self.mcp_http = None;
        }
        if self.mcp_http_error.is_some() && self.mcp_http_failed_port == Some(port) {
            return false;
        }
        match crate::mcp::serve_http(port, cfg, self.waker.clone()) {
            Ok(srv) => {
                self.mcp_http = Some(srv);
                self.mcp_http_error = None;
                self.mcp_http_failed_port = None;
                self.mcp_seen_calls = 0;
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
        let old_idx = self.note_path().and_then(|cur| self.notes.iter().position(|n| n == &cur));
        self.park();
        let mut parked = self.parked.remove(&file);
        if parked.is_some() {
            // Its last edits are queued for the writer: read the file after them.
            self.writer.wait(Duration::from_secs(2));
        }
        match vault::load_one(&file) {
            Ok(v) => {
                let new = !self.notes.contains(&file);
                if new {
                    self.notes.push(file.clone());
                }
                let new_idx = self.notes.iter().position(|n| n == &file);
                // After `enter`, which keeps Folder mode's mode when Note mode starts.
                self.enter(v);
                if new {
                    self.mode = Mode::Preview;
                    self.want_editor_focus = false;
                }
                let dir = match (old_idx, new_idx) {
                    (Some(a), Some(b)) if b < a => -1,
                    (Some(_), _) => 1,
                    (None, _) => 0,
                };
                self.bump_tab(dir);
                let rel = self.active.clone().unwrap_or_default();
                // The map's camera and folds hold even when the text changed meanwhile.
                if let Some(m) = parked.as_mut().and_then(|p| p.map.take()) {
                    self.map.views.insert(rel.clone(), m);
                }
                if let Some(p) = parked.filter(|p| self.files().get(&rel) == Some(&p.text)) {
                    if let Some(u) = p.unlocked {
                        self.unlocked.insert(rel.clone(), u);
                    }
                    if let Some(mut e) = p.edit {
                        e.focused = self.mode == Mode::Source;
                        self.edits.insert(rel, e);
                    }
                }
                if self.mode == Mode::Map {
                    self.place_due();
                }
                true
            }
            Err(e) => {
                self.notice = Some(format!("Could not open {}: {e}", store::tilde(&file)));
                false
            }
        }
    }

    /// Whether any parked Note-mode note currently holds decrypted plaintext in memory.
    pub(crate) fn parked_has_unlocked(&self) -> bool {
        self.parked.values().any(|p| p.unlocked.is_some())
    }

    /// Drops decrypted keys and editor buffers from every parked Note-mode note so
    /// `lock_all_notes` and idle auto-lock leave zero plaintext behind.
    pub(crate) fn clear_parked_unlocked(&mut self) {
        for p in self.parked.values_mut() {
            if p.unlocked.take().is_some() {
                p.edit = None;
            }
        }
    }

    /// Keeps the shown note's editor, unlocked text and map while another note shows in Note
    /// mode.
    fn park(&mut self) {
        self.map_settle();
        let Some(file) = self.note_path() else { return };
        let Some(rel) = self.active.clone() else { return };
        let Some(text) = self.files().get(&rel).cloned() else { return };
        let (edit, unlocked, map) = (self.edits.remove(&rel), self.unlocked.remove(&rel), self.map.views.remove(&rel));
        self.parked.insert(file, Parked { text, edit, unlocked, map });
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
        while let Some(next_i) = crate::sidebar::next_active(i, self.notes.len()) {
            let next = self.notes[next_i].clone();
            if self.open_note(&next) {
                // Showing the neighbour parked the closed note.
                self.parked.remove(file);
                self.tab_slide = if next_i < i { -1 } else { 1 };
                return;
            }
            self.notes.retain(|n| *n != next);
        }
        self.flush_notes();
        self.tabs.clear();
        self.edits.clear();
        self.renaming_folder = None;
        self.lock_job = None;
        self.unlocked.clear();
        self.active = None;
        self.bump_tab(0);
        self.focus = false;
    }

    /// The full path of the note shown in Note mode.
    pub fn note_path(&self) -> Option<PathBuf> {
        let v = self.vault.as_ref().filter(|v| v.scope == Scope::Note)?;
        Some(v.path.join(self.active.as_ref()?))
    }

    /// The full on-disk path of the active note, in Folder or Note mode.
    pub fn active_path(&self) -> Option<PathBuf> {
        let v = self.vault.as_ref()?;
        Some(v.path.join(self.active.as_ref()?))
    }

    /// Copies the active note's full path to the clipboard and shows a toast.
    pub fn copy_path(&mut self, tx: &mut Tx) {
        let Some(path) = self.active_path() else { return };
        tx.copy(path.to_string_lossy().into_owned());
        self.toast("Copied path", tx);
    }

    /// Copies the full on-disk path of an Open shelf tab (folder or single-file note).
    pub fn copy_shelved_path(&mut self, t: &Shelved, tx: &mut Tx) {
        let path = match t {
            Shelved::Tab(p) => self.vault.as_ref().map(|v| v.path.join(p)),
            Shelved::File(f) => Some(f.clone()),
        };
        let Some(path) = path else { return };
        tx.copy(path.to_string_lossy().into_owned());
        self.toast("Copied path", tx);
    }

    /// Closes every Open shelf tab except `keep` and makes `keep` active.
    pub fn close_other_shelved(&mut self, keep: &Shelved, tx: &mut Tx) {
        match keep {
            Shelved::Tab(p) => {
                if !self.tabs.iter().any(|t| t == p) {
                    return;
                }
                if self.active.as_deref() != Some(p.as_str()) {
                    self.open_file(p, None, tx);
                }
                let others: Vec<String> = self.tabs.iter().filter(|t| t.as_str() != p.as_str()).cloned().collect();
                for o in others {
                    self.close_tab(&o);
                }
            }
            Shelved::File(f) => {
                if !self.notes.iter().any(|n| n == f) {
                    return;
                }
                if self.note_path().as_ref() != Some(f) {
                    self.open_note(f);
                }
                let others: Vec<PathBuf> = self.notes.iter().filter(|n| *n != f).cloned().collect();
                for o in others {
                    self.close_note(&o);
                }
            }
        }
        self.keys_to_note(tx);
    }

    /// Closes every open tab in the Open shelf (Folder mode or Note mode).
    pub fn close_all_tabs(&mut self, tx: &mut Tx) {
        if self.note_mode() {
            let all: Vec<PathBuf> = self.notes.clone();
            for f in all {
                self.close_note(&f);
            }
        } else {
            let all: Vec<String> = self.tabs.clone();
            for t in all {
                self.close_tab(&t);
            }
        }
        self.keys_to_note(tx);
    }

    /// Collapses every expanded folder in the vault tree.
    pub fn collapse_all_folders(&mut self) {
        if self.note_mode() {
            return;
        }
        self.expanded.clear();
        self.persist_later();
    }

    /// Reveals `folder` (and `note` inside it) in the sidebar file tree, opening the sidebar if needed.
    pub fn reveal_folder_in_sidebar(&mut self, folder: &str, note: &str, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        self.query = TextEdit::new("");
        self.persist.sidebar.unfold(crate::sidebar::Section::Vault);
        self.expand_ancestors(note);
        if !folder.is_empty() {
            self.expanded.insert(folder.to_string());
        }
        self.tree_reveal = Some(note.to_string());
        self.show_left(tx);
        self.persist_later();
    }

    /// Opens the active Note-mode file's parent directory as a full Folder vault, keeping the note open.
    pub fn open_note_folder(&mut self, tx: &mut Tx) {
        if !self.note_mode() {
            return;
        }
        let Some(dir) = self.vault.as_ref().map(|v| v.path.clone()) else { return };
        let file = self.active.clone();
        self.open_vault(dir, tx);
        if let Some(f) = file
            && self.files().contains_key(&f)
        {
            self.open_file(&f, None, tx);
        }
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
        self.map_new_notes();
        self.flush_notes();
        self.store_session();
        self.closed_tabs.clear();
        self.nav_back.clear();
        self.nav_fwd.clear();
        match v.scope {
            Scope::Folder => {
                self.notes.clear();
                self.parked.clear();
                if let Some(left) = self.folder_left.take() {
                    self.persist.left_open = left;
                    self.mode = Mode::from_id(&self.persist.mode);
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
        let saved = match v.scope {
            Scope::Folder => self.persist.sessions.get(&*path.to_string_lossy()).cloned(),
            Scope::Note => None,
        };
        let has_saved = saved.is_some();
        let ses = match v.scope {
            Scope::Folder => saved.unwrap_or_default(),
            Scope::Note => {
                let only: Vec<String> = v.files().keys().cloned().collect();
                Session { active: only.first().cloned(), tabs: only, ..Default::default() }
            }
        };
        self.tabs = ses.tabs.into_iter().filter(|p| v.files().contains_key(p)).collect();
        self.active = ses.active.filter(|a| v.files().contains_key(a)).or_else(|| self.tabs.first().cloned());
        self.tab_slide = 0;
        self.recent = ses.recent.into_iter().filter(|p| v.files().contains_key(p)).collect();
        if self.recent.is_empty() {
            self.recent = self.tabs.clone();
        }
        self.expanded = ses.expanded.into_iter().collect();
        self.tree_reveal = None;
        // Preserve a saved session's explicitly empty `expanded: []` (SB-134). Only auto-expand
        // ancestors when no saved session exists for this vault (or via `open_startup` / `open_file`).
        if !has_saved
            && self.expanded.is_empty()
            && let Some(a) = self.active.clone()
        {
            self.expand_ancestors(&a);
            self.tree_reveal = Some(a);
        }
        if v.scope == Scope::Folder {
            if !self.persist.vaults.iter().any(|r| r.path == path) {
                self.persist.vaults.push(VaultRef { name: v.name.clone(), path: path.clone() });
            }
            self.persist.last = Some(path);
        }
        self.vault = Some(v);
        self.edits.clear();
        self.renaming_folder = None;
        self.lock_job = None;
        // An Undo offer puts a note back into the folder it left, so it ends with that folder.
        self.deleted = None;
        self.notice_open = None;
        self.trash_entries.clear();
        // Paths name notes only within one vault: a note unlocked in the last one locks again.
        self.unlocked.clear();
        self.rebuild_names();
        self.refresh_trash();
        if self.waker.is_some() {
            self.watch_vault();
        }
        self.screen = Screen::Vault;
        self.creating = None;
        self.query = TextEdit::new("");
        self.layers.clear();
        self.renaming = None;
        self.ensure_edit();
        if self.mode == Mode::Source {
            if let Some(e) = self.cur_edit_opt() {
                e.focused = true;
            }
        }
        self.want_editor_focus = self.mode == Mode::Source || (self.mode == Mode::Map && self.active_locked());
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

    pub fn forget_vault(&mut self, path: &Path, tx: &mut Tx) {
        let idx = self.persist.vaults.iter().position(|v| v.path == path);
        self.persist.vaults.retain(|v| v.path != path);
        self.persist_later();
        if self.screen == Screen::Welcome {
            let target = match (idx, self.persist.vaults.len()) {
                (Some(i), n) if n > 0 => format!("recent-{}", i.min(n - 1)),
                _ => "create".to_string(),
            };
            tx.focus(&target, tx.by_keys());
        }
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
            self.ensure_edit_of(&a);
        }
    }

    fn ensure_edit_of(&mut self, rel: &str) {
        if !self.edits.contains_key(rel) {
            let mut e = TextEdit::new(self.note_text(rel).unwrap_or_default());
            e.set_selection(0, 0);
            self.edits.insert(rel.to_string(), e);
        }
    }

    /// Note `rel`'s bytes `r` become `with` as one step of its editor's undo history, so undo
    /// in any mode takes the change back, then the note saves as `set_text` does. The caret
    /// keeps its place in the text around the change.
    pub fn splice_note(&mut self, rel: &str, r: std::ops::Range<usize>, with: &str) {
        let Some(base) = self.note_text(rel).map(str::to_string) else { return };
        let text = format!("{}{with}{}", &base[..r.start], &base[r.end..]);
        self.ensure_edit_of(rel);
        if let Some(e) = self.edits.get_mut(rel) {
            if e.text != base {
                e.replace_text(base);
            }
            let (a, f) = (e.anchor, e.focus);
            let shift = |p: usize| if p <= r.start { p } else if p >= r.end { p + with.len() - r.len() } else { r.start };
            e.undo_group(|e| {
                e.set_selection(r.start, r.end);
                e.insert(with);
            });
            e.set_selection(shift(a), shift(f));
        }
        self.set_text(rel, text);
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
    /// Records user interaction on the app clock so idle auto-lock resets its timer.
    pub fn touch_activity(&self) {
        self.last_activity.set(self.now.get());
    }

    pub fn after_edit(&mut self) {
        self.touch_activity();
        let writing_with_foot = self.writing() && self.foot_shows();
        let typewriter = self.settings().typewriter_scroll;
        self.reader.reveal.on_editor_edit(writing_with_foot, typewriter);
        let Some(a) = self.active.clone() else { return };
        if self.unlocked.contains_key(&a) {
            let changed = self.edits.get(&a).is_some_and(|e| self.unlocked.get(&a).is_some_and(|u| u.text != e.text));
            if changed && let Some(text) = self.edits.get(&a).map(|e| e.text.clone()) {
                self.store_unlocked(&a, text);
            }
            return;
        }
        if self.note_locked(&a) {
            return;
        }
        let changed = self.edits.get(&a).is_some_and(|e| self.vault.as_ref().is_some_and(|v| v.files().get(&a).map(String::as_str) != Some(e.text.as_str())));
        if changed && let Some(text) = self.edits.get(&a).map(|e| e.text.clone()) {
            let old_key = self.active_text_key().to_owned();
            let old_text = self.vault.as_mut().and_then(|v| v.replace(a.clone(), text));
            if self.vault.is_some() {
                self.mark_dirty(&a);
            }
            if let Some(old_text) = old_text {
                let new_key = self.active_text_key().to_owned();
                self.on_single_line_edit(old_key, new_key, &old_text, self.active_text());
            }
        }
    }

    pub fn focus_editor(&mut self, tx: &mut Tx) {
        if self.active_locked() {
            tx.focus(crate::ui::lock::PASS, false);
            self.want_editor_focus = !self.laid_out.get();
        } else if self.active.is_some() && self.mode == Mode::Source {
            tx.focus("editor", false);
            self.want_editor_focus = !self.laid_out.get();
        }
    }

    /// Opens `dir` at startup, optionally activating `note` and expanding its ancestor folders.
    pub(crate) fn open_startup(&mut self, dir: &Path, note: Option<&str>) {
        if self.load_vault(dir).is_err() {
            self.screen = Screen::Welcome;
        } else if let Some(n) = note.filter(|n| self.files().contains_key(*n)) {
            if !self.tabs.iter().any(|t| t == n) {
                self.tabs.push(n.to_string());
            }
            self.expand_ancestors(n);
            self.active = Some(n.to_string());
            self.tree_reveal = Some(n.to_string());
            self.ensure_edit();
            self.want_editor_focus = self.mode == Mode::Source || self.active_locked();
        }
    }

    /// Expands every ancestor folder of `path` in the vault tree.
    fn expand_ancestors(&mut self, path: &str) {
        let mut d = md::dir(path).to_string();
        while !d.is_empty() {
            self.expanded.insert(d.clone());
            d = md::dir(&d).to_string();
        }
    }

    /// Whether every ancestor folder of `path` is currently expanded in the vault tree.
    pub(crate) fn ancestors_expanded(&self, path: &str) -> bool {
        let mut d = md::dir(path);
        while !d.is_empty() {
            if !self.expanded.contains(d) {
                return false;
            }
            d = md::dir(d);
        }
        true
    }

    /// Whether the vault tree (`side-scroll`) is currently visible in the sidebar or its drawer.
    pub(crate) fn tree_visible(&self) -> bool {
        self.screen == Screen::Vault
            && !self.note_mode()
            && !self.focus
            && self.sidebar_open()
            && self.query.text.trim().is_empty()
            && self.persist.sidebar.folded != Some(crate::sidebar::Section::Vault)
    }

    /// Scrolls `side-scroll` to show `tree_reveal` once the vault tree is visible.
    fn reveal_tree(&mut self, tx: &mut Tx) {
        if !self.tree_visible() {
            return;
        }
        let Some(path) = self.tree_reveal.take() else { return };
        if self.files().contains_key(&path) && self.ancestors_expanded(&path) {
            tx.scroll_into_view_in("side-scroll", crate::folders::row_key("file", &path));
        }
    }

    fn bump_tab(&mut self, dir: i8) {
        self.tab_gen = self.tab_gen.wrapping_add(1);
        self.tab_slide = dir;
        self.mode_swapped = false;
    }

    pub fn open_file(&mut self, path: &str, sel: Option<Sel>, tx: &mut Tx) {
        if self.files().get(path).is_none() {
            return;
        }
        let old_idx = self.active.as_ref().and_then(|a| self.tabs.iter().position(|t| t == a));
        if !self.tabs.iter().any(|t| t == path) {
            self.tabs.push(path.to_string());
            self.reorder = None;
        }
        let new_idx = self.tabs.iter().position(|t| t == path);
        let changed = self.active.as_deref() != Some(path);
        let from = self.active.replace(path.to_string());
        if changed {
            self.conflict_diff = None;
            if !self.nav_jumping
                && let Some(prev) = from.as_ref()
                && self.nav_back.last() != Some(prev)
            {
                self.nav_back.push(prev.clone());
                if self.nav_back.len() > 64 {
                    self.nav_back.remove(0);
                }
                self.nav_fwd.clear();
            }
            self.cancel_unlock_job();
            let dir = match (old_idx, new_idx) {
                (Some(a), Some(b)) if b < a => -1,
                (Some(_), _) => 1,
                (None, _) => 0,
            };
            self.bump_tab(dir);
        }
        self.expand_ancestors(path);
        self.tree_reveal = Some(path.to_string());
        self.recent.retain(|x| x != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(30);
        self.renaming = None;
        if sel.is_some_and(|s| s.force_source) {
            self.mode = Mode::Source;
        }
        self.ensure_edit();
        if changed && self.mode == Mode::Map {
            self.place_due();
        }
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
        if self.closed_tabs.last().map(String::as_str) != Some(p) {
            self.closed_tabs.push(p.to_string());
            if self.closed_tabs.len() > 32 {
                self.closed_tabs.remove(0);
            }
        }
        self.reorder = None;
        self.edits.remove(p);
        self.drop_map(p);
        // A closed note locks again.
        self.unlocked.remove(p);
        if self.active.as_deref() == Some(p) {
            self.conflict_diff = None;
            let next = crate::sidebar::next_active(i, self.tabs.len());
            self.active = next.map(|n| self.tabs[n].clone());
            self.bump_tab(match next {
                Some(n) if n < i => -1,
                Some(_) => 1,
                None => 0,
            });
            if self.active.is_none() {
                self.focus = false;
            }
            // The note that shows now has its editor, so its text shows at once.
            self.ensure_edit();
            if self.mode == Mode::Map {
                self.place_due();
            }
        }
        self.renaming = None;
        self.persist_later();
    }

    /// Reopens the most recently closed tab in this folder (`app.reopen-tab`, `Ctrl+Shift+T`).
    pub fn reopen_tab(&mut self, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        while let Some(p) = self.closed_tabs.pop() {
            if self.files().contains_key(&p) && !self.tabs.contains(&p) {
                self.open_file(&p, None, tx);
                return;
            }
        }
    }

    /// Starts renaming `path` from the file tree (`F2`): opens it if needed, then focuses rename.
    pub fn rename_note_path(&mut self, path: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() || !self.files().contains_key(path) {
            return;
        }
        if self.active.as_deref() != Some(path) {
            self.open_file(path, None, tx);
        }
        self.start_rename(tx);
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

    /// Switches to `t` from the Open shelf, closes any drawer, and hands keys to the note.
    fn activate_shelved(&mut self, t: &Shelved, tx: &mut Tx) {
        if self.shelved().as_ref() != Some(t) {
            match t {
                Shelved::Tab(p) => self.open_file(p, None, tx),
                Shelved::File(f) => {
                    self.open_note(f);
                }
            }
        }
        self.close_drawer(tx);
        self.keys_to_note(tx);
    }

    /// Switches to the 0-based tab `index` in the Open shelf (`Ctrl+1`..`Ctrl+9`).
    pub fn switch_tab_index(&mut self, index: usize, tx: &mut Tx) {
        let shelf = self.shelf();
        let Some(target) = shelf.get(index).cloned() else { return };
        self.activate_shelved(&target, tx);
    }

    /// Switches to the next (`forward`) or previous tab in the Open shelf, wrapping around
    /// (`Ctrl+Tab` / `Ctrl+Shift+Tab`).
    pub fn step_tab(&mut self, forward: bool, tx: &mut Tx) {
        let shelf = self.shelf();
        if shelf.len() < 2 {
            return;
        }
        let cur = self.shelved().and_then(|on| shelf.iter().position(|t| t == &on)).unwrap_or(0);
        let next = if forward { (cur + 1) % shelf.len() } else { (cur + shelf.len() - 1) % shelf.len() };
        self.activate_shelved(&shelf[next], tx);
        self.tab_slide = if forward { 1 } else { -1 };
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

    /// Before each frame: when the active tab changed, by a click, a key, a close or a note
    /// opened elsewhere, the shelf scrolls just far enough to show it (SB-24), and the outline
    /// that was out stays folded, should its tab be shown again (SB-26).
    fn reveal_tab(&mut self, tx: &mut Tx) {
        let on = self.shelved();
        if self.outline_out.is_some() && self.outline_out != on {
            self.outline_out = None;
        }
        let key = on.map(|t| t.key());
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
        self.refresh_trash();
        self.sync_after_save();
    }

    /// Scans `<vault>/.trash/` on the `Writer` thread so the sidebar count and Trash modal stay fresh.
    pub fn refresh_trash(&self) {
        if let Some(v) = self.vault.as_ref().filter(|v| v.scope == Scope::Folder) {
            self.writer.send(Op::ListTrash { root: v.path.clone() });
        }
    }

    /// Opens the browseable Trash recovery modal (`Layer::Trash`) in folder mode.
    pub fn open_trash(&mut self, tx: &mut Tx) {
        if self.refuse_in_note_mode() || !self.folder_open() {
            return;
        }
        self.refresh_trash();
        self.open_layer(Layer::Trash(crate::layer::TrashModal::default()), tx);
    }

    /// Navigates back to the previously visited note (`Alt+Left`, `app.nav-back`).
    pub fn nav_back(&mut self, tx: &mut Tx) {
        while let Some(target) = self.nav_back.pop() {
            if !self.files().contains_key(&target) || self.active.as_deref() == Some(target.as_str()) {
                continue;
            }
            if let Some(cur) = self.active.clone() {
                self.nav_fwd.push(cur);
                if self.nav_fwd.len() > 64 {
                    self.nav_fwd.remove(0);
                }
            }
            self.nav_jumping = true;
            self.open_file(&target, None, tx);
            self.nav_jumping = false;
            return;
        }
    }

    /// Navigates forward to the next note in history (`Alt+Right`, `app.nav-forward`).
    pub fn nav_forward(&mut self, tx: &mut Tx) {
        while let Some(target) = self.nav_fwd.pop() {
            if !self.files().contains_key(&target) || self.active.as_deref() == Some(target.as_str()) {
                continue;
            }
            if let Some(cur) = self.active.clone() {
                self.nav_back.push(cur);
                if self.nav_back.len() > 64 {
                    self.nav_back.remove(0);
                }
            }
            self.nav_jumping = true;
            self.open_file(&target, None, tx);
            self.nav_jumping = false;
            return;
        }
    }

    /// Restores a trashed note or folder out of `.trash/` on the `Writer` thread.
    /// Keeps `trash-dialog` open while other items remain in `.trash` (closed automatically in
    /// `poll_trash` once the last item is restored).
    pub fn restore_trash(&mut self, trash_name: &str, _tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        let Some(root) = self.vault.as_ref().map(|v| v.path.clone()) else { return };
        if let Some(Layer::Trash(m)) = self.layers.iter_mut().find_map(|o| matches!(o.layer, Layer::Trash(_)).then_some(&mut o.layer)) {
            m.armed_empty = false;
            m.armed_delete = None;
            m.armed_at = None;
        }
        self.writer.send(Op::RestoreTrash { root, trash_name: trash_name.to_string() });
    }

    /// Two-step armed permanent delete for a single entry inside `Layer::Trash`.
    pub fn delete_trash_item_press(&mut self, trash_name: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        let lapse = std::time::Duration::from_secs(crate::ui::tokens::motion::ARMED_LAPSE_SECS);
        let armed = self.layers.iter_mut().find_map(|o| match &mut o.layer {
            Layer::Trash(m) => {
                let fresh = m.armed_at.is_some_and(|t| t.elapsed() < lapse);
                m.armed_empty = false;
                if fresh && m.armed_delete.as_deref() == Some(trash_name) {
                    m.armed_delete = None;
                    m.armed_at = None;
                    Some(true)
                } else {
                    m.armed_delete = Some(trash_name.to_string());
                    m.armed_at = Some(std::time::Instant::now());
                    tx.wake_after(lapse);
                    Some(false)
                }
            }
            _ => None,
        });
        if armed == Some(true)
            && let Some(root) = self.vault.as_ref().map(|v| v.path.clone())
        {
            self.writer.send(Op::DeleteTrashItem { root, trash_name: trash_name.to_string() });
            self.toast("Deleted permanently", tx);
        }
    }

    /// Armed two-step Empty Trash action inside `Layer::Trash`.
    pub fn empty_trash_press(&mut self, tx: &mut Tx) {
        if self.refuse_in_note_mode() || self.trash_entries.is_empty() {
            return;
        }
        let lapse = std::time::Duration::from_secs(crate::ui::tokens::motion::ARMED_LAPSE_SECS);
        let armed = self.layers.iter_mut().find_map(|o| match &mut o.layer {
            Layer::Trash(m) => {
                let fresh = m.armed_at.is_some_and(|t| t.elapsed() < lapse);
                m.armed_delete = None;
                if fresh && m.armed_empty {
                    m.armed_empty = false;
                    m.armed_at = None;
                    Some(true)
                } else {
                    m.armed_empty = true;
                    m.armed_at = Some(std::time::Instant::now());
                    tx.wake_after(lapse);
                    Some(false)
                }
            }
            _ => None,
        });
        if armed == Some(true)
            && let Some(root) = self.vault.as_ref().map(|v| v.path.clone())
        {
            self.deleted = None;
            self.writer.send(Op::EmptyTrash { root });
            self.toast("Emptied trash", tx);
        }
    }

    /// Drains completed trash scans/restores/empties from the `Writer` thread and disarms
    /// expired Trash delete confirmations (`IX-05`).
    fn poll_trash(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        let lapse = std::time::Duration::from_secs(crate::ui::tokens::motion::ARMED_LAPSE_SECS);
        for o in &mut self.layers {
            if let Layer::Trash(m) = &mut o.layer
                && let Some(t) = m.armed_at
                && t.elapsed() >= lapse
            {
                m.armed_empty = false;
                m.armed_delete = None;
                m.armed_at = None;
                changed = true;
            }
        }
        let events: Vec<crate::vault::TrashEvent> = match self.writer.trash_events.lock() {
            Ok(mut q) => std::mem::take(&mut *q),
            Err(_) => return changed,
        };
        if events.is_empty() {
            return changed;
        }
        for ev in events {
            let cur_root = self.vault.as_ref().filter(|v| v.scope == Scope::Folder).map(|v| v.path.clone());
            match ev {
                crate::vault::TrashEvent::Listed { root, entries } => {
                    if cur_root.as_ref() == Some(&root) && self.trash_entries != entries {
                        self.trash_entries = entries;
                        changed = true;
                    }
                }
                crate::vault::TrashEvent::Restored { root, original_rel, label, files, folders, open_rel, entries } => {
                    if cur_root.as_ref() != Some(&root) {
                        continue;
                    }
                    if self.deleted.as_ref().is_some_and(|d| {
                        d.rel == original_rel
                            || files.iter().any(|(p, _, _)| p == &d.rel)
                            || folders.iter().any(|f| f == &d.rel)
                    }) {
                        self.deleted = None;
                    }
                    if let Some(v) = &mut self.vault {
                        for f in folders {
                            if !v.folders.contains(&f) {
                                v.folders.push(f);
                            }
                        }
                        v.folders.sort();
                        for (rel, text, born) in files {
                            v.born.insert(rel.clone(), born);
                            v.put(rel, text);
                        }
                    }
                    self.trash_entries = entries;
                    self.rebuild_names();
                    let stay_open = !self.trash_entries.is_empty() && self.layer_is(crate::layer::Kind::Trash);
                    if !stay_open {
                        if self.layer_is(crate::layer::Kind::Trash) {
                            self.close_layer(tx);
                        }
                        if let Some(rel) = open_rel {
                            self.open_file(&rel, None, tx);
                        }
                    }
                    self.toast(&format!("Restored {label}"), tx);
                    self.sync_after_save();
                    changed = true;
                }
                crate::vault::TrashEvent::ItemDeleted { root, entries } => {
                    if cur_root.as_ref() == Some(&root) {
                        self.trash_entries = entries;
                        changed = true;
                    }
                }
                crate::vault::TrashEvent::Emptied { root } => {
                    if cur_root.as_ref() == Some(&root) {
                        self.trash_entries.clear();
                        changed = true;
                    }
                }
            }
        }
        changed
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
    /// travels to that heading in the reader, or to its node on the map, once the note is
    /// shown (RD-102, RD-04, MAP-054).
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
                let fresh = self.mode == Mode::Map && self.map.views.get(&p).is_none_or(|v| !v.placed);
                self.open_file(&p, None, tx);
                if let Some(f) = frag {
                    // `open_file` gives a map shown for the first time its fitted camera; the
                    // heading's node takes that first placement instead, at 100% (MAP-054).
                    if fresh && let Some(v) = self.map_view_mut() {
                        v.placed = false;
                    }
                    self.go_anchor(&f, tx);
                }
            }
            None => self.new_note(Some(&t), tx),
        }
    }

    /// Ticks (`done`) or clears the task box on 0-based `line` as one undo step.
    pub fn toggle_task(&mut self, line: usize, done: bool) {
        let Some(a) = self.active.clone() else { return };
        if let Some((r, with)) = crate::blocks::toggle_task_splice(self.active_text(), line, done) {
            self.splice_note(&a, r, with);
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
        if val.is_empty() || np == old {
            self.focus_editor(tx);
            return;
        }
        if exists {
            self.notice = Some(format!("A note named “{val}” already exists."));
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

    /// Shows `to`, keeping the place: the mode buttons, the palette and the mode keys
    /// (MAP-064, MAP-065).
    pub fn show_mode(&mut self, to: Mode, tx: &mut Tx) {
        if self.active.is_none() || self.mode == to {
            return;
        }
        self.mode_swapped = true;
        if self.mode == Mode::Board {
            self.leave_board(to, tx);
        } else if to == Mode::Board {
            self.enter_board(tx);
        } else if self.mode == Mode::Map {
            self.leave_map(to, tx);
        } else if to == Mode::Map {
            self.enter_map(tx);
        } else {
            self.switch_mode(tx);
        }
        self.persist_later();
    }


    /// The zoom keys: a step up or down omarky's ladder, or `None` back to 14 px.
    pub fn zoom(&mut self, up: Option<bool>, tx: &mut Tx) {
        let size = self.settings().size;
        self.set_text_size(up.map_or(crate::store::SIZE_DEFAULT, |up| crate::store::zoom_step(size, up)), tx);
    }

    /// A new text size. A map shown lands it at once (MAP-034): Rune's layout motion would move
    /// each fold ring on its own while each branch's curve jumped, so they'd part mid-flight.
    pub fn set_text_size(&mut self, size: u32, tx: &mut Tx) {
        self.settings_mut().size = size;
        if self.mode == Mode::Map {
            tx.instant();
        }
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
        if self.map_format(f) {
            return;
        }
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
        self.set_slide = 0;
        self.open_layer(crate::layer::Layer::Settings, tx);
    }

    pub fn set_settings_sec(&mut self, sec: Sec) {
        if self.set_sec == sec {
            return;
        }
        let pos = |target: Sec| SECS.iter().position(|(id, _)| *id == target).unwrap_or(0);
        self.set_slide = if pos(sec) > pos(self.set_sec) { 1 } else { -1 };
        self.set_gen += 1;
        self.set_sec = sec;
        self.key_edit = None;
    }

    pub fn open_search(&mut self, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        self.show_left(tx);
        let n = self.query.text.len();
        self.query.set_selection(0, n);
        self.query.focused = true;
        tx.focus("search", false);
    }

    /// The vault's tree, folders and notes in the tree's order, with each folder's count.
    pub fn vault_tree(&self) -> Option<std::rc::Rc<crate::folders::Folder>> {
        self.vault.as_ref().map(|v| self.tree.borrow_mut().of(v))
    }

    /// Search's results for the field's query, or None while it holds only spaces, when the
    /// lists show instead (SB-57). Note mode has no search.
    pub fn results(&self) -> Option<std::rc::Rc<crate::search::Results>> {
        let q = self.query.text.trim();
        let v = self.vault.as_ref().filter(|_| !q.is_empty() && !self.note_mode())?;
        Some(self.search.borrow_mut().results(v, q))
    }

    /// Opens a search result's note, or switches to its tab, in the current mode (SB-63,
    /// SB-64). A text result `at` its first match, as a byte offset and a line: in the writer
    /// the caret goes to the match's start with nothing selected; in preview the page goes to
    /// it and find marks it with the query, so Enter, `n` and `N` step through the others and
    /// Esc clears the mark.
    pub fn open_result(&mut self, path: &str, at: Option<(usize, usize)>, tx: &mut Tx) {
        let Some((pos, line)) = at else { return self.open_file(path, None, tx) };
        let q = self.query.text.trim().to_string();
        let seed = {
            let text_low = self.note_text(path).unwrap_or("").to_lowercase();
            if text_low.contains(&q.to_lowercase()) {
                q.clone()
            } else {
                q.split_whitespace().find(|t| text_low.contains(&t.to_lowercase())).unwrap_or(&q).to_string()
            }
        };
        if self.mode == Mode::Source {
            self.open_file(path, Some(Sel { start: pos, end: pos, line: Some(line), force_source: false }), tx);
            {
                let f = &mut self.reader.find;
                f.open = true;
                f.q = TextEdit::new(seed.clone());
                f.searched = seed;
                f.due = None;
            }
            let m = self.source_matches();
            self.reader.find.cur = m.iter().position(|r| r.start >= pos).unwrap_or(0);
            return;
        }
        self.open_file(path, None, tx);
        let f = &mut self.reader.find;
        f.open = true;
        f.q = TextEdit::new(seed.clone());
        f.searched = seed;
        f.due = None;
        f.cur = 0;
        self.show_match(false, tx);
    }

    /// Chooses the result keyed `key`, by a click, Enter or Space on its row or Enter in the
    /// field (SB-63, SB-64, SB-66, SB-88). A note's row opens it in the current mode and gives
    /// it the keyboard, closing the drawer, even when it showed already (SB-122). A tag's row
    /// lists the tag's notes; chosen by keys, the keyboard goes to the first of them.
    pub fn choose_result(&mut self, key: &str, tx: &mut Tx) {
        use crate::search::Row;
        let Some(r) = self.results() else { return };
        let Some(row) = r.rows().find(|row| row.key() == key) else { return };
        if let Row::Tag(tag, _) = row {
            self.search_tag(tag, tx);
            let first = self.results().and_then(|r| r.tagged.first().map(|p| Row::Tagged(p).key()));
            if let Some(k) = first.filter(|_| tx.by_keys()) {
                self.query.focused = false;
                tx.focus(k, true);
            }
            return;
        }
        let at = match row {
            Row::Text(h) => Some((h.first.pos, h.first.line)),
            _ => None,
        };
        self.open_result(row.note().unwrap_or_default(), at, tx);
        self.close_drawer(tx);
        self.keys_to_note(tx);
    }

    /// Esc in search, from the field or a result (SB-89, SB-90, SB-92, SB-93): a showing
    /// notice goes first; then a query clears, and the keyboard is in the field with its ring,
    /// docked or in the drawer. False when the field was empty already, for the caller's own
    /// Esc: the note takes the keyboard, or the drawer closes.
    pub fn search_escape(&mut self, tx: &mut Tx) -> bool {
        if self.esc_takes_notice() {
            return true;
        }
        if self.query.text.is_empty() {
            return false;
        }
        self.query = TextEdit::new("");
        self.query.focused = true;
        tx.focus("search", true);
        true
    }

    /// The footer's tags button (SB-74, SB-75): every tag, as the search for "#", with the
    /// keyboard in the field and the caret after it, so typing narrows them. While the tags
    /// show it clears the field instead, and a keyboard on a result goes to the field.
    pub fn toggle_tags(&mut self, tx: &mut Tx) {
        if self.query.text.trim_start().starts_with('#') {
            self.query = TextEdit::new("");
            if tx.focused().is_some_and(|k| k.starts_with("res-")) {
                self.query.focused = true;
                tx.focus("search", true);
            }
            return;
        }
        self.query = TextEdit::new("#");
        self.query.set_selection(1, 1);
        self.query.focused = true;
        tx.focus("search", false);
    }

    pub fn search_tag(&mut self, tag: &str, tx: &mut Tx) {
        if self.note_mode() {
            return;
        }
        self.show_left(tx);
        self.query = TextEdit::new(format!("#{tag}"));
        let n = self.query.text.len();
        self.query.set_selection(n, n);
        self.query.focused = true;
        tx.focus("search", false);
    }

    pub fn open_palette(&mut self, mode: PalMode, tx: &mut Tx) {
        self.open_layer(crate::layer::Layer::Palette(Palette { mode, q: TextEdit::new(""), idx: 0, move_path: None }), tx);
        tx.focus("pal-input", false);
    }

    /// Opens the folder-picker palette to move `note_path` into another folder.
    pub fn open_move_picker(&mut self, note_path: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() || !self.files().contains_key(note_path) {
            return;
        }
        self.open_layer(
            crate::layer::Layer::Palette(Palette {
                mode: PalMode::MoveFolder,
                q: TextEdit::new(""),
                idx: 0,
                move_path: Some(note_path.to_string()),
            }),
            tx,
        );
        tx.focus("pal-input", false);
    }

    /// Esc with the keyboard on a stop or in a field of the page, or anywhere in a drawer: a
    /// showing notice goes first, as it does from the note, and the keyboard stays where it was;
    /// the next Esc closes the drawer. Any other open layer (the palette, Settings, help, the
    /// lock dialog, the edit sheet, a menu, a menu over a drawer) takes Esc before the notice,
    /// so under one this does nothing.
    pub(crate) fn esc_takes_notice(&mut self) -> bool {
        if matches!(self.layer().map(Layer::kind), None | Some(crate::layer::Kind::Drawer(_))) && self.notice.take().is_some() {
            self.notice_open = None;
            return true;
        }
        false
    }

    /// Closes inline page prompts (new folder, folder rename, note rename, vault create).
    pub(crate) fn escape_prompts(&mut self, tx: &mut Tx) -> bool {
        if self.new_folder.is_some() {
            self.new_folder = None;
        } else if self.renaming_folder.is_some() {
            self.renaming_folder = None;
        } else if self.renaming.is_some() {
            self.cancel_rename(tx);
        } else if self.creating.is_some() {
            self.creating = None;
            tx.focus("create", tx.by_keys());
        } else {
            return false;
        }
        true
    }

    /// Esc on the page, with no layer open (an open layer takes Esc first, `keys::on_key`):
    /// the notice goes, then new folder, rename and the create prompt close, then zen mode ends.
    /// The keyboard stays where it was.
    pub fn escape(&mut self, tx: &mut Tx) -> bool {
        if self.mode == Mode::Board && self.board_escape_hold(tx) {
            return true;
        }
        if self.notice.take().is_some() {
            self.notice_open = None;
            return true;
        }
        if self.escape_prompts(tx) {
            return true;
        }
        if self.focus {
            self.focus = false;
            return true;
        }
        false
    }

    /// Toggles the inline line-by-line conflict diff view for the active sync conflict copy.
    pub fn toggle_conflict_diff(&mut self) {
        let Some(active) = self.active.clone() else {
            self.conflict_diff = None;
            return;
        };
        if self.conflict_diff.as_deref() == Some(active.as_str()) {
            self.conflict_diff = None;
        } else {
            self.conflict_diff = Some(active);
        }
    }

    /// Opens the original note for a sync conflict copy, or shows a notice if it was deleted.
    pub fn open_conflict_original(&mut self, orig: &str, tx: &mut Tx) {
        if self.files().contains_key(orig) {
            self.open_file(orig, None, tx);
        } else {
            self.notice = Some(format!("Original note {orig} does not exist."));
            self.notice_open = None;
        }
    }

    /// Replaces `orig` with the active conflict copy's content (as an undoable edit when open),
    /// writes atomically to disk, opens `orig`, and moves `conflict_rel` to `.trash`.
    pub fn keep_conflict_version(&mut self, conflict_rel: &str, orig: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        if self.note_locked(conflict_rel) || self.note_locked(orig) {
            self.notice = Some("Unlock both notes before resolving this conflict.".into());
            self.notice_open = None;
            return;
        }
        let Some(text) = self.note_text(conflict_rel).map(str::to_string) else { return };
        let prev_orig = self.files().get(orig).cloned();
        if prev_orig.is_some() {
            let old_len = self.note_text(orig).map_or(0, str::len);
            self.splice_note(orig, 0..old_len, &text);
        } else {
            if let Some(v) = &mut self.vault {
                v.put(orig.to_string(), text);
                v.born.insert(orig.to_string(), now_ns());
                let mut d = md::dir(orig).to_string();
                while !d.is_empty() {
                    if !v.folders.contains(&d) {
                        v.folders.push(d.clone());
                    }
                    d = md::dir(&d).to_string();
                }
                v.folders.sort();
            }
            self.mark_dirty(orig);
            self.rebuild_names();
        }
        self.flush_notes();
        self.open_file(orig, None, tx);
        self.delete_path(conflict_rel);
        if let Some(d) = &mut self.deleted {
            d.custom_msg = Some(format!("Kept version in {}", md::base(orig)));
            d.overwrite = Some((orig.to_string(), prev_orig.clone()));
            // On Undo, stay on `orig` if it existed before, otherwise reopen the conflict copy.
            d.active = if prev_orig.is_some() { Some(orig.to_string()) } else { Some(conflict_rel.to_string()) };
        }
    }

    /// Moves a sync conflict copy to `.trash` and switches to `orig` if it exists.
    pub fn discard_conflict_version(&mut self, conflict_rel: &str, orig: &str, tx: &mut Tx) {
        if self.refuse_in_note_mode() {
            return;
        }
        if self.files().contains_key(orig) {
            self.open_file(orig, None, tx);
        }
        self.delete_path(conflict_rel);
    }

    /// Moves the caret, the preview or the map to line `line`. On the map its node is selected
    /// and centred, and the map gets the keyboard (MAP-067).
    pub fn jump(&mut self, line: usize, tx: &mut Tx) {
        // From the outline's drawer: the page moves, and the drawer gives it back.
        self.close_layers(tx);
        match self.mode {
            Mode::Source => self.source_jump(line, tx),
            Mode::Preview => self.jump_block(line, tx),
            Mode::Map => {
                self.map_go_line(line, tx);
                self.keys_to_note(tx);
            }
            Mode::Board => self.board_jump(line, tx),
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
