//! Cards on the map (MAP-049 to MAP-052): a block drawn by the reader's builders in a box whose
//! size is known before Rune lays it out, so the map's layout can place it. A card is as wide
//! as its content up to `cardMaxWidth` and as tall up to a cap; what runs past the right or the
//! bottom edge fades out, and there is no scrolling inside a card (MAP-085).

use rune::{El, TextStyle, el, row, text};

use crate::app::App;
use crate::blocks::{Block, DiffKind, DiffLine, GalleryItem, Kind, Table};
use crate::md::Img;
use crate::page::{self, line, r};
use crate::roles::{Roles, mix};
use crate::ui::chip::Chip;
use crate::ui::icons;
use crate::ui::preview::code as pcode;
use crate::ui::preview::table::Grid;
use crate::ui::preview::{Cx, LOADING, Pic, Tone};

/// Code and diff cards show this many lines (MAP-049).
const CODE_LINES: usize = 12;
/// Quotes and alerts show this many lines of text, raw HTML 6 (MAP-052).
const TEXT_LINES: f32 = 8.0;
const HTML_LINES: f32 = 6.0;
/// Tables show this many body rows (MAP-051).
const TABLE_ROWS: usize = 8;
/// The fade over a card's clipped bottom.
const BOTTOM_FADE: f32 = 28.0;

/// A card's stand-in below `map_app::LITE_ZOOM` (MAP-045): its box, `w` × `h`, with a crisp
/// outline and a centred kind label so zoomed-out cards remain scannable.
pub(super) fn lite(cx: &Cx, bl: &Block, w: f32, h: f32, tail: &str) -> El {
    let c = &cx.look.c;
    let kind_label = match &bl.kind {
        Kind::Code { .. } => "CODE",
        Kind::Diff { .. } => "DIFF",
        Kind::Table(_) => "TABLE",
        Kind::Quote { .. } => "QUOTE",
        Kind::Alert { .. } => "ALERT",
        Kind::Html { .. } => "HTML",
        Kind::Image { .. } => "IMAGE",
        Kind::Gallery { .. } => "GALLERY",
        _ => "CARD",
    };
    let max_h_sz = (h - 4.0).max(11.0);
    let base = (h * 0.48).clamp(14.0, 32.0).min(max_h_sz);
    let mut sz = base;
    let mut st = cx.look.mono_at(sz, 1.2).tracking(1.0);
    st.weight = 700.0;
    let avail = (w - 8.0).max(8.0);
    let adv = rune::text::advance(kind_label, st);
    if adv > avail && adv > 0.0 {
        sz = (sz * (avail / adv)).clamp(10.0, base);
        st = cx.look.mono_at(sz, 1.2).tracking(0.5);
        st.weight = 700.0;
    }
    let edge = Roles::readable_on(c.on_page(c.hairline), c.bg, 3.0);
    let ink = Roles::readable_on(c.dim, c.on_page(c.panel), 4.8);
    el().key(format!("map-lite:{tail}"))
        .size(w, h)
        .radius(4.0)
        .fill(c.panel)
        .border(1.5, edge)
        .center()
        .clip_rounded(4.0)
        .child(text(kind_label).font(st).color(ink))
}

/// Block `bl` of node `i` as a card, and its size, or None for a block that is not one yet.
/// `tail` names the node in the card's keys. When `chip_compact` is `Some(side)`, hover chips
/// render as compact square icon-only buttons of that world-point side (clamped inside the
/// card/header) so they keep a readable screen target at Fit zoom without bloating at 0.8×.
pub(super) fn card(cx: &Cx, bl: &Block, i: usize, tail: &str, chip_compact: Option<f32>) -> Option<(El, [f32; 2])> {
    let at = bl.lines.start;
    let tone = Tone::top(cx.look.c.text, cx.look.b, 1.6);
    let max = cx.look.width;
    let (e, size) = match &bl.kind {
        Kind::Code { lang, text } => code(cx, lang, text, bl.pid, at, i, tail, chip_compact),
        Kind::Diff { lang, text, lines, added, removed } => diff(cx, lang, text, lines, (*added, *removed), bl.pid, at, i, tail, chip_compact),
        Kind::Table(t) => table(cx, t, tone, i, tail, chip_compact),
        Kind::Quote { blocks } => {
            let (_, inset, inner) = cx.quote_parts(tone);
            let [w, h] = cx.quote_size(blocks, tone, max);
            let cap = TEXT_LINES * line(page::quote_size(cx.look.b), 1.6);
            let extra = if h > cap { cx.hidden_lines(blocks, inner, (w - inset).max(1.0), cap).max(1) } else { 0 };
            capped(cx, cx.quote(blocks, tone), [w, h], cap, extra, i, tail, inset, chip_compact)
        }
        Kind::Alert { kind, blocks } => alert(cx, *kind, blocks, tone, i, tail, chip_compact),
        Kind::Html { text } => {
            let [w, h] = cx.html_size(text, max);
            let cap = HTML_LINES * line(r(cx.look.b * 0.8), 1.5);
            let inset = 1.0 + r(cx.look.b * 0.9);
            let extra = if h > cap { cx.html_hidden_lines(text, w, cap) } else { 0 };
            capped(cx, cx.html(text), [w, h], cap, extra, i, tail, inset, chip_compact)
        }
        Kind::Image { img, .. } => image(cx, img, i, tail, chip_compact),
        Kind::Gallery { items } => gallery(cx, items, i, tail),
        _ => return None,
    };
    Some((e.key(format!("map-card:{tail}")), size))
}

/// A card drawn by a reader builder (Quote or HTML): `inner` at its natural size `[w, h]`, shown
/// at most `cap` tall with its bottom fading out and a `"+N more lines"` footer aligned to its
/// text inset `inset_x` with comfortable clearance above the branch underline (MAP-052).
#[allow(clippy::too_many_arguments)]
fn capped(cx: &Cx, inner: El, [w, h]: [f32; 2], cap: f32, extra: usize, i: usize, tail: &str, inset_x: f32, chip_compact: Option<f32>) -> (El, [f32; 2]) {
    let shown = h.min(cap).ceil();
    if h <= cap || extra == 0 {
        let compact = chip_compact.map(|side| side.min((w.min(shown) - 12.0).max(16.0)));
        let out = el()
            .size(w, shown)
            .hover_group()
            .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
            .child(inner.pin(Some(0.0), Some(0.0), None, None).w(w))
            .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
        return (out, [w, shown]);
    }
    let avail = (w - inset_x).max(0.0);
    let more_el = more_fit(cx, extra, "line", tail, avail, cx.look.c.dim);
    let more_lh = more_style(cx, pcode::badge_size(cx.look.b)).line_height;
    let foot_top = shown + 4.0;
    let total_h = (foot_top + more_lh + 6.0).ceil();
    let compact = chip_compact.map(|side| side.min((w.min(total_h) - 12.0).max(16.0)));
    let clipped = el()
        .pin(Some(0.0), Some(0.0), None, None)
        .size(w, shown)
        .clip_fade_linear(0.0, 0.0, 0.0, BOTTOM_FADE)
        .child(inner.pin(Some(0.0), Some(0.0), None, None).w(w));
    let out = el()
        .size(w, total_h)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(clipped)
        .child(more_el.pin(Some(inset_x), Some(foot_top), None, None))
        .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
    (out, [w, total_h])
}

/// MAP-052: an alert card with its rounded tinted fill and border enclosing both the clipped body
/// and its `"+N more lines"` footer when taller than `TEXT_LINES`.
fn alert(cx: &Cx, kind: crate::blocks::Alert, blocks: &[Block], tone: Tone, i: usize, tail: &str, chip_compact: Option<f32>) -> (El, [f32; 2]) {
    let b = cx.look.b;
    let max = cx.look.width;
    let [w, full_h] = cx.alert_size(kind, blocks, tone, max);
    let (title, pad, gap, title_h, fill, border, surf, inner_tone) = cx.alert_chrome(kind, tone);
    let body_cap = TEXT_LINES * line(b, 1.6);
    let body_top = pad + title_h + gap;
    let full_body_h = full_h - (2.0 * pad + title_h + gap);
    if full_body_h <= body_cap {
        let h = full_h.ceil();
        let compact = chip_compact.map(|side| side.min((w.min(h) - 12.0).max(16.0)));
        let out = el()
            .size(w, h)
            .hover_group()
            .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
            .child(cx.alert(kind, blocks, tone).pin(Some(0.0), Some(0.0), None, None).w(w))
            .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
        return (out, [w, h]);
    }
    let shown_body_h = body_cap.ceil();
    let inner_w = (w - 2.0 * pad).max(1.0);
    let extra = cx.hidden_lines(blocks, inner_tone, inner_w, body_cap).max(1);
    let ink = Roles::readable_on(cx.look.c.dim, surf, 4.8);
    let more_el = more_fit(cx, extra, "line", tail, inner_w, ink);
    let more_lh = more_style(cx, pcode::badge_size(b)).line_height;
    let foot_gap = r(b * 0.35);
    let foot_top = body_top + shown_body_h + foot_gap;
    let total_h = (foot_top + more_lh + (pad * 0.75).round()).ceil();
    let compact = chip_compact.map(|side| side.min((w.min(total_h) - 12.0).max(16.0)));
    let body = el()
        .pin(Some(pad), Some(body_top), None, None)
        .size(inner_w, shown_body_h)
        .clip_fade_linear(0.0, 0.0, 0.0, BOTTOM_FADE)
        .child(cx.stack(blocks, inner_tone, false).key("body").pin(Some(0.0), Some(0.0), None, None).w(inner_w));
    let out = el()
        .size(w, total_h)
        .radius(4.0)
        .clip_rounded(4.0)
        .fill(fill)
        .border(1.0, border)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(title.pin(Some(pad), Some(pad), None, None).w(inner_w))
        .child(body)
        .child(more_el.pin(Some(pad), Some(foot_top), None, None))
        .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
    (out, [w, total_h])
}

/// "+N more lines" under a clipped card, in the badge's type.
fn more(cx: &Cx, n: usize, what: &str, tail: &str) -> El {
    let size = pcode::badge_size(cx.look.b);
    let what = if n == 1 { what.to_string() } else { format!("{what}s") };
    text(format!("+{n} more {what}")).key(format!("map-more:{tail}")).font(more_style(cx, size)).color(cx.look.c.dim)
}

/// Like `more`, shortening to `"+N more"` or `"+N"` when the card is narrower than the full text.
fn more_fit(cx: &Cx, n: usize, what: &str, tail: &str, avail_w: f32, ink: rune::Color) -> El {
    let style = more_style(cx, pcode::badge_size(cx.look.b));
    let plural = if n == 1 { what.to_string() } else { format!("{what}s") };
    let full = format!("+{n} more {plural}");
    let label = if rune::text::advance(&full, style) <= avail_w {
        full
    } else {
        let mid = format!("+{n} more");
        if rune::text::advance(&mid, style) <= avail_w { mid } else { format!("+{n}") }
    };
    text(label).key(format!("map-more:{tail}")).font(style).color(ink)
}

fn more_style(cx: &Cx, size: f32) -> TextStyle {
    cx.look.mono_at(size, 1.6)
}

/// Compact square icon-only card chip used below `COMPACT_CHIP_ZOOM` (`0.65×`).
fn icon_chip(cx: &Cx, key: &str, glyph: &'static rune::Icon, side: f32, tint: rune::Color, a11y: &'static str, tip: &'static str) -> El {
    let c = &cx.look.c;
    let icon_sz = (side * 0.52).round().max(11.0);
    row()
        .key(key)
        .size(side, side)
        .center()
        .radius(4.0)
        .fill(c.panel)
        .border(1.0, c.on_page(c.hairline))
        .hover(c.hover)
        .hover_group()
        .hover_motion(crate::ui::chip::fade(), crate::ui::chip::fade())
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(a11y)
        .help(tip)
        .child(rune::icon(glyph, icon_sz).color(tint).group_color(c.text))
}

/// The Edit chip beside Copy: the Edit Sheet on the card's block (MAP-049).
fn edit_chip(cx: &Cx, i: usize, tail: &str, compact: Option<f32>) -> El {
    let c = &cx.look.c;
    let key = format!("map-edit:{tail}");
    if let Some(side) = compact {
        return icon_chip(cx, &key, &icons::PENCIL, side, c.readable_dim(), "Edit the Markdown", "Edit")
            .on_press_tx(move |s: &mut App, tx| s.map_click(i, true, tx));
    }
    let size = pcode::chip_size(cx.look.b);
    Chip { key: &key, glyph: &icons::PENCIL, size, icon: r(size * 1.1), label: "Edit", mono: cx.look.mono, tint: c.readable_dim(), active: false, fill: Some(c.panel) }
        .el(c)
        .label("Edit the Markdown")
        .on_press_tx(move |s: &mut App, tx| s.map_click(i, true, tx))
}

/// The Copy chip: normal labelled pill above `0.65×`, compact icon button below `0.65×`.
fn copy_chip(cx: &Cx, key: &str, code: String, at: usize, compact: Option<f32>) -> El {
    let c = &cx.look.c;
    let (glyph, label, tint) = if pcode::copied(cx, at) { (&icons::CHECK, "Copied", c.readable_green()) } else { (&icons::COPY, "Copy", c.readable_dim()) };
    if let Some(side) = compact {
        return icon_chip(cx, key, glyph, side, tint, "Copy the code", label)
            .on_press_tx(move |s: &mut App, tx| s.copy_code(&code, at, tx));
    }
    let size = pcode::chip_size(cx.look.b);
    Chip { key, glyph, size, icon: r(size * 1.1), label, mono: cx.look.mono, tint, active: false, fill: Some(c.panel) }
        .el(c)
        .label("Copy the code")
        .on_press_tx(move |s: &mut App, tx| s.copy_code(&code, at, tx))
}

/// A chip's width with `label`, as `Chip::el` lays it out.
fn chip_w(cx: &Cx, label: &str) -> f32 {
    let s = pcode::chip_size(cx.look.b);
    r(s * 1.3) + r(s * 1.1) + r(s * 0.45) + rune::text::advance(label, cx.look.mono_at(s, 2.0)).ceil()
}

/// The gap between the Copy and Edit chips.
const CHIP_GAP: f32 = 4.0;

/// The Copy and Edit chips, shown under the pointer and while Copy says Copied.
fn chips(cx: &Cx, code: &str, at: usize, i: usize, tail: &str, compact: Option<f32>) -> El {
    let copied = pcode::copied(cx, at);
    let gap = if compact.is_some() { 6.0 } else { CHIP_GAP };
    row()
        .gap(gap)
        .child(copy_chip(cx, &format!("map-copy:{tail}"), code.to_string(), at, compact))
        .child(edit_chip(cx, i, tail, compact))
        .group_opacity(if copied { 1.0 } else { 0.0 }, 1.0)
}

/// How wide the chips are, with the Copy chip saying Copied, so the card never grows for it.
fn chips_w(cx: &Cx) -> f32 {
    chip_w(cx, "Copied") + CHIP_GAP + chip_w(cx, "Edit")
}

/// Clamps compact icon-chip side so two side-by-side chips fit cleanly inside `[w, max_h]`.
fn clamp_compact_pair(side: f32, w: f32, max_h: f32) -> f32 {
    let max_side_w = ((w - 18.0) / 2.0).max(16.0);
    side.min(max_side_w).min(max_h.max(16.0))
}

/// MAP-049: the reader's code block, capped at 12 lines, with no scrolling.
#[allow(clippy::too_many_arguments)]
fn code(cx: &Cx, lang: &str, src: &str, pid: Option<usize>, at: usize, i: usize, tail: &str, chip_compact: Option<f32>) -> (El, [f32; 2]) {
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
        let num_ink = Roles::readable_on(c.dim, c.on_page(c.panel), 4.8);
        lines = lines.child(text(nums).font(style.features("'tnum' 1")).color(num_ink).align(rune::Align::End).w(nw).mr(gap));
    }
    let marks = cx.marks(pid, &body, 0).0;
    lines = lines.child(text(body).spans(spans).key("code").font(style).color(c.text).text_highlights(marks));
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
    let compact = chip_compact.map(|side| clamp_compact_pair(side, w, h - 12.0));
    out = out.child(chips(cx, src, at, i, tail, compact).pin(None, Some(top), Some(pad_r - 6.0), None));
    if rest > 0 {
        out = out.child(more(cx, rest, "line", tail).pin(Some(pad_x), Some(pad_top + shown as f32 * lh + r(pad_b / 2.0)), None, None));
    }
    (out, [w, h])
}

/// MAP-050: the reader's diff block in the capped code card, with Copy and Edit in its header.
#[allow(clippy::too_many_arguments)]
fn diff(cx: &Cx, lang: &str, src: &str, lines: &[DiffLine], (added, removed): (usize, usize), pid: Option<usize>, at: usize, i: usize, tail: &str, chip_compact: Option<f32>) -> (El, [f32; 2]) {
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
    let mut found = 0;
    let card = c.on_page(c.panel);
    for (k, (l, t)) in lines.iter().zip(texts).enumerate() {
        let (marks, n) = cx.marks(pid, &t, found);
        found += n;
        let (band, ink) = match l.kind {
            DiffKind::Add => (Some(c.green.alpha(0.14)), mix(c.text, c.green, 0.3)),
            DiffKind::Del => (Some(c.red.alpha(0.14)), mix(c.text, c.red, 0.3)),
            DiffKind::Hunk => (Some(c.blue.alpha(0.07)), c.blue),
            DiffKind::Meta => (None, c.dim),
            DiffKind::Ctx => (None, c.text),
        };
        // Preview's inks fall just short on some bands; the map lifts them to 4.5:1 (MAP-074).
        let ink = Roles::readable_on(ink, band.map_or(card, |f| mix(card, f.alpha(1.0), f.a)), 4.8);
        let mut rw = row()
            .key(format!("l-{k}"))
            .px(pad_x)
            .child(text(l.sign).key("sign").font(style).color(ink).w(sign_w))
            .child(text(if t.is_empty() { " ".to_string() } else { t }).key("text").font(style).color(ink).text_highlights(marks));
        if let Some(f) = band {
            rw = rw.fill(f);
        }
        rows = rows.child(rw);
    }
    let fades = (if natural > look.width { r(b * 2.4) } else { 0.0 }, if rest > 0 { BOTTOM_FADE } else { 0.0 });
    let body = el().pin(Some(0.0), Some(head_h), None, None).size(w, body_h).clip_fade_linear(0.0, 0.0, fades.0, fades.1).child(rows);
    let pad_r = r(pad_x * 0.4) + r(pad_x * 0.5);
    let compact = chip_compact.map(|side| clamp_compact_pair(side, w, h - 8.0));
    let chip_h = compact.unwrap_or_else(|| r(pcode::chip_size(b) * 2.0));
    let chip_top = match compact {
        Some(_) => 4.0,
        None => ((head_h - chip_h) / 2.0).max(1.0),
    };
    let mut inner = el()
        .pin(Some(0.0), Some(0.0), None, None)
        .size(w, h)
        .radius(4.0)
        .clip_rounded(4.0)
        .fill(c.panel)
        .border(1.0, c.on_page(c.hairline_soft))
        .child(head.pin(Some(0.0), Some(0.0), None, None))
        .child(body);
    if rest > 0 {
        inner = inner.child(more(cx, rest, "line", tail).pin(Some(pad_x), Some(head_h + body_h - r(py / 2.0)), None, None));
    }
    let out = el()
        .size(w, h)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(inner)
        .child(chips(cx, src, at, i, tail, compact).pin(None, Some(chip_top), Some(pad_r - 6.0), None));
    (out, [w, h])
}

/// MAP-051: the reader's table at the card's widest, capped at 8 body rows. When truncated,
/// `"+N more rows"` sits inside the same bordered table card with matching horizontal inset.
fn table(cx: &Cx, t: &Table, tone: Tone, i: usize, tail: &str, chip_compact: Option<f32>) -> (El, [f32; 2]) {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let mut g = Grid::new(cx, t, tone, look.width - 2.0);
    let shown = g.rows().min(TABLE_ROWS);
    let rest = g.rows() - shown;
    // The frame's 1 point of padding each side, round the grid.
    let natural = g.widths.iter().sum::<f32>() + 2.0;
    let pad_x = r(b * 0.85);
    let foot_w = if rest > 0 {
        let what = if rest == 1 { "row" } else { "rows" };
        let label = format!("+{rest} more {what}");
        rune::text::measure(&label, &[], more_style(cx, pcode::badge_size(b)), None).0.ceil() + 2.0 * pad_x + 4.0
    } else {
        0.0
    };
    let w = natural.max(foot_w).min(look.width);
    if w > natural && !g.widths.is_empty() {
        let extra = w - natural;
        let n = g.widths.len();
        let per = (extra / n as f32).floor();
        for (idx, cw) in g.widths.iter_mut().enumerate() {
            *cw += per + if idx + 1 == n { extra - per * n as f32 } else { 0.0 };
        }
    }
    let heights = g.heights();
    let inner_h = heights[..=shown].iter().sum::<f32>();
    let fades = (if natural > w { r(b * 2.8) } else { 0.0 }, if rest > 0 { BOTTOM_FADE } else { 0.0 });
    if rest == 0 {
        let grid_h = inner_h + 2.0;
        let compact = chip_compact.map(|side| side.min((w.min(grid_h) - 12.0).max(16.0)));
        let frame = g.frame(cx, shown).pin(Some(0.0), Some(0.0), None, None);
        let clipped = el().pin(Some(0.0), Some(0.0), None, None).size(w, grid_h).clip_fade_linear(0.0, 0.0, fades.0, 0.0).child(frame);
        let out = el()
            .size(w, grid_h)
            .hover_group()
            .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
            .child(clipped)
            .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
        return (out, [w, grid_h]);
    }
    let foot_py = r(b * 0.3);
    let foot_h = more_style(cx, pcode::badge_size(b)).line_height + 2.0 * foot_py;
    let total_h = inner_h + foot_h + 2.0;
    let compact = chip_compact.map(|side| side.min((w.min(total_h) - 12.0).max(16.0)));
    let grid = el()
        .pin(Some(1.0), Some(1.0), None, None)
        .size(w - 2.0, inner_h)
        .clip_fade_linear(0.0, 0.0, fades.0, fades.1)
        .child(g.inner(cx, shown).pin(Some(0.0), Some(0.0), None, None));
    let foot = row()
        .pin(Some(1.0), Some(1.0 + inner_h), None, None)
        .size(w - 2.0, foot_h)
        .px(pad_x)
        .center_y()
        .fill(c.bg)
        .border_sides([1.0, 0.0, 0.0, 0.0], c.hairline_soft)
        .child(more(cx, rest, "row", tail));
    let out = el()
        .size(w, total_h)
        .radius(4.0)
        .clip_rounded(4.0)
        .border(1.0, c.hairline)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12))
        .child(grid)
        .child(foot)
        .child(edit_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
    (out, [w, total_h])
}

/// MAP-046: the picture at its natural size, never larger, at most `B × 20` by `B × 14`, with
/// its alt text under it in at most two lines; the placeholder while it loads or when it is
/// missing. MAP-047: the Expand chip at its top-right corner under the pointer.
fn image(cx: &Cx, img: &Img, i: usize, tail: &str, chip_compact: Option<f32>) -> (El, [f32; 2]) {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let (pic, [pw, ph]) = thumb(cx, img, [r(b * 20.0), r(b * 14.0)]);
    let compact = chip_compact.map(|side| side.min((pw.min(ph) - 12.0).max(16.0)));
    let pic = pic.key(format!("map-pic:{tail}")).child(expand_chip(cx, i, tail, compact).pin(None, Some(6.0), Some(6.0), None).group_opacity(0.0, 1.0));
    let mut w = pw;
    let mut h = ph;
    let mut out = rune::col().items_center().hover_group().hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12)).child(pic);
    if !img.alt.is_empty() {
        let style = look.prose(r(b * 0.84), 1.5).italic();
        w = w.max(rune::text::measure(&img.alt, &[], style, None).0.ceil().min(r(b * 20.0)));
        let cap_h = rune::text::measure(&img.alt, &[], style, Some(w)).1.min(2.0 * style.line_height);
        let gap = r(b * 0.6);
        h += gap + cap_h;
        out = out.child(text(img.alt.clone()).key(format!("map-caption:{tail}")).font(style).color(c.dim).wrap().max_lines(2).align(rune::Align::Center).w(w).h(cap_h).mt(gap));
    }
    (out.size(w, h), [w, h])
}

/// Image `img` fitted inside `max` without growing, its corners rounded under a hairline, or
/// the placeholder at its own width up to `max`'s.
fn thumb(cx: &Cx, img: &Img, [max_w, max_h]: [f32; 2]) -> (El, [f32; 2]) {
    let c = &cx.look.c;
    let ready = match cx.pic(img) {
        Pic::Ready(im) => im.size().filter(|[w, h]| *w > 0 && *h > 0).map(|[w, h]| (im, [w as f32, h as f32])).ok_or_else(|| LOADING.to_string()),
        Pic::Loading => Err(LOADING.to_string()),
        Pic::Missing(msg) => Err(msg),
    };
    match ready {
        Ok((im, [nw, nh])) => {
            let k = 1f32.min(max_w / nw).min(max_h / nh);
            let [w, h] = [(nw * k).round().max(1.0), (nh * k).round().max(1.0)];
            let alt = if img.alt.is_empty() { "Image".to_string() } else { img.alt.clone() };
            let edge = el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(4.0).border(1.0, c.hairline);
            (el().size(w, h).radius(4.0).clip_rounded(4.0).role("img").label(alt).child(el().pixels(im.pixels()).size(w, h)).child(edge), [w, h])
        }
        Err(msg) => {
            let w = cx.placeholder_w(&msg).min(max_w);
            let h = r(cx.look.b * 3.4).min(max_h);
            (cx.placeholder(msg, None).size(w, h), [w, h])
        }
    }
}

/// The Expand chip: the lightbox on the image (MAP-047).
fn expand_chip(cx: &Cx, i: usize, tail: &str, compact: Option<f32>) -> El {
    let c = &cx.look.c;
    let key = format!("map-expand:{tail}");
    if let Some(side) = compact {
        return icon_chip(cx, &key, &icons::EXPAND, side, c.readable_dim(), "Expand the image", "Expand")
            .on_press_tx(move |s: &mut App, tx| s.map_lightbox(i, 0, tx));
    }
    let size = pcode::chip_size(cx.look.b);
    Chip { key: &key, glyph: &icons::EXPAND, size, icon: r(size * 1.1), label: "", mono: cx.look.mono, tint: c.readable_dim(), active: false, fill: Some(c.panel) }
        .el(c)
        .label("Expand the image")
        .on_press_tx(move |s: &mut App, tx| s.map_lightbox(i, 0, tx))
}

/// MAP-047: a gallery's images as thumbnails at most `B × 8` tall, in rows that wrap at the
/// card's widest, each opening the lightbox on itself. Each sits in a tile as tall as its row,
/// so a small one beside a tall one leaves no hole in the grid.
fn gallery(cx: &Cx, items: &[GalleryItem], i: usize, tail: &str) -> (El, [f32; 2]) {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let pad = r(b * 0.5);
    let gap = r(b * 0.5);
    let room = look.width - 2.0 * pad;
    let mut rows: Vec<Vec<(usize, El, [f32; 2])>> = Vec::new();
    let mut line = Vec::new();
    let mut lw = 0f32;
    for (k, it) in items.iter().enumerate() {
        let (t, [tw, th]) = thumb(cx, &it.img, [room, r(b * 8.0)]);
        if lw > 0.0 && lw + gap + tw > room {
            rows.push(std::mem::take(&mut line));
            lw = 0.0;
        }
        lw += if lw > 0.0 { gap } else { 0.0 } + tw;
        line.push((k, t, [tw, th]));
    }
    if !line.is_empty() {
        rows.push(line);
    }
    let size = |line: &[(usize, El, [f32; 2])]| {
        let w = line.iter().map(|(_, _, [w, _])| *w).sum::<f32>() + gap * line.len().saturating_sub(1) as f32;
        [w, line.iter().map(|(_, _, [_, h])| *h).fold(0.0, f32::max)]
    };
    let w = rows.iter().map(|l| size(l)[0]).fold(0.0, f32::max) + 2.0 * pad;
    let h = rows.iter().map(|l| size(l)[1]).sum::<f32>() + gap * rows.len().saturating_sub(1) as f32 + 2.0 * pad;
    let mut out = rune::col().key(format!("map-gallery:{tail}")).size(w, h).padding(pad).gap(gap).radius(4.0).fill(c.panel).border(1.0, c.on_page(c.hairline_soft));
    for (n, line) in rows.into_iter().enumerate() {
        let lh = size(&line)[1];
        let mut rw = row().key(format!("row-{n}")).gap(gap);
        for (k, t, [tw, _]) in line {
            let it = &items[k];
            let tip = if it.title.is_empty() { &it.img.alt } else { &it.title };
            let mut tile = el()
                .key(format!("map-thumb:{tail}:{k}"))
                .size(tw, lh)
                .radius(4.0)
                .fill(c.raised)
                .center()
                .child(t)
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx| s.map_lightbox(i, k, tx));
            if !tip.is_empty() {
                tile = tile.help(tip.clone());
            }
            rw = rw.child(tile);
        }
        out = out.child(rw);
    }
    (out, [w, h])
}
