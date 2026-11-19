//! Shortcut labels and the window's shortcuts.

use rune::{KeyEvent, Tx};

use crate::app::{App, Screen};
use crate::focus::Pane;
use crate::keymap::{self, Ctx};
use crate::layer::{Kind, Layer};

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
    if esc_again(ev, tx) {
        return true;
    }
    let stop = tx.focused().and_then(crate::focus::stop);
    page_key(s, ev, tx, stop)
}

/// A key for the page, with the stop that holds the keyboard, if one does.
fn page_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx, stop: Option<Pane>) -> bool {
    let preset = s.preset();
    let vault = s.screen == Screen::Vault;
    // The toast is not a layer: its Undo key acts whatever is open.
    if vault && keymap::lookup(preset, Ctx::Global, ev).iter().any(|a| a.id == "app.undo-delete") {
        s.undo_delete(tx);
        return true;
    }
    if let Some(kind) = s.layer().map(Layer::kind) {
        return layer_key(s, kind, ev, tx);
    }
    // A focused stop owns the keyboard's plain keys: past its own, Esc and F6 act. A chord acts
    // as it would on the note (`focus`).
    if let Some(pane) = stop
        && let Some(done) = s.stop_key(pane, ev, tx)
    {
        return done;
    }
    let stack: &[Ctx] = if stop.is_some() && !crate::focus::chord(ev) { &[Ctx::Global] } else { page_stack(s) };
    let field = tx.focused().is_some_and(crate::focus::field);
    // The Menu key or Shift+F10 on the note: the page's block menu, as Rune opens a focused
    // control's (a sidebar row's, say).
    if vault && s.mode == crate::app::Mode::Preview && crate::focus::on_note(tx) && menu_key(ev) {
        s.block_menu_here(tx);
        return true;
    }
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
            // Away from the notes, only Esc and Quit act; from a text field, nothing at the
            // note's caret does.
            if (!vault && !matches!(a.id, "app.escape" | "app.quit")) || (field && crate::focus::at_caret(a)) {
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
            if let Some(run) = handler(a.id) {
                run(s, tx);
                return true;
            }
        }
    }
    false
}

/// Esc with Shift, Ctrl or Alt is Esc wherever Esc does something (MODESC2): no preset gives
/// it a meaning of its own. It goes round again as plain Esc, which then reaches every
/// handler as Esc would, Rune's included: a modal's dismissal and a field's blur take plain
/// Esc only. The window's keys call this first, and so does a handler that hears keys before
/// them and acts on plain Esc alone. With the logo key, Esc stays the system's.
pub fn esc_again(ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    if !matches!(ev.key, rune::Key::Escape) || m.logo || m == rune::Mods::default() {
        return false;
    }
    tx.send_key(KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
    true
}

/// The Menu key, or Shift+F10: a context menu from the keyboard.
fn menu_key(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    let plain = !m.cmd && !m.ctrl && !m.alt && !m.logo;
    match ev.key {
        rune::Key::ContextMenu => plain,
        rune::Key::Function(10) => plain && m.shift,
        _ => false,
    }
}

/// A key while a layer is open, which holds the keyboard: the page behind never hears it.
///
/// A light layer (a menu, a drawer) takes Esc, and lets a shortcut with a modifier act as if
/// it had not been there: it closes, and the key goes to what is under it. The key that
/// toggles it closes it. A heavier layer acts on its own keys, then on the window's
/// shortcuts (none over help and the edit sheet), which close it first unless they toggle
/// it. Any other key goes on to Rune, which keeps it inside the layer, except over help and
/// the edit sheet, which swallow it.
fn layer_key(s: &mut App, kind: Kind, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let preset = s.preset();
    if kind.light() {
        if matches!(ev.key, rune::Key::Escape) {
            if !s.esc_takes_notice() {
                s.dismiss(tx);
            }
            return true;
        }
        if !crate::focus::chord(ev) {
            return false;
        }
        // The keys of what is under the light layers: under a row's menu in the sidebar's
        // drawer, the page's, so Ctrl+F closes both and opens find.
        let under = s.open_layers().rev().skip(1).map(Layer::kind).find(|k| !k.light());
        if under.is_none() && s.edit_chord_to_note(ev, tx) {
            return true;
        }
        let (own, window) = under.map_or((page_stack(s), &[][..]), layer_stack);
        let field = tx.focused().is_some_and(crate::focus::field);
        let Some(a) = own.iter().chain(window).flat_map(|c| keymap::lookup(preset, *c, ev)).find(|a| handler(a.id).is_some() && !(field && crate::focus::at_caret(a))) else { return false };
        if crate::layer::toggles(a.id) == Some(kind) {
            handler(a.id).expect("found above")(s, tx);
            return true;
        }
        // The stop inside the closed layer no longer holds the keyboard.
        s.close_layer(tx);
        return page_key(s, ev, tx, None);
    }
    let (own, window) = layer_stack(kind);
    for a in own.iter().flat_map(|c| keymap::lookup(preset, *c, ev)) {
        if let Some(run) = handler(a.id) {
            run(s, tx);
            return true;
        }
    }
    for a in window.iter().flat_map(|c| keymap::lookup(preset, *c, ev)) {
        if a.id == "app.escape" {
            s.dismiss(tx);
            return true;
        }
        let Some(run) = handler(a.id) else { continue };
        if crate::layer::toggles(a.id) != Some(kind) {
            s.close_layers(tx);
        }
        run(s, tx);
        return true;
    }
    // Help and the edit sheet swallow every other key (RD-80, SHEET-014).
    matches!(kind, Kind::Help | Kind::Sheet)
}

/// A heavy layer's own contexts, and the window's it takes: help and the edit sheet take
/// only their own keys (RD-80, SHEET-036).
fn layer_stack(kind: Kind) -> (&'static [Ctx], &'static [Ctx]) {
    match kind {
        Kind::Help => (&[Ctx::Help], &[]),
        Kind::Sheet => (&[Ctx::Sheet], &[]),
        _ => (&[], &[Ctx::Global]),
    }
}

/// The contexts a key goes through with no layer open, top first (KEY-02).
pub(crate) fn page_stack(s: &App) -> &'static [Ctx] {
    if s.screen != Screen::Vault {
        &[Ctx::Global]
    } else if s.mode == crate::app::Mode::Preview {
        &[Ctx::Reader, Ctx::Global]
    } else {
        &[Ctx::Writer, Ctx::Global]
    }
}

type Run = fn(&mut App, &mut Tx);

/// What an action does, or None while the feature it belongs to is not built yet, so its key
/// falls through. Mouse-only actions run from their pointer handlers instead.
pub fn handler(id: &str) -> Option<Run> {
    use crate::app::PalMode;
    use crate::format::Fmt;
    // The closed note's editor goes away with it: the note shown now gets the keys.
    fn close_tab(s: &mut App, tx: &mut Tx) {
        if let Some(a) = s.active.clone() {
            s.close_tab(&a);
            s.keys_to_note(tx);
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
        "app.next-pane" => |s, tx| s.step_pane(false, tx),
        "app.previous-pane" => |s, tx| s.step_pane(true, tx),
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
        "reader.help" | "writer.help" => |s, tx| s.toggle_help(tx),
        "reader.edit-block" => |s, tx| s.open_sheet(crate::sheet::Scope::Block, tx),
        "reader.edit-section" => |s, tx| s.open_sheet(crate::sheet::Scope::Section, tx),
        "sheet.save" => |s, tx| s.sheet_save(tx),
        "sheet.cancel" => |s, tx| s.sheet_escape(tx),
        "help.close" => |s, tx| s.close_layer(tx),
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
