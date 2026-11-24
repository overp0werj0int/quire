//! A note as a flat list of blocks (`behavior/blocks.md`, section 1), parsed with pulldown-cmark.
//!
//! The model and its rules follow omarky's `lib/blocks.js`: every top-level block knows its source
//! lines; lists, quotes, alerts and footnotes hold their child blocks; inline text is a list of
//! styled runs whose plain text is exactly what the reader sees. Front matter is cut out first
//! with its lines blanked, so every later block keeps its true line numbers (BLK-04).

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::md::{self, Img};

/// One stretch of inline text with one style (omarky's run).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub t: String,
    pub b: bool,
    pub i: bool,
    pub s: bool,
    pub code: bool,
    /// Raw inline HTML, shown literally (PAGE-80).
    pub raw: bool,
    /// A hard break: `t` is "\n".
    pub br: bool,
    /// A soft break: `t` is " ".
    pub soft: bool,
    /// A footnote citation, its number in `t`.
    pub note: bool,
    /// The anchor this run is, `fnref-N-M` for a citation.
    pub anchor: Option<String>,
    /// An inline image's source; `t` is its alt text.
    pub img: Option<String>,
    pub link: Option<Link>,
}

impl Run {
    fn text(t: impl Into<String>) -> Run {
        Run { t: t.into(), ..Run::default() }
    }
    /// Plain prose: no code, HTML, break, citation, image or link, so typographer, autolinks
    /// and tags may touch it.
    fn plain(&self) -> bool {
        !self.code && !self.raw && !self.br && !self.soft && !self.note && self.img.is_none() && self.anchor.is_none() && self.link.is_none()
    }
    fn same_style(&self, o: &Run) -> bool {
        self.b == o.b && self.i == o.i && self.s == o.s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Url(String),
    /// `#fn-N` and other anchors in the note.
    Anchor(String),
    /// `[[target]]`, Quire's note link (BLK-101).
    Wiki(String),
    /// `#tag`, Quire's tag (BLK-101).
    Tag(String),
}

/// The text the reader sees in `runs`.
pub fn plain(runs: &[Run]) -> String {
    runs.iter().map(|r| r.t.as_str()).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: Kind,
    /// Source lines, 0-based, end exclusive.
    pub lines: Range<usize>,
    /// A hash of the block's source lines and its ordinal among equal ones, so a block keeps its
    /// key across edits elsewhere and reloads. Set on top-level blocks only.
    pub key: (u64, u32),
    /// The block's find piece (`Doc::pieces`), for paragraphs, headings, code and diffs.
    pub pid: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    FrontMatter { title: String, fields: Vec<Field> },
    Heading { level: usize, runs: Vec<Run>, text: String, slug: String },
    Paragraph { runs: Vec<Run> },
    List(List),
    Quote { blocks: Vec<Block> },
    Alert { kind: Alert, blocks: Vec<Block> },
    Code { lang: String, text: String },
    Diff { lang: String, text: String, lines: Vec<DiffLine>, added: usize, removed: usize },
    Table(Table),
    Rule,
    Image { img: Img, title: String },
    /// HTML that is only `<img>` tags, or a paragraph of only images (BLK-101).
    Gallery { items: Vec<GalleryItem> },
    /// Raw HTML, shown literally. Comments are dropped.
    Html { text: String },
    Footnotes { items: Vec<Footnote> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub kind: FieldKind,
    pub value: String,
    pub items: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Pill,
    List,
    Link,
    Text,
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct List {
    pub ordered: bool,
    pub start: u64,
    pub tight: bool,
    pub depth: usize,
    pub tasks: bool,
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    None,
    Todo,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// The item's own number, or start + position.
    pub number: u64,
    pub task: Task,
    /// The source line of the item's marker.
    pub line: usize,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alert {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl Alert {
    fn parse(s: &str) -> Option<Alert> {
        Some(match s.to_ascii_uppercase().as_str() {
            "NOTE" => Alert::Note,
            "TIP" => Alert::Tip,
            "IMPORTANT" => Alert::Important,
            "WARNING" => Alert::Warning,
            "CAUTION" => Alert::Caution,
            _ => return None,
        })
    }
    pub fn title(self) -> &'static str {
        match self {
            Alert::Note => "Note",
            Alert::Tip => "Tip",
            Alert::Important => "Important",
            Alert::Warning => "Warning",
            Alert::Caution => "Caution",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Meta,
    Hunk,
    Add,
    Del,
    Ctx,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub sign: &'static str,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub aligns: Vec<Align>,
    pub header: Vec<Vec<Run>>,
    pub rows: Vec<Vec<Vec<Run>>>,
    /// Columns whose every non-empty body cell is a number, set right-aligned in tabular figures.
    pub numeric: Vec<bool>,
    pub header_pids: Vec<usize>,
    pub row_pids: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GalleryItem {
    pub img: Img,
    pub title: String,
    /// CSS px from `width` and `height`, 0 when unset.
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Footnote {
    pub number: usize,
    pub label: String,
    pub anchor: String,
    /// How many times it is cited.
    pub refs: usize,
    pub blocks: Vec<Block>,
    /// The definition's own source lines: definitions may sit anywhere in the note.
    pub lines: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub slug: String,
    /// The top-level block index.
    pub index: usize,
    pub line: usize,
}

/// A piece of searchable text: its id is its index in `Doc::pieces`.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub block: usize,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Doc {
    pub blocks: Vec<Block>,
    pub headings: Vec<Heading>,
    /// Anchor name to top-level block index: heading slugs, `fn-N` and `fnref-N-M` (BLK-07).
    pub anchors: HashMap<String, usize>,
    pub pieces: Vec<Piece>,
    pub line_count: usize,
}

// ---------- Building ----------

pub fn build(src: &str) -> Doc {
    let line_count = src.split('\n').count();
    let fm = front_matter::split(src);
    let body = fm.as_ref().map_or(src, |f| f.body.as_str());
    let opts = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_SMART_PUNCTUATION | Options::ENABLE_WIKILINKS;
    let ev: Vec<(Event, Range<usize>)> = Parser::new_ext(body, opts).into_offset_iter().collect();
    let mut w = Walker { src: body, ev, i: 0, starts: line_starts(body), cites: Vec::new(), defs: Vec::new() };
    let mut blocks = w.collect(0);
    if let Some(f) = &fm {
        blocks.insert(0, front_matter::block(f));
    }
    if let Some(b) = w.footnotes() {
        blocks.push(b);
    }
    stamp_keys(src, &mut blocks);
    let (headings, anchors, pieces) = index(&mut blocks);
    Doc { blocks, headings, anchors, pieces, line_count }
}

fn line_starts(s: &str) -> Vec<usize> {
    std::iter::once(0).chain(s.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

struct Walker<'a> {
    src: &'a str,
    ev: Vec<(Event<'a>, Range<usize>)>,
    i: usize,
    starts: Vec<usize>,
    /// Cited labels in order of first citation, lowercased, with their counts.
    cites: Vec<(String, usize)>,
    /// Definitions in source order: label as written, blocks, first line.
    defs: Vec<(String, Vec<Block>, Range<usize>)>,
}

fn is_inline(e: &Event) -> bool {
    match e {
        Event::Text(_) | Event::Code(_) | Event::InlineHtml(_) | Event::FootnoteReference(_) | Event::SoftBreak | Event::HardBreak => true,
        Event::Start(t) => matches!(t, Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }),
        Event::End(t) => matches!(t, TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link | TagEnd::Image),
        _ => false,
    }
}

impl<'a> Walker<'a> {
    fn line(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at).saturating_sub(1)
    }

    /// The lines a byte range covers, trailing blank lines left out.
    fn lines(&self, r: &Range<usize>) -> Range<usize> {
        let b = self.src.as_bytes();
        let mut end = r.end.min(b.len());
        while end > r.start && b[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        let first = self.line(r.start);
        first..(self.line(end.max(r.start + 1) - 1) + 1).max(first + 1)
    }

    fn block(&self, kind: Kind, r: &Range<usize>) -> Block {
        Block { kind, lines: self.lines(r), key: (0, 0), pid: None }
    }

    /// Blocks until the end of the enclosing container (consumed) or of the events.
    fn collect(&mut self, depth: usize) -> Vec<Block> {
        let mut out = Vec::new();
        while self.i < self.ev.len() {
            let e = &self.ev[self.i].0;
            if is_inline(e) {
                // A tight list item's text, which has no paragraph around it.
                let r0 = self.ev[self.i].1.clone();
                let (runs, r1) = self.inline();
                out.push(self.block(Kind::Paragraph { runs }, &(r0.start..r1.end)));
                continue;
            }
            match e {
                Event::End(_) => {
                    self.i += 1;
                    break;
                }
                Event::Start(_) => {
                    if let Some(b) = self.one(depth) {
                        out.push(b);
                    }
                }
                Event::Rule => {
                    let r = self.ev[self.i].1.clone();
                    self.i += 1;
                    out.push(self.block(Kind::Rule, &r));
                }
                _ => self.i += 1,
            }
        }
        out
    }

    fn one(&mut self, depth: usize) -> Option<Block> {
        let (e, r) = self.ev[self.i].clone();
        let Event::Start(tag) = e else { unreachable!() };
        match tag {
            Tag::Paragraph => Some(self.paragraph(&r)),
            Tag::Heading { level, .. } => {
                self.i += 1;
                let (runs, _) = self.inline();
                self.i += 1;
                let text = plain(&runs).trim().to_string();
                Some(self.block(Kind::Heading { level: level as usize, runs, text, slug: String::new() }, &r))
            }
            Tag::List(start) => Some(self.list(start, depth, &r)),
            Tag::BlockQuote(_) => {
                self.i += 1;
                let mut blocks = self.collect(0);
                Some(match alert_of(&mut blocks) {
                    Some(kind) => self.block(Kind::Alert { kind, blocks }, &r),
                    None => self.block(Kind::Quote { blocks }, &r),
                })
            }
            Tag::CodeBlock(k) => {
                self.i += 1;
                let text = self.text_until_end();
                let lang = match k {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                Some(self.block(code(lang, text), &r))
            }
            Tag::HtmlBlock => {
                self.i += 1;
                let text = self.text_until_end();
                html(text.trim_end()).map(|k| self.block(k, &r))
            }
            Tag::Table(aligns) => Some(self.table(aligns, &r)),
            Tag::FootnoteDefinition(label) => {
                self.i += 1;
                let lines = self.lines(&r);
                let blocks = self.collect(0);
                if !self.defs.iter().any(|d| d.0.to_lowercase() == label.to_lowercase()) {
                    self.defs.push((label.to_string(), blocks, lines));
                }
                None
            }
            _ => {
                // Anything else (definition lists, metadata) is skipped whole.
                self.skip();
                None
            }
        }
    }

    /// Skips from a start event through its matching end.
    fn skip(&mut self) {
        let mut d = 0;
        while self.i < self.ev.len() {
            match self.ev[self.i].0 {
                Event::Start(_) => d += 1,
                Event::End(_) => d -= 1,
                _ => {}
            }
            self.i += 1;
            if d == 0 {
                break;
            }
        }
    }

    /// The text of a code or HTML block, through its end event.
    fn text_until_end(&mut self) -> String {
        let mut s = String::new();
        while self.i < self.ev.len() {
            match &self.ev[self.i].0 {
                Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => s.push_str(t),
                Event::End(_) => {
                    self.i += 1;
                    break;
                }
                _ => {}
            }
            self.i += 1;
        }
        s
    }

    fn paragraph(&mut self, r: &Range<usize>) -> Block {
        self.i += 1;
        let images = self.only_images();
        let (runs, _) = self.inline();
        self.i += 1;
        match images {
            Some(mut imgs) if imgs.len() == 1 => {
                let (img, title) = imgs.remove(0);
                self.block(Kind::Image { img, title }, r)
            }
            Some(imgs) => self.block(Kind::Gallery { items: imgs.into_iter().map(|(img, title)| GalleryItem { img, title, w: 0.0, h: 0.0 }).collect() }, r),
            None => self.block(Kind::Paragraph { runs }, r),
        }
    }

    /// The images of a paragraph that holds only images, soft breaks and spaces (BLK-05, BLK-101).
    fn only_images(&self) -> Option<Vec<(Img, String)>> {
        let mut out = Vec::new();
        let mut j = self.i;
        while j < self.ev.len() {
            match &self.ev[j].0 {
                Event::End(TagEnd::Paragraph) => break,
                Event::SoftBreak => {}
                Event::Text(t) if t.trim().is_empty() => {}
                Event::Start(Tag::Image { link_type, dest_url, title, .. }) => {
                    let img = image(*link_type, dest_url, &self.alt_at(j + 1))?;
                    out.push((img, title.to_string()));
                    while !matches!(self.ev[j].0, Event::End(TagEnd::Image)) {
                        j += 1;
                    }
                }
                _ => return None,
            }
            j += 1;
        }
        (!out.is_empty()).then_some(out)
    }

    /// The alt text of an image whose content starts at event `j`.
    fn alt_at(&self, mut j: usize) -> String {
        let mut s = String::new();
        while j < self.ev.len() && !matches!(self.ev[j].0, Event::End(TagEnd::Image)) {
            if let Event::Text(t) | Event::Code(t) = &self.ev[j].0 {
                s.push_str(t);
            }
            j += 1;
        }
        s
    }

    /// Inline events from here to the first block event, as runs, and the byte range they span.
    fn inline(&mut self) -> (Vec<Run>, Range<usize>) {
        let mut out: Vec<Run> = Vec::new();
        let (mut b, mut it, mut s) = (0, 0, 0);
        let mut links: Vec<Link> = Vec::new();
        let start = self.ev.get(self.i).map_or(0, |e| e.1.start);
        let mut end = start;
        let push = |out: &mut Vec<Run>, mut r: Run, b: i32, it: i32, s: i32, links: &[Link]| {
            r.b = b > 0;
            r.i = it > 0;
            r.s = s > 0;
            if r.link.is_none() {
                r.link = links.last().cloned();
            }
            out.push(r);
        };
        while self.i < self.ev.len() && is_inline(&self.ev[self.i].0) {
            let (e, r) = self.ev[self.i].clone();
            end = end.max(r.end);
            self.i += 1;
            match e {
                Event::Text(t) => push(&mut out, Run::text(t.to_string()), b, it, s, &links),
                Event::Code(t) => push(&mut out, Run { t: t.to_string(), code: true, ..Run::default() }, b, it, s, &links),
                Event::SoftBreak => push(&mut out, Run { t: " ".into(), soft: true, ..Run::default() }, b, it, s, &links),
                Event::HardBreak => push(&mut out, Run { t: "\n".into(), br: true, ..Run::default() }, b, it, s, &links),
                Event::InlineHtml(h) => {
                    let h = h.trim();
                    if h.starts_with("<!--") {
                    } else if is_br(h) {
                        push(&mut out, Run { t: "\n".into(), br: true, ..Run::default() }, b, it, s, &links);
                    } else {
                        push(&mut out, Run { t: h.to_string(), raw: true, ..Run::default() }, b, it, s, &links);
                    }
                }
                Event::FootnoteReference(label) => {
                    let key = label.to_lowercase();
                    let n = match self.cites.iter().position(|c| c.0 == key) {
                        Some(k) => k,
                        None => {
                            self.cites.push((key, 0));
                            self.cites.len() - 1
                        }
                    };
                    self.cites[n].1 += 1;
                    let (num, sub) = (n + 1, self.cites[n].1);
                    let r = Run { t: num.to_string(), note: true, anchor: Some(format!("fnref-{num}-{sub}")), link: Some(Link::Anchor(format!("fn-{num}"))), ..Run::default() };
                    push(&mut out, r, b, it, s, &links);
                }
                Event::Start(Tag::Strong) => b += 1,
                Event::End(TagEnd::Strong) => b = (b - 1).max(0),
                Event::Start(Tag::Emphasis) => it += 1,
                Event::End(TagEnd::Emphasis) => it = (it - 1).max(0),
                Event::Start(Tag::Strikethrough) => s += 1,
                Event::End(TagEnd::Strikethrough) => s = (s - 1).max(0),
                Event::Start(Tag::Link { link_type, dest_url, .. }) => links.push(match link_type {
                    LinkType::WikiLink { .. } => Link::Wiki(dest_url.to_string()),
                    LinkType::Email => Link::Url(format!("mailto:{dest_url}")),
                    _ => Link::Url(dest_url.to_string()),
                }),
                Event::End(TagEnd::Link) => {
                    links.pop();
                }
                Event::Start(Tag::Image { link_type, dest_url, .. }) => {
                    let alt = self.alt_at(self.i);
                    while self.i < self.ev.len() && !matches!(self.ev[self.i].0, Event::End(TagEnd::Image)) {
                        self.i += 1;
                    }
                    self.i += 1;
                    match image(link_type, &dest_url, &alt) {
                        Some(img) => {
                            let link = links.last().cloned().or_else(|| Some(Link::Url(img.src.clone())));
                            let t = if alt.is_empty() { "image".to_string() } else { alt };
                            push(&mut out, Run { t, img: Some(img.src), link, ..Run::default() }, b, it, s, &links);
                        }
                        // `![[Note]]`, an embed of a note: its link.
                        None => push(&mut out, Run { t: dest_url.to_string(), link: Some(Link::Wiki(dest_url.to_string())), ..Run::default() }, b, it, s, &links),
                    }
                }
                _ => {}
            }
        }
        (finish(out), start..end)
    }

    fn list(&mut self, start: Option<u64>, depth: usize, r: &Range<usize>) -> Block {
        self.i += 1;
        let ordered = start.is_some();
        let first = start.unwrap_or(1);
        let mut tight = true;
        let mut items = Vec::new();
        while self.i < self.ev.len() {
            let (e, ir) = self.ev[self.i].clone();
            match e {
                Event::End(TagEnd::List(_)) => {
                    self.i += 1;
                    break;
                }
                Event::Start(Tag::Item) => {
                    if self.has_paragraph() {
                        tight = false;
                    }
                    self.i += 1;
                    let number = if ordered { own_number(&self.src[ir.start..]).unwrap_or(first + items.len() as u64) } else { 0 };
                    let mut blocks = self.collect(depth + 1);
                    let task = task_of(&mut blocks);
                    items.push(Item { number, task, line: self.line(ir.start), blocks });
                }
                _ => self.i += 1,
            }
        }
        let tasks = items.iter().any(|it| it.task != Task::None);
        self.block(Kind::List(List { ordered, start: first, tight, depth, tasks, items }), r)
    }

    /// Whether the item starting here has a paragraph of its own, which pulldown-cmark leaves
    /// out in tight lists.
    fn has_paragraph(&self) -> bool {
        let mut d = 0;
        for (e, _) in &self.ev[self.i + 1..] {
            match e {
                Event::Start(Tag::Paragraph) if d == 0 => return true,
                Event::Start(_) => d += 1,
                Event::End(_) if d == 0 => return false,
                Event::End(_) => d -= 1,
                _ => {}
            }
        }
        false
    }

    fn table(&mut self, aligns: Vec<Alignment>, r: &Range<usize>) -> Block {
        self.i += 1;
        let mut header = Vec::new();
        let mut rows: Vec<Vec<Vec<Run>>> = Vec::new();
        let mut in_head = false;
        while self.i < self.ev.len() {
            match &self.ev[self.i].0 {
                Event::End(TagEnd::Table) => {
                    self.i += 1;
                    break;
                }
                Event::Start(Tag::TableHead) => in_head = true,
                Event::End(TagEnd::TableHead) => in_head = false,
                Event::Start(Tag::TableRow) => rows.push(Vec::new()),
                Event::Start(Tag::TableCell) => {
                    self.i += 1;
                    let (runs, _) = self.inline();
                    if in_head {
                        header.push(runs);
                    } else if let Some(row) = rows.last_mut() {
                        row.push(runs);
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
        let cols = header.len();
        for row in &mut rows {
            row.resize(cols, Vec::new());
        }
        let numeric = (0..cols)
            .map(|c| {
                let cells: Vec<String> = rows.iter().map(|row| plain(&row[c]).trim().to_string()).filter(|t| !t.is_empty()).collect();
                !cells.is_empty() && cells.iter().all(|t| is_numeric(t))
            })
            .collect();
        let aligns = aligns
            .into_iter()
            .map(|a| match a {
                Alignment::None => Align::None,
                Alignment::Left => Align::Left,
                Alignment::Center => Align::Center,
                Alignment::Right => Align::Right,
            })
            .collect();
        self.block(Kind::Table(Table { aligns, header, rows, numeric, header_pids: Vec::new(), row_pids: Vec::new() }), r)
    }

    /// The footnotes block: cited notes in order of first citation, then the rest as written.
    fn footnotes(&mut self) -> Option<Block> {
        if self.defs.is_empty() {
            return None;
        }
        let mut defs = std::mem::take(&mut self.defs);
        let mut picked: Vec<(String, usize, Vec<Block>, Range<usize>)> = Vec::new();
        for (label, n) in &self.cites {
            if let Some(k) = defs.iter().position(|d| d.0.to_lowercase() == *label) {
                let (l, b, line) = defs.remove(k);
                picked.push((l, *n, b, line));
            }
        }
        picked.extend(defs.into_iter().map(|(l, b, line)| (l, 0, b, line)));
        let start = picked.iter().map(|p| p.3.start).min().unwrap_or(0);
        let end = picked.iter().map(|p| p.3.start + 1).max().unwrap_or(0);
        let items = picked.into_iter().enumerate().map(|(k, (label, refs, blocks, lines))| Footnote { number: k + 1, label, anchor: format!("fn-{}", k + 1), refs, blocks, lines }).collect();
        Some(Block { kind: Kind::Footnotes { items }, lines: start..end, key: (0, 0), pid: None })
    }
}

fn is_br(h: &str) -> bool {
    let l = h.to_ascii_lowercase();
    let inner = l.strip_prefix("<br").and_then(|r| r.strip_suffix('>')).map(|r| r.trim_end().trim_end_matches('/').trim());
    inner == Some("")
}

/// An image from an image event: `![alt](src)`, or `![[file.png]]` (BLK-101). A `![[Note]]` is
/// not an image.
fn image(link_type: LinkType, dest: &str, alt: &str) -> Option<Img> {
    if let LinkType::WikiLink { .. } = link_type {
        let name = dest.trim();
        if !md::is_image(name) {
            return None;
        }
        let stem = name.rsplit('/').next().unwrap_or(name);
        let stem = stem.rsplit_once('.').map_or(stem, |(b, _)| b);
        return Some(Img { alt: stem.to_string(), src: name.to_string(), embed: true });
    }
    Some(Img { alt: alt.to_string(), src: md::image_src(dest), embed: false })
}

/// The number an ordered item's marker shows, from the source at the item's start.
fn own_number(s: &str) -> Option<u64> {
    let s = s.trim_start_matches(|c: char| c == ' ' || c == '\t' || c == '>');
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

// ---------- Runs: merge, typographer, autolinks, tags ----------

/// Joins neighbouring plain runs of one style, then applies the typographer's remaining
/// replacements (PAGE-81), autolinks (PAGE-82) and `#tags` (BLK-101).
fn finish(runs: Vec<Run>) -> Vec<Run> {
    let mut merged: Vec<Run> = Vec::new();
    for r in runs {
        if let Some(last) = merged.last_mut() {
            if last.plain() && r.plain() && last.same_style(&r) {
                last.t.push_str(&r.t);
                continue;
            }
        }
        merged.push(r);
    }
    let mut out = Vec::new();
    for mut r in merged {
        if !r.plain() || r.t.is_empty() {
            if !r.t.is_empty() {
                out.push(r);
            }
            continue;
        }
        r.t = typographer(&r.t);
        split_links(r, &mut out);
    }
    out
}

/// markdown-it's replacements that pulldown-cmark's smart punctuation leaves out.
fn typographer(s: &str) -> String {
    if !s.contains(['(', '+', '.', '?', '!', ',']) {
        return s.to_string();
    }
    let mut t = s.to_string();
    for (a, b) in [("(c)", "©"), ("(C)", "©"), ("(r)", "®"), ("(R)", "®"), ("(tm)", "™"), ("(TM)", "™"), ("(Tm)", "™"), ("(tM)", "™"), ("+-", "±")] {
        t = t.replace(a, b);
    }
    let mut out = String::with_capacity(t.len());
    let c: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let run = |ch: char| c[i..].iter().take_while(|&&x| x == ch).count();
        match c[i] {
            // Two or more dots, or pulldown-cmark's ellipsis and any dots after it: one
            // ellipsis, or two dots after `?` and `!`.
            '.' | '…' if c[i] == '…' || c.get(i + 1) == Some(&'.') || c.get(i + 1) == Some(&'…') => {
                let n = c[i..].iter().take_while(|&&x| x == '.' || x == '…').count();
                if out.ends_with(['?', '!']) {
                    out.push_str("..");
                } else {
                    out.push('…');
                }
                i += n;
            }
            '?' | '!' if run(c[i]) >= 4 => {
                let n = run(c[i]);
                (0..3).for_each(|_| out.push(c[i]));
                i += n;
            }
            ',' if run(',') >= 2 => {
                out.push(',');
                i += run(',');
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// Splits a plain run at autolinks and tags.
fn split_links(r: Run, out: &mut Vec<Run>) {
    let mut marks: Vec<(Range<usize>, Link)> = autolinks(&r.t).into_iter().map(|(rg, url)| (rg, Link::Url(url))).collect();
    for (tok, rg) in md::inline_tokens(&r.t) {
        if tok == md::Tok::Tag && !marks.iter().any(|(m, _)| m.start < rg.end && rg.start < m.end) {
            let tag = r.t[rg.start + 1..rg.end].to_string();
            marks.push((rg, Link::Tag(tag)));
        }
    }
    if marks.is_empty() {
        out.push(r);
        return;
    }
    marks.sort_by_key(|m| m.0.start);
    let mut at = 0;
    for (rg, link) in marks {
        if rg.start > at {
            out.push(Run { t: r.t[at..rg.start].to_string(), ..r.clone() });
        }
        out.push(Run { t: r.t[rg.clone()].to_string(), link: Some(link), ..r.clone() });
        at = rg.end;
    }
    if at < r.t.len() {
        out.push(Run { t: r.t[at..].to_string(), ..r });
    }
}

/// omarky's linkify: http, https and ftp URLs, `www.` hosts and e-mail addresses, with their
/// trailing punctuation trimmed and only balanced closing parentheses kept (PAGE-82).
pub fn autolinks(s: &str) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    if !(s.contains("://") || s.contains("www.") || s.contains('@')) {
        return out;
    }
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !s.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let rest = &s[i..];
        let url = ["https://", "http://", "ftp://", "www."].iter().any(|p| rest.starts_with(p));
        let m = if url { url_at(s, i) } else { email_at(s, i) };
        let Some(end) = m else {
            i += 1;
            continue;
        };
        let raw = &s[i..end];
        let glued = i > 0 && b[i - 1].is_ascii_alphanumeric() && !raw.contains('@');
        if !glued {
            let t = trim_parens(raw);
            let link = if t.starts_with("www.") {
                format!("http://{t}")
            } else if t.contains('@') && !t.contains("://") {
                format!("mailto:{t}")
            } else {
                t.to_string()
            };
            out.push((i..i + t.len(), link));
        }
        i = end;
    }
    out
}

/// `(scheme://|www.)[^\s<>]*[^\s<>.,:;"'!?*_~\]]`: the end of a URL starting at `i`.
fn url_at(s: &str, i: usize) -> Option<usize> {
    let run = s[i..].find(|c: char| c.is_whitespace() || c == '<' || c == '>').map_or(s.len(), |k| i + k);
    let t = s[i..run].trim_end_matches(['.', ',', ':', ';', '"', '\'', '!', '?', '*', '_', '~', ']']);
    let prefix = if s[i..].starts_with("www.") { 4 } else { s[i..].find("://").map_or(0, |k| k + 3) };
    // The pattern needs one character after the prefix.
    (t.len() > prefix).then_some(i + t.len())
}

/// `[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}` starting at `i`.
fn email_at(s: &str, i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let local = |c: u8| c.is_ascii_alphanumeric() || b"._%+-".contains(&c);
    let mut j = i;
    while j < b.len() && local(b[j]) {
        j += 1;
    }
    if j == i || b.get(j) != Some(&b'@') {
        return None;
    }
    let d0 = j + 1;
    let mut k = d0;
    while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'-' || b[k] == b'.') {
        k += 1;
    }
    // The longest prefix of the domain that ends in `.` and two or more letters.
    (d0..=k).rev().find(|&e| {
        let d = &s[d0..e];
        let labels: Vec<&str> = d.split('.').collect();
        labels.len() >= 2
            && labels.iter().all(|l| !l.is_empty() && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
            && labels.last().is_some_and(|l| l.len() >= 2 && l.bytes().all(|c| c.is_ascii_alphabetic()))
    })
}

fn trim_parens(u: &str) -> &str {
    let open = u.matches('(').count();
    let mut close = u.matches(')').count();
    let mut t = u;
    while close > open && t.ends_with(')') {
        t = &t[..t.len() - 1];
        close -= 1;
    }
    t
}

// ---------- Block rules ----------

/// `[ ]`, `[x]` or `[X]` and a space at the start of an item makes it a task (BLK-28).
fn task_of(blocks: &mut Vec<Block>) -> Task {
    let Some(Block { kind: Kind::Paragraph { runs }, .. }) = blocks.first_mut() else { return Task::None };
    let Some(r) = runs.first_mut() else { return Task::None };
    if r.code || r.raw || r.link.is_some() {
        return Task::None;
    }
    let t = r.t.as_bytes();
    if t.len() < 3 || t[0] != b'[' || t[2] != b']' || !matches!(t[1], b' ' | b'x' | b'X') {
        return Task::None;
    }
    let rest = &r.t[3..];
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return Task::None;
    }
    let done = t[1] != b' ';
    r.t = rest.trim_start().to_string();
    if r.t.is_empty() {
        runs.remove(0);
    }
    if done { Task::Done } else { Task::Todo }
}

/// `> [!NOTE]` alone on a quote's first line makes it an alert (BLK-101 section 7).
fn alert_of(blocks: &mut Vec<Block>) -> Option<Alert> {
    let Some(Block { kind: Kind::Paragraph { runs }, .. }) = blocks.first_mut() else { return None };
    let mut line = String::new();
    let mut n = 0;
    while n < runs.len() {
        let r = &runs[n];
        if r.soft || r.br {
            break;
        }
        if r.code || r.raw || (r.link.is_some() && !matches!(r.link, Some(Link::Tag(_)))) {
            return None;
        }
        line.push_str(&r.t);
        n += 1;
    }
    let inner = line.trim().strip_prefix("[!")?.strip_suffix(']')?;
    if inner.is_empty() || !inner.bytes().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let kind = Alert::parse(inner)?;
    runs.drain(..(n + 1).min(runs.len()));
    if runs.is_empty() {
        blocks.remove(0);
    }
    Some(kind)
}

fn code(lang: String, text: String) -> Kind {
    if matches!(lang.to_lowercase().as_str(), "diff" | "patch" | "udiff") {
        let (mut added, mut removed) = (0, 0);
        let lines = text
            .split('\n')
            .map(|l| {
                let meta = ["diff ", "index ", "--- ", "+++ ", "new file", "deleted file", "similarity", "rename ", "old mode", "new mode", "Binary files"];
                let (kind, sign, body) = if meta.iter().any(|m| l.starts_with(m)) {
                    (DiffKind::Meta, "", l)
                } else if l.starts_with("@@") {
                    (DiffKind::Hunk, "", l)
                } else if let Some(r) = l.strip_prefix('+') {
                    added += 1;
                    (DiffKind::Add, "+", r)
                } else if let Some(r) = l.strip_prefix('-') {
                    removed += 1;
                    (DiffKind::Del, "−", r)
                } else if let Some(r) = l.strip_prefix(' ') {
                    (DiffKind::Ctx, " ", r)
                } else if l.starts_with('\\') {
                    (DiffKind::Meta, "", l)
                } else {
                    (DiffKind::Ctx, " ", l)
                };
                DiffLine { kind, sign, text: body.to_string() }
            })
            .collect();
        return Kind::Diff { lang, text, lines, added, removed };
    }
    Kind::Code { lang, text }
}

/// `^[-+−]?[$€£¥]?\d[\d,]*(\.\d+)?(%|:\d+(\.\d+)?|[kKmMbBx]|ms|s|h)?$`
fn is_numeric(s: &str) -> bool {
    let mut c: &str = s;
    c = c.strip_prefix(['-', '+', '−']).unwrap_or(c);
    c = c.strip_prefix(['$', '€', '£', '¥']).unwrap_or(c);
    let digits = |t: &str| t.len() - t.trim_start_matches(|ch: char| ch.is_ascii_digit()).len();
    if !c.starts_with(|ch: char| ch.is_ascii_digit()) {
        return false;
    }
    c = c.trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == ',');
    if let Some(r) = c.strip_prefix('.') {
        if digits(r) == 0 {
            return false;
        }
        c = &r[digits(r)..];
    }
    if c.is_empty() || matches!(c, "%" | "k" | "K" | "m" | "M" | "b" | "B" | "x" | "ms" | "s" | "h") {
        return true;
    }
    let Some(r) = c.strip_prefix(':') else { return false };
    let n = digits(r);
    if n == 0 {
        return false;
    }
    let r = &r[n..];
    r.is_empty() || r.strip_prefix('.').is_some_and(|f| !f.is_empty() && digits(f) == f.len())
}

/// An HTML block: dropped when it is only comments, a gallery when it is only images.
fn html(text: &str) -> Option<Kind> {
    if only_comments(text) {
        return None;
    }
    if let Some(items) = html_images(text) {
        return Some(Kind::Gallery { items });
    }
    Some(Kind::Html { text: text.to_string() })
}

fn only_comments(t: &str) -> bool {
    let mut rest = t.trim();
    if rest.is_empty() {
        return false;
    }
    while !rest.is_empty() {
        let Some(r) = rest.strip_prefix("<!--") else { return false };
        let Some(end) = r.find("-->") else { return false };
        rest = r[end + 3..].trim_start();
    }
    true
}

fn unescape_html(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

fn css_px(v: &str) -> f32 {
    let v = v.trim();
    let v = v.strip_suffix("px").or_else(|| v.strip_suffix("PX")).unwrap_or(v);
    if v.is_empty() || !v.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return 0.0;
    }
    v.parse().unwrap_or(0.0)
}

/// HTML that is nothing but `<img>` tags, optionally wrapped in `<p>`, `<div>`, `<center>` or
/// `<a>` and spaced with `<br>` or `&nbsp;`.
fn html_images(text: &str) -> Option<Vec<GalleryItem>> {
    let mut items = Vec::new();
    let mut rest = text;
    loop {
        let Some(lt) = rest.find('<') else {
            if !rest.replace("&nbsp;", "").trim().is_empty() {
                return None;
            }
            break;
        };
        if !rest[..lt].replace("&nbsp;", "").trim().is_empty() {
            return None;
        }
        let tag = &rest[lt + 1..];
        // The tag's end, skipping `>` inside quotes.
        let mut q = None;
        let mut end = None;
        for (k, ch) in tag.char_indices() {
            match (q, ch) {
                (None, '"' | '\'') => q = Some(ch),
                (Some(o), c) if c == o => q = None,
                (None, '>') => {
                    end = Some(k);
                    break;
                }
                _ => {}
            }
        }
        let end = end?;
        let body = &tag[..end];
        rest = &tag[end + 1..];
        let closing = body.starts_with('/');
        let body = body.trim_start_matches('/');
        let name: String = body.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase();
        if name.is_empty() {
            return None;
        }
        if name == "img" && !closing {
            let attrs = attributes(body[3..].trim_end().trim_end_matches('/'));
            let get = |k: &str| attrs.iter().find(|a| a.0 == k).map(|a| a.1.clone());
            let src = get("src")?;
            let style: Vec<(String, String)> = get("style")
                .unwrap_or_default()
                .split(';')
                .filter_map(|d| {
                    let kv: Vec<&str> = d.split(':').collect();
                    (kv.len() == 2).then(|| (kv[0].trim().to_ascii_lowercase(), kv[1].trim().to_string()))
                })
                .collect();
            let sty = |k: &str| style.iter().find(|s| s.0 == k).map_or(0.0, |s| css_px(&s.1));
            let w = Some(sty("width")).filter(|&x| x > 0.0).unwrap_or_else(|| css_px(&get("width").unwrap_or_default()));
            let h = Some(sty("height")).filter(|&x| x > 0.0).unwrap_or_else(|| css_px(&get("height").unwrap_or_default()));
            let img = Img { alt: unescape_html(&get("alt").unwrap_or_default()), src: unescape_html(&src), embed: false };
            items.push(GalleryItem { img, title: unescape_html(&get("title").unwrap_or_default()), w, h });
        } else if !matches!(name.as_str(), "p" | "div" | "center" | "a" | "br") {
            return None;
        }
    }
    (!items.is_empty()).then_some(items)
}

/// `name=value` pairs of a tag, names lowercased; values quoted with either quote or bare.
fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if !(c[i].is_ascii_alphabetic() || c[i] == '-') {
            i += 1;
            continue;
        }
        let n0 = i;
        while i < c.len() && (c[i].is_ascii_alphabetic() || c[i] == '-') {
            i += 1;
        }
        let name: String = c[n0..i].iter().collect::<String>().to_ascii_lowercase();
        let mut j = i;
        while j < c.len() && c[j].is_whitespace() {
            j += 1;
        }
        if j >= c.len() || c[j] != '=' {
            continue;
        }
        j += 1;
        while j < c.len() && c[j].is_whitespace() {
            j += 1;
        }
        let value: String;
        if j < c.len() && (c[j] == '"' || c[j] == '\'') {
            let q = c[j];
            let v0 = j + 1;
            let mut k = v0;
            while k < c.len() && c[k] != q {
                k += 1;
            }
            value = c[v0..k.min(c.len())].iter().collect();
            i = k + 1;
        } else {
            let v0 = j;
            let mut k = v0;
            while k < c.len() && !c[k].is_whitespace() && !matches!(c[k], '"' | '\'' | '>') {
                k += 1;
            }
            value = c[v0..k].iter().collect();
            i = k;
        }
        out.push((name, value));
    }
    out
}

// ---------- Keys and index ----------

/// Hashes a source line for `Block::key`, normalizing Markdown task checkboxes (`[x]` / `[X]`
/// -> `[ ]`) so toggling a task in preview keeps the list block's identity and avoids unmount
/// / growth-animation reflow of the blocks below it.
fn hash_block_line<H: Hasher>(line: &str, h: &mut H) {
    let bytes = line.as_bytes();
    let mut i = 0;
    loop {
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'>' {
            i += 1;
            continue;
        }
        break;
    }
    let after_marker = if i + 1 < bytes.len() && matches!(bytes[i], b'-' | b'*' | b'+') && matches!(bytes[i + 1], b' ' | b'\t') {
        Some(i + 1)
    } else {
        let mut d = i;
        while d < bytes.len() && bytes[d].is_ascii_digit() {
            d += 1;
        }
        if d > i && d + 1 < bytes.len() && matches!(bytes[d], b'.' | b')') && matches!(bytes[d + 1], b' ' | b'\t') {
            Some(d + 1)
        } else {
            None
        }
    };
    if let Some(mut j) = after_marker {
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
            j += 1;
        }
        if j + 3 <= bytes.len()
            && bytes[j] == b'['
            && matches!(bytes[j + 1], b' ' | b'x' | b'X')
            && bytes[j + 2] == b']'
            && (j + 3 == bytes.len() || matches!(bytes[j + 3], b' ' | b'\t'))
        {
            line[..j].hash(h);
            "[ ]".hash(h);
            line[j + 3..].hash(h);
            return;
        }
    }
    line.hash(h);
}

/// Each top-level block's key: its source text's hash and how many blocks before it had the same.
fn stamp_keys(src: &str, blocks: &mut [Block]) {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut seen: HashMap<u64, u32> = HashMap::new();
    for b in blocks {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let (a, z) = (b.lines.start.min(lines.len()), b.lines.end.min(lines.len()));
        (z - a).hash(&mut h);
        for &ln in &lines[a..z] {
            hash_block_line(ln, &mut h);
        }
        std::mem::discriminant(&b.kind).hash(&mut h);
        let hash = h.finish();
        let n = seen.entry(hash).or_insert(0);
        b.key = (hash, *n);
        *n += 1;
    }
}

/// GitHub's heading anchors: lower case, punctuation and symbols dropped, each space a hyphen.
pub fn slugify(t: &str) -> String {
    t.to_lowercase()
        .chars()
        .filter(|&c| {
            let u = c as u32;
            !(u <= 0x1f
                || ('!'..=',').contains(&c)
                || c == '.'
                || c == '/'
                || (':'..='@').contains(&c)
                || ('['..='^').contains(&c)
                || c == '`'
                || ('{'..='~').contains(&c)
                || ('\u{a1}'..='\u{bf}').contains(&c)
                || c == '×'
                || c == '÷'
                || ('\u{2000}'..='\u{206f}').contains(&c)
                || ('\u{2190}'..='\u{2bff}').contains(&c)
                || ('\u{3000}'..='\u{303f}').contains(&c)
                || ('\u{fe00}'..='\u{fe0f}').contains(&c))
        })
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Stamps slugs and find pieces, and lists headings and anchors.
fn index(blocks: &mut [Block]) -> (Vec<Heading>, HashMap<String, usize>, Vec<Piece>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut headings = Vec::new();
    for (i, b) in blocks.iter_mut().enumerate() {
        if let Kind::Heading { level, text, slug, .. } = &mut b.kind {
            let base = slugify(text);
            let n = seen.entry(base.clone()).or_insert(0);
            *slug = if *n > 0 { format!("{base}-{n}") } else { base };
            *n += 1;
            headings.push(Heading { level: *level, text: text.clone(), slug: slug.clone(), index: i, line: b.lines.start });
        }
    }
    let mut anchors = HashMap::new();
    for (i, b) in blocks.iter().enumerate() {
        match &b.kind {
            Kind::Heading { slug, .. } => {
                anchors.entry(slug.clone()).or_insert(i);
            }
            Kind::Footnotes { items } => {
                for it in items {
                    anchors.insert(it.anchor.clone(), i);
                }
            }
            _ => cited(b, i, &mut anchors),
        }
    }
    let mut pieces = Vec::new();
    for (i, b) in blocks.iter_mut().enumerate() {
        stamp(b, i, &mut pieces);
    }
    (headings, anchors, pieces)
}

fn cited(b: &Block, top: usize, out: &mut HashMap<String, usize>) {
    let mut runs = |rs: &[Run]| {
        for r in rs {
            if let Some(a) = &r.anchor {
                out.entry(a.clone()).or_insert(top);
            }
        }
    };
    match &b.kind {
        Kind::Paragraph { runs: rs } | Kind::Heading { runs: rs, .. } => runs(rs),
        Kind::Table(t) => t.header.iter().chain(t.rows.iter().flatten()).for_each(|c| runs(c)),
        Kind::Quote { blocks } | Kind::Alert { blocks, .. } => blocks.iter().for_each(|c| cited(c, top, out)),
        Kind::List(l) => l.items.iter().flat_map(|it| &it.blocks).for_each(|c| cited(c, top, out)),
        _ => {}
    }
}

fn stamp(b: &mut Block, top: usize, out: &mut Vec<Piece>) {
    let add = |text: String, out: &mut Vec<Piece>| {
        out.push(Piece { block: top, text });
        out.len() - 1
    };
    match &mut b.kind {
        Kind::Paragraph { runs } | Kind::Heading { runs, .. } => b.pid = Some(add(plain(runs), out)),
        Kind::Code { text, .. } | Kind::Diff { text, .. } => b.pid = Some(add(text.clone(), out)),
        Kind::List(l) => l.items.iter_mut().flat_map(|it| &mut it.blocks).for_each(|c| stamp(c, top, out)),
        Kind::Quote { blocks } | Kind::Alert { blocks, .. } => blocks.iter_mut().for_each(|c| stamp(c, top, out)),
        Kind::Footnotes { items } => items.iter_mut().flat_map(|it| &mut it.blocks).for_each(|c| stamp(c, top, out)),
        Kind::Table(t) => {
            t.header_pids = t.header.iter().map(|c| add(plain(c), out)).collect();
            t.row_pids = t.rows.iter().map(|r| r.iter().map(|c| add(plain(c), out)).collect()).collect();
        }
        _ => {}
    }
}

/// The find pieces inside block `bl`, as `stamp` numbered them: (piece, its text).
pub fn pieces_in(bl: &Block, out: &mut Vec<(usize, String)>) {
    match &bl.kind {
        Kind::Paragraph { runs } | Kind::Heading { runs, .. } => out.extend(bl.pid.map(|p| (p, plain(runs)))),
        Kind::Code { text, .. } | Kind::Diff { text, .. } => out.extend(bl.pid.map(|p| (p, text.clone()))),
        Kind::List(l) => l.items.iter().flat_map(|it| &it.blocks).for_each(|c| pieces_in(c, out)),
        Kind::Quote { blocks } | Kind::Alert { blocks, .. } => blocks.iter().for_each(|c| pieces_in(c, out)),
        Kind::Footnotes { items } => items.iter().flat_map(|it| &it.blocks).for_each(|c| pieces_in(c, out)),
        Kind::Table(t) => {
            out.extend(t.header_pids.iter().zip(&t.header).map(|(&p, c)| (p, plain(c))));
            for (pids, row) in t.row_pids.iter().zip(&t.rows) {
                out.extend(pids.iter().zip(row).map(|(&p, c)| (p, plain(c))));
            }
        }
        _ => {}
    }
}

/// A find match: the piece, its top-level block, and which match within the piece it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub pid: usize,
    pub block: usize,
    pub k: usize,
}

/// Every case-insensitive, non-overlapping match of `q`, in document order (RD-55).
pub fn find_all(doc: &Doc, q: &str) -> Vec<Match> {
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    let needle = q.to_lowercase();
    for (pid, p) in doc.pieces.iter().enumerate() {
        let hay = p.text.to_lowercase();
        let mut at = 0;
        let mut k = 0;
        while let Some(f) = hay[at..].find(&needle) {
            out.push(Match { pid, block: p.block, k });
            k += 1;
            at += f + needle.len();
        }
    }
    out
}

/// Byte range and replacement character to tick (`done`) or clear the task box on 0-based `line`.
pub fn toggle_task_splice(src: &str, line: usize, done: bool) -> Option<(std::ops::Range<usize>, &'static str)> {
    let mut start = 0;
    for (i, l) in src.split('\n').enumerate() {
        if i == line {
            let at = task_box(l)?;
            return Some((start + at..start + at + 1, if done { "x" } else { " " }));
        }
        start += l.len() + 1;
    }
    None
}

/// `src` with the task box on 0-based `line` ticked or cleared, or `None` when that line is
/// not a task item. Only the one character changes.
pub fn toggle_task(src: &str, line: usize, done: bool) -> Option<String> {
    let (r, with) = toggle_task_splice(src, line, done)?;
    let mut out = src.to_string();
    out.replace_range(r, with);
    Some(out)
}

/// The byte of the task box's mark on a line: quote marks, indent, a bullet or number, then `[`.
fn task_box(l: &str) -> Option<usize> {
    let b = l.as_bytes();
    let mut i = 0;
    loop {
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if b.get(i) == Some(&b'>') {
            i += 1;
        } else {
            break;
        }
    }
    match b.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        c if c.is_ascii_digit() => {
            let d0 = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - d0 > 9 || !matches!(b.get(i), Some(b'.' | b')')) {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    let s0 = i;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i == s0 || b.get(i) != Some(&b'[') || !matches!(b.get(i + 1), Some(b' ' | b'x' | b'X')) || b.get(i + 2) != Some(&b']') {
        return None;
    }
    matches!(b.get(i + 3), None | Some(b' ' | b'\t' | b'\r')).then_some(i + 1)
}

// ---------- Front matter ----------

pub(crate) mod front_matter {
    use super::{Block, Field, FieldKind, Kind};

    pub struct Split {
        pub raw: String,
        /// Lines the front matter takes, through its closing fence or blank line.
        pub end: usize,
        /// The source with those lines blanked.
        pub body: String,
    }

    pub fn split(src: &str) -> Option<Split> {
        let mut lines: Vec<&str> = src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
        if let Some(f) = lines.first_mut() {
            *f = f.strip_prefix('\u{feff}').unwrap_or(f);
        }
        fenced(&lines).or_else(|| bare(&lines))
    }

    fn cut(lines: &[&str], from: usize, to: usize, end: usize) -> Split {
        let body = std::iter::repeat_n("", end).chain(lines[end..].iter().copied()).collect::<Vec<_>>().join("\n");
        Split { raw: lines[from..to].join("\n"), end, body }
    }

    fn fenced(lines: &[&str]) -> Option<Split> {
        if lines.first() != Some(&"---") {
            return None;
        }
        let i = lines.iter().skip(1).position(|l| *l == "---" || *l == "...")? + 1;
        Some(cut(lines, 1, i, i + 1))
    }

    fn bare_key(l: &str) -> bool {
        let b = l.as_bytes();
        if b.is_empty() || !(b[0].is_ascii_alphabetic() || b[0] == b'_') {
            return false;
        }
        let n = b.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_' || **c == b'-').count();
        b.get(n) == Some(&b':') && matches!(b.get(n + 1), None | Some(b' '))
    }

    /// How many lines bare front matter takes at the top of `lines`, up to the blank line
    /// that ends it.
    pub fn bare_len(lines: &[&str]) -> Option<usize> {
        let mut keys = 0;
        let mut i = 0;
        while i < lines.len() && !lines[i].trim().is_empty() {
            if bare_key(lines[i]) {
                keys += 1;
            } else if !(keys > 0 && lines[i].starts_with(char::is_whitespace)) {
                return None;
            }
            i += 1;
        }
        (keys >= 2).then_some(i)
    }

    fn bare(lines: &[&str]) -> Option<Split> {
        let i = bare_len(lines)?;
        Some(cut(lines, 0, i, (i + 1).min(lines.len())))
    }

    fn unquote(v: &str) -> String {
        let v = v.trim();
        let b = v.as_bytes();
        if b.len() >= 2 && ((b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\'')) {
            return v[1..v.len() - 1].to_string();
        }
        // ` # comment` at the end goes.
        match v.char_indices().find(|&(k, c)| c == '#' && k > 0 && v[..k].ends_with(char::is_whitespace)) {
            Some((k, _)) => v[..k].trim_end().to_string(),
            None => v.to_string(),
        }
    }

    fn flow_list(v: &str) -> Vec<String> {
        let inner = &v.trim()[1..v.trim().len() - 1];
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut quote: Option<char> = None;
        for c in inner.chars() {
            match quote {
                Some(q) => {
                    if c == q {
                        quote = None;
                    }
                    cur.push(c);
                }
                None if c == '"' || c == '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                None if c == ',' => out.push(unquote(&std::mem::take(&mut cur))),
                None => cur.push(c),
            }
        }
        if !cur.trim().is_empty() {
            out.push(unquote(&cur));
        }
        out.into_iter().filter(|s| !s.is_empty()).collect()
    }

    fn indent(l: &str) -> usize {
        l.len() - l.trim_start_matches(' ').len()
    }

    /// `key:` and the rest of a line: `^([A-Za-z0-9_][A-Za-z0-9_ .-]*?)\s*:(?:\s+(.*))?$`.
    fn key_line(l: &str) -> Option<(String, String)> {
        let b = l.as_bytes();
        if b.is_empty() || !(b[0].is_ascii_alphanumeric() || b[0] == b'_') {
            return None;
        }
        for (k, c) in l.char_indices() {
            if c == ':' {
                let after = &l[k + 1..];
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return Some((l[..k].trim_end().to_string(), after.trim().to_string()));
                }
            }
            if !(c.is_ascii_alphanumeric() || matches!(c, '_' | ' ' | '.' | '-' | '\t')) {
                return None;
            }
        }
        None
    }

    pub struct RawField {
        pub key: String,
        pub value: String,
        pub items: Option<Vec<String>>,
    }

    pub fn parse(raw: &str) -> Vec<RawField> {
        let lines: Vec<&str> = raw.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let Some((key, rest)) = key_line(line).filter(|_| indent(line) == 0) else {
                i += 1;
                continue;
            };
            i += 1;
            let mut body: Vec<&str> = Vec::new();
            while i < lines.len() && (lines[i].trim().is_empty() || indent(lines[i]) > 0 || lines[i].starts_with("- ")) {
                if lines[i].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                    break;
                }
                body.push(lines[i]);
                i += 1;
            }
            while body.last().is_some_and(|l| l.trim().is_empty()) {
                body.pop();
            }
            let trimmed: Vec<&str> = body.iter().map(|l| l.trim()).collect();
            if matches!(rest.as_str(), ">" | "|" | ">+" | ">-" | "|+" | "|-") {
                let text = trimmed.join("\n");
                let value = if rest.starts_with('|') { text } else { fold(&text) };
                out.push(RawField { key, value, items: None });
            } else if rest.is_empty() && !body.is_empty() && body.iter().all(|l| l.trim().is_empty() || l.trim_start().starts_with("- ")) {
                let items = body.iter().filter(|l| !l.trim().is_empty()).map(|l| unquote(l.trim_start().strip_prefix("- ").unwrap_or(l))).collect();
                out.push(RawField { key, value: String::new(), items: Some(items) });
            } else if rest.is_empty() {
                out.push(RawField { key, value: trimmed.join(", "), items: None });
            } else if rest.starts_with('[') && rest.ends_with(']') {
                out.push(RawField { key, value: String::new(), items: Some(flow_list(&rest)) });
            } else {
                let mut v = unquote(&rest);
                if !body.is_empty() {
                    v = format!("{v} {}", trimmed.join(" "));
                }
                out.push(RawField { key, value: v, items: None });
            }
        }
        out
    }

    /// A folded scalar: single newlines become spaces, blank lines one newline.
    fn fold(t: &str) -> String {
        let mut out = String::new();
        for (k, para) in t.split("\n\n").enumerate() {
            if k > 0 {
                out.push('\n');
            }
            out.push_str(&para.split('\n').collect::<Vec<_>>().join(" "));
        }
        out
    }

    pub fn block(f: &Split) -> Block {
        let mut title = String::new();
        let mut fields = Vec::new();
        for r in parse(&f.raw) {
            if r.key.eq_ignore_ascii_case("title") && r.items.is_none() && title.is_empty() {
                title = r.value;
                continue;
            }
            let link = |v: &str| (v.starts_with("http://") || v.starts_with("https://") || v.starts_with("mailto:")) && !v.contains(char::is_whitespace) && v.len() > v.find(':').unwrap_or(0) + 1;
            let kind = if r.items.is_some() {
                FieldKind::List
            } else if r.value.is_empty() {
                FieldKind::Empty
            } else if link(&r.value) {
                FieldKind::Link
            } else if r.value.chars().count() <= 32 && r.value.split_whitespace().count() <= 4 {
                FieldKind::Pill
            } else {
                FieldKind::Text
            };
            fields.push(Field { key: r.key, kind, value: r.value, items: r.items.unwrap_or_default() });
        }
        Block { kind: Kind::FrontMatter { title, fields }, lines: 0..f.end, key: (0, 0), pid: None }
    }
}

#[cfg(test)]
mod tests;
