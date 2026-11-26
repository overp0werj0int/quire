//! Unified `<kbd>` key cap (`SZ-07`, `LK-01`): 10 pt compact (dense rows and footers) or 11 pt
//! full (shortcut tables and headers), inner radius 4 pt, panel `dim` ink (`>= 4.5:1`).

use rune::{Color, El, row, text};

use super::label;
use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::styles::{self, CAPTION, keys};
use crate::ui::tokens::{radius, space};

pub fn kbd(p: &Pal, label_: impl Into<String>, compact: bool) -> El {
    let c = Roles::panel(p);
    let (sz, lh, px, py) = if compact {
        (CAPTION, 12.0, space::PX_4, space::PX_2)
    } else {
        (11.0, 14.0, space::PX_6, space::PX_2)
    };
    row()
        .key("kbd")
        .px(px)
        .py(py)
        .radius(radius::INNER)
        .fill(c.raised)
        .border(1.0, c.hairline)
        .border_layout()
        .center()
        .child(text(label_.into()).font(keys(sz).lh(lh)).color(c.dim))
}

/// Compact key cap paired with a muted hint caption (used in palette footers and help search).
pub fn key_hint(p: &Pal, key_text: impl Into<String>, caption: impl Into<String>, caption_color: Color) -> El {
    row()
        .gap(space::PX_4)
        .center_y()
        .child(kbd(p, key_text, true))
        .child(label(caption, styles::small(), caption_color))
}
