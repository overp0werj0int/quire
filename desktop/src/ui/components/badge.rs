//! Unified count pills and readable error/status lines (`SZ-09`, `LK-09`).

use rune::{Color, El, el, row, text};

use super::padded;
use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::styles::{self, error_ink, nav};
use crate::ui::tokens::{radius, space};

/// Unified numeric count pill (11 pt panel font, `SZ-09`).
pub fn count_pill(p: &Pal, key: impl Into<String>, count: usize) -> El {
    let c = Roles::panel(p);
    row()
        .key(key.into())
        .h(16.0)
        .px(space::PX_6)
        .center()
        .radius(radius::PILL)
        .fill(c.hover)
        .child(text(count.to_string()).font(nav(11.0, false)).color(c.faint))
}

/// Accent-tinted tag badge (`radius::INNER`, `p.soft` fill, `p.acc` medium ink), used for palette
/// mode badges (`open`, `run`, `move`) and MCP capability tags (`writes`).
pub fn tag_badge(p: &Pal, label_: impl Into<String>) -> El {
    padded(label_, styles::sans(styles::scale::SMALL, 500.0), p.acc, space::PX_6, space::PX_2, 0.0)
        .radius(radius::INNER)
        .fill(p.soft)
}

/// Small 6 pt circular status indicator dot.
pub fn status_dot(color: Color) -> El {
    el().size(space::PX_6, space::PX_6).radius(radius::PILL).fill(color)
}

/// Unified readable error line (`LK-09`): `styles::control()` (12.5 pt) in `error_ink(p)`
/// (`>= 5.0:1` across all 15 themes), wrapped.
pub fn error_line(p: &Pal, msg: impl Into<String>) -> El {
    text(msg.into())
        .font(styles::control())
        .color(error_ink(p))
        .wrap()
        .shrink()
        .min_w(0.0)
}
