//! omarky's chip button (BLK-100): a quiet icon with an optional label that takes the hover
//! fill and a hairline under the pointer or while its feature is on (RD-35).

use rune::{Color, El, Family, Icon, TextStyle, el, icon, row, text};

use crate::roles::Roles;

/// Colour changes on a chip take 120 ms (BLK-100).
pub(super) fn fade() -> rune::Anim {
    rune::Curve::linear(0.12).into()
}

pub(super) struct Chip<'a> {
    pub key: &'a str,
    pub glyph: &'static Icon,
    /// The type size: the chip is twice this tall.
    pub size: f32,
    /// The drawn icon's size. omarky's glyphs fill the type size; Quire's icons differ in
    /// optical size, so each caller picks one. An icon-only chip is as wide as a glyph of
    /// the type size would make it.
    pub icon: f32,
    pub label: &'a str,
    pub mono: Family,
    /// The icon and label colour at rest. They turn to text when hovered or active.
    pub tint: Color,
    pub active: bool,
    /// A fill that replaces the hover fill: the code block's Copy chip sits on the panel colour.
    pub fill: Option<Color>,
}

impl Chip<'_> {
    pub fn el(self, c: &Roles) -> El {
        let s = self.size;
        let h = r(s * 2.0);
        let pad = r(s * if self.label.is_empty() { 1.1 } else { 1.3 });
        let ink = if self.active { c.text } else { self.tint };
        let mut out = row()
            .key(self.key)
            .h(h)
            .px(pad / 2.0)
            .center()
            .gap(r(s * 0.45))
            .radius(3.0)
            .hover_group()
            .hover_motion(fade(), fade())
            .cursor(rune::Cursor::Pointer)
            .role("button")
            .child(icon(self.glyph, self.icon).color(ink).group_color(c.text));
        if self.label.is_empty() {
            out = out.w(s + pad);
        } else {
            let style = TextStyle::new(s, 400.0).family(self.mono).lh(h);
            out = out.child(text(self.label.to_string()).font(style).color(ink).group_color(c.text));
        }
        // The hairline, at once while active, else with the pointer.
        let line = el().key("edge").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(3.0).border(1.0, c.hairline);
        out = out.child(if self.active { line } else { line.group_opacity(0.0, 1.0) });
        match self.fill {
            Some(f) => out.fill(f),
            None if self.active => out.fill(c.hover),
            None => out.hover(c.hover),
        }
    }
}

fn r(x: f32) -> f32 {
    x.round()
}
