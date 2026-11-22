//! Cards on the map (MAP-049 to MAP-052): a block drawn by the reader's builders in a box whose
//! size is known before Rune lays it out, so the map's layout can place it. A card is as wide
//! as its content up to `cardMaxWidth` and as tall up to a cap; what runs past the right or the
//! bottom edge fades out, and there is no scrolling inside a card (MAP-085).

use rune::{El, TextStyle, el, row, text};

use crate::app::App;
use crate::blocks::{Block, DiffKind, DiffLine, Kind, Table};
use crate::page::{self, line, r};
use crate::roles::mix;
use crate::ui::chip::Chip;
use crate::ui::icons;
use crate::ui::preview::code as pcode;
use crate::ui::preview::table::Grid;
use crate::ui::preview::{Cx, Tone};

/// Code and diff cards show this many lines (MAP-049).
const CODE_LINES: usize = 12;
/// Quotes and alerts show this many lines of text, raw HTML 6 (MAP-052).
const TEXT_LINES: f32 = 8.0;
const HTML_LINES: f32 = 6.0;
/// Tables show this many body rows (MAP-051).
const TABLE_ROWS: usize = 8;
/// The fade over a card's clipped bottom.
const BOTTOM_FADE: f32 = 28.0;

/// Block `bl` of node `i` as a card, and its size, or None for a block that is not one yet.
/// `tail` names the node in the card's keys.
pub(super) fn card(cx: &Cx, bl: &Block, i: usize, tail: &str) -> Option<(El, [f32; 2])> {
    let at = bl.lines.start;
    let tone = Tone::top(cx.look.c.text, cx.look.b, 1.6);
    let max = cx.look.width;
    let (e, size) = match &bl.kind {
        Kind::Code { lang, text } => code(cx, lang, text, at, i, tail),
        Kind::Diff { lang, text, lines, added, removed } => diff(cx, lang, text, lines, (*added, *removed), at, i, tail),
        Kind::Table(t) => table(cx, t, tone, tail),
        Kind::Quote { blocks } => capped(cx.quote(blocks, tone), cx.quote_size(blocks, tone, max), TEXT_LINES * line(page::quote_size(cx.look.b), 1.6)),
        Kind::Alert { kind, blocks } => {
            let [w, h] = cx.alert_size(*kind, blocks, tone, max);
            let text_h = cx.alert_size(*kind, &[], tone, max)[1];
            capped(cx.alert(*kind, blocks, tone), [w, h], text_h + TEXT_LINES * line(cx.look.b, 1.6))
        }
        Kind::Html { text } => capped(cx.html(text), cx.html_size(text, max), HTML_LINES * line(r(cx.look.b * 0.8), 1.5)),
        _ => return None,
    };
    Some((e.key(format!("map-card:{tail}")), size))
}

/// A card drawn by a reader builder: `inner` at its natural size `[w, h]`, shown at most `cap`
/// tall with its bottom fading out when it is taller.
fn capped(inner: El, [w, h]: [f32; 2], cap: f32) -> (El, [f32; 2]) {
    let shown = h.min(cap).ceil();
    let fade = if h > cap { BOTTOM_FADE } else { 0.0 };
    (el().size(w, shown).clip_fade_linear(0.0, 0.0, 0.0, fade).child(inner.pin(Some(0.0), Some(0.0), None, None).w(w)), [w, shown])
}

/// "+N more lines" under a clipped card, in the badge's type.
fn more(cx: &Cx, n: usize, what: &str, tail: &str) -> El {
    let size = pcode::badge_size(cx.look.b);
    let what = if n == 1 { what.to_string() } else { format!("{what}s") };
    text(format!("+{n} more {what}")).key(format!("map-more:{tail}")).font(more_style(cx, size)).color(cx.look.c.readable_dim())
}

fn more_style(cx: &Cx, size: f32) -> TextStyle {
    cx.look.mono_at(size, 1.6)
}

/// The Edit chip beside Copy: the Edit Sheet on the card's block (MAP-049).
fn edit_chip(cx: &Cx, i: usize, tail: &str) -> El {
    let c = &cx.look.c;
    let size = pcode::chip_size(cx.look.b);
    let key = format!("map-edit:{tail}");
    Chip { key: &key, glyph: &icons::PENCIL, size, icon: r(size * 1.1), label: "Edit", mono: cx.look.mono, tint: c.readable_dim(), active: false, fill: Some(c.panel) }
        .el(c)
        .label("Edit the Markdown")
        .on_press_tx(move |s: &mut App, tx| s.map_click(i, true, tx))
}

/// A chip's width with `label`, as `Chip::el` lays it out.
fn chip_w(cx: &Cx, label: &str) -> f32 {
    let s = pcode::chip_size(cx.look.b);
    r(s * 1.3) + r(s * 1.1) + r(s * 0.45) + rune::text::advance(label, cx.look.mono_at(s, 2.0)).ceil()
}

/// The gap between the Copy and Edit chips.
const CHIP_GAP: f32 = 4.0;

/// The Copy and Edit chips, shown under the pointer and while Copy says Copied.
fn chips(cx: &Cx, code: &str, at: usize, i: usize, tail: &str) -> El {
    let copied = pcode::copied(cx, at);
    row()
        .gap(CHIP_GAP)
        .child(pcode::copy_chip(cx, &format!("map-copy:{tail}"), code.to_string(), at))
        .child(edit_chip(cx, i, tail))
        .group_opacity(if copied { 1.0 } else { 0.0 }, 1.0)
}

/// How wide the chips are, with the Copy chip saying Copied, so the card never grows for it.
fn chips_w(cx: &Cx) -> f32 {
    chip_w(cx, "Copied") + CHIP_GAP + chip_w(cx, "Edit")
}

/// MAP-049: the reader's code block, capped at 12 lines, with no scrolling.
fn code(cx: &Cx, lang: &str, src: &str, at: usize, i: usize, tail: &str) -> (El, [f32; 2]) {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let size = page::code_size(b);
    let style = look.mono_at(size, 1.5);
    let lh = style.line_height;
    let all = pcode::expand_tabs(src);
    let n = all.split('\n').count();
    let shown = n.min(CODE_LINES);
    let body = all.split('\n').take(shown).collect::<Vec<_>>().join("\n");
    let spans = pcode::highlight(cx, lang, &body);
    let body = if body.is_empty() { " ".to_string() } else { body };
    let pad_x = r(b * 1.1);
    let (top, pad_top) = pcode::top_clearance(b);
    let pad_b = r(b * 0.85);
    let gap = r(size * 1.4);
    let nums_w = (n > 8).then(|| rune::text::advance(&shown.to_string(), style).ceil());
    let natural = (nums_w.map_or(0.0, |w| w + gap) + rune::text::measure(&body, &spans, style, None).0 + 2.0 * pad_x).ceil();
    let pad_r = r(pad_x * 0.4) + r(pad_x * 0.5);
    let w = natural.max(pad_x + chips_w(cx) + pad_r - 6.0).min(look.width);
    let rest = n - shown;
    let more_h = if rest > 0 { more_style(cx, pcode::badge_size(b)).line_height } else { 0.0 };
    let h = pad_top + shown as f32 * lh + pad_b + more_h;
    let mut lines = row().items_start().pin(Some(0.0), Some(0.0), None, None);
    if let Some(nw) = nums_w {
        let nums = (1..=shown).map(|k| k.to_string()).collect::<Vec<_>>().join("\n");
        lines = lines.child(text(nums).font(style.features("'tnum' 1")).color(c.ghost).align(rune::Align::End).w(nw).mr(gap));
    }
    lines = lines.child(text(body).spans(spans).key("code").font(style).color(c.text));
    let fades = (if natural > look.width { r(b * 2.4) } else { 0.0 }, if rest > 0 { BOTTOM_FADE } else { 0.0 });
    let clip = el().pin(Some(pad_x), Some(pad_top), None, None).size(w - 2.0 * pad_x, shown as f32 * lh).clip_fade_linear(0.0, 0.0, fades.0, fades.1).child(lines);
    let copied = pcode::copied(cx, at);
    let mut out = el()
        .size(w, h)
        .radius(4.0)
        .fill(c.panel)
        .border(1.0, c.on_page(c.hairline_soft))
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(clip);
    if !lang.is_empty() {
        out = out.child(pcode::badge(cx, lang).group_opacity(if copied { 0.0 } else { 1.0 }, 0.0).pin(None, Some(top), Some(pad_r), None));
    }
    out = out.child(chips(cx, src, at, i, tail).pin(None, Some(top), Some(pad_r - 6.0), None));
    if rest > 0 {
        out = out.child(more(cx, rest, "line", tail).pin(Some(pad_x), Some(pad_top + shown as f32 * lh + r(pad_b / 2.0)), None, None));
    }
    (out, [w, h])
}

/// MAP-050: the reader's diff block in the capped code card, with Copy and Edit in its header.
#[allow(clippy::too_many_arguments)]
fn diff(cx: &Cx, lang: &str, src: &str, lines: &[DiffLine], (added, removed): (usize, usize), at: usize, i: usize, tail: &str) -> (El, [f32; 2]) {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let size = page::code_size(b);
    let style = look.mono_at(size, 1.5);
    let lh = style.line_height;
    let pad_x = r(b * 1.1);
    let bs = pcode::badge_size(b);
    let count = pcode::count_style(cx);
    let head_h = r(b * 2.1).max(r(pcode::chip_size(b) * 2.0) + 4.0);
    let gap = r(b * 0.6);
    let py = r(b * 0.7);
    let sign_w = r(size * 1.6);
    let shown = lines.len().min(CODE_LINES);
    let rest = lines.len() - shown;
    let texts: Vec<String> = lines[..shown].iter().map(|l| pcode::expand_tabs(&l.text)).collect();
    let widest = texts.iter().map(|t| rune::text::advance(t, style)).fold(0.0, f32::max);
    let natural = (sign_w + widest + 2.0 * pad_x).ceil();
    let (plus, minus) = (format!("+{added}"), format!("\u{2212}{removed}"));
    let counts = rune::text::advance(&plus, count) + gap + rune::text::advance(&minus, count);
    let w = natural.max((2.0 * pad_x + counts + gap + chips_w(cx)).ceil()).min(look.width);
    let more_h = if rest > 0 { more_style(cx, bs).line_height } else { 0.0 };
    let body_h = 2.0 * py + shown as f32 * lh;
    let h = head_h + body_h + more_h;
    let copied = pcode::copied(cx, at);
    let head = row()
        .key("head")
        .size(w, head_h)
        .px(pad_x)
        .gap(gap)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], c.hairline_soft)
        .child(text(plus).key("added").font(count).color(c.readable_green()))
        .child(text(minus).key("removed").font(count).color(c.readable_red()))
        .child(el().key("space").grow(1.0))
        .child(if lang.is_empty() { el() } else { pcode::badge(cx, lang).group_opacity(if copied { 0.0 } else { 1.0 }, 0.0) });
    let mut rows = rune::col().pin(Some(0.0), Some(py), None, None).min_w(w);
    for (k, (l, t)) in lines.iter().zip(texts).enumerate() {
        let (band, ink) = match l.kind {
            DiffKind::Add => (Some(c.green.alpha(0.14)), mix(c.text, c.green, 0.3)),
            DiffKind::Del => (Some(c.red.alpha(0.14)), mix(c.text, c.red, 0.3)),
            DiffKind::Hunk => (Some(c.blue.alpha(0.07)), c.blue),
            DiffKind::Meta => (None, c.faint),
            DiffKind::Ctx => (None, c.text),
        };
        let mut rw = row()
            .key(format!("l-{k}"))
            .px(pad_x)
            .child(text(l.sign).key("sign").font(style).color(ink).w(sign_w))
            .child(text(if t.is_empty() { " ".to_string() } else { t }).key("text").font(style).color(ink));
        if let Some(f) = band {
            rw = rw.fill(f);
        }
        rows = rows.child(rw);
    }
    let fades = (if natural > look.width { r(b * 2.4) } else { 0.0 }, if rest > 0 { BOTTOM_FADE } else { 0.0 });
    let body = el().pin(Some(0.0), Some(head_h), None, None).size(w, body_h).clip_fade_linear(0.0, 0.0, fades.0, fades.1).child(rows);
    let pad_r = r(pad_x * 0.4) + r(pad_x * 0.5);
    let mut out = el()
        .size(w, h)
        .radius(4.0)
        .clip_rounded(4.0)
        .fill(c.panel)
        .border(1.0, c.on_page(c.hairline_soft))
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(head.pin(Some(0.0), Some(0.0), None, None))
        .child(body)
        .child(chips(cx, src, at, i, tail).pin(None, Some((head_h - r(pcode::chip_size(b) * 2.0)) / 2.0), Some(pad_r - 6.0), None));
    if rest > 0 {
        out = out.child(more(cx, rest, "line", tail).pin(Some(pad_x), Some(head_h + body_h - r(py / 2.0)), None, None));
    }
    (out, [w, h])
}

/// MAP-051: the reader's table at the card's widest, capped at 8 body rows.
fn table(cx: &Cx, t: &Table, tone: Tone, tail: &str) -> (El, [f32; 2]) {
    let look = cx.look;
    let b = look.b;
    let g = Grid::new(cx, t, tone, look.width - 2.0);
    let heights = g.heights();
    let shown = g.rows().min(TABLE_ROWS);
    let rest = g.rows() - shown;
    // The frame's 1 point of padding each side, round the grid.
    let natural = g.widths.iter().sum::<f32>() + 2.0;
    let w = natural.min(look.width);
    let grid_h = heights[..=shown].iter().sum::<f32>() + 2.0;
    let more_h = if rest > 0 { more_style(cx, pcode::badge_size(b)).line_height } else { 0.0 };
    let fades = (if natural > w { r(b * 2.8) } else { 0.0 }, if rest > 0 { BOTTOM_FADE } else { 0.0 });
    let frame = g.frame(cx, shown).pin(Some(0.0), Some(0.0), None, None);
    let mut out = el().size(w, grid_h + more_h).child(el().size(w, grid_h).clip_fade_linear(0.0, 0.0, fades.0, fades.1).child(frame));
    if rest > 0 {
        out = out.child(more(cx, rest, "row", tail).pin(Some(0.0), Some(grid_h), None, None));
    }
    (out, [w, grid_h + more_h])
}
