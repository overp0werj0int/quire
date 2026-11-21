//! The map: a note's Markdown as a tree of nodes, for Map mode (decision 0014,
//! `docs/reader-writer/behavior/map.md`, MAP-001 to MAP-012).
//!
//! The tree is an arena in preorder: the root at 0, every node before its descendants, and a
//! node's descendants in `idx + 1..node.end`. Each node keeps the source lines it came from, so
//! an edit made on the map can be spliced back into the note.

use std::hash::{Hash, Hasher};
use std::ops::Range;

use crate::blocks::{self, Block, Doc, Kind, Task};

pub type NodeId = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// The synthesized root of a note with no single leading H1 (MAP-002).
    Root,
    Heading { level: usize, setext: bool },
    /// A list item. `marker` is as written (`-`, `1.`, `2)`); `indent` is the marker's column and
    /// `content_col` the column its text starts at.
    Item { marker: String, ordered: bool, task: Task, indent: usize, content_col: usize },
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
    /// The label is a summary of a card rather than text from the note.
    pub card: bool,
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

    /// The deepest node whose own lines hold `line`, else the deepest whose span does, else
    /// the root (MAP-065).
    pub fn at_line(&self, line: usize) -> usize {
        let own = self.nodes.iter().rposition(|n| n.own.contains(&line));
        let span = || self.nodes.iter().rposition(|n| n.span.contains(&line) && !matches!(n.kind, NodeKind::Footnotes));
        own.or_else(span).unwrap_or(ROOT)
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
            b.push(NodeKind::Root, label, 0..0, None, false);
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
        let g = b.push(NodeKind::Footnotes, "Footnotes".into(), start..start, Some(ROOT), false);
        for f in items {
            let text = f.blocks.iter().find_map(text_of).unwrap_or_default();
            b.push(NodeKind::Footnote { number: f.number }, format!("[^{}] {}", f.label, text).trim_end().to_string(), f.lines.clone(), Some(g), false);
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
    fn push(&mut self, kind: NodeKind, label: String, own: Range<usize>, parent: Option<usize>, card: bool) -> usize {
        let i = self.nodes.len();
        let depth = parent.map_or(0, |p| self.nodes[p].depth + 1);
        let fold = match kind {
            NodeKind::Heading { .. } | NodeKind::Item { .. } => directive(&self.lines[own.start.min(self.lines.len())..own.end.min(self.lines.len())]),
            _ => Fold::No,
        };
        self.nodes.push(Node { id: 0, kind, label, span: own.clone(), own, parent, children: Vec::new(), end: i + 1, depth, branch: 0, fold, card });
        if let Some(p) = parent {
            self.nodes[p].children.push(i);
        }
        i
    }

    fn heading(&mut self, bl: &Block, parent: Option<usize>) -> usize {
        let Kind::Heading { level, text, .. } = &bl.kind else { unreachable!("a heading block") };
        let first = self.lines.get(bl.lines.start).copied().unwrap_or("");
        let setext = !first.trim_start().starts_with('#');
        self.push(NodeKind::Heading { level: *level, setext }, text.clone(), bl.lines.clone(), parent, false)
    }

    /// A block under a heading, the root or a list item: a list's items, or one leaf.
    fn content(&mut self, bl: &Block, parent: usize) {
        match &bl.kind {
            Kind::List(l) => {
                for it in &l.items {
                    self.item(it, l.ordered, parent);
                }
            }
            Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => {}
            // A heading inside a list item is a leaf of that item, not a section.
            Kind::Heading { .. } => {
                self.heading(bl, Some(parent));
            }
            _ => {
                let (kind, label, card) = leaf(bl);
                self.push(NodeKind::Block(kind), label, bl.lines.clone(), Some(parent), card);
            }
        }
    }

    fn item(&mut self, it: &blocks::Item, ordered: bool, parent: usize) {
        let line = self.lines.get(it.line).copied().unwrap_or("");
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let rest = line.trim_start_matches([' ', '\t']);
        let mlen = if ordered { rest.bytes().take_while(u8::is_ascii_digit).count() + 1 } else { 1 };
        let marker = rest.get(..mlen).unwrap_or("-").to_string();
        let gap = rest.get(mlen..).map_or(1, |r| r.chars().take_while(|c| *c == ' ').count().clamp(1, 4));
        let content_col = indent + mlen + gap;
        let (label, own_end, rest_from, card) = match it.blocks.first() {
            Some(first) => match text_of(first) {
                Some(t) => (t, first.lines.end, 1, false),
                None if matches!(first.kind, Kind::List(_)) => (String::new(), it.line + 1, 0, false),
                None => {
                    let (_, l, _) = leaf(first);
                    (l, first.lines.end, 1, true)
                }
            },
            None => (String::new(), it.line + 1, 0, false),
        };
        let own = it.line..own_end.max(it.line + 1);
        let n = self.push(NodeKind::Item { marker, ordered, task: it.task, indent, content_col }, label, own, Some(parent), card);
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

/// A leaf's kind, label and whether the label is a summary of a card.
fn leaf(bl: &Block) -> (BlockKind, String, bool) {
    let first_text = |bs: &[Block]| bs.iter().find_map(text_of).unwrap_or_default();
    let n_lines = |t: &str| t.lines().count().max(1);
    let plural = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    match &bl.kind {
        Kind::Paragraph { runs } => (BlockKind::Paragraph, blocks::plain(runs).trim().to_string(), false),
        Kind::Code { lang, text } => {
            let lang = if lang.is_empty() { String::new() } else { format!(" · {lang}") };
            (BlockKind::Code, format!("code{lang} · {}", plural(n_lines(text), "line")), true)
        }
        Kind::Diff { lang, added, removed, .. } => {
            let lang = if lang.is_empty() { String::new() } else { format!(" · {lang}") };
            (BlockKind::Diff, format!("diff{lang} · +{added} −{removed}"), true)
        }
        Kind::Table(t) => (BlockKind::Table, format!("table · {} · {}", plural(t.header.len(), "column"), plural(t.rows.len(), "row")), true),
        Kind::Image { img, .. } => (BlockKind::Image, image_label(img), true),
        Kind::Gallery { items } => (BlockKind::Gallery, format!("gallery · {}", plural(items.len(), "image")), true),
        Kind::Quote { blocks } => (BlockKind::Quote, format!("quote · {}", first_text(blocks)).trim_end_matches([' ', '·']).to_string(), true),
        Kind::Alert { kind, blocks } => (BlockKind::Alert, format!("{} · {}", kind.title(), first_text(blocks)).trim_end_matches([' ', '·']).to_string(), true),
        Kind::Html { text } => (BlockKind::Html, format!("html · {}", plural(n_lines(text), "line")), true),
        Kind::List(_) | Kind::Heading { .. } | Kind::FrontMatter { .. } | Kind::Rule | Kind::Footnotes { .. } => (BlockKind::Paragraph, String::new(), false),
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
    if lines.first().map(|l| l.trim_start_matches('\u{feff}').trim_end_matches('\r')) != Some("---") {
        return o;
    }
    let Some(close) = lines.iter().skip(1).position(|l| matches!(l.trim_end_matches('\r'), "---" | "...")) else { return o };
    let body = &lines[1..close + 1];
    let Some(at) = body.iter().position(|l| l.trim_end() == "markmap:") else { return o };
    for l in body[at + 1..].iter().take_while(|l| l.starts_with([' ', '\t']) || l.trim().is_empty()) {
        let Some((k, v)) = l.trim().split_once(':') else { continue };
        let v = v.trim().trim_matches(['"', '\'']);
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
