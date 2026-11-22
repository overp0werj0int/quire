//! Tables (BLK-59 to BLK-69): columns sized from their content, a raised mono header, a
//! hairline frame and zebra rows; a table too wide for the column scrolls sideways.

use rune::text::RichLayoutOptions;
use rune::{Align as RAlign, El, TextCase, TextStyle, col, el, row, text};

use super::{Cx, Rich, Tone};
use crate::blocks::{Align, Table};
use crate::page::{self, line, r};

/// A table set for a column: its cells, styles and column widths.
pub(in crate::ui) struct Grid<'t> {
    t: &'t Table,
    tone: Tone,
    cell_style: TextStyle,
    head_style: TextStyle,
    px: f32,
    py: f32,
    /// The header's labels, upper case, as they measure.
    heads: Vec<String>,
    body: Vec<Vec<Rich>>,
    pub(in crate::ui) widths: Vec<f32>,
    /// The columns are wider than the room at their narrowest.
    pub(in crate::ui) scrolls: bool,
}

impl<'t> Grid<'t> {
    /// Table `t` in a column `avail` points wide.
    pub(in crate::ui) fn new(cx: &Cx, t: &'t Table, tone: Tone, avail: f32) -> Grid<'t> {
        let look = cx.look;
        let b = look.b;
        let cs = r(b * 0.94);
        let hs = r(b * 0.72);
        let cell_style = look.prose(cs, 1.45);
        let head_style = TextStyle::new(hs, 700.0).family(look.mono).lh(line(hs, 1.35)).tracking(0.8);
        let (px, py) = (r(b * 0.85), r(b * 0.55));
        let avg = page::avg_char(look.family) * cs;
        let n = t.header.len().max(t.aligns.len());
        let cell_tone = Tone { size: cs, lead: 1.45, ..tone };
        let heads: Vec<String> = (0..n).map(|i| t.header.get(i).map_or(String::new(), |h| crate::blocks::plain(h).to_uppercase())).collect();
        let body: Vec<Vec<Rich>> = t
            .rows
            .iter()
            .enumerate()
            .map(|(ri, rw)| (0..n).map(|i| rw.get(i).map_or_else(Rich::default, |runs| cx.rich_at(runs, cell_tone, t.row_pids.get(ri).and_then(|p| p.get(i)).copied()))).collect())
            .collect();
        let mut g = Grid { t, tone, cell_style, head_style, px, py, heads, body, widths: Vec::new(), scrolls: false };
        // Each column's natural width and minimum (BLK-63, BLK-64), padding included.
        let mut natural = vec![0f32; n];
        let mut min = vec![0f32; n];
        for i in 0..n {
            let head = rune::text::advance(&g.heads[i], head_style);
            let word = g.heads[i].split_whitespace().map(|w| rune::text::advance(w, head_style)).fold(0.0, f32::max);
            // omarky pads each cell's measure by 2 % and a point, and each column by 2 points.
            let widest = g.body.iter().map(|rw| rune::text::content_widths(&rw[i].text, &rw[i].spans, g.style_of(i)).1 * 1.02 + 1.0).fold(head, f32::max);
            natural[i] = (widest.min(r(34.0 * avg)) + 2.0 * px + 2.0).ceil();
            min[i] = natural[i].min((word + 2.0 * px + 2.0).max(10.0 * avg + 2.0 * px)).ceil();
        }
        (g.widths, g.scrolls) = page::table_widths(&natural, &min, avail, 24.0 * avg + 2.0 * px);
        g
    }

    fn numeric(&self, i: usize) -> bool {
        self.t.numeric.get(i).copied().unwrap_or(false) && matches!(self.t.aligns.get(i), None | Some(Align::None))
    }

    fn style_of(&self, i: usize) -> TextStyle {
        if self.numeric(i) { self.cell_style.features("'tnum' 1") } else { self.cell_style }
    }

    fn align(&self, i: usize) -> RAlign {
        match self.t.aligns.get(i) {
            Some(Align::Right) => RAlign::End,
            Some(Align::Center) => RAlign::Center,
            Some(Align::Left) => RAlign::Start,
            _ if self.numeric(i) => RAlign::End,
            _ => RAlign::Start,
        }
    }

    /// How many body rows the table has.
    pub(in crate::ui) fn rows(&self) -> usize {
        self.body.len()
    }

    /// The header's height, then each body row's, as the frame lays them out.
    pub(in crate::ui) fn heights(&self) -> Vec<f32> {
        let anywhere = || RichLayoutOptions::new().wrap_anywhere();
        let cell_h = |text: &str, spans: &[rune::Span], style: TextStyle, w: f32| {
            let t = if text.is_empty() { " " } else { text };
            rune::text::measure_with(t, spans, style, Some((w - 2.0 * self.px).max(0.0)), anywhere()).1
        };
        let head = self.widths.iter().enumerate().map(|(i, &w)| cell_h(&self.heads[i], &[], self.head_style, w)).fold(0.0, f32::max);
        let rows = self.body.iter().map(|rw| rw.iter().enumerate().map(|(i, c)| cell_h(&c.text, &c.spans, self.style_of(i), self.widths[i])).fold(0.0, f32::max));
        std::iter::once(head).chain(rows).map(|h| h + 2.0 * self.py).collect()
    }

    /// The framed table with its first `rows` body rows.
    pub(in crate::ui) fn frame(self, cx: &Cx, rows: usize) -> El {
        let look = cx.look;
        let c = &look.c;
        let mut head = row().key("head").fill(c.raised).border_sides([0.0, 0.0, 1.0, 0.0], c.hairline);
        for (i, w) in self.widths.iter().enumerate() {
            let label = self.t.header.get(i).map_or(String::new(), |h| crate::blocks::plain(h));
            let marks = cx.marks(self.t.header_pids.get(i).copied(), &label, 0).0;
            let cell = cx.selectable_text(text(label).font(self.head_style).color(c.dim).text_case(TextCase::Uppercase).wrap_anywhere().align(self.align(i)).w_full().text_highlights(marks));
            head = head.child(el().key(format!("c-{i}")).w(*w).px(self.px).py(self.py).child(cell));
        }
        let zebra = c.text.alpha(look.ld(0.022, 0.018));
        // The grid sits inside the 1 px frame, as in omarky, not under it (BLK-62).
        let mut frame = col().key("frame").border(1.0, c.hairline).padding(1.0).radius(4.0).clip_rounded(4.0).child(head);
        let styles: Vec<(TextStyle, RAlign)> = (0..self.widths.len()).map(|i| (self.style_of(i), self.align(i))).collect();
        for (ri, cells) in self.body.into_iter().take(rows).enumerate() {
            let mut rw = row().key(format!("r-{ri}"));
            if ri > 0 {
                rw = rw.border_sides([1.0, 0.0, 0.0, 0.0], c.hairline_soft);
            }
            if ri % 2 == 1 {
                rw = rw.fill(zebra);
            }
            for (i, cell) in cells.into_iter().enumerate() {
                // A word wider than its column (a URL, a long name) breaks inside, as Qt's wrap does
                // in omarky, instead of running into the next column.
                let e = cell.el(styles[i].0, self.tone.color, cx.sel()).wrap_anywhere().align(styles[i].1).w_full();
                rw = rw.child(el().key(format!("c-{i}")).w(self.widths[i]).px(self.px).py(self.py).child(e));
            }
            frame = frame.child(rw);
        }
        frame
    }
}

pub(super) fn table(cx: &Cx, t: &Table, tone: Tone) -> El {
    let b = cx.look.b;
    let g = Grid::new(cx, t, tone, cx.look.width - 2.0);
    let scrolls = g.scrolls;
    let frame = g.frame(cx, usize::MAX);
    // While it scrolls, each side fades into the page over its own width, only while there is
    // more that way (BLK-69).
    let fade = |w: f32| rune::ScrollFade::strength(w).full_at(1.0).linear();
    if scrolls { el().w_full().scroll_x().scroll_fade(fade(r(b * 2.2)).with_end(fade(r(b * 2.8)))).child(frame) } else { row().w_full().child(frame) }
}
