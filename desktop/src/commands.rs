//! The command palette and quick switcher: which items show, and what they do.

use rune::Tx;

use crate::app::{App, Mode, PalMode};
use crate::md;

pub type Run = fn(&mut App, &mut Tx);

pub struct Cmd {
    pub label: String,
    /// The keymap action whose chord the palette shows, or "".
    pub action: &'static str,
    pub run: Run,
}

fn cmd(label: impl Into<String>, action: &'static str, run: Run) -> Cmd {
    Cmd { label: label.into(), action, run }
}

/// The names a command goes by both in the palette and on its button's tooltip, each the
/// action a press performs in the state the app is in, so a user meets one vocabulary (TN).
pub fn show_name(m: Mode) -> &'static str {
    match m {
        Mode::Source => "Show source",
        Mode::Preview => "Show preview",
        Mode::Map => "Show map",
    }
}

/// The action whose key switches to `m`: each mode has its own idempotent chord.
pub fn mode_action(m: Mode) -> &'static str {
    match m {
        Mode::Source => "app.mode-source",
        Mode::Preview => "app.mode-preview",
        Mode::Map => "app.mode-map",
    }
}

pub fn show(m: Mode) -> Run {
    match m {
        Mode::Source => |s, tx| s.show_mode(Mode::Source, tx),
        Mode::Preview => |s, tx| s.show_mode(Mode::Preview, tx),
        Mode::Map => |s, tx| s.show_mode(Mode::Map, tx),
    }
}

pub fn zen_name(s: &App) -> &'static str {
    if s.focus { "Leave zen mode" } else { "Enter zen mode" }
}

pub fn sidebar_name(s: &App) -> &'static str {
    if s.sidebar_open() { "Hide sidebar" } else { "Show sidebar" }
}

pub fn outline_name(s: &App) -> &'static str {
    if s.outline_open() { "Hide outline" } else { "Show outline" }
}

pub const INSERT_IMAGE: &str = "Insert image…";

/// The lock's name for a note that is unlocked now (it locks again) or plain (it gets a
/// password).
pub fn lock_name(unlocked: bool) -> &'static str {
    if unlocked { "Lock note now" } else { "Lock note with a password…" }
}

pub fn commands(s: &App, dark: bool) -> Vec<Cmd> {
    let has = s.active.is_some();
    let mut v = vec![
        cmd("New note", "app.new-note", |s, tx| s.new_note(None, tx)),
        cmd("New folder", "", |s, tx| s.start_new_folder("", tx)),
        cmd("Open today's journal entry", "", |s, tx| s.open_today(tx)),
        cmd("Open quick switcher", "app.quick-switcher", |s, tx| s.quick_open(tx)),
    ];
    if has {
        for m in [Mode::Source, Mode::Preview, Mode::Map] {
            if m != s.mode {
                v.push(cmd(show_name(m), mode_action(m), show(m)));
            }
        }
    }
    v.push(cmd("Search all notes", "app.search-notes", |s, tx| s.open_search(tx)));
    if has && !s.active_locked() {
        let (find_action, find_run): (&'static str, Run) = match s.mode {
            Mode::Preview => ("reader.find", |s, tx| s.open_find(tx)),
            Mode::Map => ("map.find", |s, tx| s.open_find(tx)),
            Mode::Source => ("writer.find", |s, tx| s.writer_find(tx)),
        };
        v.push(cmd("Find in note", find_action, find_run));
        if s.mode == Mode::Source {
            v.push(cmd("Find and replace", "writer.replace", |s, tx| s.writer_replace(tx)));
        }
    }
    if has {
        v.push(cmd(zen_name(s), "app.focus-mode", |s, tx| s.toggle_focus(tx)));
    }
    v.push(cmd(sidebar_name(s), "app.toggle-sidebar", |s, tx| s.toggle_left(tx)));
    v.push(cmd(outline_name(s), "app.toggle-outline", |s, tx| s.toggle_right(tx)));
    if dark {
        v.push(cmd("Use light theme", "", |s, _| s.set_theme("light")));
    } else {
        v.push(cmd("Use dark theme", "", |s, _| s.set_theme("dark")));
    }
    if has {
        v.push(cmd("Rename note", "", |s, tx| s.start_rename(tx)));
        v.push(cmd("Delete note", "", |s, _| s.delete_note()));
        v.push(cmd("Close tab", "app.close-tab", |s, _| {
            if let Some(a) = s.active.clone() {
                s.close_tab(&a);
            }
        }));
    }
    if !s.closed_tabs.is_empty() || !s.note_mode() {
        v.push(cmd("Reopen closed tab", "app.reopen-tab", |s, tx| s.reopen_tab(tx)));
    }
    if s.sync_ready() {
        v.push(cmd("Sync now", "", |s, _| s.sync_now()));
    }
    if has && s.mode == Mode::Map && !s.active_locked() {
        v.push(cmd("Choose map layout…", "map.layout", |s, tx| s.open_layouts(tx)));
    }
    if s.active.is_some() {
        v.push(cmd(INSERT_IMAGE, "app.insert-image", |s, tx| s.pick_image(tx)));
    }
    if let Some(a) = s.active.as_deref() {
        if s.note_unlocked(a) {
            v.push(cmd(lock_name(true), "", |s, tx| {
                s.relock();
                tx.focus(crate::ui::lock::PASS, false);
            }));
            v.push(cmd("Remove password", "", |s, tx| s.ask_remove_password(tx)));
        } else if !s.note_locked(a) {
            v.push(cmd(lock_name(false), "", |s, tx| s.ask_lock(tx)));
        }
    }
    if !s.unlocked.is_empty() {
        v.push(cmd("Lock all notes", "", |s, tx| {
            s.lock_all_notes();
            s.toast("Locked all notes", tx);
        }));
    }
    let help_action = match s.mode {
        Mode::Preview => "reader.help",
        Mode::Map => "map.help",
        Mode::Source => "app.help",
    };
    v.push(cmd("Keyboard shortcuts", help_action, |s, tx| s.toggle_help(tx)));
    v.push(cmd("Open trash…", "", |s, tx| s.open_trash(tx)));
    v.push(cmd("Open settings", "app.settings", |s, tx| s.open_settings(tx)));
    if !cfg!(target_os = "macos") {
        v.push(cmd("Quit", "app.quit", |s, tx| s.quit(tx)));
    }
    v.push(cmd("Switch folder…", "", |s, tx| s.go_welcome(tx)));
    v.push(cmd("Restore sample notes", "", |s, tx| s.reset_sample(tx)));
    if s.note_mode() {
        // Note mode has no folder of notes to add to, search, rename or delete in.
        const FOLDER_ONLY: [&str; 7] = ["New note", "New folder", "Open today's journal entry", "Search all notes", "Rename note", "Delete note", "Open trash…"];
        v.retain(|c| !FOLDER_ONLY.contains(&c.label.as_str()));
        for c in &mut v {
            match c.label.as_str() {
                "Open quick switcher" => c.label = "Open a note…".into(),
                "Close tab" => c.label = "Close note".into(),
                _ => {}
            }
        }
    }
    v
}

#[derive(Clone)]
pub enum Action {
    Open(String),
    Create(String),
    Cmd(Run),
}

pub struct Item {
    /// Label pieces; true marks matched characters.
    pub segs: Vec<(String, bool)>,
    pub sub: String,
    pub action: Action,
}

pub struct PalData {
    pub mode: PalMode,
    pub items: Vec<Item>,
    pub idx: usize,
}

/// Splits `s` into runs of matched and unmatched characters.
pub fn segs(s: &str, idx: &[usize]) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (i, c) in s.chars().enumerate() {
        let hit = idx.contains(&i);
        match out.last_mut() {
            Some((t, h)) if *h == hit => t.push(c),
            _ => out.push((c.to_string(), hit)),
        }
    }
    out
}

pub fn pal_data(s: &App, dark: bool) -> Option<PalData> {
    let p = s.pal()?;
    Some(pal_data_for(s, dark, p.mode, &p.q.text, p.idx))
}

/// Builds palette items for a given mode, query, and index (shared with idle pre-warming).
pub fn pal_data_for(s: &App, dark: bool, mut mode: PalMode, raw_q: &str, raw_idx: usize) -> PalData {
    let mut q = raw_q;
    if mode == PalMode::Files && q.starts_with('>') {
        mode = PalMode::Commands;
        q = &q[1..];
    }
    let q = q.trim();
    let mut items = Vec::new();
    if mode == PalMode::Files {
        let files = s.files();
        let list: Vec<(String, Vec<usize>)> = if q.is_empty() {
            let rec: Vec<&String> = s.recent.iter().filter(|x| files.contains_key(*x)).collect();
            let mut rest: Vec<&String> = files.keys().filter(|x| !rec.contains(x)).collect();
            rest.sort();
            rec.into_iter().chain(rest).map(|x| (x.clone(), vec![])).collect()
        } else {
            let mut scored: Vec<(f32, String, Vec<usize>)> = files
                .keys()
                .filter_map(|x| {
                    if let Some((sc, idx)) = md::fuzzy(q, md::base(x)) {
                        return Some((sc + 500.0, x.clone(), idx));
                    }
                    md::fuzzy(q, md::strip_md(x)).map(|(sc, _)| (sc, x.clone(), vec![]))
                })
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.into_iter().map(|(_, x, i)| (x, i)).collect()
        };
        for (x, idx) in list.into_iter().take(50) {
            items.push(Item { segs: segs(md::base(&x), &idx), sub: md::dir(&x).to_string(), action: Action::Open(x) });
        }
        if !q.is_empty() && !files.keys().any(|x| md::base(x).to_lowercase() == q.to_lowercase()) {
            items.push(Item { segs: vec![(format!("Create note “{q}”"), false)], sub: "new".into(), action: Action::Create(q.to_string()) });
        }
    } else {
        let cmds = commands(s, dark);
        let mut list: Vec<(f32, Cmd, Vec<usize>)> = if q.is_empty() {
            cmds.into_iter().map(|c| (0.0, c, vec![])).collect()
        } else {
            cmds.into_iter().filter_map(|c| md::fuzzy(q, &c.label).map(|(sc, idx)| (sc, c, idx))).collect()
        };
        if !q.is_empty() {
            list.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        for (_, c, idx) in list {
            let sub = s.keymap.label(c.action);
            items.push(Item { segs: segs(&c.label, &idx), sub, action: Action::Cmd(c.run) });
        }
    }
    let idx = raw_idx.min(items.len().saturating_sub(1));
    PalData { mode, items, idx }
}

/// Runs a row: the palette closes first, so a command that opens a layer opens it in the
/// palette's place.
pub fn run(s: &mut App, action: Action, tx: &mut Tx) {
    s.close_layer(tx);
    match action {
        Action::Open(p) => s.open_file(&p, None, tx),
        Action::Create(q) => s.new_note(Some(&q), tx),
        Action::Cmd(f) => f(s, tx),
    }
}
