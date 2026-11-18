//! The view's half of the focus model (`crate::focus`): what a pane, a stop and the note's
//! anchor carry, so the keyboard always has a place to go back to.

use rune::{El, KeyEvent, Tx, el};

use crate::app::App;
use crate::focus::{NOTE, Pane};

/// A pane (the sidebar, the outline): with the keyboard in it, the key that hides the pane
/// hides it, ahead of a list's letters; when the pane goes away, the keyboard goes to the note.
pub(super) fn pane(el: El, pane: Pane) -> El {
    el.focus_return(NOTE).on_key(move |s: &mut App, ev: &KeyEvent, tx| s.pane_toggle_key(pane, ev, tx))
}

/// A stop: when it goes away with the keyboard on it, the keyboard goes to the note.
pub(super) fn stop(el: El) -> El {
    el.focus_return(NOTE)
}

/// The note's keyboard where no editor shows (the reader, a locked note): a point no one sees
/// and Tab skips, which focus that lost its element comes back to. It hands every key to the
/// window's shortcuts, as the reader does with nothing focused. Tab goes on from it into the
/// note's links.
pub(super) fn note_anchor() -> El {
    el().key(NOTE).absolute(0.0, 0.0).size(1.0, 1.0).focusable(true).tab_index(-1).no_focus_ring().label("Note").on_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| crate::keys::on_key(s, ev, tx))
}

/// The editor's box in the writer, keyed as the note's anchor so focus comes back to the
/// editor.
pub(super) fn note_box(el: El) -> El {
    el.key(NOTE)
}
