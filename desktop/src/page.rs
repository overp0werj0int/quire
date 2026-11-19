//! The reader page's numbers: the column, the type scale, the vertical rhythm and table widths,
//! as `docs/reader-writer/behavior/page-and-typography.md` and `blocks.md` give them. Pure
//! functions, so the unit tests below read like the spec's worked values.

use rune::Family;

/// JavaScript's `Math.round`: half rounds up.
pub fn r(x: f32) -> f32 {
    (x + 0.5).floor()
}

/// `size(step)` = round(B × 1.25^step) (PAGE-13).
pub fn step(b: f32, step: f32) -> f32 {
    r(b * 1.25f32.powf(step))
}

/// A line box: round(size × leading) (PAGE-11).
pub fn line(size: f32, leading: f32) -> f32 {
    r(size * leading)
}

/// A heading's text size and leading at `level` 1 to 6, at body size `b` (PAGE-03, PAGE-14).
pub fn heading_type(b: f32, level: usize) -> (f32, f32) {
    let level = level.clamp(1, 6);
    (step(b, [3.0, 2.0, 1.0, 0.5, 0.0, 0.0][level - 1]), if level <= 2 { 1.25 } else { 1.35 })
}

/// The text size of a code or diff block, at body size `b`.
pub fn code_size(b: f32) -> f32 {
    r(b * 0.84)
}

/// The text size inside a quote, at body size `b` (BLK-33).
pub fn quote_size(b: f32) -> f32 {
    r(b * 0.94)
}

/// How tall a line of a top-level block's own text is, at body size `b`, as the reader draws
/// it: a heading's, a code block's or a quote's, else a body line. Find centres a far match by
/// it before its row is built (`App::center_match`).
pub fn text_line(b: f32, kind: &crate::blocks::Kind) -> f32 {
    use crate::blocks::Kind;
    match kind {
        Kind::Heading { level, .. } => {
            let (size, lead) = heading_type(b, *level);
            line(size, lead)
        }
        Kind::Code { .. } | Kind::Diff { .. } => line(code_size(b), 1.5),
        Kind::Quote { .. } => line(quote_size(b), 1.6),
        _ => line(b, 1.6),
    }
}

/// The OS/2 `xAvgCharWidth` of a bundled face, in em (PAGE-21).
pub fn avg_char(f: Family) -> f32 {
    match f {
        Family::Named("Newsreader") => 0.5435,
        Family::Named("Instrument Sans") => 0.569,
        Family::Named("iA Writer Quattro S") => 0.608,
        _ => 0.6,
    }
}

/// The OS/2 x-height of a bundled face, in em, for centring markers (BLK-25).
pub fn x_height(f: Family) -> f32 {
    match f {
        Family::Named("Newsreader") => 0.426,
        Family::Named("Instrument Sans") => 0.51,
        Family::Named("Courier Prime") => 0.4512,
        Family::Named("Spline Sans Mono") => 0.5455,
        _ => 0.516,
    }
}

/// Where the column sits in the page area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    /// From the page area's left edge to the column's.
    pub gutter: f32,
    pub width: f32,
}

/// The column in a page area `view` wide (PAGE-21 to PAGE-27). `lean` is the left panel's width
/// minus the right panel's.
pub fn column(view: f32, b: f32, avg: f32, wide: bool, lean: f32) -> Column {
    let measure = r(avg * b * if wide { 100.0 } else { 68.0 });
    let side = 24f32.max(56f32.min(view * 0.06));
    let width = 200f32.max(measure.min(view - 2.0 * side));
    let free = view - width;
    let limit = (free / 2.0 - free / 2.618).max(0.0);
    let shift = lean.signum() * (lean.abs() / 2.0).min(limit);
    let gutter = r(side.max((free - side).min(free / 2.0 - shift)));
    Column { gutter, width }
}

/// Room above the first block (PAGE-28) and after the last (PAGE-29).
pub const TOP_ROOM: f32 = 44.0;
pub fn bottom_room(view_h: f32) -> f32 {
    120f32.max(view_h * 0.3)
}

/// The room after the last line: the reader's (PAGE-29) or half the view in the writer (WR-86),
/// and at least `band`, the toast band's height, so the last line can sit above it (RD-104).
pub fn end_room(writer: bool, view_h: f32, band: f32) -> f32 {
    if writer { view_h * 0.5 } else { bottom_room(view_h) }.max(band)
}

/// What the rhythm needs to know of a block (PAGE-58).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rk {
    Heading(usize),
    Rule,
    FrontMatter,
    Footnotes,
    /// Code, diff, table, alert, image, gallery and quote.
    Framed,
    List,
    Other,
}

impl Rk {
    fn framed(self) -> bool {
        matches!(self, Rk::Framed | Rk::FrontMatter)
    }
}

/// The rhythm's view of a block (PAGE-58).
pub fn rk(k: &crate::blocks::Kind) -> Rk {
    use crate::blocks::Kind;
    match k {
        Kind::Heading { level, .. } => Rk::Heading(*level),
        Kind::Rule => Rk::Rule,
        Kind::FrontMatter { .. } => Rk::FrontMatter,
        Kind::Footnotes { .. } => Rk::Footnotes,
        Kind::Code { .. } | Kind::Diff { .. } | Kind::Table(_) | Kind::Alert { .. } | Kind::Image { .. } | Kind::Gallery { .. } | Kind::Quote { .. } => Rk::Framed,
        Kind::List(_) => Rk::List,
        Kind::Paragraph { .. } | Kind::Html { .. } => Rk::Other,
    }
}

/// The height the page's virtual list gives a row not yet laid out, at body size `b`: three
/// body lines. The page map (`reader::PageMap`) counts the same.
pub fn row_estimate(b: f32) -> f32 {
    r(b * 1.6) * 3.0
}

/// The gap above each top-level block of the page, at body size `b`.
pub fn gaps(b: f32, blocks: &[crate::blocks::Block]) -> Vec<f32> {
    let mut prev = None;
    blocks
        .iter()
        .map(|bl| {
            let k = rk(&bl.kind);
            let g = gap(b, prev, k);
            prev = Some(k);
            g
        })
        .collect()
}

/// The gap above `cur` after `prev` on the page (PAGE-54 to PAGE-65). The first match wins.
pub fn gap(b: f32, prev: Option<Rk>, cur: Rk) -> f32 {
    let Some(prev) = prev else { return 0.0 };
    let f = match (prev, cur) {
        (Rk::Rule, Rk::Heading(_)) => 1.5,
        (Rk::Heading(p), Rk::Heading(l)) if p < l => {
            if p <= 2 {
                1.25
            } else {
                0.7
            }
        }
        (_, Rk::Heading(l)) => [2.6, 2.2, 1.7, 1.4, 1.3, 1.3][l.clamp(1, 6) - 1],
        (Rk::Heading(p), c) => {
            if p <= 2 || c.framed() {
                0.75
            } else {
                0.5
            }
        }
        (Rk::FrontMatter, _) => 2.2,
        (Rk::Rule, _) | (_, Rk::Rule) => 1.9,
        (_, Rk::Footnotes) => 2.6,
        (p, c) if p.framed() || c.framed() => 1.25,
        _ => 0.9,
    };
    r(b * f)
}

/// The gap inside a container (PAGE-66), and inside a tight list item (PAGE-67).
pub fn inner_gap(b: f32, prev: Option<Rk>, cur: Rk, tight: bool) -> f32 {
    if prev.is_none() {
        return 0.0;
    }
    if tight && (cur == Rk::List || prev == Some(Rk::List)) {
        return r(b * 0.3);
    }
    gap(b, prev, cur).min(r(b * 0.9))
}

/// Column widths for a table (BLK-63 to BLK-65): the natural widths when they fit, else each
/// column gives up width in proportion to its slack above its minimum, else the table scrolls
/// with each column capped at `scroll_cap` and at least its minimum. Returns the widths and
/// whether the table scrolls.
pub fn table_widths(natural: &[f32], min: &[f32], avail: f32, scroll_cap: f32) -> (Vec<f32>, bool) {
    let nat: f32 = natural.iter().sum();
    if nat <= avail {
        return (natural.to_vec(), false);
    }
    let lo: f32 = min.iter().sum();
    if lo <= avail {
        let slack: f32 = natural.iter().zip(min).map(|(n, m)| n - m).sum();
        let give = avail - lo;
        let w = natural.iter().zip(min).map(|(n, m)| if slack > 0.0 { (m + (n - m) * give / slack).floor() } else { *m }).collect();
        return (w, false);
    }
    (natural.iter().zip(min).map(|(n, m)| n.min(scroll_cap).max(*m).ceil()).collect(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUATTRO: Family = Family::Named("iA Writer Quattro S");

    #[test]
    fn page_13_14_15_type_scale() {
        let h: Vec<f32> = [3.0, 2.0, 1.0, 0.5, 0.0, 0.0].iter().map(|s| step(14.0, *s)).collect();
        assert_eq!(h, vec![27.0, 22.0, 18.0, 16.0, 14.0, 14.0]);
        let lines: Vec<f32> = h.iter().zip([1.25, 1.25, 1.35, 1.35, 1.35, 1.35]).map(|(s, l)| line(*s, l)).collect();
        assert_eq!(lines, vec![34.0, 28.0, 24.0, 22.0, 19.0, 19.0]);
        assert_eq!(line(14.0, 1.6), 22.0);
        assert_eq!(r(14.0 * 0.84), 12.0);
        assert_eq!(r(2.5), 3.0);
    }

    #[test]
    fn page_21_to_27_column() {
        // 68 Quattro characters at 14 px; wide is 100.
        let c = column(1400.0, 14.0, avg_char(QUATTRO), false, 0.0);
        assert_eq!(c.width, 579.0);
        assert_eq!(column(1400.0, 14.0, avg_char(QUATTRO), true, 0.0).width, 851.0);
        // Centred with no lean.
        assert_eq!(c.gutter, r((1400.0 - 579.0) / 2.0));
        // A narrow view keeps side room and a 200 px floor.
        let n = column(500.0, 14.0, avg_char(QUATTRO), false, 0.0);
        assert_eq!(n.width, 500.0 - 2.0 * 30.0);
        assert_eq!(column(100.0, 14.0, 0.6, false, 0.0).width, 200.0);
        // A wider left panel moves the column left, by at most half the lean, never past the
        // golden-section point.
        let l = column(1000.0, 14.0, avg_char(QUATTRO), false, 40.0);
        assert_eq!(l.gutter, r((1000.0 - 579.0) / 2.0 - 20.0));
        let far = column(1000.0, 14.0, avg_char(QUATTRO), false, 2000.0);
        let free = 1000.0 - 579.0;
        assert_eq!(far.gutter, r(free / 2.618));
        let right = column(1000.0, 14.0, avg_char(QUATTRO), false, -40.0);
        assert_eq!(right.gutter, r(free / 2.0 + 20.0));
    }

    #[test]
    fn page_54_to_65_rhythm() {
        let g = |p, c| gap(14.0, p, c);
        assert_eq!(g(None, Rk::Heading(1)), 0.0);
        assert_eq!(g(Some(Rk::Rule), Rk::Heading(2)), 21.0);
        assert_eq!(g(Some(Rk::Heading(2)), Rk::Heading(3)), 18.0);
        assert_eq!(g(Some(Rk::Heading(3)), Rk::Heading(4)), 10.0);
        assert_eq!(g(Some(Rk::Heading(3)), Rk::Heading(3)), 24.0);
        assert_eq!(g(Some(Rk::Other), Rk::Heading(1)), 36.0);
        assert_eq!(g(Some(Rk::Other), Rk::Heading(2)), 31.0);
        assert_eq!(g(Some(Rk::Other), Rk::Heading(6)), 18.0);
        assert_eq!(g(Some(Rk::Heading(1)), Rk::Other), 11.0);
        assert_eq!(g(Some(Rk::Heading(3)), Rk::Other), 7.0);
        assert_eq!(g(Some(Rk::Heading(3)), Rk::Framed), 11.0);
        assert_eq!(g(Some(Rk::FrontMatter), Rk::Other), 31.0);
        assert_eq!(g(Some(Rk::Other), Rk::Rule), 27.0);
        assert_eq!(g(Some(Rk::Rule), Rk::Other), 27.0);
        assert_eq!(g(Some(Rk::Other), Rk::Footnotes), 36.0);
        assert_eq!(g(Some(Rk::Other), Rk::Framed), 18.0);
        assert_eq!(g(Some(Rk::Framed), Rk::List), 18.0);
        assert_eq!(g(Some(Rk::Other), Rk::List), 13.0);
    }

    #[test]
    fn page_66_67_inner_rhythm() {
        assert_eq!(inner_gap(14.0, Some(Rk::Other), Rk::Framed, false), 13.0);
        assert_eq!(inner_gap(14.0, Some(Rk::Other), Rk::Other, false), 13.0);
        assert_eq!(inner_gap(14.0, Some(Rk::Other), Rk::List, true), 4.0);
        assert_eq!(inner_gap(14.0, None, Rk::List, true), 0.0);
    }

    #[test]
    fn blk_63_to_65_table_widths() {
        assert_eq!(table_widths(&[100.0, 200.0], &[50.0, 80.0], 400.0, 150.0), (vec![100.0, 200.0], false));
        // 300 natural, 130 minimum, 230 available: 100 of 170 slack is kept, floored as in omarky.
        assert_eq!(table_widths(&[100.0, 200.0], &[50.0, 80.0], 230.0, 150.0), (vec![79.0, 150.0], false));
        assert_eq!(table_widths(&[100.0, 200.0], &[50.0, 160.0], 100.0, 150.0), (vec![100.0, 160.0], true));
    }
}
