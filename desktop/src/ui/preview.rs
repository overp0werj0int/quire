//! The preview: a note drawn as a page from `crate::blocks`, as omarky's reader draws it.

use rune::{Color, El, Family, Span, TextStyle, Tx, col, el, icon, rich, row, text};

use super::V;
use super::widgets::{CHECK, label};
use crate::app::App;
use crate::blocks::{self as bk, Alert, Align, Block, DiffKind, Doc, FieldKind, Kind, Link, Run, Task};
use crate::md;
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

/// How a container sets the text inside it: a quote's dim italic, a done task's strike.
#[derive(Clone, Copy)]
struct Tone {
    color: Color,
    italic: bool,
    strike: bool,
}

impl Rich {
    fn push(&mut self, s: &str, sp: Span) {
        let a = self.text.len();
        self.text.push_str(s);
        let b = self.text.len();
        if a < b && (sp.weight.is_some() || sp.italic || sp.color.is_some()) {
            self.spans.push(Span { start: a as u32, end: b as u32, ..sp });
        }
    }

    /// Runs (`crate::blocks::Run`) to styled text.
    fn runs(&mut self, runs: &[Run], p: &Pal, look: &Look, names: &md::Names, doc: &Doc) {
        for r in runs {
            let base = Span { weight: r.b.then_some(600.0), italic: r.i, color: r.b.then_some(look.c.strong), ..Span::default() };
            let a = self.text.len();
            if r.br {
                self.text.push('\n');
                continue;
            }
            if r.code {
                // `padding: .12em .38em` at .8em: the background grows by the vertical
                // padding, and spaces sized to the rest of the horizontal padding take room.
                let size = look.size * 0.8;
                self.chip(&r.t, size, look.mono_w, 0.12 * size, 0.38 * size, Some(look.c.code_text), look.c.code_bg);
                continue;
            }
            if r.raw {
                self.spans.push(Span { mono: true, size: Some(look.size * 0.8), color: Some(look.c.dim), ..Span::new(a, a + r.t.len()) });
                self.text.push_str(&r.t);
                continue;
            }
            match &r.link {
                Some(Link::Tag(tag)) => {
                    let size = look.size * 0.72;
                    self.chip(&r.t, size, look.mono_w, 0.18 * size, 0.45 * size, Some(p.acc), p.soft);
                    let b = self.text.len();
                    self.links.push((a..b, format!("tag:{tag}")));
                    continue;
                }
                Some(Link::Wiki(target)) => {
                    let ok = names.resolve(target).is_some();
                    let color = if ok { look.c.link } else { look.c.faint };
                    self.push(&r.t, Span { color: Some(color), underline: Some(if ok { look.c.link.alpha(0.4) } else { look.c.ghost }), ..base });
                    self.links.push((a..self.text.len(), format!("wiki:{}", target.trim())));
                }
                Some(Link::Url(u)) if r.img.is_none() => {
                    self.push(&r.t, Span { color: Some(look.c.link), underline: Some(look.c.link), ..base });
                    self.links.push((a..self.text.len(), format!("url:{u}")));
                }
                Some(Link::Anchor(an)) => {
                    let line = doc.anchors.get(an).and_then(|&i| doc.blocks.get(i)).map(|b| b.lines.start);
                    let size = if r.note { Some(look.size * 0.7) } else { None };
                    self.push(&r.t, Span { color: Some(look.c.link), size, ..base });
                    if let Some(l) = line {
                        self.links.push((a..self.text.len(), format!("line:{l}")));
                    }
                }
                // An image inside a sentence shows its alt text.
                _ if r.img.is_some() => self.push(&r.t, Span { color: Some(look.c.dim), italic: true, ..base }),
                _ => self.push(&r.t, base),
            }
            if r.s {
                let b = self.text.len();
                self.spans.push(Span { color: Some(look.c.dim), ..Span::new(a, b) });
                self.strikes.push((a, b, Some(look.c.dim)));
            }
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
            e = e.link(rune::Link::new(r, id));
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
    let doc = bk::build(s.active_text());
    let cx = Cx { v, look: &look, doc: &doc };
    // Collapsed top margin left after the breadcrumb's 30 px bottom margin.
    let prev = if v.focus() { 0.0 } else { 30.0 };
    cx.stack(&doc.blocks, cx.plain(), prev, true).key("preview").w_full()
}

/// What every block needs: the view, the page's look and the whole document (for anchors).
#[derive(Clone, Copy)]
struct Cx<'a> {
    v: V<'a>,
    look: &'a Look,
    doc: &'a Doc,
}

impl Cx<'_> {
    fn plain(&self) -> Tone {
        Tone { color: self.look.c.text, italic: false, strike: false }
    }

    fn rich(&self, runs: &[Run]) -> Rich {
        let mut r = Rich::default();
        r.runs(runs, self.v.p, self.look, &self.v.s.names, self.doc);
        r
    }

    /// Runs as one wrapped text in the container's tone.
    fn para(&self, runs: &[Run], style: TextStyle, tone: Tone) -> El {
        let r = self.rich(runs);
        let n = r.text.len();
        let mut e = r.el(if tone.italic { style.italic() } else { style }, tone.color);
        if tone.strike {
            e = e.strike(0, n, Some(self.look.c.faint));
        }
        e
    }

    /// Blocks one under another with collapsed CSS margins. `top` keys each block by its
    /// first line so the outline and links can scroll to it.
    fn stack(&self, blocks: &[Block], tone: Tone, prev_mb: f32, top: bool) -> El {
        let mut prev = prev_mb;
        let mut out = col().w_full();
        for (i, b) in blocks.iter().enumerate() {
            let (mt, mb, e) = self.block(b, tone);
            let gap = if i == 0 { (mt - prev).max(0.0) } else { mt.max(prev) };
            let key = if top { format!("pv-{}", b.lines.start) } else { format!("b-{i}") };
            out = out.child(e.key(key).mt(gap));
            prev = mb;
        }
        out
    }

    /// One block with its CSS top and bottom margins (collapsed by `stack`).
    fn block(&self, b: &Block, tone: Tone) -> (f32, f32, El) {
        let look = self.look;
        let c = &look.c;
        let em = look.size;
        let body = look.body();
        match &b.kind {
            Kind::FrontMatter { title, fields } => (0.0, em * 1.4, self.front_matter(title, fields)),
            Kind::Heading { level, runs, text: t, .. } => {
                let level = *level;
                let (k, lh, w, mt, mb, track) = match level {
                    1 => (1.72, 1.15, if look.serif { 500.0 } else { 600.0 }, 0.0, 0.7, -0.012),
                    2 => (1.3, 1.25, 600.0, 1.7, 0.55, 0.0),
                    _ => (1.08, 1.3, 600.0, 1.5, 0.45, 0.0),
                };
                let size = em * k;
                let style = TextStyle::new(size, w).family(look.family).lh(size * lh).tracking(size * track);
                let color = if level <= 4 { c.strong } else { c.dim };
                let e = self.para(runs, style, Tone { color, ..tone }).w_full().role("heading").level(level as u32).label(t.clone());
                (size * mt, size * mb, e)
            }
            Kind::Paragraph { runs } if is_tag_line(runs) => (0.0, em * 0.95, tag_line(self.v, look, runs)),
            Kind::Paragraph { runs } => (0.0, em * 0.95, self.para(runs, body, tone).w_full()),
            Kind::List(l) => (if l.depth == 0 { 0.0 } else { em * 0.25 }, if l.depth == 0 { em } else { 0.0 }, self.list(l, tone)),
            Kind::Quote { blocks } => {
                let inner = self.stack(blocks, Tone { color: c.dim, italic: true, ..tone }, 0.0, false);
                let e = el().w_full().py(em * 0.1).pl(em * 1.1 + 2.0).border_sides([0.0, 0.0, 0.0, 2.0], c.accent).child(inner);
                (em * 1.3, em * 1.3, e)
            }
            Kind::Alert { kind, blocks } => {
                let color = match kind {
                    Alert::Note => c.blue,
                    Alert::Tip => c.green,
                    Alert::Important => c.magenta,
                    Alert::Warning => c.yellow,
                    Alert::Caution => c.red,
                };
                let title = text(kind.title()).font(TextStyle::new(em * 0.8, 600.0).family(look.family).lh(em * 1.4)).color(color);
                let inner = self.stack(blocks, tone, 0.0, false);
                let e = col()
                    .w_full()
                    .gap(em * 0.3)
                    .py(em * 0.5)
                    .pl(em * 1.1 + 2.0)
                    .pr(em)
                    .radius(4.0)
                    .fill(color.alpha(0.06))
                    .border_sides([0.0, 0.0, 0.0, 2.0], color)
                    .child(title)
                    .child(inner);
                (em * 1.3, em * 1.3, e)
            }
            Kind::Code { text: t, .. } => (em * 1.2, em * 1.3, self.code(t.split('\n').map(|l| (l.to_string(), c.code_text)).collect())),
            Kind::Diff { lines, .. } => {
                let ls = lines
                    .iter()
                    .map(|l| {
                        let color = match l.kind {
                            DiffKind::Add => c.green,
                            DiffKind::Del => c.red,
                            DiffKind::Hunk => c.cyan,
                            DiffKind::Meta => c.faint,
                            DiffKind::Ctx => c.code_text,
                        };
                        (format!("{}{}", l.sign, l.text), color)
                    })
                    .collect();
                (em * 1.2, em * 1.3, self.code(ls))
            }
            Kind::Table(t) => (em * 1.2, em * 1.3, self.table(t, tone)),
            Kind::Rule => {
                let dot = || el().size(3.0, 3.0).radius(1.5).fill(c.faint);
                (em * 2.2, em * 2.2, row().w_full().justify_center().gap(em * 0.9).child(dot()).child(dot()).child(dot()))
            }
            Kind::Image { img, .. } => (em * 0.4, em * 0.95, picture(self.v, img, look)),
            Kind::Gallery { items } => {
                let mut r = row().w_full().flex_wrap().gap(em * 0.6);
                for (i, it) in items.iter().enumerate() {
                    let w = if it.w > 0.0 { it.w } else { 240.0 };
                    r = r.child(el().key(format!("img-{i}")).max_w(w).grow(1.0).min_w(0.0).child(picture(self.v, &it.img, look)));
                }
                (em * 0.4, em * 0.95, r)
            }
            Kind::Html { text: t } => {
                let style = TextStyle::new((em * 0.8).round(), 400.0).family(look.mono).lh((em * 0.8).round() * 1.5);
                (0.0, em * 0.95, text(t.clone()).font(style).color(c.dim).wrap().w_full())
            }
            Kind::Footnotes { items } => (em * 2.0, 0.0, self.footnotes(items, tone)),
        }
    }

    fn list(&self, l: &bk::List, tone: Tone) -> El {
        let look = self.look;
        let c = &look.c;
        let em = look.size;
        let body = look.body();
        let text_base = look.baseline(em, em * look.lh, look.asc, look.desc);
        let gap = if l.tight { em * 0.25 } else { em * 0.6 };
        let mut out = col().w_full().role("list");
        for (i, it) in l.items.iter().enumerate() {
            let done = it.task == Task::Done;
            let marker = match it.task {
                Task::None if l.ordered => {
                    let ns = em * 0.78;
                    let nlh = ns * look.lh;
                    let shift = text_base - look.baseline(ns, nlh, 1.025, 0.275);
                    el().key("num").min_w(ns * 1.3 + 2.0).mt(shift).child(text(format!("{}.", it.number)).font(TextStyle::new(ns, 400.0).family(look.mono).lh(nlh)).color(c.faint))
                }
                Task::None => {
                    let bullet = ["•", "◦", "▪"][l.depth.min(2)];
                    row().key("bullet").w(em).justify_center().child(text(bullet).font(body).color(c.dim))
                }
                _ => {
                    let sz = em * 0.85;
                    let top = text_base - sz + em * 0.1;
                    let line_no = it.line;
                    let mut bx = row()
                        .key("box")
                        .size(sz, sz)
                        .mt(top)
                        .center()
                        .radius(3.0)
                        .border(1.5, if done { c.accent } else { c.faint })
                        .role("checkbox")
                        .checked(done)
                        .cursor(rune::Cursor::Pointer)
                        .on_press(move |s: &mut App| s.toggle_task(line_no, !done));
                    if done {
                        bx = bx.fill(c.accent).child(icon(&CHECK, 9.0).color(c.bg));
                    }
                    bx
                }
            };
            let t = if done { Tone { color: c.dim, strike: true, ..tone } } else { tone };
            let mut inner = col().w_full().grow(1.0).shrink().min_w(0.0);
            let mut prev = 0.0;
            for (j, b) in it.blocks.iter().enumerate() {
                // Inside an item, paragraphs sit tight; a loose list spaces them like the page.
                let (mt, mb, e) = self.block(b, t);
                let mb = if l.tight && matches!(b.kind, Kind::Paragraph { .. }) { 0.0 } else { mb };
                let g = if j == 0 { 0.0 } else { mt.max(prev) };
                inner = inner.child(e.key(format!("b-{j}")).mt(g));
                prev = mb;
            }
            let li = row().key(format!("li-{}", it.line)).w_full().gap(em * 0.65).mt(if i == 0 { 0.0 } else { gap }).child(marker).child(inner);
            out = out.child(li);
        }
        if l.depth > 0 { out.pl(em * 0.2) } else { out }
    }

    /// Monospace lines on a panel: code, or a diff with each line in its colour.
    fn code(&self, lines: Vec<(String, Color)>) -> El {
        let look = self.look;
        let size = (look.size * 0.74).round();
        let style = TextStyle::new(size, 400.0).family(look.mono).lh(size * 1.65);
        let mut c = col();
        for (i, (l, color)) in lines.into_iter().enumerate() {
            c = c.child(text(if l.is_empty() { " ".into() } else { l }).key(format!("l-{i}")).font(style).color(color));
        }
        el().w_full().radius(4.0).fill(look.c.panel).border(1.0, look.c.hairline_soft).scroll_x().child(el().py(14.0).px(18.0).child(c))
    }

    fn table(&self, t: &bk::Table, tone: Tone) -> El {
        let look = self.look;
        let c = &look.c;
        let size = look.size * 0.9;
        let style = TextStyle::new(size, 400.0).family(look.family).lh(size * 1.5);
        let cell = |runs: &[Run], col_i: usize, head: bool| {
            let numeric = t.numeric.get(col_i).copied().unwrap_or(false);
            let align = match t.aligns.get(col_i) {
                Some(Align::Right) => rune::Align::End,
                Some(Align::Center) => rune::Align::Center,
                Some(Align::Left) => rune::Align::Start,
                _ if numeric => rune::Align::End,
                _ => rune::Align::Start,
            };
            let st = if head { TextStyle { weight: 600.0, ..style } } else { style };
            let e = self.para(runs, st, Tone { color: if head { c.strong } else { tone.color }, ..tone }).align(align).w_full();
            el().key(format!("c-{col_i}")).w_frac(1.0 / t.aligns.len().max(1) as f32).min_w(0.0).px(size * 0.6).py(size * 0.35).child(e)
        };
        let mut out = col().w_full();
        let mut head = row().key("head").w_full().border_sides([0.0, 0.0, 1.0, 0.0], c.hairline);
        for (i, h) in t.header.iter().enumerate() {
            head = head.child(cell(h, i, true));
        }
        out = out.child(head);
        for (ri, r) in t.rows.iter().enumerate() {
            let mut rw = row().key(format!("r-{ri}")).w_full().border_sides([0.0, 0.0, 1.0, 0.0], c.hairline_soft);
            for (i, x) in r.iter().enumerate() {
                rw = rw.child(cell(x, i, false));
            }
            out = out.child(rw);
        }
        out
    }

    fn footnotes(&self, items: &[bk::Footnote], tone: Tone) -> El {
        let look = self.look;
        let c = &look.c;
        let em = look.size;
        let size = em * 0.85;
        let mut out = col().w_full().gap(em * 0.4).pt(em).border_sides([1.0, 0.0, 0.0, 0.0], c.hairline);
        for f in items {
            let num = text(format!("{}.", f.number)).font(TextStyle::new(size, 400.0).family(look.mono).lh(size * 1.6)).color(c.faint);
            let inner = self.stack(&f.blocks, Tone { color: c.dim, ..tone }, 0.0, false);
            out = out.child(row().key(format!("fn-{}", f.number)).w_full().gap(em * 0.5).child(el().min_w(em * 1.5).child(num)).child(el().grow(1.0).min_w(0.0).child(inner)));
        }
        out
    }

    /// The front matter as a card: the title, then each field on its own line.
    fn front_matter(&self, title: &str, fields: &[bk::Field]) -> El {
        let look = self.look;
        let c = &look.c;
        let em = look.size;
        let small = TextStyle::new(em * 0.8, 400.0).family(look.family).lh(em * 1.3);
        let mut card = col().w_full().gap(em * 0.35).px(em).py(em * 0.8).radius(6.0).fill(c.card).border(1.0, c.hairline_soft);
        if !title.is_empty() {
            card = card.child(text(title.to_string()).key("title").font(TextStyle::new(em * 1.1, 600.0).family(look.family).lh(em * 1.5)).color(c.strong).wrap());
        }
        for (i, f) in fields.iter().enumerate() {
            let key = text(f.key.clone()).font(small).color(c.faint);
            let value: El = match f.kind {
                FieldKind::Pill | FieldKind::List => {
                    let vals: Vec<String> = if f.items.is_empty() { vec![f.value.clone()] } else { f.items.clone() };
                    let mut r = row().flex_wrap().gap(em * 0.3);
                    for (j, v) in vals.into_iter().enumerate() {
                        r = r.child(el().key(format!("v-{j}")).px(em * 0.4).py(em * 0.1).radius(4.0).fill(c.chip_fill).child(text(v).font(small).color(c.dim)));
                    }
                    r
                }
                FieldKind::Link => text(f.value.clone()).font(small).color(c.link).wrap().w_full(),
                FieldKind::Empty => text("—").font(small).color(c.ghost),
                FieldKind::Text => text(f.value.clone()).font(small).color(c.text).wrap().w_full(),
            };
            card = card.child(row().key(format!("f-{i}")).w_full().gap(em * 0.8).items_start().child(el().w(em * 7.0).child(key)).child(el().grow(1.0).min_w(0.0).child(value)));
        }
        card
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
fn is_tag_line(runs: &[Run]) -> bool {
    runs.iter().any(|r| matches!(r.link, Some(Link::Tag(_)))) && runs.iter().all(|r| matches!(r.link, Some(Link::Tag(_))) || (r.link.is_none() && r.t.trim().is_empty()))
}

/// Tags drawn as chips with CSS's exact padding, on the paragraph's baseline.
fn tag_line(v: V, look: &Look, runs: &[Run]) -> El {
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
    let mut r = row().w_full().h(lh).flex_wrap().gap(space).items_start();
    for (i, run) in runs.iter().enumerate() {
        let Some(Link::Tag(tag)) = &run.link else { continue };
        let tag = tag.clone();
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
                .label(run.t.clone())
                .on_press(move |s: &mut App| s.search_tag(&tag))
                .child(text(run.t.clone()).font(TextStyle::new(size, 400.0).family(look.mono).lh(ca + cd)).color(p.acc)),
        );
    }
    r
}
