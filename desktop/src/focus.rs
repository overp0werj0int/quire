//! The keyboard's focus (FOCUS): one rule for every place the keyboard can rest.
//!
//! The keyboard is on the note (the editor in the writer, the reader in preview) or on a
//! stop: a row of a side list, a sidebar or toolbar button, a panel tab, the search field.
//! Stops live in three panes, left to right: the sidebar, the note (its toolbar) and the
//! outline. Every stop keeps the same rules:
//!
//! - A focused stop owns the plain keys. Its own keys act (Space, Enter, the arrows, a letter
//!   that jumps to a row), then Esc and F6. The key that shows or hides the stop's pane hides
//!   it, as a layer's own key closes the layer. Every other plain key is dropped: none reaches
//!   the reader or the writer behind it. A shortcut with Ctrl, Alt or the logo key acts as it
//!   would with the keyboard on the note (`chord`), except that a text field keeps the
//!   writer's chords at the caret from the note (`field`). The editor's own chords, which its
//!   text field handles, go on to the editor (`edit_chord`).
//! - Esc gives the keyboard back to the note (`App::keys_to_note`).
//! - When the element that holds focus goes away, for any reason (its pane hides, zen mode,
//!   its row is gone, a rename ends, find closes, its tab closes), the keyboard goes to the
//!   note. Every stop and every pane carries `focus_return(NOTE)` (`ui::focus`), and NOTE
//!   keys the editor's box in the writer and the reader's keyboard anchor elsewhere. Find,
//!   the search field's Esc and a closed tab call `App::keys_to_note` themselves.
//! - F6 and Shift+F6 move between the panes that show: the sidebar's current list, the note,
//!   the outline's current heading. Tab moves inside a pane and on to the next. Into the
//!   writer, Tab and F6 alike leave the caret where it was, in view (`tab_into_note`).
//!
//! Rune would hand focus that loses its element to the next control in the old tree. In the
//! reader that is a paragraph with links, focused with no link in it: it reads as nothing
//! focused, yet Rune presses it on Space and Enter, so they did nothing. `focus_return` asks
//! for the note before that fallback runs.

use rune::{Key, KeyEvent, Tx};

use crate::app::{App, Mode, Screen};

/// The key of the element the keyboard goes back to: the editor's box in the writer, else the
/// reader's keyboard anchor (`ui::focus::note_anchor`).
pub const NOTE: &str = "note-keys";

/// The panes F6 moves between, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Sidebar,
    Note,
    Outline,
}

/// Every stop, by the start of its element's key, and its pane. The stops' builders use these
/// keys, and a test checks that every focusable element in the panes is here.
pub const STOPS: &[(&str, Pane)] = &[
    ("tab:", Pane::Sidebar),
    ("note:", Pane::Sidebar),
    ("label-", Pane::Sidebar),
    ("new-note", Pane::Sidebar),
    ("new-folder", Pane::Sidebar),
    ("file:", Pane::Sidebar),
    ("dir:", Pane::Sidebar),
    ("search", Pane::Sidebar),
    ("res:", Pane::Sidebar),
    ("hit:", Pane::Sidebar),
    ("tag-row:", Pane::Sidebar),
    ("foot-", Pane::Sidebar),
    ("fmt-", Pane::Note),
    ("ol-", Pane::Outline),
    ("bl-", Pane::Outline),
];

/// The text fields on the page, outside the note: the search field, find's, the note's name
/// and a new folder's. While one has focus its own keys edit it, and the writer's chords at
/// the caret (`at_caret`) do nothing; the window's shortcuts still act. The palette's,
/// Settings' and the lock dialog's fields sit in layers, which take only the window's keys.
pub const FIELDS: &[&str] = &["search", "find-input", "rename", "new-folder"];

/// Whether the element keyed `key` is one of the page's text fields.
pub fn field(key: &str) -> bool {
    FIELDS.contains(&key)
}

/// An action that works on the note's text at its caret: the edits the writer shares with the
/// edit sheet (a format, a heading, indent, a list's Enter) and following the link there.
pub fn at_caret(a: &crate::keymap::Action) -> bool {
    a.ctx.contains(&crate::keymap::Ctx::Sheet) || a.id == "writer.follow-link"
}

/// The pane of the stop keyed `key`, or None when `key` is not a stop (the editor, a link in
/// the reader, a layer's control).
pub fn stop(key: &str) -> Option<Pane> {
    STOPS.iter().find(|(start, _)| key.starts_with(start)).map(|&(_, pane)| pane)
}

/// The pane that holds the keyboard: a stop's, else the note's.
fn pane_now(tx: &Tx) -> Pane {
    tx.focused().and_then(stop).unwrap_or(Pane::Note)
}

/// Whether the keyboard is on the note with nothing else focused: nothing has focus, or the
/// reader's anchor has.
pub fn on_note(tx: &Tx) -> bool {
    matches!(tx.focused(), None | Some(NOTE))
}

/// A shortcut with Ctrl, Alt or the logo key. It reaches past a focused stop and acts as it
/// would with the keyboard on the note; plain keys stay the stop's.
pub fn chord(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    m.cmd || m.ctrl || m.alt || m.logo
}

/// The editor's own chords: undo, redo, select all and the clipboard's. Its text field acts on
/// them, so they reach the writer only while the editor has focus (`App::edit_chord_to_note`).
pub fn edit_chord(ev: &KeyEvent) -> bool {
    let m = ev.mods;
    let Key::Command(c) = ev.key else { return false };
    (m.cmd || m.ctrl) && !m.alt && !m.logo && matches!((c.to_ascii_lowercase(), m.shift), ('a' | 'c' | 'x' | 'v' | 'y', false) | ('z', _))
}

/// The actions that show and hide a pane.
fn toggles(pane: Pane, id: &str) -> bool {
    match pane {
        Pane::Sidebar => id == "app.toggle-sidebar",
        Pane::Outline => matches!(id, "app.toggle-outline" | "reader.toggle-outline"),
        Pane::Note => false,
    }
}

impl App {
    /// A key while the stop keyed `key` has focus, before the window's shortcuts: Some when
    /// the stop's rule decides it, None when the window's shortcuts take it on.
    pub(crate) fn stop_key(&mut self, pane: Pane, ev: &KeyEvent, tx: &mut Tx) -> Option<bool> {
        if chord(ev) {
            return self.edit_chord_to_note(ev, tx).then_some(true);
        }
        Some(match ev.key {
            Key::Escape => {
                if !self.esc_takes_notice() {
                    self.keys_to_note(tx);
                }
                true
            }
            // F6 is a window shortcut; Shift+F10 and the Menu key open the stop's menu in Rune.
            Key::Function(_) | Key::ContextMenu => return None,
            _ => {
                self.pane_toggle_key(pane, ev, tx);
                true
            }
        })
    }

    /// The editor's own chord (`edit_chord`) from a stop that is not a text field, in the
    /// writer: the light layers close, the editor takes the keyboard and the chord acts there,
    /// as Ctrl+B does from a row. A text field keeps the chord for itself.
    pub(crate) fn edit_chord_to_note(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !self.writing_note() || !edit_chord(ev) || tx.focused().is_some_and(field) {
            return false;
        }
        self.close_layers(tx);
        self.focus_editor(tx);
        tx.send_key(ev.clone());
        true
    }

    /// Tab from the formatting toolbar, or Shift+Tab from the docked outline's first list, into
    /// the writer: the editor takes the keyboard with its caret where it was, as F6 gives it.
    /// Rune's Tab would scroll the whole editor into view, and the caret with it off screen.
    pub(crate) fn tab_into_note(&mut self, from: Pane, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if ev.key != Key::Tab || chord(ev) || !self.writing_note() {
            return false;
        }
        let shift = ev.mods.shift;
        let into = match from {
            Pane::Note => !shift,
            Pane::Outline => shift && self.docked().1 && tx.focused().is_some_and(|k| k.starts_with("ol-") || !self.outline_stop().starts_with("ol-")),
            Pane::Sidebar => false,
        };
        if into {
            self.focus_editor(tx);
        }
        into
    }

    /// The key that shows or hides `pane`, pressed while the keyboard is in it: it hides the
    /// pane, ahead of a list's letters.
    pub(crate) fn pane_toggle_key(&mut self, pane: Pane, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.layer().is_some() {
            return false;
        }
        let preset = self.preset();
        let ctxs = crate::keys::page_stack(self);
        let Some(a) = ctxs.iter().flat_map(|c| crate::keymap::lookup(preset, *c, ev)).find(|a| toggles(pane, a.id)) else { return false };
        let Some(run) = crate::keys::handler(a.id) else { return false };
        run(self, tx);
        true
    }

    /// F6, or Shift+F6 when `back`: the keyboard moves to the next pane that shows, left to
    /// right, round to the first.
    pub(crate) fn step_pane(&mut self, back: bool, tx: &mut Tx) {
        let (left, right) = self.docked();
        let panes: Vec<Pane> = [(Pane::Sidebar, left), (Pane::Note, true), (Pane::Outline, right)].into_iter().filter(|p| p.1).map(|p| p.0).collect();
        let here = panes.iter().position(|&p| p == pane_now(tx)).unwrap_or(1.min(panes.len() - 1));
        let n = panes.len();
        let to = panes[if back { (here + n - 1) % n } else { (here + 1) % n }];
        match to {
            Pane::Note => self.keys_to_note(tx),
            Pane::Sidebar => tx.focus(self.sidebar_stop(), true),
            Pane::Outline => tx.focus(self.outline_stop(), true),
        }
    }

    /// Whether the sidebar and the outline are docked beside the page, where F6 reaches them.
    /// A drawer is a layer, which holds the keyboard itself.
    pub(crate) fn docked(&self) -> (bool, bool) {
        let w = self.view_w.get();
        let vault = self.screen == Screen::Vault;
        let doc = vault && self.active.as_ref().is_some_and(|a| self.files().contains_key(a));
        let zen = vault && self.focus && doc;
        let left = vault && self.persist.left_open && !zen && w >= crate::app::SIDEBAR_DOCK_W;
        let right = vault && self.persist.right_open && !zen && doc && w >= crate::app::OUTLINE_DOCK_W;
        (left, right)
    }

    /// Where F6 puts the keyboard in the sidebar: the first result while search shows, the
    /// tags while they show, else the tree's current row, else the tree, else the active tab,
    /// else the search field.
    pub(crate) fn sidebar_stop(&self) -> String {
        if self.note_mode() {
            let shown = self.shelved().or_else(|| self.shelf().into_iter().next());
            return shown.map_or_else(|| "open-notes".into(), |t| t.key());
        }
        let q = self.query.text.trim();
        if !q.is_empty() {
            let first = crate::md::search(self.files(), q).into_iter().next();
            return first.map_or_else(|| "search".into(), |r| format!("res:{}", r.path));
        }
        if self.show_tags {
            return if crate::md::tags(self.files()).is_empty() { "foot-tags".into() } else { "tags".into() };
        }
        let tab = self.shelved().map(|t| t.key());
        // A folded vault shows no tree: the active tab, else the vault's label.
        if tab.is_some() && self.persist.sidebar.folded == Some(crate::sidebar::Section::Vault) {
            return tab.unwrap_or_else(|| "label-vault".into());
        }
        let shows = |path: &str| {
            let dir = crate::md::dir(path);
            dir.is_empty() || dir.split('/').scan(String::new(), |at, part| {
                if !at.is_empty() {
                    at.push('/');
                }
                at.push_str(part);
                Some(at.clone())
            }).all(|d| self.expanded.contains(&d))
        };
        match self.active.as_deref() {
            Some(a) if shows(a) => crate::folders::row_key("file", a),
            _ if !self.files().is_empty() || self.vault.as_ref().is_some_and(|v| !v.folders.is_empty()) => "tree".into(),
            _ => tab.unwrap_or_else(|| "search".into()),
        }
    }

    /// Where F6 puts the keyboard in the outline: the heading being read or written, else the
    /// first row.
    fn outline_stop(&self) -> String {
        let doc = self.active_text();
        let heads = crate::md::headings(doc);
        let shown: Vec<usize> = (0..heads.len()).filter(|&i| heads[i].level <= 4).collect();
        let cur = if self.mode == Mode::Source {
            let caret = self.active.as_ref().and_then(|a| self.edits.get(a)).map_or(0, |e| e.focus.min(doc.len()));
            let (line, _) = crate::md::line_col(doc, caret);
            shown.iter().rposition(|&i| heads[i].line <= line)
        } else {
            self.reader.map.borrow().marked
        };
        match cur.or((!shown.is_empty()).then_some(0)) {
            Some(n) => format!("ol-{}", shown[n]),
            None => "outline-links".into(),
        }
    }
}
