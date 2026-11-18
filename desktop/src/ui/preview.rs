//! The preview: a note drawn as omarky's reader page, from `crate::blocks`. The numbers come
//! from `docs/reader-writer/behavior/page-and-typography.md` and `blocks.md`, by item id.

mod code;
mod front;
mod list;
mod table;

use std::borrow::Cow;
use std::rc::Rc;

use rune::{Color, El, Family, LinkPaint, Span, TextCase, TextHighlight, TextStyle, Tx, col, el, icon, rich, row, text};

use super::V;
use super::octicons;
use crate::app::App;
use crate::blocks::{Alert, Block, Doc, Footnote, GalleryItem, Kind, Link, Run};
use crate::md::Img;
use crate::page::{self, line, r, step};
use crate::roles::{Roles, mix};
use crate::theme;

/// The page's fonts, sizes and colours.
struct Look {
    /// The body size `B` (PAGE-10).
    b: f32,
    family: Family,
    mono: Family,
    /// The mono font's advance, in em.
    mono_w: f32,
    asc: f32,
    desc: f32,
    mono_asc: f32,
    mono_desc: f32,
    /// The prose font's x-height, in em.
    xh: f32,
    /// The column width (PAGE-24).
    width: f32,
    dark: bool,
    /// The page's colours (`crate::roles`), named by `behavior/blocks.md`.
    c: Roles,
}

impl Look {
    fn prose(&self, size: f32, lead: f32) -> TextStyle {
        TextStyle::new(size, 400.0).family(self.family).lh(line(size, lead))
    }
    fn mono_at(&self, size: f32, lead: f32) -> TextStyle {
        TextStyle::new(size, 400.0).family(self.mono).lh(line(size, lead)).features("'liga' 0, 'clig' 0, 'calt' 0")
    }
    /// A prose baseline below the top of a line box `lh` tall.
    fn baseline(&self, size: f32, lh: f32) -> f32 {
        (lh - (self.asc + self.desc) * size) / 2.0 + self.asc * size
    }
    /// The centre of the x-height in that line box (BLK-25).
    fn x_mid(&self, size: f32, lh: f32) -> f32 {
        self.baseline(size, lh) - self.xh * size / 2.0
    }
    /// A light or dark factor.
    fn ld(&self, light: f32, dark: f32) -> f32 {
        if self.dark { dark } else { light }
    }
}

/// How a container sets what it holds: size, colour tier and leading (BLK-10).
#[derive(Clone, Copy)]
struct Tone {
    color: Color,
    /// The text size `T`.
    size: f32,
    lead: f32,
    /// Inside a quote, alert or footnote: lists indent rather than hang (BLK-21).
    contained: bool,
    quote_depth: u8,
}

/// The column the page sits in (PAGE-21 to PAGE-27). The editor lays the page out with it.
pub fn column(v: V) -> page::Column {
    let set = v.s.settings();
    let prose = theme::pick(&theme::PROSE_FONTS, &set.preview_font);
    let left = if v.left_visible() { 249.0 } else { 0.0 };
    let right = if v.right_visible() { 236.0 } else { 0.0 };
    page::column(v.vw - left - right, set.size as f32, page::avg_char(prose.family), v.s.wide(), left - right)
}

/// Rich text for one block: its string, spans, struck ranges and links.
#[derive(Default)]
struct Rich {
    text: String,
    spans: Vec<Span>,
    strikes: Vec<(usize, usize, Option<Color>)>,
    links: Vec<(std::ops::Range<usize>, String, Color)>,
    /// Find matches painted over the text (RD-59).
    marks: Vec<TextHighlight>,
    /// Inline-code chips, which get a hairline edge (PAGE-76).
    chips: Vec<(usize, usize, Color)>,
}

impl Rich {
    fn push(&mut self, s: &str, sp: Span) {
        let a = self.text.len();
        self.text.push_str(s);
        let b = self.text.len();
        if a < b && (sp.weight.is_some() || sp.italic || sp.color.is_some() || sp.size.is_some() || sp.mono) {
            self.spans.push(Span { start: a as u32, end: b as u32, ..sp });
        }
    }

    /// Runs (`crate::blocks::Run`) to styled text (PAGE-70 to PAGE-79).
    fn runs(&mut self, runs: &[Run], cx: &Cx, tone: Tone) {
        let look = cx.look;
        let c = &look.c;
        // Inline code is at the body's code size wherever it sits: a table cell, a footnote or a
        // heading (PAGE-75).
        let code = r(look.b * 0.86);
        for run in runs {
            let a = self.text.len();
            if run.br {
                self.text.push('\n');
                continue;
            }
            let linked = matches!(run.link, Some(Link::Url(_) | Link::Wiki(_) | Link::Anchor(_))) && run.img.is_none();
            let base = Span {
                weight: run.b.then_some(700.0),
                italic: run.i,
                color: if run.b && !linked { Some(c.strong) } else { None },
                ..Span::default()
            };
            if run.code {
                self.chip(&run.t, code, tone.size, look, Some(c.code_text));
                continue;
            }
            if run.raw {
                self.push(&run.t, Span { mono: true, size: Some(code), color: Some(c.faint), ..Span::default() });
                continue;
            }
            match &run.link {
                Some(Link::Tag(tag)) => {
                    self.chip(&run.t, code, tone.size, look, Some(c.accent));
                    self.links.push((a..self.text.len(), format!("tag:{tag}"), c.accent));
                    continue;
                }
                Some(Link::Wiki(target)) => {
                    let (name, _) = crate::md::wiki_parts(target);
                    let ok = name.is_empty() || cx.v.s.names.resolve(&name).is_some();
                    let color = if ok { c.link } else { c.faint };
                    self.push(&run.t, Span { color: Some(color), ..base });
                    self.links.push((a..self.text.len(), format!("wiki:{}", target.trim()), color));
                }
                Some(Link::Url(u)) if run.img.is_none() => {
                    self.push(&run.t, Span { color: Some(c.link), ..base });
                    self.links.push((a..self.text.len(), format!("url:{u}"), c.link));
                }
                Some(Link::Anchor(an)) => {
                    // A footnote reference is small (PAGE-78, adapted): omarky's <sup> is 10 px at
                    // 14 px. It sits on the baseline, since Rune has no raised spans.
                    let sup = if run.note { Span { size: Some(r(cx.look.b * 0.7)), ..base } } else { base };
                    self.push(&run.t, Span { color: Some(c.link), ..sup });
                    if let Some(l) = cx.anchor_line(an) {
                        self.links.push((a..self.text.len(), format!("line:{l}"), c.link));
                    }
                }
                // An image inside a sentence shows its alt text.
                _ if run.img.is_some() => self.push(&run.t, Span { color: Some(c.dim), italic: true, ..base }),
                _ => self.push(&run.t, base),
            }
            if run.s {
                let b = self.text.len();
                self.spans.push(Span { color: Some(c.dim), ..Span::new(a, b) });
                self.strikes.push((a, b, Some(c.dim)));
            }
        }
    }

    /// Inline code on a chip (PAGE-75, PAGE-76): mono, a quarter em of room each side, the chip
    /// 1.5 × its size tall.
    fn chip(&mut self, t: &str, size: f32, around: f32, look: &Look, color: Option<Color>) {
        let bg = look.c.code_bg;
        let pad_y = ((r(size * 1.5) - (look.mono_asc + look.mono_desc) * size) / 2.0).max(0.0);
        // Each side is a four-per-em space of the text around the chip, as omarky's U+2005 sits
        // outside the code: a mono space (`mono_w` em) at `side` is a quarter em of `around`.
        let side = 0.25 * around / look.mono_w;
        // No-break spaces: a plain one is a break opportunity, and a line broken there left the
        // chip's leading side behind as a dot of chip at the end of the line above.
        let a = self.text.len();
        self.text.push('\u{a0}');
        let b = self.text.len();
        self.text.push_str(t);
        let c = self.text.len();
        self.text.push('\u{a0}');
        let d = self.text.len();
        self.spans.push(Span { mono: true, size: Some(side), bg: Some(bg), bg_pad: pad_y, ..Span::new(a, b) });
        self.spans.push(Span { mono: true, size: Some(side), bg: Some(bg), bg_pad: pad_y, ..Span::new(c, d) });
        self.spans.push(Span { mono: true, size: Some(size), color, bg: Some(bg), bg_pad: pad_y, ..Span::new(b, c) });
        self.chips.push((a, d, look.c.chip_edge));
    }

    fn el(self, style: TextStyle, color: Color) -> El {
        let mut e = rich(if self.text.is_empty() { " ".to_string() } else { self.text }, self.spans).font(style).color(color).wrap().span_radius(4.0);
        for (a, b, c) in self.strikes {
            e = e.strike(a, b, c);
        }
        e = e.text_highlights(self.marks).span_rings(self.chips.into_iter().map(|(a, b, c)| (a, b, c, 1.0)));
        for (range, id, color) in self.links {
            // Links underline only under the pointer (PAGE-73).
            e = e.link(rune::Link::new(range, id).hover(LinkPaint::new().underline(color)));
        }
        e.on_link(on_link)
    }
}

fn on_link(s: &mut App, id: &str, tx: &mut Tx) {
    if let Some(t) = id.strip_prefix("wiki:") {
        s.open_link(t, tx);
    } else if let Some(l) = id.strip_prefix("line:").and_then(|l| l.parse().ok()) {
        s.jump(l, tx);
    } else if let Some(t) = id.strip_prefix("tag:") {
        s.search_tag(t, tx);
    } else if let Some(u) = id.strip_prefix("url:") {
        s.follow(u, tx);
    }
}

/// The page's look at the reader's size and column.
fn look(v: V) -> Look {
    let set = v.s.settings();
    let prose = theme::pick(&theme::PROSE_FONTS, &set.preview_font);
    let mono = theme::pick(&theme::CODE_FONTS, &set.code_font).family;
    let b = set.size as f32;
    let (asc, desc) = theme::metrics(prose.family);
    let (mono_asc, mono_desc) = theme::metrics(mono);
    Look {
        b,
        family: prose.family,
        mono,
        mono_w: page::avg_char(mono),
        asc,
        desc,
        mono_asc,
        mono_desc,
        xh: page::x_height(prose.family),
        width: column(v).width,
        dark: v.p.dark,
        c: Roles::new(v.p),
    }
}

/// The page (VER-32): a virtual list of the note's top-level blocks, so only the rows near the
/// view exist. Each row is its block with the rhythm's gap above it (PAGE-54 to PAGE-67), keyed
/// by the block's content key, so the outline and links can scroll to it and the page holds its
/// place when lines above it change (RD-11).
pub fn preview(v: V) -> El {
    let s = v.s;
    let doc = s.doc();
    let find = s.find_query().map(|q| {
        let cur = s.find_matches().get(s.reader.find.cur).map(|m| (m.pid, m.k));
        Hl { q: q.to_lowercase(), q_ascii: q.to_ascii_lowercase(), cur }
    });
    // A row is built again only when something it draws from changes.
    let set = s.settings();
    let version = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
        s.doc_hash(),
        s.active.as_deref(),
        s.vault.as_ref().map(|v| &v.path),
        (set.size, &set.preview_font, &set.code_font),
        format!("{:?}{:?}", v.p, column(v)),
        find.as_ref().map(|f| (&f.q, f.cur)),
        &s.names,
        s.images.epoch.get(),
        s.reader.copied.map(|(l, _)| l),
        )
            .hash(&mut h);
        h.finish()
    };
    let look = Rc::new(look(v));
    let find = Rc::new(find);
    let gaps = Rc::new(page::gaps(look.b, &doc.blocks));
    let estimate = page::row_estimate(look.b);
    let (p, dark, vw, vh, trailing) = (v.p, v.dark, v.vw, v.vh, v.trailing);
    // The rows read the blocks this view parsed: `App::doc` hashes the whole note to find its
    // cache, and the list asks every row's key when it is made.
    let rows = doc.clone();
    col()
        .key("preview")
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            doc.blocks.len(),
            estimate,
            move |_: &App, i| Cow::Owned(doc.blocks.get(i).map_or_else(String::new, crate::reader::block_key)),
            move |s: &App, i| {
                let v = V { s, p, dark, vw, vh, trailing };
                let doc = rows.clone();
                let Some(bl) = doc.blocks.get(i) else { return el() };
                let gap = gaps.get(i).copied().unwrap_or(0.0);
                let key = crate::reader::block_key(bl);
                rune::memo(key.clone(), version, || {
                    let cx = Cx { v, look: &look, doc: &doc, find: (*find).as_ref() };
                    let tone = Tone { color: look.c.text, size: look.b, lead: 1.6, contained: false, quote_depth: 0 };
                    // A right-click on the row, its gap above included, opens its block's menu (RD-68).
                    let line = bl.lines.start;
                    el().w_full()
                        .pt(gap)
                        .child(cx.top_block(bl, tone))
                        .on_layout_tx(move |s: &mut App, rect, tx| s.row_laid_out(&key, rect, tx))
                        .on_secondary_press(move |s: &mut App, at, tx| s.open_menu(crate::layer::MenuOf::Block(line), (at.x, at.y), tx))
                })
            },
        )
        .virtual_rows_version(s.doc_hash())
}

/// Top-level block `i` of `doc` drawn by the page's renderer in the dim tier, for the edit
/// sheet's context (SHEET-022).
pub fn context(v: V, doc: &Doc, i: usize) -> El {
    let look = look(v);
    let Some(bl) = doc.blocks.get(i) else { return el() };
    let cx = Cx { v, look: &look, doc, find: None };
    let tone = Tone { color: look.c.dim, size: look.b, lead: 1.6, contained: false, quote_depth: 0 };
    cx.block(bl, tone).w_full()
}

/// The find query, lower-cased, and the current match as (piece, match in the piece).
struct Hl {
    q: String,
    /// For text whose lower case changes its length, where byte offsets would not line up.
    q_ascii: String,
    cur: Option<(usize, usize)>,
}

/// What every block needs: the view, the page's look, the whole document (for anchors) and
/// the find query.
#[derive(Clone, Copy)]
struct Cx<'a> {
    v: V<'a>,
    look: &'a Look,
    doc: &'a Doc,
    find: Option<&'a Hl>,
}

impl Cx<'_> {
    /// The first source line of the block an anchor names.
    fn anchor_line(&self, anchor: &str) -> Option<usize> {
        self.doc.anchors.get(anchor).and_then(|&i| self.doc.blocks.get(i)).map(|b| b.lines.start)
    }

    fn rich(&self, runs: &[Run], tone: Tone) -> Rich {
        let mut r = Rich::default();
        r.runs(runs, self, tone);
        r
    }

    /// `rich`, with the find matches of piece `pid` marked.
    fn rich_at(&self, runs: &[Run], tone: Tone, pid: Option<usize>) -> Rich {
        let mut r = self.rich(runs, tone);
        r.marks = self.marks(pid, &r.text, 0).0;
        r
    }

    /// Find highlights over `text`, piece `pid`, numbering matches from `k0` (RD-59): the find
    /// colour, and the current match solid accent with its key for scrolling. Returns the
    /// highlights and the number of matches.
    fn marks(&self, pid: Option<usize>, text: &str, k0: usize) -> (Vec<TextHighlight>, usize) {
        let (Some(h), Some(pid)) = (self.find, pid) else { return (Vec::new(), 0) };
        let lower = text.to_lowercase();
        let (hay, q) = if lower.len() == text.len() { (lower, &h.q) } else { (text.to_ascii_lowercase(), &h.q_ascii) };
        let c = &self.look.c;
        let mut out = Vec::new();
        let mut at = 0;
        let mut k = k0;
        while let Some(f) = hay[at..].find(q.as_str()) {
            let (a, b) = (at + f, at + f + q.len());
            out.push(if h.cur == Some((pid, k)) {
                TextHighlight::new(a..b, c.find_current).color(c.find_current_text).key(crate::reader::FIND_CUR)
            } else {
                TextHighlight::new(a..b, c.find)
            });
            k += 1;
            at = b.max(a + 1);
        }
        (out, k - k0)
    }

    /// A top-level block of the page. The block under the pointer is the one Ctrl+Shift+C and e
    /// act on (RD-100).
    fn top_block(&self, bl: &Block, tone: Tone) -> El {
        let line = bl.lines.start;
        self.block(bl, tone)
            .key("block")
            .on_hover_change(move |s: &mut App, on, tx| {
                if on {
                    s.reader.hover = Some(line);
                } else if s.reader.hover == Some(line) {
                    s.reader.hover = None;
                }
                // Nothing on the page draws the hover, so a scroll that moves blocks under the
                // pointer runs no view.
                tx.live_only();
            })
    }

    /// Blocks one under another inside a block (a quote, an alert, a list item), each with the
    /// gap the rhythm gives it (PAGE-66, PAGE-67).
    fn stack(&self, blocks: &[Block], tone: Tone, tight: bool) -> El {
        let b = self.look.b;
        let mut out = col();
        let mut prev = None;
        for (i, bl) in blocks.iter().enumerate() {
            let k = page::rk(&bl.kind);
            out = out.child(self.block(bl, tone).mt(page::inner_gap(b, prev, k, tight)).key(format!("b-{i}")));
            prev = Some(k);
        }
        out
    }

    fn block(&self, bl: &Block, tone: Tone) -> El {
        match &bl.kind {
            Kind::FrontMatter { title, fields } => front::card(self, title, fields),
            Kind::Heading { level, runs, text: t, slug } => self.heading(*level, runs, t, slug, bl.pid, tone),
            Kind::Paragraph { runs } if is_tag_line(runs) => tag_line(self, runs, tone),
            Kind::Paragraph { runs } => self.rich_at(runs, tone, bl.pid).el(self.look.prose(tone.size, tone.lead), tone.color).w_full(),
            Kind::List(l) => list::list(self, l, tone),
            Kind::Quote { blocks } => self.quote(blocks, tone),
            Kind::Alert { kind, blocks } => self.alert(*kind, blocks, tone),
            Kind::Code { lang, text: t } => code::code(self, lang, t, bl.pid, bl.lines.start),
            Kind::Diff { lang, lines, added, removed, .. } => code::diff(self, lang, lines, *added, *removed, bl.pid),
            Kind::Table(t) => table::table(self, t, tone),
            Kind::Rule => self.rule(),
            Kind::Image { img, .. } => self.image(img),
            Kind::Gallery { items } => self.gallery(items),
            Kind::Html { text: t } => self.html(t),
            Kind::Footnotes { items } => self.footnotes(items),
        }
    }

    /// PAGE-03, PAGE-14, PAGE-15, BLK-11 to BLK-14.
    fn heading(&self, level: usize, runs: &[Run], label: &str, slug: &str, pid: Option<usize>, tone: Tone) -> El {
        let look = self.look;
        let level = level.clamp(1, 6);
        let size = step(look.b, [3.0, 2.0, 1.0, 0.5, 0.0, 0.0][level - 1]);
        let lead = if level <= 2 { 1.25 } else { 1.35 };
        let (weight, color) = if level <= 4 { (700.0, look.c.strong) } else { (400.0, look.c.dim) };
        let mut style = TextStyle::new(size, weight).family(look.family).lh(line(size, lead));
        if level == 6 {
            style = style.tracking(0.6);
        }
        let mut e = self.rich_at(runs, Tone { size, ..tone }, pid).el(style, color).w_full().role("heading").level(level as u32).label(label.to_string());
        if level == 6 {
            e = e.text_case(TextCase::Uppercase);
        }
        if tone.quote_depth > 0 || tone.contained || slug.is_empty() {
            return e;
        }
        // The `#` in the margin while the pointer is over the heading (BLK-15, BLK-16).
        let hs = r(look.b * 0.95);
        let hash_style = TextStyle::new(hs, 400.0).family(look.mono).lh(line(hs, 1.2));
        let w = rune::text::advance("#", hash_style).ceil();
        let top = ((line(size, lead) - line(hs, 1.2)) / 2.0).max(0.0);
        let slug = slug.to_string();
        // Faint, and accent under the pointer itself; fades and colour changes take 120 ms.
        let fade = || rune::Curve::linear(0.12);
        let hash = el()
            .key("anchor")
            .hover_group()
            .hover_motion(fade(), fade())
            .group_opacity(0.0, 1.0)
            .cursor(rune::Cursor::Pointer)
            .role("button")
            .label("Copy a link to this heading")
            .on_press_tx(move |s: &mut App, tx| s.copy_heading_link(&slug, tx))
            .child(text("#").font(hash_style).color(look.c.faint).group_color(look.c.accent))
            .pin(Some(-(w + r(look.b * 0.6))), Some(top), None, None);
        el().w_full().hover_group().hover_motion(fade(), fade()).child(e).child(hash)
    }

    /// Three faint dots (BLK-17).
    fn rule(&self) -> El {
        let b = self.look.b;
        let d = 3f32.max(r(b * 0.22));
        let dot = |i: usize| el().key(format!("dot-{i}")).size(d, d).radius(d / 2.0).fill(self.look.c.faint);
        row().w_full().h(r(b * 1.2)).justify_center().center_y().gap(r(b * 0.9)).child(dot(0)).child(dot(1)).child(dot(2))
    }

    /// BLK-33 to BLK-36.
    fn quote(&self, blocks: &[Block], tone: Tone) -> El {
        let look = self.look;
        let b = look.b;
        let bar = 2f32.max(r(b / 6.0));
        let inset = if tone.quote_depth >= 8 { r(b * 0.5) } else { r(b * 1.2) };
        let inner = Tone { color: look.c.dim, size: r(b * 0.94), contained: true, quote_depth: tone.quote_depth + 1, ..tone };
        row()
            .w_full()
            .items_stretch()
            .child(el().key("bar").w(bar).radius(bar / 2.0).fill(mix(look.c.accent, look.c.bg, 0.35)))
            .child(self.stack(blocks, inner, false).key("body").grow(1.0).min_w(0.0).ml(inset - bar))
    }

    /// BLK-37 to BLK-42.
    fn alert(&self, kind: Alert, blocks: &[Block], tone: Tone) -> El {
        let look = self.look;
        let c = &look.c;
        let b = look.b;
        let (tint, glyph) = match kind {
            Alert::Note => (c.blue, &octicons::INFO),
            Alert::Tip => (c.green, &octicons::LIGHT_BULB),
            Alert::Important => (c.magenta, &octicons::REPORT),
            Alert::Warning => (c.yellow, &octicons::ALERT),
            Alert::Caution => (c.red, &octicons::STOP),
        };
        let ts = r(b * 0.76);
        let isz = r(b * 0.95);
        // omarky draws the icon as a Nerd Font glyph, so the row is as tall as that font's line:
        // JetBrains Mono's ascent and descent come to 1.32 em [18].
        let title = row()
            .key("title")
            .h((isz * 1.32).ceil())
            .gap(r(b * 0.5))
            .center_y()
            .child(icon(glyph, isz).color(tint))
            .child(text(kind.title()).font(TextStyle::new(ts, 700.0).family(look.mono).lh(line(ts, 1.3)).tracking(r(b * 0.08))).color(tint).text_case(TextCase::Uppercase));
        let inner = Tone { color: c.text, size: b, contained: true, ..tone };
        let fill = look.ld(0.075, 0.09);
        col()
            .w_full()
            .padding(r(b * 1.05))
            .gap(r(b * 0.5))
            .radius(4.0)
            .fill(tint.alpha(fill))
            .border(1.0, c.on_page(tint.alpha(look.ld(0.3, 0.24))))
            .child(title)
            .child(self.stack(blocks, inner, false).key("body").w_full())
    }

    /// Raw HTML, shown literally (BLK-76).
    fn html(&self, t: &str) -> El {
        let look = self.look;
        let b = look.b;
        row()
            .w_full()
            .items_stretch()
            .child(el().key("rule").w(1.0).fill(look.c.hairline))
            .child(text(t.to_string()).key("text").font(look.mono_at(r(b * 0.8), 1.5)).color(look.c.faint).wrap_anywhere().grow(1.0).min_w(0.0).ml(r(b * 0.9)))
    }

    /// An image's pixels, or the placeholder card (BLK-70, BLK-72). `size` is a gallery's shown
    /// size; without one the image keeps its natural size, capped at the column.
    fn picture(&self, img: &Img, size: Option<(f32, f32)>) -> El {
        let look = self.look;
        let s = self.v.s;
        let handle = match s.image_source(img) {
            crate::images::Source::File(path) => s.images.file(&path).map(Ok),
            crate::images::Source::Url(url) => match s.images.remote(&url) {
                crate::images::Remote::Ready(i) => Some(Ok(i)),
                crate::images::Remote::Loading => None,
                crate::images::Remote::Failed(e) => Some(Err(format!("Image not found · {url} ({e})"))),
            },
            crate::images::Source::Missing(src) => Some(Err(format!("Image not found · {src}"))),
        };
        let alt = if img.alt.is_empty() { "Image".to_string() } else { img.alt.clone() };
        match handle {
            Some(Ok(i)) if i.error().is_none() => {
                let mut pic = el().pixels(i.pixels()).radius(4.0).clip_rounded(4.0).role("img").label(alt.clone()).help(alt);
                match (size, i.size()) {
                    (Some((w, h)), _) => pic = pic.size(w, h),
                    (None, Some([w, h])) if w > 0 && h > 0 => {
                        let w_shown = (w as f32).min(look.width);
                        pic = pic.size(w_shown, w_shown * h as f32 / w as f32);
                    }
                    _ => pic = pic.w_full(),
                }
                pic
            }
            None => self.placeholder("Loading image…".into(), size),
            _ if img.src.is_empty() => self.placeholder("No image".into(), size),
            Some(Err(msg)) => self.placeholder(msg, size),
            Some(Ok(_)) => self.placeholder(format!("Image not found · {}", img.src), size),
        }
    }

    fn placeholder(&self, msg: String, size: Option<(f32, f32)>) -> El {
        let look = self.look;
        let b = look.b;
        let ts = r(b * 0.76);
        let card = row()
            .h(r(b * 3.4))
            .max_w(look.width)
            .px(r(b * 1.2))
            .gap(r(b * 0.5))
            .center_y()
            .radius(4.0)
            .fill(look.c.card)
            .border(1.0, look.c.on_page(look.c.hairline))
            .child(icon(&octicons::IMAGE, r(b * 1.05)).color(look.c.faint));
        let msg = text(msg).font(look.mono_at(ts, 1.3)).color(look.c.faint).min_w(0.0);
        match size {
            // A gallery cell: the message wraps inside the cell and never spills out of it.
            Some((w, h)) => card.size(w, h).px(r(b * 0.6)).items_start().pt(r(b * 0.6)).clip_rounded(4.0).child(msg.wrap_anywhere().grow(1.0)),
            None => card.child(msg.shrink()),
        }
    }

    /// The natural size of an image once it has loaded.
    fn natural(&self, img: &Img) -> Option<[f32; 2]> {
        let s = self.v.s;
        let i = match s.image_source(img) {
            crate::images::Source::File(path) => s.images.file(&path)?,
            crate::images::Source::Url(url) => match s.images.remote(&url) {
                crate::images::Remote::Ready(i) => i,
                _ => return None,
            },
            crate::images::Source::Missing(_) => return None,
        };
        i.size().filter(|[w, h]| *w > 0 && *h > 0).map(|[w, h]| [w as f32, h as f32])
    }

    /// An image block: the picture centred, its alt text as the caption (BLK-70, BLK-73).
    fn image(&self, img: &Img) -> El {
        let look = self.look;
        let b = look.b;
        let mut c = col().w_full().items_center().child(self.picture(img, None).key("pic"));
        if !img.alt.is_empty() {
            let size = r(b * 0.84);
            c = c.child(text(img.alt.clone()).key("caption").font(look.prose(size, 1.5).italic()).color(look.c.dim).wrap().w_full().align(rune::Align::Center).mt(r(b * 0.6)));
        }
        c
    }

    /// Images in centred, bottom-aligned rows (BLK-77, BLK-78).
    fn gallery(&self, items: &[GalleryItem]) -> El {
        let b = self.look.b;
        let css = b / 16.0;
        let cap = r(b * 16.0);
        let mut out = row().w_full().flex_wrap().justify_center().items_end().gap(r(b * 0.5));
        for (i, it) in items.iter().enumerate() {
            let nat = self.natural(&it.img);
            let aspect = nat.map_or(0.75, |[w, h]| w / h);
            let (mut w, mut h) = if it.h > 0.0 {
                (it.h * css * aspect, it.h * css)
            } else if it.w > 0.0 {
                (it.w * css, it.w * css / aspect)
            } else {
                let h = nat.map_or(r(b * 10.0), |[_, h]| h.min(cap));
                (h * aspect, h)
            };
            if w > self.look.width {
                h *= self.look.width / w;
                w = self.look.width;
            }
            let mut pic = self.picture(&it.img, Some((r(w), r(h)))).key(format!("img-{i}"));
            let tip = if it.title.is_empty() { &it.img.alt } else { &it.title };
            if !tip.is_empty() {
                pic = pic.help(tip.clone());
            }
            out = out.child(pic);
        }
        out
    }

    /// The notes under a short hairline, numbered, with back-links (BLK-80 to BLK-84).
    fn footnotes(&self, items: &[Footnote]) -> El {
        let look = self.look;
        let c = &look.c;
        let b = look.b;
        let size = r(b * 0.88);
        let lh = line(size, 1.6);
        let ns = r(size * 0.9);
        let tone = Tone { color: c.dim, size, lead: 1.6, contained: true, quote_depth: 0 };
        let mut out = col().w_full().gap(r(b * 0.55)).child(el().key("rule").w_full().h(r(b * 0.6)).child(el().w_frac(0.3).h(1.0).fill(c.hairline)));
        for f in items {
            let mut blocks = f.blocks.clone();
            backlinks(&mut blocks, f);
            let cite = self.anchor_line(&format!("fnref-{}-1", f.number));
            let mut num = text(format!("{}.", f.number))
                .key("num")
                .font(TextStyle::new(ns, 400.0).family(look.mono).lh(lh).features("'tnum' 1"))
                .color(c.link)
                .w(r(b * 1.9));
            if let Some(l) = cite.filter(|_| f.refs > 0) {
                num = num.cursor(rune::Cursor::Pointer).on_press_tx(move |s: &mut App, tx| s.jump(l, tx));
            }
            out = out.child(row().key(format!("fn-{}", f.number)).w_full().items_start().child(num).child(self.stack(&blocks, tone, false).key("body").grow(1.0).min_w(0.0)));
        }
        out
    }
}

/// " ↩" links back to each citation, on the note's last paragraph, numbered when the note is
/// cited more than once (BLK-84).
fn backlinks(blocks: &mut Vec<Block>, f: &Footnote) {
    let links: Vec<Run> = (1..=f.refs)
        .map(|k| {
            let t = if f.refs > 1 { format!(" ↩{k}") } else { " ↩".to_string() };
            Run { t, link: Some(Link::Anchor(format!("fnref-{}-{k}", f.number))), ..Run::default() }
        })
        .collect();
    if links.is_empty() {
        return;
    }
    match blocks.last_mut().map(|b| &mut b.kind) {
        Some(Kind::Paragraph { runs }) => runs.extend(links),
        _ => blocks.push(Block { kind: Kind::Paragraph { runs: links }, lines: 0..0, key: (0, 0), pid: None }),
    }
}

/// A paragraph of nothing but `#tags`.
fn is_tag_line(runs: &[Run]) -> bool {
    runs.iter().any(|r| matches!(r.link, Some(Link::Tag(_)))) && runs.iter().all(|r| matches!(r.link, Some(Link::Tag(_))) || (r.link.is_none() && r.t.trim().is_empty()))
}

/// Tags drawn as chips (BLK-101, PAGE-76), on the paragraph's baseline.
fn tag_line(cx: &Cx, runs: &[Run], tone: Tone) -> El {
    let look = cx.look;
    let size = r(look.b * 0.86);
    let lh = line(tone.size, tone.lead);
    let chip_h = r(size * 1.5);
    let top = (look.baseline(tone.size, lh) - look.mono_asc * size - (chip_h - (look.mono_asc + look.mono_desc) * size) / 2.0).max(0.0);
    let mut out = row().w_full().min_h(lh).flex_wrap().gap(r(tone.size * 0.35)).items_start();
    for (i, run) in runs.iter().enumerate() {
        let Some(Link::Tag(tag)) = &run.link else { continue };
        let tag = tag.clone();
        out = out.child(
            row()
                .key(format!("tag-{i}"))
                .mt(top)
                .h(chip_h)
                .center_y()
                .px(r(size * 0.25))
                .radius(3f32.max(r(chip_h * 0.22)))
                .fill(look.c.code_bg)
                .border(1.0, look.c.chip_edge)
                .cursor(rune::Cursor::Pointer)
                .role("link")
                .label(run.t.clone())
                .on_press_tx(move |s: &mut App, tx| s.search_tag(&tag, tx))
                .child(text(run.t.clone()).font(look.mono_at(size, 1.3)).color(look.c.accent)),
        );
    }
    out
}
