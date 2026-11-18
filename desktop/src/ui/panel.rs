//! The side panels' type and spacing, taken from omarky's outline panel
//! (`components/Outline.qml`, decision 0008). omarky sizes its chrome from a 12 px base:
//! captions 10 px, small body 11 px, body 12 px, all in the panel family (`styles::nav`).

use rune::{Color, El, TextStyle, row};

use crate::roles::{Roles, mix};
use super::styles::{nav, panel_line};

pub const CAPTION: f32 = 10.0;
pub const BODY_SMALL: f32 = 11.0;
pub const BODY: f32 = 12.0;
/// A caption's distance from the top of its section. Its left edge is the rows' text edge,
/// `TEXT_X`.
pub const CAPTION_Y: f32 = 18.0;
/// From a caption to the first row under it.
pub const CAPTION_GAP: f32 = 10.0;
/// The room a focused row's ring takes past the row's edge (`ui::focus_ring`), which a side
/// list keeps clear at its ends (RING).
pub const RING_ROOM: f32 = 3.0;
/// The row shade's inset from each side of the panel.
pub const INSET: f32 = 10.0;
/// A row's text starts here, plus `INDENT` a level.
pub const TEXT_X: f32 = 22.0;
pub const INDENT: f32 = 14.0;
/// A row's text ends this far from the panel's right edge.
pub const RIGHT: f32 = 18.0;
const RADIUS: f32 = 3.0;

/// A row is its text's line plus 12 px.
pub fn row_h(size: f32) -> f32 {
    panel_line(size) + 12.0
}

/// A row's shade: inset `INSET` each side, the hover colour under the pointer, a tenth of the
/// accent on the current row. `lead` is the room left of the text for a chevron.
pub fn shade(key: impl Into<String>, size: f32, depth: usize, lead: f32, c: &Roles, current: bool) -> El {
    row()
        .key(key.into())
        .h(row_h(size))
        .mx(INSET)
        .pl(TEXT_X - INSET - lead + depth as f32 * INDENT)
        .pr(RIGHT - INSET)
        .center_y()
        .radius(RADIUS)
        .fill(shade_fill(c, current))
        .hover(c.hover)
}

/// A row's fill: a wash of the accent when current.
pub fn shade_fill(c: &Roles, current: bool) -> Color {
    if current { c.accent.alpha(0.1) } else { Color::TRANSPARENT }
}

/// The current row's dot: the accent, sitting 13 px left of the row's text (`DOT_GAP`).
pub fn dot_fill(c: &Roles, current: bool) -> Color {
    if current { c.accent } else { Color::TRANSPARENT }
}

/// The dot's size and the room between it and the row's text.
pub const DOT: f32 = 5.0;
pub const DOT_GAP: f32 = 13.0;

/// The text colour of a row: the text colour when current, dim at the top level, between dim
/// and faint below it.
pub fn ink(c: &Roles, depth: usize, current: bool) -> Color {
    if current {
        c.text
    } else if depth == 0 {
        c.dim
    } else {
        mix(c.dim, c.faint, 0.5)
    }
}

/// Body text of a row at `depth`: 12 px bold at the top level, 11 px below.
pub fn row_font(depth: usize) -> TextStyle {
    if depth == 0 { nav(BODY, true) } else { nav(BODY_SMALL, false) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme_pal;

    /// RD-45 and RD-47: rows are the line plus 12 px, so 28 px at body size and 26 px at small
    /// body, as omarky's are with JetBrains Mono.
    #[test]
    fn rd_45_row_heights() {
        assert_eq!((row_h(BODY), row_h(BODY_SMALL)), (28.0, 26.0));
    }

    /// RD-46: the current row in the text colour, dim at the top, between dim and faint below.
    #[test]
    fn rd_46_row_colours() {
        let c = Roles::new(theme_pal("paper", false));
        assert_eq!(ink(&c, 3, true), c.text);
        assert_eq!(ink(&c, 0, false), c.dim);
        assert_eq!(ink(&c, 1, false), mix(c.dim, c.faint, 0.5));
    }
}
