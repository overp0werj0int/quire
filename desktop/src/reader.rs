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
/// The help sheet's scroll viewport.
pub const HELP_SCROLL: &str = "help-scroll";
pub const HELP_SEARCH: &str = "help-search";
/// The current find match's highlight, which scrolling centres.
pub const FIND_CUR: &str = "find-cur";
/// The key of the current match's second highlight while it waits to be placed above a band.
pub fn place_key(n: u32) -> String {
    format!("find-place-{n}")
}
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
    pub toast: Option<(String, f64)>,
    /// The code block whose chip says Copied, by its first source line, and since when.
    pub copied: Option<(usize, f64)>,
    /// When an outside change last reached the open note (RD-14).
    pub reloaded: Option<f64>,
    /// The first source line of the top-level block under the pointer (RD-100).
    pub hover: Option<usize>,
    /// The block menu a right-click opened (RD-68): where, and its block's first line.
    /// Where the page's blocks sit, from the last view. Shared with the scroll handler, which
    /// draws the reading progress from it without running the view (VER-32).
    pub map: Rc<RefCell<PageMap>>,
    /// The viewport's offset, scroll range, height and width, from its scroll handler.
    pub scroll: Live<[f32; 4]>,
    /// The help sheet's scroll viewport state, synchronized between wheel/touchpad scrolling
    /// (`on_scroll_offset`) and keyboard scrolling (`App::scroll_help`).
    pub help_scroll: Rc<std::cell::Cell<HelpScroll>>,
    /// Unified document reveal controller: toast/notice band lifts, hand-control tracking,
    /// deferred caret reveals, end-room release, and find-match placement (`crate::reveal`).
    pub reveal: crate::reveal::Reveal,
    /// The progress line, [x, y, w, h], set with the scroll in the frame it draws (RD-38, WR-178).
    pub bar: Live<[f32; 4]>,
    /// The writer's scroll thumb, [x, y, w, h], and its opacity, pulsed by each scroll (WR-93).
    pub thumb: Live<[f32; 4]>,
    pub thumb_shown: Live<f32>,
    /// The first key of a two-key chord and when it came.
    pub(crate) pending: Option<(KeyEvent, f64)>,
    /// Where the last key glide is heading and when it started.
    glide: Option<(f32, f64)>,
    /// Each tab's reading position (RD-103).
    positions: HashMap<String, f32>,
    /// The active note's blocks, kept until its `TextKey` changes, with a monotonic version id.
    doc: RefCell<Option<(crate::app::TextKey, u64, Rc<Doc>)>>,
    doc_ver: std::cell::Cell<u64>,
}

impl Default for Reader {
    fn default() -> Self {
        Reader {
            wide: None,
            find: Find { open: false, q: TextEdit::new(""), searched: String::new(), due: None, cur: 0, case_sensitive: false, whole_word: false, replace_open: false, replace: TextEdit::new("") },
            toast: None,
            copied: None,
            reloaded: None,
            hover: None,
            map: Rc::default(),
            scroll: Live::new([0.0; 4]),
            help_scroll: Rc::new(std::cell::Cell::new(HelpScroll::default())),
            reveal: crate::reveal::Reveal::default(),
            bar: Live::new([0.0; 4]),
            thumb: Live::new([0.0; 4]),
            thumb_shown: Live::new(0.0),
            pending: None,
            glide: None,
            positions: HashMap::new(),
            doc: RefCell::new(None),
            doc_ver: std::cell::Cell::new(0),
        }
    }
}

/// Live scroll state of the help sheet's viewport (`HELP_SCROLL`), shared between its
/// `on_scroll_offset` callback and `App::scroll_help` so wheel/touchpad scrolling and keyboard
/// navigation stay in sync.
#[derive(Clone, Copy, Default)]
pub struct HelpScroll {
    /// `[offset, max, viewport_h, viewport_w]` as last reported by `help-scroll`.
    pub pos: [f32; 4],
    /// Active keyboard glide target and clock time; cleared whenever a hand scrolls the sheet.
    pub glide: Option<(f32, f64)>,
    /// Clock time and velocity (pt/s) of the most recent hand-driven scroll frame.
    pub hand_at: f64,
    pub hand_vel: f32,
}

impl HelpScroll {
    pub fn on_offset(&mut self, next: [f32; 4], by_hand: bool, t: f64) {
        let dy = next[0] - self.pos[0];
        self.pos = next;
        if by_hand && dy.abs() > 0.01 {
            self.glide = None;
            let dt = (t - self.hand_at) as f32;
            self.hand_vel = if dt > 0.001 && dt < 0.12 { dy / dt } else { dy * 60.0 };
            self.hand_at = t;
        }
    }

    /// Starting offset for a relative keyboard step `delta` at clock time `now`.
    pub fn step_from(&self, delta: f32, now: f64) -> f32 {
        let offset = self.pos[0];
        let from = glide_from(offset, self.glide, now);
        if self.glide.is_some() && from != offset {
            return from;
        }
        // Preserve in-flight wheel momentum when stepping in the same direction so a rapid
        // wheel + key sequence never truncates the wheel ease and settles deeper.
        if now - self.hand_at < 0.08 && self.hand_vel * delta > 0.0 {
            return offset + (self.hand_vel * 0.06).clamp(-120.0, 120.0);
        }
        offset
    }
}

/// Where each top-level block of the page sits in the scroll viewport's content, so the block at
/// the top of the view follows from the scroll offset alone. Quire keeps this itself, so it can
/// mark the block you are in. The page is a virtual list: each row laid out reports its height
/// and where it is (`El::on_layout`), and a row not yet laid out counts the list's estimate.
/// The rows add up to where each block would be, and the last row laid out says how far that is
/// from where it is, so blocks near the view are exact and blocks far from it are estimates
/// that a trip to them corrects as it arrives.
#[derive(Default)]
pub struct PageMap {
    /// The reader page is on screen.
    pub reading: bool,
    /// The writer source editor is on screen (unlocked).
    pub writing: bool,
    /// The note the rows are for, by its text's hash.
    note: Option<u64>,
    /// Last seen `(doc_hash, caret_byte)` in source mode so caret moves update the outline
    /// while hand scrolling owns the outline until the caret moves again.
    pub source_caret: Option<(Option<u64>, usize)>,
    /// True after a source-mode caret move until a hand scroll takes over.
    pub source_from_caret: bool,
    /// Each block's row key and its index, and each laid-out row's height by key.
    keys: Vec<String>,
    index: HashMap<String, usize>,
    heights: HashMap<String, f32>,
    /// The height the list gives a row not yet laid out.
    estimate: f32,
    /// The gap above each block, from the view, and the body size bits those gaps came from.
    gaps: Rc<Vec<f32>>,
    gaps_size: u32,
    /// Which `note` version `starts` currently reflects.
    starts_note: Option<u64>,
    /// Each block's top with the rows added up from 0, and how far the rows really are from that.
    base: Vec<f32>,
    shift: f32,
    /// The scroll content's top in the window, as last laid out.
    content_y: f32,
    /// The rows' left edge in the window, as last laid out.
    left: f32,
    /// Each top-level block's first source line, and the note's line count.
    pub starts: Vec<usize>,
    pub lines: usize,
    /// The writer's text field in the window as last laid out: its top and its height.
    field: [f32; 2],
    /// The outline panel's rows, and the entries of the shelf's outline under the active tab
    /// (SB-29), when they show.
    pub outline: Marks,
    pub shelf: Marks,
    /// Bumped whenever `outline` is cleared outside its memoized builder so the memo rebuilds
    /// fresh `Live` colour handles (`LIVEPACE`).
    pub outline_gen: u64,
    /// A trip waiting for its row to be laid out, to land by where the row is.
    pending: Option<Pending>,
    /// A trip into a note whose page isn't mapped yet (a `[[note#Heading]]` link): the block's
    /// key, whether it travels, and when it was asked, on the clock.
    later: Option<(String, bool, f64)>,
}

/// A trip to a row that was not laid out when it started (RD-24, RD-25, RD-60).
struct Pending {
    key: String,
    how: Arrive,
    /// When it started, on the clock: it gives up after `PENDING` seconds.
    since: f64,
}

enum Arrive {
    /// Put the row `at` points under the view's top, at once.
    Jump { at: f32 },
    /// A long trip's cut, placed by the map's estimate: once the row is laid out, anchor the cut
    /// to 0.8 of a view short of the row and glide the rest.
    Cut { gap: f32 },
    /// Glide the block's top to 20 points under the view's top; `to` is the offset asked for
    /// last.
    Glide { to: f32 },
}

/// What a row laid out asks of the scroll viewport.
pub enum Arrival {
    Key(f32),
    /// Anchor the row `at` points under the view's top, then glide to `to`.
    CutKey(f32, f32),
    Glide(f32),
}

/// How long a trip waits for its row, in seconds.
const PENDING: f64 = 0.6;

/// An outline's rows: each heading's block and its row, the row current above the first, each
/// row's colours, and the row marked now.
#[derive(Default)]
pub struct Marks {
    pub heads: Vec<(usize, Option<usize>)>,
    pub first_row: Option<usize>,
    pub marks: Vec<Mark>,
    pub marked: Option<usize>,
}

impl Marks {
    pub fn clear(&mut self) {
        self.heads.clear();
        self.marks.clear();
        self.marked = None;
    }

    /// Marks `row`, and no other, through the rows' live colours.
    fn mark(&mut self, row: Option<usize>) {
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
}

/// An outline row's live colours, [fill, text], and their values when current and not.
pub struct Mark {
    pub fill: Live<Color>,
    pub ink: Live<Color>,
    /// Fill and ink when the row is current, and when it is not.
    pub on: [Color; 2],
    pub off: [Color; 2],
}

impl PageMap {
    /// The note's blocks: their row keys (built only when the note changes), the gap above
    /// each (built only when the note or body size changes), and the list's estimate for a row
    /// not yet laid out. A key that leaves the page forgets its height, as the list does.
    pub fn set_rows(&mut self, note: Option<u64>, body_size: f32, keys: impl FnOnce() -> Vec<String>, gaps: impl FnOnce() -> Vec<f32>, estimate: f32) {
        let mut changed = false;
        let note_changed = note != self.note || self.note.is_none();
        if note_changed {
            self.note = note;
            self.keys = keys();
            self.index = self.keys.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
            let index = &self.index;
            self.heights.retain(|k, _| index.contains_key(k));
            changed = true;
        }
        let size_bits = body_size.to_bits();
        if note_changed || size_bits != self.gaps_size || estimate != self.estimate {
            self.gaps = Rc::new(gaps());
            self.gaps_size = size_bits;
            self.estimate = estimate;
            changed = true;
        }
        if changed {
            self.join();
        }
    }

    /// Cached per-block top gaps computed for the current note and body size (`COLDPACE`).
    pub fn gaps(&self) -> Rc<Vec<f32>> {
        self.gaps.clone()
    }

    /// Updates `starts` only when `note` changed (`LIVEPACE`).
    pub fn set_starts(&mut self, note: Option<u64>, blocks: &[crate::blocks::Block]) {
        if note != self.starts_note || self.starts_note.is_none() {
            self.starts_note = note;
            self.starts.clear();
            self.starts.extend(blocks.iter().map(|b| b.lines.start));
        }
    }

    /// Clears `starts` when leaving preview mode.
    pub fn clear_starts(&mut self) {
        self.starts_note = None;
        self.starts.clear();
    }

    /// Clears the outline's live marks and bumps `outline_gen` so the memoized outline rebuilds
    /// fresh live handles on next mount.
    pub fn clear_outline(&mut self) {
        if !self.outline.marks.is_empty() || !self.outline.heads.is_empty() {
            self.outline.clear();
            self.outline_gen = self.outline_gen.wrapping_add(1);
        }
    }

    /// The scroll content laid out with its top at window `y`.
    pub fn content_laid_out(&mut self, y: f32) {
        self.content_y = y;
    }

    /// The writer's text field laid out at window `y`, `h` points tall. Whether it moved.
    pub fn field_laid_out(&mut self, y: f32, h: f32) -> bool {
        let moved = self.field != [y, h];
        self.field = [y, h];
        moved
    }

    /// Row `key` laid out at window `x`, `y`, `h` points tall. Whether that moved any block.
    pub fn row_laid_out(&mut self, key: &str, x: f32, y: f32, h: f32) -> bool {
        let Some(&i) = self.index.get(key) else { return false };
        self.left = x;
        let mut moved = false;
        if self.heights.get(key) != Some(&h) {
            self.heights.insert(key.to_owned(), h);
            self.join();
            moved = true;
        }
        let row = self.base[i] - self.gaps.get(i).copied().unwrap_or(0.0);
        let shift = y - self.content_y - row;
        if (shift - self.shift).abs() > 0.25 {
            self.shift = shift;
            moved = true;
        }
        moved
    }

    fn join(&mut self) {
        let mut y = 0.0;
        self.base.clear();
        for (i, k) in self.keys.iter().enumerate() {
            self.base.push(y + self.gaps.get(i).copied().unwrap_or(0.0));
            y += self.heights.get(k).copied().unwrap_or(self.estimate);
        }
    }

    /// Block `i`'s top in the scroll content.
    pub fn block_top(&self, i: usize) -> Option<f32> {
        self.base.get(i).map(|t| t + self.shift)
    }

    /// Where the keyboard opens block `i`'s menu, in the window, with the page scrolled to
    /// `offset`: at the text's left edge, below the block's first line still in view.
    pub fn menu_at(&self, i: usize, offset: f32, line: f32) -> Option<(f32, f32)> {
        let top = self.block_top(i)?.max(offset);
        Some((self.left, self.content_y + top + line))
    }

    /// The map covers `blocks` blocks: right after a switch it may still hold the last note's
    /// page (RD-04).
    pub fn covers(&self, blocks: usize) -> bool {
        self.base.len() == blocks
    }

    /// Block `i`'s height, its gap above left out.
    fn height(&self, i: usize) -> f32 {
        let row = self.keys.get(i).and_then(|k| self.heights.get(k)).copied().unwrap_or(self.estimate);
        row - self.gaps.get(i).copied().unwrap_or(0.0)
    }

    /// Waits for row `key` to be laid out, then arrives `how`.
    fn wait(&mut self, key: String, how: Arrive, now: f64) {
        self.pending = Some(Pending { key, how, since: now });
    }

    /// Row `key` laid out with the view at `[offset, max, ..]`: what a trip waiting for it asks.
    pub fn arrive(&mut self, key: &str, [offset, max, vh, _]: [f32; 4], now: f64) -> Option<Arrival> {
        let p = self.pending.as_ref()?;
        if now - p.since > PENDING {
            self.pending = None;
            return None;
        }
        if p.key != key {
            return None;
        }
        let top = self.index.get(key).and_then(|&i| self.block_top(i))?;
        let p = self.pending.as_mut()?;
        match &mut p.how {
            Arrive::Jump { at } => {
                let at = *at;
                self.pending = None;
                Some(Arrival::Key(at))
            }
            Arrive::Cut { gap } => {
                let gap = *gap;
                let want = (top - 20.0).clamp(0.0, max.max(0.0));
                p.how = Arrive::Glide { to: want };
                if (want - offset).abs() > 0.8 * vh {
                    let at = 20.0 - gap + (want - offset).signum() * 0.8 * vh;
                    Some(Arrival::CutKey(at, want))
                } else {
                    Some(Arrival::Glide(want))
                }
            }
            Arrive::Glide { to } => {
                let want = (top - 20.0).clamp(0.0, max.max(0.0));
                if (want - *to).abs() < 0.5 {
                    self.pending = None;
                    return None;
                }
                *to = want;
                Some(Arrival::Glide(want))
            }
        }
    }

    /// The last block whose top is at or above 21 points under the viewport's top, at
    /// `offset`.
    pub fn top(&self, offset: f32) -> usize {
        let y = offset + 21.0;
        self.base.partition_point(|&t| t + self.shift <= y).saturating_sub(1)
    }

    /// The scan line in scroll-content coordinates for outline selection at `[offset, max, vh]`:
    /// 100 pt below the viewport top while more than one screen remains, sweeping smoothly to
    /// the viewport bottom over the final screen so headings near the end of the note are reached.
    fn scan_y(offset: f32, max: f32, vh: f32) -> f32 {
        if max <= 0.5 || offset <= 0.5 {
            return offset + 100.0;
        }
        if max - offset <= 1.0 {
            return offset + vh;
        }
        let last = vh.min(max);
        let left = max - offset;
        if last > 0.0 && left < last {
            let t = (1.0 - left / last).clamp(0.0, 1.0);
            offset + 100.0 + (vh - 100.0).max(0.0) * t
        } else {
            offset + 100.0
        }
    }

    /// Heading `key`'s top in scroll-content coordinates: block index in preview, source line
    /// index in source mode.
    fn heading_y(&self, key: usize) -> Option<f32> {
        if self.reading {
            self.block_top(key)
        } else if self.writing {
            let [fy, fh] = self.field;
            if fh <= 0.0 || self.lines == 0 {
                return None;
            }
            let top = fy - self.content_y;
            Some(top + (key as f32 / self.lines as f32) * fh)
        } else {
            None
        }
    }

    /// The row of outline `m` current at scroll `now` (`[offset, max, vh, _]`): the last heading
    /// whose top is at or above `scan_y`.
    pub fn head_row(&self, m: &Marks, [offset, max, vh, _]: [f32; 4]) -> Option<usize> {
        let y = if self.reading {
            Self::scan_y(offset, max, vh)
        } else if self.writing {
            let [fy, fh] = self.field;
            if fh <= 0.0 || self.lines == 0 {
                return m.first_row;
            }
            let top = fy - self.content_y;
            let end = (top + fh - vh).clamp(0.0, max.max(0.0));
            if end > 0.5 && offset >= end - 1.0 {
                top + fh
            } else {
                let eff_max = if end > 0.5 { end } else { max };
                Self::scan_y(offset.min(eff_max), eff_max, vh)
            }
        } else {
            return m.first_row;
        };
        m.heads.iter().rev().find(|(k, _)| self.heading_y(*k).is_some_and(|t| t <= y)).map_or(m.first_row, |(_, row)| *row)
    }

    /// The row of outline `m` current at source `line` (when `m.heads` stores `(h.line, row)`).
    pub fn head_row_line(m: &Marks, line: usize) -> Option<usize> {
        m.heads.iter().rev().find(|(l, _)| *l <= line).map_or(m.first_row, |(_, row)| *row)
    }

    /// The outline highlight and the reading progress line (RD-38) at the scroll `now`
    /// ([offset, max, height, width]), without a view run (VER-32).
    pub fn follow(&mut self, now: [f32; 4], by_hand: bool, bar: &Live<[f32; 4]>) {
        let k = if self.reading {
            self.mark(now);
            self.progress(now)
        } else {
            if self.writing && (by_hand || !self.source_from_caret) {
                if by_hand {
                    self.source_from_caret = false;
                }
                self.mark(now);
            }
            self.source_progress(now)
        };
        bar.set([0.0, 0.0, now[3] * k.clamp(0.0, 1.0), 2.0]);
    }

    /// Moves both outlines' highlights to the rows current at scroll `now`, without a view run.
    pub fn mark(&mut self, now: [f32; 4]) {
        let rows = [self.head_row(&self.outline, now), self.head_row(&self.shelf, now)];
        self.outline.mark(rows[0]);
        self.shelf.mark(rows[1]);
    }

    /// Moves both outlines' highlights to the rows at source `line`, without a view run.
    pub fn mark_line(&mut self, line: usize) {
        let rows = [Self::head_row_line(&self.outline, line), Self::head_row_line(&self.shelf, line)];
        self.outline.mark(rows[0]);
        self.shelf.mark(rows[1]);
    }

    /// The reading progress (RD-38) at the scroll `[offset, max, height, _]`.
    pub fn progress(&self, [offset, max, vh, _]: [f32; 4]) -> f32 {
        let line = self.starts.get(self.top(offset)).copied().unwrap_or(0);
        progress(line, self.lines, offset, max, vh)
    }

    /// The writer's progress, in the reader's measure (WR-178, RD-38): the line at the view's
    /// top over the line count, full once the text's end is in view. The field reports no
    /// line positions, so the line is its share of the field's height.
    pub fn source_progress(&self, [offset, max, vh, _]: [f32; 4]) -> f32 {
        let [y, h] = self.field;
        let top = y - self.content_y;
        let end = (top + h - vh).clamp(0.0, max.max(0.0));
        let line = if h > 0.0 { (self.lines as f32 * ((offset + 21.0 - top) / h).clamp(0.0, 1.0)) as usize } else { 0 };
        progress(line, self.lines, offset, end, vh)
    }
}

pub struct Find {
    pub open: bool,
    pub q: TextEdit,
    /// The query the matches are for: the bar's text once the search has run (RD-56).
    pub searched: String,
    /// When the search runs, on the clock, while one waits.
    pub due: Option<f64>,
    /// The current match, an index into `blocks::find_all`.
    pub cur: usize,
    /// Source-mode case-sensitive (`Aa`) and whole-word (`W`) toggles.
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// Whether the Source-mode Replace row is expanded inside the find bar (`Ctrl+H`).
    pub replace_open: bool,
    pub replace: TextEdit,
}

/// Find bar result state, separating empty, debounced-pending, settled zero-match, and matched
/// queries so fast Tab never skips valid Preview matches during debounce (`FINDSTATE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindState {
    Empty,
    Pending,
    NoMatches,
    Matches(usize),
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
/// of the way to 1 over the last screen, or over the whole scroll of a page shorter than two
/// screens. 0 at the very top.
pub fn progress(top_line: usize, lines: usize, offset: f32, max: f32, vh: f32) -> f32 {
    if offset <= 0.5 || lines == 0 {
        return 0.0;
    }
    if max - offset <= 0.5 {
        return 1.0;
    }
    let base = (top_line as f32 / lines as f32).clamp(0.0, 1.0);
    // A page shorter than two views eases over all of its scroll, so its top still reads 0.
    let last = vh.min(max);
    let left = max - offset;
    if last > 0.0 && left < last { base + (1.0 - base) * (1.0 - left / last) } else { base }
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

impl App {
    /// The active note's blocks, parsed once per `TextKey` (`LIVEPACE`).
    pub fn doc(&self) -> Rc<Doc> {
        let key = self.active_text_key();
        let mut cache = self.reader.doc.borrow_mut();
        if let Some((k, _, d)) = &*cache
            && k.matches(key)
        {
            return d.clone();
        }
        let d = Rc::new(blocks::build(self.active_text()));
        let ver = self.reader.doc_ver.get().wrapping_add(1).max(1);
        self.reader.doc_ver.set(ver);
        *cache = Some((key.to_owned(), ver, d.clone()));
        d
    }

    /// The version of the text `doc` was parsed from: it names the blocks' version.
    pub fn doc_hash(&self) -> Option<u64> {
        self.reader.doc.borrow().as_ref().map(|(_, v, _)| *v)
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

    pub(crate) fn b(&self) -> f32 {
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

    /// Travels to the block whose first line is `line` (RD-24, RD-25, RD-49).
    pub fn jump_block(&mut self, line: usize, tx: &mut Tx) {
        if let Some(i) = self.block_at_line(line) {
            self.land(i, true, tx);
        }
    }

    /// The last block starting at or before `line`.
    pub(crate) fn block_at_line(&self, line: usize) -> Option<usize> {
        self.doc().blocks.iter().rposition(|b| b.lines.start <= line)
    }

    /// Puts block `i` 20 px under the top (RD-25), at once or, when `travel`, as a long jump
    /// travels (RD-24): past 1.5 views it cuts to 0.8 of a view short, then glides 240 ms. A row
    /// near the view is laid out, so its top in the page map is where it is. A row far from it
    /// may not be, so the trip starts from the map's estimate and lands again once the row lays
    /// out (`App::row_laid_out`): a long trip's cut is placed again a frame later, by the row.
    pub(crate) fn land(&mut self, i: usize, travel: bool, tx: &mut Tx) {
        self.unfold_to_block(i);
        let doc = self.doc();
        let Some(b) = doc.blocks.get(i) else { return };
        let gap = crate::page::gap(self.b(), i.checked_sub(1).map(|p| crate::page::rk(&doc.blocks[p].kind)), crate::page::rk(&b.kind));
        let key = block_key(b);
        self.reader.glide = None;
        let [offset, max, vh, _] = self.reader.scroll.get();
        let now = self.now.get();
        let mut map = self.reader.map.borrow_mut();
        // Right after a switch the map may still hold the last note's page (RD-04): the trip
        // starts once the new page's first rows are laid out.
        let Some(top) = map.block_top(i).filter(|_| map.covers(doc.blocks.len()) && map.index.contains_key(&key)) else {
            map.later = Some((key, travel, now));
            return;
        };
        let to = (top - 20.0).clamp(0.0, max.max(0.0));
        let near = top > offset - vh && top < offset + 2.0 * vh;
        if !travel {
            if near {
                return tx.scroll_key_to(key, 20.0 - gap);
            }
            tx.scroll_to_instant(SCROLL, 0.0, to);
            return map.wait(key, Arrive::Jump { at: 20.0 - gap }, now);
        }
        let far = (to - offset).abs() > 1.5 * vh;
        if far {
            tx.scroll_to_instant(SCROLL, 0.0, if to > offset { to - 0.8 * vh } else { to + 0.8 * vh });
        }
        tx.scroll_to_with(SCROLL, 0.0, to, Spring::new(0.24, 0.0));
        if !near {
            map.wait(key, if far { Arrive::Cut { gap } } else { Arrive::Glide { to } }, now);
        }
    }

    /// Brings the current find match into view (RD-60, adapted): a match whose block shows
    /// between 20% and 75% of the view's height, and above a toast's band while one shows,
    /// stays put, and any other goes where `App::place_match` puts it. A step (`glide`) glides; a
    /// search as you type jumps. A block far from the view may not be laid out, and the rows
    /// between may not have measured: the page jumps there in one move, held until they have.
    pub(crate) fn center_match(&mut self, glide: bool, tx: &mut Tx) {
        let m = self.find_matches();
        let doc = self.doc();
        let Some(hit) = m.get(self.reader.find.cur).copied() else { return };
        let i = hit.block;
        self.unfold_to_block(i);
        let Some(b) = doc.blocks.get(i) else { return };
        let [offset, _, vh, _] = self.reader.scroll.get();
        let band = if self.foot_shows() { self.reader.reveal.band } else { 0.0 };
        let map = self.reader.map.borrow();
        let Some(top) = map.block_top(i).filter(|_| map.covers(doc.blocks.len())) else {
            drop(map);
            return self.reveal_match(glide);
        };
        let (y0, y1) = (top - offset, top - offset + map.height(i));
        if y0 >= 0.2 * vh && y1 <= 0.75 * vh && y1 <= vh - band {
            // Its line is not reported, so the next match is not held by it.
            self.reader.reveal.find_line = None;
            return;
        }
        if top > offset - vh && top < offset + 2.0 * vh {
            drop(map);
            return self.reveal_match(glide);
        }
        // Rune's anchor goes where the list's estimates put the row, which mounts it, and places
        // the match's highlight again from its layout before the frame draws, centred by the
        // matched sub-block's rendered line height (including headings inside quotes). Preview's
        // `doc-scroll` starts strictly below `find_backing` whenever Find is open, so the match
        // roof inside `doc-scroll` is 0 (`FINDSTATE`, RD-104).
        drop(map);
        let roof = 0.0;
        let line_h = crate::page::match_line(self.b(), b, hit.pid);
        let to = crate::reveal::match_to(vh, line_h, roof, band);
        let n = self.reader.reveal.begin_match(self.now.get(), false, true, line_h);
        tx.scroll_key_to(format!("{}/{}", block_key(b), place_key(n)), to);
    }

    /// Brings the current match into view once it has laid out (`App::place_match`). Nothing
    /// scrolls before then, so the page makes one move, to one place.
    pub(crate) fn reveal_match(&mut self, glide: bool) {
        self.reader.reveal.begin_match(self.now.get(), glide, false, 0.0);
    }

    /// Places the current match once its highlight lays out (`crate::reveal::Reveal`).
    pub(crate) fn place_match(&mut self, n: u32, rect: [f32; 4], view: [f32; 4], roof: f32, band: f32, tx: &mut Tx) {
        let scroll_offset = self.reader.scroll.get()[0];
        let now = self.now.get();
        match self.reader.reveal.on_match_layout(n, rect, view, roof, band, scroll_offset, now) {
            crate::reveal::MatchAction::None => {}
            crate::reveal::MatchAction::Glide(to) => {
                // The step asked for a glide, though the layout's report comes in a commit of its own.
                tx.animated();
                tx.scroll_to(SCROLL, 0.0, to);
            }
            crate::reveal::MatchAction::Jump(to) => tx.scroll_to_instant(SCROLL, 0.0, to),
        }
    }

    /// The toast band laid out at window rect `[x, y, w, h]` (RD-104), routed into the unified
    /// reveal controller (`crate::reveal::Reveal`).
    pub(crate) fn band_laid_out(&mut self, rect: [f32; 4], caret: Option<[f32; 4]>, find_at: Option<[f32; 4]>, roof: f32, tx: &mut Tx) {
        let scroll = self.reader.scroll.get();
        let now = self.now.get();
        let typewriter = self.settings().typewriter_scroll;
        let has_find = self.find_query().is_some();
        tx.live_only();
        if self.reader.reveal.on_band_layout(rect, scroll, caret, find_at, roof, now, typewriter, has_find) {
            tx.wake_after(std::time::Duration::ZERO);
        }
    }

    /// Records an editor typing/caret key in the unified reveal controller so `follow_caret()` is
    /// not mistaken for a hand swipe and any deferred caret reveal glides smoothly above the band.
    pub(crate) fn note_editor_key(&mut self, tx: &mut Tx) {
        let writing_with_foot = self.writing() && self.foot_shows();
        let typewriter = self.settings().typewriter_scroll;
        if self.reader.reveal.on_editor_key(writing_with_foot, typewriter) {
            tx.wake_after(std::time::Duration::ZERO);
        }
    }

    /// Reveals the writer's caret smoothly above a visible foot band once layout reports its rect.
    pub(crate) fn reveal_caret(&mut self, seq: u32, caret: [f32; 4], view: [f32; 4], band: f32, tx: &mut Tx) {
        let scroll = self.reader.scroll.get();
        let now = self.now.get();
        let typewriter = self.settings().typewriter_scroll;
        let reduced = crate::motion::reduced();
        tx.live_only();
        match self.reader.reveal.on_caret_layout(seq, caret, view, band, scroll, now, typewriter, reduced) {
            crate::reveal::RevealAction::None | crate::reveal::RevealAction::Wake => {}
            crate::reveal::RevealAction::Glide(to) => {
                tx.animated();
                tx.scroll_to_with(SCROLL, 0.0, to, crate::reveal::REVEAL_SPRING);
            }
            crate::reveal::RevealAction::Instant(to) => tx.scroll_to_instant(SCROLL, 0.0, to),
        }
    }

    /// Runs once per frame before view commits: advances the unified reveal controller (pending
    /// band lifts, deferred caret-measure wakes, active glide settles, and gradual end-room release).
    pub(crate) fn poll_reveal(&mut self, tx: &mut Tx) -> bool {
        let foot = self.foot_shows();
        let now = self.now.get();
        let base = crate::page::end_room(self.mode == Mode::Source, self.view_h.get(), 0.0);
        let scroll_offset = self.reader.scroll.get()[0];
        let reduced = crate::motion::reduced();
        let (action, rebuild, wake) = self.reader.reveal.poll(foot, now, base, scroll_offset, reduced);
        match action {
            crate::reveal::RevealAction::None | crate::reveal::RevealAction::Wake => {}
            crate::reveal::RevealAction::Glide(to) => {
                tx.animated();
                tx.scroll_to_with(SCROLL, 0.0, to, crate::reveal::REVEAL_SPRING);
            }
            crate::reveal::RevealAction::Instant(to) => tx.scroll_to_instant(SCROLL, 0.0, to),
        }
        if wake {
            tx.wake_after(std::time::Duration::ZERO);
        }
        rebuild
    }

    /// The writer's text field laid out at window rect `[x, y, w, h]`: the progress line takes
    /// its place (WR-178).
    pub(crate) fn field_laid_out(&mut self, [_, y, _, h]: [f32; 4], tx: &mut Tx) {
        let now = self.reader.scroll.get();
        let mut map = self.reader.map.borrow_mut();
        if map.field_laid_out(y, h) {
            map.follow(now, false, &self.reader.bar);
        }
        tx.live_only();
    }

    /// Row `key` of the page laid out at window rect `[x, y, w, h]` (`ui/preview.rs`): the page
    /// map takes its height and place, and a trip waiting for the row lands by it.
    pub(crate) fn row_laid_out(&mut self, key: &str, [x, y, _, h]: [f32; 4], tx: &mut Tx) {
        let now = self.reader.scroll.get();
        let mut map = self.reader.map.borrow_mut();
        if map.row_laid_out(key, x, y, h) {
            map.follow(now, false, &self.reader.bar);
        }
        if let Some((k, travel, since)) = map.later.take() {
            match map.index.get(&k).copied() {
                Some(i) => {
                    drop(map);
                    return self.land(i, travel, tx);
                }
                None if self.now.get() - since < 2.0 => map.later = Some((k, travel, since)),
                None => {}
            }
        }
        let arrival = map.arrive(key, now, self.now.get());
        drop(map);
        match arrival {
            Some(Arrival::Key(at)) => tx.scroll_key_to(key.to_owned(), at),
            Some(Arrival::CutKey(at, to)) => {
                tx.scroll_key_to(key.to_owned(), at);
                tx.scroll_to_with(SCROLL, 0.0, to, Spring::new(0.24, 0.0));
            }
            Some(Arrival::Glide(to)) => tx.scroll_to_with(SCROLL, 0.0, to, Spring::new(0.24, 0.0)),
            None => tx.live_only(),
        }
    }

    /// The inclusive span of top-level blocks touched by the active text selection.
    pub(crate) fn selected_block_span(&self, doc: &Doc, tx: &Tx) -> Option<(usize, usize)> {
        let sel = tx.text_selection()?;
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;
        for t in sel.texts().iter().filter(|t| t.is_selected()) {
            let Some(k) = t.path().split('/').find(|seg| seg.starts_with("pb-")) else { continue };
            let Some(i) = doc.blocks.iter().position(|b| block_key(b) == k) else { continue };
            first = Some(first.map_or(i, |f| f.min(i)));
            last = Some(last.map_or(i, |l| l.max(i)));
        }
        first.zip(last)
    }

    /// The source lines of the selected block(s), else the block under the pointer, else the top
    /// block (RD-69, RD-100).
    fn block_here(&self, tx: &Tx) -> Option<std::ops::Range<usize>> {
        let doc = self.doc();
        if let Some((first, last)) = self.selected_block_span(&doc, tx) {
            return Some(doc.blocks[first].lines.start..doc.blocks[last].lines.end);
        }
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

    /// The text a reader sees in the block at `line`, without its Markdown (RD-68).
    pub fn copy_block_text(&mut self, line: usize, tx: &mut Tx) {
        let doc = self.doc();
        if let Some(i) = doc.blocks.iter().position(|b| b.lines.start == line) {
            let text: Vec<&str> = doc.pieces.iter().filter(|p| p.block == i).map(|p| p.text.as_str()).collect();
            tx.copy(text.join("\n"));
            self.toast("Copied the block's text", tx);
        }
    }

    /// The edit sheet on the block at `line`, or on its section (RD-68, SHEET-001).
    pub fn edit_block_at(&mut self, line: usize, scope: crate::sheet::Scope, tx: &mut Tx) {
        self.open_sheet_at(Some(line), scope, tx);
    }

    /// The Menu key or Shift+F10 in the reader: the menu of the selected or hovered block, else
    /// of the block at the top of the view (RD-68), as a right-click on it opens it.
    pub fn block_menu_here(&mut self, tx: &mut Tx) {
        let Some(lines) = self.block_here(tx) else { return };
        let Some(i) = self.doc().blocks.iter().position(|b| b.lines.start == lines.start) else { return };
        let at = self.reader.map.borrow().menu_at(i, self.reader.scroll.get()[0], self.b() * 1.6);
        if let Some(at) = at {
            self.open_menu(crate::layer::MenuOf::Block(lines.start), at, tx);
            // Opened from the keyboard, its first item shows marked, as the writer's does.
            tx.focus("block-menu", true);
        }
    }

    pub fn copy_block_here(&mut self, tx: &mut Tx) {
        if let Some(l) = self.block_here(tx) {
            tx.copy(source_lines(self.active_text(), l));
            self.toast("Copied the block's Markdown", tx);
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
        if self.active.is_none() || self.active_locked() {
            return;
        }
        tx.copy(self.active_text().to_string());
        self.toast("Copied the whole note", tx);
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

    /// A toast shows: a message, or a delete's Undo.
    pub(crate) fn toast_shows(&self) -> bool {
        self.deleted.is_some() || self.reader.toast.is_some()
    }

    /// A pill shows at the note area's foot: a toast or the notice (RD-104).
    pub(crate) fn foot_shows(&self) -> bool {
        self.toast_shows() || self.notice.is_some()
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

    /// Whether opening Find inserts an in-flow `find_backing` header slot above `doc-scroll`
    /// (Preview at any width, or toolbar-hidden Source / zen mode).
    fn has_find_slot(&self) -> bool {
        let has_doc = self.screen == crate::app::Screen::Vault && self.active.as_ref().is_some_and(|a| self.files().contains_key(a));
        if !has_doc || self.active_locked() || self.mode == Mode::Map {
            return false;
        }
        let shows_toolbar = !self.focus && self.mode == Mode::Source && self.settings().toolbar;
        !shows_toolbar
    }

    /// Compensates `doc-scroll` by the in-flow `find_backing` height when Find opens or closes
    /// while scrolled, keeping the visible reading line stationary (`FINDSTATE`). Leaves
    /// unscrolled `offset == 0` untouched so the top heading stays cleanly below Find.
    fn compensate_find_slot(&mut self, opening: bool, tx: &mut Tx) {
        if !self.has_find_slot() {
            return;
        }
        let offset = self.reader.scroll.get()[0];
        if offset <= 0.5 {
            return;
        }
        let h = crate::ui::reader::find_slot_h_for_app(self);
        let to = if opening { offset + h } else { (offset - h).max(0.0) };
        self.reader.glide = None;
        tx.scroll_to_instant(SCROLL, 0.0, to);
    }

    /// Ctrl+F or `/` (RD-52): open the bar, or select its text when it is open.
    pub fn open_find(&mut self, tx: &mut Tx) {
        if self.active.is_none() || self.active_locked() {
            return;
        }
        // Find and a drawer never share the page.
        self.close_layers(tx);
        let was_open = self.reader.find.open;
        let f = &mut self.reader.find;
        f.open = true;
        let n = f.q.text.len();
        f.q.set_selection(0, n);
        f.q.focused = true;
        let needs_search = !f.q.text.is_empty() && f.q.text != f.searched;
        tx.focus("find-input", false);
        if !was_open {
            self.compensate_find_slot(true, tx);
        }
        if needs_search {
            self.find_changed(tx);
        }
    }

    /// The title bar's Find button (RD-34): omarky's header chip toggles the bar.
    pub fn toggle_find(&mut self, tx: &mut Tx) {
        if self.reader.find.open {
            self.close_find(tx);
        } else if self.active.is_some() && !self.active_locked() {
            if self.mode == Mode::Source {
                self.writer_find(tx);
            } else {
                self.open_find(tx);
            }
        }
    }

    /// Closes find and clears its query. Its field goes away, so the note gets the keys.
    pub fn close_find(&mut self, tx: &mut Tx) {
        if self.reader.find.open {
            self.drop_find();
            self.compensate_find_slot(false, tx);
            self.keys_to_note(tx);
        }
    }

    /// A layer opens over find: the bar closes and keeps its query for Ctrl+F. In the writer a
    /// selected match gives way to a caret at its start, so typing after the layer closes does
    /// not replace it. Whether find was open.
    pub(crate) fn hide_find(&mut self, tx: &mut Tx) -> bool {
        if !self.reader.find.open {
            return false;
        }
        if self.mode == crate::app::Mode::Source {
            let cur = self.source_matches().get(self.reader.find.cur).cloned();
            if let (Some(r), Some(e)) = (cur, self.cur_edit_opt())
                && (e.anchor.min(e.focus), e.anchor.max(e.focus)) == (r.start, r.end)
            {
                e.set_selection(r.start, r.start);
            }
        }
        let f = &mut self.reader.find;
        f.open = false;
        f.q.focused = false;
        f.replace_open = false;
        f.replace.focused = false;
        f.due = None;
        self.compensate_find_slot(false, tx);
        true
    }

    /// Closes the find bar and forgets its query.
    pub(crate) fn drop_find(&mut self) {
        let f = &mut self.reader.find;
        f.open = false;
        f.q = TextEdit::new("");
        f.searched.clear();
        f.due = None;
        f.cur = 0;
        f.replace_open = false;
        f.replace = TextEdit::new("");
        self.reader.reveal.clear_find();
    }

    /// Whether the Find bar is currently showing its Source-mode Replace row.
    pub fn show_replace_row(&self) -> bool {
        self.reader.find.open && self.reader.find.replace_open && self.mode == Mode::Source && !self.active_locked()
    }

    pub fn find_query(&self) -> Option<&str> {
        let f = &self.reader.find;
        Some(f.searched.as_str()).filter(|q| f.open && !q.is_empty())
    }

    /// Whether a non-empty Find query is awaiting Preview debounce settlement (`FINDSTATE`).
    pub fn find_pending(&self) -> bool {
        let f = &self.reader.find;
        !f.q.text.is_empty() && (f.due.is_some() || f.q.text != f.searched)
    }

    /// Current Find bar result state (`FINDSTATE`).
    pub fn find_state(&self) -> FindState {
        let f = &self.reader.find;
        if f.q.text.is_empty() {
            FindState::Empty
        } else if self.find_pending() {
            FindState::Pending
        } else {
            match self.find_count() {
                0 => FindState::NoMatches,
                n => FindState::Matches(n),
            }
        }
    }

    /// How many matches the find bar counts, in any mode.
    pub fn find_count(&self) -> usize {
        match self.mode {
            Mode::Source => self.source_matches().len(),
            Mode::Map | Mode::Board => self.map_hits().len(),
            Mode::Preview => self.find_matches().len(),
        }
    }

    /// How many matches there are and the one a search starts from: the first at or below the
    /// top block (RD-57), or on the map at or after the selected node (MAP-066).
    fn find_start(&self) -> (usize, usize) {
        if self.mode == Mode::Map {
            let h = self.map_hits();
            return (h.len(), self.map_first_hit(&h));
        }
        let m = self.find_matches();
        (m.len(), first_match(&m, self.top()))
    }

    /// Brings the current match into view: on the page, or on the map.
    pub(crate) fn show_match(&mut self, glide: bool, tx: &mut Tx) {
        if self.mode == Mode::Map { self.map_show_hit(tx) } else { self.center_match(glide, tx) }
    }

    /// The matches of the find query in the open note.
    pub fn find_matches(&self) -> Vec<Match> {
        self.find_query().map_or_else(Vec::new, |q| blocks::find_all(&self.doc(), q))
    }

    /// The query changed. The writer searches at once; the reader 40 ms after the last
    /// keystroke, or 120 ms in a note of over 400 blocks (RD-56).
    pub fn find_changed(&mut self, tx: &mut Tx) {
        if self.mode == crate::app::Mode::Source {
            self.reader.find.searched = self.reader.find.q.text.clone();
            self.reader.find.due = None;
            return self.source_found();
        }
        if self.reader.find.q.text.is_empty() {
            self.reader.find.searched.clear();
            self.reader.find.due = None;
            self.reader.find.cur = 0;
            self.reader.reveal.clear_find();
            return;
        }
        let wait = if self.doc().blocks.len() > 400 { 0.12 } else { 0.04 };
        let due = tx.clock() + wait;
        self.reader.find.due = Some(due);
        tx.wake_at(due);
    }

    /// Runs a reader search whose wait is over: picks the first match from the top of the view
    /// (RD-57). Whether one ran.
    pub(crate) fn search_due(&mut self, t: f64, tx: &mut Tx) -> bool {
        let f = &mut self.reader.find;
        match f.due {
            Some(due) if t >= due => f.due = None,
            Some(due) => {
                tx.wake_at(due);
                return false;
            }
            None => return false,
        }
        f.searched = f.q.text.clone();
        self.reader.find.cur = self.find_start().1;
        self.show_match(false, tx);
        true
    }

    /// Enter / n and Shift+Enter / N (RD-58). Flushes any pending Preview debounce immediately
    /// before stepping (`FINDSTATE`).
    pub fn find_step(&mut self, forward: bool, tx: &mut Tx) {
        if self.mode == crate::app::Mode::Source {
            return self.source_step(forward, tx);
        }
        if self.reader.find.q.text.is_empty() {
            return;
        }
        if self.find_pending() {
            let f = &mut self.reader.find;
            f.searched = f.q.text.clone();
            f.due = None;
            let (n, first) = self.find_start();
            self.reader.find.cur = if forward { first } else { step(first, n, false) };
            self.show_match(true, tx);
            return;
        }
        let n = self.find_count();
        let f = &mut self.reader.find;
        f.cur = step(f.cur, n, forward);
        self.show_match(true, tx);
    }

    /// `?` or F1 in the reader and F1 in the writer: the keyboard help, or closes it. As it
    /// closes, the caret goes back to the note (WR-158).
    pub fn toggle_help(&mut self, tx: &mut Tx) {
        if self.layer_is(crate::layer::Kind::Help) {
            self.close_layer(tx);
        } else {
            self.reader.help_scroll.set(HelpScroll::default());
            self.open_layer(crate::layer::Layer::Help(crate::help::Search::for_mode(self.mode)), tx);
            tx.focus(HELP_SEARCH, false);
        }
    }

    /// Scrolls the help sheet with navigation keys while `Layer::Help` is open.
    pub fn scroll_help(&mut self, key: rune::Key, tx: &mut Tx) -> bool {
        use rune::Key;
        let mut st = self.reader.help_scroll.get();
        let [_, max, vh, _] = st.pos;
        let now = tx.clock();
        let page = if vh > 0.0 { (vh - 48.0).max(80.0) } else { 240.0 };
        let line = 48.0;
        let raw = match key {
            Key::Down => st.step_from(line, now) + line,
            Key::Up => (st.step_from(-line, now) - line).max(0.0),
            Key::PageDown => st.step_from(page, now) + page,
            Key::PageUp => (st.step_from(-page, now) - page).max(0.0),
            Key::Home => 0.0,
            Key::End => if max > 0.0 { max } else { 10_000.0 },
            _ => return false,
        };
        let to = if max > 0.0 { raw.clamp(0.0, max) } else { raw.max(0.0) };
        st.glide = Some((to, now));
        self.reader.help_scroll.set(st);
        tx.scroll_to_with(HELP_SCROLL, 0.0, to, Spring::new(0.16, 0.0));
        true
    }

    /// Esc in the reader (RD-85): dismiss the notice, else close find, else clear the
    /// selection, else Quire's Esc.
    pub fn reader_escape(&mut self, tx: &mut Tx) -> bool {
        if self.notice.take().is_some() {
            return true;
        }
        if self.reader.find.open {
            self.close_find(tx);
            return true;
        }
        if tx.text_selection().is_some() {
            tx.clear_text_selection();
            return true;
        }
        self.escape(tx)
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
                // The link that had the keyboard is gone with its note: the next note takes it.
                self.keys_to_note(tx);
            }
            // Note mode has no folder of notes: a link to a note file opens it as one more note.
            Target::Note(rel, frag) if self.note_mode() && root.join(&rel).is_file() => {
                if self.open_note(&root.join(&rel)) {
                    if let Some(f) = frag {
                        self.go_anchor(&f, tx);
                    }
                    self.keys_to_note(tx);
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

    /// A heading slug or footnote anchor in the open note (RD-04, RD-62), or its node on the
    /// map (MAP-054).
    pub fn go_anchor(&mut self, anchor: &str, tx: &mut Tx) {
        let doc = self.doc();
        let a = crate::md::percent_decode(anchor);
        let a = if doc.anchors.contains_key(a.as_str()) { a } else { blocks::slugify(&a) };
        let Some(&i) = doc.anchors.get(a.as_str()) else { return };
        let line = match &doc.blocks[i].kind {
            blocks::Kind::Footnotes { items } => items.iter().find(|f| f.anchor == a).map(|f| f.lines.start),
            _ => None,
        };
        match self.mode {
            Mode::Source => self.source_jump(line.unwrap_or(doc.blocks[i].lines.start), tx),
            Mode::Preview => self.land(i, true, tx),
            Mode::Map => self.map_go_line(line.unwrap_or(doc.blocks[i].lines.start), tx),
            Mode::Board => {
                let t = self.map_tree();
                if let Some(item) = crate::board::item_of(&t, t.at_line(line.unwrap_or(doc.blocks[i].lines.start))) {
                    self.board_select(t.nodes[item].id, tx);
                }
            }
        }
    }

    /// `Ctrl+Z` and `Ctrl+Shift+Z` / `Ctrl+Y` in Preview: undoes or redoes on the note's shared
    /// `TextEdit` history so task toggles and Edit Sheet saves can be undone without leaving Preview.
    pub fn reader_undo(&mut self, redo: bool, tx: &mut Tx) {
        if self.mode != Mode::Preview || self.active.is_none() || self.active_locked() {
            return;
        }
        let before = self.active_text().to_string();
        let e = self.cur_edit();
        if redo {
            e.redo();
        } else {
            e.undo();
        }
        self.after_edit();
        if self.active_text() == before {
            return;
        }
        self.flush_notes();
        self.toast(if redo { "Redid" } else { "Undid" }, tx);
    }
}

/// The page's folds: the map's folded headings for this tab, or the folds the note opens with
/// (`<!-- markmap: fold -->`, MAP-042), so a section folded in one view is folded in the other.
#[derive(Default)]
pub struct PageFolds {
    /// Per top-level block: inside a folded heading's section, so drawn as nothing.
    pub hidden: Vec<bool>,
    /// Per heading block with a section: its map node, whether it is folded, and how many
    /// nodes the fold hides, as the map's `+N` says.
    pub heads: HashMap<usize, (usize, bool, usize)>,
    /// Changes whenever any heading folds or unfolds.
    pub sig: u64,
}

impl App {
    pub fn page_folds(&self) -> PageFolds {
        use std::hash::{Hash, Hasher};
        let doc = self.doc();
        let t = self.map_tree();
        let opening;
        let folds = match self.map_view() {
            Some(v) => &v.folds,
            None => {
                opening = t.initial_folds().into_iter().collect();
                &opening
            }
        };
        let mut out = PageFolds { hidden: vec![false; doc.blocks.len()], ..PageFolds::default() };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut under: Option<usize> = None;
        for (i, bl) in doc.blocks.iter().enumerate() {
            if let blocks::Kind::Heading { level, .. } = bl.kind {
                if under.is_some_and(|u| level <= u) {
                    under = None;
                }
                let n = t.at_line(bl.lines.start);
                let node = &t.nodes[n];
                if under.is_none() && matches!(node.kind, crate::map::NodeKind::Heading { .. }) && !node.children.is_empty() {
                    let f = folds.whole(node.id);
                    out.heads.insert(i, (n, f, node.end - n - 1));
                    (i, f).hash(&mut h);
                    if f {
                        under = Some(level);
                    }
                    continue;
                }
            }
            out.hidden[i] = under.is_some() && !matches!(bl.kind, blocks::Kind::Footnotes { .. });
        }
        out.sig = h.finish();
        out
    }

    /// Folds or unfolds the heading whose map node is `n`, on the page and on the map.
    pub fn toggle_page_fold(&mut self, n: usize) {
        let t = self.map_tree();
        if let Some(v) = self.map_view_mut() {
            let fold = !v.folds.whole(t.nodes[n].id);
            v.folds.set(&t, &crate::map_layout::sides(&t), n, None, fold);
            v.fold_ver += 1;
        }
    }

    /// Keyboard heading fold toggle in Preview: toggles the hovered heading, else the nearest
    /// foldable heading at or above the top visible block (or the first foldable heading).
    pub fn toggle_current_page_fold(&mut self) {
        if self.mode != Mode::Preview || self.active_locked() {
            return;
        }
        let doc = self.doc();
        let pf = self.page_folds();
        if pf.heads.is_empty() {
            return;
        }
        let hovered_idx = self
            .reader
            .hover
            .and_then(|line| doc.blocks.iter().position(|b| b.lines.start == line));
        let pick_bi = hovered_idx
            .filter(|i| pf.heads.contains_key(i))
            .or_else(|| {
                let top = hovered_idx.unwrap_or_else(|| self.top());
                pf.heads.keys().copied().filter(|&i| i <= top).max()
            })
            .or_else(|| pf.heads.keys().copied().min());
        if let Some(bi) = pick_bi
            && let Some(&(n, _, _)) = pf.heads.get(&bi)
        {
            self.toggle_page_fold(n);
        }
    }

    /// Opens the folds that hide top-level block `i` on the page, and selects its node on the map.
    fn unfold_to_block(&mut self, i: usize) {
        if self.page_folds().hidden.get(i) == Some(&true) {
            let line = self.doc().blocks[i].lines.start;
            let n = self.map_tree().at_line(line);
            self.map_open_to(n);
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
        assert!(progress(0, 100, 2.0, 300.0, 500.0) < 0.01, "a short page reads 0 at its top");
        assert!((progress(0, 100, 150.0, 300.0, 500.0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn rd_57_58_find_order() {
        let m = |block| Match { pid: 0, block, k: 0 };
        let ms = [m(1), m(3), m(3), m(7)];
        assert_eq!(first_match(&ms, 2), 1);
        assert_eq!(first_match(&ms, 9), 0);
        assert_eq!((step(3, 4, true), step(0, 4, false), step(0, 0, true)), (0, 3, 0));
    }

    /// VER-32, SB-29: one scroll offset moves the outline panel's highlight and the shelf's
    /// current entry through their live colours, with no view run. The panel marks its first row
    /// above the first heading; the shelf marks nothing there.
    #[test]
    fn both_outlines_follow_the_scroll() {
        let mut map = PageMap::default();
        map.reading = true;
        map.set_rows(Some(1), 14.0, || (0..10).map(|i| format!("b{i}")).collect(), || vec![0.0; 10], 100.0);
        let rows = |n: usize| (0..n).map(|_| Mark { fill: Live::new(Color::TRANSPARENT), ink: Live::new(Color::BLACK), on: [Color::WHITE, Color::WHITE], off: [Color::TRANSPARENT, Color::BLACK] }).collect::<Vec<_>>();
        map.outline = Marks { heads: vec![(2, Some(0)), (4, Some(1)), (7, Some(2))], first_row: Some(0), marks: rows(3), marked: None };
        map.shelf = Marks { heads: vec![(4, Some(0)), (7, Some(1))], first_row: None, marks: rows(2), marked: None };
        let inks = |m: &Marks| m.marks.iter().map(|r| r.ink.get() == Color::WHITE).collect::<Vec<_>>();
        map.mark([0.0, 900.0, 400.0, 800.0]);
        assert_eq!((map.outline.marked, map.shelf.marked), (Some(0), None), "above the first H1 the shelf marks nothing");
        assert_eq!((inks(&map.outline), inks(&map.shelf)), (vec![true, false, false], vec![false, false]));
        map.mark([350.0, 900.0, 400.0, 800.0]);
        assert_eq!((inks(&map.outline), inks(&map.shelf)), (vec![false, true, false], vec![true, false]), "block 4 is at the view's top");
        map.mark([900.0, 900.0, 400.0, 800.0]);
        assert_eq!((inks(&map.outline), inks(&map.shelf)), (vec![false, false, true], vec![false, true]));
    }

    #[test]
    fn outline_reaches_last_heading_at_bottom_in_preview_and_source() {
        let rows = |n: usize| (0..n).map(|_| Mark { fill: Live::new(Color::TRANSPARENT), ink: Live::new(Color::BLACK), on: [Color::WHITE, Color::WHITE], off: [Color::TRANSPARENT, Color::BLACK] }).collect::<Vec<_>>();

        // Preview: last heading sits in the lower half of the final viewport (y=1200 with max=900, vh=600).
        let mut map = PageMap::default();
        map.reading = true;
        map.set_rows(Some(1), 14.0, || (0..15).map(|i| format!("b{i}")).collect(), || vec![0.0; 15], 100.0);
        map.outline = Marks { heads: vec![(0, Some(0)), (6, Some(1)), (12, Some(2))], first_row: Some(0), marks: rows(3), marked: None };
        map.mark([0.0, 900.0, 600.0, 800.0]);
        assert_eq!(map.outline.marked, Some(0));
        map.mark([900.0, 900.0, 600.0, 800.0]);
        assert_eq!(map.outline.marked, Some(2), "scrolling to bottom selects the last heading even below offset+100");

        // Source mode: live outline updates smoothly on hand scroll all the way to the last heading.
        let mut smap = PageMap::default();
        smap.writing = true;
        smap.lines = 100;
        smap.content_laid_out(40.0);
        smap.field_laid_out(100.0, 2000.0); // top=60, h=2000 -> text ends at 2060
        smap.outline = Marks { heads: vec![(0, Some(0)), (50, Some(1)), (92, Some(2))], first_row: Some(0), marks: rows(3), marked: None };
        let bar = Live::new([0.0; 4]);
        smap.follow([0.0, 1760.0, 600.0, 800.0], true, &bar);
        assert_eq!(smap.outline.marked, Some(0));
        smap.follow([1000.0, 1760.0, 600.0, 800.0], true, &bar);
        assert_eq!(smap.outline.marked, Some(1));
        smap.follow([1460.0, 1760.0, 600.0, 800.0], true, &bar);
        assert_eq!(smap.outline.marked, Some(2), "scrolling to end of source text selects the last heading");
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

    #[test]
    fn help_scroll_hand_clears_stale_glide_and_preserves_momentum() {
        let mut hs = HelpScroll::default();
        hs.on_offset([0.0, 300.0, 300.0, 472.0], false, 0.1);
        hs.glide = Some((300.0, 0.40));
        // Wheel up arrives within KEY_GLIDE: clears stale End glide target immediately.
        hs.on_offset([270.0, 300.0, 300.0, 472.0], true, 0.44);
        assert!(hs.glide.is_none(), "hand scroll clears stale key glide");
        hs.on_offset([230.0, 300.0, 300.0, 472.0], true, 0.46);
        let from_up = hs.step_from(-48.0, 0.48);
        assert!(from_up < 230.0, "same-direction key preserves wheel momentum: {from_up}");
        let from_down = hs.step_from(48.0, 0.48);
        assert_eq!(from_down, 230.0, "opposite-direction key starts from current offset");
    }

    #[test]
    fn preview_undo_redo_reverts_and_reapplies_task_toggle() {
        let dir = std::env::temp_dir().join(format!("quire-preview-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Tasks.md"), "# Tasks\n\n- [ ] First task\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Tasks.md", None, tx);
            s.mode = crate::app::Mode::Preview;
            s.toggle_task(2, true);
        });
        assert!(rt.state.active_text().contains("- [x] First task"));
        rt.update(|s, tx| s.reader_undo(false, tx));
        assert!(rt.state.active_text().contains("- [ ] First task"), "Ctrl+Z in Preview must undo task toggle");
        rt.update(|s, tx| s.reader_undo(true, tx));
        assert!(rt.state.active_text().contains("- [x] First task"), "Ctrl+Shift+Z in Preview must redo task toggle");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
