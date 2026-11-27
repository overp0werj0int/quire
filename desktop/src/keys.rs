//! Shortcut labels and the window's shortcuts.

use rune::{KeyEvent, Tx};

use crate::app::{App, Screen};
use crate::focus::Pane;
use crate::keymap::Ctx;
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
    s.key_moved = true;
    s.touch_activity();
    if esc_again(ev, tx) {
        return true;
    }
    let stop = tx.focused().and_then(crate::focus::stop);
    page_key(s, ev, tx, stop)
}

/// A key for the page, with the stop that holds the keyboard, if one does.
fn page_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx, stop: Option<Pane>) -> bool {
    let vault = s.screen == Screen::Vault;
    // The toast is not a layer: its Undo key acts whatever is open.
    if vault && s.keymap.lookup(Ctx::Global, ev).iter().any(|a| a.id == "app.undo-delete") {
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
    let writing = s.writing_note();
    // The Menu key or Shift+F10 on the note: the page's block menu, as Rune opens a focused
    // control's (a sidebar row's, say).
    if vault && s.mode == crate::app::Mode::Preview && crate::focus::on_note(tx) && menu_key(ev) {
        s.block_menu_here(tx);
        return true;
    }
    if vault && s.mode == crate::app::Mode::Map && !s.active_locked() && crate::focus::on_note(tx) && menu_key(ev) {
        s.map_node_menu(s.map_selected(), None, tx);
        return true;
    }
    for ctx in stack {
        if matches!(ctx, Ctx::Reader | Ctx::Map) {
            // The second key of `g g` (RD-23).
            let now = s.now.get();
            if let Some((first, at)) = s.reader.pending.take()
                && now - at < 0.9
                && let Some(run) = s.keymap.lookup_seq(*ctx, &first, ev).iter().find_map(|a| handler(a.id))
            {
                run(s, tx);
                return true;
            }
            if s.keymap.starts_seq(*ctx, ev) {
                s.reader.pending = Some((ev.clone(), now));
                return true;
            }
        }
        let actions = s.keymap.lookup(*ctx, ev);
        for a in actions {
            // Away from the notes, only Esc and Quit act; from a text field or when no editable
            // note is open (empty vault, no tab, locked note), nothing at the note's caret does
            // (so Tab/Shift+Tab/Enter reach Rune's focus traversal cleanly, `FOCUSMODEL`).
            if (!vault && !matches!(a.id, "app.escape" | "app.quit")) || ((field || !writing) && crate::focus::at_caret(a)) {
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
            if a.id == "map.escape" {
                return s.map_escape(tx);
            }
            // A locked note's map is hidden behind the lock panel: its node keys select nothing.
            if *ctx == Ctx::Map
                && s.active_locked()
                && !matches!(
                    a.id,
                    "map.toggle-sidebar" | "map.toggle-outline" | "map.help" | "map.read-from-disk" | "map.mode-preview" | "map.mode-map" | "map.settings"
                )
            {
                continue;
            }
            // From Find or another text field, Reader/Map single-key shortcuts stay quiet so typing
            // in the field never fires them (MAP-060, MAP-066).
            if matches!(ctx, Ctx::Reader | Ctx::Map)
                && field
                && !matches!(a.id, "reader.find" | "reader.find-next" | "reader.find-previous" | "map.find" | "map.find-next" | "map.find-previous")
            {
                continue;
            }
            // n and N step only while a query exists; F3, Shift+F3, Ctrl+G and Ctrl+Shift+G open Find.
            if matches!(a.id, "reader.find-next" | "reader.find-previous" | "map.find-next" | "map.find-previous")
                && s.find_query().is_none()
                && matches!(ev.key, rune::Key::Text(_))
            {
                continue;
            }
            if let Some(run) = handler(a.id) {
                run(s, tx);
                return true;
            }
        }
    }
    false
}

/// Esc with Shift, Ctrl or Alt is Esc wherever Esc does something (MODESC2): no action gives
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
    if kind == Kind::Settings && s.set_sec == crate::app::Sec::Keys && s.key_edit.is_some() {
        return s.handle_key_edit(ev);
    }
    if kind == Kind::Help {
        return s.help_key(ev, tx);
    }
    if kind == Kind::Lightbox && ev.mods == rune::Mods::default() {
        match ev.key {
            rune::Key::Left => {
                s.lightbox_step(-1);
                return true;
            }
            rune::Key::Right => {
                s.lightbox_step(1);
                return true;
            }
            _ => {}
        }
    }
    if kind.light() {
        if matches!(ev.key, rune::Key::Escape) {
            if !s.esc_takes_notice() {
                s.dismiss(tx);
            }
            return true;
        }
        if kind == Kind::Layouts && s.keymap.lookup(Ctx::Map, ev).iter().any(|a| a.id == "map.layout") {
            s.close_layer(tx);
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
        let Some(a) = own.iter().chain(window).flat_map(|c| s.keymap.lookup(*c, ev)).find(|a| handler(a.id).is_some() && !(field && crate::focus::at_caret(a))) else { return false };
        if crate::layer::toggles(a.id) == Some(kind) {
            handler(a.id).expect("found above")(s, tx);
            return true;
        }
        // The stop inside the closed layer no longer holds the keyboard.
        s.close_layer(tx);
        return page_key(s, ev, tx, None);
    }
    let (own, window) = layer_stack(kind);
    for a in own.iter().flat_map(|c| s.keymap.lookup(*c, ev)) {
        if let Some(run) = handler(a.id) {
            run(s, tx);
            return true;
        }
    }
    for a in window.iter().flat_map(|c| s.keymap.lookup(*c, ev)) {
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
    // The edit sheet swallows every other key (SHEET-014).
    kind == Kind::Sheet
}

/// A heavy layer's own contexts, and the window's it takes: help and the edit sheet take
/// only their own keys (RD-80, SHEET-036).
fn layer_stack(kind: Kind) -> (&'static [Ctx], &'static [Ctx]) {
    match kind {
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
    } else if s.mode == crate::app::Mode::Map && s.map.inline.is_some() {
        &[Ctx::MapEdit, Ctx::Global]
    } else if s.mode == crate::app::Mode::Map {
        &[Ctx::Map, Ctx::Global]
    } else {
        &[Ctx::Writer, Ctx::Global]
    }
}

type Run = fn(&mut App, &mut Tx);

/// What an action does, or None while the feature it belongs to is not built yet, so its key
/// falls through. Mouse-only actions run from their pointer handlers instead.
pub fn handler(id: &str) -> Option<Run> {
    use crate::app::{Mode, PalMode};
    use crate::format::Fmt;
    use crate::map_geom::Arrow;
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
        "app.search-notes" => |s, tx| s.open_search(tx),
        "app.toggle-sidebar" | "reader.toggle-sidebar" | "map.toggle-sidebar" => |s, tx| s.toggle_left(tx),
        "app.toggle-outline" | "reader.toggle-outline" | "map.toggle-outline" => |s, tx| s.toggle_right(tx),
        "app.mode-source" => |s, tx| s.show_mode(Mode::Source, tx),
        "app.mode-preview" | "reader.mode-preview" | "map.mode-preview" => |s, tx| s.show_mode(Mode::Preview, tx),
        "app.mode-map" | "reader.mode-map" | "map.mode-map" => |s, tx| s.show_mode(Mode::Map, tx),
        "app.mode-board" => |s, tx| s.toggle_board(tx),
        "app.settings" | "reader.settings" | "map.settings" => |s, tx| s.toggle_settings(tx),
        "app.focus-mode" | "reader.focus-mode" | "map.focus-mode" => |s, tx| s.toggle_focus(tx),
        "app.new-note" => |s, tx| s.new_note(None, tx),
        // macOS quits from its app menu.
        "app.quit" if !cfg!(target_os = "macos") => |s, tx| s.quit(tx),
        "app.insert-image" | "reader.insert-image" | "map.insert-image" => |s, tx| s.pick_image(tx),
        "app.save" | "reader.save" | "map.save" => |s, tx| s.save_now(tx),
        "app.close-tab" => close_tab,
        "app.reopen-tab" => |s, tx| s.reopen_tab(tx),
        "app.read-from-disk" | "reader.read-from-disk" | "map.read-from-disk" => |s, _| s.read_from_disk(),
        "app.zoom-in" => |s, tx| {
            if s.mode == Mode::Map {
                s.map_zoom_step(1.25, tx);
            } else {
                s.zoom(Some(true), tx);
            }
        },
        "app.zoom-out" => |s, tx| {
            if s.mode == Mode::Map {
                s.map_zoom_step(0.8, tx);
            } else {
                s.zoom(Some(false), tx);
            }
        },
        "app.zoom-reset" => |s, tx| {
            if s.mode == Mode::Map {
                s.map_zoom_reset(tx);
            } else {
                s.zoom(None, tx);
            }
        },
        "app.help" | "reader.help" | "map.help" => |s, tx| s.toggle_help(tx),
        "app.undo-delete" => |s, tx| s.undo_delete(tx),
        "app.next-pane" => |s, tx| s.step_pane(false, tx),
        "app.previous-pane" => |s, tx| s.step_pane(true, tx),
        "app.tab-1" => |s, tx| s.switch_tab_index(0, tx),
        "app.tab-2" => |s, tx| s.switch_tab_index(1, tx),
        "app.tab-3" => |s, tx| s.switch_tab_index(2, tx),
        "app.tab-4" => |s, tx| s.switch_tab_index(3, tx),
        "app.tab-5" => |s, tx| s.switch_tab_index(4, tx),
        "app.tab-6" => |s, tx| s.switch_tab_index(5, tx),
        "app.tab-7" => |s, tx| s.switch_tab_index(6, tx),
        "app.tab-8" => |s, tx| s.switch_tab_index(7, tx),
        "app.tab-9" => |s, tx| s.switch_tab_index(8, tx),
        "app.next-tab" => |s, tx| s.step_tab(true, tx),
        "app.previous-tab" => |s, tx| s.step_tab(false, tx),
        "app.nav-back" => |s, tx| s.nav_back(tx),
        "app.nav-forward" => |s, tx| s.nav_forward(tx),
        "note.lock" => |s, tx| s.ask_lock(tx),
        "note.lock-all" => |s, tx| {
            if !s.unlocked.is_empty() || s.parked_has_unlocked() {
                s.lock_all_notes(tx);
                s.toast("Locked all notes", tx);
            }
        },
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
        "reader.find-next" => |s, tx| find_step_or_open(s, true, tx),
        "reader.find-previous" => |s, tx| find_step_or_open(s, false, tx),
        "find.next" => |s, tx| s.find_step(true, tx),
        "find.previous" => |s, tx| s.find_step(false, tx),
        "find.close" => |s, tx| s.close_find(tx),
        "find.select-all" => |s, tx| s.open_find(tx),
        "reader.copy-selection" => |s, tx| s.copy_selection(tx),
        "reader.copy-block-under-pointer" => |s, tx| s.copy_block_here(tx),
        "reader.copy-all" | "map.copy-all" => |s, tx| s.copy_all(tx),
        "reader.edit-block" => |s, tx| s.open_sheet(crate::sheet::Scope::Block, tx),
        "reader.edit-section" => |s, tx| s.open_sheet(crate::sheet::Scope::Section, tx),
        "reader.undo" => |s, tx| s.reader_undo(false, tx),
        "reader.redo" => |s, tx| s.reader_undo(true, tx),
        "reader.toggle-fold" => |s, _| s.toggle_current_page_fold(),
        "sheet.save" => |s, tx| s.sheet_save(tx),
        "sheet.cancel" => |s, tx| s.sheet_escape(tx),
        "help.close" => |s, tx| s.close_layer(tx),
        "writer.bold" => |s, tx| s.format(Fmt::Bold, tx),
        "writer.italic" => |s, tx| s.format(Fmt::Italic, tx),
        "writer.inline-code" => |s, tx| s.format(Fmt::Code, tx),
        "writer.strikethrough" => |s, tx| s.format(Fmt::Strike, tx),
        "writer.link" => |s, tx| s.format(Fmt::Link, tx),
        "writer.wiki-link" => |s, tx| s.format(Fmt::Wiki, tx),
        "writer.blockquote" => |s, tx| s.format(Fmt::Quote, tx),
        "writer.toggle-checkbox" => |s, tx| s.writer_toggle_checkbox(tx),
        "writer.move-up" => |s, tx| s.writer_move_lines(false, tx),
        "writer.move-down" => |s, tx| s.writer_move_lines(true, tx),
        "writer.heading-1" => |s, tx| s.writer_heading(1, tx),
        "writer.heading-2" => |s, tx| s.writer_heading(2, tx),
        "writer.heading-3" => |s, tx| s.writer_heading(3, tx),
        "writer.heading-4" => |s, tx| s.writer_heading(4, tx),
        "writer.heading-5" => |s, tx| s.writer_heading(5, tx),
        "writer.heading-6" => |s, tx| s.writer_heading(6, tx),
        "writer.plain-paragraph" => |s, tx| s.writer_heading(0, tx),
        "writer.numbered-list" => |s, tx| s.format(Fmt::Number, tx),
        "writer.bulleted-list" => |s, tx| s.format(Fmt::Bullet, tx),
        "writer.task-list" => |s, tx| s.format(Fmt::Task, tx),
        "writer.indent" => |s, tx| s.writer_tab(false, tx),
        "writer.outdent" => |s, tx| s.writer_tab(true, tx),
        "writer.continue-block" => |s, tx| s.writer_enter(tx),
        "writer.follow-link" => |s, tx| crate::ui::editor::follow_link_at_caret(s, tx),
        "writer.find" => |s, tx| s.writer_find(tx),
        "writer.replace" => |s, tx| s.writer_replace(tx),
        "writer.find-next" => |s, tx| s.source_step(true, tx),
        "writer.find-previous" => |s, tx| s.source_step(false, tx),
        "writer.paragraph-focus" => |s, tx| s.toggle_paragraph_focus(tx),
        "writer.typewriter" => |s, tx| s.toggle_typewriter(tx),
        "writer.next-font" => |s, tx| s.next_font(tx),
        "writer.escape" => |s, tx| {
            s.writer_escape(tx);
        },
        "map.left" => |s, tx| s.map_arrow(Arrow::Left, tx),
        "map.right" => |s, tx| s.map_arrow(Arrow::Right, tx),
        "map.up" => |s, tx| s.map_arrow(Arrow::Up, tx),
        "map.down" => |s, tx| s.map_arrow(Arrow::Down, tx),
        "map.root" => |s, tx| s.map_root(tx),
        "map.toggle-fold" => |s, tx| s.map_toggle_selected(false, tx),
        "map.toggle-fold-recursive" => |s, tx| s.map_toggle_selected(true, tx),
        "map.expand-all" => |s, tx| s.map_expand_all(tx),
        "map.collapse-to-level" => |s, tx| s.map_collapse_to_level(tx),
        "map.fit" => |s, tx| s.map_fit(tx),
        "map.layout" => |s, tx| s.open_layouts(tx),
        "map.center" => |s, tx| s.map_center(tx),
        "map.zoom-in" => |s, tx| s.map_zoom_step(1.25, tx),
        "map.zoom-out" => |s, tx| s.map_zoom_step(0.8, tx),
        "map.delete" => |s, tx| s.map_delete(tx),
        "map.edit" => |s, tx| s.map_edit(tx),
        "map.follow-link" => |s, tx| s.map_follow_link(tx),
        "map.edit-subtree" => |s, tx| s.map_edit_branch(tx),
        "map.add-child" => |s, tx| s.map_add(crate::map_app::Place::Child, tx),
        "map.add-sibling" => |s, tx| s.map_add(crate::map_app::Place::After, tx),
        "map.add-sibling-before" => |s, tx| s.map_add(crate::map_app::Place::Before, tx),
        "map.toggle-task" => |s, tx| s.map_toggle_task(s.map_selected(), tx),
        "map.move-left" => |s, tx| s.map_move_arrow(Arrow::Left, tx),
        "map.move-right" => |s, tx| s.map_move_arrow(Arrow::Right, tx),
        "map.move-up" => |s, tx| s.map_move_arrow(Arrow::Up, tx),
        "map.move-down" => |s, tx| s.map_move_arrow(Arrow::Down, tx),
        "map.outdent" => |s, tx| s.map_step(crate::map::Step::Out, tx),
        "map.heading-1" => |s, tx| s.map_level(1, tx),
        "map.heading-2" => |s, tx| s.map_level(2, tx),
        "map.heading-3" => |s, tx| s.map_level(3, tx),
        "map.heading-4" => |s, tx| s.map_level(4, tx),
        "map.heading-5" => |s, tx| s.map_level(5, tx),
        "map.heading-6" => |s, tx| s.map_level(6, tx),
        "map.paragraph" => |s, tx| s.map_convert(crate::map::NewKind::Paragraph, tx),
        "map.task" => |s, tx| s.map_convert(crate::map::NewKind::Task, tx),
        "map.bulleted" => |s, tx| s.map_convert(crate::map::NewKind::Bullet, tx),
        "map.numbered" => |s, tx| s.map_convert(crate::map::NewKind::Number, tx),
        "map.undo" => |s, tx| s.map_undo(false, tx),
        "map.redo" => |s, tx| s.map_undo(true, tx),
        "map.cut" => |s, tx| s.map_cut(tx),
        "map.copy-node" => |s, tx| s.map_copy(false, tx),
        "map.copy-subtree" => |s, tx| s.map_copy(true, tx),
        "map.paste" => |s, tx| s.map_paste_key(tx),
        "map.find" => |s, tx| s.open_find(tx),
        "map.find-next" => |s, tx| find_step_or_open(s, true, tx),
        "map.find-previous" => |s, tx| find_step_or_open(s, false, tx),
        "map.escape" => |s, tx| {
            s.map_escape(tx);
        },
        // The editor follows a Ctrl-clicked link itself; this entry lists it in Settings.
        "writer.follow-link-click" => |_, _| {},
        _ => return None,
    })
}

/// F3, Shift+F3, Ctrl+G, Ctrl+Shift+G, n and N in Preview and Map: the next or previous match,
/// or Find opening when no query exists.
fn find_step_or_open(s: &mut App, forward: bool, tx: &mut Tx) {
    if s.find_query().is_some() {
        s.find_step(forward, tx);
    } else {
        s.open_find(tx);
    }
}
