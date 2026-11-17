//! Markdown analysis shared by the editor, the preview, the panels and the MCP server.
//! It is a port of the design prototype's logic, so the results match it line for line.

use std::collections::BTreeMap;

/// The file name of a note without its folder and without `.md`.
pub fn base(p: &str) -> &str {
    let name = p.rsplit('/').next().unwrap_or(p);
    strip_md(name)
}

/// The folder part of a note path, or an empty string for a root note.
pub fn dir(p: &str) -> &str {
    p.rfind('/').map_or("", |i| &p[..i])
}

pub fn join(d: &str, n: &str) -> String {
    if d.is_empty() { n.to_string() } else { format!("{d}/{n}") }
}

pub fn strip_md(p: &str) -> &str {
    if p.len() >= 3 && p[p.len() - 3..].eq_ignore_ascii_case(".md") { &p[..p.len() - 3] } else { p }
}

/// Makes a name safe as a file name: path separators and colons become dashes.
pub fn clean_name(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\\' | '/' | ':') { '-' } else { c }).collect::<String>().trim().to_string()
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Lowercased lookup from a note name (or a path without `.md`) to its path.
#[derive(Default, Clone, Hash)]
pub struct Names(BTreeMap<String, String>);

impl Names {
    pub fn build(files: &BTreeMap<String, String>) -> Names {
        let mut m = BTreeMap::new();
        for p in files.keys() {
            m.entry(base(p).to_lowercase()).or_insert_with(|| p.clone());
            m.insert(strip_md(p).to_lowercase(), p.clone());
        }
        Names(m)
    }
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.0.get(&name.trim().to_lowercase()).map(String::as_str)
    }
}

/// The target of a `[[link|alias]]`, without the alias.
pub fn link_target(inner: &str) -> String {
    inner.split('|').next().unwrap_or("").trim().replace('\u{a0}', " ")
}

/// A wiki link's note name and heading: `[[note#Heading|alias]]` gives ("note", "Heading").
/// An empty name points into the note the link is in.
pub fn wiki_parts(inner: &str) -> (String, Option<String>) {
    let t = link_target(inner);
    match t.split_once('#') {
        Some((name, frag)) => (name.trim().to_string(), Some(frag.trim().to_string()).filter(|f| !f.is_empty())),
        None => (t, None),
    }
}

/// CSS `text-wrap: pretty` keeps a paragraph from ending on one word. Joining the last two
/// words with a no-break space has the same effect on the last line.
pub fn pretty(t: &str) -> String {
    let t = t.trim_end();
    match t.rfind(' ') {
        Some(i) if t[..i].trim_end().contains(' ') => format!("{}\u{a0}{}", &t[..i], &t[i + 1..]),
        _ => t.to_string(),
    }
}

// ---------- Inline tokens ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Code,
    Wiki,
    Bold,
    Italic,
    Strike,
    Link,
    Tag,
}

/// Finds inline Markdown tokens in one line, as the prototype's `inlineRe` does: the leftmost
/// match wins, and at one position the first kind in this order.
pub fn inline_tokens(s: &str) -> Vec<(Tok, std::ops::Range<usize>)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some((t, end)) = token_at(s, i) {
            out.push((t, i..end));
            i = end;
        } else {
            i += s[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn prev_char(s: &str, i: usize) -> Option<char> {
    s[..i].chars().next_back()
}

/// Index of the first byte at or after `from` that is `stop`, scanning while `ok` holds.
fn scan(b: &[u8], from: usize, ok: impl Fn(u8) -> bool) -> usize {
    let mut j = from;
    while j < b.len() && ok(b[j]) {
        j += 1;
    }
    j
}

fn token_at(s: &str, i: usize) -> Option<(Tok, usize)> {
    let b = s.as_bytes();
    let at = |k: usize| b.get(k).copied();
    let c = b[i];
    // `code`
    if c == b'`' {
        let j = scan(b, i + 1, |x| x != b'`' && x != b'\n');
        if j > i + 1 && at(j) == Some(b'`') {
            return Some((Tok::Code, j + 1));
        }
    }
    // [[wiki]]
    if c == b'[' && at(i + 1) == Some(b'[') {
        let j = scan(b, i + 2, |x| x != b']' && x != b'\n');
        if j > i + 2 && at(j) == Some(b']') && at(j + 1) == Some(b']') {
            return Some((Tok::Wiki, j + 2));
        }
    }
    // **bold** or __bold__
    if (c == b'*' || c == b'_') && at(i + 1) == Some(c) {
        let j = scan(b, i + 2, |x| x != c && x != b'\n');
        if j > i + 2 && at(j) == Some(c) && at(j + 1) == Some(c) {
            return Some((Tok::Bold, j + 2));
        }
    }
    // *italic*
    if c == b'*' {
        if let Some(n) = at(i + 1) {
            if n != b'*' && !(n as char).is_whitespace() {
                let j = scan(b, i + 2, |x| x != b'*' && x != b'\n');
                if at(j) == Some(b'*') {
                    return Some((Tok::Italic, j + 1));
                }
            }
        }
    }
    // _italic_ with no word character on either side
    if c == b'_' && !prev_char(s, i).is_some_and(is_word) {
        if let Some(n) = at(i + 1) {
            if n != b'_' && !(n as char).is_whitespace() {
                let j = scan(b, i + 2, |x| x != b'_' && x != b'\n');
                if at(j) == Some(b'_') && !s[j + 1..].chars().next().is_some_and(is_word) {
                    return Some((Tok::Italic, j + 1));
                }
            }
        }
    }
    // ~~strike~~
    if c == b'~' && at(i + 1) == Some(b'~') {
        let j = scan(b, i + 2, |x| x != b'~' && x != b'\n');
        if j > i + 2 && at(j) == Some(b'~') && at(j + 1) == Some(b'~') {
            return Some((Tok::Strike, j + 2));
        }
    }
    // [text](url)
    if c == b'[' {
        let j = scan(b, i + 1, |x| x != b']' && x != b'\n');
        if j > i + 1 && at(j) == Some(b']') && at(j + 1) == Some(b'(') {
            let k = scan(b, j + 2, |x| x != b')' && !(x as char).is_whitespace());
            if k > j + 2 && at(k) == Some(b')') {
                return Some((Tok::Link, k + 1));
            }
        }
    }
    // #tag
    if c == b'#' && !prev_char(s, i).is_some_and(|p| is_word(p) || p == '#' || p == '&' || p == '/') {
        if at(i + 1).is_some_and(|x| x.is_ascii_alphabetic()) {
            let j = scan(b, i + 2, |x| x.is_ascii_alphanumeric() || x == b'_' || x == b'/' || x == b'-');
            return Some((Tok::Tag, j));
        }
    }
    None
}

// ---------- Line kinds (source highlighting and list continuation) ----------

/// `^(#{1,6})(\s+)(.*)$`: the marker with its spaces, and the heading text.
pub fn heading_line(ln: &str) -> Option<(usize, usize)> {
    let h = ln.bytes().take_while(|&c| c == b'#').count();
    if !(1..=6).contains(&h) {
        return None;
    }
    let sp = ln[h..].chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    if sp == 0 {
        return None;
    }
    Some((h, h + sp))
}

/// A list line: `^(\s*)([-*+]|\d+[.)])(\s+)(\[[ xX]\]\s)?(.*)$`.
#[derive(Clone, Debug)]
pub struct ListLine {
    pub indent: usize,
    /// The marker's byte range.
    pub marker: std::ops::Range<usize>,
    /// The end of the spaces after the marker.
    pub after: usize,
    /// The task box `[ ] ` byte range, with one trailing space.
    pub task: Option<std::ops::Range<usize>>,
    pub done: bool,
    /// The number of an ordered marker and its closing `.` or `)`.
    pub number: Option<(u64, char)>,
}

pub fn list_line(ln: &str) -> Option<ListLine> {
    let b = ln.as_bytes();
    let indent = ln.len() - ln.trim_start().len();
    let mut i = indent;
    let start = i;
    let mut number = None;
    match b.get(i) {
        Some(b'-' | b'*' | b'+') => i += 1,
        Some(c) if c.is_ascii_digit() => {
            let j = scan(b, i, |x| x.is_ascii_digit());
            match b.get(j) {
                Some(&p @ (b'.' | b')')) => {
                    number = Some((ln[i..j].parse::<u64>().unwrap_or(0), p as char));
                    i = j + 1;
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    let marker = start..i;
    let sp = ln[i..].chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    if sp == 0 {
        return None;
    }
    let after = i + sp;
    let mut task = None;
    let mut done = false;
    if b.get(after) == Some(&b'[') && b.get(after + 2) == Some(&b']') {
        if let Some(&m) = b.get(after + 1) {
            if matches!(m, b' ' | b'x' | b'X') {
                if let Some(c) = ln[after + 3..].chars().next() {
                    if c.is_whitespace() {
                        task = Some(after..after + 3 + c.len_utf8());
                        done = m != b' ';
                    }
                }
            }
        }
    }
    Some(ListLine { indent, marker, after, task, done, number })
}

// ---------- Document facts ----------

#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub line: usize,
}

pub fn headings(text: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence = false;
    for (i, ln) in text.split('\n').enumerate() {
        if ln.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if let Some((h, start)) = heading_line(ln) {
            let mut t = ln[start..].trim_end();
            t = t.trim_end_matches('#').trim_end();
            if t.is_empty() {
                continue;
            }
            let clean = t.replace("**", "").replace("__", "").replace("[[", "").replace("]]", "");
            let clean: String = clean.chars().filter(|c| !matches!(c, '*' | '_' | '`')).collect();
            out.push(Heading { level: h, text: clean, line: i });
        }
    }
    out
}

fn strip_code(t: &str) -> String {
    // Remove fenced blocks, then inline code spans.
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(a) = rest.find("```") {
        out.push_str(&rest[..a]);
        match rest[a + 3..].find("```") {
            Some(b) => rest = &rest[a + 3 + b + 3..],
            None => {
                rest = &rest[a..];
                break;
            }
        }
    }
    out.push_str(rest);
    let mut res = String::with_capacity(out.len());
    for line in out.split_inclusive('\n') {
        let mut l = line;
        loop {
            let Some(a) = l.find('`') else { break };
            let Some(b) = l[a + 1..].find('`') else { break };
            if b == 0 {
                res.push_str(&l[..a + 1]);
                l = &l[a + 1..];
                continue;
            }
            res.push_str(&l[..a]);
            l = &l[a + 1 + b + 1..];
        }
        res.push_str(l);
    }
    res
}

/// Every `#tag` in a note, lowercased and once each.
pub fn note_tags(text: &str) -> Vec<String> {
    let t = strip_code(text);
    let mut seen: Vec<String> = Vec::new();
    let b = t.as_bytes();
    for (i, c) in t.char_indices() {
        if c != '#' {
            continue;
        }
        let ok_before = match prev_char(&t, i) {
            None => true,
            Some(p) => p.is_whitespace() || p == '(',
        };
        if !ok_before || !b.get(i + 1).is_some_and(|x| x.is_ascii_alphabetic()) {
            continue;
        }
        let j = scan(b, i + 2, |x| x.is_ascii_alphanumeric() || x == b'_' || x == b'/' || x == b'-');
        let tag = t[i + 1..j].to_lowercase();
        if !seen.contains(&tag) {
            seen.push(tag);
        }
    }
    seen
}

/// Tags across all notes with the number of notes using each, most used first.
pub fn tags(files: &BTreeMap<String, String>) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for t in files.values() {
        for tag in note_tags(t) {
            *m.entry(tag).or_default() += 1;
        }
    }
    let mut v: Vec<(String, usize)> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// The `[[...]]` links in a text, as inner text.
pub fn wiki_links(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut off = 0;
    while let Some(a) = rest.find("[[") {
        let after = &rest[a + 2..];
        let end = after.find(|c| c == ']' || c == '\n');
        match end {
            Some(e) if e > 0 && after[e..].starts_with("]]") => {
                out.push(&text[off + a + 2..off + a + 2 + e]);
                let adv = a + 2 + e + 2;
                off += adv;
                rest = &rest[adv..];
            }
            _ => {
                off += a + 2;
                rest = &rest[a + 2..];
            }
        }
    }
    out
}

/// Notes that link to `active`, with the number of links in each.
pub fn backlinks(files: &BTreeMap<String, String>, active: &str) -> Vec<(String, usize)> {
    let b = base(active).to_lowercase();
    let full = strip_md(active).to_lowercase();
    let mut out = Vec::new();
    for (p, t) in files {
        if p == active {
            continue;
        }
        let n = wiki_links(t).iter().filter(|x| {
            let t = link_target(x).to_lowercase();
            t == b || t == full
        }).count();
        if n > 0 {
            out.push((p.clone(), n));
        }
    }
    out
}

/// The paragraph (run of non-blank lines) around byte `pos`, as first and last line index.
pub fn para_range(text: &str, pos: usize) -> (usize, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut li = 0;
    let mut acc = 0;
    while li < lines.len() - 1 {
        if pos <= acc + lines[li].len() {
            break;
        }
        acc += lines[li].len() + 1;
        li += 1;
    }
    if lines[li].trim().is_empty() {
        return (li, li);
    }
    let (mut a, mut b) = (li, li);
    while a > 0 && !lines[a - 1].trim().is_empty() {
        a -= 1;
    }
    while b < lines.len() - 1 && !lines[b + 1].trim().is_empty() {
        b += 1;
    }
    (a, b)
}

/// Fuzzy match of `q` in `s`: a score and the matched character indices.
pub fn fuzzy(q: &str, s: &str) -> Option<(f32, Vec<usize>)> {
    let ql: Vec<char> = q.to_lowercase().chars().collect();
    let sl: Vec<char> = s.to_lowercase().chars().collect();
    if ql.is_empty() {
        return Some((0.0, vec![]));
    }
    let slen = s.chars().count() as f32;
    if let Some(sub) = sl.windows(ql.len()).position(|w| w == ql.as_slice()) {
        let idx = (sub..sub + ql.len()).collect();
        let score = 1000.0 - sub as f32 * 2.0 - slen * 0.1 + if sub == 0 { 50.0 } else { 0.0 };
        return Some((score, idx));
    }
    let (mut j, mut prev, mut score) = (0usize, -2i64, 0.0f32);
    let mut idx = Vec::new();
    for (i, &c) in sl.iter().enumerate() {
        if j >= ql.len() {
            break;
        }
        if c == ql[j] {
            idx.push(i);
            let boundary = i == 0 || matches!(sl[i - 1], ' ' | '\t' | '/' | '_' | '-');
            score += if i as i64 == prev + 1 { 4.0 } else { 1.0 } + if boundary { 3.0 } else { 0.0 };
            prev = i as i64;
            j += 1;
        }
    }
    (j == ql.len()).then_some((score, idx))
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub line: usize,
    /// Byte offset of the match in its line.
    pub col: usize,
    /// Byte offset of the match in the note.
    pub pos: usize,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub path: String,
    pub hits: Vec<Hit>,
    pub name_hit: bool,
}

/// Case-insensitive full-text search, sorted as the prototype sorts.
pub fn search(files: &BTreeMap<String, String>, q: &str) -> Vec<SearchResult> {
    let ql = q.to_lowercase();
    let mut out = Vec::new();
    if ql.is_empty() {
        return out;
    }
    for (p, t) in files {
        // A locked note's text is sealed, so search has nothing to find in it.
        if t.starts_with(crate::lock::MAGIC) {
            continue;
        }
        let mut hits = Vec::new();
        let mut off = 0;
        for (li, ln) in t.split('\n').enumerate() {
            let low = ln.to_lowercase();
            // Lowercasing can change byte lengths; only trust offsets when it does not.
            if low.len() == ln.len() {
                let mut from = 0;
                while let Some(i) = low[from..].find(&ql) {
                    let col = from + i;
                    hits.push(Hit { line: li, col, pos: off + col });
                    from = col + ql.len().max(1);
                    if from >= low.len() {
                        break;
                    }
                }
            } else if low.contains(&ql) {
                hits.push(Hit { line: li, col: 0, pos: off });
            }
            off += ln.len() + 1;
        }
        let name_hit = base(p).to_lowercase().contains(&ql);
        if !hits.is_empty() || name_hit {
            out.push(SearchResult { path: p.clone(), hits, name_hit });
        }
    }
    out.sort_by(|a, b| {
        (b.name_hit as u8).cmp(&(a.name_hit as u8)).then(b.hits.len().cmp(&a.hits.len())).then_with(|| a.path.cmp(&b.path))
    });
    out
}

/// A search hit's line as the sidebar shows it (C17): its Markdown syntax gone, cut at word
/// edges to what `rows` lines of `cols` monospaced characters hold, with whole words ahead of
/// the match while they fit `before` characters, and "…" where it was cut. Returns the text and
/// the match's byte range in it, empty when the match was all syntax, such as a link's address.
pub fn snippet(line: &str, col: usize, len: usize, before: usize, cols: usize, rows: usize) -> (String, std::ops::Range<usize>) {
    let (plain, map) = strip_line(line);
    let inside: Vec<usize> = (0..plain.len()).filter(|&i| plain.is_char_boundary(i) && (col..col + len).contains(&map[i])).collect();
    let m0 = inside.first().copied().unwrap_or_else(|| (0..plain.len()).find(|&i| map[i] >= col).unwrap_or(plain.len()));
    let m1 = inside.last().map_or(m0, |&i| i + plain[i..].chars().next().map_or(1, char::len_utf8));
    let space = |i: usize| plain.as_bytes()[i] == b' ';
    let starts: Vec<usize> = (0..=plain.len()).filter(|&i| plain.is_char_boundary(i) && (i == 0 || (i < plain.len() && !space(i) && space(i - 1)))).collect();
    let ends: Vec<usize> = (0..=plain.len()).filter(|&i| plain.is_char_boundary(i) && (i == plain.len() || (i > 0 && space(i) && !space(i - 1)))).collect();
    let show = |a: usize, b: usize| format!("{}{}{}", if a > 0 { "…" } else { "" }, &plain[a..b], if b < plain.len() { "…" } else { "" });
    // The match's own words always show.
    let first = starts.iter().rev().find(|&&a| a <= m0).copied().unwrap_or(0);
    let last = ends.iter().find(|&&b| b >= m1).copied().unwrap_or(plain.len());
    let a = starts.iter().copied().filter(|&a| a <= first && plain[a..m0].chars().count() <= before && wrapped(&show(a, last), cols) <= rows).min().unwrap_or(first);
    let b = ends.iter().copied().filter(|&b| b >= last && wrapped(&show(a, b), cols) <= rows).max().unwrap_or(last);
    let lead = if a > 0 { "…".len() } else { 0 };
    (show(a, b), m0 - a + lead..m1 - a + lead)
}

/// How many lines of `cols` monospaced characters `text` takes, broken at spaces.
fn wrapped(text: &str, cols: usize) -> usize {
    let (mut lines, mut used) = (1, 0);
    for w in text.split(' ').map(|w| w.chars().count()) {
        let need = if used == 0 { w } else { used + 1 + w };
        if need <= cols || used == 0 {
            used = need;
        } else {
            lines += 1 + (w.saturating_sub(1)) / cols.max(1);
            used = w % cols.max(1);
        }
    }
    lines
}

/// One line of Markdown as text: no heading, quote, list or task marks, and inline syntax
/// reduced to what it shows. Each byte of the text comes with its byte in `ln`.
fn strip_line(ln: &str) -> (String, Vec<usize>) {
    let mut i = ln.len() - ln.trim_start().len();
    loop {
        let rest = &ln[i..];
        let hashes = rest.len() - rest.trim_start_matches('#').len();
        if (1..=6).contains(&hashes) && rest[hashes..].starts_with(' ') {
            i += hashes + 1;
        } else if let Some(r) = rest.strip_prefix('>') {
            i += 1 + (r.len() - r.trim_start().len());
        } else if let Some(l) = list_line(rest) {
            i += l.task.map_or(l.after, |t| t.end).max(l.after);
        } else {
            break;
        }
    }
    let (mut out, mut map) = (String::new(), Vec::new());
    strip_inline(&ln[i..], i, &mut out, &mut map);
    let lead = out.len() - out.trim_start().len();
    (out.trim().to_string(), map[lead..lead + out.trim().len()].to_vec())
}

fn strip_inline(s: &str, base: usize, out: &mut String, map: &mut Vec<usize>) {
    let keep = |from: usize, to: usize, out: &mut String, map: &mut Vec<usize>| {
        out.push_str(&s[from..to]);
        map.extend((from..to).map(|k| base + k));
    };
    let mut at = 0;
    for (tok, r) in inline_tokens(s) {
        let mut upto = r.start;
        // An image's "!" goes with its link.
        if matches!(tok, Tok::Link) && s[..r.start].ends_with('!') {
            upto -= 1;
        }
        keep(at, upto, out, map);
        let t = &s[r.clone()];
        let inner = match tok {
            Tok::Tag => Some(0..t.len()),
            Tok::Code => {
                let n = t.len() - t.trim_start_matches('`').len();
                Some(n..t.len() - n)
            }
            Tok::Bold | Tok::Strike => Some(2..t.len() - 2),
            Tok::Italic => Some(1..t.len() - 1),
            Tok::Wiki => {
                let body = 2..t.len() - 2;
                Some(t[body.clone()].find('|').map_or(body.clone(), |bar| body.start + bar + 1..body.end))
            }
            Tok::Link => t.find("](").map(|close| 1..close),
        };
        match inner {
            Some(x) if x.start <= x.end && matches!(tok, Tok::Code | Tok::Tag) => keep(r.start + x.start, r.start + x.end, out, map),
            Some(x) if x.start <= x.end => strip_inline(&t[x.clone()], base + r.start + x.start, out, map),
            _ => keep(r.start, r.end, out, map),
        }
        at = r.end;
    }
    keep(at, s.len(), out, map);
}

/// Line index and byte offset of the line start for byte `pos`.
pub fn line_col(text: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(text.len());
    let mut line = 0;
    let mut ls = 0;
    for (i, b) in text.as_bytes()[..pos].iter().enumerate() {
        if *b == b'\n' {
            line += 1;
            ls = i + 1;
        }
    }
    (line, ls)
}

/// Byte offset where line `line` starts.
pub fn line_start(text: &str, line: usize) -> usize {
    let mut pos = 0;
    for _ in 0..line {
        match text[pos..].find('\n') {
            Some(n) => pos += n + 1,
            None => break,
        }
    }
    pos
}

/// Rewrites `[[old]]` and `[[old|alias]]` links to `new`, case-insensitively.
pub fn rewrite_links(text: &str, old: &str, new: &str) -> String {
    let old_l = old.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(a) = rest.find("[[") {
        out.push_str(&rest[..a]);
        let after = &rest[a + 2..];
        let Some(e) = after.find("]]") else {
            out.push_str(&rest[a..]);
            rest = "";
            break;
        };
        let inner = &after[..e];
        let (target, alias) = match inner.find('|') {
            Some(k) => (&inner[..k], &inner[k..]),
            None => (inner, ""),
        };
        if !inner.contains('\n') && target.to_lowercase() == old_l {
            out.push_str("[[");
            out.push_str(new);
            out.push_str(alias);
            out.push_str("]]");
        } else {
            out.push_str("[[");
            out.push_str(inner);
            out.push_str("]]");
        }
        rest = &after[e + 2..];
    }
    out.push_str(rest);
    out
}

/// The file types the preview shows as images, and that sync carries next to the notes.
pub const IMAGE_EXT: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"];

/// Whether a path names an image file, by its extension.
pub fn is_image(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, e)| IMAGE_EXT.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// An image in a note: `![alt](src)`, or `![[name]]` as other Markdown apps write an embed.
#[derive(Clone, Debug, PartialEq)]
pub struct Img {
    pub alt: String,
    pub src: String,
    /// Written as `![[name]]`: `src` is a file name to find anywhere in the folder.
    pub embed: bool,
}

/// The path or URL of `![alt](…)`: without `<…>`, without a `"title"`, with `%20` and the
/// other escapes decoded for paths.
pub fn image_src(raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t.strip_prefix('<').and_then(|r| r.strip_suffix('>')) {
        return inner.to_string();
    }
    let t = match t.find(" \"") {
        Some(i) => &t[..i],
        None => t,
    };
    if t.contains("://") {
        return t.to_string();
    }
    percent_decode(t)
}

pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hexval = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hexval(b[i + 1]) * 16 + hexval(b[i + 2]));
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// A path for a Markdown link: spaces and the characters that end a link become escapes.
pub fn link_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            _ => out.push(c),
        }
    }
    out
}

/// `to` written relative to the folder `from_dir`, both relative to the notes folder, with
/// `../` to go up.
pub fn relative(from_dir: &str, to: &str) -> String {
    let a: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let same = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; a.len() - same];
    parts.extend(&b[same..]);
    parts.join("/")
}

/// `rel` inside the folder `dir`, with `.` and `..` resolved. None when it leaves the notes
/// folder.
pub fn resolve(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = if rel.starts_with('/') { Vec::new() } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_follow_the_prototype_order() {
        let s = "See [[Ideas]] and **bold** with *em* and #tag, `code` ~~x~~ [a](b)";
        let t: Vec<Tok> = inline_tokens(s).into_iter().map(|x| x.0).collect();
        assert_eq!(t, vec![Tok::Wiki, Tok::Bold, Tok::Italic, Tok::Tag, Tok::Code, Tok::Strike, Tok::Link]);
        assert!(inline_tokens("a#b").is_empty());
        assert!(inline_tokens("snake_case_name").is_empty());
    }

    #[test]
    fn list_lines_parse_tasks_and_numbers() {
        let l = list_line("  - [x] done").unwrap();
        assert!(l.done && l.task.is_some());
        assert_eq!(list_line("12. twelve").unwrap().number, Some((12, '.')));
        assert!(list_line("-nope").is_none());
    }

    #[test]
    fn paragraphs() {
        assert_eq!(para_range("a\nb\n\nc", 0), (0, 1));
    }

    #[test]
    fn links_rewrite_with_alias() {
        assert_eq!(rewrite_links("x [[Old|o]] [[old]] [[Other]]", "Old", "New"), "x [[New|o]] [[New]] [[Other]]");
    }

    /// C17: a search snippet shows the text, not the Markdown, cut at word edges with "…" at a
    /// cut, fits its lines, and keeps the match marked.
    #[test]
    fn snippets_read_as_text() {
        let ln = "- [x] Link **notes** with [[Reading list|a list]] and [the site](https://notes.example) or `code` here";
        let col = ln.find("notes").unwrap();
        let (t, m) = snippet(ln, col, 5, 20, 80, 1);
        assert_eq!(&t[m.clone()], "notes");
        assert!(t.starts_with("Link notes with a list and the site"), "{t}");
        assert!(!t.contains("**") && !t.contains("[[") && !t.contains("](") && !t.contains('`'), "{t}");
        let long = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen";
        let (t, m) = snippet(long, long.find("eight").unwrap(), 5, 12, 14, 2);
        assert_eq!(t, "…six seven eight nine…");
        assert_eq!(&t[m], "eight");
        assert!(wrapped(&t, 14) <= 2);
        let (t, _) = snippet("# Plain notes", 8, 5, 20, 30, 2);
        assert_eq!(t, "Plain notes");
        // A match inside a link's address shows the link's text, unmarked.
        let ln = "see [here](https://notes.example) now";
        let (t, m) = snippet(ln, ln.find("notes").unwrap(), 5, 20, 30, 2);
        assert_eq!((t.as_str(), m.is_empty()), ("see here now", true));
        assert_eq!(snippet("x_y and *é* done", 0, 1, 20, 30, 2).0, "x_y and é done");
        assert_eq!(wrapped("aaa bbb ccc", 7), 2);
    }

    #[test]
    fn fuzzy_prefers_substrings() {
        let (a, idx) = fuzzy("slo", "On slow software").unwrap();
        assert_eq!(idx, vec![3, 4, 5]);
        let (b, _) = fuzzy("oss", "On slow software").unwrap();
        assert!(a > b);
        assert!(fuzzy("zz", "abc").is_none());
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;

    #[test]
    fn image_sources_decode() {
        assert_eq!(image_src("attachments/My%20cat.png"), "attachments/My cat.png");
        assert_eq!(image_src("a.png \"Title\""), "a.png");
        assert_eq!(image_src("<with space.png>"), "with space.png");
        assert_eq!(image_src("https://example.com/a%20b.png"), "https://example.com/a%20b.png");
        assert_eq!(percent_decode("%E2%9C%93 100%"), "✓ 100%");
        assert_eq!(percent_decode("%é"), "%é");
    }

    #[test]
    fn image_paths() {
        assert_eq!(relative("Drafts/Old", "attachments/a.png"), "../../attachments/a.png");
        assert_eq!(relative("", "attachments/a.png"), "attachments/a.png");
        assert_eq!(resolve("Drafts", "../attachments/a.png").as_deref(), Some("attachments/a.png"));
        assert_eq!(resolve("", "../a.png"), None);
        assert_eq!(link_path("My cat (1).png"), "My%20cat%20%281%29.png");
        assert!(is_image("x/Y.JPG") && !is_image("x.md"));
    }
}
