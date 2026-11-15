//! The reader's behaviour (`docs/reader-writer/behavior/reader.md`, phase 6): key scrolling
//! and its glide, the wide measure, find, copy, links, the help sheet, toasts and the reading
//! progress. The page itself is `ui/preview.rs`; its chrome is `ui/reader.rs`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rune::{Color, KeyEvent, Live, Spring, TextEdit, Tx};

use crate::app::{App, Mode};
use crate::blocks::{self, Doc, Match};
use crate::page::r;

/// The note's scroll viewport, shared by source and preview.
pub const SCROLL: &str = "doc-scroll";
/// The current find match's highlight, which scrolling centres.
pub const FIND_CUR: &str = "find-cur";
/// How long a key glide lasts; a key inside it continues from where it is heading (RD-22).
const KEY_GLIDE: f64 = 0.11;
/// The second key of a two-key chord comes within this (RD-23).
const SEQ: f64 = 0.9;
/// A toast shows this long (RD-98).
pub const TOAST: f64 = 1.6;
/// How long a code block's chip says Copied (BLK-51).
pub const COPIED: f64 = 1.4;
/// The "reloaded" mark: fade in, hold, fade out (RD-33).
pub const RELOADED: f64 = 0.12 + 1.1 + 0.5;

pub struct Reader {
    /// The W toggle, for this session only; None follows the width setting (RD-51).
    pub wide: Option<bool>,
    /// The find bar. Kept while closed, because its field's edit hook needs somewhere to write.
    pub find: Find,
    pub help: bool,
    pub toast: Option<(String, f64)>,
    /// The code block whose chip says Copied, by its first source line, and since when.
    pub copied: Option<(usize, f64)>,
    /// When an outside change last reached the open note (RD-14).
    pub reloaded: Option<f64>,
    /// The first source line of the top-level block under the pointer (RD-100).
    pub hover: Option<usize>,
    /// Where the page's blocks sit, from the last view. Shared with the scroll handler, which
    /// draws the reading progress from it without running the view (VER-32).
    pub map: Rc<RefCell<PageMap>>,
    /// The viewport's offset, scroll range, height and width, from its scroll handler.
    pub scroll: Live<[f32; 4]>,
    /// The writer's progress bar, [x, y, w, h], set with the scroll in the frame it draws (WR-178).
    pub bar: Live<[f32; 4]>,
    /// The first key of a two-key chord and when it came.
    pub(crate) pending: Option<(KeyEvent, f64)>,
    /// Where the last key glide is heading and when it started.
    glide: Option<(f32, f64)>,
    /// Each tab's reading position (RD-103).
    positions: HashMap<String, f32>,
    /// The active note's blocks, kept until its text changes.
    doc: RefCell<Option<(u64, Rc<Doc>)>>,
}

impl Default for Reader {
    fn default() -> Self {
        Reader {
            wide: None,
            find: Find { open: false, q: TextEdit::new(""), cur: 0 },
            help: false,
            toast: None,
            copied: None,
            reloaded: None,
            hover: None,
            map: Rc::default(),
            scroll: Live::new([0.0; 4]),
            bar: Live::new([0.0; 4]),
            pending: None,
            glide: None,
            positions: HashMap::new(),
            doc: RefCell::new(None),
        }
    }
}

/// Where each top-level block of the page sits in the scroll viewport's content, so the block at
/// the top of the view follows from the scroll offset alone. The page is a virtual list: Rune
/// reports each row's top (`El::on_row_tops`), measured where the row has been laid out and
/// estimated where not, and a row's block sits its rhythm gap below the row's top.
#[derive(Default)]
pub struct PageMap {
    /// The page is on screen.
    pub reading: bool,
    /// Each row's top, from Rune, and the gap above each block, from the view.
    row_tops: Vec<f32>,
    gaps: Vec<f32>,
    /// Each top-level block's top: the two added.
    pub tops: Vec<f32>,
    /// Each top-level block's first source line, and the note's line count.
    pub starts: Vec<usize>,
    pub lines: usize,
    /// The outline's rows, when it shows: each heading's block and its row, the row current
    /// above the first, and each row's colours.
    pub heads: Vec<(usize, Option<usize>)>,
    pub first_row: Option<usize>,
    pub marks: Vec<Mark>,
    pub marked: Option<usize>,
}

/// An outline row's live colours, [fill, text], and their values when current and not.
pub struct Mark {
    pub fill: Live<Color>,
    pub ink: Live<Color>,
    pub on: [Color; 2],
    pub off: [Color; 2],
}

impl PageMap {
    /// The rows' tops, as Rune reports them.
    pub fn set_row_tops(&mut self, tops: &[f32]) {
        self.row_tops.clear();
        self.row_tops.extend_from_slice(tops);
        self.join();
    }

    /// The gap above each block, from the view.
    pub fn set_gaps(&mut self, gaps: Vec<f32>) {
        if gaps != self.gaps {
            self.gaps = gaps;
            self.join();
        }
    }

    /// The gap above block `i`.
    pub fn gap(&self, i: usize) -> f32 {
        self.gaps.get(i).copied().unwrap_or(0.0)
    }

    fn join(&mut self) {
        let gaps = &self.gaps;
        self.tops = self.row_tops.iter().enumerate().map(|(i, t)| t + gaps.get(i).copied().unwrap_or(0.0)).collect();
    }

    /// The last block whose top is at or above 21 points under the viewport's top, at
    /// `offset`.
    pub fn top(&self, offset: f32) -> usize {
        let y = offset + 21.0;
        self.tops.partition_point(|&t| t <= y).saturating_sub(1)
    }

    /// The outline row current at `offset`: the last heading whose top is above the viewport's
    /// top plus 100 points.
    pub fn head_row(&self, offset: f32) -> Option<usize> {
        let y = offset + 100.0;
        self.heads.iter().rev().find(|(k, _)| self.tops.get(*k).is_some_and(|&t| t <= y)).map_or(self.first_row, |(_, row)| *row)
    }

    /// The outline highlight and the reading progress line (RD-38) at the scroll `now`
    /// ([offset, max, height, width]), without a view run (VER-32).
    pub fn follow(&mut self, now: [f32; 4], bar: &Live<[f32; 4]>) {
        let k = if self.reading {
            self.mark(now[0]);
            self.progress(now)
        } else if now[1] > 0.0 {
            (now[0] / now[1]).clamp(0.0, 1.0)
        } else {
            0.0
        };
        bar.set([0.0, 0.0, now[3] * k.clamp(0.0, 1.0), 2.0]);
    }

    /// Moves the outline's highlight to the row current at `offset`, without a view run.
    pub fn mark(&mut self, offset: f32) {
        let row = self.head_row(offset);
        if row == self.marked {
            return;
        }
        for (n, m) in self.marks.iter().enumerate() {
            let [fill, ink] = if Some(n) == row { m.on } else { m.off };
            m.fill.set(fill);
            m.ink.set(ink);
        }
        self.marked = row;
    }

    /// The reading progress (RD-38) at the scroll `[offset, max, height, _]`.
    pub fn progress(&self, [offset, max, vh, _]: [f32; 4]) -> f32 {
        let line = self.starts.get(self.top(offset)).copied().unwrap_or(0);
        progress(line, self.lines, offset, max, vh)
    }
}

pub struct Find {
    pub open: bool,
    pub q: TextEdit,
    /// The current match, an index into `blocks::find_all`.
    pub cur: usize,
}

// ---------- The numbers ----------

/// Three body lines (RD-20): round(B × 1.6 × 3), 67 px at 14 px.
pub fn line_step(b: f32) -> f32 {
    r(b * 1.6 * 3.0)
}

/// A view less 80 px of context, at least a line step (RD-21).
pub fn page_step(b: f32, vh: f32) -> f32 {
    line_step(b).max(vh - 80.0)
}

/// Where a key scroll starts: where the running glide is heading, else where the view is
/// (RD-22).
pub fn glide_from(offset: f32, glide: Option<(f32, f64)>, now: f64) -> f32 {
    match glide {
        Some((to, at)) if now - at < KEY_GLIDE => to,
        _ => offset,
    }
}

/// Reading progress (RD-38): the top block's source line over the line count, eased the rest
/// of the way to 1 over the last screen. 0 at the very top.
pub fn progress(top_line: usize, lines: usize, offset: f32, max: f32, vh: f32) -> f32 {
    if offset <= 0.5 || lines == 0 {
        return 0.0;
    }
    if max - offset <= 0.5 {
        return 1.0;
    }
    let base = (top_line as f32 / lines as f32).clamp(0.0, 1.0);
    let left = max - offset;
    if vh > 0.0 && left < vh { base + (1.0 - base) * (1.0 - left / vh) } else { base }
}

/// The first match at or below the top block, else the first (RD-57).
pub fn first_match(matches: &[Match], top: usize) -> usize {
    matches.iter().position(|m| m.block >= top).unwrap_or(0)
}

/// The next or previous of `n`, wrapping at both ends (RD-58).
pub fn step(cur: usize, n: usize, forward: bool) -> usize {
    if n == 0 {
        0
    } else if forward {
        (cur + 1) % n
    } else {
        (cur + n - 1) % n
    }
}

/// Where a link in the page goes (RD-62 to RD-66).
#[derive(Debug, PartialEq)]
pub enum Target {
    /// A heading or footnote in this note.
    Anchor(String),
    /// A web page or mail address, for the system.
    Web(String),
    /// A Markdown file: a note path relative to the notes folder when it is in it, with the
    /// fragment.
    Note(String, Option<String>),
    /// Any other local file, by absolute path, for the system's default app.
    File(std::path::PathBuf),
    /// A scheme the reader does not hand on.
    None,
}

const MD_EXT: [&str; 5] = ["md", "markdown", "mdown", "mkd", "mdx"];

/// `url`, from a note at `note` (relative to `root`).
pub fn target(url: &str, root: &std::path::Path, note: &str) -> Target {
    let url = url.trim();
    if let Some(f) = url.strip_prefix('#') {
        return Target::Anchor(f.to_string());
    }
    if let Some((scheme, _)) = url.split_once(':')
        && scheme.len() > 1
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    {
        let s = scheme.to_ascii_lowercase();
        return if matches!(s.as_str(), "http" | "https" | "mailto" | "ftp") { Target::Web(url.to_string()) } else { Target::None };
    }
    let (path, frag) = match url.split_once('#') {
        Some((p, f)) => (p, Some(f.to_string()).filter(|f| !f.is_empty())),
        None => (url, None),
    };
    let path = crate::md::percent_decode(path);
    if path.is_empty() {
        return frag.map_or(Target::None, Target::Anchor);
    }
    let abs = if let Some(rest) = path.strip_prefix("~/") {
        std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join(rest)
    } else if path.starts_with('/') {
        std::path::PathBuf::from(&path)
    } else {
        root.join(crate::md::dir(note)).join(&path)
    };
    let abs = normalize(&abs);
    let is_md = abs.extension().and_then(|e| e.to_str()).is_some_and(|e| MD_EXT.iter().any(|m| m.eq_ignore_ascii_case(e)));
    match abs.strip_prefix(normalize(root)) {
        Ok(rel) if is_md => {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel == note { frag.map_or(Target::None, Target::Anchor) } else { Target::Note(rel, frag) }
        }
        _ => Target::File(abs),
    }
}

/// `a/b/../c` as `a/c`, without touching the disk.
fn normalize(p: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// The source lines `lines` of `text`, as written.
pub fn source_lines(text: &str, lines: std::ops::Range<usize>) -> String {
    text.split('\n').skip(lines.start).take(lines.len()).collect::<Vec<_>>().join("\n")
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ---------- What the keys and the page do ----------

impl App {
    /// The active note's blocks, parsed once per text.
    pub fn doc(&self) -> Rc<Doc> {
        let text = self.active_text();
        let h = hash(text);
        let mut cache = self.reader.doc.borrow_mut();
        if let Some((k, d)) = &*cache
            && *k == h
        {
            return d.clone();
        }
        let d = Rc::new(blocks::build(text));
        *cache = Some((h, d.clone()));
        d
    }

    /// The hash of the text `doc` was parsed from: it names the blocks' version.
    pub fn doc_hash(&self) -> Option<u64> {
        self.reader.doc.borrow().as_ref().map(|(h, _)| *h)
    }

    /// The index of the top-level block at the top of the view.
    pub fn top(&self) -> usize {
        self.reader.map.borrow().top(self.reader.scroll.get()[0])
    }

    pub fn wide(&self) -> bool {
        self.reader.wide.unwrap_or(self.settings().width != "readable")
    }

    pub fn toggle_wide(&mut self) {
        self.reader.wide = Some(!self.wide());
    }

    fn b(&self) -> f32 {
        self.settings().size as f32
    }

    /// A key scroll by `dy`, gliding from where the last one is heading (RD-20 to RD-22).
    pub fn scroll_by(&mut self, dy: f32, tx: &mut Tx) {
        let [offset, max, ..] = self.reader.scroll.get();
        let now = self.now.get();
        let to = (glide_from(offset, self.reader.glide, now) + dy).clamp(0.0, max.max(0.0));
        self.reader.glide = Some((to, now));
        tx.scroll_to_with(SCROLL, 0.0, to, Spring::new(0.16, 0.0));
    }

    pub fn scroll_lines(&mut self, down: bool, tx: &mut Tx) {
        let d = line_step(self.b());
        self.scroll_by(if down { d } else { -d }, tx);
    }

    pub fn scroll_page(&mut self, down: bool, tx: &mut Tx) {
        let d = page_step(self.b(), self.reader.scroll.get()[2]);
        self.scroll_by(if down { d } else { -d }, tx);
    }

    /// A long trip (RD-24): past 1.5 views it cuts to 0.8 of a view short, then glides.
    pub fn travel(&mut self, to: f32, tx: &mut Tx) {
        let [offset, max, vh, _] = self.reader.scroll.get();
        let to = to.clamp(0.0, max.max(0.0));
        self.reader.glide = None;
        if (to - offset).abs() > 1.5 * vh {
            tx.scroll_to_instant(SCROLL, 0.0, if to > offset { to - 0.8 * vh } else { to + 0.8 * vh });
        }
        tx.scroll_to_with(SCROLL, 0.0, to, Spring::new(0.24, 0.0));
    }

    pub fn scroll_top(&mut self, tx: &mut Tx) {
        self.travel(0.0, tx);
    }

    pub fn scroll_bottom(&mut self, tx: &mut Tx) {
        let max = self.reader.scroll.get()[1];
        self.travel(max, tx);
    }

    /// The block whose first line is `line`, landed 20 px under the top (RD-25).
    pub fn jump_block(&mut self, line: usize, tx: &mut Tx) {
        let doc = self.doc();
        if let Some(i) = doc.blocks.iter().rposition(|b| b.lines.start <= line) {
            self.land(i, tx);
        }
    }

    /// Scrolls block `i` to 20 px under the top. Its row starts at the gap above it, so the row
    /// lands that much higher.
    fn land(&self, i: usize, tx: &mut Tx) {
        let doc = self.doc();
        if let Some(b) = doc.blocks.get(i) {
            tx.scroll_key_to(block_key(b), 20.0 - self.reader.map.borrow().gap(i));
        }
    }

    /// Centres the current find match. It names its block's row first, so a match in a row
    /// not yet built is reached too.
    fn center_match(&self, tx: &mut Tx) {
        let m = self.find_matches();
        let doc = self.doc();
        if let Some(b) = m.get(self.reader.find.cur).and_then(|m| doc.blocks.get(m.block)) {
            tx.scroll_center(format!("{}/{FIND_CUR}", block_key(b)));
        }
    }

    /// The source lines of the block under the pointer, else the top block (RD-69, RD-100).
    fn block_here(&self) -> Option<std::ops::Range<usize>> {
        let doc = self.doc();
        let b = match self.reader.hover {
            Some(l) => doc.blocks.iter().find(|b| b.lines.start == l),
            None => doc.blocks.get(self.top()),
        }?;
        Some(b.lines.clone())
    }

    pub fn copy_block(&mut self, line: usize, tx: &mut Tx) {
        let doc = self.doc();
        if let Some(b) = doc.blocks.iter().find(|b| b.lines.start == line) {
            tx.copy(source_lines(self.active_text(), b.lines.clone()));
            self.toast("Copied the block's Markdown", tx);
        }
    }

    pub fn copy_block_here(&mut self, tx: &mut Tx) {
        if let Some(l) = self.block_here() {
            self.copy_block(l.start, tx);
        }
    }

    /// The heading `#`: the note's path in the folder and the slug (RD-71, BLK-16).
    pub fn copy_heading_link(&mut self, slug: &str, tx: &mut Tx) {
        let t = format!("{}#{slug}", self.active.clone().unwrap_or_default());
        tx.copy(t.clone());
        self.toast(&format!("Copied {t}"), tx);
    }

    /// A code block's Copy chip (BLK-51).
    pub fn copy_code(&mut self, code: &str, line: usize, tx: &mut Tx) {
        tx.copy(code.to_string());
        self.reader.copied = Some((line, self.now.get()));
        tx.wake_after(std::time::Duration::from_secs_f64(COPIED + 0.05));
        self.toast("Copied", tx);
    }

    pub fn copy_all(&mut self, tx: &mut Tx) {
        tx.copy(self.active_text().to_string());
        self.toast("Copied the whole file", tx);
    }

    pub fn copy_selection(&mut self, tx: &mut Tx) {
        if let Some(t) = tx.text_selection().map(|s| s.text().to_string()).filter(|t| !t.is_empty()) {
            tx.copy(t);
            self.toast("Copied", tx);
        }
    }

    pub fn toast(&mut self, msg: &str, tx: &mut Tx) {
        self.reader.toast = Some((msg.to_string(), self.now.get()));
        tx.wake_after(std::time::Duration::from_secs_f64(TOAST + 0.05));
    }

    /// Clears a toast or mark whose time is up. True when something went.
    pub(crate) fn expire_reader(&mut self, t: f64) -> bool {
        let mut changed = false;
        if self.reader.toast.as_ref().is_some_and(|(_, at)| t - at >= TOAST) {
            self.reader.toast = None;
            changed = true;
        }
        if self.reader.copied.is_some_and(|(_, at)| t - at >= COPIED) {
            self.reader.copied = None;
            changed = true;
        }
        if self.reader.reloaded.is_some_and(|at| t - at >= RELOADED) {
            self.reader.reloaded = None;
            changed = true;
        }
        if self.reader.pending.as_ref().is_some_and(|(_, at)| t - at >= SEQ) {
            self.reader.pending = None;
        }
        changed
    }

    /// An outside change reached note `rel` (RD-14).
    pub(crate) fn note_reloaded(&mut self, rel: &str) {
        if self.active.as_deref() == Some(rel) {
            self.reader.reloaded = Some(self.now.get());
        }
    }

    /// Ctrl+R: read the note from disk now, through the watcher's path (RD-18).
    pub fn read_from_disk(&mut self) {
        let (Some(a), Some(v), Some(w)) = (self.active.clone(), &self.vault, &self.watcher) else { return };
        if self.dirty.contains(&a) {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(v.path.join(&a))
            && let Ok(mut inbox) = w.inbox.lock()
        {
            inbox.changed.push((a, text));
        }
    }

    // ---------- Find ----------

    /// Ctrl+F or `/` (RD-52): open the bar, or select its text when it is open.
    pub fn open_find(&mut self, tx: &mut Tx) {
        let f = &mut self.reader.find;
        f.open = true;
        let n = f.q.text.len();
        f.q.set_selection(0, n);
        tx.focus("find-input", false);
    }

    pub fn close_find(&mut self, tx: &mut Tx) {
        let f = &mut self.reader.find;
        if f.open {
            f.open = false;
            f.q = TextEdit::new("");
            f.cur = 0;
            tx.blur();
        }
    }

    pub fn find_query(&self) -> Option<&str> {
        let f = &self.reader.find;
        Some(f.q.text.as_str()).filter(|q| f.open && !q.is_empty())
    }

    /// How many matches the find bar counts, in either mode.
    pub fn find_count(&self) -> usize {
        if self.mode == crate::app::Mode::Source { self.source_matches().len() } else { self.find_matches().len() }
    }

    /// The matches of the find query in the open note.
    pub fn find_matches(&self) -> Vec<Match> {
        self.find_query().map_or_else(Vec::new, |q| blocks::find_all(&self.doc(), q))
    }

    /// The query changed: pick the first match from the top of the view (RD-57).
    pub fn find_changed(&mut self, tx: &mut Tx) {
        if self.mode == crate::app::Mode::Source {
            return self.source_found(tx);
        }
        let m = self.find_matches();
        let top = self.top();
        self.reader.find.cur = first_match(&m, top);
        self.center_match(tx);
    }

    /// Enter / n and Shift+Enter / N (RD-58).
    pub fn find_step(&mut self, forward: bool, tx: &mut Tx) {
        if self.mode == crate::app::Mode::Source {
            return self.source_step(forward, tx);
        }
        let n = self.find_matches().len();
        let f = &mut self.reader.find;
        f.cur = step(f.cur, n, forward);
        self.center_match(tx);
    }

    /// Esc in the reader (RD-85): close find, else clear the selection, else Quire's Esc.
    pub fn reader_escape(&mut self, tx: &mut Tx) -> bool {
        if self.reader.help {
            self.reader.help = false;
        } else if self.reader.find.open {
            self.close_find(tx);
        } else if tx.text_selection().is_some() {
            tx.clear_text_selection();
        } else if !self.escape(tx) {
            // Nothing else takes Esc: back to the writer (WR-111).
            if self.active.is_none() {
                return false;
            }
            self.toggle_mode(tx);
        }
        true
    }

    // ---------- Tabs and links ----------

    /// Keeps the reading position of the note being left and gives the one being shown its
    /// own (RD-103). Find closes.
    pub(crate) fn switch_reading(&mut self, from: Option<&str>, to: &str, tx: &mut Tx) {
        if let Some(f) = from {
            self.reader.positions.insert(f.to_string(), self.reader.scroll.get()[0]);
        }
        self.close_find(tx);
        self.reader.hover = None;
        self.reader.glide = None;
        let y = if self.mode == Mode::Preview { self.reader.positions.get(to).copied().unwrap_or(0.0) } else { 0.0 };
        tx.scroll_to_instant(SCROLL, 0.0, y);
    }

    /// A link in the page (RD-62 to RD-66).
    pub fn follow(&mut self, url: &str, tx: &mut Tx) {
        let (Some(note), Some(root)) = (self.active.clone(), self.vault.as_ref().map(|v| v.path.clone())) else { return };
        match target(url, &root, &note) {
            Target::Anchor(a) => self.go_anchor(&a, tx),
            Target::Web(u) => {
                if let Err(e) = tx.open_url(&u) {
                    self.notice = Some(format!("Could not open {u}: {e}"));
                }
            }
            Target::Note(rel, frag) if self.files().contains_key(&rel) => {
                self.open_file(&rel, None, tx);
                if let Some(f) = frag {
                    self.go_anchor(&f, tx);
                }
            }
            Target::Note(rel, _) => self.notice = Some(format!("No note at {rel}")),
            Target::File(p) => {
                if let Err(e) = tx.open_url(&format!("file://{}", p.display())) {
                    self.notice = Some(format!("Could not open {}: {e}", p.display()));
                }
            }
            Target::None => {}
        }
    }

    /// A heading slug or footnote anchor in the open note (RD-04, RD-62).
    pub fn go_anchor(&mut self, anchor: &str, tx: &mut Tx) {
        let doc = self.doc();
        let a = crate::md::percent_decode(anchor);
        let i = doc.anchors.get(a.as_str()).or_else(|| doc.anchors.get(blocks::slugify(&a).as_str()));
        if let Some(&i) = i {
            self.land(i, tx);
        }
    }
}

/// A top-level block's element key: its content key, so the page holds its place when lines
/// above it change (RD-11).
pub fn block_key(b: &blocks::Block) -> String {
    format!("pb-{:x}-{}", b.key.0, b.key.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rd_20_21_steps() {
        assert_eq!(line_step(14.0), 67.0);
        assert_eq!(page_step(14.0, 700.0), 620.0);
        assert_eq!(page_step(14.0, 100.0), 67.0);
    }

    #[test]
    fn rd_22_a_key_inside_a_glide_continues_from_its_target() {
        assert_eq!(glide_from(10.0, Some((77.0, 1.0)), 1.05), 77.0);
        assert_eq!(glide_from(10.0, Some((77.0, 1.0)), 1.2), 10.0);
        assert_eq!(glide_from(10.0, None, 1.0), 10.0);
    }

    #[test]
    fn rd_38_progress() {
        assert_eq!(progress(40, 100, 0.0, 1000.0, 500.0), 0.0);
        assert_eq!(progress(40, 100, 1000.0, 1000.0, 500.0), 1.0);
        assert_eq!(progress(40, 100, 200.0, 1000.0, 500.0), 0.4);
        // Over the last screen it eases toward 1: half a view left is half way.
        assert!((progress(40, 100, 750.0, 1000.0, 500.0) - 0.7).abs() < 1e-5);
    }

    #[test]
    fn rd_57_58_find_order() {
        let m = |block| Match { pid: 0, block, k: 0 };
        let ms = [m(1), m(3), m(3), m(7)];
        assert_eq!(first_match(&ms, 2), 1);
        assert_eq!(first_match(&ms, 9), 0);
        assert_eq!((step(3, 4, true), step(0, 4, false), step(0, 0, true)), (0, 3, 0));
    }

    #[test]
    fn rd_62_to_66_link_targets() {
        let root = Path::new("/notes");
        assert_eq!(target("#intro", root, "a/b.md"), Target::Anchor("intro".into()));
        assert_eq!(target("https://x.org", root, "b.md"), Target::Web("https://x.org".into()));
        assert_eq!(target("MAILTO:me@x.org", root, "b.md"), Target::Web("MAILTO:me@x.org".into()));
        assert_eq!(target("javascript:alert(1)", root, "b.md"), Target::None);
        assert_eq!(target("../c.MD#Two", root, "a/b.md"), Target::Note("c.MD".into(), Some("Two".into())));
        assert_eq!(target("d%20e.markdown", root, "a/b.md"), Target::Note("a/d e.markdown".into(), None));
        assert_eq!(target("c.md#x", root, "a/b.md"), Target::Note("a/c.md".into(), Some("x".into())), "relative to the note's folder");
        assert_eq!(target("b.md#x", root, "a/b.md"), Target::Anchor("x".into()), "the same note by name");
        assert_eq!(target("./b.md#x", root, "b.md"), Target::Anchor("x".into()), "the same note");
        assert_eq!(target("../../out.md", root, "b.md"), Target::File("/out.md".into()), "outside the folder");
        assert_eq!(target("pic.png", root, "a/b.md"), Target::File("/notes/a/pic.png".into()));
    }

    #[test]
    fn rd_68_source_lines() {
        assert_eq!(source_lines("a\nb\nc\nd", 1..3), "b\nc");
    }
}
