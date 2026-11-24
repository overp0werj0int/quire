//! The writer's rules (`docs/reader-writer/behavior/writer.md`), as pure functions: the live
//! formatting of the Markdown source line by line (section 1), the editing keys (section 2)
//! and the word count (WR-188). The source text is never changed by formatting: every line
//! keeps its characters, and only spans and strike lines are drawn over them.

use rune::{Color, Span};

use crate::format::Edit;
use crate::md;
use crate::roles::Roles;

/// The colours the source uses (WR-197 to WR-209).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    pub text: Color,
    pub strong: Color,
    pub dim: Color,
    pub faint: Color,
    /// The paragraph focus fade (WR-201).
    pub ghost: Color,
    pub mark: Color,
    pub link: Color,
    pub code: Color,
    pub code_bg: Color,
    /// `==highlight==` (WR-208).
    pub wash: Color,
}

impl Ink {
    pub fn new(c: &Roles) -> Ink {
        Ink {
            text: c.text,
            strong: c.strong,
            dim: c.dim,
            faint: c.faint,
            ghost: c.editor_ghost,
            mark: c.mark,
            link: c.link,
            code: c.code_text,
            code_bg: c.code_bg,
            wash: c.accent.alpha(0.22),
        }
    }
}

/// What one line carries into the next (WR-04): an open fence, front matter or HTML comment,
/// and whether the line had text, for setext underlines (WR-18).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub block: Block,
    pub after_text: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Block {
    #[default]
    None,
    /// A fence of `len` backticks or tildes (`ch`).
    Fence(u8, usize),
    Front,
    Comment,
}

/// A line's formatting, in byte offsets within the line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Styled {
    pub spans: Vec<Span>,
    pub strikes: Vec<(usize, usize, Color)>,
    /// The line's hanging mark (WR-16, WR-19, WR-22, WR-25): its bytes `.0..hang.1` hang in the
    /// gutter, and its first `.0` bytes, a list item's leading whitespace, indent every line of
    /// it instead. `(0, 0)` hangs nothing.
    pub hang: (usize, usize),
}

/// Sizes at the editor size `e` (WR-09, WR-14, WR-30).
pub fn code_size(e: f32) -> f32 {
    (e * 0.88).round()
}

fn small_size(e: f32) -> f32 {
    (e * 0.86).round()
}

pub fn heading_size(e: f32, level: usize) -> f32 {
    let scale = match level {
        1 => 1.5,
        2 => 1.28,
        3 => 1.12,
        _ => 1.0,
    };
    (e * scale).round()
}

const BOLD: f32 = 700.0;
const REGULAR: f32 = 400.0;

fn sp(a: usize, b: usize) -> Span {
    Span::new(a, b)
}

fn colored(a: usize, b: usize, c: Color) -> Span {
    Span { color: Some(c), ..sp(a, b) }
}

/// Up to three spaces of indent.
fn indent3(b: &[u8]) -> Option<usize> {
    let n = b.iter().take_while(|&&c| c == b' ').count();
    (n <= 3).then_some(n)
}

/// A fence opener (WR-09): the character and the run's length.
fn fence_open(ln: &str) -> Option<(u8, usize)> {
    let b = ln.as_bytes();
    let i = indent3(b)?;
    let ch = *b.get(i)?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let n = b[i..].iter().take_while(|&&c| c == ch).count();
    if n < 3 || (ch == b'`' && ln[i + n..].contains('`')) {
        return None;
    }
    Some((ch, n))
}

fn fence_close(ln: &str, ch: u8, len: usize) -> bool {
    let b = ln.as_bytes();
    let Some(i) = indent3(b) else { return false };
    let n = b[i..].iter().take_while(|&&c| c == ch).count();
    n >= len && ln[i + n..].trim().is_empty()
}

/// An ATX heading (WR-14): its level, the end of its mark, and where a closing run of `#`
/// starts (the line's end when it has none).
pub fn atx(ln: &str) -> Option<(usize, usize, usize)> {
    let b = ln.as_bytes();
    let i = indent3(b)?;
    let n = b[i..].iter().take_while(|&&c| c == b'#').count();
    if !(1..=6).contains(&n) || !matches!(b.get(i + n), None | Some(b' ' | b'\t')) {
        return None;
    }
    let mark = i + n + b[i + n..].iter().take_while(|&&c| c == b' ' || c == b'\t').count();
    // A closing run: whitespace, then `#`s at the line end.
    let body = ln.trim_end();
    let hashes = body.len() - body.trim_end_matches('#').len();
    let mut close = ln.len();
    if hashes > 0 && body.len() - hashes >= mark {
        let before = &body[..body.len() - hashes];
        if before.len() == mark || before.ends_with([' ', '\t']) {
            close = before.trim_end().len().max(mark);
        }
    }
    Some((n, mark, close))
}

/// `---`, `***` or `___`, three or more, spaces allowed (WR-17).
fn thematic(ln: &str) -> bool {
    let t: Vec<u8> = ln.bytes().filter(|&c| c != b' ' && c != b'\t').collect();
    t.len() >= 3 && matches!(t[0], b'-' | b'*' | b'_') && t.iter().all(|&c| c == t[0]) && indent3(ln.as_bytes()).is_some()
}

/// A run of `=` or `-` (WR-18).
fn setext(ln: &str) -> bool {
    let t = ln.trim();
    !t.is_empty() && (t.bytes().all(|c| c == b'=') || t.bytes().all(|c| c == b'-')) && indent3(ln.as_bytes()).is_some()
}

/// The end of a block quote's marks (WR-19): one or more `>`, each after up to three spaces
/// and with one optional space after it.
pub fn quote(ln: &str) -> Option<usize> {
    let b = ln.as_bytes();
    let mut i = 0;
    let mut any = false;
    loop {
        let sp = b[i..].iter().take_while(|&&c| c == b' ').count();
        if sp > 3 || b.get(i + sp) != Some(&b'>') {
            break;
        }
        i += sp + 1;
        if b.get(i) == Some(&b' ') {
            i += 1;
        }
        any = true;
    }
    any.then_some(i)
}

/// `|---|:--:|` and the like (WR-23).
fn table_rule(ln: &str) -> bool {
    let t = ln.trim();
    t.contains('|') && t.contains('-') && t.bytes().all(|c| matches!(c, b'|' | b'-' | b':' | b' ' | b'\t')) && t.split('|').map(str::trim).filter(|c| !c.is_empty()).all(|c| c.trim_matches(':').bytes().all(|x| x == b'-') && c.contains('-'))
}

/// `[id]: url`, not a footnote (WR-24).
fn ref_def(ln: &str) -> bool {
    let t = ln.trim_start();
    t.starts_with('[') && !t.starts_with("[^") && t.find("]:").is_some_and(|j| j > 1 && !t[1..j].contains(']'))
}

/// `[^x]:`, its label and colon's end (WR-25).
fn footnote_def(ln: &str) -> Option<usize> {
    let t = ln.trim_start();
    let lead = ln.len() - t.len();
    if !t.starts_with("[^") {
        return None;
    }
    let j = t.find("]:")?;
    (j > 2 && !t[2..j].contains(']')).then_some(lead + j + 2)
}

/// Formats one source line, given what the line before it carried (section 1). `first` says
/// it is the note's first line, where front matter may open. `resolves` says whether a
/// `[[link]]` target names a note. Emits strictly sorted, non-overlapping spans so Rune's
/// per-paragraph layout and paint take their O(log N) binary-search path (`LIVEPACE`).
pub fn line(ln: &str, first: bool, st: State, e: f32, ink: &Ink, resolves: &dyn Fn(&str) -> bool) -> (Styled, State) {
    let (mut out, next) = line_raw(ln, first, st, e, ink, resolves);
    out.spans = flatten_spans(out.spans);
    (out, next)
}

fn same_span_style(a: &Span, b: &Span) -> bool {
    a.weight == b.weight
        && a.mono == b.mono
        && a.size == b.size
        && a.color == b.color
        && a.bg == b.bg
        && a.bg_pad == b.bg_pad
        && a.underline == b.underline
        && a.italic == b.italic
}

/// Flattens overlapping or unsorted spans within a single line into non-empty, sorted,
/// non-overlapping intervals with identical later-span-wins semantics to Rune's `flatten_spans`.
fn flatten_spans(mut spans: Vec<Span>) -> Vec<Span> {
    spans.retain(|s| s.start < s.end);
    if spans.len() <= 1 || spans.windows(2).all(|w| w[0].end <= w[1].start) {
        return spans;
    }
    let mut bounds = Vec::with_capacity(spans.len() * 2);
    let mut order: Vec<usize> = (0..spans.len()).collect();
    for s in &spans {
        bounds.push(s.start);
        bounds.push(s.end);
    }
    bounds.sort_unstable();
    bounds.dedup();
    order.sort_by_key(|&i| spans[i].start);
    let mut active: Vec<usize> = Vec::new();
    let mut next = 0;
    let mut out: Vec<Span> = Vec::with_capacity(bounds.len());
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        active.retain(|&i| spans[i].end > a);
        while next < order.len() && spans[order[next]].start <= a {
            let i = order[next];
            let at = active.partition_point(|&j| j < i);
            active.insert(at, i);
            next += 1;
        }
        if active.is_empty() {
            continue;
        }
        let mut m = Span::new(a as usize, b as usize);
        for &i in &active {
            let s = &spans[i];
            if s.weight.is_some() {
                m.weight = s.weight;
            }
            m.mono |= s.mono;
            if s.size.is_some() {
                m.size = s.size;
            }
            if s.color.is_some() {
                m.color = s.color;
            }
            if s.bg.is_some() {
                m.bg = s.bg;
                m.bg_pad = s.bg_pad;
            }
            if s.underline.is_some() {
                m.underline = s.underline;
            }
            m.italic |= s.italic;
        }
        if let Some(last) = out.last_mut()
            && last.end == m.start
            && same_span_style(last, &m)
        {
            last.end = m.end;
        } else {
            out.push(m);
        }
    }
    out
}

fn line_raw(ln: &str, first: bool, st: State, e: f32, ink: &Ink, resolves: &dyn Fn(&str) -> bool) -> (Styled, State) {
    let mut out = Styled::default();
    let n = ln.len();
    let has_text = !ln.trim().is_empty();
    let next = |block| State { block, after_text: has_text };
    let code = |a, b, c| Span { mono: true, size: Some(code_size(e)), color: Some(c), ..sp(a, b) };
    match st.block {
        Block::Fence(ch, len) => {
            if fence_close(ln, ch, len) {
                out.spans.push(code(0, n, ink.faint));
                return (out, next(Block::None));
            }
            out.spans.push(code(0, n, ink.code));
            return (out, next(st.block));
        }
        Block::Front => {
            if ln == "---" || ln == "..." {
                out.spans.push(Span { mono: true, ..colored(0, n, ink.faint) });
                return (out, next(Block::None));
            }
            out.spans.push(Span { mono: true, ..colored(0, n, ink.dim) });
            if let Some(c) = ln.find(':') {
                out.spans.push(colored(0, c + 1, ink.mark));
            }
            return (out, next(Block::Front));
        }
        Block::Comment => {
            out.spans.push(colored(0, n, ink.faint));
            return (out, next(if ln.contains("-->") { Block::None } else { Block::Comment }));
        }
        Block::None => {}
    }
    if first && ln == "---" {
        out.spans.push(Span { mono: true, ..colored(0, n, ink.faint) });
        return (out, next(Block::Front));
    }
    if let Some((ch, len)) = fence_open(ln) {
        out.spans.push(code(0, n, ink.faint));
        return (out, next(Block::Fence(ch, len)));
    }
    let t = ln.trim_start();
    if t.starts_with("<!--") {
        out.spans.push(colored(0, n, ink.faint));
        let open = !t[4..].contains("-->");
        return (out, next(if open { Block::Comment } else { Block::None }));
    }
    if let Some((level, mark, close)) = atx(ln) {
        let size = heading_size(e, level);
        let faint = |a, b| Span { weight: Some(REGULAR), color: Some(ink.faint), size: Some(size), ..sp(a, b) };
        out.spans.push(Span { weight: Some(BOLD), color: Some(ink.strong), size: Some(size), ..sp(0, n) });
        out.spans.push(faint(0, mark));
        out.hang = (0, mark);
        if close < n {
            out.spans.push(faint(close, n));
        }
        inline(ln, mark, close, e, ink, resolves, &mut out);
        return (out, next(Block::None));
    }
    if thematic(ln) || (st.after_text && setext(ln)) {
        out.spans.push(colored(0, n, ink.faint));
        return (out, State { block: Block::None, after_text: false });
    }
    if let Some(q) = quote(ln) {
        out.spans.push(Span { color: Some(ink.dim), italic: true, ..sp(q, n) });
        out.spans.push(colored(0, q, ink.mark));
        out.hang = (0, q);
        inline(ln, q, n, e, ink, resolves, &mut out);
        return (out, next(Block::None));
    }
    if let Some(l) = md::list_line(ln) {
        out.spans.push(colored(l.marker.start, l.after, ink.mark));
        // The task box is the item's text and stays in the column (WR-22).
        out.hang = (l.marker.start, l.after);
        let from = match &l.task {
            Some(t) => {
                out.spans.push(Span { mono: true, ..colored(t.start, t.start + 3, ink.mark) });
                if l.done && t.end < n {
                    out.spans.push(colored(t.end, n, ink.faint));
                    out.strikes.push((t.end, n, ink.faint));
                }
                t.end
            }
            None => l.after,
        };
        inline(ln, from, n, e, ink, resolves, &mut out);
        return (out, next(Block::None));
    }
    if table_rule(ln) || ref_def(ln) {
        out.spans.push(colored(0, n, ink.faint));
        return (out, next(Block::None));
    }
    if let Some(f) = footnote_def(ln) {
        out.spans.push(colored(0, f, ink.mark));
        // The label, its colon and one space after hang (WR-25).
        out.hang = (0, f + usize::from(ln[f..].starts_with(' ')));
        inline(ln, f, n, e, ink, resolves, &mut out);
        return (out, next(Block::None));
    }
    inline(ln, 0, n, e, ink, resolves, &mut out);
    (out, next(Block::None))
}

const ESCAPABLE: &[u8] = b"\\`*_{}[]()#+-.!|~>=";

/// The inline rules (WR-27 to WR-38, WR-220, WR-221) over `ln[from..to]`. A character an
/// earlier rule takes as literal cannot start a later rule's span.
fn inline(ln: &str, from: usize, to: usize, e: f32, ink: &Ink, resolves: &dyn Fn(&str) -> bool, out: &mut Styled) {
    let b = ln.as_bytes();
    let mut taken = vec![false; ln.len()];
    let free = |taken: &[bool], a: usize, z: usize| a <= z && z <= to && !taken[a..z].iter().any(|&t| t);
    let take = |taken: &mut [bool], a: usize, z: usize| taken[a..z].iter_mut().for_each(|t| *t = true);
    let faint = |a, z| colored(a, z, ink.faint);
    let small = |a, z| Span { size: Some(small_size(e)), ..faint(a, z) };

    // Escapes (WR-27).
    let mut i = from;
    while i + 1 < to {
        if b[i] == b'\\' && ESCAPABLE.contains(&b[i + 1]) {
            out.spans.push(faint(i, i + 1));
            take(&mut taken, i, i + 2);
            i += 2;
        } else {
            i += 1;
        }
    }

    // Code spans (WR-28).
    let mut i = from;
    while i < to {
        if b[i] != b'`' || taken[i] {
            i += 1;
            continue;
        }
        let run = |at: usize| b[at..to].iter().take_while(|&&c| c == b'`').count();
        let n = run(i);
        let mut j = i + n;
        let mut close = None;
        while j < to {
            if b[j] == b'`' {
                let m = run(j);
                if m == n {
                    close = Some(j);
                    break;
                }
                j += m;
            } else {
                j += 1;
            }
        }
        match close {
            Some(j) => {
                let chip = |a, z, c| Span { mono: true, size: Some(code_size(e)), color: Some(c), bg: Some(ink.code_bg), ..sp(a, z) };
                out.spans.push(chip(i, j + n, ink.code));
                out.spans.push(chip(i, i + n, ink.faint));
                out.spans.push(chip(j, j + n, ink.faint));
                take(&mut taken, i, j + n);
                i = j + n;
            }
            None => i += n,
        }
    }

    // Inline HTML tags (WR-29).
    for i in from..to {
        if b[i] != b'<' || taken[i] {
            continue;
        }
        if let Some(z) = html_tag(&ln[i..to]).map(|n| i + n)
            && free(&taken, i, z)
        {
            out.spans.push(faint(i, z));
            take(&mut taken, i, z);
        }
    }

    // Note links and image embeds (WR-220).
    let mut i = from;
    while i + 1 < to {
        if &b[i..i + 2] != b"[[" || taken[i] {
            i += 1;
            continue;
        }
        let Some(k) = ln[i + 2..to].find("]]").map(|k| i + 2 + k) else { break };
        let inner = &ln[i + 2..k];
        if inner.is_empty() || inner.contains('[') || !free(&taken, i, k + 2) {
            i += 2;
            continue;
        }
        let embed = i > from && b[i - 1] == b'!' && !taken[i - 1];
        let a = if embed { i - 1 } else { i };
        if embed {
            out.spans.push(Span { color: Some(ink.dim), italic: true, ..sp(i + 2, k) });
        } else {
            let target = md::link_target(inner);
            out.spans.push(colored(i + 2, k, if resolves(&target) { ink.link } else { ink.dim }));
        }
        out.spans.push(faint(a, i + 2));
        out.spans.push(faint(k, k + 2));
        take(&mut taken, a, k + 2);
        i = k + 2;
    }

    // Inline links and images (WR-30, WR-31), then reference links (WR-32).
    let mut i = from;
    while i < to {
        if b[i] != b'[' || taken[i] || b.get(i + 1) == Some(&b'^') {
            i += 1;
            continue;
        }
        let Some(c) = ln[i + 1..to].find(']').map(|c| i + 1 + c) else { break };
        let image = i > from && b[i - 1] == b'!' && !taken[i - 1];
        let a = if image { i - 1 } else { i };
        if b.get(c + 1) == Some(&b'(')
            && let Some(p) = ln[c + 2..to].find(')').map(|p| c + 2 + p)
            && free(&taken, c, p + 1)
            && !taken[a]
        {
            if image {
                out.spans.push(Span { color: Some(ink.dim), italic: true, ..sp(i + 1, c) });
            } else {
                out.spans.push(colored(i + 1, c, ink.link));
            }
            out.spans.push(faint(a, i + 1));
            out.spans.push(small(c, p + 1));
            take(&mut taken, a, i + 1);
            take(&mut taken, c, p + 1);
            i = p + 1;
            continue;
        }
        if !image
            && b.get(c + 1) == Some(&b'[')
            && let Some(d) = ln[c + 2..to].find(']').map(|d| c + 2 + d)
            && free(&taken, c, d + 1)
        {
            out.spans.push(colored(i + 1, c, ink.link));
            out.spans.push(faint(i, i + 1));
            out.spans.push(faint(c, d + 1));
            take(&mut taken, i, i + 1);
            take(&mut taken, c, d + 1);
            i = d + 1;
            continue;
        }
        i += 1;
    }

    // Autolinks (WR-33).
    for i in from..to {
        if b[i] != b'<' || taken[i] {
            continue;
        }
        let rest = &ln[i + 1..to];
        if (rest.starts_with("https://") || rest.starts_with("http://") || rest.starts_with("mailto:"))
            && let Some(k) = rest.find('>').map(|k| i + 1 + k)
            && !ln[i + 1..k].contains(char::is_whitespace)
            && free(&taken, i, k + 1)
        {
            out.spans.push(colored(i + 1, k, ink.link));
            out.spans.push(faint(i, i + 1));
            out.spans.push(faint(k, k + 1));
            take(&mut taken, i, k + 1);
        }
    }

    // Bare URLs (WR-34).
    let mut i = from;
    while i < to {
        let rest = &ln[i..to];
        let starts = rest.starts_with("http://") || rest.starts_with("https://") || rest.starts_with("www.");
        let before_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'(' | b'<' | b'/' | b'_'));
        if !starts || taken[i] || !before_ok {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let mut z = i + rest.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(rest.len());
        while z > i && matches!(b[z - 1], b'.' | b',' | b';' | b':' | b'!' | b'?' | b'\'' | b'"' | b')' | b']' | b'}' | b'>' | b'(' | b'[' | b'{') {
            z -= 1;
        }
        if free(&taken, i, z) {
            out.spans.push(colored(i, z, ink.link));
            take(&mut taken, i, z);
        }
        i = z.max(i + 1);
    }

    // Footnote references (WR-35).
    let mut i = from;
    while i + 2 < to {
        if &b[i..i + 2] == b"[^"
            && let Some(k) = ln[i + 2..to].find(']').map(|k| i + 2 + k)
            && k > i + 2
            && free(&taken, i, k + 1)
        {
            out.spans.push(colored(i, k + 1, ink.mark));
            take(&mut taken, i, k + 1);
            i = k + 1;
        } else {
            i += 1;
        }
    }

    // Tags (WR-221), as Quire's index reads them.
    for (t, r) in md::inline_tokens(&ln[from..to]) {
        let (a, z) = (from + r.start, from + r.end);
        if t == md::Tok::Tag && free(&taken, a, z) {
            out.spans.push(colored(a, z, ink.mark));
            take(&mut taken, a, z);
        }
    }

    // Emphasis, longest mark first (WR-36, WR-37).
    emphasis(ln, from, to, "**", &mut taken, out, ink);
    emphasis(ln, from, to, "__", &mut taken, out, ink);
    emphasis(ln, from, to, "*", &mut taken, out, ink);
    emphasis(ln, from, to, "_", &mut taken, out, ink);
    emphasis(ln, from, to, "~~", &mut taken, out, ink);
    emphasis(ln, from, to, "==", &mut taken, out, ink);

    // Table pipes (WR-38).
    if ln.trim_start().starts_with('|') {
        for i in from..to {
            if b[i] == b'|' && !taken[i] && (i == 0 || b[i - 1] != b'\\') {
                out.spans.push(faint(i, i + 1));
            }
        }
    }
}

/// The length of an HTML tag at the start of `s`: `<tag ...>`, `</tag>` or `<tag/>`.
fn html_tag(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 1;
    if b.get(i) == Some(&b'/') {
        i += 1;
    }
    if !b.get(i)?.is_ascii_alphabetic() {
        return None;
    }
    i += b[i..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'-').count();
    match b.get(i)? {
        b'>' => Some(i + 1),
        b'/' if b.get(i + 1) == Some(&b'>') => Some(i + 2),
        c if c.is_ascii_whitespace() => {
            let k = s[i..].find(['<', '>'])?;
            (b[i + k] == b'>').then_some(i + k + 1)
        }
        _ => None,
    }
}

/// One emphasis kind: both marks faint and taken, the text between styled.
fn emphasis(ln: &str, from: usize, to: usize, m: &str, taken: &mut [bool], out: &mut Styled, ink: &Ink) {
    let b = ln.as_bytes();
    let n = m.len();
    let star = m == "*";
    let under = m == "_";
    // Bytes, not `&str`: the scan steps a byte at a time, and a slice of the line would panic
    // inside a character such as `…`. The marks are ASCII, so a byte match is a char match.
    let is_mark = |taken: &[bool], i: usize| i + n <= to && &b[i..i + n] == m.as_bytes() && !taken[i..i + n].iter().any(|&t| t);
    // A single `*` touching another `*`, or after a backslash, is no mark.
    let lone = |i: usize| !star || ((i == 0 || b[i - 1] != b'*') && b.get(i + 1) != Some(&b'*') && (i == 0 || b[i - 1] != b'\\'));
    let word = |i: Option<usize>| i.and_then(|i| ln[i..].chars().next()).is_some_and(|c| c.is_alphanumeric());
    let prev = |i: usize| ln[..i].char_indices().next_back().map(|(k, _)| k);
    let mut i = from;
    while i < to {
        let opens = is_mark(taken, i) && lone(i) && b.get(i + n).is_some_and(|c| !c.is_ascii_whitespace()) && (!under || i == 0 || !word(prev(i)));
        if !opens {
            i += 1;
            continue;
        }
        let mut j = i + n + 1;
        let mut close = None;
        while j + n <= to {
            if is_mark(taken, j) && lone(j) && !b[j - 1].is_ascii_whitespace() && (!under || !word(Some(j + n).filter(|&k| k < ln.len()))) {
                close = Some(j);
                break;
            }
            j += 1;
        }
        let Some(j) = close else {
            i += n;
            continue;
        };
        let inner = sp(i + n, j);
        out.spans.push(match m {
            "**" | "__" => Span { weight: Some(BOLD), color: Some(ink.strong), ..inner },
            "*" | "_" => Span { italic: true, ..inner },
            "~~" => {
                out.strikes.push((i + n, j, ink.dim));
                Span { color: Some(ink.dim), ..inner }
            }
            _ => Span { bg: Some(ink.wash), ..inner },
        });
        out.spans.push(colored(i, i + n, ink.faint));
        out.spans.push(colored(j, j + n, ink.faint));
        taken[i..i + n].iter_mut().for_each(|t| *t = true);
        taken[j..j + n].iter_mut().for_each(|t| *t = true);
        i = j + n;
    }
}

/// A line outside the current paragraph (WR-82): every colour becomes the fade colour, and
/// backgrounds go. Sizes, faces and strike lines stay. Preserves strictly sorted, non-overlapping
/// spans (`LIVEPACE`).
pub fn fade(s: &Styled, len: usize, ghost: Color) -> Styled {
    let mut spans: Vec<Span> = Vec::with_capacity(s.spans.len() + 1);
    let push_merged = |out: &mut Vec<Span>, m: Span| {
        if m.start >= m.end {
            return;
        }
        if let Some(last) = out.last_mut()
            && last.end == m.start
            && same_span_style(last, &m)
        {
            last.end = m.end;
        } else {
            out.push(m);
        }
    };
    let mut at = 0usize;
    for x in &s.spans {
        let (a, b) = ((x.start as usize).min(len), (x.end as usize).min(len));
        if a >= b {
            continue;
        }
        if a > at {
            push_merged(&mut spans, colored(at, a, ghost));
        }
        push_merged(&mut spans, Span { start: a as u32, end: b as u32, color: Some(ghost), bg: None, bg_pad: 0.0, ..x.clone() });
        at = b;
    }
    if at < len {
        push_merged(&mut spans, colored(at, len, ghost));
    }
    Styled { spans, strikes: s.strikes.iter().map(|&(a, b, _)| (a, b, ghost)).collect(), hang: s.hang }
}

// ---------- Word count (WR-188) ----------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub words: usize,
    pub chars: usize,
    pub minutes: usize,
}

/// Words, characters and reading time, as omarky counts them.
pub fn count(text: &str) -> Count {
    let body = strip_front(text);
    let chars = body.chars().filter(|c| !c.is_whitespace()).count();
    let words = count_words(body);
    let minutes = count_minutes(words);
    Count { words, chars, minutes }
}

pub(crate) fn count_minutes(words: usize) -> usize {
    if words == 0 { 0 } else { ((words as f32 / 230.0).round() as usize).max(1) }
}

pub(crate) fn strip_front_offset(text: &str) -> usize {
    if !(text.starts_with("---\n") || text == "---") {
        return 0;
    }
    let mut at = 4.min(text.len());
    for ln in text[at..].split_inclusive('\n') {
        at += ln.len();
        let t = ln.trim_end_matches(['\n', '\r']);
        if t == "---" || t == "..." {
            return at;
        }
    }
    0
}

fn strip_front(text: &str) -> &str {
    &text[strip_front_offset(text)..]
}

/// Words in the text: fenced code, link targets, HTML tags, bare URLs and Markdown marks
/// left out, then runs of letters or digits, joined by `'`, `’` or `.`. Streams directly over
/// lines without allocating intermediate stripped strings (`LIVEPACE`).
pub fn count_words(text: &str) -> usize {
    let mut n = 0usize;
    let mut fence: Option<(u8, usize)> = None;
    for ln in text.split('\n') {
        match fence {
            Some((ch, len)) => {
                if fence_close(ln, ch, len) {
                    fence = None;
                }
                continue;
            }
            None => {
                if let Some(f) = fence_open(ln) {
                    fence = Some(f);
                    continue;
                }
            }
        }
        n += count_words_line(ln);
    }
    n
}

/// Words within the byte selection `a..b` of `doc`, applying the same front-matter and fenced-code
/// exclusion as [`count`] so `Ctrl+A` always equals the note's total word count and selections
/// inside front matter or code fences yield 0 (`WORDSEL`, WR-188).
pub fn count_words_range(doc: &str, a: usize, b: usize) -> usize {
    let mut lo = a.min(b).min(doc.len());
    let mut hi = a.max(b).min(doc.len());
    while lo > 0 && !doc.is_char_boundary(lo) {
        lo -= 1;
    }
    while hi < doc.len() && !doc.is_char_boundary(hi) {
        hi += 1;
    }
    if lo >= hi {
        return 0;
    }
    let front = strip_front_offset(doc);
    lo = lo.max(front);
    if lo >= hi {
        return 0;
    }
    let mut n = 0usize;
    let mut fence: Option<(u8, usize)> = None;
    let mut at = front;
    for ln in doc[front..].split('\n') {
        let line_end = at + ln.len();
        if at >= hi {
            break;
        }
        match fence {
            Some((ch, len)) => {
                if fence_close(ln, ch, len) {
                    fence = None;
                }
                at = line_end + 1;
                continue;
            }
            None => {
                if let Some(f) = fence_open(ln) {
                    fence = Some(f);
                    at = line_end + 1;
                    continue;
                }
            }
        }
        let seg_a = lo.max(at);
        let seg_b = hi.min(line_end);
        if seg_a < seg_b {
            n += count_words_line(&doc[seg_a..seg_b]);
        }
        at = line_end + 1;
    }
    n
}

/// Zero-allocation word count for one non-fenced line, stripping link targets, HTML tags,
/// bare URLs and Markdown marks on the fly.
pub(crate) fn count_words_line(ln: &str) -> usize {
    let b = ln.as_bytes();
    let mut n = 0usize;
    let mut in_word = false;
    let mut pending_joiner = false;
    let feed_sep = |in_word: &mut bool, pending_joiner: &mut bool| {
        *in_word = false;
        *pending_joiner = false;
    };
    let mut i = 0;
    while i < ln.len() {
        let ch = b[i];
        if ch == b']' && b.get(i + 1) == Some(&b'(') {
            if let Some(k) = ln[i..].find(')') {
                feed_sep(&mut in_word, &mut pending_joiner);
                i += k + 1;
                continue;
            }
        }
        if ch == b'<' {
            let rest = &ln[i..];
            if let Some(k) = html_tag(rest).or_else(|| rest.find('>').filter(|&k| rest[1..k].starts_with("http") || rest[1..k].starts_with("mailto:")).map(|k| k + 1)) {
                feed_sep(&mut in_word, &mut pending_joiner);
                i += k;
                continue;
            }
        }
        if (ch == b'h' || ch == b'w') && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let rest = &ln[i..];
            if rest.starts_with("http://") || rest.starts_with("https://") || rest.starts_with("www.") {
                let k = rest.find(char::is_whitespace).unwrap_or(rest.len());
                feed_sep(&mut in_word, &mut pending_joiner);
                i += k;
                continue;
            }
        }
        if ch < 0x80 {
            if ch.is_ascii_alphanumeric() {
                if !in_word {
                    n += 1;
                    in_word = true;
                }
                pending_joiner = false;
            } else if in_word && !pending_joiner && matches!(ch, b'\'' | b'.') {
                pending_joiner = true;
            } else {
                feed_sep(&mut in_word, &mut pending_joiner);
            }
            i += 1;
        } else {
            let c = ln[i..].chars().next().unwrap_or(' ');
            if c.is_alphanumeric() {
                if !in_word {
                    n += 1;
                    in_word = true;
                }
                pending_joiner = false;
            } else if in_word && !pending_joiner && c == '\u{2019}' {
                pending_joiner = true;
            } else {
                feed_sep(&mut in_word, &mut pending_joiner);
            }
            i += c.len_utf8();
        }
    }
    n
}

// ---------- Editing keys (section 2) ----------

fn line_at(text: &str, at: usize) -> (usize, usize) {
    let ls = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let le = text[at..].find('\n').map_or(text.len(), |i| at + i);
    (ls, le)
}

fn caret(at: usize) -> (usize, usize) {
    (at, at)
}

/// Enter on a list item or a quote (WR-39 to WR-44). None is a plain newline.
pub fn enter(text: &str, a: usize, b: usize) -> Option<Edit> {
    if a != b {
        return None;
    }
    let (ls, le) = line_at(text, a);
    let ln = &text[ls..le];
    let col = a - ls;
    if let Some(l) = md::list_line(ln) {
        let p = l.task.as_ref().map_or(l.after, |t| t.end);
        if col < p {
            return None;
        }
        if ln[p..].trim().is_empty() {
            return Some(Edit { start: ls, end: le, insert: String::new(), sel: caret(ls) });
        }
        let mk = match l.number {
            Some((n, d)) => format!("{}{d}", n + 1),
            None => ln[l.marker.clone()].to_string(),
        };
        let task = if l.task.is_some() { "[ ] " } else { "" };
        let insert = format!("\n{}{mk}{}{task}", &ln[..l.indent], &ln[l.marker.end..l.after]);
        return Some(Edit { start: a, end: a, sel: caret(a + insert.len()), insert });
    }
    if let Some(q) = quote(ln) {
        if col < q {
            return None;
        }
        if ln[q..].trim().is_empty() {
            return Some(Edit { start: ls, end: le, insert: String::new(), sel: caret(ls) });
        }
        let insert = format!("\n{}", &ln[..q]);
        return Some(Edit { start: a, end: a, sel: caret(a + insert.len()), insert });
    }
    None
}

/// A list item's indent step (WR-47): 2 for a bullet, the marker and its spaces for a number.
fn step(l: &md::ListLine) -> usize {
    if l.number.is_some() { l.after - l.marker.start } else { 2 }
}

/// The lines `a..b` touches, as `(start, end)` byte ranges.
fn lines_of(text: &str, a: usize, b: usize) -> Vec<(usize, usize)> {
    let (ls, _) = line_at(text, a);
    // A selection that ends at a line's start leaves that line alone.
    let b2 = if b > a && text[..b].ends_with('\n') { b - 1 } else { b };
    let (_, le) = line_at(text, b2);
    let mut out = Vec::new();
    let mut at = ls;
    for ln in text[ls..le].split('\n') {
        out.push((at, at + ln.len()));
        at += ln.len() + 1;
    }
    out
}

/// Moves `pos` across insertions and removals at line starts: `(at, delta)` pairs in order.
fn shift(pos: usize, edits: &[(usize, isize)]) -> usize {
    let mut p = pos as isize;
    for &(at, d) in edits {
        if pos > at || (pos == at && d > 0) {
            p += d;
            if d < 0 {
                p = p.max(at as isize);
            }
        }
    }
    p.max(0) as usize
}

/// Rebuilds the lines `a..b` touches with `f`, giving back one replacement (WR-63).
fn per_line(text: &str, a: usize, b: usize, f: impl Fn(&str) -> Option<(String, isize)>) -> Option<Edit> {
    let lines = lines_of(text, a.min(b), a.max(b));
    let (start, end) = (lines[0].0, lines[lines.len() - 1].1);
    let mut out = String::new();
    let mut moves = Vec::new();
    let mut any = false;
    for (k, &(ls, le)) in lines.iter().enumerate() {
        if k > 0 {
            out.push('\n');
        }
        match f(&text[ls..le]) {
            Some((new, d)) => {
                any = true;
                moves.push((ls, d));
                out.push_str(&new);
            }
            None => out.push_str(&text[ls..le]),
        }
    }
    any.then(|| Edit { start, end, insert: out, sel: (shift(a, &moves), shift(b, &moves)) })
}

/// Tab (WR-47, WR-48): a list item indents by its step, other lines get four spaces at the
/// caret. With a selection, every list line it touches indents and other lines stay.
pub fn tab(text: &str, a: usize, b: usize) -> Option<Edit> {
    let indent = |ln: &str| md::list_line(ln).map(|l| (format!("{}{ln}", " ".repeat(step(&l))), step(&l) as isize));
    if a == b {
        let (ls, le) = line_at(text, a);
        if md::list_line(&text[ls..le]).is_none() {
            return Some(Edit { start: a, end: a, insert: "    ".into(), sel: caret(a + 4) });
        }
    }
    per_line(text, a, b, indent)
}

/// Shift+Tab (WR-50): a list item with an indent loses up to its step of it. None when no line
/// changes; the key is consumed either way.
pub fn shift_tab(text: &str, a: usize, b: usize) -> Option<Edit> {
    per_line(text, a, b, |ln| {
        let l = md::list_line(ln)?;
        let n = step(&l).min(l.indent);
        (n > 0).then(|| (ln[n..].to_string(), -(n as isize)))
    })
}

/// A heading prefix as the heading keys count it (WR-61): up to three spaces, one to six `#`,
/// then spaces, tabs or the line end. Its level and length.
fn heading_prefix(ln: &str) -> Option<(usize, usize)> {
    let b = ln.as_bytes();
    let i = indent3(b)?;
    let n = b[i..].iter().take_while(|&&c| c == b'#').count();
    if !(1..=6).contains(&n) || !matches!(b.get(i + n), None | Some(b' ' | b'\t')) {
        return None;
    }
    Some((n, i + n + b[i + n..].iter().take_while(|&&c| c == b' ' || c == b'\t').count()))
}

/// Ctrl+Alt+1 to Ctrl+Alt+6 and Ctrl+Alt+0 (WR-59, WR-60): sets the caret line's heading level, clears it
/// when the level is the line's own or 0. The caret keeps its place in the heading text.
pub fn heading(text: &str, focus: usize, level: usize) -> Edit {
    let (ls, le) = line_at(text, focus);
    let ln = &text[ls..le];
    let (cur, plen) = heading_prefix(ln).unwrap_or((0, 0));
    let prefix = if level == 0 || level == cur { String::new() } else { format!("{} ", "#".repeat(level)) };
    let col = (focus - ls).saturating_sub(plen);
    let at = ls + prefix.len() + col;
    Edit { start: ls, end: ls + plen, insert: prefix, sel: caret(at) }
}

/// Ctrl+Shift+Enter: toggles `[ ]` / `[x]` on every task line in the selection (if all tasks are
/// checked, unchecks them; otherwise checks all unchecked tasks). If no line in the selection is
/// a task yet, converts via `Fmt::Task`.
pub fn toggle_checkbox(text: &str, a: usize, b: usize) -> Option<Edit> {
    let lines = lines_of(text, a.min(b), a.max(b));
    let mut has_task = false;
    let mut all_checked = true;
    for &(ls, le) in &lines {
        let ln = &text[ls..le];
        if let Some(l) = md::list_line(ln)
            && let Some(t) = l.task
        {
            has_task = true;
            if &ln[t.start + 1..t.start + 2] == " " {
                all_checked = false;
            }
        }
    }
    if !has_task {
        return Some(crate::format::apply(text, a, b, crate::format::Fmt::Task));
    }
    let mark = if all_checked { " " } else { "x" };
    per_line(text, a, b, |ln| {
        let l = md::list_line(ln)?;
        let t = l.task?;
        let mut s = ln.to_string();
        s.replace_range(t.start + 1..t.start + 2, mark);
        (s != ln).then_some((s, 0))
    })
}

/// Alt+Up / Alt+Down: moves the line or selected lines up or down one line as a single edit,
/// keeping the caret or selection at the same offset within the moved lines.
pub fn move_lines(text: &str, a: usize, b: usize, down: bool) -> Option<Edit> {
    let lines = lines_of(text, a.min(b), a.max(b));
    let (start, end) = (lines[0].0, lines[lines.len() - 1].1);
    let block = &text[start..end];
    if !down {
        if start == 0 {
            return None;
        }
        let prev_end = start - 1;
        let prev_start = text[..prev_end].rfind('\n').map_or(0, |i| i + 1);
        let prev = &text[prev_start..prev_end];
        let delta = prev.len() + 1;
        let insert = format!("{block}\n{prev}");
        Some(Edit { start: prev_start, end, insert, sel: (a - delta, b - delta) })
    } else {
        if end >= text.len() {
            return None;
        }
        let next_start = end + 1;
        let next_end = text[next_start..].find('\n').map_or(text.len(), |i| next_start + i);
        let next = &text[next_start..next_end];
        let delta = next.len() + 1;
        let insert = format!("{next}\n{block}");
        Some(Edit { start, end: next_end, insert, sel: (a + delta, b + delta) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink() -> Ink {
        let c = |r: u8| Color::BLACK.alpha(r as f32 / 16.0);
        Ink { text: c(1), strong: c(2), dim: c(3), faint: c(4), ghost: c(5), mark: c(6), link: c(7), code: c(8), code_bg: c(9), wash: c(10) }
    }

    fn style(ln: &str, st: State) -> (Styled, State) {
        line(ln, false, st, 14.0, &ink(), &|t| t == "Ideas")
    }

    /// The colour the last span over `[a, b)` gives it, as Rune's flattening does.
    fn color_at(s: &Styled, at: usize) -> Option<Color> {
        s.spans.iter().filter(|x| x.start as usize <= at && at < x.end as usize).filter_map(|x| x.color).last()
    }

    fn span_at(s: &Styled, at: usize) -> Span {
        let mut m = Span::new(at, at + 1);
        for x in s.spans.iter().filter(|x| x.start as usize <= at && at < x.end as usize) {
            if x.weight.is_some() {
                m.weight = x.weight;
            }
            if x.color.is_some() {
                m.color = x.color;
            }
            if x.size.is_some() {
                m.size = x.size;
            }
            if x.bg.is_some() {
                m.bg = x.bg;
            }
            m.mono |= x.mono;
            m.italic |= x.italic;
        }
        m
    }

    #[test]
    fn marks_next_to_wide_characters_style_without_panicking() {
        // Every emphasis mark at every spot around characters of two, three and four bytes:
        // the scanner steps byte by byte and must not cut one of them in half.
        for ln in ["a…*b*", "…_x_…", "→**b**…", "é*", "*…", "~~é~~", "==😀==", "“_”", "*a…*", "x…_"] {
            style(ln, State::default());
        }
        let (s, _) = style("so… *this* stays", State::default());
        assert!(span_at(&s, "so… *".len()).italic, "the mark after the ellipsis still works");
    }

    #[test]
    fn wr_04_state_carries_and_lines_repeat() {
        let (_, s1) = style("```rust", State::default());
        assert_eq!(s1.block, Block::Fence(b'`', 3));
        let (a, s2) = style("let x = **1**;", s1);
        assert_eq!(color_at(&a, 9), Some(ink().code), "no inline parsing in a fence (WR-10)");
        assert_eq!(s2, s1);
        assert_eq!(style("let x = **1**;", s1), (a, s2), "same text and state, same spans");
        let (c, s3) = style("```", s2);
        assert_eq!((color_at(&c, 0), s3.block), (Some(ink().faint), Block::None));
        assert_eq!(style("````", State { block: Block::Fence(b'`', 4), ..s3 }).1.block, Block::None);
        assert_eq!(style("```", State { block: Block::Fence(b'`', 4), ..s3 }).1.block, Block::Fence(b'`', 4), "a shorter run does not close");
    }

    #[test]
    fn wr_09_fences() {
        assert_eq!(fence_open("   ~~~ python"), Some((b'~', 3)));
        assert_eq!(fence_open("    ```"), None);
        assert_eq!(fence_open("``` a`b"), None);
        let (s, _) = style("```js", State::default());
        assert_eq!(span_at(&s, 0).size, Some(12.0));
        assert!(span_at(&s, 0).mono);
    }

    #[test]
    fn wr_12_13_front_matter_and_comments() {
        let (_, s) = line("---", true, State::default(), 14.0, &ink(), &|_| false);
        assert_eq!(s.block, Block::Front);
        assert_eq!(line("---", false, State::default(), 14.0, &ink(), &|_| false).1.block, Block::None, "only on line 1");
        let (k, s) = style("title: Notes", s);
        assert_eq!((color_at(&k, 2), color_at(&k, 8)), (Some(ink().mark), Some(ink().dim)));
        assert_eq!(style("...", s).1.block, Block::None);
        let (_, c) = style("<!-- a", State::default());
        assert_eq!(c.block, Block::Comment);
        let (x, c) = style("**b** -->", c);
        assert_eq!((color_at(&x, 2), c.block), (Some(ink().faint), Block::None));
        assert_eq!(style("<!-- one line -->", State::default()).1.block, Block::None);
    }

    #[test]
    fn wr_14_15_headings() {
        let (s, _) = style("## Plain *files* ##", State::default());
        let h = span_at(&s, 4);
        assert_eq!((h.weight, h.color, h.size), (Some(700.0), Some(ink().strong), Some(18.0)));
        let m = span_at(&s, 0);
        assert_eq!((m.weight, m.color, m.size), (Some(400.0), Some(ink().faint), Some(18.0)));
        assert_eq!(color_at(&s, 17), Some(ink().faint), "the closing run");
        assert!(span_at(&s, 10).italic, "inline rules run in the heading");
        assert_eq!(atx("#tag"), None);
        assert_eq!(atx("#"), Some((1, 1, 1)));
        assert_eq!(heading_size(14.0, 1), 21.0);
        assert_eq!(heading_size(14.0, 3), 16.0);
        assert_eq!(heading_size(14.0, 5), 14.0);
    }

    #[test]
    fn wr_17_18_breaks_and_setext() {
        assert_eq!(color_at(&style("* * *", State::default()).0, 0), Some(ink().faint));
        let after = State { block: Block::None, after_text: true };
        assert_eq!(color_at(&style("===", after).0, 0), Some(ink().faint));
        assert_eq!(color_at(&style("===", State::default()).0, 0), None);
    }

    #[test]
    fn wr_19_20_21_quotes_lists_tasks() {
        let (q, _) = style("> > - item", State::default());
        assert_eq!(color_at(&q, 2), Some(ink().mark));
        let s = span_at(&q, 6);
        assert!(s.italic && s.color == Some(ink().dim), "a list in a quote is a quote (WR-08)");
        let (l, _) = style("  12) [x] done **b**", State::default());
        assert_eq!(color_at(&l, 2), Some(ink().mark));
        assert!(span_at(&l, 6).mono);
        assert_eq!(color_at(&l, 11), Some(ink().faint));
        assert_eq!(l.strikes, vec![(10, 20, ink().faint)]);
        assert_eq!(color_at(&style("-x", State::default()).0, 0), None);
    }

    #[test]
    fn wr_23_24_25_tables_refs_footnotes() {
        assert_eq!(color_at(&style("|---|:--:|", State::default()).0, 1), Some(ink().faint));
        assert_eq!(color_at(&style("[id]: https://x.org", State::default()).0, 1), Some(ink().faint));
        let (f, _) = style("[^1]: note *a*", State::default());
        assert_eq!((color_at(&f, 2), color_at(&f, 6)), (Some(ink().mark), None));
        let (t, _) = style("| a | b\\| |", State::default());
        assert_eq!((color_at(&t, 0), color_at(&t, 4), color_at(&t, 8), color_at(&t, 10)), (Some(ink().faint), Some(ink().faint), None, Some(ink().faint)));
        assert_eq!(color_at(&t, 7), Some(ink().faint), "the escape's backslash");
    }

    #[test]
    fn wr_27_28_escapes_and_code() {
        let (s, _) = style(r"\*not* and `a **b**` x", State::default());
        assert_eq!(color_at(&s, 0), Some(ink().faint));
        assert!(!span_at(&s, 3).italic, "an escaped mark opens nothing");
        let chip = span_at(&s, 14);
        assert_eq!((chip.mono, chip.bg, chip.color, chip.weight), (true, Some(ink().code_bg), Some(ink().code), None));
        assert_eq!(color_at(&s, 11), Some(ink().faint), "the backticks are faint on the chip");
        let (d, _) = style("``a ` b``", State::default());
        assert_eq!(span_at(&d, 4).bg, Some(ink().code_bg));
    }

    #[test]
    fn wr_29_to_35_html_links_urls_footnotes() {
        let (s, _) = style("<b>x</b> [text](u \"t\") ![alt](i.png) [r][id]", State::default());
        assert_eq!((color_at(&s, 0), color_at(&s, 5)), (Some(ink().faint), Some(ink().faint)));
        assert_eq!((color_at(&s, 9), color_at(&s, 10)), (Some(ink().faint), Some(ink().link)));
        assert_eq!(span_at(&s, 15).size, Some(12.0));
        let alt = span_at(&s, 26);
        assert!(alt.italic && alt.color == Some(ink().dim));
        assert_eq!((color_at(&s, 38), color_at(&s, 40)), (Some(ink().link), Some(ink().faint)));
        let (u, _) = style("see https://a.org/x). and <mailto:me@x.org> [^n]", State::default());
        assert_eq!((color_at(&u, 4), color_at(&u, 18), color_at(&u, 19)), (Some(ink().link), Some(ink().link), None));
        assert_eq!((color_at(&u, 26), color_at(&u, 27)), (Some(ink().faint), Some(ink().link)));
        assert_eq!(color_at(&u, 45), Some(ink().mark));
        assert_eq!(color_at(&style("x(https://a.org)", State::default()).0, 3), None);
    }

    #[test]
    fn wr_36_37_emphasis() {
        let (s, _) = style("**b** *i* _u_ ~~s~~ ==h== snake_case 2*3*4", State::default());
        assert_eq!(span_at(&s, 2).weight, Some(700.0));
        assert_eq!(color_at(&s, 0), Some(ink().faint));
        assert!(span_at(&s, 7).italic && span_at(&s, 11).italic);
        assert_eq!(s.strikes, vec![(16, 17, ink().dim)]);
        assert_eq!(span_at(&s, 22).bg, Some(ink().wash));
        assert!(!span_at(&s, 32).italic, "no italic inside a word");
        let (n, _) = style("** not bold**", State::default());
        assert_eq!(span_at(&n, 4).weight, None);
        let (bi, _) = style("***both***", State::default());
        assert_eq!(span_at(&bi, 4).weight, Some(700.0));
    }

    #[test]
    fn wr_220_221_note_links_and_tags() {
        let (s, _) = style("[[Ideas]] [[Gone]] ![[a.png]] #tag x#no", State::default());
        assert_eq!((color_at(&s, 0), color_at(&s, 3), color_at(&s, 12)), (Some(ink().faint), Some(ink().link), Some(ink().dim)));
        assert!(span_at(&s, 22).italic);
        assert_eq!(color_at(&s, 31), Some(ink().mark));
        assert_eq!(color_at(&s, 37), None);
    }

    #[test]
    fn wr_82_fade_swaps_colours() {
        let (s, _) = style("a `c` **b**", State::default());
        let f = fade(&s, 11, ink().ghost);
        assert!(f.spans.iter().all(|x| x.color == Some(ink().ghost) && x.bg.is_none()));
        assert_eq!(span_at(&f, 8).weight, Some(700.0));
    }

    #[test]
    fn wr_188_count() {
        let t = "---\ntitle: x\n---\n# Two words\n\n```\nnot counted\n```\nIt's [a link](https://x.org) e.g. well-known <b>tag</b> https://y.org";
        let c = count(t);
        // Two, words, It's, a, link, e.g, well, known, tag.
        assert_eq!(c.words, 9);
        assert_eq!(c.minutes, 1);
        assert_eq!(count("").minutes, 0);
        assert_eq!(count("a b").chars, 2);
        // a'b, a, b, a, b, e.g, foo, bar -> 8 words.
        assert_eq!(count_words("a'b a''b a.'b e.g. foo' bar."), 8);
        assert_eq!(count_words("don\u{2019}t stop\u{2019}\u{2019}now"), 3);

        // WORDSEL: range count excludes front matter and open code fences identically to `count`.
        let doc = "---\ntitle: Front Matter Words\ntags: [alpha, beta]\n---\n\nOne two three four.\n\n```rust\nlet hidden = five_six;\n```\n\nSeven eight.\n";
        assert_eq!(count(doc).words, 6);
        assert_eq!(count_words_range(doc, 0, doc.len()), count(doc).words, "Ctrl+A matches total");
        let front_end = strip_front_offset(doc);
        assert_eq!(count_words_range(doc, 0, front_end), 0, "selection inside front matter is 0");
        assert_eq!(count_words_range(doc, 4, 25), 0, "partial front-matter selection is 0");
        let two_end = doc.find("three").unwrap();
        assert_eq!(count_words_range(doc, 10, two_end), 2, "selection crossing delimiter counts body only");
        let code_a = doc.find("let hidden").unwrap();
        let code_b = doc.find(";\n").unwrap() + 1;
        assert_eq!(count_words_range(doc, code_a, code_b), 0, "selection starting mid-fence is 0");
        let seven_a = doc.find("Seven").unwrap();
        assert_eq!(count_words_range(doc, code_a, doc.len()), 2, "mid-fence to end counts trailing prose only");
        assert_eq!(count_words_range(doc, doc.len(), seven_a), 2, "reversed selection range normalizes");
    }

    #[test]
    fn livepace_line_and_fade_spans_are_sorted_and_disjoint() {
        let i = ink();
        let long = crate::bench::LONG;
        let mut st = State::default();
        for (idx, ln) in long.split('\n').enumerate() {
            let (raw, next_raw) = line_raw(ln, idx == 0, st, 14.0, &i, &|t| t == "Ideas");
            let (flat, next_flat) = line(ln, idx == 0, st, 14.0, &i, &|t| t == "Ideas");
            assert_eq!(next_raw, next_flat);
            assert!(
                flat.spans.iter().all(|s| s.start < s.end) && flat.spans.windows(2).all(|w| w[0].end <= w[1].start),
                "line {idx} spans must be strictly non-empty, sorted, and disjoint: {:?}",
                flat.spans
            );
            // Every byte resolves to the exact same merged attributes as unflattened spans.
            for b in 0..ln.len() {
                assert_eq!(span_at(&raw, b), span_at(&flat, b), "line {idx} byte {b}");
            }
            let faded = fade(&flat, ln.len(), i.ghost);
            assert!(
                faded.spans.iter().all(|s| s.start < s.end) && faded.spans.windows(2).all(|w| w[0].end <= w[1].start),
                "line {idx} faded spans must be strictly non-empty, sorted, and disjoint: {:?}",
                faded.spans
            );
            st = next_flat;
        }
    }

    fn run(text: &str, at: usize, f: impl Fn(&str, usize, usize) -> Option<Edit>) -> Option<(String, usize)> {
        f(text, at, at).map(|e| (format!("{}{}{}", &text[..e.start], e.insert, &text[e.end..]), e.sel.0))
    }

    #[test]
    fn wr_39_to_44_enter() {
        assert_eq!(run("1. a", 4, enter), Some(("1. a\n2. ".into(), 8)));
        assert_eq!(run("9) a", 4, enter), Some(("9) a\n10) ".into(), 9)));
        assert_eq!(run("  - [x] a", 9, enter), Some(("  - [x] a\n  - [ ] ".into(), 18)));
        assert_eq!(run("- one\n- ", 8, enter), Some(("- one\n".into(), 6)));
        assert_eq!(run("> > q", 5, enter), Some(("> > q\n> > ".into(), 10)));
        assert_eq!(run("> ", 2, enter), Some(("".into(), 0)));
        assert_eq!(run("- item", 1, enter), None, "inside the prefix");
        assert_eq!(run("plain", 5, enter), None);
        assert_eq!(enter("- a", 0, 3), None, "a selection");
    }

    #[test]
    fn wr_47_48_50_tab() {
        assert_eq!(run("2. x", 4, tab), Some(("   2. x".into(), 7)));
        assert_eq!(run("10. x", 5, tab), Some(("    10. x".into(), 9)));
        assert_eq!(run("- x", 3, tab), Some(("  - x".into(), 5)));
        assert_eq!(run("abc", 1, tab), Some(("a    bc".into(), 5)));
        assert_eq!(run("   2. x", 7, shift_tab), Some(("2. x".into(), 4)));
        assert_eq!(run(" - x", 1, shift_tab), Some(("- x".into(), 0)));
        assert_eq!(run("- x", 3, shift_tab), None);
        assert_eq!(run("text", 2, shift_tab), None);
        // A selection indents the list lines it touches (open question, WR-49).
        let e = tab("- a\nb\n- c", 0, 9).unwrap();
        assert_eq!(e.insert, "  - a\nb\n  - c");
        assert_eq!(e.sel, (2, 13));
    }

    #[test]
    fn wr_59_60_61_headings() {
        let h = |t: &str, at: usize, l: usize| {
            let e = heading(t, at, l);
            (format!("{}{}{}", &t[..e.start], e.insert, &t[e.end..]), e.sel.0)
        };
        assert_eq!(h("word", 4, 2), ("## word".into(), 7));
        assert_eq!(h("## word", 7, 2), ("word".into(), 4));
        assert_eq!(h("### word", 5, 0), ("word".into(), 1));
        assert_eq!(h("#idea", 0, 2), ("## #idea".into(), 3));
        assert_eq!(h("- item", 6, 1), ("# - item".into(), 8));
        assert_eq!(h("# a\nb", 5, 3), ("# a\n### b".into(), 9));
    }

    #[test]
    fn toggle_checkbox_and_move_lines() {
        let ed = toggle_checkbox("- [ ] milk\n- [x] bread", 2, 2).unwrap();
        let mut s = "- [ ] milk\n- [x] bread".to_string();
        s.replace_range(ed.start..ed.end, &ed.insert);
        assert_eq!(s, "- [x] milk\n- [x] bread");
        assert_eq!(ed.sel, (2, 2));

        let ed = toggle_checkbox(&s, 0, s.len()).unwrap();
        s.replace_range(ed.start..ed.end, &ed.insert);
        assert_eq!(s, "- [ ] milk\n- [ ] bread");

        let ed = toggle_checkbox("plain line", 3, 3).unwrap();
        let mut p = "plain line".to_string();
        p.replace_range(ed.start..ed.end, &ed.insert);
        assert_eq!(p, "- [ ] plain line");

        let text = "one\ntwo\nthree";
        let up = move_lines(text, 5, 5, false).unwrap();
        let mut t = text.to_string();
        t.replace_range(up.start..up.end, &up.insert);
        assert_eq!(t, "two\none\nthree");
        assert_eq!(up.sel, (1, 1));
        assert!(move_lines(&t, 1, 1, false).is_none());

        let dn = move_lines(text, 1, 1, true).unwrap();
        let mut t = text.to_string();
        t.replace_range(dn.start..dn.end, &dn.insert);
        assert_eq!(t, "two\none\nthree");
        assert_eq!(dn.sel, (5, 5));
        assert!(move_lines(text, 10, 10, true).is_none());
    }
}
