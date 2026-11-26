//! Unified single-line text inputs, password/secret fields, search boxes and text areas
//! (`SZ-04`, `LK-08`, `LK-12`).
//!
//! Two standard heights: `InputSize::Dialog` (32 pt, radius 8) for forms and dialogs, and
//! `InputSize::Panel` (28 pt, radius 6) for panel search boxes.

use rune::{El, ScrollSpec, TextEdit, Tx, col, row, text, text_input};

use super::{Control, label};
use crate::app::App;
use crate::theme::Pal;
use crate::ui::V;
use crate::ui::styles::{self, literal, placeholder_ink};
use crate::ui::tokens::{FocusRing, radius, size, space};

/// Standard single-line input heights and corner radii (Decisions 3A and 6A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSize {
    /// Dialog and form text fields: 32 pt tall, radius 8.
    Dialog,
    /// Compact panel search field: 28 pt tall, radius 6.
    Panel,
}

impl InputSize {
    pub const fn height(self) -> f32 {
        match self {
            InputSize::Dialog => size::CONTROL_DIALOG,
            InputSize::Panel => size::CONTROL_PANEL,
        }
    }

    pub const fn radius(self) -> f32 {
        match self {
            InputSize::Dialog => radius::FIELD,
            InputSize::Panel => radius::CONTROL,
        }
    }
}

/// Shared bordered frame for single-line text inputs with scoped `FocusRing::Input` (`SB-95`).
pub fn input_frame(p: &Pal, key: impl Into<String>, sz: InputSize, bad: bool, focused: bool) -> El {
    let rad = sz.radius();
    FocusRing::Input.apply_when(
        row()
            .key(key.into())
            .w_full()
            .h(sz.height())
            .center_y()
            .radius(rad)
            .fill(p.bg)
            .border(1.0, if bad { p.err } else { p.line })
            .border_layout()
            .clip_rounded(rad),
        p,
        focused,
    )
}

/// Shared bordered frame for multi-line text areas (such as Sync credentials JSON).
pub fn area_frame(p: &Pal, key: impl Into<String>, h: f32, bad: bool, focused: bool) -> El {
    FocusRing::Input.apply_when(
        col()
            .key(key.into())
            .w_full()
            .h(h)
            .py(space::PX_8)
            .px(space::PAD_INPUT_X)
            .radius(radius::FIELD)
            .fill(p.bg)
            .border(1.0, if bad { p.err } else { p.line })
            .border_layout()
            .scroll(ScrollSpec { stick_bottom: false }),
        p,
        focused,
    )
}

/// Trailing `show` / `hide` toggle inside a secret input or token box.
pub fn secret_toggle(
    p: &Pal,
    key: impl Into<String>,
    revealed: bool,
    aria_noun: &str,
    on_toggle: impl Fn(&mut App, &mut Tx) + 'static,
) -> El {
    let tip = if revealed { format!("Hide {aria_noun}") } else { format!("Show {aria_noun}") };
    row()
        .key(key.into())
        .h_full()
        .px(space::PAD_INPUT_X)
        .center()
        .border_sides([0.0, 0.0, 0.0, 1.0], p.line)
        .border_layout()
        .hover(p.hover)
        .hover_group()
        .role("button")
        .label(tip.clone())
        .help(tip)
        .control(on_toggle)
        .child(label(if revealed { "hide" } else { "show" }, styles::small(), p.mute).group_color(p.ink))
}

/// Password field backed by `App::fields` with built-in `show` / `hide` toggle and submit handler.
pub fn password_field(
    v: V,
    key: &'static str,
    ph: &'static str,
    bad: bool,
    on_change: fn(&mut App),
    submit: fn(&mut App, &mut Tx),
) -> El {
    let p = v.p;
    let revealed = v.s.reveal.contains(key);
    let mut edit = v.s.fields.get(key).cloned().unwrap_or_else(|| TextEdit::new(""));
    if !revealed {
        edit = edit.secure();
    }
    input_frame(p, format!("box-{key}"), InputSize::Dialog, bad, edit.focused)
        .child(
            text_input(&edit, styles::field())
                .key(key)
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .px(space::PAD_INPUT_X)
                .color(p.ink)
                .placeholder(ph, placeholder_ink(p))
                .caret(p.ink, p.sel)
                .label(ph)
                .on_edit(move |s: &mut App| s.field(key, String::new))
                .on_input_change(move |s: &mut App, _| on_change(s))
                .on_submit(submit)
                .on_input_key(|_: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx| crate::keys::esc_again(ev, tx)),
        )
        .child(secret_toggle(p, format!("reveal-{key}"), revealed, "password", move |s: &mut App, _: &mut Tx| {
            if !s.reveal.remove(key) {
                s.reveal.insert(key.into());
            }
            if let Some(e) = s.fields.get_mut(key) {
                e.set_secure(!s.reveal.contains(key));
            }
        }))
}

/// Settings literal text field (32 pt, radius 8) backed by `App::fields`, with optional secret
/// masking and `show` / `hide` button.
pub fn literal_field(
    v: V,
    fk: &str,
    aria_label: &str,
    value: String,
    ph: &'static str,
    secret: bool,
    bad: bool,
    max_w: f32,
    set: impl Fn(&mut App, String) + 'static,
) -> El {
    let p = v.p;
    let s = v.s;
    let revealed = s.reveal.contains(fk);
    let mut edit = s.fields.get(fk).cloned().unwrap_or_else(|| TextEdit::new(value.clone()));
    if secret && !revealed {
        edit = edit.secure();
    }
    let focused = edit.focused;
    let fk_edit = fk.to_string();
    let fk_change = fk.to_string();
    let init = value.clone();
    let mut b = input_frame(p, format!("box-{fk}"), InputSize::Dialog, bad, focused).child(
        text_input(&edit, literal(styles::scale::CONTROL))
            .key(fk.to_string())
            .single_line()
            .grow(1.0)
            .min_w(0.0)
            .px(space::PAD_INPUT_X)
            .color(p.ink)
            .label(aria_label)
            .placeholder(ph, placeholder_ink(p))
            .caret(p.ink, p.sel)
            .on_edit(move |s: &mut App| s.field(&fk_edit, || init.clone()))
            .on_input_change(move |s: &mut App, _| {
                let t = s.fields.get(&fk_change).map(|e| e.text.clone()).unwrap_or_default();
                set(s, t);
            })
            .on_input_key(|_: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx| crate::keys::esc_again(ev, tx)),
    );
    if max_w.is_finite() {
        b = b.max_w(max_w);
    }
    if secret {
        let rk = fk.to_string();
        b = b.child(secret_toggle(p, "reveal", revealed, "value", move |s: &mut App, _: &mut Tx| {
            if !s.reveal.remove(&rk) {
                s.reveal.insert(rk.clone());
            }
            if let Some(e) = s.fields.get_mut(&rk) {
                e.set_secure(!s.reveal.contains(&rk));
            }
        }));
    }
    b
}

/// Read-only masked secret display box (such as the MCP access token) sharing `InputSize::Dialog`
/// geometry and `secret_toggle`.
pub fn secret_display(v: V, box_key: &'static str, reveal_key: &'static str, value: &str) -> El {
    let p = v.p;
    let revealed = v.s.reveal.contains(reveal_key);
    let shown = if revealed { value.to_string() } else { "•".repeat(value.chars().count()) };
    input_frame(p, box_key, InputSize::Dialog, false, false)
        .grow(1.0)
        .min_w(0.0)
        .child(
            text(shown)
                .font(literal(styles::scale::CONTROL))
                .color(p.ink)
                .selectable(revealed)
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .px(space::PAD_INPUT_X),
        )
        .child(secret_toggle(p, "reveal", revealed, "token", move |s: &mut App, _: &mut Tx| {
            if !s.reveal.remove(reveal_key) {
                s.reveal.insert(reveal_key.into());
            }
        }))
}
