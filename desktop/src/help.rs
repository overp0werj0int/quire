//! The keyboard help sheet's rows, per mode, and its search: which rows a query keeps, which
//! one is selected, and what Enter runs (RD-86 to RD-91, RD-105, WR-166, MAP-063).

use rune::{Key, KeyEvent, TextEdit, Tx};

use crate::app::{App, Mode};
use crate::keymap::{self, Overrides};

/// One row of the help sheet: what it does and the actions whose keys it shows.
pub struct Row {
    pub what: &'static str,
    pub ids: &'static [&'static str],
    /// Whether Enter or a click runs the row's first action. A range ("Heading 1 to 6") or a
    /// key that only acts in a field shows its keys and runs nothing.
    pub runs: bool,
}

pub type Group = (&'static str, &'static [Row]);

const fn h(what: &'static str, ids: &'static [&'static str]) -> Row {
    Row { what, ids, runs: true }
}

/// A row that lists keys and runs nothing.
const fn k(what: &'static str, ids: &'static [&'static str]) -> Row {
    Row { what, ids, runs: false }
}

/// The window's own keys, the same in every mode.
const APP: Group = (
    "App",
    &[
        h("Command palette", &["app.command-palette"]),
        h("Search all notes", &["app.search-notes"]),
        h("Sidebar", &["app.toggle-sidebar"]),
        h("Next / previous pane", &["app.next-pane", "app.previous-pane"]),
        h("Zen mode", &["app.focus-mode"]),
        h("Settings", &["app.settings"]),
        h("Insert an image", &["app.insert-image"]),
        h("Undo the delete", &["app.undo-delete"]),
        h("Quit", &["app.quit"]),
    ],
);

/// The reader's groups (RD-90), then the window's.
const READER: &[Group] = &[
    (
        "Read",
        &[
            h("Scroll", &["reader.scroll-down", "reader.scroll-up"]),
            h("Page down / up", &["reader.page-down", "reader.page-up"]),
            h("Top / bottom", &["reader.top", "reader.bottom"]),
            h("Sidebar", &["reader.toggle-sidebar"]),
            h("Outline", &["reader.toggle-outline", "app.toggle-outline"]),
            h("Readable / full width", &["reader.toggle-wide"]),
            h("Zoom text", &["app.zoom-in", "app.zoom-out", "app.zoom-reset"]),
            h("Source / preview / map", &["app.mode-source", "reader.mode-preview", "app.mode-preview", "reader.mode-map", "app.mode-map"]),
            h("Zen / settings / image", &["reader.focus-mode", "reader.settings", "reader.insert-image"]),
        ],
    ),
    (
        "Find & copy",
        &[
            h("Find", &["reader.find"]),
            h("Next / previous match", &["reader.find-next", "reader.find-previous"]),
            h("Close find or clear the selection", &["reader.escape", "app.escape"]),
            k("Copy or edit a block", &["reader.block-menu"]),
            k("Copy the block under the mouse", &["reader.copy-block-under-pointer"]),
            h("Copy the selection", &["reader.copy-selection"]),
            h("Copy the whole note", &["reader.copy-all"]),
        ],
    ),
    (
        "Edit",
        &[
            h("Edit the whole note", &["app.mode-source"]),
            k("Edit the block under the mouse", &["reader.edit-block"]),
            k("Edit its section", &["reader.edit-section"]),
            k("In the edit sheet: save to the file / cancel", &["sheet.save", "sheet.cancel"]),
        ],
    ),
    (
        "File",
        &[
            h("Open a note", &["app.quick-switcher"]),
            h("New note", &["app.new-note"]),
            h("Save now", &["reader.save", "app.save"]),
            h("Read from disk", &["reader.read-from-disk", "app.read-from-disk"]),
            h("Next / previous tab", &["app.next-tab", "app.previous-tab"]),
            k("Switch tab 1 to 9", &["app.tab-1", "app.tab-9"]),
            h("Close / reopen tab", &["app.close-tab", "app.reopen-tab"]),
            h("This sheet", &["reader.help", "app.help"]),
        ],
    ),
    APP,
];

/// The writer's groups (WR-166), then the window's.
const WRITER: &[Group] = &[
    (
        "Write",
        &[
            h("Bold / italic", &["writer.bold", "writer.italic"]),
            h("Inline code / strikethrough", &["writer.inline-code", "writer.strikethrough"]),
            h("Link / note link", &["writer.link", "writer.wiki-link"]),
            h("Blockquote", &["writer.blockquote"]),
            k("Heading 1 to 6", &["writer.heading-1", "writer.heading-6"]),
            h("Plain paragraph", &["writer.plain-paragraph"]),
            h("Numbered / bulleted / task list", &["writer.numbered-list", "writer.bulleted-list", "writer.task-list"]),
            h("Toggle task", &["writer.toggle-checkbox"]),
            h("Move line up / down", &["writer.move-up", "writer.move-down"]),
            h("Indent / outdent", &["writer.indent", "writer.outdent"]),
            h("Continue a list or quote", &["writer.continue-block"]),
            h("Follow the link", &["writer.follow-link", "writer.follow-link-click"]),
        ],
    ),
    (
        "View",
        &[
            h("Source / preview / map", &["app.mode-source", "app.mode-preview", "app.mode-map"]),
            h("Find", &["writer.find"]),
            h("Next / previous match", &["writer.find-next", "writer.find-previous"]),
            h("Close find, else show the preview", &["writer.escape", "app.escape"]),
            h("Paragraph focus", &["writer.paragraph-focus"]),
            h("Typewriter scrolling", &["writer.typewriter"]),
            h("Next font", &["writer.next-font"]),
            h("Outline", &["app.toggle-outline"]),
            h("Zoom text", &["app.zoom-in", "app.zoom-out", "app.zoom-reset"]),
        ],
    ),
    (
        "File",
        &[
            h("Save", &["app.save"]),
            h("Read from disk", &["app.read-from-disk"]),
            h("New note", &["app.new-note"]),
            h("Open a note", &["app.quick-switcher"]),
            h("Next / previous tab", &["app.next-tab", "app.previous-tab"]),
            k("Switch tab 1 to 9", &["app.tab-1", "app.tab-9"]),
            h("Close / reopen tab", &["app.close-tab", "app.reopen-tab"]),
            h("This sheet", &["app.help"]),
        ],
    ),
    APP,
];

/// The map's groups (MAP-063): its edits on the left, its views, files and the window's on the right.
const MAP: &[Group] = &[
    (
        "Navigate",
        &[
            h("Select left / right", &["map.left", "map.right"]),
            h("Select up / down", &["map.up", "map.down"]),
            h("Root", &["map.root"]),
            h("Fold or unfold", &["map.toggle-fold"]),
            h("Fold or unfold the branch", &["map.toggle-fold-recursive"]),
            h("Unfold everything / fold to a level", &["map.expand-all", "map.collapse-to-level"]),
            h("Follow the node's link", &["map.follow-link"]),
        ],
    ),
    (
        "Edit",
        &[
            h("Edit the node / its branch", &["map.edit", "map.edit-subtree"]),
            k("Node menu", &["map.node-menu"]),
            h("Add a child / sibling / sibling before", &["map.add-child", "map.add-sibling", "map.add-sibling-before"]),
            h("Delete the branch", &["map.delete"]),
            h("Tick the task", &["map.toggle-task"]),
            h("Move the node", &["map.move-up", "map.move-down", "map.move-left", "map.move-right"]),
            h("Outdent", &["map.outdent"]),
            k("Heading 1 to 6", &["map.heading-1", "map.heading-6"]),
            h("Undo / redo", &["map.undo", "map.redo"]),
            h("Cut / copy node / branch", &["map.cut", "map.copy-node", "map.copy-subtree"]),
            h("Copy the whole note", &["map.copy-all"]),
            k("Paste as child / sibling", &["map.paste"]),
        ],
    ),
    (
        "Naming a node",
        &[
            k("Commit / new line", &["mapedit.commit", "mapedit.newline"]),
            k("Cancel", &["mapedit.cancel"]),
            k("Commit and add a child / outdent", &["mapedit.commit-add-child", "mapedit.commit-outdent"]),
            k("Bold / italic / inline code", &["writer.bold", "writer.italic", "writer.inline-code"]),
            k("Strikethrough / link / note link", &["writer.strikethrough", "writer.link", "writer.wiki-link"]),
            k("New node: paragraph", &["mapedit.paragraph"]),
            k("New node: heading 1 to 6", &["mapedit.heading-1", "mapedit.heading-6"]),
            k("New node: numbered / bulleted / task", &["mapedit.numbered", "mapedit.bulleted", "mapedit.task"]),
        ],
    ),
    (
        "Canvas",
        &[
            h("Fit the map", &["map.fit"]),
            h("Choose the layout", &["map.layout"]),
            h("Zoom in / out", &["map.zoom-in", "app.zoom-in", "map.zoom-out", "app.zoom-out"]),
            h("Zoom 100%", &["app.zoom-reset"]),
            h("Centre the node", &["map.center"]),
        ],
    ),
    (
        "Find & views",
        &[
            h("Find", &["map.find"]),
            h("Next / previous match", &["map.find-next", "map.find-previous"]),
            h("Close find or cancel drag", &["map.escape", "app.escape"]),
            h("Sidebar", &["map.toggle-sidebar"]),
            h("Outline", &["map.toggle-outline", "app.toggle-outline"]),
            h("Source / preview / map", &["app.mode-source", "map.mode-preview", "app.mode-preview", "map.mode-map", "app.mode-map"]),
            h("Zen / settings / image", &["map.focus-mode", "map.settings", "map.insert-image"]),
            h("This sheet", &["map.help", "app.help"]),
        ],
    ),
    (
        "File",
        &[
            h("Open a note", &["app.quick-switcher"]),
            h("New note", &["app.new-note"]),
            h("Save now", &["map.save", "app.save"]),
            h("Read from disk", &["map.read-from-disk", "app.read-from-disk"]),
            h("Next / previous tab", &["app.next-tab", "app.previous-tab"]),
            k("Switch tab 1 to 9", &["app.tab-1", "app.tab-9"]),
            h("Close / reopen tab", &["app.close-tab", "app.reopen-tab"]),
        ],
    ),
    APP,
];

/// A mode's groups, and how many of them make the left column on a wide window (WR-163).
pub fn groups(mode: Mode) -> (&'static [Group], usize) {
    match mode {
        Mode::Preview => (READER, READER.len()),
        Mode::Source => (WRITER, 2),
        Mode::Map => (MAP, 3),
    }
}

/// Whether two ids are the ends of a numbered range (`app.tab-1`, `app.tab-9`).
pub fn is_range(a: &str, b: &str) -> bool {
    a.strip_suffix("-1").is_some_and(|stem| b.strip_prefix(stem).is_some_and(|n| matches!(n, "-6" | "-9")))
}

/// Every user-facing chord of `id` from `km` (showing secondary chords too for discoverability).
pub fn chords(km: &Overrides, id: &str) -> Vec<String> {
    km.display_labels(id)
}

/// What runs when the row is picked: its first action that does something here.
pub fn action(r: &Row) -> Option<&'static keymap::Action> {
    r.runs.then(|| r.ids.iter().find(|id| crate::keys::handler(id).is_some()).and_then(|id| keymap::action(id))).flatten()
}

/// The groups the query keeps, each with the rows whose words, keys or action names hold every
/// word of it; with no query, every row.
pub fn filter(km: &Overrides, mode: Mode, q: &str) -> Vec<(&'static str, Vec<&'static Row>)> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    groups(mode)
        .0
        .iter()
        .map(|(title, rows)| {
            let kept = rows
                .iter()
                .filter(|r| {
                    let hay = std::iter::once(r.what.to_string())
                        .chain(std::iter::once(title.to_string()))
                        .chain(r.ids.iter().flat_map(|id| keymap::action(id).map(|a| a.name.to_string()).into_iter().chain(chords(km, id))))
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase();
                    words.iter().all(|w| hay.contains(w.as_str()))
                })
                .collect();
            (*title, kept)
        })
        .filter(|(_, rows): &(_, Vec<_>)| !rows.is_empty())
        .collect()
}

/// The rows Up and Down step through, in order: those that run something.
pub fn picks(found: &[(&'static str, Vec<&'static Row>)]) -> Vec<&'static Row> {
    found.iter().flat_map(|(_, rows)| rows.iter().copied()).filter(|r| action(r).is_some()).collect()
}

/// The help sheet's search: its query and which runnable row is selected.
#[derive(Default)]
pub struct Search {
    pub q: TextEdit,
    pub idx: usize,
}

impl App {
    pub fn help_search(&self) -> Option<&Search> {
        self.open_layers().find_map(|l| match l {
            crate::layer::Layer::Help(h) => Some(h),
            _ => None,
        })
    }

    pub fn help_search_mut(&mut self) -> Option<&mut Search> {
        self.layers.iter_mut().find_map(|o| match &mut o.layer {
            crate::layer::Layer::Help(h) => Some(h),
            _ => None,
        })
    }

    /// The rows the help's query keeps, and the selected one among those that run.
    pub fn help_found(&self) -> (Vec<(&'static str, Vec<&'static Row>)>, Option<&'static Row>) {
        let (q, idx) = self.help_search().map_or(("", 0), |h| (h.q.text.as_str(), h.idx));
        let found = filter(&self.keymap, self.mode, q);
        let pick = picks(&found).get(idx).copied();
        (found, pick)
    }

    /// Closes the help and runs the row's action, as its keys would on the page.
    pub fn help_run(&mut self, r: &'static Row, tx: &mut Tx) {
        let Some(a) = action(r) else { return };
        self.close_layer(tx);
        if crate::layer::toggles(a.id) != Some(crate::layer::Kind::Help) {
            crate::keys::handler(a.id).expect("action() found one")(self, tx);
        }
    }

    /// A key in the open help (KEY-88, RD-105): Up and Down pick a row, Enter runs it, Esc
    /// clears the query and then closes, and every other key that is not the sheet's own goes
    /// to the search field.
    pub fn help_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        let chord = crate::focus::chord(ev);
        let query = self.help_search().is_some_and(|h| !h.q.text.is_empty());
        match ev.key {
            Key::Escape if query => {
                if let Some(h) = self.help_search_mut() {
                    h.q = TextEdit::new("");
                    h.idx = 0;
                }
                return true;
            }
            Key::Up | Key::Down if !chord && !ev.mods.shift => {
                let (found, _) = self.help_found();
                let n = picks(&found).len();
                if let Some(h) = self.help_search_mut()
                    && n > 0
                {
                    h.idx = if ev.key == Key::Down { (h.idx + 1).min(n - 1) } else { h.idx.saturating_sub(1) };
                    tx.scroll_into_view(row_key(h.idx));
                }
                return true;
            }
            Key::Enter if !chord => {
                if let (_, Some(r)) = self.help_found() {
                    self.help_run(r, tx);
                }
                return true;
            }
            Key::PageUp | Key::PageDown if !chord && !ev.mods.shift => return self.scroll_help(ev.key.clone(), tx),
            // With a query, Home and End move the caret in it.
            Key::Home | Key::End if !chord && !ev.mods.shift && !query => return self.scroll_help(ev.key.clone(), tx),
            _ => {}
        }
        let own = self.keymap.lookup(keymap::Ctx::Help, ev);
        // `?` closes while the query is empty; Esc, F1 and Ctrl+/ always close.
        let closes = own.iter().any(|a| a.id == "help.close") && (chord || matches!(ev.key, Key::Escape | Key::Function(_)) || (!query && ev.key == Key::Text("?".into())));
        if closes {
            self.close_layer(tx);
            return true;
        }
        false
    }
}

/// The key of the `i`th runnable row, which Up and Down scroll into view.
pub fn row_key(i: usize) -> String {
    format!("help-pick-{i}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Ctx, Status, TABLE};

    /// Every key bound in a mode shows in that mode's help sheet.
    #[test]
    fn every_bound_key_shows_in_its_modes_help() {
        let modes: [(Mode, &[Ctx]); 3] = [
            (Mode::Preview, &[Ctx::Global, Ctx::Reader]),
            (Mode::Source, &[Ctx::Global, Ctx::Writer]),
            (Mode::Map, &[Ctx::Global, Ctx::Map, Ctx::MapEdit]),
        ];
        let mut missing = Vec::new();
        for (mode, ctxs) in modes {
            let shown: Vec<&str> = groups(mode).0.iter().flat_map(|(_, rows)| rows.iter().flat_map(|r| r.ids.iter().copied())).collect();
            let covers = |id: &str| {
                shown.contains(&id) || id.rsplit_once('-').is_some_and(|(stem, n)| n.parse::<u8>().is_ok() && shown.contains(&format!("{stem}-1").as_str()))
            };
            for a in TABLE.iter().filter(|a| a.ctx.iter().any(|c| ctxs.contains(c)) && !matches!(a.status, Status::Deferred | Status::Dropped)) {
                if !a.chords.is_empty() && !covers(a.id) {
                    missing.push(format!("{mode:?}: {} ({})", a.id, a.name));
                }
            }
        }
        assert!(missing.is_empty(), "keys missing from help:\n{}", missing.join("\n"));
    }

    /// A mode's sheet shows only keys that act in that mode: each action is bound in one of the
    /// contexts the mode's keys go through (the preview's edit sheet is opened from it).
    #[test]
    fn every_key_shown_acts_in_its_mode() {
        let modes: [(Mode, &[Ctx]); 3] = [
            (Mode::Preview, &[Ctx::Global, Ctx::Reader, Ctx::Sheet]),
            (Mode::Source, &[Ctx::Global, Ctx::Writer]),
            (Mode::Map, &[Ctx::Global, Ctx::Map, Ctx::MapEdit]),
        ];
        for (mode, ctxs) in modes {
            for (_, rows) in groups(mode).0 {
                for r in rows.iter() {
                    for id in r.ids {
                        let a = keymap::action(id).expect("an action");
                        assert!(a.ctx.iter().any(|c| ctxs.contains(c)), "{mode:?} shows {id}, which acts only in {:?}", a.ctx);
                        assert!(!matches!(a.status, Status::Deferred | Status::Dropped), "{mode:?} shows {id}, which is not built");
                    }
                }
            }
        }
    }

    /// Every id a row names is an action, and a row that runs has one that does something.
    #[test]
    fn every_row_names_real_actions() {
        for mode in [Mode::Preview, Mode::Source, Mode::Map] {
            for (_, rows) in groups(mode).0 {
                for r in rows.iter() {
                    for id in r.ids {
                        assert!(keymap::action(id).is_some() || *id == "map.node-menu", "{id} is not an action");
                    }
                    assert!(!r.runs || action(r).is_some(), "{} runs nothing", r.what);
                }
            }
        }
    }

    #[test]
    fn a_query_keeps_rows_by_words_and_keys() {
        let km = Overrides::default();
        let found = filter(&km, Mode::Source, "bold");
        assert_eq!(found.iter().map(|(_, r)| r.iter().map(|r| r.what).collect::<Vec<_>>()).collect::<Vec<_>>(), [["Bold / italic"]]);
        let by_key = filter(&km, Mode::Source, "ctrl+shift+9");
        assert_eq!(by_key.iter().flat_map(|(_, r)| r.iter().map(|r| r.what)).collect::<Vec<_>>(), ["Numbered / bulleted / task list"]);
        // Secondary chords are searchable and visible too.
        let by_secondary = filter(&km, Mode::Preview, "down");
        assert!(by_secondary.iter().flat_map(|(_, r)| r.iter().map(|r| r.what)).any(|w| w == "Scroll"));
        assert!(filter(&km, Mode::Map, "zzz").is_empty());
    }
}
