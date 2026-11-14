//! The preview: a note drawn as a page, as the design's `renderPreview` draws it.

use rune::{Color, El, Family, Link, Span, TextStyle, Tx, col, el, icon, rich, row, text};

use super::V;
use super::widgets::{CHECK, label};
use crate::app::App;
use crate::md::{self, Block, Tok};
use crate::roles::Roles;
use crate::theme::{self, Pal};

struct Look {
    serif: bool,
    size: f32,
    lh: f32,
    family: Family,
    mono: Family,
    /// The mono font's advance, in em.
    mono_w: f32,
    asc: f32,
    desc: f32,
    /// The page's colours (`crate::roles`), named by `behavior/blocks.md`.
    c: Roles,
}

impl Look {
    fn body(&self) -> TextStyle {
        TextStyle::new(self.size, 400.0).family(self.family).lh(self.size * self.lh)
    }
    /// Baseline below the top of a line box of this body text.
    fn baseline(&self, size: f32, lh: f32, asc: f32, desc: f32) -> f32 {
        (lh - (asc + desc) * size) / 2.0 + asc * size
    }
}

/// Rich text for one block: its string, spans, struck ranges and links.
#[derive(Default)]
struct Rich {
    text: String,
    spans: Vec<Span>,
    strikes: Vec<(usize, usize, Option<Color>)>,
    links: Vec<(std::ops::Range<usize>, String)>,
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    bold: bool,
    italic: bool,
}

impl Rich {
    fn push(&mut self, s: &str, cx: Ctx, color: Option<Color>) {
        let a = self.text.len();
        self.text.push_str(s);
        let b = self.text.len();
        if a < b && (cx.bold || cx.italic || color.is_some()) {
            self.spans.push(Span { weight: cx.bold.then_some(600.0), italic: cx.italic, color, ..Span::new(a, b) });
        }
    }

    /// The design's `inl()`: inline Markdown to styled runs.
    fn inline(&mut self, s: &str, cx: Ctx, p: &Pal, look: &Look, names: &md::Names, base_color: Option<Color>) {
        let mut last = 0;
        for (t, r) in md::inline_tokens(s) {
            // `![alt](src)` inside a sentence shows its alt text. Lines of images are blocks.
            let image = t == Tok::Link && r.start > last && s[..r.start].ends_with('!');
            let pre_end = if image { r.start - 1 } else { r.start };
            if pre_end > last {
                self.push(&s[last..pre_end], cx, base_color);
            }
            let tok = &s[r.clone()];
            match t {
                Tok::Code => {
                    // `padding: .12em .38em` at .8em: the background grows by the vertical
                    // padding, and spaces sized to the rest of the horizontal padding take room.
                    let size = look.size * 0.8;
                    self.chip(&tok[1..tok.len() - 1], size, look.mono_w, 0.12 * size, 0.38 * size, base_color.or(Some(look.c.code_text)), look.c.code_bg);
                }
                Tok::Wiki => {
                    let inner = &tok[2..tok.len() - 2];
                    let mut parts = inner.splitn(2, '|');
                    let target = parts.next().unwrap_or("").replace('\u{a0}', " ");
                    let alias = parts.next();
                    let shown = alias.unwrap_or(&target).trim().to_string();
                    let ok = names.resolve(&target).is_some();
                    let a = self.text.len();
                    self.text.push_str(&shown);
                    let b = self.text.len();
                    self.spans.push(Span {
                        weight: cx.bold.then_some(600.0),
                        italic: cx.italic,
                        color: Some(if ok { look.c.link } else { look.c.faint }),
                        underline: Some(if ok { look.c.link.alpha(0.4) } else { look.c.ghost }),
                        ..Span::new(a, b)
                    });
                    self.links.push((a..b, format!("wiki:{}", target.trim())));
                }
                Tok::Bold => self.inline(&tok[2..tok.len() - 2], Ctx { bold: true, ..cx }, p, look, names, base_color),
                Tok::Italic => self.inline(&tok[1..tok.len() - 1], Ctx { italic: true, ..cx }, p, look, names, base_color),
                Tok::Strike => {
                    let a = self.text.len();
                    self.push(&tok[2..tok.len() - 2], cx, Some(look.c.dim));
                    let b = self.text.len();
                    self.strikes.push((a, b, Some(look.c.dim)));
                }
                Tok::Link if image => {
                    let j = tok.find("](").unwrap_or(tok.len());
                    let a = self.text.len();
                    self.text.push_str(if j > 1 { &tok[1..j] } else { "image" });
                    let b = self.text.len();
                    self.spans.push(Span { color: Some(look.c.dim), italic: true, ..Span::new(a, b) });
                }
                Tok::Link => {
                    let j = tok.find("](").unwrap_or(tok.len());
                    let url = tok[j + 2..tok.len() - 1].to_string();
                    let a = self.text.len();
                    self.text.push_str(&tok[1..j]);
                    let b = self.text.len();
                    self.spans.push(Span { color: Some(look.c.link), underline: Some(look.c.link), weight: cx.bold.then_some(600.0), italic: cx.italic, ..Span::new(a, b) });
                    self.links.push((a..b, format!("url:{url}")));
                }
                Tok::Tag => {
                    let size = look.size * 0.72;
                    let a = self.text.len();
                    self.chip(tok, size, look.mono_w, 0.18 * size, 0.45 * size, Some(p.acc), p.soft);
                    let b = self.text.len();
                    self.links.push((a..b, format!("tag:{}", &tok[1..])));
                }
            }
            last = r.end;
        }
        if last < s.len() {
            self.push(&s[last..], cx, base_color);
        }
    }

    /// A run with a padded background, as CSS draws inline code and tags.
    fn chip(&mut self, t: &str, size: f32, mono_w: f32, pad_y: f32, pad_x: f32, color: Option<Color>, bg: Color) {
        // A mono space is `mono_w` em wide, so a space at this size is exactly the padding left
        // over after the background's own outset.
        let side = (pad_x - pad_y).max(0.0);
        let space = side / mono_w;
        let a = self.text.len();
        self.text.push(' ');
        let b = self.text.len();
        self.text.push_str(t);
        let c = self.text.len();
        self.text.push(' ');
        let d = self.text.len();
        if space > 0.5 {
            self.spans.push(Span { mono: true, size: Some(space), bg: Some(bg), bg_pad: pad_y, ..Span::new(a, b) });
            self.spans.push(Span { mono: true, size: Some(space), bg: Some(bg), bg_pad: pad_y, ..Span::new(c, d) });
        }
        self.spans.push(Span { mono: true, size: Some(size), color, bg: Some(bg), bg_pad: pad_y, ..Span::new(b, c) });
    }

    fn el(self, style: TextStyle, color: Color) -> El {
        let mut e = rich(if self.text.is_empty() { " ".to_string() } else { self.text }, self.spans).font(style).color(color).wrap().span_radius(4.0);
        for (a, b, c) in self.strikes {
            e = e.strike(a, b, c);
        }
        for (r, id) in self.links {
            e = e.link(Link::new(r, id));
        }
        e.on_link(on_link)
    }
}

fn on_link(s: &mut App, id: &str, tx: &mut Tx) {
    if let Some(t) = id.strip_prefix("wiki:") {
        s.open_link(t, tx);
    } else if let Some(t) = id.strip_prefix("tag:") {
        s.search_tag(t);
    } else if let Some(u) = id.strip_prefix("url:") {
        if let Err(e) = tx.open_url(u) {
            s.notice = Some(format!("Could not open {u}: {e}"));
        }
    }
}

pub fn preview(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let set = s.settings();
    let prose = theme::pick(&theme::PROSE_FONTS, &set.preview_font);
    let size = set.size as f32;
    let (asc, desc) = theme::metrics(prose.family);
    let look = Look {
        serif: prose.family == theme::SERIF,
        size,
        lh: 1.6,
        family: prose.family,
        mono: theme::pick(&theme::CODE_FONTS, &set.code_font).family,
        mono_w: 0.6,
        asc,
        desc,
        c: Roles::new(p),
    };
    let em = size;
    let names = &s.names;
    let doc = s.active_text();
    let blocks = md::blocks(doc);
    // Collapsed top margin left after the breadcrumb's 30 px bottom margin.
    let mut prev_mb: f32 = if v.focus() { 0.0 } else { 30.0 };
    let mut first = true;
    let mut out = col().key("preview").w_full();
    for b in blocks {
        let (mt, mb, el_) = block(v, &look, names, b);
        let gap = if first { (mt - prev_mb).max(0.0) } else { mt.max(prev_mb) };
        first = false;
        out = out.child(el_.mt(gap));
        prev_mb = mb;
    }
    let _ = (em, p);
    out
}

/// One block with its CSS top and bottom margins (collapsed by the caller).
fn block(v: V, look: &Look, names: &md::Names, b: Block) -> (f32, f32, El) {
    let p = v.p;
    let em = look.size;
    let body = look.body();
    match b {
        Block::Image { line, images } => {
            let mut c = col().key(format!("pv-{line}")).w_full().gap(em * 0.6);
            for (i, img) in images.iter().enumerate() {
                c = c.child(picture(v, img, look).key(format!("img-{i}")));
            }
            (em * 0.4, em * 0.95, c)
        }
        Block::Pre { line, text: t } => {
            let style = TextStyle::new((em * 0.74).round(), 400.0).family(look.mono).lh((em * 0.74).round() * 1.65);
            let e = el()
                .key(format!("pv-{line}"))
                .w_full()
                .radius(4.0)
                .fill(look.c.panel)
                .border(1.0, look.c.hairline_soft)
                .scroll_x()
                .child(el().py(14.0).px(18.0).child(text(if t.is_empty() { " ".into() } else { t }).font(style).color(look.c.text)));
            (em * 1.2, em * 1.3, e)
        }
        Block::Heading { line, level, index, text: t } => {
            let (k, lh, w, mt, mb, track) = match level {
                1 => (1.72, 1.15, if look.serif { 500.0 } else { 600.0 }, 0.0, 0.7, -0.012),
                2 => (1.3, 1.25, 600.0, 1.7, 0.55, 0.0),
                _ => (1.08, 1.3, 600.0, 1.5, 0.45, 0.0),
            };
            let size = em * k;
            let style = TextStyle::new(size, w).family(look.family).lh(size * lh).tracking(size * track);
            let mut r = Rich::default();
            r.inline(&t, Ctx::default(), p, look, names, None);
            let e = r.el(style, if level <= 4 { look.c.strong } else { look.c.dim }).key(format!("pv-{line}")).w_full().role("heading").level(level as u32).label(t.clone());
            let _ = index;
            (size * mt, size * mb, e)
        }
        Block::Rule => (em * 2.2, em * 2.2, el().h(1.0).w_full().fill(look.c.hairline)),
        Block::Quote { line, text: t } => {
            let mut r = Rich::default();
            r.inline(&md::pretty(&t), Ctx { italic: true, bold: false }, p, look, names, None);
            let e = el()
                .key(format!("pv-{line}"))
                .w_full()
                .pt(em * 0.1)
                .pb(em * 0.1)
                .pl(em * 1.1 + 2.0)
                .border_sides([0.0, 0.0, 0.0, 2.0], look.c.accent)
                .child(r.el(body.italic(), look.c.dim).w_full());
            (em * 1.3, em * 1.3, e)
        }
        Block::List { line, items } => {
            let mut c = col().key(format!("pv-{line}")).w_full().role("list");
            let gap = em * 0.25;
            let text_base = look.baseline(em, em * look.lh, look.asc, look.desc);
            for (i, it) in items.into_iter().enumerate() {
                let mut r = Rich::default();
                let base_color = if it.done { Some(look.c.dim) } else { None };
                r.inline(&md::pretty(&it.text), Ctx::default(), p, look, names, base_color);
                let n = r.text.len();
                let mut te = r.el(body, look.c.text).grow(1.0).shrink().min_w(0.0);
                if it.done {
                    te = te.strike(0, n, Some(look.c.faint));
                }
                let marker = if it.task {
                    let sz = em * 0.85;
                    let top = text_base - sz + em * 0.1;
                    let line_no = it.line;
                    let mut bx = row()
                        .key("box")
                        .size(sz, sz)
                        .mt(top)
                        .center()
                        .radius(3.0)
                        .border(1.5, if it.done { look.c.accent } else { look.c.faint })
                        .role("checkbox")
                        .checked(it.done)
                        .cursor(rune::Cursor::Pointer)
                        .on_press(move |s: &mut App| s.toggle_task(line_no));
                    if it.done {
                        bx = bx.fill(look.c.accent).child(icon(&CHECK, 9.0).color(look.c.bg));
                    }
                    bx
                } else if it.marker.as_bytes()[0].is_ascii_digit() {
                    let ns = em * 0.78;
                    let nlh = ns * look.lh;
                    let shift = text_base - look.baseline(ns, nlh, 1.025, 0.275);
                    el().key("num").min_w(ns * 1.3).mt(shift).child(text(it.marker.clone()).font(TextStyle::new(ns, 400.0).family(look.mono).lh(nlh)).color(look.c.faint))
                } else {
                    row().key("bullet").w(em).justify_center().child(text("•").font(body).color(look.c.dim))
                };
                let li = row()
                    .key(format!("li-{}", it.line))
                    .w_full()
                    .gap(em * 0.65)
                    .ml((it.indent / 2) as f32 * 1.5 * em)
                    .mt(if i == 0 { 0.0 } else { gap })
                    .child(marker)
                    .child(te);
                c = c.child(li);
            }
            (gap, em, c)
        }
        Block::Para { line, text: t } if is_tag_line(&t) => (0.0, em * 0.95, tag_line(v, look, line, &t)),
        Block::Para { line, text: t } => {
            let mut r = Rich::default();
            r.inline(&md::pretty(&t), Ctx::default(), p, look, names, None);
            (0.0, em * 0.95, r.el(body, look.c.text).key(format!("pv-{line}")).w_full())
        }
    }
}

/// One image at its own size, or narrower to fit the column. A box holds its place while it
/// loads, and a line says so when there is no file.
fn picture(v: V, img: &md::Img, look: &Look) -> El {
    let s = v.s;
    let note = |msg: String| {
        row()
            .w_full()
            .px(14.0)
            .py(10.0)
            .radius(6.0)
            .fill(look.c.card)
            .child(label(msg, theme::sans(look.size * 0.8, 400.0), look.c.dim).wrap().grow(1.0).shrink())
    };
    let handle = match s.image_source(img) {
        crate::images::Source::File(path) => s.images.file(&path).map(Ok),
        crate::images::Source::Url(url) => match s.images.remote(&url) {
            crate::images::Remote::Ready(i) => Some(Ok(i)),
            crate::images::Remote::Loading => None,
            crate::images::Remote::Failed(e) => Some(Err(format!("Could not load {url}: {e}"))),
        },
        crate::images::Source::Missing(src) => Some(Err(format!("No image at {src}"))),
    };
    let alt = if img.alt.is_empty() { "Image".to_string() } else { img.alt.clone() };
    match handle {
        Some(Ok(i)) if i.error().is_some() => note(format!("Could not show {}: {}", img.src, i.error().map_or("", |e| e.0.as_str()))),
        Some(Ok(i)) => {
            // A pending image paints when its pixels arrive. Its own width caps the picture.
            let mut pic = el().pixels(i.pixels()).w_full().radius(6.0).clip_rounded(6.0).role("img").label(alt.clone()).help(alt);
            if let Some([w, _]) = i.size() {
                pic = pic.max_w(w as f32);
            }
            row().w_full().child(pic)
        }
        Some(Err(msg)) => note(msg),
        _ => el().w_full().h(look.size * 6.0).radius(6.0).fill(look.c.card).role("img").label(alt),
    }
}

/// A paragraph of nothing but `#tags`.
fn is_tag_line(t: &str) -> bool {
    let words: Vec<&str> = t.split_whitespace().collect();
    !words.is_empty() && words.iter().all(|w| md::inline_tokens(w).first().is_some_and(|(k, r)| *k == Tok::Tag && r.start == 0 && r.end == w.len()))
}

/// Tags drawn as chips with CSS's exact padding, on the paragraph's baseline.
fn tag_line(v: V, look: &Look, line: usize, t: &str) -> El {
    let p = v.p;
    let em = look.size;
    let lh = em * look.lh;
    let (a, d) = (theme::px(look.asc * em), theme::px(look.desc * em));
    let base = (lh - a - d) / 2.0 + a;
    let size = em * 0.72;
    let (ma, md_) = theme::metrics(look.mono);
    let (ca, cd) = (theme::px(ma * size), theme::px(md_ * size));
    let pad_y = 0.18 * size;
    let space = em * 0.25;
    let mut r = row().key(format!("pv-{line}")).w_full().h(lh).flex_wrap().gap(space).items_start();
    for (i, w) in t.split_whitespace().enumerate() {
        let tag = w[1..].to_string();
        r = r.child(
            row()
                .key(format!("tag-{i}"))
                .mt(base - ca - pad_y)
                .py(pad_y)
                .px(0.45 * size)
                .radius(4.0)
                .fill(p.soft)
                .cursor(rune::Cursor::Pointer)
                .role("link")
                .label(w.to_string())
                .on_press(move |s: &mut App| s.search_tag(&tag))
                .child(text(w).font(TextStyle::new(size, 400.0).family(look.mono).lh(ca + cd)).color(p.acc)),
        );
    }
    r
}


