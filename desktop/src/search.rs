//! Search of every note in the folder, as the sidebar shows it (SB-57 to SB-77): the notes
//! whose name holds the query, the notes whose text holds it and the tags that hold it, or for
//! a query that starts with "#" the tags alone and the notes of a whole tag.
//!
//! Results are worked out when the query or the notes change, not when a frame is drawn.
//! `Index` keeps each note's name and text in lower case and its tags, and lowers a note again
//! only when the vault says it was written. `Cache` keeps the last results: the same query on
//! the same notes gives them back with no scan, and a query that adds letters to the last one
//! searches only the last one's notes. `md::search` stays for the MCP server.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::md;
use crate::vault::Vault;

/// A note's text in lower case, for matching.
enum Lowered {
    /// Lowering kept every character's length, so byte offsets are the note's own.
    Same(String),
    /// Some character changed length: each line lowered alone, with where it starts in the
    /// note, its length there and whether its offsets are its own. A line that changed length
    /// gives one hit at its start, as `md::search` does.
    Lines(Vec<(usize, usize, String)>),
    /// A locked note: its text is sealed, so there is nothing to find in it (SB-59).
    Sealed,
}

struct Entry {
    /// The vault's `ver` of the note when it was lowered.
    ver: u64,
    /// The note's name without ".md", in lower case.
    name: String,
    text: Lowered,
    tags: Vec<String>,
}

impl Entry {
    fn new(path: &str, text: &str, ver: u64) -> Entry {
        let sealed = crate::lock::is_locked(text);
        Entry {
            ver,
            name: md::base(path).to_lowercase(),
            text: if sealed { Lowered::Sealed } else { lower(text) },
            tags: if sealed { Vec::new() } else { md::note_tags(text) },
        }
    }

    /// How many times the lowered query `q` is in the text, and where the first one is.
    fn hits(&self, q: &str) -> Option<(usize, md::Hit)> {
        match &self.text {
            Lowered::Same(t) => {
                let mut found = t.match_indices(q);
                let (pos, _) = found.next()?;
                let line = t[..pos].bytes().filter(|&b| b == b'\n').count();
                let col = pos - t[..pos].rfind('\n').map_or(0, |i| i + 1);
                Some((1 + found.count(), md::Hit { line, col, pos }))
            }
            Lowered::Lines(lines) => {
                let mut count = 0;
                let mut first = None;
                for (i, (start, len, low)) in lines.iter().enumerate() {
                    let n = if low.len() == *len { low.matches(q).count() } else { usize::from(low.contains(q)) };
                    if n > 0 && first.is_none() {
                        let col = if low.len() == *len { low.find(q).unwrap_or(0) } else { 0 };
                        first = Some(md::Hit { line: i, col, pos: start + col });
                    }
                    count += n;
                }
                first.map(|f| (count, f))
            }
            Lowered::Sealed => None,
        }
    }
}

/// `text` in lower case. ASCII lowers in place; other text keeps its offsets only when no
/// character changes length.
fn lower(text: &str) -> Lowered {
    if text.is_ascii() {
        return Lowered::Same(text.to_ascii_lowercase());
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let at = out.len();
        out.extend(c.to_lowercase());
        if out.len() - at != c.len_utf8() {
            let mut start = 0;
            let lines = text
                .split('\n')
                .map(|ln| {
                    let l = (start, ln.len(), ln.to_lowercase());
                    start += ln.len() + 1;
                    l
                })
                .collect();
            return Lowered::Lines(lines);
        }
    }
    Lowered::Same(out)
}

/// Every note of one vault, lowered, and its tags with their note counts.
#[derive(Default)]
struct Index {
    /// The vault it was read from (`Vault::id`) and that vault's `writes` then.
    read: (u64, u64),
    notes: BTreeMap<String, Entry>,
    /// Every tag with how many notes carry it, most first, then by name.
    tags: Vec<(String, usize)>,
    /// How many notes it has lowered, for the tests.
    lowered: usize,
    /// How many notes it has searched the text of, for the tests.
    scanned: usize,
}

/// What a search may take from the last results (`Index::search`).
enum From<'a> {
    All,
    /// The last results, for a query that only added letters to theirs.
    Within(&'a Results),
    /// The last results for the same query, and the vault's `writes` when they were found.
    Since(&'a Results, u64),
}

impl Index {
    /// Catches up with `vault`: a note gone is dropped, and a note new or written since is
    /// lowered again. Whether anything changed.
    fn sync(&mut self, vault: &Vault) -> bool {
        let now = (vault.id(), vault.writes());
        if self.read == now {
            return false;
        }
        if self.read.0 != vault.id() {
            self.notes.clear();
        }
        self.read = now;
        let files = vault.files();
        self.notes.retain(|p, _| files.contains_key(p));
        for (p, t) in files {
            let ver = vault.ver(p);
            if self.notes.get(p).is_none_or(|e| e.ver != ver) {
                self.notes.insert(p.clone(), Entry::new(p, t, ver));
                self.lowered += 1;
            }
        }
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for e in self.notes.values() {
            for t in &e.tags {
                *counts.entry(t).or_default() += 1;
            }
        }
        let mut tags: Vec<(String, usize)> = counts.into_iter().map(|(t, n)| (t.to_string(), n)).collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        self.tags = tags;
        true
    }

    /// The results for the trimmed query `q`. `from` says what the last results can spare:
    /// for a query that only added letters to theirs, only their notes are looked at; for the
    /// same query after the notes changed, a note unwritten since keeps its last result and
    /// only the rest are searched.
    fn search(&mut self, q: &str, from: From) -> Results {
        let mut out = Results { query: q.to_string(), ..Results::default() };
        if let Some(rest) = q.strip_prefix('#') {
            // Tags only (SB-76): those that hold the rest, and the notes of a whole tag.
            let rest = rest.trim().to_lowercase();
            out.tags = self.tags.iter().filter(|(t, _)| t.contains(&rest)).cloned().collect();
            if !rest.is_empty() && self.tags.iter().any(|(t, _)| *t == rest) {
                out.tagged = self.notes.iter().filter(|(_, e)| e.tags.contains(&rest)).map(|(p, _)| p.clone()).collect();
                out.tagged.sort_by(|a, b| tree_cmp(a, b));
            }
            return out;
        }
        let ql = q.to_lowercase();
        // A note's name holding the query makes it a title; else its text is searched.
        let mut scanned = 0;
        let mut check = |p: &String, e: &Entry, out: &mut Results| {
            if e.name.contains(&ql) {
                out.titles.push(p.clone());
                return;
            }
            scanned += 1;
            if let Some((count, first)) = e.hits(&ql) {
                out.text.push(TextHit { path: p.clone(), count, first });
            }
        };
        match from {
            From::Within(last) => {
                for p in last.titles.iter().chain(last.text.iter().map(|h| &h.path)) {
                    if let Some(e) = self.notes.get(p) {
                        check(p, e, &mut out);
                    }
                }
            }
            From::Since(last, then) => {
                let titles: std::collections::HashSet<&str> = last.titles.iter().map(String::as_str).collect();
                let text: std::collections::HashMap<&str, &TextHit> = last.text.iter().map(|h| (h.path.as_str(), h)).collect();
                for (p, e) in &self.notes {
                    if e.ver > then {
                        check(p, e, &mut out);
                    } else if titles.contains(p.as_str()) {
                        out.titles.push(p.clone());
                    } else if let Some(h) = text.get(p.as_str()) {
                        out.text.push((*h).clone());
                    }
                }
            }
            From::All => {
                for (p, e) in &self.notes {
                    check(p, e, &mut out);
                }
            }
        }
        self.scanned += scanned;
        out.titles.sort_by(|a, b| tree_cmp(a, b));
        out.text.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
        out.tags = self.tags.iter().filter(|(t, _)| t.contains(&ql)).cloned().collect();
        out
    }
}

/// A note whose text holds the query: how many times, and the first.
#[derive(Clone, Debug, PartialEq)]
pub struct TextHit {
    pub path: String,
    pub count: usize,
    pub first: md::Hit,
}

/// What a query finds, in the order the list shows it (SB-58 to SB-62, SB-76).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Results {
    /// The query, trimmed.
    pub query: String,
    /// The notes whose name holds the query, in the tree's order.
    pub titles: Vec<String>,
    /// The notes whose text holds it, most matches first, then by path. A note in `titles`
    /// is not here (SB-60).
    pub text: Vec<TextHit>,
    /// The tags that hold it, with their note counts, most first.
    pub tags: Vec<(String, usize)>,
    /// For "#" and a whole tag, the notes that carry the tag, in the tree's order.
    pub tagged: Vec<String>,
}

/// A row of the results.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row<'a> {
    Title(&'a str),
    Text(&'a TextHit),
    Tag(&'a str, usize),
    Tagged(&'a str),
}

impl Row<'_> {
    /// The row's element key, with `|` for `/` as the tree's rows have it.
    pub fn key(&self) -> String {
        let row_key = crate::folders::row_key;
        match self {
            Row::Title(p) => row_key("res-title", p),
            Row::Text(h) => row_key("res-text", &h.path),
            Row::Tag(t, _) => row_key("res-tag", t),
            Row::Tagged(p) => row_key("res-tagged", p),
        }
    }

    /// The note the row shows, if it shows one.
    pub fn note(&self) -> Option<&str> {
        match self {
            Row::Title(p) | Row::Tagged(p) => Some(p),
            Row::Text(h) => Some(&h.path),
            Row::Tag(..) => None,
        }
    }
}

impl Results {
    /// Every row, in the list's order.
    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        let titles = self.titles.iter().map(|p| Row::Title(p));
        let text = self.text.iter().map(Row::Text);
        let tags = self.tags.iter().map(|(t, n)| Row::Tag(t, *n));
        let tagged = self.tagged.iter().map(|p| Row::Tagged(p));
        titles.chain(text).chain(tags).chain(tagged)
    }

    /// The first row: the one Enter in the field opens (SB-65).
    pub fn first(&self) -> Option<Row<'_>> {
        self.rows().next()
    }
}

/// The tree's order (SB-54): in each folder its subfolders first, then its notes, each by name
/// ignoring case.
pub fn tree_cmp(a: &str, b: &str) -> Ordering {
    let natural = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b));
    let (mut pa, mut pb) = (a.split('/').peekable(), b.split('/').peekable());
    loop {
        match (pa.next(), pb.next()) {
            (Some(x), Some(y)) => {
                let (note_a, note_b) = (pa.peek().is_none(), pb.peek().is_none());
                if note_a != note_b {
                    return if note_a { Ordering::Greater } else { Ordering::Less };
                }
                let by = if note_a { natural(md::strip_md(x), md::strip_md(y)) } else { natural(x, y) };
                if by != Ordering::Equal {
                    return by;
                }
            }
            (x, y) => return x.is_some().cmp(&y.is_some()),
        }
    }
}

/// Search's results, kept for the query and the notes they were found in. Read through a
/// `RefCell` in the view, which sees the app only by reference.
#[derive(Default)]
pub struct Cache {
    index: Index,
    /// The last results, with the vault and its `writes` they were found in.
    last: Option<((u64, u64), Rc<Results>)>,
}

impl Cache {
    /// The results for `q` in `vault`. The same query on unchanged notes gives the last
    /// results back with no scan.
    pub fn results(&mut self, vault: &Vault, q: &str) -> Rc<Results> {
        let q = q.trim();
        let now = (vault.id(), vault.writes());
        if let Some((at, r)) = &self.last
            && *at == now
            && r.query == q
        {
            return r.clone();
        }
        self.index.sync(vault);
        // Letters added to the last query, on the same notes, can only find fewer of them; the
        // same query on notes written since needs only those notes searched again.
        let last = self.last.as_ref().filter(|(at, r)| at.0 == now.0 && !r.query.starts_with('#'));
        let ql = q.to_lowercase();
        let from = match last {
            Some((at, r)) if *at == now && ql.starts_with(&r.query.to_lowercase()) => From::Within(r),
            Some((at, r)) if ql == r.query.to_lowercase() => From::Since(r, at.1),
            _ => From::All,
        };
        let r = Rc::new(self.index.search(q, from));
        self.last = Some((now, r.clone()));
        r
    }

    /// How many notes the index has lowered since it began.
    #[cfg(test)]
    pub(crate) fn lowered(&self) -> usize {
        self.index.lowered
    }

    /// How many notes' text it has searched since it began.
    #[cfg(test)]
    fn scanned(&self) -> usize {
        self.index.scanned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(notes: &[(&str, &str)]) -> Vault {
        Vault::for_tests(notes.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect())
    }

    fn titles(r: &Results) -> Vec<&str> {
        r.titles.iter().map(String::as_str).collect()
    }

    fn text(r: &Results) -> Vec<(&str, usize)> {
        r.text.iter().map(|h| (h.path.as_str(), h.count)).collect()
    }

    /// SB-59: a title is a note whose name holds the query, ignoring case, in the tree's order:
    /// a folder's subfolders before its notes.
    #[test]
    fn titles_match_the_name() {
        let v = vault(&[("Plan.md", ""), ("b/Planets.md", ""), ("a/z/plank.md", ""), ("a/aplan.md", ""), ("Other.md", "")]);
        let r = Cache::default().results(&v, " PLAN ");
        assert_eq!(r.query, "PLAN", "the query is trimmed");
        assert_eq!(titles(&r), ["a/z/plank.md", "a/aplan.md", "b/Planets.md", "Plan.md"]);
    }

    /// SB-59 and SB-60: text results come most matches first, then by path, and a note listed
    /// under Titles is not listed again. The first hit is where the first match is.
    #[test]
    fn a_title_is_not_repeated_as_text() {
        let v = vault(&[("ideas.md", "one idea"), ("b.md", "an idea\nanother IDEA, idea"), ("a.md", "x idea"), ("c.md", "none")]);
        let r = Cache::default().results(&v, "idea");
        assert_eq!(titles(&r), ["ideas.md"]);
        assert_eq!(text(&r), [("b.md", 3), ("a.md", 1)]);
        assert_eq!((r.text[0].first.line, r.text[0].first.col, r.text[0].first.pos), (0, 3, 3));
        let rows: Vec<String> = r.rows().map(|row| row.key()).collect();
        assert_eq!(rows, ["res-title:ideas.md", "res-text:b.md", "res-text:a.md"]);
    }

    /// SB-76: "#" alone lists every tag; "#" and letters the tags that hold them, and no
    /// titles or text. A plain query lists the tags that hold it after the notes (SB-58).
    #[test]
    fn hash_searches_tags() {
        let v = vault(&[("a.md", "#plan #work"), ("b.md", "#plan and #plant"), ("plan.md", "no tags")]);
        let mut c = Cache::default();
        let all = c.results(&v, "#");
        assert_eq!(all.tags, [("plan".to_string(), 2), ("plant".to_string(), 1), ("work".to_string(), 1)]);
        assert!(all.titles.is_empty() && all.text.is_empty() && all.tagged.is_empty());
        let some = c.results(&v, "#pla");
        assert_eq!(some.tags, [("plan".to_string(), 2), ("plant".to_string(), 1)]);
        assert!(some.titles.is_empty() && some.text.is_empty() && some.tagged.is_empty(), "no notes until the tag is whole");
        let plain = c.results(&v, "plan");
        assert_eq!(titles(&plain), ["plan.md"]);
        assert_eq!(text(&plain), [("b.md", 2), ("a.md", 1)]);
        assert_eq!(plain.tags, [("plan".to_string(), 2), ("plant".to_string(), 1)]);
    }

    /// SB-76: a whole tag after "#" lists the notes that carry it, in the tree's order, under
    /// its tag.
    #[test]
    fn a_whole_tag_lists_its_notes() {
        let v = vault(&[("z.md", "#plan"), ("Box/a.md", "#Plan"), ("b.md", "#plant")]);
        let r = Cache::default().results(&v, "#plan");
        assert_eq!(r.tags, [("plan".to_string(), 2), ("plant".to_string(), 1)]);
        assert_eq!(r.tagged, ["Box/a.md", "z.md"]);
        let rows: Vec<String> = r.rows().map(|row| row.key()).collect();
        assert_eq!(rows, ["res-tag:plan", "res-tag:plant", "res-tagged:Box|a.md", "res-tagged:z.md"]);
    }

    /// SB-59 and SB-133: a locked note is found by its name and never by its text or tags.
    #[test]
    fn sealed_notes_match_by_name() {
        let sealed = format!("{} v1 pbkdf2-sha256 1 AAAA\n\nsecret words #hidden", crate::lock::MAGIC);
        let v = vault(&[("secret.md", &sealed), ("open.md", "secret words")]);
        let mut c = Cache::default();
        let r = c.results(&v, "secret");
        assert_eq!(titles(&r), ["secret.md"], "the locked note is found by its name");
        assert_eq!(text(&r), [("open.md", 1)], "and only the open note by its text");
        let r = c.results(&v, "words");
        assert_eq!(text(&r), [("open.md", 1)], "the sealed text is not searched");
        assert!(c.results(&v, "#hidden").tags.is_empty(), "nor are its tags");
    }

    /// SB-71: results follow the notes. A note written is lowered again, alone, and an
    /// unchanged query on unchanged notes is the same results with nothing lowered, however
    /// many frames ask.
    #[test]
    fn the_same_query_after_an_edit_searches_that_note_alone() {
        let mut v = vault(&[("a.md", "one apple"), ("b.md", "two apples, apple"), ("c.md", "no fruit"), ("apple pie.md", "x")]);
        let mut c = Cache::default();
        let r = c.results(&v, "apple");
        assert_eq!((titles(&r), text(&r)), (vec!["apple pie.md"], vec![("b.md", 2), ("a.md", 1)]));
        let before = c.scanned();
        v.put("c.md".into(), "apple apple apple".into());
        let r = c.results(&v, "apple");
        assert_eq!(c.scanned(), before + 1, "only the note written since is searched");
        assert_eq!((titles(&r), text(&r)), (vec!["apple pie.md"], vec![("c.md", 3), ("b.md", 2), ("a.md", 1)]), "and the results are what a full search finds");
        v.put("b.md".into(), "pears".into());
        let r = c.results(&v, "apple");
        assert_eq!(text(&r), [("c.md", 3), ("a.md", 1)], "a note that lost the query leaves");
        let mut fresh = Cache::default();
        assert_eq!(*fresh.results(&v, "apple"), *r, "as a fresh search agrees");
    }

    #[test]
    fn an_edit_relowers_one_note() {
        let mut v = vault(&[("a.md", "alpha"), ("b.md", "beta"), ("c.md", "gamma")]);
        let mut c = Cache::default();
        let first = c.results(&v, "a");
        assert_eq!(c.lowered(), 3);
        for _ in 0..100 {
            assert!(Rc::ptr_eq(&first, &c.results(&v, "a")), "a frame with nothing changed gets the same results");
        }
        assert_eq!(c.lowered(), 3, "and lowers nothing");
        v.put("b.md".into(), "betta".into());
        for _ in 0..100 {
            c.results(&v, "tt");
        }
        assert_eq!(c.lowered(), 4, "an edit lowers its note once");
        assert_eq!(text(&c.results(&v, "tt")), [("b.md", 1)]);
        v.take("c.md");
        assert_eq!(text(&c.results(&v, "a")), [("b.md", 1)], "a note taken out is gone from the results");
        assert_eq!(c.lowered(), 4);
        v.put("d.md".into(), "delta".into());
        assert_eq!(text(&c.results(&v, "elt")), [("d.md", 1)], "a new note is found");
        assert_eq!(c.lowered(), 5);
        let other = vault(&[("a.md", "other")]);
        assert_eq!(text(&c.results(&other, "oth")), [("a.md", 1)], "another vault is read afresh");
    }

    /// A query that adds letters to the last one searches only its notes, and finds what a
    /// fresh search finds: a note that was a title for the shorter query can be text now.
    #[test]
    fn a_longer_query_refines() {
        let v = vault(&[("pat.md", "a pattern"), ("b.md", "patter"), ("c.md", "nothing"), ("d.md", "PAT")]);
        let mut c = Cache::default();
        c.results(&v, "pat");
        let refined = c.results(&v, "patt");
        assert_eq!(*refined, Cache::default().results(&v, "patt").as_ref().clone());
        assert_eq!(text(&refined), [("b.md", 1), ("pat.md", 1)]);
        let wider = c.results(&v, "pa");
        assert_eq!(*wider, Cache::default().results(&v, "pa").as_ref().clone(), "a shorter query searches every note");
    }

    /// Text whose lower case changes a character's length keeps offsets per line: a match
    /// on a line that changed has its hit at the line's start.
    #[test]
    fn lowering_that_changes_lengths_keeps_offsets() {
        let v = vault(&[("a.md", "Ünïcode word\nİstanbul word")]);
        let r = Cache::default().results(&v, "word");
        assert_eq!(text(&r), [("a.md", 2)]);
        assert_eq!((r.text[0].first.line, r.text[0].first.col), (0, 10));
        let r = Cache::default().results(&v, "stanbul");
        assert_eq!((r.text[0].first.line, r.text[0].first.col, r.text[0].first.pos), (1, 0, 15));
    }
}
