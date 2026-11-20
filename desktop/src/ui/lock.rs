//! Locked notes on screen: the panel that asks for the password, and the dialog that sets one.

use crate::roles::Roles;
use rune::{El, TextEdit, Tx, col, el, icon, row, text_input};

use super::V;
use super::layer::Card;
use super::icons::LOCK;
use super::widgets::{label, outline_button, primary_button};
use crate::app::App;
use super::styles::sans;
use crate::theme;

/// A password field kept in `App::fields`. Enter runs `submit`.
fn password(v: V, key: &'static str, ph: &'static str, bad: bool, submit: fn(&mut App, &mut Tx)) -> El {
    let p = v.p;
    let edit = v.s.fields.get(key).cloned().unwrap_or_else(|| TextEdit::new("")).secure();
    super::widgets::input_focus_ring(
        row()
            .key(format!("box-{key}"))
            .w_full()
            .h(32.0)
            .center_y()
            .radius(7.0)
            .fill(p.bg)
            .border(1.0, if bad { p.err } else { p.line })
            .border_layout()
            .clip_rounded(7.0),
        p,
    )
    .child(
            text_input(&edit, sans(13.0, 400.0))
                .key(key)
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .px(10.0)
                .color(p.ink)
                .placeholder(ph, super::styles::placeholder_ink(p))
                .caret(p.ink, p.sel)
                .label(ph)
                .on_edit(move |s: &mut App| s.field(key, String::new))
                .on_submit(submit)
                .on_input_key(|_: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx| crate::keys::esc_again(ev, tx)),
        )
}

/// In place of a locked note's text: the password field and the Unlock button.
pub fn panel(v: V) -> El {
    let p = v.p;
    let err = v.s.unlock_error.clone();
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
                .child(el().grow(1.0).min_w(0.0).child(password(v, "unlock-pass", "Password", err.is_some(), |s, tx| s.unlock(tx))))
                .child(primary_button(p, "unlock", "Unlock", 32.0, 14.0, 13.0, |s, tx| s.unlock(tx))),
        );
    if let Some(e) = err {
        c = c.child(label(e, sans(12.5, 400.0), p.err).wrap());
    }
    // The column of the note is wider than the panel; the panel sits at its left edge.
    row().key("lock-wrap").w_full().child(c)
}

/// The dialog that locks the active note with a new password.
pub(super) fn dialog(v: V) -> Card {
    let p = v.p;
    let err = v.s.lock_error().map(str::to_string);
    let field = |key: &'static str, title: &str, ph: &'static str| {
        col().w_full().gap(6.0).child(label(title, sans(12.5, 500.0), p.ink2)).child(password(v, key, ph, err.is_some(), |s, tx| s.lock_note(tx)))
    };
    let mut panel = col()
        .key("lock-dialog")
        .w_full()
        .max_w(400.0)
        .padding(24.0)
        .gap(16.0)
        .radius(12.0)
        .fill(p.bg)
        .border(1.0, Roles::new(p).hairline)
        .box_shadow(theme::settings_shadows()[0])
        .box_shadow(theme::settings_shadows()[1])
        .role("dialog")
        .label("Lock this note")
        .child(
            col()
                .gap(6.0)
                // The same styles as a settings row, so the dialog matches the rest of the app.
                .child(label("Lock this note", sans(14.0, 500.0), p.ink))
                .child(label("Quire encrypts the note with this password. If you forget it, nobody can open the note.", sans(12.5, 400.0).lh(12.5 * 1.45), p.mute).wrap()),
        )
        .child(field("lock-pass", "Password", "At least 8 characters"))
        .child(field("lock-repeat", "Repeat password", "The same password again"));
    if let Some(e) = err {
        panel = panel.child(label(e, sans(12.5, 400.0), p.err).wrap());
    }
    panel = panel.child(
        row()
            .w_full()
            .gap(8.0)
            .mt(4.0)
            .child(el().grow(1.0))
            .child(outline_button(p, "lock-cancel", "Cancel", 32.0, |s, tx| s.close_layer(tx)))
            .child(primary_button(p, "lock-ok", "Lock", 32.0, 14.0, 13.0, |s, tx| s.lock_note(tx))),
    );
    Card { el: panel, plane: Some(col().center().px(16.0)) }
}
