//! What Quire keeps between runs: settings, known folders and each folder's open tabs.
//! It lives in the user's config folder as `state.json`, never inside a notes folder.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// light, dark or system.
    pub theme: String,
    /// The theme for light mode. None picks Paper, or Omarchy's theme under "Match system".
    pub light_theme: Option<String>,
    /// The theme for dark mode. None picks Ink, or Omarchy's theme under "Match system".
    pub dark_theme: Option<String>,
    /// `#rrggbb`, or empty for the theme's own accent.
    pub accent: String,
    /// The source-mode font, an id in `theme::EDITOR_FONTS`.
    pub font: String,
    /// The body size in px, 9 to 32.
    pub size: u32,
    pub width: String,
    /// The preview's prose font, an id in `theme::PROSE_FONTS`.
    pub preview_font: String,
    /// Code in both modes, an id in `theme::CODE_FONTS`.
    pub code_font: String,
    /// The keymap preset, "omarky" or "quire" (`docs/reader-writer/behavior/keymap.md`).
    pub keymap: String,
    pub spellcheck: bool,
    pub continue_lists: bool,
    /// Lines outside the paragraph being written fade (WR-79).
    pub paragraph_focus: bool,
    /// The caret line stays at the middle of the view (WR-85).
    pub typewriter_scroll: bool,
    /// The formatting toolbar above the note, in source mode.
    pub toolbar: bool,
    pub update_links: bool,
    pub sync: Sync,
    pub mcp: Mcp,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            // omarky follows the Omarchy theme, and the system's mode elsewhere.
            theme: "system".into(),
            light_theme: None,
            dark_theme: None,
            accent: String::new(),
            font: "quattro".into(),
            size: SIZE_DEFAULT,
            width: "readable".into(),
            preview_font: "quattro".into(),
            code_font: "ia-mono".into(),
            keymap: "omarky".into(),
            spellcheck: false,
            continue_lists: true,
            paragraph_focus: false,
            typewriter_scroll: false,
            toolbar: true,
            update_links: true,
            sync: Sync::default(),
            mcp: Mcp::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sync {
    pub provider: String,
    /// Provider id to field key to value.
    pub cfg: BTreeMap<String, BTreeMap<String, String>>,
    pub when: String,
    pub encrypt: bool,
    pub pass: String,
    /// The last "Test connection" passed with these settings.
    pub connected: bool,
    /// When the last sync finished, in milliseconds since 1970.
    pub last_sync: Option<i64>,
    /// The remote folder of each notes folder, by its path. A notes folder without one uses
    /// `quire/<folder name>/`, so two notes folders never share a remote folder by accident.
    pub folders: BTreeMap<String, String>,
}

impl Default for Sync {
    fn default() -> Self {
        Sync { provider: "none".into(), cfg: BTreeMap::new(), when: "save".into(), encrypt: false, pass: String::new(), connected: false, last_sync: None, folders: BTreeMap::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Mcp {
    pub enabled: bool,
    pub transport: String,
    pub port: String,
    pub access: String,
    pub confirm: bool,
    /// Folders apps cannot see. An empty string is the folder root.
    pub hidden: Vec<String>,
    pub token: String,
    pub tools: BTreeMap<String, bool>,
}

pub const MCP_TOOLS: [(&str, &str, bool); 8] = [
    ("list_notes", "List notes and folders", false),
    ("read_note", "Read a note as Markdown", false),
    ("search_notes", "Full-text and tag search", false),
    ("get_backlinks", "Find notes that link to a note", false),
    ("create_note", "Create a new note", true),
    ("update_note", "Replace or append to a note", true),
    ("rename_note", "Rename a note and update links", true),
    ("delete_note", "Move a note to the trash", true),
];

impl Default for Mcp {
    fn default() -> Self {
        let on = ["list_notes", "read_note", "search_notes", "get_backlinks", "create_note", "update_note"];
        Mcp {
            enabled: false,
            transport: "stdio".into(),
            port: "3917".into(),
            access: "read".into(),
            confirm: true,
            hidden: Vec::new(),
            token: new_token(),
            tools: MCP_TOOLS.iter().map(|(id, _, _)| (id.to_string(), on.contains(id))).collect(),
        }
    }
}

impl Mcp {
    /// Whether a connected app may call `tool`, given the access level.
    pub fn allows(&self, tool: &str) -> bool {
        let write = MCP_TOOLS.iter().find(|t| t.0 == tool).is_some_and(|t| t.2);
        (!write || self.access == "write") && self.tools.get(tool).copied().unwrap_or(false)
    }
}

/// A random `qk_` token, 28 characters from an unambiguous alphabet.
pub fn new_token() -> String {
    let a = b"abcdefghijkmnopqrstuvwxyz23456789";
    let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
    seed ^= std::process::id() as u64;
    let mut bytes = [0u8; 28];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes)).is_err() {
        for b in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = seed as u8;
        }
    }
    let mut t = String::from("qk_");
    for b in bytes {
        t.push(a[b as usize % a.len()] as char);
    }
    t
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VaultRef {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub recent: Vec<String>,
    pub expanded: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Persist {
    pub vaults: Vec<VaultRef>,
    /// The folder open when Quire quit. None opens the welcome screen.
    pub last: Option<PathBuf>,
    pub sessions: BTreeMap<String, Session>,
    pub mode: String,
    #[serde(default = "yes")]
    pub left_open: bool,
    #[serde(default = "yes")]
    pub right_open: bool,
    pub settings: Settings,
    /// Set once the sample folder exists, so deleting it does not bring it back.
    pub seeded: bool,
    /// The sidebar's fold and handle, once for every folder (SB-46, SB-47).
    pub sidebar: crate::sidebar::Layout,
}

impl Default for Persist {
    fn default() -> Self {
        Persist {
            vaults: Vec::new(),
            last: None,
            sessions: BTreeMap::new(),
            mode: "source".into(),
            left_open: true,
            // omarky opens with its outline closed.
            right_open: false,
            settings: Settings::default(),
            seeded: false,
            sidebar: crate::sidebar::Layout::default(),
        }
    }
}

pub fn dirs() -> Option<rune::dirs::AppDirs> {
    rune::dirs::AppDirs::new("quire")
}

fn state_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("QUIRE_STATE") {
        return Some(PathBuf::from(p));
    }
    dirs().map(|d| d.config().join("state.json"))
}

pub const SIZE_MIN: u32 = 9;
pub const SIZE_MAX: u32 = 32;
pub const SIZE_DEFAULT: u32 = 14;

/// omarky's zoom ladder: round(14.4 × z) for z from 0.6 to 2.2 in steps of 0.1.
const ZOOM_LADDER: [u32; 17] = [9, 10, 12, 13, 14, 16, 17, 19, 20, 22, 23, 24, 26, 27, 29, 30, 32];

/// The next size on the zoom ladder above (`up`) or below the current one. A size set off
/// the ladder in Settings joins it here.
pub fn zoom_step(size: u32, up: bool) -> u32 {
    if up {
        ZOOM_LADDER.iter().copied().find(|&z| z > size).unwrap_or(SIZE_MAX)
    } else {
        ZOOM_LADDER.iter().rev().copied().find(|&z| z < size).unwrap_or(SIZE_MIN)
    }
}

pub fn load() -> Persist {
    let Some(p) = state_path() else { return Persist::default() };
    let mut st: Persist = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    st.settings.size = st.settings.size.clamp(SIZE_MIN, SIZE_MAX);
    st
}

/// Writes the state file atomically, readable only by the user (it can hold sync keys).
pub fn save(p: &Persist) -> std::io::Result<()> {
    let Some(path) = state_path() else { return Ok(()) };
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(p).unwrap_or_default())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path)
}

/// `~/Documents`, or the XDG documents folder on Linux.
pub fn documents_dir() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "linux") {
        if let Ok(conf) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) {
            for line in conf.lines() {
                if let Some(v) = line.strip_prefix("XDG_DOCUMENTS_DIR=") {
                    let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                    if !v.is_empty() {
                        return PathBuf::from(v);
                    }
                }
            }
        }
    }
    home.join("Documents")
}

/// A path with the home folder written as `~`.
pub fn tilde(p: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) if !h.as_os_str().is_empty() && p.starts_with(&h) => {
            let rest = p.strip_prefix(&h).unwrap_or(p);
            if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.to_string_lossy()) }
        }
        _ => p.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    /// PAGE-10, PAGE-20: the body is 14 px by default, and a stored size outside 9 to 32 is
    /// clamped on load.
    #[test]
    fn page_20_stored_size_is_clamped_on_load() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(super::Settings::default().size, 14);
        let path = std::env::temp_dir().join(format!("quire-size-{}.json", std::process::id()));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe { std::env::set_var("QUIRE_STATE", &path) };
        for (stored, loaded) in [(40, 32), (4, 9), (20, 20)] {
            std::fs::write(&path, format!(r#"{{"settings":{{"size":{stored},"font":"plex"}}}}"#)).unwrap();
            let st = super::load();
            assert_eq!(st.settings.size, loaded, "size {stored} loads as {loaded}");
            assert_eq!(st.settings.font, "plex");
            assert_eq!(st.settings.code_font, "ia-mono", "a missing code font takes the default");
        }
        unsafe { std::env::remove_var("QUIRE_STATE") };
        let _ = std::fs::remove_file(&path);
    }

    /// SB-46, SB-47, SB-135: a state written before the sidebar's two lists opens them split,
    /// with the shelf at its fitted height, and the fold and the handle's place are kept once
    /// for the app.
    #[test]
    fn an_old_state_opens_split() {
        use crate::sidebar::{Layout, Section};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let path = std::env::temp_dir().join(format!("quire-old-split-{}.json", std::process::id()));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe { std::env::set_var("QUIRE_STATE", &path) };
        std::fs::write(&path, r#"{"mode":"preview","left_open":true,"settings":{"font":"plex"}}"#).unwrap();
        let st = super::load();
        assert_eq!(st.sidebar, Layout::default(), "no fold and no stored height");
        assert_eq!(st.settings.font, "plex", "the rest still loads");
        let mut st = st;
        st.sidebar = Layout { folded: Some(Section::Vault), shelf_h: Some(212.0) };
        std::fs::write(&path, serde_json::to_vec(&st).unwrap()).unwrap();
        let back = super::load();
        assert_eq!(back.sidebar, st.sidebar, "the fold and the handle come back");
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains(r#""sidebar":{"folded":"vault","shelf_h":212.0}"#), "kept once for the app: {json}");
        unsafe { std::env::remove_var("QUIRE_STATE") };
        let _ = std::fs::remove_file(&path);
    }

    /// SB-13: a settings file that still names a new notes folder loads, and the folder is
    /// dropped when it is written again.
    #[test]
    fn an_old_new_folder_setting_still_loads() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let path = std::env::temp_dir().join(format!("quire-old-nf-{}.json", std::process::id()));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe { std::env::set_var("QUIRE_STATE", &path) };
        std::fs::write(&path, r#"{"settings":{"font":"plex","new_folder":"Inbox","update_links":false}}"#).unwrap();
        let st = super::load();
        assert_eq!((st.settings.font.as_str(), st.settings.update_links), ("plex", false), "the rest of the settings load");
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("new_folder"), "the folder is not written back: {json}");
        unsafe { std::env::remove_var("QUIRE_STATE") };
        let _ = std::fs::remove_file(&path);
    }

    /// PAGE-19: the zoom keys walk omarky's ladder, and an off-ladder size joins it.
    #[test]
    fn page_19_zoom_follows_the_ladder() {
        use super::zoom_step;
        let ladder: Vec<u32> = (6..=22).map(|z| (14.4 * z as f64 / 10.0).round() as u32).collect();
        assert_eq!(ladder, super::ZOOM_LADDER, "the ladder is round(14.4 × z)");
        assert_eq!((zoom_step(14, true), zoom_step(14, false)), (16, 13));
        assert_eq!((zoom_step(15, true), zoom_step(15, false)), (16, 14));
        assert_eq!((zoom_step(32, true), zoom_step(9, false)), (32, 9), "the ends hold");
        assert_eq!((zoom_step(11, true), zoom_step(11, false)), (12, 10));
    }
}
