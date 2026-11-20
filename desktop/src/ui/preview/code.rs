//! Code blocks (BLK-43 to BLK-50) and diff blocks (BLK-53 to BLK-58).

use rune::{El, Span, TextStyle, col, el, row, text};

use super::Cx;
use crate::ui::icons;
use crate::blocks::{DiffKind, DiffLine};
use crate::page::{line, r};
use crate::roles::mix;

/// Tabs to spaces at stops of 4 characters (BLK-49).
fn expand_tabs(s: &str) -> String {
    if !s.contains('\t') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 8);
    let mut col = 0;
    for ch in s.chars() {
        match ch {
            '\t' => {
                let n = 4 - col % 4;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            '\n' => {
                out.push(ch);
                col = 0;
            }
            _ => {
                out.push(ch);
                col += 1;
            }
        }
    }
    out
}

/// Scaled font size for the Copy chip (12 pt at default 14 pt body -> 24 pt chip height),
/// scaling proportionally with preview body size `b / 14.0`, clamped for extreme zoom levels.
pub(super) fn chip_size(b: f32) -> f32 {
    r((12.0 * (b / 14.0)).clamp(10.0, 20.0))
}

/// Scaled font size for code/diff metadata labels (language badge, diff counts), proportional
/// to preview body size `b / 14.0` (11 pt at default 14 pt body), clamped sensibly.
pub(super) fn badge_size(b: f32) -> f32 {
    r((11.0 * (b / 14.0)).clamp(9.0, 18.0))
}

/// Top inset for the badge / Copy chip and top padding for the code body (BLK-44, BLK-51).
/// Keeps at least 4 pt of vertical clearance between the chip and line 1 at every zoom level.
pub(super) fn top_clearance(b: f32) -> (f32, f32) {
    let chip_h = r(chip_size(b) * 2.0);
    let top = 4f32.max(r(b * 0.3));
    let pad_top = r(b * 1.25).max(top + chip_h + 4.0);
    (top, pad_top)
}

/// The language badge: lower case, mono scaled with preview body size, readable on the code
/// panel across every theme (BLK-50).
fn badge(cx: &Cx, lang: &str) -> El {
    let b = cx.look.b;
    let sz = badge_size(b);
    let lh = r(chip_size(b) * 2.0);
    text(lang.to_lowercase()).key("badge").font(TextStyle::new(sz, 400.0).family(cx.look.mono).lh(lh).tracking(0.4)).color(cx.look.c.readable_dim())
}

pub(super) fn code(cx: &Cx, lang: &str, src: &str, pid: Option<usize>, at: usize) -> El {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let size = crate::page::code_size(b);
    let style = look.mono_at(size, 1.5);
    let src = expand_tabs(src);
    let spans: Vec<Span> = crate::highlight::highlight(lang, &src)
        .iter()
        .map(|(range, st)| {
            let ink = c.code(*st);
            Span {
                color: Some(ink.color),
                italic: ink.italic,
                weight: ink.bold.then_some(700.0),
                underline: ink.underline.then_some(ink.color),
                ..Span::new(range.start, range.end)
            }
        })
        .collect();
    let n = src.split('\n').count();
    let pad_x = r(b * 1.1);
    let (top, pad_top) = top_clearance(b);
    // The numbers stay put while the code scrolls; the scroll area sits inside the panel's
    // padding, and its right edge fades while there is more that way (BLK-46 to BLK-48).
    let mut body = row().key("lines").w_full().pt(pad_top).pb(r(b * 0.85)).px(pad_x).items_start();
    if n > 8 {
        let nums = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let numw = rune::text::advance(&n.to_string(), style);
        body = body.child(text(nums).key("nums").font(style.features("'tnum' 1")).color(c.ghost).align(rune::Align::End).w(numw.ceil()).mr(r(size * 1.4)));
    }
    let marks = cx.marks(pid, &src, 0).0;
    let copy = src.clone();
    // Code never wraps: a long line runs on and the area scrolls sideways (BLK-47).
    let code = cx.selectable_text(text(if src.is_empty() { " ".to_string() } else { src }).spans(spans).key("code").font(style).color(c.text).text_highlights(marks));
    let fade = rune::ScrollFade::strength(r(b * 2.4)).full_at(1.0).linear().end_only();
    body = body.child(el().key("scroll").grow(1.0).min_w(0.0).scroll_x().scroll_fade(fade).child(code));
    let pad_r = r(pad_x * 0.4) + r(pad_x * 0.5);
    let chip_sz = chip_size(b);
    // Over the block the badge gives way to a Copy chip, which says Copied in green for a
    // moment after a click, with or without the pointer (BLK-51).
    let copied = cx.v.s.reader.copied.is_some_and(|(l, _)| l == at);
    let (glyph, label, tint) = if copied { (&icons::CHECK, "Copied", c.green) } else { (&icons::COPY, "Copy", c.readable_dim()) };
    let chip = crate::ui::chip::Chip { key: "copy", glyph, size: chip_sz, icon: r(chip_sz * 1.1), label, mono: look.mono, tint, active: false, fill: Some(c.panel) }
        .el(c)
        .group_opacity(if copied { 1.0 } else { 0.0 }, 1.0)
        .label("Copy the code")
        .on_press_tx(move |s: &mut crate::app::App, tx| s.copy_code(&copy, at, tx))
        .pin(None, Some(top), Some(pad_r - 6.0), None);
    let mut out = el().w_full().hover_group().hover_motion(rune::Curve::linear(0.12), rune::Curve::linear(0.12)).radius(4.0).fill(c.panel).border(1.0, c.on_page(c.hairline_soft)).child(body);
    if !lang.is_empty() {
        out = out.child(badge(cx, lang).group_opacity(if copied { 0.0 } else { 1.0 }, 0.0).pin(None, Some(top), Some(pad_r), None));
    }
    out.child(chip)
}

pub(super) fn diff(cx: &Cx, lang: &str, lines: &[DiffLine], added: usize, removed: usize, pid: Option<usize>) -> El {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let size = crate::page::code_size(b);
    let style = look.mono_at(size, 1.5);
    let pad_x = r(b * 1.1);
    let bs = badge_size(b);
    let count = TextStyle::new(bs, 700.0).family(look.mono).lh(line(bs, 1.3));
    let head = row()
        .key("head")
        .w_full()
        .h(r(b * 2.1).max(r(chip_size(b) * 2.0) + 4.0))
        .px(pad_x)
        .gap(r(b * 0.6))
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], c.hairline_soft)
        .child(text(format!("+{added}")).key("added").font(count).color(c.green))
        .child(text(format!("\u{2212}{removed}")).key("removed").font(count).color(c.red))
        .child(el().key("space").grow(1.0))
        .child(if lang.is_empty() { el() } else { badge(cx, lang) });
    // Bands run the full width of the panel, or of the longest line when it scrolls.
    let mut body = col().key("lines").min_w(look.width - 2.0).py(r(b * 0.7));
    let mut k = 0;
    for (i, l) in lines.iter().enumerate() {
        let (band, ink) = match l.kind {
            DiffKind::Add => (Some(c.green.alpha(0.14)), mix(c.text, c.green, 0.3)),
            DiffKind::Del => (Some(c.red.alpha(0.14)), mix(c.text, c.red, 0.3)),
            DiffKind::Hunk => (Some(c.blue.alpha(0.07)), c.blue),
            DiffKind::Meta => (None, c.faint),
            DiffKind::Ctx => (None, c.text),
        };
        let t = expand_tabs(&l.text);
        let (marks, n) = cx.marks(pid, &t, k);
        k += n;
        let mut rw = row()
            .key(format!("l-{i}"))
            .px(pad_x)
            .child(text(l.sign).key("sign").font(style).color(ink).w(r(size * 1.6)))
            .child(cx.selectable_text(text(if t.is_empty() { " ".to_string() } else { t }).key("text").font(style).color(ink).text_highlights(marks)));
        if let Some(f) = band {
            rw = rw.fill(f);
        }
        body = body.child(rw);
    }
    col().w_full().radius(4.0).clip_rounded(4.0).fill(c.panel).border(1.0, c.on_page(c.hairline_soft)).child(head).child(el().key("scroll").w_full().scroll_x().child(body))
}

#[cfg(test)]
mod tests {
    #[test]
    fn blk_49_tab_stops() {
        assert_eq!(super::expand_tabs("\tx\n ab\ty"), "    x\n ab y");
    }

    #[test]
    fn targets24_code_meta_scales_with_zoom() {
        assert_eq!(super::chip_size(14.0), 12.0, "default Copy chip is 24 pt tall");
        assert!(super::chip_size(17.0) > super::chip_size(14.0), "Copy chip grows with zoom");
        assert!(super::badge_size(17.0) > super::badge_size(14.0), "language badge grows with zoom");
        assert_eq!(super::chip_size(6.0), 10.0, "lower clamp");
        assert_eq!(super::chip_size(40.0), 20.0, "upper clamp");
    }

    #[test]
    fn codeclear_top_padding_clears_copy_chip() {
        for b in [9.0, 10.0, 14.0, 18.0, 20.0, 28.0] {
            let (top, pad_top) = super::top_clearance(b);
            let chip_h = super::r(super::chip_size(b) * 2.0);
            assert!(pad_top >= top + chip_h + 4.0, "b={b}: pad_top {pad_top} < top {top} + chip_h {chip_h} + 4");
        }
    }
}
