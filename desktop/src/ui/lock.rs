//! Locked notes on screen: the panel that asks for the password, and the dialog that sets one.

use rune::{El, Tx, col, el, icon, row};

use super::V;
use super::components::{Button, dialog_frame, dialog_plane, error_line, label, password_field};
use super::layer::Card;
use super::icons::LOCK;
use super::styles::{self, sans};
use super::tokens::{size, space};
use crate::app::App;
use crate::layer::LockDialog;

/// Key of the lock panel's password field.
pub const PASS: &str = "unlock-pass";

fn password(v: V, key: &'static str, ph: &'static str, bad: bool, submit: fn(&mut App, &mut Tx)) -> El {
    let on_change: fn(&mut App) = if key == PASS {
        |s| s.unlock_error = None
    } else {
        |s| s.clear_lock_dialog_error()
    };
    password_field(v, key, ph, bad, on_change, submit)
}

/// In place of a locked note's text: the password field and the Unlock button.
pub fn panel(v: V) -> El {
    let p = v.p;
    let err = v.s.unlock_error.clone();
    let busy = v.s.lock_busy() == Some(crate::lock_app::LockJobKind::Unlocking);
    let submit_btn = Button::primary(p, "unlock", if busy { "Unlocking…" } else { "Unlock" })
        .busy(busy)
        .busy_label_key("lock-busy")
        .on_press(|s, tx| s.unlock(tx));
    let mut c = col()
        .key("lock-panel")
        .w_full()
        .max_w(360.0)
        .mt(56.0)
        .gap(10.0)
        .child(icon(&LOCK, 28.0).color(p.mute).mb(6.0))
        .child(label("This note is locked.", sans(17.0, 600.0), p.ink))
        .child(label("Enter the password to read and edit it.", sans(13.0, 400.0).lh(13.0 * 1.45), p.mute).wrap())
        .child(
            row()
                .w_full()
                .gap(8.0)
                .mt(8.0)
                .center_y()
                .child(el().grow(1.0).min_w(0.0).child(password(v, PASS, "Password", err.is_some(), |s, tx| s.unlock(tx))))
                .child(submit_btn),
        );
    if let Some(e) = err {
        c = c.child(super::components::error_line(p, e));
    }
    // The column of the note is wider than the panel; the panel sits at its left edge.
    row().key("lock-wrap").w_full().child(c)
}

/// The dialog that locks the active note with a new password, or confirms removing it.
pub(super) fn dialog(v: V, dlg: &LockDialog) -> Card {
    let p = v.p;
    if matches!(dlg, LockDialog::ConfirmRemove) {
        let remove = Button::destructive(p, "remove-pass-ok", "Remove password")
            .on_press(|s: &mut App, tx| s.confirm_remove_password(tx));
        let panel = dialog_frame(p, "remove-pass-dialog", Some(size::DIALOG_W_SM))
            .role("alertdialog")
            .label("Remove password?")
            .child(
                col()
                    .w_full()
                    .padding(space::PAD_DIALOG)
                    .gap(space::GAP_SECTION)
                    .child(
                        col()
                            .gap(space::GAP_FIELD)
                            .child(label("Remove password?", styles::title(), p.ink))
                            .child(label("This note will be saved as plain text on disk and will no longer require a password to read.", styles::desc(), p.mute).wrap()),
                    )
                    .child(
                        row()
                            .w_full()
                            .gap(space::GAP_CONTROL)
                            .mt(space::PX_4)
                            .child(el().grow(1.0))
                            .child(Button::outline(p, "remove-pass-cancel", "Cancel").autofocus().on_press(|s, tx| s.close_layer(tx)))
                            .child(remove),
                    ),
            );
        return Card { el: panel, plane: Some(dialog_plane()) };
    }
    let (changing, err) = match dlg {
        LockDialog::Set(e) => (false, e.as_deref()),
        LockDialog::Change(e) => (true, e.as_deref()),
        LockDialog::ConfirmRemove => (false, None),
    };
    let title = if changing { "Change password" } else { "Lock this note" };
    let desc = if changing {
        "Re-encrypts this note with a new password without writing plain text to disk. If you forget it, nobody can open the note."
    } else {
        "Quire encrypts the note with this password. If you forget it, nobody can open the note."
    };
    let field = |key: &'static str, title: &str, ph: &'static str, submit: fn(&mut App, &mut Tx)| {
        col().w_full().gap(space::GAP_FIELD).child(label(title, styles::control_medium(), p.ink2)).child(password(v, key, ph, err.is_some(), submit))
    };
    let focus_repeat: fn(&mut App, &mut Tx) = |s, tx| {
        s.field("lock-repeat", String::new).focused = true;
        tx.focus("lock-repeat", false);
    };
    let mut inner = col()
        .w_full()
        .padding(space::PAD_DIALOG)
        .gap(space::GAP_SECTION)
        .child(
            col()
                .gap(space::GAP_FIELD)
                .child(label(title, styles::title(), p.ink))
                .child(label(desc, styles::desc(), p.mute).wrap()),
        )
        .child(field("lock-pass", if changing { "New password" } else { "Password" }, "At least 8 characters", focus_repeat))
        .child(field("lock-repeat", if changing { "Repeat new password" } else { "Repeat password" }, "The same password again", |s, tx| s.lock_note(tx)));
    if let Some(e) = err {
        inner = inner.child(error_line(p, e));
    }
    let busy = v.s.lock_busy() == Some(crate::lock_app::LockJobKind::Locking);
    let ok_label = if busy {
        if changing { "Changing…" } else { "Locking…" }
    } else if changing {
        "Change"
    } else {
        "Lock"
    };
    let lock_btn = Button::primary(p, "lock-ok", ok_label)
        .busy(busy)
        .busy_label_key("lock-busy")
        .on_press(|s, tx| s.lock_note(tx));
    inner = inner.child(
        row()
            .w_full()
            .gap(space::GAP_CONTROL)
            .mt(space::PX_4)
            .child(el().grow(1.0))
            .child(Button::outline(p, "lock-cancel", "Cancel").on_press(|s, tx| s.cancel_lock(tx)))
            .child(lock_btn),
    );
    let panel = dialog_frame(p, "lock-dialog", Some(size::DIALOG_W_SM))
        .role("dialog")
        .label(title)
        .child(inner);
    Card { el: panel, plane: Some(dialog_plane()) }
}
