//! Shortcut labels and the window's shortcuts.

use rune::{KeyEvent, Tx};

use crate::app::{App, Screen};
use crate::keymap::{self, Ctx};

pub fn is_mac() -> bool {
    cfg!(target_os = "macos")
}

/// A shortcut label as the platform writes it: `⌘⇧F` on macOS, `Ctrl+Shift+F` elsewhere.
pub fn k(keys: &[&str]) -> String {
    let mac = is_mac();
    let parts: Vec<String> = keys
        .iter()
        .map(|x| match (*x, mac) {
            ("Mod", true) => "⌘".into(),
            ("Shift", true) => "⇧".into(),
            ("Alt", true) => "⌥".into(),
            ("Mod", false) => "Ctrl".into(),
            (s, _) if s.chars().count() == 1 => s.to_uppercase(),
            (s, _) => s.into(),
        })
        .collect();
    if mac { parts.concat() } else { parts.join("+") }
}

pub fn mod_click() -> String {
    format!("{}-click", if is_mac() { "⌘" } else { "Ctrl" })
}

/// The window's shortcuts (`AppSpec::on_key`). Keys reach this when the focused control
/// does not take them. Each context on top of the window gets the key in turn (KEY-02), and
/// the first action with a handler runs.
pub fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let preset = s.preset();
    let vault = s.screen == Screen::Vault;
    let stack: &[Ctx] = if !vault || s.settings_open {
        &[Ctx::Global]
    } else if s.palette.is_some() {
        // An editing key never changes the note behind the palette.
        &[Ctx::Palette, Ctx::Global]
    } else if s.mode == crate::app::Mode::Preview {
        &[Ctx::Reader, Ctx::Global]
    } else {
        &[Ctx::Writer, Ctx::Global]
    };
    for ctx in stack {
        for a in keymap::lookup(preset, *ctx, ev) {
            // Away from the notes, only Esc and Quit act.
            if !vault && !matches!(a.id, "app.escape" | "app.quit") {
                continue;
            }
            if a.id == "app.escape" {
                return s.escape(tx);
            }
            if let Some(run) = handler(a.id) {
                run(s, tx);
                return true;
            }
        }
    }
    false
}

type Run = fn(&mut App, &mut Tx);

/// What an action does, or None while the feature it belongs to is not built yet, so its key
/// falls through. Mouse-only actions run from their pointer handlers instead.
pub fn handler(id: &str) -> Option<Run> {
    use crate::app::PalMode;
    use crate::format::Fmt;
    fn close_tab(s: &mut App, _: &mut Tx) {
        if let Some(a) = s.active.clone() {
            s.close_tab(&a);
        }
    }
    Some(match id {
        "app.command-palette" => |s, tx| s.open_palette(PalMode::Commands, tx),
        "app.quick-switcher" | "writer.recent-notes" => |s, tx| s.open_palette(PalMode::Files, tx),
        "app.search-notes" => |s, tx| s.open_search(tx),
        "app.toggle-sidebar" => |s, _| s.toggle_left(),
        "app.toggle-outline" | "reader.toggle-outline" => |s, _| s.toggle_right(),
        "app.toggle-mode" => |s, tx| s.toggle_mode(tx),
        "app.settings" => |s, tx| s.toggle_settings(tx),
        "app.focus-mode" => |s, tx| s.toggle_focus(tx),
        "app.new-note" => |s, tx| s.new_note(None, tx),
        // macOS quits from its app menu.
        "app.quit" if !cfg!(target_os = "macos") => |s, tx| s.quit(tx),
        "app.insert-image" => |s, tx| s.pick_image(tx),
        "app.save" => |s, _| {
            s.flush_notes();
            s.notice = Some("Saved".into());
        },
        "app.close-tab" | "reader.close-tab" => close_tab,
        "app.zoom-in" => |s, _| s.zoom(Some(true)),
        "app.zoom-out" => |s, _| s.zoom(Some(false)),
        "reader.zoom-reset" => |s, _| s.zoom(None),
        "app.escape" => |s, tx| {
            s.escape(tx);
        },
        "writer.bold" => |s, tx| s.format(Fmt::Bold, tx),
        "writer.italic" => |s, tx| s.format(Fmt::Italic, tx),
        "writer.inline-code" => |s, tx| s.format(Fmt::Code, tx),
        "writer.strikethrough" => |s, tx| s.format(Fmt::Strike, tx),
        "writer.link" => |s, tx| s.format(Fmt::Link, tx),
        // The editor follows a Ctrl-clicked link itself; this entry lists it in Settings.
        "writer.follow-link-click" => |_, _| {},
        _ => return None,
    })
}
