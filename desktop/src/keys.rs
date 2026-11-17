//! Shortcut labels and the window's shortcuts.

use rune::{KeyEvent, Tx};

use crate::app::{App, Screen};
use crate::keymap::{self, Ctx};

pub fn is_mac() -> bool {
    cfg!(target_os = "macos")
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
    let stack: &[Ctx] = if s.sheet.is_some() {
        // The sheet is modal: only its own keys act, and every other key stops here
        // (SHEET-014, SHEET-036).
        &[Ctx::Sheet]
    } else if !vault || s.settings_open {
        &[Ctx::Global]
    } else if s.palette.is_some() {
        // An editing key never changes the note behind the palette.
        &[Ctx::Palette, Ctx::Global]
    } else if s.shown_drawer().is_some() {
        // A drawer holds the keyboard: the reader's and the writer's keys never reach the page
        // behind it.
        &[Ctx::Global]
    } else if s.reader.help {
        // Only the keys that close the sheet act; the rest are swallowed (RD-80).
        &[Ctx::Help]
    } else if s.mode == crate::app::Mode::Preview {
        &[Ctx::Reader, Ctx::Global]
    } else {
        &[Ctx::Writer, Ctx::Global]
    };
    for ctx in stack {
        if *ctx == Ctx::Reader {
            // The second key of `g g` (RD-23).
            let now = s.now.get();
            if let Some((first, at)) = s.reader.pending.take()
                && now - at < 0.9
                && let Some(run) = keymap::lookup_seq(preset, Ctx::Reader, &first, ev).iter().find_map(|a| handler(a.id))
            {
                run(s, tx);
                return true;
            }
            if keymap::starts_seq(preset, Ctx::Reader, ev) {
                s.reader.pending = Some((ev.clone(), now));
                return true;
            }
        }
        for a in keymap::lookup(preset, *ctx, ev) {
            // Away from the notes, only Esc and Quit act.
            if !vault && !matches!(a.id, "app.escape" | "app.quit") {
                continue;
            }
            if a.id == "app.escape" {
                return s.escape(tx);
            }
            if a.id == "reader.escape" {
                return s.reader_escape(tx);
            }
            if a.id == "writer.escape" {
                return s.writer_escape(tx);
            }
            if a.id == "sheet.cancel" {
                s.sheet_escape(tx);
                return true;
            }
            if let Some(run) = handler(a.id) {
                run(s, tx);
                return true;
            }
        }
    }
    s.reader.help || s.sheet.is_some()
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
        "app.quick-switcher" => |s, tx| s.quick_open(tx),
        "writer.recent-notes" => |s, tx| s.open_palette(PalMode::Files, tx),
        "app.search-notes" => |s, tx| s.open_search(tx),
        "app.toggle-sidebar" => |s, tx| s.toggle_left(tx),
        "app.toggle-outline" | "reader.toggle-outline" => |s, tx| s.toggle_right(tx),
        "app.toggle-mode" => |s, tx| s.toggle_mode(tx),
        "app.settings" => |s, tx| s.toggle_settings(tx),
        "app.focus-mode" => |s, tx| s.toggle_focus(tx),
        "app.new-note" => |s, tx| s.new_note(None, tx),
        // macOS quits from its app menu.
        "app.quit" if !cfg!(target_os = "macos") => |s, tx| s.quit(tx),
        "app.insert-image" => |s, tx| s.pick_image(tx),
        "app.save" => |s, tx| s.save_now(tx),
        "app.close-tab" | "reader.close-tab" => close_tab,
        "app.zoom-in" => |s, _| s.zoom(Some(true)),
        "app.zoom-out" => |s, _| s.zoom(Some(false)),
        "reader.zoom-reset" => |s, _| s.zoom(None),
        "app.undo-delete" => |s, tx| s.undo_delete(tx),
        "app.escape" => |s, tx| {
            s.escape(tx);
        },
        "reader.scroll-down" => |s, tx| s.scroll_lines(true, tx),
        "reader.scroll-up" => |s, tx| s.scroll_lines(false, tx),
        "reader.page-down" => |s, tx| s.scroll_page(true, tx),
        "reader.page-up" => |s, tx| s.scroll_page(false, tx),
        "reader.top" => |s, tx| s.scroll_top(tx),
        "reader.bottom" => |s, tx| s.scroll_bottom(tx),
        "reader.toggle-wide" => |s, _| s.toggle_wide(),
        "reader.find" => |s, tx| s.open_find(tx),
        // n and N step only while a query exists (RD-58).
        "reader.find-next" => |s, tx| {
            if s.find_query().is_some() {
                s.find_step(true, tx);
            }
        },
        "reader.find-previous" => |s, tx| {
            if s.find_query().is_some() {
                s.find_step(false, tx);
            }
        },
        "find.next" => |s, tx| s.find_step(true, tx),
        "find.previous" => |s, tx| s.find_step(false, tx),
        "find.close" => |s, tx| s.close_find(tx),
        "find.select-all" => |s, tx| s.open_find(tx),
        "reader.copy-selection" => |s, tx| s.copy_selection(tx),
        "reader.copy-block-under-pointer" => |s, tx| s.copy_block_here(tx),
        "reader.copy-all" => |s, tx| s.copy_all(tx),
        "reader.read-from-disk" => |s, _| s.read_from_disk(),
        "reader.help" => |s, _| {
            s.cover_page();
            s.reader.help = true;
        },
        "reader.edit-block" => |s, tx| s.open_sheet(crate::sheet::Scope::Block, tx),
        "reader.edit-section" => |s, tx| s.open_sheet(crate::sheet::Scope::Section, tx),
        "sheet.save" => |s, tx| s.sheet_save(tx),
        "sheet.cancel" => |s, tx| s.sheet_escape(tx),
        "help.close" => |s, tx| s.close_help(tx),
        "writer.bold" => |s, tx| s.format(Fmt::Bold, tx),
        "writer.italic" => |s, tx| s.format(Fmt::Italic, tx),
        "writer.inline-code" => |s, tx| s.format(Fmt::Code, tx),
        "writer.strikethrough" => |s, tx| s.format(Fmt::Strike, tx),
        "writer.link" => |s, tx| s.format(Fmt::Link, tx),
        "writer.heading-1" => |s, tx| s.writer_heading(1, tx),
        "writer.heading-2" => |s, tx| s.writer_heading(2, tx),
        "writer.heading-3" => |s, tx| s.writer_heading(3, tx),
        "writer.heading-4" => |s, tx| s.writer_heading(4, tx),
        "writer.heading-5" => |s, tx| s.writer_heading(5, tx),
        "writer.heading-6" => |s, tx| s.writer_heading(6, tx),
        "writer.plain-paragraph" => |s, tx| s.writer_heading(0, tx),
        "writer.indent" => |s, tx| s.writer_tab(false, tx),
        "writer.outdent" => |s, tx| s.writer_tab(true, tx),
        "writer.continue-block" => |s, tx| s.writer_enter(tx),
        "writer.follow-link" => |s, tx| crate::ui::editor::follow_link_at_caret(s, tx),
        "writer.find" => |s, tx| s.writer_find(tx),
        "writer.find-next" => |s, tx| s.source_step(true, tx),
        "writer.find-previous" => |s, tx| s.source_step(false, tx),
        "writer.help" => |s, _| {
            s.cover_page();
            s.reader.help = !s.reader.help;
        },
        "writer.paragraph-focus" => |s, tx| s.toggle_paragraph_focus(tx),
        "writer.typewriter" => |s, tx| s.toggle_typewriter(tx),
        "writer.next-font" => |s, tx| s.next_font(tx),
        "writer.escape" => |s, tx| {
            s.writer_escape(tx);
        },
        // The editor follows a Ctrl-clicked link itself; this entry lists it in Settings.
        "writer.follow-link-click" => |_, _| {},
        _ => return None,
    })
}
