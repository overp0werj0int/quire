//! The map: a note's Markdown as a tree of nodes, for Map mode (decision 0014,
//! `docs/reader-writer/behavior/map.md`, MAP-001 to MAP-012).
//!
//! The tree is an arena in preorder: the root at 0, every node before its descendants, and a
//! node's descendants in `idx + 1..node.end`. Each node keeps the source lines it came from, so
//! an edit made on the map can be spliced back into the note.

use std::hash::{Hash, Hasher};
use std::ops::Range;

use crate::blocks::{self, Block, Doc, Kind, Link, Run, Task};

pub type NodeId = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// The synthesized root of a note with no single leading H1 (MAP-002).
    Root,
    Heading { level: usize, setext: bool },
    /// A list item. `marker` is as written (`-`, `1.`, `2)`); `indent` is the marker's column and
    /// `content_col` the column its text starts at. `list` is the first line of its list, which
    /// tells two lists under one parent apart.
    Item { marker: String, ordered: bool, task: Task, indent: usize, content_col: usize, list: usize },
    /// A block that is not a heading or a list: a leaf, or the card a list item starts with.
    Block(BlockKind),
    /// The one group that gathers the note's footnote definitions (MAP-008).
    Footnotes,
    Footnote { number: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Paragraph,
    Code,
    Diff,
    Table,
    Image,
    Gallery,
    Quote,
    Alert,
    Html,
}

/// What a node draws (MAP-038, MAP-046 to MAP-052).
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// Its label as it is: a root named by its file or front matter, the footnotes group.
    Plain,
    /// Inline runs: a heading's, a paragraph's, a list item's or a footnote's text.
    Runs(Vec<Run>),
    /// A card: its block, drawn as the reader draws it.
    Card(Block),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    No,
    /// `<!-- markmap: fold -->`: the node starts folded.
    One,
    /// `<!-- markmap: foldAll -->`: the node and all below it start folded.
    All,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Plain text: a heading's or item's text, or a one-line summary of a card.
    pub label: String,
    /// The node's own source lines, 0-based, end exclusive. Empty for the virtual root.
    pub own: Range<usize>,
    /// From `own.start` through the end of the last descendant.
    pub span: Range<usize>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// One past the index of the node's last descendant.
    pub end: usize,
    pub depth: usize,
    /// The colour the node's branch draws in: 0 is the accent, k > 0 is palette entry k - 1.
    pub branch: usize,
    pub fold: Fold,
    pub body: Body,
    /// A hash of what the node draws: its own source lines and its label. Two nodes with the
    /// same `src` and tier are the same size.
    pub src: u64,
}

impl Node {
    /// A card, whose label is a summary of it rather than text from the note.
    pub fn card(&self) -> bool {
        matches!(self.body, Body::Card(_))
    }

    /// An image or a gallery card, whose size changes as its pictures load.
    pub fn pictures(&self) -> bool {
        matches!(&self.body, Body::Card(bl) if matches!(bl.kind, Kind::Image { .. } | Kind::Gallery { .. }))
    }
}

/// The way a map's branches grow from its root (MAP-079): the `layout` option's value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layout {
    #[default]
    Right,
    Left,
    /// The root's children split between the right and the left.
    Both,
    Down,
    Up,
}

impl Layout {
    pub const ALL: [Layout; 5] = [Layout::Right, Layout::Left, Layout::Both, Layout::Down, Layout::Up];

    /// The front matter value.
    pub fn word(self) -> &'static str {
        match self {
            Layout::Right => "right",
            Layout::Left => "left",
            Layout::Both => "both",
            Layout::Down => "down",
            Layout::Up => "up",
        }
    }

    pub fn from_word(w: &str) -> Option<Layout> {
        Layout::ALL.into_iter().find(|l| l.word().eq_ignore_ascii_case(w))
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::Right => "Right",
            Layout::Left => "Left",
            Layout::Both => "Both sides",
            Layout::Down => "Down",
            Layout::Up => "Up",
        }
    }
}

/// The `markmap:` options in fenced front matter that Quire honours (MAP-011), clamped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub initial_expand_level: Option<usize>,
    /// `Some(0.0)` turns wrapping off, up to the safety cap.
    pub max_width: Option<f32>,
    pub color_freeze_level: Option<usize>,
    pub spacing_horizontal: Option<f32>,
    pub spacing_vertical: Option<f32>,
    /// Right when the note names none, or one Quire does not know.
    pub layout: Layout,
}

#[derive(Clone, Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub opts: Options,
    /// Front matter fields other than `title` and `markmap`, for the properties badge (MAP-006).
    pub props: usize,
}

pub const ROOT: usize = 0;
const PALETTE_LEN: usize = 6;

impl Tree {
    pub fn root(&self) -> &Node {
        &self.nodes[ROOT]
    }

    pub fn find(&self, id: NodeId) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// The deepest node whose own lines hold `line`, else the deepest whose span does (MAP-065).
    /// A line past every span, such as the blank line at the note's end, takes the nearest
    /// node above it, so the caret at the end selects the last node, not the root.
    pub fn at_line(&self, line: usize) -> usize {
        let own = self.nodes.iter().rposition(|n| n.own.contains(&line));
        let span = || self.nodes.iter().rposition(|n| n.span.contains(&line) && !matches!(n.kind, NodeKind::Footnotes));
        let above = || {
            let starts = self.nodes.iter().enumerate().filter(|(_, n)| !n.own.is_empty() && n.own.start <= line);
            starts.max_by_key(|(_, n)| n.own.start).map(|(i, _)| i)
        };
        own.or_else(span).or_else(above).unwrap_or(ROOT)
    }

    /// The folds a note opens with: front matter's `initialExpandLevel`, `markmap:` directives
    /// and the footnotes group (MAP-007, MAP-008, MAP-042).
    pub fn initial_folds(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut all_until = 0;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.fold == Fold::All && i >= all_until {
                all_until = n.end;
            }
            let deep = self.opts.initial_expand_level.is_some_and(|k| n.depth + 1 >= k);
            let folded = deep || i < all_until || n.fold == Fold::One || n.kind == NodeKind::Footnotes;
            if folded && !n.children.is_empty() {
                out.push(n.id);
            }
        }
        out
    }
}

pub fn build(doc: &Doc, src: &str, name: &str) -> Tree {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut b = Builder { lines: &lines, nodes: Vec::new() };
    let front = doc.blocks.iter().find_map(|bl| match &bl.kind {
        Kind::FrontMatter { title, fields } => Some((title.clone(), fields.iter().filter(|f| !f.key.eq_ignore_ascii_case("markmap")).count())),
        _ => None,
    });
    let visible = |bl: &&Block| !matches!(bl.kind, Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. });
    let h1s = doc.blocks.iter().filter(|bl| matches!(bl.kind, Kind::Heading { level: 1, .. })).count();
    let first = doc.blocks.iter().position(|bl| visible(&bl));
    let h1_root = first.filter(|&i| h1s == 1 && matches!(doc.blocks[i].kind, Kind::Heading { level: 1, .. }));
    let mut stack: Vec<(usize, usize)> = Vec::new();
    match h1_root {
        Some(i) => {
            let r = b.heading(&doc.blocks[i], None);
            stack.push((1, r));
        }
        None => {
            let label = front.as_ref().map(|f| f.0.clone()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| crate::md::base(name).to_string());
            b.push(NodeKind::Root, label, 0..0, None, Body::Plain);
        }
    }
    for (i, bl) in doc.blocks.iter().enumerate() {
        if Some(i) == h1_root {
            continue;
        }
        match &bl.kind {
            Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => {}
            Kind::Heading { level, .. } => {
                while stack.last().is_some_and(|&(l, _)| l >= *level) {
                    stack.pop();
                }
                let parent = stack.last().map_or(ROOT, |&(_, n)| n);
                let n = b.heading(bl, Some(parent));
                stack.push((*level, n));
            }
            _ => {
                let parent = stack.last().map_or(ROOT, |&(_, n)| n);
                b.content(bl, parent);
            }
        }
    }
    if let Some(items) = doc.blocks.iter().find_map(|bl| match &bl.kind {
        Kind::Footnotes { items } => Some(items),
        _ => None,
    }) {
        let start = items.iter().map(|f| f.lines.start).min().unwrap_or(0);
        let g = b.push(NodeKind::Footnotes, "Footnotes".into(), start..start, Some(ROOT), Body::Plain);
        for f in items {
            let text = f.blocks.iter().find_map(text_of).unwrap_or_default();
            // Numbered as the preview numbers it, the number a link to its first citation.
            let cite = (f.refs > 0).then(|| Link::Anchor(format!("fnref-{}-1", f.number)));
            let mut runs = vec![Run { t: format!("{}.", f.number), link: cite, ..Run::default() }, Run { t: " ".into(), ..Run::default() }];
            runs.extend(f.blocks.iter().find_map(runs_of).unwrap_or_default().iter().cloned());
            b.push(NodeKind::Footnote { number: f.number }, format!("{}. {}", f.number, text).trim_end().to_string(), f.lines.clone(), Some(g), Body::Runs(runs));
        }
    }
    let mut t = Tree { nodes: b.nodes, opts: options(&lines), props: front.map_or(0, |f| f.1) };
    finish(&mut t, &lines);
    t
}

struct Builder<'a> {
    lines: &'a [&'a str],
    nodes: Vec<Node>,
}

impl Builder<'_> {
    fn push(&mut self, kind: NodeKind, label: String, own: Range<usize>, parent: Option<usize>, body: Body) -> usize {
        let i = self.nodes.len();
        let depth = parent.map_or(0, |p| self.nodes[p].depth + 1);
        let fold = match kind {
            NodeKind::Heading { .. } | NodeKind::Item { .. } => directive(&self.lines[own.start.min(self.lines.len())..own.end.min(self.lines.len())]),
            _ => Fold::No,
        };
        self.nodes.push(Node { id: 0, kind, label, span: own.clone(), own, parent, children: Vec::new(), end: i + 1, depth, branch: 0, fold, body, src: 0 });
        if let Some(p) = parent {
            self.nodes[p].children.push(i);
        }
        i
    }

    fn heading(&mut self, bl: &Block, parent: Option<usize>) -> usize {
        let Kind::Heading { level, text, runs, .. } = &bl.kind else { unreachable!("a heading block") };
        let first = self.lines.get(bl.lines.start).copied().unwrap_or("");
        let setext = !first.trim_start().starts_with('#');
        self.push(NodeKind::Heading { level: *level, setext }, text.clone(), bl.lines.clone(), parent, Body::Runs(runs.clone()))
    }

    /// A block under a heading, the root or a list item: a list's items, or one leaf.
    fn content(&mut self, bl: &Block, parent: usize) {
        match &bl.kind {
            Kind::List(l) => {
                for it in &l.items {
                    self.item(it, l.ordered, bl.lines.start, parent);
                }
            }
            Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => {}
            // A heading inside a list item is a leaf of that item, not a section.
            Kind::Heading { .. } => {
                self.heading(bl, Some(parent));
            }
            _ => {
                let (kind, label, body) = leaf(bl);
                self.push(NodeKind::Block(kind), label, bl.lines.clone(), Some(parent), body);
            }
        }
    }

    fn item(&mut self, it: &blocks::Item, ordered: bool, list: usize, parent: usize) {
        let line = self.lines.get(it.line).copied().unwrap_or("");
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let rest = line.trim_start_matches([' ', '\t']);
        let mlen = if ordered { rest.bytes().take_while(u8::is_ascii_digit).count() + 1 } else { 1 };
        let marker = rest.get(..mlen).unwrap_or("-").to_string();
        let gap = rest.get(mlen..).map_or(1, |r| r.chars().take_while(|c| *c == ' ').count().clamp(1, 4));
        let content_col = indent + mlen + gap;
        let (label, own_end, rest_from, body) = match it.blocks.first() {
            Some(first) => match text_of(first) {
                Some(t) => (t, first.lines.end, 1, Body::Runs(runs_of(first).unwrap_or_default().to_vec())),
                None if matches!(first.kind, Kind::List(_)) => (String::new(), it.line + 1, 0, Body::Plain),
                None => {
                    let (_, l, body) = leaf(first);
                    (l, first.lines.end, 1, body)
                }
            },
            None => (String::new(), it.line + 1, 0, Body::Plain),
        };
        let own = it.line..own_end.max(it.line + 1);
        let n = self.push(NodeKind::Item { marker, ordered, task: it.task, indent, content_col, list }, label, own, Some(parent), body);
        for bl in &it.blocks[rest_from..] {
            self.content(bl, n);
        }
    }
}

/// A heading's or paragraph's text, or None for a card.
fn text_of(bl: &Block) -> Option<String> {
    match &bl.kind {
        Kind::Paragraph { runs } => Some(blocks::plain(runs).trim().to_string()),
        Kind::Heading { text, .. } => Some(text.clone()),
        _ => None,
    }
}

/// A heading's or paragraph's inline runs, or None for a card.
fn runs_of(bl: &Block) -> Option<&[Run]> {
    match &bl.kind {
        Kind::Paragraph { runs } | Kind::Heading { runs, .. } => Some(runs),
        _ => None,
    }
}

/// A leaf's kind, its label (a summary for a card) and what it draws.
fn leaf(bl: &Block) -> (BlockKind, String, Body) {
    let (kind, label) = summary(bl);
    let body = match &bl.kind {
        Kind::Paragraph { runs } => Body::Runs(runs.clone()),
        Kind::List(_) | Kind::Heading { .. } | Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => Body::Plain,
        _ => Body::Card(bl.clone()),
    };
    (kind, label, body)
}

fn summary(bl: &Block) -> (BlockKind, String) {
    let first_text = |bs: &[Block]| bs.iter().find_map(text_of).unwrap_or_default();
    let n_lines = |t: &str| t.lines().count().max(1);
    let plural = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    match &bl.kind {
        Kind::Paragraph { runs } => (BlockKind::Paragraph, blocks::plain(runs).trim().to_string()),
        Kind::Code { lang, text } => {
            let lang = if lang.is_empty() { String::new() } else { format!(" · {lang}") };
            (BlockKind::Code, format!("code{lang} · {}", plural(n_lines(text), "line")))
        }
        Kind::Diff { lang, added, removed, .. } => {
            let lang = if lang.is_empty() { String::new() } else { format!(" · {lang}") };
            (BlockKind::Diff, format!("diff{lang} · +{added} −{removed}"))
        }
        Kind::Table(t) => (BlockKind::Table, format!("table · {} · {}", plural(t.header.len(), "column"), plural(t.rows.len(), "row"))),
        Kind::Image { img, .. } => (BlockKind::Image, image_label(img)),
        Kind::Gallery { items } => (BlockKind::Gallery, format!("gallery · {}", plural(items.len(), "image"))),
        Kind::Quote { blocks } => (BlockKind::Quote, format!("quote · {}", first_text(blocks)).trim_end_matches([' ', '·']).to_string()),
        Kind::Alert { kind, blocks } => (BlockKind::Alert, format!("{} · {}", kind.title(), first_text(blocks)).trim_end_matches([' ', '·']).to_string()),
        Kind::Html { text } => (BlockKind::Html, format!("html · {}", plural(n_lines(text), "line"))),
        Kind::List(_) | Kind::Heading { .. } | Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => (BlockKind::Paragraph, String::new()),
    }
}

fn image_label(img: &crate::md::Img) -> String {
    if !img.alt.trim().is_empty() {
        return img.alt.trim().to_string();
    }
    let path = img.src.split(['?', '#']).next().unwrap_or(&img.src);
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() { "image".into() } else { name.to_string() }
}

/// The fold a `<!-- markmap: fold -->` or `<!-- markmap: foldAll -->` comment in `lines` asks for.
fn directive(lines: &[&str]) -> Fold {
    let mut out = Fold::No;
    for l in lines {
        let mut rest = *l;
        while let Some(at) = rest.find("<!--") {
            rest = &rest[at + 4..];
            let Some(close) = rest.find("-->") else { break };
            let inner = rest[..close].trim();
            rest = &rest[close + 3..];
            match inner.strip_prefix("markmap:").map(str::trim) {
                Some("foldAll") => return Fold::All,
                Some("fold") => out = Fold::One,
                _ => {}
            }
        }
    }
    out
}

/// `markmap:` options from fenced front matter (MAP-011): an indented `key: value` block.
fn options(lines: &[&str]) -> Options {
    let mut o = Options::default();
    let end = front_end(lines);
    if end == 0 {
        return o;
    }
    let body = &lines[1..end - 1];
    let Some(at) = body.iter().position(|l| markmap_key(l) == Some("")) else { return o };
    for l in body[at + 1..].iter().take_while(|l| l.starts_with([' ', '\t']) || l.trim().is_empty()) {
        let Some((k, v)) = l.trim().split_once(':') else { continue };
        let v = scalar(v);
        if k.trim() == "layout" {
            o.layout = Layout::from_word(v).unwrap_or_default();
            continue;
        }
        let Ok(n) = v.parse::<f64>() else { continue };
        if !n.is_finite() {
            continue;
        }
        match k.trim() {
            "initialExpandLevel" => o.initial_expand_level = (n >= 0.0).then(|| n.min(64.0) as usize),
            "maxWidth" => o.max_width = Some(if n <= 0.0 { 0.0 } else { n.clamp(40.0, 1200.0) as f32 }),
            "colorFreezeLevel" => o.color_freeze_level = Some(n.clamp(0.0, 64.0) as usize),
            "spacingHorizontal" => o.spacing_horizontal = Some(n.clamp(0.0, 400.0) as f32),
            "spacingVertical" => o.spacing_vertical = Some(n.clamp(0.0, 200.0) as f32),
            _ => {}
        }
    }
    o
}

/// What front matter line `l` gives the `markmap` key, None when it is another line: empty
/// when an indented block of options follows, else options written on the line itself.
fn markmap_key(l: &str) -> Option<&str> {
    let rest = l.strip_prefix("markmap:")?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| scalar(rest))
}

/// A front matter value without its quotes or the ` # comment` after it.
fn scalar(v: &str) -> &str {
    let v = v.trim();
    if let Some(q) = v.chars().next().filter(|c| matches!(c, '"' | '\''))
        && let Some(n) = v[1..].find(q)
    {
        return &v[1..1 + n];
    }
    let cut = v.char_indices().find(|&(k, c)| c == '#' && (k == 0 || v[..k].ends_with(char::is_whitespace)));
    v[..cut.map_or(v.len(), |(k, _)| k)].trim_end()
}

/// The lines of a note's fenced front matter, empty when it has none.
pub fn front_lines(src: &str) -> Range<usize> {
    0..front_end(&src.split('\n').collect::<Vec<_>>())
}

/// The line after fenced front matter's closing fence, or 0 when the note has none.
fn front_end(lines: &[&str]) -> usize {
    if lines.first().map(|l| l.trim_start_matches('\u{feff}').trim_end_matches('\r')) != Some("---") {
        return 0;
    }
    lines.iter().skip(1).position(|l| matches!(l.trim_end_matches('\r'), "---" | "...")).map_or(0, |c| c + 2)
}

/// Spans, descendant ends, ids and branch colours, once every node is in place.
fn finish(t: &mut Tree, lines: &[&str]) {
    let n = t.nodes.len();
    for i in (0..n).rev() {
        let (mut end, mut last) = (i + 1, t.nodes[i].span.end);
        for &c in &t.nodes[i].children {
            end = end.max(t.nodes[c].end);
            last = last.max(t.nodes[c].span.end);
        }
        let node = &mut t.nodes[i];
        node.end = end;
        node.span.end = last;
        if node.kind == NodeKind::Root || node.kind == NodeKind::Footnotes {
            node.span.start = node.own.start.min(node.span.end);
        }
    }
    for node in &mut t.nodes {
        let text = lines.get(node.own.clone()).unwrap_or(&[]);
        node.src = hash(&(&node.label, text));
    }
    // Ids: the parent's id, the node's kind and content, and its ordinal among equal siblings.
    t.nodes[ROOT].id = hash(&("root", 0u8));
    for i in 0..n {
        let parent_id = t.nodes[i].id;
        let mut seen: Vec<(u64, u32)> = Vec::new();
        for c in t.nodes[i].children.clone() {
            let content = content_hash(&t.nodes[c], lines);
            let ord = match seen.iter_mut().find(|s| s.0 == content) {
                Some(s) => {
                    s.1 += 1;
                    s.1
                }
                None => {
                    seen.push((content, 0));
                    0
                }
            };
            t.nodes[c].id = hash(&(parent_id, content, ord));
        }
    }
    // Branch colours (MAP-040): `colorFreezeLevel` is 1-based, 0 cycles every node.
    let freeze = t.opts.color_freeze_level.unwrap_or(2);
    let mut next = 0;
    for i in 1..n {
        let d = t.nodes[i].depth;
        if freeze == 0 || d < freeze {
            t.nodes[i].branch = 1 + next % PALETTE_LEN;
            next += 1;
        } else {
            let p = t.nodes[i].parent.unwrap_or(ROOT);
            t.nodes[i].branch = t.nodes[p].branch;
        }
    }
}

fn content_hash(n: &Node, lines: &[&str]) -> u64 {
    match &n.kind {
        NodeKind::Block(k) => {
            let text: Vec<&str> = lines.get(n.own.clone()).unwrap_or(&[]).iter().map(|l| l.trim_end()).collect();
            hash(&("block", k, text))
        }
        NodeKind::Heading { .. } => hash(&("heading", &n.label)),
        NodeKind::Item { .. } => hash(&("item", &n.label)),
        NodeKind::Footnote { .. } => hash(&("footnote", &n.label)),
        NodeKind::Footnotes => hash(&"footnotes"),
        NodeKind::Root => hash(&"root"),
    }
}

fn hash(v: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

// ---------- Rewrites: a gesture on the map as a splice of the note's lines (MAP-013 to MAP-025) ----------

pub const LEAF_CHILD: &str = "Only headings and list items can have children";
pub const ROOT_DELETE: &str = "The map's root can't be deleted";
pub const FOOTNOTES: &str = "Edit footnotes in Source or the Edit Sheet";

/// Why a gesture changes nothing: a toast, or nothing to say (the footnotes, MAP-008).
pub type Refused = Option<&'static str>;

/// Old lines `lines` become `with`. `focus` is the line in the new text whose node the map
/// selects next, or None for the root. Lines are the text split at `\n`, so the last one is
/// empty when the text ends in a newline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    pub lines: Range<usize>,
    pub with: Vec<String>,
    pub focus: Option<usize>,
    /// Where each old line went, for a move: its lines leave one place and land in another.
    pub moved: Option<Vec<Option<usize>>>,
}

impl Splice {
    pub fn apply(&self, src: &str) -> String {
        let old: Vec<&str> = src.split('\n').collect();
        let mut out: Vec<&str> = old[..self.lines.start].to_vec();
        out.extend(self.with.iter().map(String::as_str));
        out.extend_from_slice(&old[self.lines.end..]);
        out.join("\n")
    }

    /// Where old line `l` is in the new text, or None when the splice dropped it.
    pub fn line_after(&self, l: usize) -> Option<usize> {
        if let Some(m) = &self.moved {
            return m.get(l).copied().flatten();
        }
        if l < self.lines.start {
            Some(l)
        } else if l >= self.lines.end {
            Some(l + self.with.len() - self.lines.len())
        } else {
            (l - self.lines.start < self.with.len()).then_some(l)
        }
    }

    /// The new text's lines the splice wrote.
    pub fn written(&self) -> Range<usize> {
        self.lines.start..self.lines.start + self.with.len()
    }
}

/// The splice from `old` lines to `new` lines: the lines both start and end with stay out.
fn splice_of(old: &[&str], new: &[String], focus: Option<usize>) -> Splice {
    let pre = old.iter().zip(new).take_while(|(a, b)| **a == b.as_str()).count();
    let room = old.len().min(new.len()) - pre;
    let post = old.iter().rev().zip(new.iter().rev()).take(room).take_while(|(a, b)| **a == b.as_str()).count();
    Splice { lines: pre..old.len() - post, with: new[pre..new.len() - post].to_vec(), focus, moved: None }
}

pub const INLINE_OPTIONS: &str = "Change this note's layout in Source: its markmap options are on one line";

/// The splice that makes the note say `layout` (MAP-089): one `layout:` line under `markmap:`
/// in its front matter, which it adds, with `markmap:` or the whole fence if need be. Bare
/// front matter (BLK-85) gets the fence round it, so its fields stay front matter. Right is
/// the default, so it takes the line away, then an emptied `markmap:`, then an emptied front
/// matter: Down and then Right give back the bytes the note began with. Refused with no word
/// when the note already lays out that way.
pub fn layout_splice(src: &str, layout: Layout) -> Result<Splice, Refused> {
    const BOM: char = '\u{feff}';
    let old: Vec<&str> = src.split('\n').collect();
    if options(&old).layout == layout {
        return Err(None);
    }
    let eol = if old[0].ends_with('\r') { "\r" } else { "" };
    let mut new: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    // A byte order mark stays the note's first character, before the fence.
    let bom = new[0].starts_with(BOM);
    if bom {
        new[0].remove(0);
    }
    let (fenced, bare) = {
        let lines: Vec<&str> = new.iter().map(|l| l.trim_end_matches('\r')).collect();
        (front_end(&lines), crate::blocks::front_matter::bare_len(&lines).unwrap_or(0))
    };
    let fe = if fenced == 0 {
        new.insert(bare, format!("---{eol}"));
        new.insert(0, format!("---{eol}"));
        bare + 2
    } else {
        fenced
    };
    let inner = 1..fe - 1;
    let head = inner.clone().find_map(|i| markmap_key(&new[i]).map(|v| (i, v.is_empty())));
    if matches!(head, Some((_, false))) {
        return Err(Some(INLINE_OPTIONS));
    }
    let block = head.map_or(inner.end..inner.end, |(at, _)| {
        at + 1..(at + 1..inner.end).find(|&i| !(new[i].starts_with([' ', '\t']) || blank(&new[i]))).unwrap_or(inner.end)
    });
    let owns: Vec<usize> = block.clone().filter(|&i| new[i].trim().split_once(':').is_some_and(|(k, _)| k.trim() == "layout")).collect();
    let indent = |l: &str| l[..l.len() - l.trim_start().len()].to_string();
    let set = |indent: &str| format!("{indent}layout: {}{eol}", layout.word());
    let mut gone = owns.clone();
    if layout == Layout::Right {
        if let Some((at, _)) = head
            && block.clone().all(|i| owns.contains(&i) || blank(&new[i]))
        {
            gone.push(at);
        }
        // Nothing but the fence would be left.
        if gone.len() == fe - 2 {
            gone = (0..fe).collect();
        }
    } else if let Some(&first) = owns.first() {
        new[first] = set(&indent(&new[first]));
        gone.remove(0);
    } else {
        let last = block.clone().rev().find(|&i| !blank(&new[i]));
        let (at, indent) = match (last, head) {
            (Some(i), _) => (i + 1, indent(&new[i])),
            (None, Some((at, _))) => (at + 1, "  ".to_string()),
            (None, None) => {
                new.insert(inner.end, format!("markmap:{eol}"));
                (inner.end + 1, "  ".to_string())
            }
        };
        new.insert(at, set(&indent));
    }
    let mut new: Vec<String> = new.into_iter().enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, l)| l).collect();
    if bom && let Some(first) = new.first_mut() {
        first.insert(0, BOM);
    }
    Ok(splice_of(&old, &new, None))
}

/// The splice between two texts, for undo and redo (MAP-014).
pub fn diff(old: &str, new: &str) -> Splice {
    let o: Vec<&str> = old.split('\n').collect();
    let n: Vec<String> = new.split('\n').map(str::to_string).collect();
    let s = splice_of(&o, &n, None);
    Splice { focus: Some(s.lines.start), ..s }
}

/// Node `id` of `old` in `new` after splice `s` (MAP-010): the same id when it survives, else
/// the node of the same sort that now starts where it started.
pub fn follow(old: &Tree, new: &Tree, s: &Splice, id: NodeId) -> Option<NodeId> {
    if new.find(id).is_some() {
        return Some(id);
    }
    let n = &old.nodes[old.find(id)?];
    if n.own.is_empty() {
        return new.nodes.iter().find(|m| m.own.is_empty() && sort(&m.kind) == sort(&n.kind)).map(|m| m.id);
    }
    let l = s.line_after(n.own.start)?;
    new.nodes.iter().find(|m| m.own.start == l && !m.own.is_empty() && sort(&m.kind) == sort(&n.kind)).map(|m| m.id)
}

fn sort(k: &NodeKind) -> std::mem::Discriminant<NodeKind> {
    std::mem::discriminant(k)
}

fn blank(l: &str) -> bool {
    l.trim().is_empty()
}

/// The columns a line's leading whitespace takes, tabs to the next multiple of 4, and its bytes.
fn lead(l: &str) -> (usize, usize) {
    let mut col = 0;
    for (b, c) in l.char_indices() {
        match c {
            ' ' => col += 1,
            '\t' => col = col / 4 * 4 + 4,
            _ => return (col, b),
        }
    }
    (col, l.len())
}

/// `l` with its indent moved by `by` columns, at least to 0, tabs written as spaces (MAP-024).
fn shift(l: &str, by: isize) -> String {
    if blank(l) || by == 0 {
        return l.to_string();
    }
    let (col, b) = lead(l);
    format!("{}{}", " ".repeat((col as isize + by).max(0) as usize), &l[b..])
}

/// `l` less up to `cols` columns of indent; a tab cut part way leaves spaces (MAP-016).
fn dedent(l: &str, cols: usize) -> String {
    let mut col = 0;
    for (b, c) in l.char_indices() {
        if col >= cols {
            return l[b..].to_string();
        }
        match c {
            ' ' => col += 1,
            '\t' => {
                let next = col / 4 * 4 + 4;
                if next > cols {
                    return format!("{}{}", " ".repeat(next - cols), &l[b + 1..]);
                }
                col = next;
            }
            _ => return l[b..].to_string(),
        }
    }
    String::new()
}

/// Where a label line's kept end starts: a `markmap:` directive, an ATX heading's closing
/// `#`s and trailing whitespace stay as written (MAP-016).
fn tail_at(body: &str, atx: bool) -> usize {
    let mut end = body.trim_end().len();
    let s = &body[..end];
    if let Some(inner) = s.strip_suffix("-->")
        && let Some(open) = inner.rfind("<!--")
        && inner[open + 4..].trim().starts_with("markmap:")
    {
        end = s[..open].trim_end().len();
    }
    if atx {
        let s = &body[..end];
        let before = s.trim_end_matches('#');
        if before.len() < s.len() && (before.is_empty() || before.ends_with([' ', '\t'])) {
            end = before.trim_end().len();
        }
    }
    end
}

/// The marker's number on an ordered item's line.
fn number(l: &str) -> Option<u64> {
    let t = l.trim_start();
    t[..t.bytes().take_while(u8::is_ascii_digit).count()].parse().ok()
}

/// A node's text as the inline field holds it: lines `lines` less `head` on the first, `tail`
/// on the last and up to `col` columns of indent on the others (MAP-016).
struct Text {
    lines: Range<usize>,
    head: String,
    tail: String,
    /// The indent read off continuation lines, and the one written back.
    col: (usize, usize),
    bodies: Vec<String>,
}

impl Text {
    fn read(old: &[&str], lines: Range<usize>, head_len: usize, col: (usize, usize), atx: bool) -> Text {
        let first = old[lines.start];
        let head = first[..head_len].to_string();
        let mut bodies: Vec<String> = old[lines.clone()].iter().enumerate().map(|(k, l)| if k == 0 { first[head_len..].to_string() } else { dedent(l, col.0) }).collect();
        let last = bodies.last_mut().expect("a node has a line");
        let cut = tail_at(last, atx);
        let tail = last[cut..].to_string();
        last.truncate(cut);
        Text { lines, head, tail, col, bodies }
    }

    /// The lines that write `text` back, keeping every line whose text is unchanged as it was.
    fn write(&self, old: &[&str], text: &[&str]) -> Vec<String> {
        let head = if self.head.is_empty() || self.head.ends_with([' ', '\t']) || text[0].is_empty() { self.head.clone() } else { format!("{} ", self.head) };
        let (n, last) = (text.len(), self.bodies.len() - 1);
        text.iter()
            .enumerate()
            .map(|(k, t)| {
                if k <= last && self.bodies[k] == *t && (k == n - 1) == (k == last) {
                    return old[self.lines.start + k].to_string();
                }
                let start = if k == 0 { head.clone() } else { " ".repeat(self.col.1) };
                let end = if k == n - 1 { self.tail.as_str() } else { "" };
                format!("{start}{t}{end}")
            })
            .collect()
    }
}

/// How editing node `i` in place goes (MAP-016, MAP-019, MAP-020).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InPlace {
    /// The inline field, holding this text; `multi` takes Shift+Enter as a new line.
    Field { text: String, multi: bool },
    /// The Edit Sheet on these lines.
    Sheet(Range<usize>),
    /// The note's file name, in the title bar.
    FileName,
    Nothing,
}

pub fn in_place(src: &str, t: &Tree, i: usize) -> InPlace {
    let old: Vec<&str> = src.split('\n').collect();
    let n = &t.nodes[i];
    match &n.kind {
        NodeKind::Root if title_line(&old).is_some() || !has_h1(t) => InPlace::Field { text: n.label.clone(), multi: false },
        NodeKind::Root => InPlace::FileName,
        NodeKind::Footnotes => InPlace::Nothing,
        NodeKind::Footnote { .. } => InPlace::Sheet(n.own.clone()),
        _ if n.card() => InPlace::Sheet(n.own.clone()),
        NodeKind::Block(_) | NodeKind::Heading { .. } | NodeKind::Item { .. } => match text(&old, t, i) {
            Some(x) => InPlace::Field { text: x.bodies.join("\n"), multi: !matches!(n.kind, NodeKind::Heading { .. }) },
            None => InPlace::Sheet(n.own.clone()),
        },
    }
}

/// Node `i`'s text, or None when it is not a text node the inline field can hold.
fn text(old: &[&str], t: &Tree, i: usize) -> Option<Text> {
    let n = &t.nodes[i];
    let first = old.get(n.own.start)?;
    let (_, b) = lead(first);
    match &n.kind {
        NodeKind::Heading { setext: false, .. } => {
            let h = b + first[b..].bytes().take_while(|&c| c == b'#').count();
            let ws = first[h..].len() - first[h..].trim_start().len();
            Some(Text::read(old, n.own.clone(), h + ws, (0, 0), true))
        }
        NodeKind::Heading { setext: true, .. } if n.own.len() == 2 => Some(Text::read(old, n.own.start..n.own.start + 1, b, (0, 0), false)),
        NodeKind::Heading { .. } => None,
        NodeKind::Item { ordered, content_col, .. } => {
            let rest = &first[b..];
            let m = if *ordered { rest.bytes().take_while(u8::is_ascii_digit).count() + 1 } else { 1 };
            let after = &rest[m.min(rest.len())..];
            let mut h = b + m + (after.len() - after.trim_start().len());
            let boxed = &first[h..];
            if boxed.len() >= 3 && matches!(&boxed[..3], "[ ]" | "[x]" | "[X]") && boxed[3..].starts_with([' ', '\t']) {
                h += 3 + (boxed[3..].len() - boxed[3..].trim_start().len());
            }
            if first[h..].trim().is_empty() && n.own.len() > 1 {
                return None;
            }
            Some(Text::read(old, n.own.clone(), h, (*content_col, *content_col), false))
        }
        NodeKind::Block(BlockKind::Paragraph) => {
            let col = match n.parent.map(|p| &t.nodes[p].kind) {
                Some(NodeKind::Item { content_col, .. }) => (*content_col, *content_col),
                _ => (usize::MAX, 0),
            };
            Some(Text::read(old, n.own.clone(), b, col, false))
        }
        _ => None,
    }
}

fn has_h1(t: &Tree) -> bool {
    t.root().children.iter().any(|&c| matches!(t.nodes[c].kind, NodeKind::Heading { level: 1, .. }))
}

/// Front matter's `title:` line and the lines its value runs on to.
fn title_line(old: &[&str]) -> Option<Range<usize>> {
    let end = front_end(old);
    let at = (1..end.saturating_sub(1)).find(|&l| !old[l].starts_with([' ', '\t']) && old[l].split_once(':').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("title")))?;
    let more = old[at + 1..end - 1].iter().take_while(|l| l.starts_with([' ', '\t'])).count();
    Some(at..at + 1 + more)
}

/// The text typed in the field as lines: blank lines would split the node, so they go, and a
/// heading's lines join into one.
fn typed(text: &str, multi: bool) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.split('\n').filter(|l| !blank(l)).collect();
    if lines.is_empty() {
        return None;
    }
    Some(if multi { lines.iter().map(|l| l.to_string()).collect() } else { vec![lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ")] })
}

/// Node `i`'s text becomes `text` (MAP-016 to MAP-019). None when nothing changes: the same
/// text, an empty one, or a root named by the file.
pub fn rename(src: &str, t: &Tree, i: usize, text_in: &str) -> Option<Splice> {
    let old: Vec<&str> = src.split('\n').collect();
    let n = &t.nodes[i];
    let multi = !matches!(n.kind, NodeKind::Heading { .. } | NodeKind::Root);
    let lines = typed(text_in, multi)?;
    if n.kind == NodeKind::Root {
        let label = &lines[0];
        if *label == n.label {
            return None;
        }
        let mut new: Vec<String> = old.iter().map(|l| l.to_string()).collect();
        if let Some(r) = title_line(&old) {
            let key = old[r.start].split_once(':').map_or("title", |(k, _)| k);
            let value = label.replace('\\', "\\\\").replace('"', "\\\"");
            new.splice(r.clone(), [format!("{key}: \"{value}\"")]);
            return Some(splice_of(&old, &new, None));
        }
        if has_h1(t) {
            return None;
        }
        let fe = front_end(&old);
        let real = old.len() - usize::from(src.ends_with('\n'));
        if let Some(at) = (fe..real).find(|&l| !blank(old[l])) {
            new.splice(at..at, [format!("# {label}"), String::new()]);
            return Some(splice_of(&old, &new, Some(at)));
        }
        new.splice(fe..new.len(), [format!("# {label}"), String::new()]);
        return Some(splice_of(&old, &new, Some(fe)));
    }
    let x = text(&old, t, i)?;
    let typed: Vec<&str> = lines.iter().map(String::as_str).collect();
    if typed == x.bodies {
        return None;
    }
    let mut new: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    new.splice(x.lines.clone(), x.write(&old, &typed));
    Some(splice_of(&old, &new, Some(n.own.start)))
}

/// Where a new node goes, relative to node `.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Add {
    Child(usize),
    After(usize),
    Before(usize),
}

impl Add {
    /// The node the place is relative to.
    pub fn node(self) -> usize {
        match self {
            Add::Child(j) | Add::After(j) | Add::Before(j) => j,
        }
    }

    /// The node whose child the new node is: `Enter` on the root adds a child (MAP-023).
    fn child_of(self) -> Option<usize> {
        match self {
            Add::Child(i) | Add::After(i @ ROOT) => Some(i),
            _ => None,
        }
    }
}

/// What a new node is written as: the types the add toolbar offers (MAP-087).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewKind {
    Paragraph,
    Heading(usize),
    Bullet,
    Number,
    Task,
}

impl NewKind {
    const ALL: [NewKind; 10] = [
        NewKind::Paragraph,
        NewKind::Heading(1),
        NewKind::Heading(2),
        NewKind::Heading(3),
        NewKind::Heading(4),
        NewKind::Heading(5),
        NewKind::Heading(6),
        NewKind::Bullet,
        NewKind::Number,
        NewKind::Task,
    ];

    fn bit(self) -> u16 {
        match self {
            NewKind::Paragraph => 1,
            NewKind::Heading(l) => 1 << l.min(6),
            NewKind::Bullet => 1 << 7,
            NewKind::Number => 1 << 8,
            NewKind::Task => 1 << 9,
        }
    }

    /// The kind of an existing node, for a sibling like it.
    fn of(k: &NodeKind) -> NewKind {
        match k {
            NodeKind::Heading { level, .. } => NewKind::Heading(*level),
            NodeKind::Item { ordered: true, .. } => NewKind::Number,
            NodeKind::Item { task: Task::None, .. } => NewKind::Bullet,
            NodeKind::Item { .. } => NewKind::Task,
            _ => NewKind::Paragraph,
        }
    }

    /// Whether node kind `k` is one of these.
    fn is(self, k: &NodeKind) -> bool {
        match (self, k) {
            (NewKind::Paragraph, NodeKind::Block(BlockKind::Paragraph)) => true,
            (NewKind::Heading(l), NodeKind::Heading { level, .. }) => l == *level,
            (NewKind::Number, NodeKind::Item { ordered, .. }) => *ordered,
            (NewKind::Bullet, NodeKind::Item { ordered, task, .. }) => !*ordered && *task == Task::None,
            (NewKind::Task, NodeKind::Item { ordered, task, .. }) => !*ordered && *task != Task::None,
            _ => false,
        }
    }

    /// Item `k`'s list takes an item of this kind: bullets and tasks share one, numbers don't.
    fn joins(self, k: &NodeKind) -> bool {
        match (self, k) {
            (NewKind::Bullet | NewKind::Task, NodeKind::Item { ordered, .. }) => !*ordered,
            (NewKind::Number, NodeKind::Item { ordered, .. }) => *ordered,
            _ => false,
        }
    }
}

/// The kinds a new node can take at one place, and the level its Heading button picks (MAP-087).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    set: u16,
    pub heading: Option<usize>,
}

impl Kinds {
    pub fn has(self, k: NewKind) -> bool {
        self.set & k.bit() != 0
    }
}

/// A new node: its first line's `head`, the indent of its other lines (None: one line only),
/// the line it goes in at, whether blank lines set it apart, and the ordered items after it
/// whose numbers go up by one.
struct Plan {
    at: usize,
    head: String,
    col: Option<usize>,
    blank: bool,
    bump: Vec<usize>,
}

/// The kind a new node at `at` is until another is picked (MAP-021, MAP-023): a sibling's own
/// kind; under a list item, an item like its other children or a bullet; under a heading or the
/// root, a heading when the section goes on in subheadings, else a paragraph.
pub fn default_kind(t: &Tree, at: Add) -> Result<NewKind, Refused> {
    let i = at.node();
    let n = &t.nodes[i];
    let Some(i) = at.child_of() else {
        return match n.kind {
            _ if i == ROOT => Err(None),
            NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(None),
            _ => Ok(NewKind::of(&n.kind)),
        };
    };
    let kids = kids_but(t, i, usize::MAX);
    match &n.kind {
        NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(None),
        NodeKind::Block(_) => Err(Some(LEAF_CHILD)),
        NodeKind::Heading { .. } if !takes_children(t, i) => Err(Some(LEAF_CHILD)),
        NodeKind::Item { .. } => Ok(kids.iter().rev().find(|&&c| is_item(t, c)).map_or(NewKind::Bullet, |&c| NewKind::of(&t.nodes[c].kind))),
        NodeKind::Root => Ok(kids.iter().map(|&c| level(t, c)).filter(|&l| l > 0).min().map_or(NewKind::Paragraph, NewKind::Heading)),
        NodeKind::Heading { .. } => Ok(match kids.iter().rev().find(|&&c| level(t, c) > 0 || is_item(t, c)) {
            Some(&c) if level(t, c) > 0 => NewKind::Heading(level(t, c)),
            _ => NewKind::Paragraph,
        }),
    }
}

/// The kinds a new node at `at` can take (MAP-087): each that goes in as a node at that place
/// without changing what any other node is.
pub fn kinds(src: &str, t: &Tree, at: Add) -> Kinds {
    let old: Vec<&str> = src.split('\n').collect();
    let set = NewKind::ALL.iter().filter(|&&k| matches!(place(src, &old, t, at, k, "\u{200b}"), Ok((_, _, true)))).fold(0, |s, k| s | k.bit());
    let i = at.node();
    let above = std::iter::successors(Some(i), |&k| t.nodes[k].parent).map(|k| level(t, k)).find(|&l| l > 0).unwrap_or(0);
    let near = match at.child_of() {
        Some(i) => kids_but(t, i, usize::MAX).iter().rev().map(|&c| level(t, c)).find(|&l| l > 0).unwrap_or(above + 1),
        None if level(t, i) > 0 => above,
        None => above + 1,
    };
    let heading = (1..=6).filter(|&l| set & NewKind::Heading(l).bit() != 0).min_by_key(|&l| (l.abs_diff(near), std::cmp::Reverse(l)));
    Kinds { set, heading }
}

/// A new node of kind `kind` with text `text` (MAP-021 to MAP-024).
pub fn add(src: &str, t: &Tree, at: Add, kind: NewKind, text: &str) -> Result<Splice, Refused> {
    let old: Vec<&str> = src.split('\n').collect();
    let (new, focus, _) = place(src, &old, t, at, kind, text)?;
    Ok(splice_of(&old, &new, Some(focus)))
}

/// The note's lines with a new node in them, the line it starts on, and whether it went in as
/// meant: a node of its kind at its place in the tree, every other node as it was.
fn place(src: &str, old: &[&str], t: &Tree, at: Add, kind: NewKind, text: &str) -> Result<(Vec<String>, usize, bool), Refused> {
    let p = plan(old, t, at, kind)?;
    let lines = typed(text, p.col.is_some()).ok_or(None)?;
    let mut base: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    for &j in &p.bump {
        renumber(&mut base, t, j, 1);
    }
    let node: Vec<String> = lines.iter().enumerate().map(|(k, l)| if k == 0 { format!("{}{l}", p.head) } else { format!("{}{l}", " ".repeat(p.col.unwrap_or(0))) }).collect();
    let real = old.len() - usize::from(src.ends_with('\n'));
    let build = |apart: bool| {
        let before = apart && p.at > 0 && !blank(&base[p.at - 1]);
        let after = apart && p.at < real && !blank(&base[p.at]);
        let mut new = base.clone();
        let mut ins = Vec::new();
        ins.extend(before.then(String::new));
        ins.extend(node.iter().cloned());
        ins.extend(after.then(String::new));
        new.splice(p.at..p.at, ins);
        (new, p.at + usize::from(before))
    };
    let depth = at.child_of().map_or(t.nodes[at.node()].depth, |i| t.nodes[i].depth + 1);
    let fits = |(new, focus): &(Vec<String>, usize)| {
        let t2 = reparse(new);
        let Some(k) = t2.nodes.iter().position(|m| m.own.start == *focus && !m.own.is_empty()) else { return false };
        if t2.nodes[k].depth != depth || !kind.is(&t2.nodes[k].kind) {
            return false;
        }
        let mut s = shape(&t2);
        s.remove(k - 1);
        s == shape(t)
    };
    let first = build(p.blank);
    if fits(&first) {
        return Ok((first.0, first.1, true));
    }
    match (!p.blank).then(|| build(true)).filter(|b| fits(b)) {
        Some((new, focus)) => Ok((new, focus, true)),
        None => Ok((first.0, first.1, false)),
    }
}

/// A new block of `text`, such as an image's link (MAP-048). Under a heading or the root it is
/// a paragraph after the section's own blocks and before its first subheading, and after a
/// heading inside a list item it is a paragraph under that heading, where a typed node would
/// be a heading or a list item. Anywhere else it goes in as `add` puts a node of the default kind.
pub fn add_block(src: &str, t: &Tree, at: Add, text: &str) -> Result<Splice, Refused> {
    let kind = match (at, &t.nodes[at.node()].kind) {
        (Add::Child(i), NodeKind::Root | NodeKind::Heading { .. }) if takes_children(t, i) => NewKind::Paragraph,
        (Add::After(_), NodeKind::Heading { .. }) => NewKind::Paragraph,
        _ => default_kind(t, at)?,
    };
    add(src, t, at, kind, text)
}

fn reparse(lines: &[String]) -> Tree {
    let src = lines.join("\n");
    build(&blocks::build(&src), &src, "")
}

/// Every node but the root as its sort, depth and label, to tell a rewrite that changed what
/// the lines around it mean.
fn shape(t: &Tree) -> Vec<(std::mem::Discriminant<NodeKind>, usize, &str)> {
    t.nodes[1..].iter().map(|n| (sort(&n.kind), n.depth, n.label.as_str())).collect()
}

fn plan(old: &[&str], t: &Tree, at: Add, kind: NewKind) -> Result<Plan, Refused> {
    match (at, at.child_of()) {
        (_, Some(i)) => child_plan(old, t, i, kind),
        (Add::Before(ROOT), _) => Err(None),
        (Add::After(i) | Add::Before(i), _) => sibling_plan(old, t, i, matches!(at, Add::Before(_)), kind),
        (Add::Child(_), None) => unreachable!("a child has a parent"),
    }
}

fn is_item(t: &Tree, i: usize) -> bool {
    matches!(t.nodes[i].kind, NodeKind::Item { .. })
}

/// The items of node `j`'s list, in order.
fn list_of(t: &Tree, j: usize) -> Vec<usize> {
    let NodeKind::Item { list, .. } = t.nodes[j].kind else { return vec![j] };
    let p = t.nodes[j].parent.unwrap_or(ROOT);
    t.nodes[p].children.iter().copied().filter(|&c| matches!(t.nodes[c].kind, NodeKind::Item { list: l, .. } if l == list)).collect()
}

/// The list counts up by one from its first number, so inserts and deletes renumber it (MAP-024).
fn counts(old: &[&str], t: &Tree, items: &[usize]) -> bool {
    let nums: Vec<Option<u64>> = items.iter().map(|&j| number(old[t.nodes[j].own.start])).collect();
    nums.iter().enumerate().all(|(k, n)| n.is_some() && *n == nums[0].map(|f| f + k as u64))
}

/// Blank lines part the list's items (BLK-22).
fn loose(old: &[&str], t: &Tree, items: &[usize]) -> bool {
    items.iter().skip(1).any(|&j| blank(old[t.nodes[j].own.start - 1]))
}

/// Item `j`'s number moves by `by`; its other lines move with the marker's width (MAP-024).
fn renumber(lines: &mut [String], t: &Tree, j: usize, by: i64) {
    let n = &t.nodes[j];
    let l = &lines[n.own.start];
    let (_, b) = lead(l);
    let digits = l[b..].bytes().take_while(u8::is_ascii_digit).count();
    let Ok(num) = l[b..b + digits].parse::<i64>() else { return };
    let new = (num + by).max(0).to_string();
    let delta = new.len() as isize - digits as isize;
    lines[n.own.start] = format!("{}{new}{}", &l[..b], &l[b + digits..]);
    for l in &mut lines[n.span.start + 1..n.span.end] {
        *l = shift(l, delta);
    }
}

/// A new item of `kind` after or before item `j` of the same list, in at line `at` (MAP-023,
/// MAP-024).
fn item_plan(old: &[&str], t: &Tree, j: usize, at: usize, before: bool, kind: NewKind) -> Plan {
    let n = &t.nodes[j];
    let NodeKind::Item { marker, ordered, task, indent, content_col, .. } = &n.kind else { unreachable!("an item") };
    let line = old[n.own.start];
    let (_, b) = lead(line);
    let items = list_of(t, j);
    let k = items.iter().position(|&c| c == j).unwrap_or(0);
    let counting = *ordered && counts(old, t, &items);
    let marker = match number(line) {
        Some(num) if counting => format!("{}{}", num + u64::from(!before), &marker[marker.len() - 1..]),
        _ => marker.clone(),
    };
    let gap = content_col.saturating_sub(indent + marker_len(&n.kind)).max(1);
    // A numbered task list keeps its boxes.
    let boxed = if kind == NewKind::Task || (*ordered && *task != Task::None) { "[ ] " } else { "" };
    let head = format!("{}{marker}{}{boxed}", &line[..b], " ".repeat(gap));
    let bump = if counting { items[if before { k } else { k + 1 }..].to_vec() } else { Vec::new() };
    let tight_spot = if before { at == n.own.start } else { at == n.span.end };
    Plan { at, head, col: Some(indent + marker.len() + gap), blank: !tight_spot || loose(old, t, &items), bump }
}

/// A list of one new item of `kind` at `indent` (`col` columns), with bullet `bullet` unless
/// numbered.
fn new_list(indent: &str, col: usize, kind: NewKind, bullet: &str, at: usize, blank: bool) -> Plan {
    let marker = if kind == NewKind::Number { "1." } else { bullet };
    let boxed = if kind == NewKind::Task { "[ ] " } else { "" };
    Plan { at, head: format!("{indent}{marker} {boxed}"), col: Some(col + marker.len() + 1), blank, bump: Vec::new() }
}

fn marker_len(k: &NodeKind) -> usize {
    match k {
        NodeKind::Item { marker, .. } => marker.len(),
        _ => 0,
    }
}

/// Whether node `i` can hold a child: the root, items, and headings outside items (MAP-021).
pub fn takes_children(t: &Tree, i: usize) -> bool {
    let n = &t.nodes[i];
    match n.kind {
        NodeKind::Root | NodeKind::Item { .. } => true,
        NodeKind::Heading { .. } => !n.parent.is_some_and(|p| is_item(t, p)),
        _ => false,
    }
}

/// A new child of node `i`. A paragraph or list goes after the section's own blocks and before
/// its first subheading; a heading after the whole branch (MAP-021).
fn child_plan(old: &[&str], t: &Tree, i: usize, kind: NewKind) -> Result<Plan, Refused> {
    let n = &t.nodes[i];
    match &n.kind {
        NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(None),
        NodeKind::Block(_) => Err(Some(LEAF_CHILD)),
        NodeKind::Heading { .. } if !takes_children(t, i) => Err(Some(LEAF_CHILD)),
        NodeKind::Item { marker, ordered, content_col, .. } => {
            let pad = " ".repeat(*content_col);
            match kind {
                NewKind::Heading(_) => Err(None),
                NewKind::Paragraph => Ok(Plan { at: n.span.end, head: pad, col: Some(*content_col), blank: true, bump: Vec::new() }),
                _ => Ok(match n.children.iter().rev().copied().find(|&c| is_item(t, c)) {
                    Some(last) if kind.joins(&t.nodes[last].kind) => item_plan(old, t, last, n.span.end, false, kind),
                    _ => new_list(&pad, *content_col, kind, if *ordered { "-" } else { marker }, n.span.end, false),
                }),
            }
        }
        NodeKind::Heading { .. } | NodeKind::Root => {
            let kids = kids_but(t, i, usize::MAX);
            let own = if n.own.is_empty() { front_end(old) } else { n.own.end };
            let end = kids.last().map_or(own, |&c| t.nodes[c].span.end);
            let text = kids.iter().take_while(|&&c| level(t, c) == 0).last().copied();
            let text_end = text.map_or_else(|| kids.first().map_or(own, |&c| t.nodes[c].own.start), |c| t.nodes[c].span.end);
            Ok(match kind {
                NewKind::Paragraph => Plan { at: text_end, head: String::new(), col: Some(0), blank: true, bump: Vec::new() },
                NewKind::Heading(l) => Plan { at: end, head: format!("{} ", "#".repeat(l)), col: None, blank: true, bump: Vec::new() },
                _ => match text {
                    Some(j) if kind.joins(&t.nodes[j].kind) => item_plan(old, t, j, text_end, false, kind),
                    _ => new_list("", 0, kind, "-", text_end, true),
                },
            })
        }
    }
}

/// A new sibling of node `i`, after its branch or before it (MAP-023).
fn sibling_plan(old: &[&str], t: &Tree, i: usize, before: bool, kind: NewKind) -> Result<Plan, Refused> {
    let n = &t.nodes[i];
    let at = if before { n.own.start } else { n.span.end };
    let (indent, col) = match &n.kind {
        NodeKind::Root | NodeKind::Footnotes | NodeKind::Footnote { .. } => return Err(None),
        NodeKind::Heading { .. } | NodeKind::Item { .. } => {
            let line = old[n.own.start];
            let (col, b) = lead(line);
            (line[..b].to_string(), col)
        }
        NodeKind::Block(_) => match n.parent.map(|p| &t.nodes[p].kind) {
            Some(NodeKind::Item { content_col, .. }) => (" ".repeat(*content_col), *content_col),
            _ => (String::new(), 0),
        },
    };
    Ok(match kind {
        NewKind::Heading(l) => Plan { at, head: format!("{indent}{} ", "#".repeat(l)), col: None, blank: true, bump: Vec::new() },
        NewKind::Paragraph => Plan { at, head: indent, col: Some(col), blank: true, bump: Vec::new() },
        _ if kind.joins(&n.kind) => item_plan(old, t, i, at, before, kind),
        _ => new_list(&indent, col, kind, "-", at, true),
    })
}

/// Node `i` and everything under it go (MAP-025), with one blank line beside it when blank
/// lines or the file's ends bound it on both sides. Also the number of nodes deleted.
pub fn delete(src: &str, t: &Tree, i: usize) -> Result<(Splice, usize), Refused> {
    let n = &t.nodes[i];
    match n.kind {
        _ if i == ROOT => return Err(Some(ROOT_DELETE)),
        NodeKind::Footnotes | NodeKind::Footnote { .. } => return Err(Some(FOOTNOTES)),
        _ => {}
    }
    let old: Vec<&str> = src.split('\n').collect();
    let real = old.len() - usize::from(src.ends_with('\n'));
    let mut base: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    if matches!(n.kind, NodeKind::Item { ordered: true, .. }) {
        let items = list_of(t, i);
        if counts(&old, t, &items) {
            for &j in items.iter().skip_while(|&&j| j != i).skip(1) {
                renumber(&mut base, t, j, -1);
            }
        }
    }
    let span = n.span.clone();
    let mut r = span.clone();
    if (r.start == 0 || blank(old[r.start - 1])) && (r.end >= real || blank(old[r.end])) {
        if r.end < real {
            r.end += 1;
        } else if r.start > 0 {
            r.start -= 1;
        }
    }
    let p = n.parent.unwrap_or(ROOT);
    let sibs = &t.nodes[p].children;
    let k = sibs.iter().position(|&c| c == i).unwrap_or(0);
    let focus = |gone: usize| match (sibs.get(k + 1), k.checked_sub(1).map(|k| sibs[k])) {
        (Some(&next), _) => Some(t.nodes[next].own.start - gone),
        (None, Some(prev)) => Some(t.nodes[prev].own.start),
        (None, None) if t.nodes[p].own.is_empty() => None,
        (None, None) => Some(t.nodes[p].own.start),
    };
    let cut = |r: &Range<usize>, keep: bool| {
        let mut new = base.clone();
        new.splice(r.clone(), keep.then(String::new));
        new
    };
    let mut want = shape(t);
    want.drain(i - 1..n.end - 1);
    let tries = [(r.clone(), false), (span.clone(), false), (span.clone(), true)];
    let (r, keep) = tries.iter().find(|(r, keep)| shape(&reparse(&cut(r, *keep))) == want).unwrap_or(&tries[0]).clone();
    let gone = r.len() - usize::from(keep);
    Ok((splice_of(&old, &cut(&r, keep), focus(gone)), n.end - i))
}

/// An item's task box ticks or clears (MAP-053).
pub fn toggle_task(src: &str, t: &Tree, i: usize) -> Option<Splice> {
    let n = &t.nodes[i];
    let NodeKind::Item { task, .. } = n.kind else { return None };
    let new = blocks::toggle_task(src, n.own.start, task == Task::Todo)?;
    let old: Vec<&str> = src.split('\n').collect();
    let new: Vec<String> = new.split('\n').map(str::to_string).collect();
    Some(splice_of(&old, &new, Some(n.own.start)))
}

// ---------- Moves: a node and everything under it go elsewhere (MAP-026 to MAP-031) ----------

pub const HEADINGS_ONLY: &str = "A heading moves only among headings";
pub const TEXT_ABOVE: &str = "Text and lists stay above subheadings";
pub const NOT_IN_ITEM: &str = "A heading can't go inside a list item";
pub const PAST_H6: &str = "Headings stop at H6";
pub const ROOT_MOVE: &str = "The map's root can't move";
pub const STRUCTURE: &str = "That move would change the note's structure";
pub const NOT_HEADING: &str = "Only headings have a level";
const FIRST: &str = "Already first";
const LAST: &str = "Already last";
const OUTERMOST: &str = "Already at the top level";
const NONE_ABOVE: &str = "Nothing above to go under";

/// A heading that starts a section, not one written inside a list item.
fn section(t: &Tree, i: usize) -> bool {
    matches!(t.nodes[i].kind, NodeKind::Heading { .. }) && !t.nodes[i].parent.is_some_and(|p| is_item(t, p))
}

fn level(t: &Tree, i: usize) -> usize {
    match t.nodes[i].kind {
        NodeKind::Heading { level, .. } => level,
        _ => 0,
    }
}

/// Node `p`'s children but `s` and the footnotes group.
fn kids_but(t: &Tree, p: usize, s: usize) -> Vec<usize> {
    t.nodes[p].children.iter().copied().filter(|&c| c != s && t.nodes[c].kind != NodeKind::Footnotes).collect()
}

fn movable(t: &Tree, s: usize) -> Result<(), Refused> {
    match t.nodes[s].kind {
        _ if s == ROOT => Err(Some(ROOT_MOVE)),
        NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(Some(FOOTNOTES)),
        _ => Ok(()),
    }
}

/// Whether node `s` may go to `at` (MAP-027 to MAP-030), and the levels its headings shift by.
/// None for a place a drop cancels at: on the node itself or under it.
pub fn zone(t: &Tree, s: usize, at: Add) -> Result<isize, Refused> {
    movable(t, s)?;
    let j = at.node();
    if (s..t.nodes[s].end).contains(&j) || matches!(at, Add::Before(ROOT) | Add::After(ROOT)) {
        return Err(None);
    }
    if matches!(t.nodes[j].kind, NodeKind::Footnotes | NodeKind::Footnote { .. }) {
        return Err(Some(FOOTNOTES));
    }
    if !section(t, s) {
        let first = |j: usize| kids_but(t, t.nodes[j].parent.unwrap_or(ROOT), s).into_iter().find(|&c| section(t, c)) == Some(j);
        return match at {
            Add::Child(j) if !takes_children(t, j) => Err(Some(LEAF_CHILD)),
            Add::Before(j) if section(t, j) && !first(j) => Err(Some(TEXT_ABOVE)),
            Add::After(j) if section(t, j) => Err(Some(TEXT_ABOVE)),
            _ => Ok(0),
        };
    }
    let to = match at {
        Add::Child(j) if is_item(t, j) => return Err(Some(NOT_IN_ITEM)),
        Add::Child(j) if !takes_children(t, j) => return Err(Some(LEAF_CHILD)),
        Add::Child(ROOT) if t.nodes[ROOT].kind == NodeKind::Root => kids_but(t, ROOT, s).into_iter().filter(|&c| section(t, c)).map(|c| level(t, c)).min().unwrap_or(level(t, s)),
        Add::Child(j) => level(t, j) + 1,
        Add::Before(j) | Add::After(j) if !section(t, j) => return Err(Some(HEADINGS_ONLY)),
        Add::Before(j) | Add::After(j) => level(t, j),
    };
    let delta = to as isize - level(t, s) as isize;
    shifts(t, s, delta)?;
    Ok(delta)
}

/// Refused when heading `s` or a section heading under it would go past H6.
fn shifts(t: &Tree, s: usize, delta: isize) -> Result<(), Refused> {
    let deepest = (s..t.nodes[s].end).filter(|&k| k == s || section(t, k)).map(|k| level(t, k)).max().unwrap_or(0);
    if deepest as isize + delta > 6 { Err(Some(PAST_H6)) } else { Ok(()) }
}

/// A keyboard move (MAP-031).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Up,
    Down,
    Out,
    In,
}

/// Where a keyboard move takes node `s`: before the sibling above, after the one below, after
/// its parent, or under the sibling above (MAP-031).
pub fn step(t: &Tree, s: usize, dir: Step) -> Result<Add, Refused> {
    movable(t, s)?;
    let p = t.nodes[s].parent.unwrap_or(ROOT);
    let sibs: Vec<usize> = t.nodes[p].children.iter().copied().filter(|&c| t.nodes[c].kind != NodeKind::Footnotes).collect();
    let k = sibs.iter().position(|&c| c == s).unwrap_or(0);
    let above = k.checked_sub(1).map(|k| sibs[k]);
    match dir {
        Step::Up => above.map(Add::Before).ok_or(Some(FIRST)),
        Step::Down => sibs.get(k + 1).map(|&c| Add::After(c)).ok_or(Some(LAST)),
        Step::Out if p == ROOT => Err(Some(OUTERMOST)),
        Step::Out => Ok(Add::After(p)),
        Step::In => above.map(Add::Child).ok_or(Some(NONE_ABOVE)),
    }
}

/// An item's line read as its parts: the indent and its columns, the marker, the gap after it
/// (1 to 4 spaces, as CommonMark takes it) and the text from there.
struct Head<'a> {
    indent: &'a str,
    cols: usize,
    marker: &'a str,
    gap: usize,
    text: &'a str,
}

impl Head<'_> {
    fn read(l: &str) -> Head<'_> {
        let (cols, b) = lead(l);
        let rest = &l[b..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let m = (if digits > 0 { digits + 1 } else { 1 }).min(rest.len());
        let sp = rest[m..].bytes().take_while(|&c| c == b' ').count();
        let gap = if (1..=4).contains(&sp) { sp } else { 1 };
        Head { indent: &l[..b], cols, marker: &rest[..m], gap, text: &rest[m + gap.min(sp)..] }
    }

    fn content_col(&self) -> usize {
        self.cols + self.marker.len() + self.gap
    }

    fn ordered(&self) -> bool {
        self.marker.starts_with(|c: char| c.is_ascii_digit())
    }
}

/// The column node `i`'s children are written at: an item's content column, else 0.
fn content_col(lines: &[String], t: &Tree, i: usize) -> usize {
    if is_item(t, i) { Head::read(&lines[t.nodes[i].own.start]).content_col() } else { 0 }
}

/// Where a moved node lands: its new parent, the siblings it lands between, the old line it
/// goes in at, and the index in the old tree its subtree takes.
struct Slot {
    parent: usize,
    prev: Option<usize>,
    next: Option<usize>,
    at: usize,
    pos: usize,
}

fn slot(old: &[&str], t: &Tree, s: usize, at: Add) -> Slot {
    let j = at.node();
    let n = &t.nodes[j];
    match at {
        Add::Before(_) | Add::After(_) => {
            let parent = n.parent.unwrap_or(ROOT);
            let ks = kids_but(t, parent, s);
            let k = ks.iter().position(|&c| c == j).unwrap_or(0);
            match at {
                Add::Before(_) => Slot { parent, prev: k.checked_sub(1).map(|k| ks[k]), next: Some(j), at: n.own.start, pos: j },
                _ => Slot { parent, prev: Some(j), next: ks.get(k + 1).copied(), at: n.span.end, pos: n.end },
            }
        }
        Add::Child(_) => {
            let ks = kids_but(t, j, s);
            let sub = if section(t, s) || is_item(t, j) { None } else { ks.iter().position(|&c| section(t, c)) };
            match sub {
                Some(k) => Slot { parent: j, prev: k.checked_sub(1).map(|k| ks[k]), next: Some(ks[k]), at: t.nodes[ks[k]].own.start, pos: ks[k] },
                None => {
                    let at = ks.last().map_or(if n.own.is_empty() { front_end(old) } else { n.own.end }, |&c| t.nodes[c].span.end);
                    let pos = n.children.iter().copied().find(|&c| t.nodes[c].kind == NodeKind::Footnotes).unwrap_or(n.end);
                    Slot { parent: j, prev: ks.last().copied(), next: None, at, pos }
                }
            }
        }
    }
}

/// Lines with the old line each came from, None for a line the rewrite added.
type Traced = Vec<(String, Option<usize>)>;

/// Heading `s`'s lines with it and the section headings under it `delta` levels deeper: ATX
/// keeps its form, setext becomes ATX unless `delta` is 0, and every other line stays as it is.
fn relevel(lines: &[String], t: &Tree, s: usize, delta: isize) -> Traced {
    let n = &t.nodes[s];
    let heads: Vec<usize> = (s..n.end).filter(|&k| k == s || section(t, k)).collect();
    let mut out = Vec::new();
    let mut l = n.span.start;
    while l < n.span.end {
        let Some(&k) = heads.iter().find(|&&k| t.nodes[k].own.start == l && delta != 0) else {
            out.push((lines[l].clone(), Some(l)));
            l += 1;
            continue;
        };
        let h = &t.nodes[k];
        let to = "#".repeat((level(t, k) as isize + delta) as usize);
        let first = &lines[l];
        let line = match h.kind {
            NodeKind::Heading { setext: false, .. } => {
                let (_, b) = lead(first);
                let hashes = first[b..].bytes().take_while(|&c| c == b'#').count();
                format!("{}{to}{}", &first[..b], &first[b + hashes..])
            }
            _ => format!("{to} {}", lines[l..h.own.end - 1].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ")),
        };
        out.push((line, Some(l)));
        l = h.own.end;
    }
    out
}

/// Item `s` as it lands at `slot`: its marker line takes the list it joins (or starts), its
/// other lines move with the content column, and the ordered lists on both sides renumber
/// (MAP-024, MAP-028). Also whether blank lines set it apart.
fn item_lines(base: &mut [String], old: &[&str], t: &Tree, s: usize, at: Add, slot: &Slot) -> (Traced, bool) {
    let n = &t.nodes[s];
    let item = |i: Option<usize>| i.filter(|&i| is_item(t, i));
    let last_item = || if is_item(t, slot.parent) { kids_but(t, slot.parent, s).into_iter().rev().find(|&c| is_item(t, c)) } else { None };
    let join = match at {
        Add::Before(j) if is_item(t, j) => Some((j, false)),
        Add::After(j) if is_item(t, j) => Some((j, true)),
        _ => item(slot.prev).map(|k| (k, true)).or_else(|| item(slot.next).map(|k| (k, false))).or_else(|| last_item().map(|k| (k, true))),
    };
    let ordered = |items: &[usize]| matches!(t.nodes[items[0]].kind, NodeKind::Item { ordered: true, .. }) && counts(old, t, items);
    let mut renumber_to = |items: &[usize], fin: &[usize]| {
        let start = number(old[t.nodes[items[0]].own.start]).unwrap_or(1);
        for (k, &c) in fin.iter().enumerate().filter(|&(_, &c)| c != s) {
            let have = number(old[t.nodes[c].own.start]).unwrap_or(start);
            renumber(base, t, c, (start + k as u64) as i64 - have as i64);
        }
        start
    };
    let mut num = None;
    let own = list_of(t, s);
    if let Some((k, after)) = join {
        let items = list_of(t, k);
        let mut fin: Vec<usize> = items.iter().copied().filter(|&c| c != s).collect();
        let p = fin.iter().position(|&c| c == k).map_or(fin.len(), |p| p + usize::from(after));
        fin.insert(p, s);
        if ordered(&items) {
            num = Some(renumber_to(&items, &fin) + p as u64);
        }
    }
    if !join.is_some_and(|(k, _)| own.contains(&k)) && ordered(&own) {
        let fin: Vec<usize> = own.iter().copied().filter(|&c| c != s).collect();
        renumber_to(&own, &fin);
    }
    let me = Head::read(&base[n.own.start]);
    let (indent, marker, gap) = match join {
        Some((k, _)) => {
            let h = Head::read(&base[t.nodes[k].own.start]);
            let marker = num.map_or_else(|| h.marker.to_string(), |v| format!("{v}{}", &h.marker[h.marker.len() - 1..]));
            (h.indent.to_string(), marker, h.gap)
        }
        None if is_item(t, slot.parent) => {
            let h = Head::read(&base[t.nodes[slot.parent].own.start]);
            (" ".repeat(h.content_col()), if h.ordered() { "-".to_string() } else { h.marker.to_string() }, 1)
        }
        None if me.ordered() => (String::new(), format!("1{}", &me.marker[me.marker.len() - 1..]), me.gap),
        None => (String::new(), me.marker.to_string(), me.gap),
    };
    let by = (lead(&indent).0 + marker.len() + gap) as isize - me.content_col() as isize;
    let first = if me.text.is_empty() { format!("{indent}{marker}") } else { format!("{indent}{marker}{}{}", " ".repeat(gap), me.text) };
    let mut out = vec![(first, Some(n.own.start))];
    out.extend((n.own.start + 1..n.span.end).map(|l| (shift(&base[l], by), Some(l))));
    let apart = join.is_none_or(|(k, after)| {
        let next_to = if after { slot.at == t.nodes[k].span.end } else { slot.at == t.nodes[k].own.start };
        !next_to || loose(old, t, &list_of(t, k))
    });
    (out, apart)
}

/// Node `s` and everything under it move to `at` (MAP-026 to MAP-030), as one splice whose
/// `moved` says where each line went, so folds and the selection follow (MAP-010).
pub fn move_to(src: &str, t: &Tree, s: usize, at: Add) -> Result<Splice, Refused> {
    let delta = zone(t, s, at)?;
    let old: Vec<&str> = src.split('\n').collect();
    let real = old.len() - usize::from(src.ends_with('\n'));
    let n = &t.nodes[s];
    let slot = slot(&old, t, s, at);
    let mut base: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    let (body, apart) = match n.kind {
        NodeKind::Heading { .. } if section(t, s) => (relevel(&base, t, s, delta), true),
        NodeKind::Item { .. } => item_lines(&mut base, &old, t, s, at, &slot),
        _ => {
            let by = content_col(&base, t, slot.parent) as isize - content_col(&base, t, n.parent.unwrap_or(ROOT)) as isize;
            (n.span.clone().map(|l| (shift(&base[l], by), Some(l))).collect(), true)
        }
    };
    let mut want = shape(t);
    let sub: Vec<_> = want.drain(s - 1..n.end - 1).collect();
    let down = t.nodes[slot.parent].depth as isize + 1 - n.depth as isize;
    let pos = if slot.pos >= n.end { slot.pos - (n.end - s) } else { slot.pos };
    want.splice(pos - 1..pos - 1, sub.into_iter().map(|(k, d, l)| (k, (d as isize + down) as usize, l)));
    let build = |r: &Range<usize>, keep: bool, apart: bool| {
        let at = if r.contains(&slot.at) { r.start } else { slot.at };
        let mut out: Traced = Vec::with_capacity(base.len() + body.len() + 2);
        let mut ins = 0;
        for l in 0..=base.len() {
            if l == at {
                ins = out.len();
                out.extend(body.iter().cloned());
            }
            if l == base.len() {
                break;
            }
            if r.contains(&l) {
                if keep && l == r.start {
                    out.push((String::new(), None));
                }
                continue;
            }
            out.push((base[l].clone(), Some(l)));
        }
        let end = ins + body.len();
        if apart && end < out.len() - usize::from(src.ends_with('\n')) && !blank(&out[end].0) {
            out.insert(end, (String::new(), None));
        }
        if apart && ins > 0 && !blank(&out[ins - 1].0) {
            out.insert(ins, (String::new(), None));
            ins += 1;
        }
        (out, ins)
    };
    let span = n.span.clone();
    let mut r = span.clone();
    if (r.start == 0 || blank(old[r.start - 1])) && (r.end >= real || blank(old[r.end])) {
        if r.end < real {
            r.end += 1;
        } else if r.start > 0 {
            r.start -= 1;
        }
    }
    let tries = [(r, false), (span.clone(), false), (span, true)];
    let seps: &[bool] = if apart { &[true] } else { &[false, true] };
    let (out, focus) = tries
        .iter()
        .flat_map(|(r, keep)| seps.iter().map(move |&apart| (r, *keep, apart)))
        .map(|(r, keep, apart)| build(r, keep, apart))
        .find(|(out, _)| shape(&reparse(&out.iter().map(|l| l.0.clone()).collect::<Vec<_>>())) == want)
        .ok_or(Some(STRUCTURE))?;
    let new: Vec<String> = out.iter().map(|l| l.0.clone()).collect();
    if new.iter().map(String::as_str).eq(old.iter().copied()) {
        return Err(None);
    }
    Ok(traced(&old, &new, &out, focus))
}

/// The splice from `old` to `new`, which carries the old line each of its lines came from.
fn traced(old: &[&str], new: &[String], out: &Traced, focus: usize) -> Splice {
    let mut moved = vec![None; old.len()];
    for (k, (_, from)) in out.iter().enumerate() {
        if let Some(l) = from {
            moved[*l] = Some(k);
        }
    }
    Splice { moved: Some(moved), ..splice_of(old, new, Some(focus)) }
}

/// Ctrl+Alt+1 to 6 (MAP-031): heading `i` takes level `to`, and the section headings under it
/// shift with it.
pub fn set_level(src: &str, t: &Tree, i: usize, to: usize) -> Result<Splice, Refused> {
    if !matches!(t.nodes[i].kind, NodeKind::Heading { .. }) {
        return Err(Some(NOT_HEADING));
    }
    let delta = to as isize - level(t, i) as isize;
    if delta == 0 {
        return Err(None);
    }
    shifts(t, i, delta)?;
    let old: Vec<&str> = src.split('\n').collect();
    let base: Vec<String> = old.iter().map(|l| l.to_string()).collect();
    let span = t.nodes[i].span.clone();
    let mut out: Traced = (0..span.start).map(|l| (base[l].clone(), Some(l))).collect();
    out.extend(relevel(&base, t, i, delta));
    out.extend((span.end..base.len()).map(|l| (base[l].clone(), Some(l))));
    let new: Vec<String> = out.iter().map(|l| l.0.clone()).collect();
    Ok(traced(&old, &new, &out, span.start))
}

/// Node `i`'s Markdown as written: its own lines, or with `branch` everything under it too
/// (MAP-059). The virtual root's own text is its label.
pub fn markdown(src: &str, t: &Tree, i: usize, branch: bool) -> String {
    let n = &t.nodes[i];
    let lines = match (&n.kind, branch) {
        (NodeKind::Root, false) => return n.label.clone(),
        (NodeKind::Footnotes, _) | (_, true) => n.span.clone(),
        _ => n.own.clone(),
    };
    src.split('\n').skip(lines.start).take(lines.len()).collect::<Vec<_>>().join("\n")
}
