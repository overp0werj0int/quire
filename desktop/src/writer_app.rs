//! The writer on `App` (`docs/reader-writer/behavior/writer.md`): the source's formatting
//! spans, cached per line; the editing keys; paragraph focus, typewriter scrolling and the
//! font cycle; and find in the source.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;
use std::rc::Rc;

use rune::{Color, ParagraphStyle, Span, TextEdit, Tx};

use crate::app::{App, Mode, TextKey};
use crate::layer::{Menu, MenuOf};
use crate::format::Edit;
use crate::md;
use crate::writer::{self, Count, Ink, State, Styled};

/// A transparent highlight over the caret's character: typewriter scrolling centres it.
pub const CARET: &str = "caret-at";
/// A transparent highlight over a line placed at the top of the view (WR-102, WR-112).
pub const LINE_AT: &str = "line-at";
/// The source fonts Ctrl+Shift+M steps through (WR-125).
const FONT_CYCLE: [&str; 3] = ["quattro", "duospace", "ia-mono"];

/// Fast non-cryptographic hasher for internal per-line span cache lookups (`LIVEPACE`).
#[derive(Default)]
struct FxHasher(u64);

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0x517c_c1b7_2722_0a95;
        let mut h = self.0;
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            h = (h.rotate_left(5) ^ u64::from_le_bytes(c.try_into().unwrap())).wrapping_mul(K);
        }
        for &b in chunks.remainder() {
            h = (h.rotate_left(5) ^ u64::from(b)).wrapping_mul(K);
        }
        self.0 = h;
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
type Key = (u64, usize, State, bool, bool);

#[derive(Default)]
struct Cache {
    /// What the cached lines were styled with: the size and colours, and the note index.
    with: Option<(u32, [u32; 40], u64)>,
    lines: FxMap<Key, Rc<(Styled, State)>>,
}

/// Whole-note assembled spans, strikes, hangs, paragraph styles, and per-line indices reused
/// across unchanged frames and patched incrementally on single-line edits (`LIVEPACE`, `COLDPACE`).
struct Assembled {
    text_key: TextKey,
    e: f32,
    ink: Ink,
    with: (u32, [u32; 40], u64),
    focus: Option<(usize, usize)>,
    line_starts: Vec<usize>,
    span_starts: Vec<usize>,
    strike_starts: Vec<usize>,
    line_states: Vec<State>,
    has_comment: bool,
    spans: Vec<Span>,
    strikes: Vec<(usize, usize, Option<Color>)>,
    hangs: Vec<(usize, usize)>,
    para_gutter: u32,
    para_styles: Vec<ParagraphStyle>,
}

struct CachedHeadings {
    text_key: TextKey,
    ver: u64,
    heads: Rc<Vec<md::Heading>>,
    h1: Rc<Vec<md::Heading>>,
}

struct CachedBacklinks {
    vault_id: u64,
    active: String,
    other_writes: u64,
    ver: u64,
    links: Rc<Vec<(String, usize)>>,
}

#[derive(Default)]
pub struct WriterState {
    /// The caret the last frame saw, so typewriter scrolling follows moves (WR-88).
    caret: Option<(String, usize)>,
    /// Centre the caret line on the next frame (a forced move).
    pub center: bool,
    /// A source line to hold at the top of the view, and since when.
    pub top_line: Option<(usize, f64)>,
    /// Leaving source mode: the editor's scroll offset, and the preview block placed at the top.
    back: Option<(f32, usize)>,
    cache: RefCell<Cache>,
    assembled: RefCell<Option<Assembled>>,
    count: RefCell<Option<(TextKey, Count)>>,
    lines_count: RefCell<Option<(TextKey, usize)>>,
    headings: RefCell<Option<CachedHeadings>>,
    backlinks: RefCell<Option<CachedBacklinks>>,
    /// Highlighted row in the Source inline autocomplete popover (`[[` / `#`).
    pub ac_idx: usize,
    /// `(note_path, trigger_start)` dismissed with Esc until the caret leaves that trigger.
    pub ac_dismissed: Option<(String, usize)>,
}

/// One suggestion in the Source inline `[[wiki]]` / `#tag` autocomplete popover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcItem {
    pub label: String,
    pub sub: String,
    pub insert: String,
}

#[inline]
fn hash_line(ln: &str) -> u64 {
    let mut h = FxHasher::default();
    h.write(ln.as_bytes());
    h.finish()
}

fn ink_bits(i: &Ink) -> [u32; 40] {
    let mut out = [0u32; 40];
    let cs = [i.text, i.strong, i.dim, i.faint, i.ghost, i.mark, i.link, i.code, i.code_bg, i.wash];
    for (k, c) in cs.iter().enumerate() {
        for (j, v) in [c.r, c.g, c.b, c.a].iter().enumerate() {
            out[k * 4 + j] = v.to_bits();
        }
    }
    out
}

/// Literal, case-insensitive, non-overlapping matches of `q` in `text`, at most 10000 (WR-116).
pub fn find_in(text: &str, q: &str) -> Vec<Range<usize>> {
    if q.is_empty() {
        return Vec::new();
    }
    let lower = text.to_lowercase();
    let ql = q.to_lowercase();
    // Lowercasing keeps byte offsets for nearly all text; where it would not, match ASCII case
    // only, on the text itself.
    let (hay, needle) = if lower.len() == text.len() { (lower.as_str(), ql.as_str()) } else { (text, q) };
    let ascii = lower.len() != text.len();
    let mut out = Vec::new();
    let mut at = 0;
    while at + needle.len() <= hay.len() && out.len() < 10_000 {
        let hit = if ascii { hay.get(at..at + needle.len()).is_some_and(|w| w.eq_ignore_ascii_case(needle)) } else { hay[at..].starts_with(needle) };
        if hit && text.is_char_boundary(at) && text.is_char_boundary(at + needle.len()) {
            out.push(at..at + needle.len());
            at += needle.len();
        } else {
            at += hay[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Spans, strike lines and each line's hanging mark (`Styled::hang`) for a whole text.
pub type Spans = (Vec<Span>, Vec<(usize, usize, Option<Color>)>, Vec<(usize, usize)>);

/// Full indexed assembly: spans, strikes, hangs, plus per-line byte/span/strike starts and end
/// states for O(1)-line incremental patching on typing (`COLDPACE`).
struct IndexedSpans {
    spans: Vec<Span>,
    strikes: Vec<(usize, usize, Option<Color>)>,
    hangs: Vec<(usize, usize)>,
    line_starts: Vec<usize>,
    span_starts: Vec<usize>,
    strike_starts: Vec<usize>,
    line_states: Vec<State>,
    has_comment: bool,
}

fn spans_by_indexed(text: &str, cap: (usize, usize, usize), mut style: impl FnMut(usize, &str, State) -> Rc<(Styled, State)>) -> IndexedSpans {
    let mut spans = Vec::with_capacity(cap.0);
    let mut strikes = Vec::with_capacity(cap.1);
    let mut hangs = Vec::with_capacity(cap.2);
    let mut line_starts = Vec::with_capacity(cap.2);
    let mut span_starts = Vec::with_capacity(cap.2 + 1);
    let mut strike_starts = Vec::with_capacity(cap.2 + 1);
    let mut line_states = Vec::with_capacity(cap.2);
    let mut has_comment = false;
    let mut st = State::default();
    let mut at = 0;
    for (i, ln) in text.split('\n').enumerate() {
        line_starts.push(at);
        span_starts.push(spans.len());
        strike_starts.push(strikes.len());
        let styled = style(i, ln, st);
        st = styled.1;
        has_comment |= st.block == writer::Block::Comment;
        line_states.push(st);
        for s in &styled.0.spans {
            spans.push(Span { start: s.start + at as u32, end: s.end + at as u32, ..s.clone() });
        }
        strikes.extend(styled.0.strikes.iter().map(|&(a, b, c)| (a + at, b + at, Some(c))));
        hangs.push(styled.0.hang);
        at += ln.len() + 1;
    }
    span_starts.push(spans.len());
    strike_starts.push(strikes.len());
    IndexedSpans { spans, strikes, hangs, line_starts, span_starts, strike_starts, line_states, has_comment }
}

/// Byte length of the longest common prefix of `a` and `b`.
fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    let n = a.len().min(b.len());
    let mut i = 0;
    while i + 8 <= n && a[i..i + 8] == b[i..i + 8] {
        i += 8;
    }
    while i < n && a[i] == b[i] {
        i += 1;
    }
    i
}

/// Byte length of the longest common suffix of `a[pref..]` and `b[pref..]`.
fn common_suffix_len(a: &[u8], b: &[u8], pref: usize) -> usize {
    let max = a.len().min(b.len()) - pref;
    let mut i = 0;
    while i + 8 <= max && a[a.len() - i - 8..a.len() - i] == b[b.len() - i - 8..b.len() - i] {
        i += 8;
    }
    while i < max && a[a.len() - i - 1] == b[b.len() - i - 1] {
        i += 1;
    }
    i
}

fn line_affects_headings(ln: &str) -> bool {
    let t = ln.trim_start();
    t.starts_with('#') || t.starts_with("```")
}

/// Styles `text` line by line, carrying the block state from each line to the next, and
/// places each line's spans at its offset.
fn spans_by(text: &str, cap: (usize, usize, usize), style: impl FnMut(usize, &str, State) -> Rc<(Styled, State)>) -> Spans {
    let idx = spans_by_indexed(text, cap, style);
    (idx.spans, idx.strikes, idx.hangs)
}

impl App {
    /// Whether the edit menu's field has text selected, for Cut and Copy.
    pub fn edit_menu_selected(&self) -> bool {
        let Some(Menu { of: MenuOf::Edit(field), .. }) = self.menu() else { return false };
        let edit = if *field == crate::sheet::EDITOR { self.sheet().map(|s| &s.edit) } else { self.active.as_ref().and_then(|a| self.edits.get(a)) };
        edit.is_some_and(|e| e.anchor != e.focus)
    }
}

impl App {
    /// Ensures `wstate.assembled` holds the spans, strikes, hangs, and paragraph styles for the
    /// active note under `(e, ink, focus, gutter)`, reusing the previous assembly on unchanged
    /// frames (`LIVEPACE`).
    fn ensure_assembled(&self, e: f32, ink: &Ink, focus: Option<(usize, usize)>, gutter: f32) {
        let text_key = self.active_text_key();
        let with = (e.to_bits(), ink_bits(ink), self.names_gen as u64);
        let gutter_bits = gutter.to_bits();
        let mut assembled = self.wstate.assembled.borrow_mut();
        if let Some(a) = assembled.as_mut()
            && a.text_key.matches(text_key)
            && a.with == with
            && a.focus == focus
        {
            if a.para_gutter != gutter_bits {
                a.para_styles = crate::ui::editor::paragraph_styles(&a.hangs, gutter);
                a.para_gutter = gutter_bits;
            }
            return;
        }
        let cap = assembled.as_ref().map_or((0, 0, 0), |a| (a.spans.len(), a.strikes.len(), a.hangs.len()));
        let text = self.active_text();
        let mut cache = self.wstate.cache.borrow_mut();
        if cache.with != Some(with) || cache.lines.len() > 50_000 {
            cache.lines.clear();
            cache.with = Some(with);
        }
        let names = &self.names;
        let resolves = |t: &str| names.resolve(t).is_some();
        let idx = spans_by_indexed(text, cap, |i, ln, st| {
            let faded = focus.is_some_and(|(a, b)| i < a || i > b);
            let key = (hash_line(ln), ln.len(), st, i == 0, faded);
            if let Some(hit) = cache.lines.get(&key) {
                return hit.clone();
            }
            let (s, next) = writer::line(ln, i == 0, st, e, ink, &resolves);
            let s = if faded { writer::fade(&s, ln.len(), ink.ghost) } else { s };
            let v = Rc::new((s, next));
            cache.lines.insert(key, v.clone());
            v
        });
        let para_styles = crate::ui::editor::paragraph_styles(&idx.hangs, gutter);
        *assembled = Some(Assembled {
            text_key: text_key.to_owned(),
            e,
            ink: *ink,
            with,
            focus,
            line_starts: idx.line_starts,
            span_starts: idx.span_starts,
            strike_starts: idx.strike_starts,
            line_states: idx.line_states,
            has_comment: idx.has_comment,
            spans: idx.spans,
            strikes: idx.strikes,
            hangs: idx.hangs,
            para_gutter: gutter_bits,
            para_styles,
        });
    }

    /// Incrementally updates `lines_count`, `headings`, and `assembled` on single-line edits
    /// (typing/backspace/delete without newlines) instead of rescanning all lines (`COLDPACE`).
    /// Falls back automatically to full rebuild if newlines change or a block state flips.
    pub(crate) fn on_single_line_edit(&self, old_key: TextKey, new_key: TextKey, old_text: &str, new_text: &str) {
        let (ob, nb) = (old_text.as_bytes(), new_text.as_bytes());
        let pref = common_prefix_len(ob, nb);
        let suff = common_suffix_len(ob, nb, pref);
        let old_hi = ob.len() - suff;
        let new_hi = nb.len() - suff;
        if ob[pref..old_hi].contains(&b'\n') || nb[pref..new_hi].contains(&b'\n') {
            return;
        }
        // Single-line edit: line count is unchanged.
        if let Some((k, _)) = self.wstate.lines_count.borrow_mut().as_mut()
            && *k == old_key
        {
            *k = new_key.clone();
        }
        // Check whether the single edited line can affect ATX headings or fenced blocks.
        let line_start = ob[..pref].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        let old_line_end = ob[old_hi..].iter().position(|&b| b == b'\n').map_or(ob.len(), |i| old_hi + i);
        let new_line_end = nb[new_hi..].iter().position(|&b| b == b'\n').map_or(nb.len(), |i| new_hi + i);
        let old_ln = &old_text[line_start..old_line_end];
        let new_ln = &new_text[line_start..new_line_end];
        if !line_affects_headings(old_ln) && !line_affects_headings(new_ln)
            && let Some(c) = self.wstate.headings.borrow_mut().as_mut()
            && c.text_key == old_key
        {
            c.text_key = new_key.clone();
        }
        // Incrementally patch `wstate.assembled` for this one line if block state holds.
        let mut assembled = self.wstate.assembled.borrow_mut();
        let Some(a) = assembled.as_mut() else { return };
        if a.text_key != old_key || a.with.2 != self.names_gen as u64 {
            return;
        }
        let line_idx = a.line_starts.partition_point(|&s| s <= pref).saturating_sub(1);
        if line_idx >= a.line_starts.len() || a.line_starts[line_idx] != line_start {
            return;
        }
        let st_before = if line_idx == 0 { State::default() } else { a.line_states[line_idx - 1] };
        let resolves = |t: &str| self.names.resolve(t).is_some();
        let (mut styled, next_st) = writer::line(new_ln, line_idx == 0, st_before, a.e, &a.ink, &resolves);
        if next_st != a.line_states[line_idx] {
            // Fence, front-matter, comment or setext state flipped: let `ensure_assembled` rebuild.
            return;
        }
        // Incrementally patch `wstate.count` in O(line_len) when block state is unchanged and
        // outside front matter or multi-line HTML comments (so typing with Outline open never
        // rescans the full note).
        if !a.has_comment
            && let Some((k, count)) = self.wstate.count.borrow_mut().as_mut()
            && *k == old_key
        {
            let old_front = writer::strip_front_offset(old_text);
            let new_front = writer::strip_front_offset(new_text);
            if old_front == new_front && line_start >= old_front {
                let words_delta = match (st_before.block, next_st.block) {
                    (writer::Block::None, writer::Block::None) => {
                        Some(writer::count_words_line(new_ln) as isize - writer::count_words_line(old_ln) as isize)
                    }
                    (writer::Block::Fence(ca, la), writer::Block::Fence(cb, lb)) if (ca, la) == (cb, lb) => Some(0),
                    _ => None,
                };
                if let Some(dw) = words_delta {
                    let old_chars = old_ln.chars().filter(|c| !c.is_whitespace()).count() as isize;
                    let new_chars = new_ln.chars().filter(|c| !c.is_whitespace()).count() as isize;
                    count.words = (count.words as isize + dw).max(0) as usize;
                    count.chars = (count.chars as isize + (new_chars - old_chars)).max(0) as usize;
                    count.minutes = writer::count_minutes(count.words);
                    *k = new_key.clone();
                }
            }
        }
        if a.focus.is_some_and(|(fa, fb)| line_idx < fa || line_idx > fb) {
            styled = writer::fade(&styled, new_ln.len(), a.ink.ghost);
        }
        let delta_bytes = nb.len() as isize - ob.len() as isize;
        let sp_lo = a.span_starts[line_idx];
        let sp_hi = a.span_starts[line_idx + 1];
        let new_spans: Vec<Span> = styled.spans.into_iter().map(|s| Span { start: s.start + line_start as u32, end: s.end + line_start as u32, ..s }).collect();
        let delta_sp = new_spans.len() as isize - (sp_hi - sp_lo) as isize;
        let tail_sp_start = sp_lo + new_spans.len();
        a.spans.splice(sp_lo..sp_hi, new_spans);
        if delta_bytes != 0 {
            for s in &mut a.spans[tail_sp_start..] {
                s.start = (s.start as isize + delta_bytes) as u32;
                s.end = (s.end as isize + delta_bytes) as u32;
            }
        }
        let st_lo = a.strike_starts[line_idx];
        let st_hi = a.strike_starts[line_idx + 1];
        let new_strikes: Vec<(usize, usize, Option<Color>)> = styled.strikes.into_iter().map(|(x, y, c)| (x + line_start, y + line_start, Some(c))).collect();
        let delta_st = new_strikes.len() as isize - (st_hi - st_lo) as isize;
        let tail_st_start = st_lo + new_strikes.len();
        a.strikes.splice(st_lo..st_hi, new_strikes);
        if delta_bytes != 0 {
            for (x, y, _) in &mut a.strikes[tail_st_start..] {
                *x = (*x as isize + delta_bytes) as usize;
                *y = (*y as isize + delta_bytes) as usize;
            }
        }
        if a.hangs[line_idx] != styled.hang {
            a.hangs[line_idx] = styled.hang;
            a.para_styles[line_idx] = crate::ui::editor::paragraph_style(styled.hang, f32::from_bits(a.para_gutter));
        }
        if delta_bytes != 0 {
            for off in &mut a.line_starts[line_idx + 1..] {
                *off = (*off as isize + delta_bytes) as usize;
            }
        }
        if delta_sp != 0 {
            for off in &mut a.span_starts[line_idx + 1..] {
                *off = (*off as isize + delta_sp) as usize;
            }
        }
        if delta_st != 0 {
            for off in &mut a.strike_starts[line_idx + 1..] {
                *off = (*off as isize + delta_st) as usize;
            }
        }
        a.text_key = new_key;
    }

    /// The source's spans, strike lines, and paragraph styles for the whole note, cached while
    /// the note text and styling inputs are unchanged (`LIVEPACE`).
    pub fn source_spans_and_styles(&self, e: f32, ink: &Ink, focus: Option<(usize, usize)>, gutter: f32) -> (Vec<Span>, Vec<(usize, usize, Option<Color>)>, Vec<ParagraphStyle>) {
        self.ensure_assembled(e, ink, focus, gutter);
        let a = self.wstate.assembled.borrow();
        let a = a.as_ref().expect("just assembled");
        (a.spans.clone(), a.strikes.clone(), a.para_styles.clone())
    }

    /// The same styling for the edit sheet's few lines, uncached (SHEET-027).
    pub fn sheet_spans(&self, text: &str, e: f32, ink: &Ink) -> Spans {
        let resolves = |t: &str| self.names.resolve(t).is_some();
        spans_by(text, (0, 0, 0), |i, ln, st| Rc::new(writer::line(ln, i == 0, st, e, ink, &resolves)))
    }

    /// The word count of the open note, kept until its text changes (WR-188).
    pub fn word_count(&self) -> Count {
        let key = self.active_text_key();
        if let Some((k, c)) = &*self.wstate.count.borrow()
            && k.matches(key)
        {
            return *c;
        }
        let c = writer::count(self.active_text());
        *self.wstate.count.borrow_mut() = Some((key.to_owned(), c));
        c
    }

    /// Line count of the active note, cached per `TextKey` (`LIVEPACE`).
    pub fn line_count(&self) -> usize {
        if self.active.is_none() {
            return 0;
        }
        let key = self.active_text_key();
        if let Some((k, n)) = &*self.wstate.lines_count.borrow()
            && k.matches(key)
        {
            return *n;
        }
        let n = self.active_text().as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
        *self.wstate.lines_count.borrow_mut() = Some((key.to_owned(), n));
        n
    }

    /// The active note's headings and H1 sections, plus a version number that only advances when
    /// the heading list actually changes (so typing prose leaves outline memos intact).
    pub fn headings_cached(&self) -> (Rc<Vec<md::Heading>>, Rc<Vec<md::Heading>>, u64) {
        let key = self.active_text_key();
        let mut slot = self.wstate.headings.borrow_mut();
        if let Some(c) = slot.as_ref()
            && c.text_key.matches(key)
        {
            return (c.heads.clone(), c.h1.clone(), c.ver);
        }
        let fresh = md::headings(self.active_text());
        if let Some(c) = slot.as_mut()
            && *c.heads == fresh
        {
            c.text_key = key.to_owned();
            return (c.heads.clone(), c.h1.clone(), c.ver);
        }
        let ver = slot.as_ref().map_or(1, |c| c.ver.wrapping_add(1).max(1));
        let h1: Vec<md::Heading> = fresh.iter().filter(|h| h.level == 1).cloned().collect();
        let heads = Rc::new(fresh);
        let h1 = Rc::new(h1);
        *slot = Some(CachedHeadings { text_key: key.to_owned(), ver, heads: heads.clone(), h1: h1.clone() });
        (heads, h1, ver)
    }

    /// Sorted backlinks to the active note, cached while no *other* note in the vault changes
    /// (`LIVEPACE`).
    pub fn backlinks_cached(&self) -> (Rc<Vec<(String, usize)>>, u64) {
        let Some(active) = self.active.as_deref() else {
            return (Rc::new(Vec::new()), 0);
        };
        let (vault_id, other_writes) = self.vault.as_ref().map_or((0, 0), |v| (v.id(), v.other_writes(active)));
        let mut slot = self.wstate.backlinks.borrow_mut();
        if let Some(c) = slot.as_ref()
            && c.vault_id == vault_id
            && c.other_writes == other_writes
            && c.active == active
        {
            return (c.links.clone(), c.ver);
        }
        let mut back = md::backlinks(self.files(), active);
        if let Some(vault) = &self.vault {
            back.sort_by_key(|(p, _)| (vault.born.get(p).copied().unwrap_or(u128::MAX), p.clone()));
        }
        let ver = slot.as_ref().map_or(1, |c| c.ver.wrapping_add(1).max(1));
        let links = Rc::new(back);
        *slot = Some(CachedBacklinks { vault_id, active: active.to_owned(), other_writes, ver, links: links.clone() });
        (links, ver)
    }

    /// The writer's editing keys act: in the edit sheet, or on an open note in source mode.
    pub(crate) fn writing(&self) -> bool {
        self.sheet().is_some() || (self.mode == Mode::Source && self.active.as_deref().is_some_and(|a| !self.note_locked(a)))
    }

    /// The editor the editing keys change: the edit sheet's while it is open (SHEET-035), else
    /// the open note's.
    pub(crate) fn target_edit(&mut self) -> &mut TextEdit {
        if self.sheet().is_some() {
            return self.sheet_edit().expect("the sheet is open");
        }
        self.cur_edit()
    }

    /// The target editor's text changed: the note saves soon, and the sheet keeps its text
    /// until it saves.
    pub(crate) fn target_changed(&mut self, tx: &mut Tx) {
        if self.sheet().is_some() {
            tx.focus(crate::sheet::EDITOR, false);
        } else {
            self.after_edit();
            self.focus_editor(tx);
        }
    }

    /// One edit as one undo step (WR-63).
    pub fn apply_edit(&mut self, ed: Edit, tx: &mut Tx) {
        let e = self.target_edit();
        e.undo_group(|e| {
            e.set_selection(ed.start, ed.end);
            e.insert(&ed.insert);
            e.set_selection(ed.sel.0, ed.sel.1);
        });
        self.target_changed(tx);
    }

    fn sel(&mut self) -> (String, usize, usize) {
        let e = self.target_edit();
        (e.text.clone(), e.anchor, e.focus)
    }

    /// Enter (WR-39 to WR-44): continue a list or quote, else a plain newline.
    pub fn writer_enter(&mut self, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, a, b) = self.sel();
        let cont = self.settings().continue_lists;
        match writer::enter(&text, a.min(b), a.max(b)).filter(|_| cont) {
            Some(ed) => self.apply_edit(ed, tx),
            None => {
                self.target_edit().insert("\n");
                self.target_changed(tx);
            }
        }
    }

    /// Tab and Shift+Tab (WR-47 to WR-50).
    pub fn writer_tab(&mut self, out: bool, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, a, b) = self.sel();
        let ed = if out { writer::shift_tab(&text, a, b) } else { writer::tab(&text, a, b) };
        if let Some(ed) = ed {
            self.apply_edit(ed, tx);
        }
    }

    /// Ctrl+Alt+1 to Ctrl+Alt+6, Ctrl+Alt+0 (WR-59, WR-60).
    pub fn writer_heading(&mut self, level: usize, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, _, focus) = self.sel();
        self.apply_edit(writer::heading(&text, focus, level), tx);
    }

    /// Ctrl+Shift+Enter: toggle task checkbox `[ ]` / `[x]` on the current line or selection.
    pub fn writer_toggle_checkbox(&mut self, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, a, b) = self.sel();
        if let Some(ed) = writer::toggle_checkbox(&text, a, b) {
            self.apply_edit(ed, tx);
        }
    }

    /// Alt+Up / Alt+Down: move the current line or selected lines up or down one line.
    pub fn writer_move_lines(&mut self, down: bool, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, a, b) = self.sel();
        if let Some(ed) = writer::move_lines(&text, a, b, down) {
            self.apply_edit(ed, tx);
        }
    }

    /// Ctrl+Shift+D (WR-79, WR-80).
    pub fn toggle_paragraph_focus(&mut self, tx: &mut Tx) {
        let on = !self.settings().paragraph_focus;
        self.settings_mut().paragraph_focus = on;
        self.toast(if on { "Focus on" } else { "Focus off" }, tx);
    }

    /// F9 (WR-85).
    pub fn toggle_typewriter(&mut self, tx: &mut Tx) {
        let on = !self.settings().typewriter_scroll;
        self.settings_mut().typewriter_scroll = on;
        self.wstate.center = on;
        self.toast(if on { "Typewriter on" } else { "Typewriter off" }, tx);
    }

    /// Ctrl+Shift+M (WR-125): Quattro S, Duospace, Mono S, then Quattro S again.
    pub fn next_font(&mut self, tx: &mut Tx) {
        let cur = self.settings().font.clone();
        let next = FONT_CYCLE.iter().position(|f| *f == cur).map_or(FONT_CYCLE[0], |i| FONT_CYCLE[(i + 1) % FONT_CYCLE.len()]);
        self.settings_mut().font = next.to_string();
        let label = crate::theme::editor_font(next).label;
        self.toast(label, tx);
    }

    /// Ctrl+S (WR-64).
    pub fn save_now(&mut self, tx: &mut Tx) {
        self.flush_notes();
        self.toast("Saved", tx);
    }

    /// Esc in the writer (WR-77): the notice, the find bar, then Quire's own overlays and zen
    /// mode, then the preview. Help and every other layer take Esc first (`keys::on_key`).
    pub fn writer_escape(&mut self, tx: &mut Tx) -> bool {
        if self.notice.take().is_some() {
            return true;
        }
        if self.reader.find.open {
            self.close_find(tx);
            return true;
        }
        if self.escape(tx) {
            return true;
        }
        if self.active.is_none() {
            return false;
        }
        self.show_mode(crate::app::Mode::Preview, tx);
        true
    }

    // ---------- Find in the source (WR-115 to WR-123) ----------

    pub fn source_matches(&self) -> Vec<Range<usize>> {
        self.find_query().map_or_else(Vec::new, |q| find_in(self.active_text(), q))
    }

    /// Ctrl+F in the writer: the editor's selection becomes the query (WR-115).
    pub fn writer_find(&mut self, tx: &mut Tx) {
        if self.active.is_none() || self.active_locked() {
            return;
        }
        if let Some(e) = self.cur_edit_opt() {
            let (a, b) = (e.anchor.min(e.focus), e.anchor.max(e.focus));
            let sel = e.text[a..b].to_string();
            if !sel.is_empty() && !sel.contains('\n') {
                self.reader.find.q = rune::TextEdit::new(sel);
            }
        }
        self.open_find(tx);
        self.reader.find.searched = self.reader.find.q.text.clone();
        if self.find_query().is_some() {
            self.source_found();
        }
    }

    /// The query changed: the first match at or after the editor's selection (WR-117).
    pub fn source_found(&mut self) {
        let m = self.source_matches();
        let from = self.cur_edit_opt().map_or(0, |e| e.anchor.min(e.focus));
        self.reader.find.cur = m.iter().position(|r| r.start >= from).unwrap_or(0);
        self.select_match(&m, false);
    }

    /// Ctrl+G, F3 and Shift+F3, or Enter in the bar (WR-119, WR-120).
    pub fn source_step(&mut self, forward: bool, tx: &mut Tx) {
        if self.find_query().is_none() {
            self.writer_find(tx);
            return;
        }
        let m = self.source_matches();
        if m.is_empty() {
            return;
        }
        let n = m.len();
        let cur = self.reader.find.cur.min(n - 1);
        self.reader.find.cur = if forward { (cur + 1) % n } else { (cur + n - 1) % n };
        self.select_match(&m, true);
    }

    /// Selects the current match in the editor and brings it into view (WR-118), above the
    /// toast's band while one shows (RD-104): a step glides, a search as you type jumps.
    fn select_match(&mut self, m: &[Range<usize>], glide: bool) {
        let Some(r) = m.get(self.reader.find.cur).cloned() else { return };
        if let Some(e) = self.cur_edit_opt() {
            e.set_selection(r.start, r.end);
            e.break_history_group();
        }
        self.reveal_match(glide);
    }

    // ---------- Find & Replace in Source (`Ctrl+H`) ----------

    /// `Ctrl+H` in Source mode: opens the Find bar with the Replace row expanded, focusing
    /// `find-input` (so typing immediately searches) and seeding any single-line selection.
    pub fn writer_replace(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Source || self.active.is_none() || self.active_locked() {
            return;
        }
        let was_open = self.reader.find.open;
        let had_replace = self.reader.find.replace_open;
        self.reader.find.replace_open = true;
        self.writer_find(tx);
        if was_open && !had_replace {
            self.adjust_find_slot(crate::ui::reader::FIND_REPLACE_H, tx);
        }
    }

    /// Toggles the Source-mode Replace row inside an open Find bar.
    pub fn toggle_replace(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Source || self.active.is_none() || self.active_locked() {
            return;
        }
        let next = !self.reader.find.replace_open;
        self.reader.find.replace_open = next;
        let delta = if next { crate::ui::reader::FIND_REPLACE_H } else { -crate::ui::reader::FIND_REPLACE_H };
        self.adjust_find_slot(delta, tx);
        if next && !self.reader.find.q.text.is_empty() {
            let n = self.reader.find.replace.text.len();
            self.reader.find.replace.set_selection(0, n);
            self.reader.find.replace.focused = true;
            tx.focus("replace-input", false);
        } else {
            self.reader.find.replace.focused = false;
            tx.focus("find-input", false);
        }
    }

    fn adjust_find_slot(&mut self, delta: f32, tx: &mut Tx) {
        let shows_toolbar = !self.focus && self.mode == Mode::Source && self.settings().toolbar;
        if shows_toolbar {
            return;
        }
        let offset = self.reader.scroll.get()[0];
        if offset <= 0.5 {
            return;
        }
        let to = (offset + delta).max(0.0);
        tx.scroll_to_instant(crate::reader::SCROLL, 0.0, to);
    }

    /// Replaces the current Find match in Source mode as one undo step and advances to the next.
    pub fn replace_one(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Source || self.active.is_none() || self.active_locked() {
            return;
        }
        let m = self.source_matches();
        if m.is_empty() {
            return;
        }
        let cur = self.reader.find.cur.min(m.len() - 1);
        let r = m[cur].clone();
        let rep = self.reader.find.replace.text.clone();
        if let Some(e) = self.cur_edit_opt() {
            e.undo_group(|e| {
                e.set_selection(r.start, r.end);
                e.insert(&rep);
                e.set_selection(r.start + rep.len(), r.start + rep.len());
            });
        }
        self.after_edit();
        let after = r.start + rep.len();
        let m2 = self.source_matches();
        if m2.is_empty() {
            self.reader.find.cur = 0;
        } else {
            self.reader.find.cur = m2.iter().position(|x| x.start >= after).unwrap_or(0);
            self.select_match(&m2, true);
        }
        let _ = tx;
    }

    /// Replaces every Find match in the active Source note in a single undo group.
    pub fn replace_all(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Source || self.active.is_none() || self.active_locked() {
            return;
        }
        let m = self.source_matches();
        if m.is_empty() {
            return;
        }
        let count = m.len();
        let rep = self.reader.find.replace.text.clone();
        if let Some(e) = self.cur_edit_opt() {
            e.undo_group(|e| {
                for r in m.iter().rev() {
                    e.set_selection(r.start, r.end);
                    e.insert(&rep);
                }
                if let Some(first) = m.first() {
                    let at = first.start + rep.len();
                    e.set_selection(at, at);
                }
            });
        }
        self.after_edit();
        self.reader.find.cur = 0;
        self.toast(&format!("Replaced {} {}", count, if count == 1 { "match" } else { "matches" }), tx);
    }

    // ---------- Inline Autocomplete (`[[wiki]]` and `#tag`) ----------

    /// Active inline autocomplete state `(trigger, suggestions, selected_index)` in Source mode.
    pub fn ac_state(&self) -> Option<(writer::AcTrigger, Vec<AcItem>, usize)> {
        if self.mode != Mode::Source || self.sheet().is_some() || self.active_locked() || self.layer().is_some() || self.reader.find.open {
            return None;
        }
        let active = self.active.as_deref()?;
        let edit = self.edits.get(active)?;
        if !edit.focused || edit.anchor != edit.focus {
            return None;
        }
        let trig = writer::ac_trigger(&edit.text, edit.focus.min(edit.text.len()))?;
        if self.wstate.ac_dismissed.as_ref().is_some_and(|(p, st)| p == active && *st == trig.start) {
            return None;
        }
        let items = match trig.kind {
            writer::AcKind::Wiki => self.ac_wiki_items(&trig.query),
            writer::AcKind::Tag => self.ac_tag_items(&trig.query, &edit.text),
        };
        if items.is_empty() {
            return None;
        }
        let idx = self.wstate.ac_idx.min(items.len() - 1);
        Some((trig, items, idx))
    }

    pub fn ac_active(&self) -> bool {
        self.ac_state().is_some()
    }

    fn ac_wiki_items(&self, q: &str) -> Vec<AcItem> {
        let files = self.files();
        let paths: Vec<String> = if q.is_empty() {
            let rec: Vec<&String> = self.recent.iter().filter(|x| files.contains_key(*x)).collect();
            let mut rest: Vec<&String> = files.keys().filter(|x| !rec.contains(x)).collect();
            rest.sort();
            rec.into_iter().chain(rest).take(8).cloned().collect()
        } else {
            let mut scored: Vec<(f32, String)> = files
                .keys()
                .filter_map(|x| {
                    if let Some((sc, _)) = md::fuzzy(q, md::base(x)) {
                        return Some((sc + 500.0, x.clone()));
                    }
                    md::fuzzy(q, md::strip_md(x)).map(|(sc, _)| (sc, x.clone()))
                })
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().take(8).map(|(_, x)| x).collect()
        };
        paths
            .into_iter()
            .map(|p| {
                let base = md::base(&p);
                let target = if md::dir(&p).is_empty() || self.names.resolve(base) == Some(p.as_str()) {
                    base.to_string()
                } else {
                    md::strip_md(&p).to_string()
                };
                AcItem {
                    label: base.to_string(),
                    sub: md::dir(&p).to_string(),
                    insert: format!("[[{target}]]"),
                }
            })
            .collect()
    }

    fn ac_tag_items(&self, q: &str, live_text: &str) -> Vec<AcItem> {
        use std::collections::BTreeMap;
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        if let Some(v) = &self.vault {
            for (t, n) in self.search.borrow_mut().all_tags(v) {
                counts.insert(t, n);
            }
            for text in v.files().values() {
                for t in frontmatter_tags(text) {
                    counts.entry(t).or_insert(1);
                }
            }
        }
        for t in md::note_tags(live_text).into_iter().chain(frontmatter_tags(live_text)) {
            counts.entry(t).or_insert(1);
        }
        let ql = q.to_lowercase();
        let mut scored: Vec<(i32, usize, String)> = counts
            .into_iter()
            .filter_map(|(t, n)| {
                if ql.is_empty() {
                    Some((0, n, t))
                } else if t.starts_with(&ql) {
                    Some((200, n, t))
                } else if t.contains(&ql) {
                    Some((100, n, t))
                } else {
                    md::fuzzy(&ql, &t).map(|(sc, _)| (sc as i32, n, t))
                }
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)).then_with(|| a.2.cmp(&b.2)));
        scored
            .into_iter()
            .take(8)
            .map(|(_, n, t)| AcItem {
                label: format!("#{t}"),
                sub: if n > 0 { n.to_string() } else { String::new() },
                insert: format!("#{t}"),
            })
            .collect()
    }

    pub fn ac_after_edit(&mut self) {
        self.wstate.ac_idx = 0;
        let Some(active) = self.active.as_deref() else {
            self.wstate.ac_dismissed = None;
            return;
        };
        let keep = self.edits.get(active).and_then(|e| writer::ac_trigger(&e.text, e.focus.min(e.text.len()))).is_some_and(|trig| {
            self.wstate.ac_dismissed.as_ref().is_some_and(|(p, st)| p == active && *st == trig.start)
        });
        if !keep {
            self.wstate.ac_dismissed = None;
        }
    }

    pub fn ac_step(&mut self, down: bool) -> bool {
        let Some((_, items, cur)) = self.ac_state() else { return false };
        let n = items.len();
        self.wstate.ac_idx = if down { (cur + 1) % n } else { (cur + n - 1) % n };
        true
    }

    pub fn ac_dismiss(&mut self) -> bool {
        let Some((trig, _, _)) = self.ac_state() else { return false };
        self.wstate.ac_dismissed = self.active.clone().map(|a| (a, trig.start));
        true
    }

    pub fn ac_accept(&mut self, index: Option<usize>, tx: &mut Tx) -> bool {
        let Some((trig, items, cur)) = self.ac_state() else { return false };
        let item = &items[index.unwrap_or(cur).min(items.len() - 1)];
        let mut end = trig.end;
        if trig.kind == writer::AcKind::Wiki && self.active_text().get(end..).is_some_and(|rest| rest.starts_with("]]")) {
            end += 2;
        }
        let insert = item.insert.clone();
        let caret = trig.start + insert.len();
        if let Some(e) = self.cur_edit_opt() {
            e.undo_group(|e| {
                e.set_selection(trig.start, end);
                e.insert(&insert);
                e.set_selection(caret, caret);
            });
        }
        self.wstate.ac_idx = 0;
        self.wstate.ac_dismissed = None;
        self.after_edit();
        self.focus_editor(tx);
        true
    }

    // ---------- Scrolling ----------

    /// Holds source line `line` at `min(top padding, view height / 3)` below the view's top
    /// (WR-102). The line carries a keyed highlight for one moment so the scroll can find it.
    pub fn place_line(&mut self, line: usize, tx: &mut Tx) {
        let vh = self.view_h.get();
        self.wstate.top_line = Some((line, self.now.get()));
        tx.scroll_key_to(LINE_AT, (vh * 0.12).max(56.0).min(vh / 3.0));
    }

    /// Puts the caret at line `line`'s start and brings the line into view: at the top, or at
    /// the middle with typewriter scrolling on (WR-102).
    pub fn source_jump(&mut self, line: usize, tx: &mut Tx) {
        let pos = md::line_start(self.active_text(), line);
        if let Some(e) = self.cur_edit_opt() {
            e.set_selection(pos, pos);
            e.break_history_group();
        }
        if self.settings().typewriter_scroll {
            self.wstate.center = true;
        } else {
            self.place_line(line, tx);
        }
        self.focus_editor(tx);
    }

    /// Source to preview: the block holding the caret's line goes to the top (WR-108).
    /// Preview to source: back where the editor was, unless the preview was scrolled; then
    /// the line at the preview's top goes to the editor's top (WR-112).
    pub fn switch_mode(&mut self, tx: &mut Tx) {
        tx.clear_text_selection();
        if self.reader.find.open && self.find_pending() {
            self.reader.find.searched = self.reader.find.q.text.clone();
            self.reader.find.due = None;
        }
        if self.mode == Mode::Source {
            let text = self.active_text();
            let caret = self.edits.get(self.active.as_deref().unwrap_or_default()).map_or(0, |e| e.focus.min(text.len()));
            let line = md::line_col(text, caret).0;
            let doc = self.doc();
            let block = doc.blocks.iter().rposition(|b| b.lines.start <= line).unwrap_or(0);
            self.wstate.back = Some((self.reader.scroll.get()[0], block));
            self.mode = Mode::Preview;
            // The editor leaves with the keyboard: hand focus to the reader's keyboard anchor
            // (`focus::NOTE`) so Preview shortcuts and Tab into links stay wired (`FOCUSMODEL`).
            self.keys_to_note(tx);
            if block == 0 {
                // The first block leaves the preview at its top, where progress reads 0 (RD-38).
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, 0.0);
            } else {
                self.land(block, false, tx);
            }
        } else {
            let top = self.top();
            let line = self.doc().blocks.get(top).map_or(0, |b| b.lines.start);
            self.mode = Mode::Source;
            // Not scrolled: the top block is the one placed there, or the page sits clamped at
            // its end above that block.
            let [offset, max, _, _] = self.reader.scroll.get();
            match self.wstate.back.take() {
                Some((y, block)) if block == top || (offset >= max - 1.0 && top <= block) => tx.scroll_to_instant(crate::reader::SCROLL, 0.0, y),
                _ => self.place_line(line, tx),
            }
            self.keys_to_note(tx);
        }
    }

    /// Before each frame: typewriter scrolling follows the caret.
    pub(crate) fn poll_writer(&mut self, t: f64, tx: &mut Tx) {
        if self.writing() {
            let at = self.active.clone().zip(self.cur_edit_opt().map(|e| e.focus));
            if self.wstate.center || (self.settings().typewriter_scroll && at != self.wstate.caret) {
                tx.scroll_center(CARET);
            }
            self.wstate.caret = at;
        }
        self.wstate.center = false;
        if self.wstate.top_line.is_some_and(|(_, since)| t - since > 0.5) {
            self.wstate.top_line = None;
        }
    }
}

/// Extracts lowercased tag names from YAML front matter `tags:` / `tag:` fields (inline `[a, b]`,
/// comma-separated, or bulleted list).
fn frontmatter_tags(text: &str) -> Vec<String> {
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut in_list = false;
    for ln in text.lines().skip(1) {
        let t = ln.trim();
        if t == "---" || t == "..." {
            break;
        }
        if in_list {
            if let Some(rest) = t.strip_prefix("- ") {
                let tag = rest.trim().trim_matches(|c| c == '"' || c == '\'').trim_start_matches('#').to_lowercase();
                if !tag.is_empty() && !out.contains(&tag) {
                    out.push(tag);
                }
                continue;
            }
            if !t.is_empty() {
                in_list = false;
            }
        }
        if let Some((k, v)) = ln.split_once(':') {
            if matches!(k.trim().to_ascii_lowercase().as_str(), "tags" | "tag") {
                let v = v.trim();
                if v.is_empty() {
                    in_list = true;
                } else {
                    let inner = v.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(v);
                    for part in inner.split(',') {
                        let tag = part.trim().trim_matches(|c| c == '"' || c == '\'').trim_start_matches('#').to_lowercase();
                        if !tag.is_empty() && !out.contains(&tag) {
                            out.push(tag);
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{find_in, frontmatter_tags};

    #[test]
    fn frontmatter_tags_parses_inline_and_block_lists() {
        assert_eq!(frontmatter_tags("---\ntags: [design, roadmap]\n---\nBody"), vec!["design", "roadmap"]);
        assert_eq!(frontmatter_tags("---\ntags:\n  - alpha\n  - #beta\n---\n"), vec!["alpha", "beta"]);
        assert!(frontmatter_tags("# No front matter\ntags: [x]").is_empty());
    }

    #[test]
    fn wr_116_find_is_literal_and_case_blind() {
        assert_eq!(find_in("Fast fast FAST", "fast"), vec![0..4, 5..9, 10..14]);
        assert_eq!(find_in("aaaa", "aa"), vec![0..2, 2..4], "no overlaps");
        assert_eq!(find_in("a.b", "."), vec![1..2]);
        assert_eq!(find_in("Straße fast", "fast"), vec![8..12]);
        assert!(find_in("x", "").is_empty());
    }

    /// COLDPACE: incremental single-line patching of `Assembled`, `line_count`, and
    /// `headings_cached` produces bit-for-bit identical spans, strikes, paragraph styles,
    /// line counts, and headings as a full rebuild across prose, heading, UTF-8, bold/strike,
    /// fence toggle, and newline edits.
    #[test]
    fn coldpace_incremental_patch_matches_full_assembly() {
        use crate::roles::Roles;
        use crate::writer::Ink;
        let dir = std::env::temp_dir().join(format!("quire-coldpace-patch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let initial = "# Title\n\nFirst paragraph with **bold** and ~~strike~~.\n\n## Second\n\n- bullet item\n- another item\n\n```rust\nlet x = 1;\n```\n\nTail line.\n";
        std::fs::write(dir.join("note.md"), initial).unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("note.md", None, tx);
            s.mode = crate::app::Mode::Source;
        });
        let p = crate::theme::theme_pal("paper", false);
        let c = Roles::new(p);
        let ink = Ink::new(&c);
        let e = 14.0f32;
        let gutter = crate::ui::editor::hang_gutter(e);

        let edits: &[(&str, &str)] = &[
            ("prose-insert", "# Title\n\nFirst paragraph with **bold** and ~~strike~~ and café.\n\n## Second\n\n- bullet item\n- another item\n\n```rust\nlet x = 1;\n```\n\nTail line.\n"),
            ("prose-delete", "# Title\n\nFirst para with **bold** and ~~strike~~ and café.\n\n## Second\n\n- bullet item\n- another item\n\n```rust\nlet x = 1;\n```\n\nTail line.\n"),
            ("fenced-code-edit", "# Title\n\nFirst para with **bold** and ~~strike~~ and café.\n\n## Second\n\n- bullet item\n- another item\n\n```rust\nlet x = 42 + y;\n```\n\nTail line.\n"),
            ("heading-prefix", "livecheck # Title\n\nFirst para with **bold** and ~~strike~~ and café.\n\n## Second\n\n- bullet item\n- another item\n\n```rust\nlet x = 42 + y;\n```\n\nTail line.\n"),
            ("list-hang-change", "livecheck # Title\n\nFirst para with **bold** and ~~strike~~ and café.\n\n## Second\n\nplain item\n- another item\n\n```rust\nlet x = 42 + y;\n```\n\nTail line.\n"),
            ("fence-open-fallback", "livecheck # Title\n\n```\n\n## Second\n\nplain item\n- another item\n\n```rust\nlet x = 42 + y;\n```\n\nTail line.\n"),
            ("newline-fallback", "# Title\n\nLine A\nLine B\n\n## Second\n\nTail.\n"),
        ];

        // Prime caches on initial text.
        rt.update_if(|s, _| {
            let _ = s.source_spans_and_styles(e, &ink, None, gutter);
            let _ = s.line_count();
            let _ = s.headings_cached();
            let _ = s.word_count();
            false
        });

        for &(name, next) in edits {
            let next_s = next.to_string();
            rt.update_if(move |s, _| {
                s.cur_edit().replace_text(&next_s);
                s.after_edit();
                let got_spans = s.source_spans_and_styles(e, &ink, None, gutter);
                let got_lines = s.line_count();
                let got_heads = s.headings_cached().0;
                let got_count = s.word_count();
                // Force full uncached rebuild and compare bit-for-bit.
                *s.wstate.assembled.borrow_mut() = None;
                *s.wstate.lines_count.borrow_mut() = None;
                *s.wstate.headings.borrow_mut() = None;
                *s.wstate.count.borrow_mut() = None;
                let want_spans = s.source_spans_and_styles(e, &ink, None, gutter);
                let want_lines = s.line_count();
                let want_heads = s.headings_cached().0;
                let want_count = s.word_count();
                assert_eq!(got_spans.0, want_spans.0, "{name}: spans mismatch");
                assert_eq!(got_spans.1, want_spans.1, "{name}: strikes mismatch");
                assert_eq!(got_spans.2, want_spans.2, "{name}: para_styles mismatch");
                assert_eq!(got_lines, want_lines, "{name}: line_count mismatch");
                assert_eq!(*got_heads, *want_heads, "{name}: headings mismatch");
                assert_eq!(got_count, want_count, "{name}: word_count mismatch");
                false
            });
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
