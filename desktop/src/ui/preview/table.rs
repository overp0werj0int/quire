//! Tables (BLK-59 to BLK-69): columns sized from their content, a raised mono header, a
//! hairline frame and zebra rows; a table too wide for the column scrolls sideways.

use rune::{Align as RAlign, El, TextCase, TextStyle, col, el, row, text};

use super::{Cx, Rich, Tone};
use crate::blocks::{Align, Table};
use crate::page::{self, line, r};

pub(super) fn table(cx: &Cx, t: &Table, tone: Tone) -> El {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let cs = r(b * 0.94);
    let hs = r(b * 0.72);
    let cell_style = look.prose(cs, 1.45);
    let head_style = TextStyle::new(hs, 700.0).family(look.mono).lh(line(hs, 1.35)).tracking(0.8);
    let (px, py) = (r(b * 0.85), r(b * 0.55));
    let avg = page::avg_char(look.family) * cs;
    let n = t.header.len().max(t.aligns.len());
    let cell_tone = Tone { size: cs, lead: 1.45, ..tone };
    let numeric = |i: usize| t.numeric.get(i).copied().unwrap_or(false) && matches!(t.aligns.get(i), None | Some(Align::None));
    let style_of = |i: usize| if numeric(i) { cell_style.features("'tnum' 1") } else { cell_style };

    // Each column's natural width and minimum (BLK-63, BLK-64), padding included.
    let heads: Vec<String> = (0..n).map(|i| t.header.get(i).map_or(String::new(), |h| crate::blocks::plain(h).to_uppercase())).collect();
    let body: Vec<Vec<Rich>> = t.rows.iter().map(|rw| (0..n).map(|i| rw.get(i).map_or_else(Rich::default, |runs| cx.rich(runs, cell_tone))).collect()).collect();
    let mut natural = vec![0f32; n];
    let mut min = vec![0f32; n];
    for i in 0..n {
        let head = rune::text::advance(&heads[i], head_style);
        let word = heads[i].split_whitespace().map(|w| rune::text::advance(w, head_style)).fold(0.0, f32::max);
        let widest = body.iter().map(|rw| rune::text::content_widths(&rw[i].text, &rw[i].spans, style_of(i)).1).fold(head, f32::max);
        natural[i] = widest.min(34.0 * avg).ceil() + 2.0 * px;
        min[i] = (word.max(10.0 * avg).ceil() + 2.0 * px).min(natural[i]);
    }
    let (widths, scrolls) = page::table_widths(&natural, &min, look.width - 2.0, 24.0 * avg + 2.0 * px);
    let align = |i: usize| match t.aligns.get(i) {
        Some(Align::Right) => RAlign::End,
        Some(Align::Center) => RAlign::Center,
        Some(Align::Left) => RAlign::Start,
        _ if numeric(i) => RAlign::End,
        _ => RAlign::Start,
    };

    let mut head = row().key("head").fill(c.raised).border_sides([0.0, 0.0, 1.0, 0.0], c.hairline);
    for (i, w) in widths.iter().enumerate() {
        let label = t.header.get(i).map_or(String::new(), |h| crate::blocks::plain(h));
        let cell = text(label).font(head_style).color(c.dim).text_case(TextCase::Uppercase).wrap().align(align(i)).w_full();
        head = head.child(el().key(format!("c-{i}")).w(*w).px(px).py(py).child(cell));
    }
    let zebra = c.text.alpha(look.ld(0.022, 0.018));
    let mut frame = col().key("frame").border(1.0, c.hairline).radius(4.0).clip_rounded(4.0).child(head);
    for (ri, cells) in body.into_iter().enumerate() {
        let mut rw = row().key(format!("r-{ri}"));
        if ri > 0 {
            rw = rw.border_sides([1.0, 0.0, 0.0, 0.0], c.hairline_soft);
        }
        if ri % 2 == 1 {
            rw = rw.fill(zebra);
        }
        for (i, cell) in cells.into_iter().enumerate() {
            let e = cell.el(style_of(i), tone.color).align(align(i)).w_full();
            rw = rw.child(el().key(format!("c-{i}")).w(widths[i]).px(px).py(py).child(e));
        }
        frame = frame.child(rw);
    }
    if scrolls { el().w_full().scroll_x().child(frame) } else { row().w_full().child(frame) }
}
