//! The Markdown toolbar's actions, as plain text edits. Each action looks at the text and the
//! selection, and gives back one replacement and the selection after it. The Android app has
//! the same rules.

/// A toolbar action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fmt {
    Bold,
    Italic,
    Strike,
    Code,
    Link,
    Wiki,
    Heading,
    Bullet,
    Number,
    Task,
    Quote,
}

/// Replace `text[start..end]` with `insert`, then select `sel` (byte offsets in the new text).
#[derive(Debug, PartialEq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub insert: String,
    pub sel: (usize, usize),
}

pub fn apply(text: &str, a: usize, b: usize, f: Fmt) -> Edit {
    let (s, e) = (a.min(b).min(text.len()), a.max(b).min(text.len()));
    match f {
        Fmt::Bold => wrap(text, s, e, "**"),
        Fmt::Italic => wrap(text, s, e, "*"),
        Fmt::Strike => wrap(text, s, e, "~~"),
        Fmt::Code if text[s..e].contains('\n') => fence(text, s, e),
        Fmt::Code => wrap(text, s, e, "`"),
        Fmt::Link => link(text, s, e),
        Fmt::Wiki => wiki(text, s, e),
        Fmt::Heading | Fmt::Bullet | Fmt::Number | Fmt::Task | Fmt::Quote => lines(text, s, e, f),
    }
}

/// `m` is the whole mark at `at`: a `*` that is not part of `**`.
fn mark_at(text: &str, at: usize, m: &str) -> bool {
    if at + m.len() > text.len() || &text[at..at + m.len()] != m {
        return false;
    }
    if m != "*" {
        return true;
    }
    let before = at > 0 && text.as_bytes()[at - 1] == b'*';
    let after = text.as_bytes().get(at + 1) == Some(&b'*');
    !before && !after
}

/// Puts `m` on both sides of the selection, or takes it away when it is already there.
fn wrap(text: &str, s: usize, e: usize, m: &str) -> Edit {
    let n = m.len();
    // The marks sit just outside the selection.
    if s >= n && mark_at(text, s - n, m) && mark_at(text, e, m) {
        return Edit { start: s - n, end: e + n, insert: text[s..e].to_string(), sel: (s - n, e - n) };
    }
    // The selection holds the marks.
    let inner = &text[s..e];
    if inner.len() >= 2 * n && inner.starts_with(m) && inner.ends_with(m) && (m != "*" || (!inner.starts_with("**") && !inner.ends_with("**"))) {
        return Edit { start: s, end: e, insert: inner[n..inner.len() - n].to_string(), sel: (s, e - 2 * n) };
    }
    Edit { start: s, end: e, insert: format!("{m}{inner}{m}"), sel: (s + n, e + n) }
}

fn fence(text: &str, s: usize, e: usize) -> Edit {
    let lead = if s > 0 && text.as_bytes()[s - 1] != b'\n' { "\n" } else { "" };
    let body = text[s..e].trim_end_matches('\n');
    let insert = format!("{lead}```\n{body}\n```");
    let start_body = s + lead.len() + 4;
    Edit { start: s, end: e, sel: (start_body, start_body + body.len()), insert }
}

/// Ctrl+K and the link button (WR-57, WR-58): a selected URL becomes `[](url)` with the caret
/// in the brackets; anything else becomes `[text]()` with the caret in the parentheses.
fn link(text: &str, s: usize, e: usize) -> Edit {
    let sel = &text[s..e];
    let url = ["http://", "https://", "mailto:"].iter().any(|p| sel.len() > p.len() && sel.starts_with(p)) && !sel.contains(char::is_whitespace);
    if url {
        return Edit { start: s, end: e, insert: format!("[]({sel})"), sel: (s + 1, s + 1) };
    }
    let u = s + 1 + sel.len() + 2;
    Edit { start: s, end: e, insert: format!("[{sel}]()"), sel: (u, u) }
}

fn wiki(text: &str, s: usize, e: usize) -> Edit {
    if s >= 2 && text[..s].ends_with("[[") && text[e..].starts_with("]]") {
        return Edit { start: s - 2, end: e + 2, insert: text[s..e].to_string(), sel: (s - 2, e - 2) };
    }
    Edit { start: s, end: e, insert: format!("[[{}]]", &text[s..e]), sel: (s + 2, e + 2) }
}

/// Leading indentation (spaces/tabs) and the start of a line's text after a list marker, a task
/// box, a quote mark or a heading's `#` run: `(indent, marker, end_of_marker)`.
fn prefix(line: &str) -> (&str, &str, usize) {
    let lead = line.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
    let indent = &line[..lead];
    let t = &line[lead..];
    for p in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ ", "> "] {
        if t.starts_with(p) {
            return (indent, p, lead + p.len());
        }
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return (indent, &t[..digits + 2], lead + digits + 2);
    }
    let hashes = t.bytes().take_while(|&c| c == b'#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        return (indent, &t[..hashes + 1], lead + hashes + 1);
    }
    (indent, "", lead)
}

fn kind(p: &str) -> Option<Fmt> {
    match p {
        "- [ ] " | "- [x] " | "- [X] " => Some(Fmt::Task),
        "- " | "* " | "+ " => Some(Fmt::Bullet),
        "> " => Some(Fmt::Quote),
        p if p.starts_with('#') => Some(Fmt::Heading),
        p if p.ends_with(". ") => Some(Fmt::Number),
        _ => None,
    }
}

/// Heading, lists, tasks and quotes work on whole lines: every line the selection touches.
fn lines(text: &str, s: usize, e: usize, f: Fmt) -> Edit {
    let ls = text[..s].rfind('\n').map_or(0, |i| i + 1);
    // A selection that ends at the start of a line leaves that line alone.
    let e_line = if e > s && text[..e].ends_with('\n') { e - 1 } else { e };
    let le = text[e_line..].find('\n').map_or(text.len(), |i| e_line + i);
    let block: Vec<&str> = text[ls..le].split('\n').collect();
    let many = block.len() > 1;
    let used: Vec<&&str> = block.iter().filter(|l| !many || !l.trim().is_empty()).collect();
    let all = !used.is_empty() && used.iter().all(|l| kind(prefix(l).1) == Some(f));
    let level = |l: &str| l.bytes().take_while(|&c| c == b'#').count();
    let mut n = 0;
    let out: Vec<String> = block
        .iter()
        .map(|l| {
            if many && l.trim().is_empty() {
                return l.to_string();
            }
            let (indent, p, len) = prefix(l);
            let rest = &l[len..];
            match f {
                Fmt::Heading => {
                    let next = if kind(p) == Some(Fmt::Heading) { (level(p) + 1) % 4 } else { 1 };
                    if next == 0 { rest.to_string() } else { format!("{} {rest}", "#".repeat(next)) }
                }
                _ if all => format!("{indent}{rest}"),
                Fmt::Bullet => format!("{indent}- {rest}"),
                Fmt::Task => format!("{indent}- [ ] {rest}"),
                Fmt::Quote => format!("> {l}"),
                Fmt::Number => {
                    n += 1;
                    format!("{indent}{n}. {rest}")
                }
                _ => l.to_string(),
            }
        })
        .collect();
    let insert = out.join("\n");
    let sel = if many {
        (ls, ls + insert.len())
    } else {
        // One line: the caret keeps its place in the line's text.
        let shift = insert.len() as isize - (le - ls) as isize;
        let at = |x: usize| ((x as isize + shift).max(ls as isize) as usize).min(ls + insert.len());
        (at(s), at(e))
    };
    Edit { start: ls, end: le, insert, sel }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text after the edit, with `|` at the selection's ends.
    fn run(text: &str, f: Fmt) -> String {
        let a = text.find('|').unwrap();
        let t2 = text.replacen('|', "", 1);
        let b = t2.find('|').unwrap_or(a);
        let t = t2.replacen('|', "", 1);
        let ed = apply(&t, a, b, f);
        let mut out = format!("{}{}{}", &t[..ed.start], ed.insert, &t[ed.end..]);
        let (x, y) = ed.sel;
        out.insert(y, '|');
        if x != y {
            out.insert(x, '|');
        }
        out
    }

    #[test]
    fn marks_wrap_and_unwrap() {
        assert_eq!(run("a |word| b", Fmt::Bold), "a **|word|** b");
        assert_eq!(run("a **|word|** b", Fmt::Bold), "a |word| b");
        assert_eq!(run("a |**word**| b", Fmt::Bold), "a |word| b");
        assert_eq!(run("a | b", Fmt::Bold), "a **|** b");
        assert_eq!(run("a |word| b", Fmt::Italic), "a *|word|* b");
        assert_eq!(run("a *|word|* b", Fmt::Italic), "a |word| b");
        assert_eq!(run("**|word|**", Fmt::Italic), "***|word|***", "italic inside bold adds a mark");
        assert_eq!(run("|x|", Fmt::Strike), "~~|x|~~");
        assert_eq!(run("run |ls| now", Fmt::Code), "run `|ls|` now");
        assert_eq!(run("Text\n|a\nb|", Fmt::Code), "Text\n```\n|a\nb|\n```");
    }

    #[test]
    fn links() {
        assert_eq!(run("see |docs|", Fmt::Link), "see [docs](|)");
        assert_eq!(run("|https://x.io|", Fmt::Link), "[|](https://x.io)");
        assert_eq!(run("|", Fmt::Link), "[](|)");
        assert_eq!(run("|mailto:a@b.c|", Fmt::Link), "[|](mailto:a@b.c)");
        assert_eq!(run("|https://x.io and|", Fmt::Link), "[https://x.io and](|)");
        assert_eq!(run("|Plan|", Fmt::Wiki), "[[|Plan|]]");
        assert_eq!(run("[[|Plan|]]", Fmt::Wiki), "|Plan|");
    }

    #[test]
    fn lines_toggle() {
        assert_eq!(run("Ti|tle", Fmt::Heading), "# Ti|tle");
        assert_eq!(run("# Ti|tle", Fmt::Heading), "## Ti|tle");
        assert_eq!(run("### Ti|tle", Fmt::Heading), "Ti|tle");
        assert_eq!(run("|a\nb|", Fmt::Bullet), "|- a\n- b|");
        assert_eq!(run("|- a\n- b|", Fmt::Bullet), "|a\nb|");
        assert_eq!(run("|a\n\nb|", Fmt::Number), "|1. a\n\n2. b|");
        assert_eq!(run("|- a\n- b|", Fmt::Number), "|1. a\n2. b|");
        assert_eq!(run("bu|y milk", Fmt::Task), "- [ ] bu|y milk");
        assert_eq!(run("- [ ] bu|y milk", Fmt::Task), "bu|y milk");
        assert_eq!(run("- bu|y", Fmt::Task), "- [ ] bu|y");
        assert_eq!(run("|said\nthis|", Fmt::Quote), "|> said\n> this|");
        assert_eq!(run("one\n|two\n|three", Fmt::Bullet), "one\n- |two|\nthree", "a selection that ends at a line start leaves that line");
    }

    #[test]
    fn nested_list_toggle_preserves_indentation() {
        assert_eq!(run("  - nested |bullet", Fmt::Bullet), "  nested |bullet");
        assert_eq!(run("  nested |bullet", Fmt::Bullet), "  - nested |bullet");
        assert_eq!(run("    1. deep |item", Fmt::Number), "    deep |item");
        assert_eq!(run("  - [ ] nested |task", Fmt::Task), "  nested |task");
        assert_eq!(run("  - nested |item", Fmt::Task), "  - [ ] nested |item");
        assert_eq!(run("  - nested |item", Fmt::Number), "  1. nested |item");
        assert_eq!(run("\t- tabbed |item", Fmt::Bullet), "\ttabbed |item");
    }
}
