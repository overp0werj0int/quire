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
                    self.item(it, l.ordered, bl.lines.start, parent);
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

    fn item(&mut self, it: &blocks::Item, ordered: bool, list: usize, parent: usize) {
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
        let n = self.push(NodeKind::Item { marker, ordered, task: it.task, indent, content_col, list }, label, own, Some(parent), card);
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
    let end = front_end(lines);
    if end == 0 {
        return o;
    }
    let body = &lines[1..end - 1];
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
    Splice { lines: pre..old.len() - post, with: new[pre..new.len() - post].to_vec(), focus }
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
        _ if n.card => InPlace::Sheet(n.own.clone()),
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
        let real = old.len() - usize::from(src.ends_with('\n'));
        let at = (front_end(&old)..real).find(|&l| !blank(old[l])).unwrap_or(real);
        let mut add = vec![format!("# {label}")];
        if at < real {
            add.push(String::new());
        }
        new.splice(at..at, add);
        return Some(splice_of(&old, &new, Some(at)));
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

/// A new node with text `text` (MAP-021 to MAP-024).
pub fn add(src: &str, t: &Tree, at: Add, text: &str) -> Result<Splice, Refused> {
    let old: Vec<&str> = src.split('\n').collect();
    let p = plan(&old, t, at)?;
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
    let first = build(p.blank);
    let fits = |(new, focus): &(Vec<String>, usize)| {
        let t2 = reparse(new);
        let Some(k) = t2.nodes.iter().position(|m| m.own.start == *focus && !m.own.is_empty()) else { return false };
        let mut s = shape(&t2);
        s.remove(k - 1);
        s == shape(t)
    };
    let (new, focus) = if !fits(&first) && !p.blank && fits(&build(true)) { build(true) } else { first };
    Ok(splice_of(&old, &new, Some(focus)))
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

fn plan(old: &[&str], t: &Tree, at: Add) -> Result<Plan, Refused> {
    match at {
        Add::Child(i) => child_plan(old, t, i),
        Add::After(ROOT) => child_plan(old, t, ROOT),
        Add::Before(ROOT) => Err(None),
        Add::After(i) | Add::Before(i) => sibling_plan(old, t, i, matches!(at, Add::Before(_))),
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

/// A new item after or before item `j` of the same list, in at line `at` (MAP-023, MAP-024).
fn item_plan(old: &[&str], t: &Tree, j: usize, at: usize, before: bool) -> Plan {
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
    let boxed = if *task == Task::None { "" } else { "[ ] " };
    let head = format!("{}{marker}{}{boxed}", &line[..b], " ".repeat(gap));
    let bump = if counting { items[if before { k } else { k + 1 }..].to_vec() } else { Vec::new() };
    let tight_spot = if before { at == n.own.start } else { at == n.span.end };
    Plan { at, head, col: Some(indent + marker.len() + gap), blank: !tight_spot || loose(old, t, &items), bump }
}

fn marker_len(k: &NodeKind) -> usize {
    match k {
        NodeKind::Item { marker, .. } => marker.len(),
        _ => 0,
    }
}

fn child_plan(old: &[&str], t: &Tree, i: usize) -> Result<Plan, Refused> {
    let n = &t.nodes[i];
    let kids: Vec<usize> = n.children.iter().copied().filter(|&c| t.nodes[c].kind != NodeKind::Footnotes).collect();
    let level_of = |c: usize| match t.nodes[c].kind {
        NodeKind::Heading { level, .. } => Some(level),
        _ => None,
    };
    match &n.kind {
        NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(None),
        NodeKind::Block(_) => Err(Some(LEAF_CHILD)),
        NodeKind::Heading { .. } if n.parent.is_some_and(|p| is_item(t, p)) => Err(Some(LEAF_CHILD)),
        NodeKind::Item { marker, ordered, content_col, .. } => Ok(match kids.iter().rev().find(|&&c| is_item(t, c)) {
            Some(&last) => item_plan(old, t, last, n.span.end, false),
            None => {
                let bullet = if *ordered { "-" } else { marker.as_str() };
                Plan { at: n.span.end, head: format!("{}{bullet} ", " ".repeat(*content_col)), col: Some(content_col + 2), blank: false, bump: Vec::new() }
            }
        }),
        NodeKind::Heading { .. } | NodeKind::Root => {
            let own_level = level_of(i).unwrap_or(0);
            let end = kids.last().map_or(if n.own.is_empty() { front_end(old) } else { n.own.end }, |&c| t.nodes[c].span.end);
            let decider = kids.iter().rev().copied().find(|&c| level_of(c).is_some() || is_item(t, c));
            let level = if own_level == 0 { kids.iter().filter_map(|&c| level_of(c)).min() } else { decider.and_then(level_of) };
            let last_item = kids.iter().rev().copied().find(|&c| is_item(t, c));
            Ok(match (level, last_item) {
                (Some(l), _) => Plan { at: end, head: format!("{} ", "#".repeat(l)), col: None, blank: true, bump: Vec::new() },
                (None, Some(j)) => item_plan(old, t, j, end, false),
                (None, None) if own_level == 1 => Plan { at: end, head: "## ".into(), col: None, blank: true, bump: Vec::new() },
                (None, None) => Plan { at: end, head: "- ".into(), col: Some(2), blank: true, bump: Vec::new() },
            })
        }
    }
}

fn sibling_plan(old: &[&str], t: &Tree, i: usize, before: bool) -> Result<Plan, Refused> {
    let n = &t.nodes[i];
    let at = if before { n.own.start } else { n.span.end };
    match &n.kind {
        NodeKind::Root | NodeKind::Footnotes | NodeKind::Footnote { .. } => Err(None),
        NodeKind::Heading { level, .. } => {
            let line = old[n.own.start];
            let head = format!("{}{} ", &line[..lead(line).1], "#".repeat(*level));
            Ok(Plan { at, head, col: None, blank: true, bump: Vec::new() })
        }
        NodeKind::Item { .. } => Ok(item_plan(old, t, i, at, before)),
        NodeKind::Block(_) => {
            let col = match n.parent.map(|p| &t.nodes[p].kind) {
                Some(NodeKind::Item { content_col, .. }) => *content_col,
                _ => 0,
            };
            Ok(Plan { at, head: " ".repeat(col), col: Some(col), blank: true, bump: Vec::new() })
        }
    }
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
