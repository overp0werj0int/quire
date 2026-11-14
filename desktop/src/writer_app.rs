//! The writer on `App` (`docs/reader-writer/behavior/writer.md`): the source's formatting
//! spans, cached per line; the editing keys; paragraph focus, typewriter scrolling and the
//! font cycle; find in the source; and the chrome that steps aside while you type.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use rune::{Color, Span, Tx};

use crate::app::{App, Mode};
use crate::format::Edit;
use crate::md;
use crate::writer::{self, Count, Ink, State, Styled};

/// A transparent highlight over the caret's character: typewriter scrolling centres it.
pub const CARET: &str = "caret-at";
/// A transparent highlight over a line placed at the top of the view (WR-102, WR-112).
pub const LINE_AT: &str = "line-at";
/// "Typing" lasts this long after the text last changed (WR-184).
pub const TYPING: f64 = 1.4;
/// The source fonts Ctrl+Shift+M steps through (WR-125).
const FONT_CYCLE: [&str; 3] = ["quattro", "duospace", "ia-mono"];

type Key = (u64, State, bool, bool);

#[derive(Default)]
struct Cache {
    /// What the cached lines were styled with: the size and colours, and the note index.
    with: Option<(u32, [u32; 40], u64)>,
    lines: HashMap<Key, Rc<(Styled, State)>>,
}

#[derive(Default)]
pub struct WriterState {
    /// The text changed since the last poll.
    pub typed: bool,
    /// The note and text hash the last poll saw, so any edit counts as typing.
    pub(crate) seen: Option<(String, u64)>,
    /// When the text last changed, and when the pointer last moved over the window.
    pub typing_at: f64,
    pub pointer_at: f64,
    /// The chrome is stepped aside, as last drawn (WR-185).
    pub hidden: bool,
    /// The caret the last frame saw, so typewriter scrolling follows moves (WR-88).
    caret: Option<(String, usize)>,
    /// Centre the caret line on the next frame (a forced move).
    pub center: bool,
    /// A source line to hold at the top of the view, and since when.
    pub top_line: Option<(usize, f64)>,
    /// Leaving source mode: the editor's scroll offset, and the preview block placed at the top.
    back: Option<(f32, usize)>,
    cache: RefCell<Cache>,
    count: RefCell<Option<(u64, Count)>>,
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
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

impl App {
    /// The source's spans and strike lines for the whole note (section 1), with the lines
    /// outside `focus` (first and last line) faded (WR-82).
    pub fn source_spans(&self, e: f32, ink: &Ink, focus: Option<(usize, usize)>) -> (Vec<Span>, Vec<(usize, usize, Option<Color>)>) {
        let text = self.active_text();
        let with = (e.to_bits(), ink_bits(ink), self.names_gen as u64);
        let mut cache = self.wstate.cache.borrow_mut();
        if cache.with != Some(with) || cache.lines.len() > 50_000 {
            cache.lines.clear();
            cache.with = Some(with);
        }
        let mut spans = Vec::new();
        let mut strikes = Vec::new();
        let mut st = State::default();
        let mut at = 0;
        let names = &self.names;
        let resolves = |t: &str| names.resolve(t).is_some();
        for (i, ln) in text.split('\n').enumerate() {
            let faded = focus.is_some_and(|(a, b)| i < a || i > b);
            let key = (hash(ln), st, i == 0, faded);
            let hit = cache.lines.get(&key).cloned();
            let styled = hit.unwrap_or_else(|| {
                let (s, next) = writer::line(ln, i == 0, st, e, ink, &resolves);
                let s = if faded { writer::fade(&s, ln.len(), ink.ghost) } else { s };
                let v = Rc::new((s, next));
                cache.lines.insert(key, v.clone());
                v
            });
            let (styled, next) = (&styled.0, styled.1);
            st = next;
            for s in &styled.spans {
                spans.push(Span { start: s.start + at as u32, end: s.end + at as u32, ..s.clone() });
            }
            strikes.extend(styled.strikes.iter().map(|&(a, b, c)| (a + at, b + at, Some(c))));
            at += ln.len() + 1;
        }
        (spans, strikes)
    }

    /// The word count of the open note, kept until its text changes (WR-188).
    pub fn word_count(&self) -> Count {
        let text = self.active_text();
        let h = hash(text);
        if let Some((k, c)) = *self.wstate.count.borrow()
            && k == h
        {
            return c;
        }
        let c = writer::count(text);
        *self.wstate.count.borrow_mut() = Some((h, c));
        c
    }

    fn writing(&self) -> bool {
        self.mode == Mode::Source && self.active.as_deref().is_some_and(|a| !self.note_locked(a))
    }

    /// One edit as one undo step (WR-63).
    pub fn apply_edit(&mut self, ed: Edit, tx: &mut Tx) {
        let e = self.cur_edit();
        e.undo_group(|e| {
            e.set_selection(ed.start, ed.end);
            e.insert(&ed.insert);
            e.set_selection(ed.sel.0, ed.sel.1);
        });
        self.after_edit();
        self.focus_editor(tx);
    }

    fn sel(&mut self) -> (String, usize, usize) {
        let e = self.cur_edit();
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
                self.cur_edit().insert("\n");
                self.after_edit();
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

    /// Ctrl+1 to Ctrl+6, Ctrl+0 (WR-59, WR-60).
    pub fn writer_heading(&mut self, level: usize, tx: &mut Tx) {
        if !self.writing() {
            return;
        }
        let (text, _, focus) = self.sel();
        self.apply_edit(writer::heading(&text, focus, level), tx);
    }

    /// Ctrl+Shift+F (WR-79, WR-80).
    pub fn toggle_paragraph_focus(&mut self, tx: &mut Tx) {
        let on = !self.settings().paragraph_focus;
        self.settings_mut().paragraph_focus = on;
        self.toast(if on { "Focus on" } else { "Focus off" }, tx);
    }

    /// Ctrl+Shift+T (WR-85).
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

    /// Esc in the writer (WR-77): the help sheet, the find bar, then Quire's own overlays and
    /// zen mode.
    pub fn writer_escape(&mut self, tx: &mut Tx) -> bool {
        if self.reader.help {
            self.reader.help = false;
            self.focus_editor(tx);
            return true;
        }
        if self.reader.find.open {
            self.close_find(tx);
            self.focus_editor(tx);
            return true;
        }
        self.escape(tx)
    }

    // ---------- Find in the source (WR-115 to WR-123) ----------

    pub fn source_matches(&self) -> Vec<Range<usize>> {
        self.find_query().map_or_else(Vec::new, |q| find_in(self.active_text(), q))
    }

    /// Ctrl+F in the writer: the editor's selection becomes the query (WR-115).
    pub fn writer_find(&mut self, tx: &mut Tx) {
        if let Some(e) = self.cur_edit_opt() {
            let (a, b) = (e.anchor.min(e.focus), e.anchor.max(e.focus));
            let sel = e.text[a..b].to_string();
            if !sel.is_empty() && !sel.contains('\n') {
                self.reader.find.q = rune::TextEdit::new(sel);
            }
        }
        self.open_find(tx);
        if self.find_query().is_some() {
            self.source_found(tx);
        }
    }

    /// The query changed: the first match at or after the editor's selection (WR-117).
    pub fn source_found(&mut self, tx: &mut Tx) {
        let m = self.source_matches();
        let from = self.cur_edit_opt().map_or(0, |e| e.anchor.min(e.focus));
        self.reader.find.cur = m.iter().position(|r| r.start >= from).unwrap_or(0);
        self.select_match(&m, tx);
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
        self.select_match(&m, tx);
    }

    /// Selects the current match in the editor and glides it into view (WR-118).
    fn select_match(&mut self, m: &[Range<usize>], tx: &mut Tx) {
        let Some(r) = m.get(self.reader.find.cur).cloned() else { return };
        if let Some(e) = self.cur_edit_opt() {
            e.set_selection(r.start, r.end);
            e.break_history_group();
        }
        tx.scroll_center(crate::reader::FIND_CUR);
    }

    // ---------- Scrolling and the chrome ----------

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
        if self.mode == Mode::Source {
            let text = self.active_text();
            let caret = self.edits.get(self.active.as_deref().unwrap_or_default()).map_or(0, |e| e.focus.min(text.len()));
            let line = md::line_col(text, caret).0;
            let doc = self.doc();
            let block = doc.blocks.iter().rposition(|b| b.lines.start <= line).unwrap_or(0);
            self.wstate.back = Some((self.reader.scroll.get()[0], block));
            self.mode = Mode::Preview;
            self.jump_block(line, tx);
        } else {
            let top = self.reader.top.get();
            let line = self.doc().blocks.get(top).map_or(0, |b| b.lines.start);
            self.mode = Mode::Source;
            // Not scrolled: the top block is the one placed there, or the page sits clamped at
            // its end above that block.
            let [offset, max, _, _] = self.reader.scroll.get();
            match self.wstate.back.take() {
                Some((y, block)) if block == top || (offset >= max - 1.0 && top <= block) => tx.scroll_to_instant(crate::reader::SCROLL, 0.0, y),
                _ => self.place_line(line, tx),
            }
            self.focus_editor(tx);
        }
    }

    /// Whether the chrome steps aside now (WR-184 to WR-187).
    fn chrome_away(&self, t: f64) -> bool {
        self.writing()
            && t - self.wstate.typing_at < TYPING
            && self.wstate.pointer_at < self.wstate.typing_at
            && self.palette.is_none()
            && !self.settings_open
            && !self.reader.find.open
            && !self.reader.help
            && self.tree_menu.is_none()
            && !self.vault_menu
    }

    /// Before each frame: typewriter scrolling follows the caret, and the chrome steps aside
    /// or comes back.
    pub(crate) fn poll_writer(&mut self, t: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        // Any change to the open note's text is typing, whichever path made it.
        let seen = self.active.clone().map(|a| (a, hash(self.active_text())));
        let edited = matches!((&self.wstate.seen, &seen), (Some((a0, h0)), Some((a1, h1))) if a0 == a1 && h0 != h1);
        if std::mem::take(&mut self.wstate.typed) || edited {
            self.wstate.typing_at = t;
        }
        self.wstate.seen = seen;
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
        let away = self.chrome_away(t);
        if away != self.wstate.hidden {
            self.wstate.hidden = away;
            changed = true;
        }
        if away {
            tx.wake_after(std::time::Duration::from_secs_f64((self.wstate.typing_at + TYPING - t).max(0.05)));
        }
        changed
    }

    /// The pointer moved: the chrome comes back (WR-187). Says whether anything changed.
    pub fn pointer_moved(&mut self) -> bool {
        if !self.wstate.hidden {
            return false;
        }
        self.wstate.pointer_at = self.now.get().max(self.wstate.typing_at) + 1e-6;
        self.wstate.hidden = false;
        true
    }

    /// The chrome's opacity and its fade: out over 600 ms, back over 160 ms (WR-186).
    pub fn chrome_fade(&self) -> (f32, rune::Spring) {
        if self.wstate.hidden { (0.0, rune::Spring::new(0.6, 0.0)) } else { (1.0, rune::Spring::new(0.16, 0.0)) }
    }
}

#[cfg(test)]
mod tests {
    use super::find_in;

    #[test]
    fn wr_116_find_is_literal_and_case_blind() {
        assert_eq!(find_in("Fast fast FAST", "fast"), vec![0..4, 5..9, 10..14]);
        assert_eq!(find_in("aaaa", "aa"), vec![0..2, 2..4], "no overlaps");
        assert_eq!(find_in("a.b", "."), vec![1..2]);
        assert_eq!(find_in("Straße fast", "fast"), vec![8..12]);
        assert!(find_in("x", "").is_empty());
    }
}
