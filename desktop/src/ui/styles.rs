//! The chrome's text styles (PAGE-09). The chrome sets its text in two families, each with one
//! role:
//!
//! - The panel family, the settings file's `panel_font` (SB-07): iA Writer Quattro S, the
//!   note's face, by default, or fontconfig's `monospace` as omarky reads it (decision 0008).
//!   It is for navigation: the side panels, their captions and footers, and the menus. Key names and
//!   literal values (paths, addresses, tokens, configuration) take it wherever they appear, so
//!   a key looks the same in the footer, the palette and the shortcut list. Tooltips take it
//!   too, since most name a key: "Hide outline (Ctrl+\)" is one line in one family.
//! - Instrument Sans is for the rest: the palette, the quick switcher, Settings, the lock
//!   dialog, help, the edit sheet, toasts, the title bar, the find bar, the welcome screen and
//!   empty states.
//!
//! Every chrome text takes one of the styles below, and `ui_builders_take_chrome_styles` keeps
//! any other family out of `ui/`. The note's own type (the page and the editor) is not chrome:
//! the page has its own looks in `preview/`, and `note` names the rest.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use rune::{Color, El, Family, TextStyle, text};

use crate::roles::Roles;
use crate::theme::{Pal, SANS, normal_lh, px};

/// Whether the panel family is the system's monospace, set once from the settings file.
static MONO: AtomicBool = AtomicBool::new(false);

/// Sets the panel family from the settings file's `panel_font`. Any value but `mono` is
/// Quattro, the default.
pub fn set_panel_font(id: &str) {
    MONO.store(id == "mono", Ordering::Relaxed);
}

/// The panel family: the settings file's choice (`set_panel_font`).
pub fn panel_family() -> Family {
    panel_family_of(if MONO.load(Ordering::Relaxed) { "mono" } else { "quattro" })
}

/// The panel family for a `panel_font` value: `mono` is the system's monospace, any other
/// value iA Writer Quattro S.
pub fn panel_family_of(id: &str) -> Family {
    if id == "mono" { system_mono() } else { Family::Named("iA Writer Quattro S") }
}

/// The family fontconfig's `monospace` resolves to (JetBrains Mono on Omarchy), read once, as
/// omarky reads it. Named rather than `Family::SystemMono`, which loads only the regular face.
fn system_mono() -> Family {
    static F: OnceLock<&'static str> = OnceLock::new();
    Family::Named(F.get_or_init(|| {
        std::process::Command::new("fc-match")
            .args(["-f", "%{family[0]}", "monospace"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map_or("IBM Plex Mono", |s| Box::leak(s.into_boxed_str()))
    }))
}

/// The panel family's line as Qt sets it for JetBrains Mono: ascent 1.02 em and descent 0.3 em,
/// each rounded.
pub fn panel_line(size: f32) -> f32 {
    px(1.02 * size) + px(0.3 * size)
}

fn panel(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(panel_family()).lh(panel_line(size))
}

/// Navigation text: the side panels' rows, captions and footers, and the menus.
pub fn nav(size: f32, bold: bool) -> TextStyle {
    panel(size, if bold { 700.0 } else { 400.0 })
}

/// Navigation text at weight 600: a folder's name in the tree (SB-55, `TYPOPOLISH`). Weight 500
/// fell back to 400 on Quattro S; 600 resolves to Bold on 400/700 families (and SemiBold where
/// available), giving folders clear visual hierarchy over regular-weight note rows.
pub fn medium(size: f32) -> TextStyle {
    panel(size, 600.0)
}

/// A key name, such as Ctrl+N, wherever it shows.
pub fn keys(size: f32) -> TextStyle {
    panel(size, 400.0)
}

/// A tooltip's text: its name and the key that does it, on one line in the panel family.
pub fn tooltip() -> TextStyle {
    panel(11.5, 400.0)
}

/// A literal value: a path, an address, a token, a name the program reads, configuration.
pub fn literal(size: f32) -> TextStyle {
    panel(size, 400.0)
}

/// Instrument Sans with CSS `line-height: normal`: dialogs, bars and everything else.
pub fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(SANS).lh(normal_lh(SANS, size))
}

/// A popup's message (RD-98): the toast, its Undo and the notice. Menus are navigation and
/// take `nav`; a tooltip names a key and takes `tooltip`; a key name inside a popup takes `keys`.
pub fn popup() -> TextStyle {
    body()
}

/// Every caption's size, in both families: omarky's caption size.
pub const CAPTION: f32 = scale::CAPTION;

/// Named chrome type scale (LK-07, LK-08): six steps plus the palette command field.
pub mod scale {
    pub const CAPTION: f32 = 10.0;
    pub const SMALL: f32 = 11.5;
    pub const CONTROL: f32 = 12.5;
    pub const BODY: f32 = 13.0;
    pub const ROW_TITLE: f32 = 14.0;
    pub const PALETTE_INPUT: f32 = 15.0;
    pub const TITLE: f32 = 17.0;
}

/// Dialog and section heading (17 pt semibold Instrument Sans).
pub fn title() -> TextStyle {
    sans(scale::TITLE, 600.0)
}

/// Settings row title and command palette row label (14 pt medium Instrument Sans).
pub fn row_title() -> TextStyle {
    sans(scale::ROW_TITLE, 500.0)
}

/// Dialog body text and toast/notice popup text (13 pt regular Instrument Sans).
pub fn body() -> TextStyle {
    sans(scale::BODY, 400.0)
}

/// Single-line text field content (13 pt regular Instrument Sans).
pub fn field() -> TextStyle {
    sans(scale::BODY, 400.0)
}

/// Command palette search input (15 pt regular Instrument Sans).
pub fn palette_input() -> TextStyle {
    sans(scale::PALETTE_INPUT, 400.0)
}

/// Controls and buttons (12.5 pt regular or medium Instrument Sans).
pub fn control() -> TextStyle {
    sans(scale::CONTROL, 400.0)
}

pub fn control_medium() -> TextStyle {
    sans(scale::CONTROL, 500.0)
}

/// Secondary descriptions under row titles and dialog headings (12.5 pt at 1.45 leading).
pub fn desc() -> TextStyle {
    sans(scale::CONTROL, 400.0).lh(scale::CONTROL * 1.45)
}

/// Small UI labels, hints, badges and secondary metadata (11.5 pt regular Instrument Sans).
pub fn small() -> TextStyle {
    sans(scale::SMALL, 400.0)
}

/// Readable error text colour on `p.bg` (`>= 5.0:1` across every theme, LK-09).
pub fn error_ink(p: &Pal) -> Color {
    Roles::new(p).readable_err()
}

/// A caption's capitals: bold, 1 px apart, at `CAPTION`, in the panel family in navigation
/// (`nav`) and in Instrument Sans elsewhere, with no colour. The panel tabs take it with their
/// own colours; a section label takes `caption`.
pub fn caps(t: &str, nav_: bool) -> El {
    let style = if nav_ { nav(CAPTION, true) } else { sans(CAPTION, 600.0) };
    text(t.to_uppercase()).font(style.tracking(1.0))
}

/// A section label (C15, HC): `caps` in the caption ink. Every section label takes it.
pub fn caption(t: &str, nav_: bool, p: &Pal) -> El {
    caps(t, nav_).color(caption_ink(p))
}

/// The ink of every section label: the side panels' faint, which reaches 4.5:1 on the window,
/// the sidebar and the outline's wash, the fills every panel and dialog sits on.
pub fn caption_ink(p: &Pal) -> Color {
    Roles::panel(p).faint
}

/// The ink of text field placeholders across panels, bars and dialogs: reaches 4.5:1 on the
/// window, sidebar and card surfaces in every theme.
pub fn placeholder_ink(p: &Pal) -> Color {
    Roles::panel(p).faint
}

/// The note's own type in `family`, not chrome: the editor's field and Settings' font samples.
pub fn note(family: Family, size: f32, lh: f32) -> TextStyle {
    TextStyle::new(size, 400.0).family(family).lh(lh)
}

/// Whether every ASCII character of `family` is exactly 0.6 em wide, so `components::label` can
/// give a run its CSS width.
pub fn fixed_advance(family: Family) -> bool {
    matches!(family, Family::Named("IBM Plex Mono" | "Courier Prime" | "Spline Sans Mono"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SB-07: the panel family is the settings file's `panel_font`, Quattro by default and the
    /// system's monospace for `mono`; a settings file without it opens with Quattro.
    #[test]
    fn the_panel_font_is_a_setting() {
        assert_eq!(crate::store::Settings::default().panel_font, "quattro");
        assert_eq!(panel_family_of("quattro"), Family::Named("iA Writer Quattro S"));
        assert_eq!(panel_family_of("anything else"), panel_family_of("quattro"), "an unknown value is the default");
        assert_ne!(panel_family_of("mono"), panel_family_of("quattro"));
        assert_eq!(panel_family_of("mono"), system_mono());
        let old: crate::store::Settings = serde_json::from_str(r#"{"font":"plex"}"#).unwrap();
        assert_eq!(old.panel_font, "quattro", "an older settings file takes the default");
    }

    /// C14: every chrome style is in one of the two families, and the two differ.
    #[test]
    fn chrome_styles_use_two_families() {
        let two = [panel_family(), SANS];
        assert_ne!(two[0], two[1]);
        for (name, s) in [("nav", nav(12.0, false)), ("nav bold", nav(12.0, true)), ("keys", keys(11.0)), ("literal", literal(12.0)), ("sans", sans(13.0, 400.0)), ("popup", popup()), ("tooltip", tooltip())] {
            assert!(two.contains(&s.family), "{name} is in {:?}, outside the chrome's two families", s.family);
        }
        assert_eq!(keys(11.0).family, nav(11.0, false).family, "key names take the panel family");
        assert_eq!(tooltip().family, keys(11.0).family, "a tooltip names its key in the key's family");
    }

    /// C14: a builder in `ui/` sets chrome text only through the styles above. The page's own
    /// builders (`preview.rs`, `preview/` and the copy chip) take their families from the page's
    /// look.
    #[test]
    fn ui_builders_take_chrome_styles() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let banned = ["TextStyle::new", ".family(", "Family::", "theme::mono", "theme::sans", "theme::serif", "theme::MONO", "theme::SANS", "theme::SERIF"];
        let mut stack = vec![ui.clone()];
        let mut found = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let rel = path.strip_prefix(&ui).unwrap().to_string_lossy().to_string();
                if path.is_dir() {
                    if rel != "preview" {
                        stack.push(path);
                    }
                    continue;
                }
                if ["styles.rs", "preview.rs"].contains(&rel.as_str()) {
                    continue;
                }
                let src = std::fs::read_to_string(&path).unwrap();
                for (i, line) in src.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    for b in banned {
                        if line.contains(b) {
                            found.push(format!("ui/{rel}:{} uses {b}", i + 1));
                        }
                    }
                }
            }
        }
        assert!(found.is_empty(), "chrome text outside ui/styles.rs:\n{}", found.join("\n"));
    }
}
