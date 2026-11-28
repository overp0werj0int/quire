//! Selectable choice cards, toggle/radio pills, and checkboxes composed from shared tokens.
//!
//! Used by Settings (theme cards, font cards, sync provider cards, choice pills, MCP tool
//! checkboxes and folder toggles).

use rune::{Color, El, Tx, col, icon, row};

use super::Control;
use crate::app::App;
use crate::theme::Pal;
use crate::ui::icons::CHECK;
use crate::ui::tokens::{FocusRing, radius, size, space};

/// Selectable bordered card (`radius::FIELD`, `role("radio")` inside a `radiogroup`
/// `focus_group`). Active selection highlighting is painted by a shared sliding
/// `components::SelectionGlass` on the containing grid so all spatial pickers share Map's look.
pub fn choice_card(
    p: &Pal,
    key: impl Into<String>,
    aria_label: impl Into<String>,
    selected: bool,
    border_idle: Option<Color>,
    run: impl Fn(&mut App, &mut Tx) + 'static,
) -> El {
    FocusRing::Standard.apply(
        col()
            .key(key.into())
            .radius(radius::FIELD)
            .border(1.0, border_idle.unwrap_or(p.line))
            .border_layout()
            .label(aria_label.into())
            .choice(selected, run),
        p,
    )
}

/// Compact `28 pt` selectable pill (`radius::CONTROL`, `space::PAD_INPUT_X`, `FocusRing::Standard`),
/// used for radio option chips (`is_radio = true`) and multi-select toggle chips (`is_radio = false`).
pub fn choice_pill(
    p: &Pal,
    key: impl Into<String>,
    aria_label: impl Into<String>,
    selected: bool,
    is_radio: bool,
    run: impl Fn(&mut App, &mut Tx) + 'static,
) -> El {
    let base = row()
        .key(key.into())
        .h(size::CONTROL_PANEL)
        .px(space::PAD_INPUT_X)
        .center()
        .radius(radius::CONTROL)
        .border(1.0, if selected { p.acc } else { p.line })
        .border_layout()
        .hover(p.hover)
        .label(aria_label.into());
    let el = if is_radio {
        base.choice(selected, run)
    } else {
        base.role("checkbox").checked(selected).control(run)
    };
    FocusRing::Standard.apply(el, p)
}

/// `16 × 16` checkbox box (`radius::INNER`, accent fill + `CHECK` glyph when checked).
pub fn checkbox_box(p: &Pal, checked: bool) -> El {
    let mut bx = row()
        .key("box")
        .size(space::PX_16, space::PX_16)
        .center()
        .radius(radius::INNER)
        .border(1.5, if checked { p.button } else { p.mark })
        .border_layout();
    if checked {
        bx = bx.fill(p.button).child(icon(&CHECK, 11.0).color(p.on_acc));
    }
    bx
}
