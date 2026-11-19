//! The side panels' lists from the keyboard (TREE): the file tree, the Open list, the search
//! results, the tags, and the outline's headings and backlinks. Every list works the same way,
//! as a Rune focus group:
//!
//! - It is one Tab stop. Tab enters it at its current row (the open note, the heading being
//!   read), else its first, and the next Tab leaves it.
//! - Up and Down move between rows and stop at the ends; Home and End go to the first and last.
//! - Enter or Space does what a click does. A row that shows a note then hands the keyboard to
//!   it (`opener`).
//! - The Menu key or Shift+F10 opens the row's menu, where it has one, as a right-click does:
//!   Rune opens it at the row's bottom-left, with its first item marked.
//! - Esc gives the keyboard back to the note.
//! - The focused row wears the app's focus ring, apart from the current row's fill.
//!
//! A click on a row leaves the keyboard where it was, so a click in the sidebar never takes it
//! from the note or the reader. Only Tab and the arrows put focus on a row.

use rune::{El, Key, KeyEvent, Mods, Tx};

use crate::app::App;
use crate::theme::Pal;

/// A list of `item`s, one Tab stop, under `role` (`list` or `tree`).
pub(super) fn list(el: El, role: &'static str) -> El {
    el.role(role).focus_group().on_key(|s: &mut App, ev: &KeyEvent, tx| {
        // In the sidebar's drawer, Esc closes the drawer, which hands the keyboard back.
        if crate::keys::esc_again(ev, tx) {
            return true;
        }
        if ev.key != Key::Escape || ev.mods != Mods::default() || s.layer().is_some() {
            return false;
        }
        if !s.esc_takes_notice() {
            s.keys_to_note(tx);
        }
        true
    })
}

/// A row of a `list`. Tab lands on it when it is `current`; a click, Enter or Space runs `run`.
pub(super) fn item(el: El, p: &Pal, current: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let (w, gap, c) = super::focus_ring(p);
    super::focus::stop(el).focusable(true).focus_selected(current).focus_ring(w, gap, c).keeps_text_selection().on_press_tx(run)
}

/// An `item` that shows a note: after `run`, the keyboard goes to the note, so the reader's
/// keys or the editor take the next key. In a drawer, the drawer closes first, even when the
/// row's note was open already and nothing under the drawer changed.
pub(super) fn opener(el: El, p: &Pal, current: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    item(el, p, current, move |s, tx| {
        run(s, tx);
        s.close_drawer();
        s.keys_to_note(tx);
    })
}

/// What a tree row is, for Left and Right.
pub(super) enum Branch {
    Note,
    /// A folder, by path, whether it is open, and its first row inside when it has one.
    Folder { path: String, open: bool, first: Option<String> },
}

/// A tree row's Left and Right: Right opens a closed folder or moves into an open one; Left
/// closes an open folder or moves to the row's folder, keyed `parent`.
pub(super) fn tree_arrows(el: El, branch: Branch, parent: Option<String>) -> El {
    el.on_key(move |s: &mut App, ev: &KeyEvent, tx| {
        if ev.mods != Mods::default() {
            return false;
        }
        match (&ev.key, &branch) {
            (Key::Right, Branch::Folder { path, open: false, .. }) | (Key::Left, Branch::Folder { path, open: true, .. }) => s.toggle_folder(path),
            (Key::Right, Branch::Folder { first: Some(k), .. }) => tx.focus(k.clone(), true),
            (Key::Left, _) => {
                if let Some(k) = &parent {
                    tx.focus(k.clone(), true);
                }
            }
            (Key::Right, _) => {}
            _ => return false,
        }
        true
    })
}
