//! Board mode state and actions on `App` (`docs/reader-writer/behavior/board.md`, decision 0015).
//!
//! Selection is shared with Map mode (`MapView::selected`); edits go through `map.rs` and
//! `App::map_apply` so undo, save, locked notes and sync work identically.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rune::{Drag, Live, Phase, Tx};

use crate::app::{App, Mode};
use crate::board::{self, Board, Slot, SlotParent};
use crate::board_drag::Hold;
use crate::map::{self, NodeId};

/// Layout rectangles reported by Rune (`El::on_layout`) during the last frame, in window points.
#[derive(Default)]
pub struct BoardLayout {
    /// Board viewport `[x, y, w, h]` in window points.
    pub view: [f32; 4],
    /// Origin `[x, y]` of `board-row` in window points when laid out.
    pub row_origin: [f32; 2],
    /// Total laid-out width of `board-row` for clamping horizontal scroll.
    pub row_w: f32,
    /// Column outer rects by column heading `NodeId`.
    pub cols: HashMap<u64, [f32; 4]>,
    /// Column header rects by column heading `NodeId`.
    pub heads: HashMap<u64, [f32; 4]>,
    /// Scroll-invariant `board-row`-local `[x, y, w, h]` of each column header for change detection.
    pub head_rel: HashMap<u64, [f32; 4]>,
    /// Column scroll-area rects by column heading `NodeId`.
    pub col_views: HashMap<u64, [f32; 4]>,
    /// Column card-stack content height by column heading `NodeId`, for clamping vertical scroll.
    pub col_content_h: HashMap<u64, f32>,
    /// Card rects by `NodeId` (both top-level cards and expanded sub-cards).
    pub cards: HashMap<NodeId, [f32; 4]>,
    /// Scroll-invariant `[row_local_x, col_content_y, w, h]` of each card for change detection.
    pub card_rel: HashMap<NodeId, [f32; 4]>,
    /// Owning column id and that column's `col_scroll` when each card was laid out, so
    /// scroll-invariant coordinates remain exact mid-glide.
    pub card_scroll: HashMap<NodeId, (u64, f32)>,
    /// Expanded subtask-list container rects by parent card `NodeId`.
    pub sub_lists: HashMap<NodeId, [f32; 4]>,
}

/// Anchor linking the mounted `board-selection` glass rim to the selected card's column scroll so
/// vertical column scrolling translates and clips the rim live on the GPU with zero view commits.
#[derive(Clone, Copy)]
pub struct SelColAnchor {
    pub col_id: u64,
    pub base_scroll: f32,
    pub vis_top: f32,
    pub card_h: f32,
    pub col_h: f32,
}

/// Board mode state held on `App`.
pub struct BoardState {
    /// Last linear mode (`Source` or `Preview`), for `Ctrl+Shift+K` toggle back (BRD-029).
    pub back: Mode,
    /// Active drag / settle state machine.
    pub hold: Hold,
    /// Layout reports from Rune (`El::on_layout_tx`).
    pub layout: RefCell<BoardLayout>,
    /// Horizontal pan state while dragging empty board space `(start_scroll_x, start_pointer_x)`.
    pub pan_drag: Option<(f32, f32)>,
    /// Current horizontal scroll offset of `board-scroll`.
    pub scroll_x: Rc<Cell<f32>>,
    /// Current vertical scroll offset per column (keyed by column heading `NodeId`).
    pub col_scroll: Rc<RefCell<HashMap<u64, f32>>>,
    /// Live vertical offset applied to `board-selection` while its owning column scrolls.
    pub sel_scroll_offset: Live<[f32; 2]>,
    /// Live opacity of `board-selection` (0 when scrolled outside its column viewport).
    pub sel_scroll_opacity: Live<f32>,
    /// Column scroll anchor for the currently mounted `board-selection` card rim.
    pub sel_col_anchor: Rc<Cell<Option<SelColAnchor>>>,
    /// Expanded parent task cards per note path (default folded).
    pub expanded: HashMap<String, HashSet<NodeId>>,
    /// Explicit per-note mode chosen by the user in this session/vault. A kanban note without
    /// an entry here automatically enters Board mode when opened.
    pub note_modes: HashMap<String, Mode>,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState {
            back: Mode::Source,
            hold: Hold::None,
            layout: RefCell::new(BoardLayout::default()),
            pan_drag: None,
            scroll_x: Rc::new(Cell::new(0.0)),
            col_scroll: Rc::new(RefCell::new(HashMap::new())),
            sel_scroll_offset: Live::new([0.0, 0.0]),
            sel_scroll_opacity: Live::new(1.0),
            sel_col_anchor: Rc::new(Cell::new(None)),
            expanded: HashMap::new(),
            note_modes: HashMap::new(),
        }
    }
}

impl App {
    /// True when the active note is unlocked and declares `kanban-plugin` in front matter.
    pub fn active_is_kanban(&self) -> bool {
        if self.screen != crate::app::Screen::Vault || self.active.is_none() || self.active_locked() {
            return false;
        }
        board::is_kanban(&self.doc())
    }

    /// True when Board mode shows an open, unlocked kanban note.
    pub fn board_editable(&self) -> bool {
        self.mode == Mode::Board && self.active_is_kanban()
    }

    /// Expanded parent card `NodeId`s for the active note (default empty = all folded).
    pub fn board_expanded(&self) -> HashSet<NodeId> {
        self.active
            .as_deref()
            .and_then(|rel| self.board.expanded.get(rel))
            .cloned()
            .unwrap_or_default()
    }

    /// True when parent card `id` is currently unfolded on the active note's board.
    pub fn board_is_expanded(&self, id: NodeId) -> bool {
        self.active
            .as_deref()
            .and_then(|rel| self.board.expanded.get(rel))
            .is_some_and(|set| set.contains(&id))
    }

    /// Builds the current task-only board projection from `map::Tree`.
    pub fn board_model(&self) -> (Rc<map::Tree>, Board) {
        let t = self.map_tree();
        let b = board::build(&t);
        (t, b)
    }

    /// Records the user's explicit mode choice for the active note so future switches respect it.
    pub(crate) fn record_note_mode(&mut self, mode: Mode) {
        if let Some(rel) = self.active.clone() {
            self.board.note_modes.insert(rel, mode);
        }
    }

    /// Leaves Board mode cleanly if the active note is no longer eligible for Kanban (locked,
    /// missing `kanban-plugin`, or switched to a non-kanban note).
    pub(crate) fn board_enforce_kanban(&mut self, tx: Option<&mut Tx>) {
        if self.mode == Mode::Board && !self.active_is_kanban() {
            self.board_cancel_hold();
            self.board.pan_drag = None;
            let back = match self.board.back {
                Mode::Preview => Mode::Preview,
                _ => Mode::Source,
            };
            self.mode = back;
            if let Some(tx) = tx {
                if self.sheet().is_some_and(|sh| sh.card.is_some()) {
                    self.close_layer(tx);
                }
                self.keys_to_note(tx);
            } else if self.sheet().is_some_and(|sh| sh.card.is_some()) {
                self.layers.clear();
            }
        }
    }

    /// Syncs `App::mode` when opening or switching to the active note:
    /// - Non-kanban notes can never stay in Board mode (fall back to `board.back`).
    /// - Kanban notes with no saved mode state automatically enter Board mode.
    /// - Kanban notes with an explicit mode state restore that mode.
    pub(crate) fn sync_mode_for_active_note(&mut self, force_source: bool, has_explicit_state: bool, tx: Option<&mut Tx>) {
        let Some(rel) = self.active.clone() else { return };
        if force_source {
            self.board.note_modes.insert(rel, Mode::Source);
            return;
        }
        if !self.active_is_kanban() {
            self.board_enforce_kanban(tx);
            return;
        }
        if let Some(&chosen) = self.board.note_modes.get(&rel) {
            if chosen != self.mode {
                if let Some(tx) = tx {
                    self.show_mode_inner(chosen, false, tx);
                } else {
                    self.mode = chosen;
                    if chosen == Mode::Board {
                        self.board_ensure_selection();
                    }
                }
            }
        } else if !has_explicit_state {
            // Kanban note with no saved mode state: enter Board mode automatically.
            if self.mode != Mode::Board {
                if let Some(tx) = tx {
                    self.enter_board(tx);
                } else {
                    if matches!(self.mode, Mode::Source | Mode::Preview) {
                        self.board.back = self.mode;
                    }
                    self.mode = Mode::Board;
                    self.board_ensure_selection();
                }
            }
        } else {
            self.board.note_modes.insert(rel, self.mode);
        }
    }

    /// `Ctrl+Shift+K` (`app.mode-board`, BRD-029): switches to Board on a kanban note, and from
    /// Board back to the last linear mode (`Source` or `Preview`).
    pub fn toggle_board(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        if self.mode == Mode::Board {
            let back = match self.board.back {
                Mode::Preview => Mode::Preview,
                _ => Mode::Source,
            };
            self.show_mode(back, tx);
        } else if self.active_is_kanban() {
            self.show_mode(Mode::Board, tx);
        } else {
            self.toast("Add kanban-plugin to front matter to use Board", tx);
        }
    }

    /// Enters Board mode, keeping selection synced with Map/Source/Preview (BRD-024).
    pub fn enter_board(&mut self, tx: &mut Tx) {
        if self.mode == Mode::Board || !self.active_is_kanban() {
            return;
        }
        tx.clear_text_selection();
        let from = self.mode;
        self.board.back = match from {
            Mode::Source | Mode::Preview => from,
            Mode::Map => match self.map.back {
                Mode::Preview => Mode::Preview,
                _ => Mode::Source,
            },
            Mode::Board => self.board.back,
        };
        let line = if !self.active_locked() && matches!(from, Mode::Source | Mode::Preview) {
            Some(self.linear_place_line())
        } else {
            None
        };
        self.mode = Mode::Board;
        self.board_cancel_hold();
        self.board.pan_drag = None;
        self.close_find(tx);
        self.keys_to_note(tx);
        let t = self.map_tree();
        let b = board::build(&t);
        let expanded = self.board_expanded();
        if let Some(l) = line {
            let at = t.at_line(l);
            if let Some(item) = board::item_of(&t, &b, &expanded, at).or_else(|| board::default_selection(&b))
                && let Some(v) = self.map_view_mut()
            {
                v.selected = t.nodes[item].id;
            }
        } else {
            self.board_ensure_selection();
        }
    }

    /// Leaves Board mode for `to`, keeping the selected node's line (BRD-024).
    pub fn leave_board(&mut self, to: Mode, tx: &mut Tx) {
        if self.mode != Mode::Board || to == Mode::Board {
            return;
        }
        self.board_cancel_hold();
        self.board.pan_drag = None;
        let line = if self.active_locked() {
            0
        } else {
            let t = self.map_tree();
            let sel = self.board_selected_index(&t).unwrap_or(map::ROOT);
            t.nodes[sel].own.start
        };
        if to == Mode::Map {
            self.board_ensure_selection();
            self.enter_map(tx);
            return;
        }
        self.mode = to;
        if to == Mode::Source {
            self.source_jump(line, tx);
        } else {
            let doc = self.doc();
            let block = doc.blocks.iter().rposition(|b| b.lines.start <= line).unwrap_or(0);
            self.keys_to_note(tx);
            if block == 0 {
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, 0.0);
            } else {
                self.land(block, false, tx);
            }
        }
    }

    fn linear_place_line(&self) -> usize {
        match self.mode {
            Mode::Source => {
                let text = self.active_text();
                let caret = self.active.as_deref().and_then(|a| self.edits.get(a)).map_or(0, |e| e.focus.min(text.len()));
                crate::md::line_col(text, caret).0
            }
            Mode::Preview => self.doc().blocks.get(self.top()).map_or(0, |b| b.lines.start),
            _ => 0,
        }
    }

    /// Selected board item index in `t`, reading `MapView::selected` and projecting any hidden or
    /// nested node to its deepest visible card/column heading, falling back to the first item.
    pub fn board_selected_index(&self, t: &map::Tree) -> Option<usize> {
        let b = board::build(t);
        let expanded = self.board_expanded();
        if let Some(v) = self.map_view()
            && let Some(i) = t.find(v.selected)
            && let Some(item) = board::item_of(t, &b, &expanded, i)
        {
            return Some(item);
        }
        board::default_selection(&b)
    }

    /// Selected board item's `NodeId`, if the board has any selectable card or column header.
    pub fn board_selected(&self) -> Option<NodeId> {
        let t = self.map_tree();
        self.board_selected_index(&t).map(|i| t.nodes[i].id)
    }

    /// Ensures `MapView::selected` points to a valid board item when entering or editing.
    pub fn board_ensure_selection(&mut self) {
        let t = self.map_tree();
        let wanted = self.board_selected_index(&t).map(|i| t.nodes[i].id);
        if let Some(id) = wanted
            && let Some(v) = self.map_view_mut()
        {
            v.selected = id;
        }
    }

    /// Selects a card or column header by its `NodeId` without stealing focus (BRD-012, BRD-024).
    pub fn board_select(&mut self, id: NodeId) {
        if let Some(v) = self.map_view_mut() {
            v.selected = id;
        }
    }

    /// Expands all ancestor cards of task card `i` so it is visible on the board.
    pub fn board_expand_ancestors(&mut self, i: usize, t: &map::Tree, model: &Board) {
        let Some(rel) = self.active.clone() else { return };
        let anc = board::ancestors_to_expand(t, model, i);
        if !anc.is_empty() {
            self.board.expanded.entry(rel).or_default().extend(anc);
        }
    }

    /// Toggles the folding handle on task card `id` (revealing or hiding its subtasks as sub-cards).
    pub fn board_toggle_fold(&mut self, id: NodeId, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let Some(rel) = self.active.clone() else { return };
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(i) = t.find(id) else { return };
        let Some(card) = model.card(i) else { return };
        if card.subtasks.is_empty() {
            return;
        }
        let set = self.board.expanded.entry(rel).or_default();
        if !set.remove(&id) {
            set.insert(id);
        }
        self.board_select(id);
        self.keys_to_note(tx);
    }

    /// `Space` (`board.toggle-fold`): toggles subtask folding on the selected card if it has subtasks.
    pub fn board_toggle_selected_fold(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        if let Some(id) = self.board_selected() {
            self.board_toggle_fold(id, tx);
        }
    }

    /// Carries expanded parent card `NodeId`s across a splice edit using `map::follow`.
    pub(crate) fn board_follow_expanded(&mut self, rel: &str, old: &map::Tree, new: &map::Tree, s: &map::Splice) {
        if let Some(set) = self.board.expanded.get_mut(rel) {
            let next: HashSet<NodeId> = set.iter().filter_map(|&id| map::follow(old, new, s, id)).collect();
            *set = next;
        }
    }

    /// Carries expanded parent card `NodeId`s across an outside reload by keeping surviving IDs.
    pub(crate) fn board_reloaded(&mut self, rel: &str, new: &map::Tree) {
        if let Some(set) = self.board.expanded.get_mut(rel) {
            set.retain(|&id| new.find(id).is_some());
        }
    }

    /// Clicks a card (opens card mini window in Preview mode) or a column header (selects, or
    /// edits on double-click). Rune drops the synthetic press after a moved drag.
    pub fn board_click(&mut self, id: NodeId, is_head: bool, double: bool, tx: &mut Tx) {
        self.board_select(id);
        self.keys_to_note(tx);
        if is_head {
            if double {
                self.board_edit_selected(tx);
            }
        } else if !double {
            self.board_open_card(id, false, tx);
        }
    }

    /// Opens the card mini window for task card `id` in Preview (`editing = false`) or Edit
    /// (`editing = true`) mode, reusing `Layer::Sheet`.
    pub fn board_open_card(&mut self, id: NodeId, editing: bool, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = t.find(id) else { return };
        if !board::is_task(&t, i) {
            return;
        }
        self.board_select(id);
        let lines = t.nodes[i].span.clone();
        if !lines.is_empty() {
            self.open_card_sheet(id, lines, editing, tx);
        }
    }

    /// `Enter` (`board.open`): opens the selected card's mini window in Preview mode, or edits
    /// the selected column header in the Edit Sheet.
    pub fn board_open_selected(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        let n = &t.nodes[i];
        if n.parent == Some(map::ROOT) && board::is_heading(&t, i) {
            if !n.own.is_empty() {
                self.open_sheet_lines(n.own.clone(), crate::sheet::Scope::Block, tx);
            }
        } else {
            self.board_open_card(n.id, false, tx);
        }
    }

    /// `e` / `F2` (`board.edit`): opens the selected card's mini window directly in Edit mode, or
    /// edits the selected column header in the Edit Sheet.
    pub fn board_edit_selected(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        let n = &t.nodes[i];
        if n.parent == Some(map::ROOT) && board::is_heading(&t, i) {
            if !n.own.is_empty() {
                self.open_sheet_lines(n.own.clone(), crate::sheet::Scope::Block, tx);
            }
        } else {
            self.board_open_card(n.id, true, tx);
        }
    }

    /// Toggles task state on node `id`, finding it again in the current tree (BRD-011, BRD-027, A6).
    pub fn board_toggle_task(&mut self, id: NodeId, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = t.find(id) else { return };
        if let Some(s) = map::toggle_task(self.active_text(), &t, i) {
            self.board_apply(&t, &s, tx);
            let new_t = self.map_tree();
            let new_text = self.active_text().to_string();
            if let Some(new_id) = map::follow(&t, &new_t, &s, id)
                && let Some(sh) = self.sheet_mut()
                && let Some(card) = sh.card.as_mut()
                && card.node == id
                && !card.editing
            {
                card.node = new_id;
                if let Some(ni) = new_t.find(new_id) {
                    sh.pick.lines = new_t.nodes[ni].span.clone();
                }
                sh.base = new_text;
            }
        }
    }

    /// Applies a `map::Splice` from Board mode through the shared `App::map_apply` pipeline
    /// (BRD-019, MAP-013, C7).
    pub fn board_apply(&mut self, old: &map::Tree, s: &map::Splice, tx: &mut Tx) {
        let rel = self.active.clone();
        self.map_apply(old, s, tx);
        if let Some(rel) = rel {
            let new_t = self.map_tree();
            self.board_follow_expanded(&rel, old, &new_t, s);
        }
        self.board_enforce_kanban(Some(tx));
        if self.mode == Mode::Board && self.layer().is_none() && crate::focus::on_note(tx) {
            self.keys_to_note(tx);
        }
    }

    /// Maximum horizontal scroll offset for `board-scroll` (C2).
    pub fn board_max_scroll_x(&self) -> f32 {
        let layout = self.board.layout.borrow();
        (layout.row_w - layout.view[2]).max(0.0)
    }

    /// Horizontal drag pan on empty board space (BRD-007).
    pub fn board_pan_drag(&mut self, d: &Drag, tx: &mut Tx) {
        if self.board.hold.active() {
            return;
        }
        match d.phase {
            Phase::Down => {
                self.board.pan_drag = Some((self.board.scroll_x.get(), d.window_point()[0]));
                tx.live_only();
            }
            Phase::Move => {
                if let Some((from_x, start_px)) = self.board.pan_drag {
                    let next = (from_x - (d.window_point()[0] - start_px)).clamp(0.0, self.board_max_scroll_x());
                    self.board.scroll_x.set(next);
                    tx.scroll_to_instant("board-scroll", next, 0.0);
                    tx.live_only();
                }
            }
            Phase::Up => {
                self.board.pan_drag = None;
            }
        }
    }

    /// Records the board viewport rect from `El::on_layout_tx`; commits only when height changes
    /// (`avail_h` depends on it), keeping pure scroll frames commit-free.
    pub fn board_view_laid_out(&self, r: [f32; 4], tx: &mut Tx) {
        let mut l = self.board.layout.borrow_mut();
        let height_changed = (l.view[3] - r[3]).abs() > 0.5;
        l.view = r;
        if !height_changed {
            tx.live_only();
        }
    }

    /// Records the board row's origin and total width from `El::on_layout_tx`; commits only on
    /// first mount so `SelectionGlass` can appear, never on scroll frames.
    pub fn board_row_laid_out(&self, r: [f32; 4], tx: &mut Tx) {
        let mut l = self.board.layout.borrow_mut();
        let first = l.row_w <= 0.0 && r[2] > 0.0;
        l.row_origin = [r[0], r[1]];
        l.row_w = r[2];
        if !first {
            tx.live_only();
        }
    }

    /// Records a column's outer rect from `El::on_layout_tx` (bookkeeping only).
    pub fn board_col_laid_out(&self, col_key: u64, r: [f32; 4], tx: &mut Tx) {
        self.board.layout.borrow_mut().cols.insert(col_key, r);
        tx.live_only();
    }

    /// Records a column header's rect from `El::on_layout_tx`; commits only when the selected
    /// header mounts or changes its row-local rect, never during scroll translation.
    pub fn board_head_laid_out(&self, col_key: u64, r: [f32; 4], tx: &mut Tx) {
        let mut l = self.board.layout.borrow_mut();
        let rel = [r[0] - l.row_origin[0], r[1] - l.row_origin[1], r[2], r[3]];
        let prev = l.head_rel.insert(col_key, rel);
        l.heads.insert(col_key, r);
        drop(l);
        let moved = prev.is_none_or(|p| (0..4).any(|k| (p[k] - rel[k]).abs() > 0.5));
        if !(moved && self.board_selected() == Some(col_key)) {
            tx.live_only();
        }
    }

    /// Records a column's cards scroll viewport rect from `El::on_layout_tx` (bookkeeping only).
    pub fn board_col_view_laid_out(&self, col_key: u64, r: [f32; 4], tx: &mut Tx) {
        self.board.layout.borrow_mut().col_views.insert(col_key, r);
        tx.live_only();
    }

    /// Records a column's card-stack content height from `El::on_layout_tx` (bookkeeping only).
    pub fn board_col_content_laid_out(&self, col_key: u64, h: f32, tx: &mut Tx) {
        self.board.layout.borrow_mut().col_content_h.insert(col_key, h);
        tx.live_only();
    }

    /// Records a card's rect and owning column's scroll offset from `El::on_layout_tx`; commits
    /// only when the selected card mounts or changes its scroll-invariant rect, never during
    /// horizontal or vertical scroll translation.
    pub fn board_card_laid_out(&self, id: NodeId, col_key: u64, r: [f32; 4], tx: &mut Tx) {
        let cy = self.board.col_scroll.borrow().get(&col_key).copied().unwrap_or(0.0);
        let mut l = self.board.layout.borrow_mut();
        let cv_y = l.col_views.get(&col_key).map_or(l.row_origin[1], |cv| cv[1]);
        let rel = [r[0] - l.row_origin[0], (r[1] - cv_y) + cy, r[2], r[3]];
        let prev = l.card_rel.insert(id, rel);
        l.cards.insert(id, r);
        l.card_scroll.insert(id, (col_key, cy));
        drop(l);
        let moved = prev.is_none_or(|p| (0..4).any(|k| (p[k] - rel[k]).abs() > 0.5));
        if !(moved && self.board_selected() == Some(id)) {
            tx.live_only();
        }
    }

    /// Records an expanded subtask container's rect from `El::on_layout_tx` (bookkeeping only).
    pub fn board_sub_list_laid_out(&self, parent_id: NodeId, r: [f32; 4], tx: &mut Tx) {
        self.board.layout.borrow_mut().sub_lists.insert(parent_id, r);
        tx.live_only();
    }

    /// Cancels any active board hold when the note changes outside the app (BRD-023).
    pub fn board_cancel_hold(&mut self) {
        self.board.hold = Hold::None;
    }

    /// Smoothly glides the board row and owning column by the least that shows board item `i`
    /// with breathing room (BRD-025, BRD-030), preserving full outer padding on first/last columns.
    pub fn board_scroll_to_item(&self, i: usize, t: &map::Tree, tx: &mut Tx) {
        let n = &t.nodes[i];
        let is_head = n.parent == Some(map::ROOT) && board::is_heading(t, i);
        let model = board::build(t);
        let col_id = if is_head {
            Some(n.id)
        } else {
            model.find_card_column(i).map(|ci| model.columns[ci].key_id(t))
        };
        let Some(cid) = col_id else { return };
        let reduced = crate::motion::reduced();
        let b = self.settings().size as f32;
        let outer_pad = crate::page::r(b * 16.0 / 14.0);
        let layout = self.board.layout.borrow();

        // 1. Horizontal canvas glide on `board-scroll` using scroll-invariant `board-row` coordinates.
        if let Some(&col_r) = layout.cols.get(&cid)
            && layout.view[2] > 0.0
        {
            let vw = layout.view[2];
            let cur_x = self.board.scroll_x.get();
            // `col_r` is the inner bordered rect (1 px inside `board-col-host`), so expand by 1 px.
            let col_left_in_row = col_r[0] - layout.row_origin[0] - 1.0;
            let col_right_in_row = col_left_in_row + col_r[2] + 2.0;
            let vis_left = col_left_in_row - cur_x;
            let vis_right = col_right_in_row - cur_x;
            let delta = if vis_left < outer_pad {
                vis_left - outer_pad
            } else if vis_right > vw - outer_pad {
                vis_right - (vw - outer_pad)
            } else {
                0.0
            };
            if delta.abs() > 0.5 {
                let max_x = (layout.row_w - vw).max(0.0);
                let next_x = (cur_x + delta).clamp(0.0, max_x);
                if (next_x - cur_x).abs() > 0.5 {
                    self.board.scroll_x.set(next_x);
                    if reduced {
                        tx.scroll_to_instant("board-scroll", next_x, 0.0);
                    } else {
                        tx.animated();
                        tx.scroll_to("board-scroll", next_x, 0.0);
                    }
                }
            }
        } else {
            tx.scroll_into_view(format!("board-col-host:{cid}"));
        }

        // 2. Vertical column glide on `board-col-scroll:{cid}` for cards.
        if !is_head {
            if let (Some(&card_r), Some(&cv)) = (layout.cards.get(&n.id), layout.col_views.get(&cid)) {
                let vpad = crate::ui::tokens::space::SCROLL_FADE_PANEL + crate::ui::tokens::space::PX_6;
                let cur_y = self.board.col_scroll.borrow().get(&cid).copied().unwrap_or(0.0);
                let laid_y = layout.card_scroll.get(&n.id).map_or(cur_y, |&(_, y)| y);
                let card_top_in_col = (card_r[1] - cv[1]) + laid_y;
                let vis_top = card_top_in_col - cur_y;
                let vis_bot = vis_top + card_r[3];
                let delta = if vis_top < vpad {
                    vis_top - vpad
                } else if vis_bot > cv[3] - vpad {
                    vis_bot - (cv[3] - vpad)
                } else {
                    0.0
                };
                if delta.abs() > 0.5 {
                    let content_h = layout.col_content_h.get(&cid).copied().unwrap_or(0.0);
                    let max_y = (content_h - cv[3]).max(0.0);
                    let next_y = (cur_y + delta).clamp(0.0, max_y);
                    if (next_y - cur_y).abs() > 0.5 {
                        self.board.col_scroll.borrow_mut().insert(cid, next_y);
                        let key = format!("board-col-scroll:{cid}");
                        if reduced {
                            tx.scroll_to_instant(key, 0.0, next_y);
                        } else {
                            tx.animated();
                            tx.scroll_to(key, 0.0, next_y);
                        }
                    }
                }
            } else {
                tx.scroll_into_view(card_key(n.id));
            }
        }
    }

    /// Selects item `i` in `t` and scrolls it into view (without stealing focus from Find).
    fn board_select_and_show(&mut self, i: usize, t: &map::Tree, tx: &mut Tx) {
        let id = t.nodes[i].id;
        self.board_select(id);
        if crate::focus::on_note(tx) {
            self.keys_to_note(tx);
        }
        self.board_scroll_to_item(i, t, tx);
    }

    /// `Up`/`k` (`dir = -1`) and `Down`/`j` (`dir = 1`) in Board mode (BRD-025), walking visible
    /// top-level cards and unfolded sub-cards in visual order.
    pub fn board_move_sel_v(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let expanded = self.board_expanded();
        let Some(cur) = self.board_selected_index(&t) else { return };
        if let Some(ci) = model.find_column(cur) {
            if dir > 0 {
                let vis = model.columns[ci].visible_cards(&t, &expanded, None);
                if let Some(&(first, _)) = vis.first() {
                    self.board_select_and_show(first, &t, tx);
                }
            }
            return;
        }
        let Some(ci) = model.find_card_column(cur) else { return };
        let vis = model.columns[ci].visible_cards(&t, &expanded, None);
        let Some(pos) = vis.iter().position(|&(n, _)| n == cur) else { return };
        if dir < 0 {
            if pos > 0 {
                self.board_select_and_show(vis[pos - 1].0, &t, tx);
            } else {
                self.board_select_and_show(model.columns[ci].heading, &t, tx);
            }
        } else if pos + 1 < vis.len() {
            self.board_select_and_show(vis[pos + 1].0, &t, tx);
        }
    }

    /// `Left`/`h` (`dir = -1`) and `Right`/`l` (`dir = 1`) in Board mode (BRD-025):
    /// from a card picks the visible card at the same vertical height in the adjacent column (else
    /// its last visible card, else its header); from a header moves to the adjacent column's header.
    pub fn board_move_sel_h(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let expanded = self.board_expanded();
        let Some(cur) = self.board_selected_index(&t) else { return };
        let (ci, is_card) = if let Some(ci) = model.find_column(cur) {
            (ci, false)
        } else if let Some(ci) = model.find_card_column(cur) {
            (ci, true)
        } else {
            return;
        };
        let Some(next_ci) = ci.checked_add_signed(dir) else { return };
        let Some(next_col) = model.columns.get(next_ci) else { return };

        let target = if is_card {
            let vis = next_col.visible_cards(&t, &expanded, None);
            if vis.is_empty() {
                Some(next_col.heading)
            } else {
                let layout = self.board.layout.borrow();
                let cur_id = t.nodes[cur].id;
                if let Some(&cur_r) = layout.cards.get(&cur_id) {
                    let cur_y = cur_r[1] + cur_r[3] * 0.5;
                    vis.iter()
                        .map(|&(c, _)| c)
                        .find(|&c| layout.cards.get(&t.nodes[c].id).is_some_and(|r| cur_y >= r[1] && cur_y <= r[1] + r[3]))
                        .or_else(|| {
                            vis.iter()
                                .map(|&(c, _)| c)
                                .filter_map(|c| {
                                    let r = layout.cards.get(&t.nodes[c].id)?;
                                    Some((c, ((r[1] + r[3] * 0.5) - cur_y).abs()))
                                })
                                .min_by(|a, b| a.1.total_cmp(&b.1))
                                .map(|(c, _)| c)
                        })
                        .or_else(|| vis.last().map(|&(c, _)| c))
                } else {
                    let cur_vis = model.columns[ci].visible_cards(&t, &expanded, None);
                    let ki = cur_vis.iter().position(|&(c, _)| c == cur).unwrap_or(0);
                    vis.get(ki).or_else(|| vis.last()).map(|&(c, _)| c)
                }
            }
        } else {
            Some(next_col.heading)
        };
        if let Some(target) = target {
            self.board_select_and_show(target, &t, tx);
        }
    }

    /// `Home` (BRD-025): selects the first column's header.
    pub fn board_home(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        if let Some(first) = model.columns.first() {
            self.board_select_and_show(first.heading, &t, tx);
        }
    }

    /// `Alt+Up` (`dir = -1`) and `Alt+Down` (`dir = 1`): moves the selected card (or sub-card)
    /// one place within its parent list (BRD-026).
    pub fn board_move_card_v(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        let Some((parent, ki)) = model.find_card_slot(cur) else { return };
        let count = model.rest(parent, None).len();
        let Some(next_ki) = ki.checked_add_signed(dir) else { return };
        if next_ki >= count {
            return;
        }
        let slot = Slot { parent, index: next_ki };
        match board::slot_add(&model, &t, cur, slot) {
            Ok(add) => self.board_run_move(&t, cur, add, tx),
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `Alt+Left` (`dir = -1`) and `Alt+Right` (`dir = 1`): moves the selected card (or sub-card)
    /// to the end of the adjacent column, or moves the selected column left/right (BRD-026).
    pub fn board_move_h(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        if let Some(ci) = model.find_column(cur) {
            let Some(next_ci) = ci.checked_add_signed(dir) else { return };
            if next_ci >= model.columns.len() {
                return;
            }
            match board::column_add(&model, &t, cur, next_ci) {
                Ok(add) => self.board_run_move(&t, cur, add, tx),
                Err(Some(why)) => self.toast(why, tx),
                Err(None) => {}
            }
            return;
        }
        let Some(ci) = model.find_card_column(cur) else { return };
        let Some(next_ci) = ci.checked_add_signed(dir) else { return };
        let Some(target_col) = model.columns.get(next_ci) else { return };
        let want = Slot { parent: SlotParent::Column(next_ci), index: target_col.cards.len() };
        if let Some(allowed) = board::nearest_allowed(&model, &t, cur, want) {
            match board::slot_add(&model, &t, cur, allowed) {
                Ok(add) => self.board_run_move(&t, cur, add, tx),
                Err(Some(why)) => self.toast(why, tx),
                Err(None) => {}
            }
        } else if let Some(why) = board::slot_refusal(&model, &t, cur, want) {
            self.toast(why, tx);
        }
    }

    /// Moves card `id` to the end of the column identified by `target_col_key_id` (BRD-028).
    pub fn board_move_card_to_column(&mut self, id: NodeId, target_col_key_id: u64, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(cur) = t.find(id) else { return };
        let model = board::build(&t);
        let Some(ci) = model.columns.iter().position(|c| c.key_id(&t) == target_col_key_id) else { return };
        let want = Slot { parent: SlotParent::Column(ci), index: model.columns[ci].cards.len() };
        if let Some(allowed) = board::nearest_allowed(&model, &t, cur, want) {
            if !board::is_same_slot(&model, cur, allowed) {
                match board::slot_add(&model, &t, cur, allowed) {
                    Ok(add) => self.board_run_move(&t, cur, add, tx),
                    Err(Some(why)) => self.toast(why, tx),
                    Err(None) => {}
                }
            }
        } else if let Some(why) = board::slot_refusal(&model, &t, cur, want) {
            self.toast(why, tx);
        }
    }

    fn board_run_move(&mut self, t: &map::Tree, cur: usize, add: map::Add, tx: &mut Tx) {
        match map::move_to(self.active_text(), t, cur, add) {
            Ok(splice) => {
                self.board_apply(t, &splice, tx);
                let t2 = self.map_tree();
                if let Some(sel) = self.board_selected_index(&t2) {
                    self.board_scroll_to_item(sel, &t2, tx);
                }
            }
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `x` (BRD-027): toggles task checkbox on the currently selected card.
    pub fn board_toggle_selected_task(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        if let Some(id) = self.board_selected() {
            self.board_toggle_task(id, tx);
        }
    }

    /// `Delete` / `Backspace` (BRD-027): deletes the selected card with Map's toast, or refuses on
    /// a column header with the exact spec message.
    pub fn board_delete(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        if t.nodes[i].parent == Some(map::ROOT) && board::is_heading(&t, i) {
            self.toast("Delete a column in Source or on the map", tx);
            return;
        }
        match map::delete(self.active_text(), &t, i) {
            Ok((s, n)) => {
                self.board_apply(&t, &s, tx);
                if n > 1 || t.nodes[i].card() {
                    let nodes = if n == 1 { "1 node".to_string() } else { format!("{n} nodes") };
                    let undo = self.keymap.label("board.undo");
                    self.toast(&format!("Deleted {nodes} · {undo} to undo"), tx);
                }
            }
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `Ctrl+C` (BRD-027): copies the selected card's Markdown (or column header's own line).
    pub fn board_copy(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        let is_col_head = t.nodes[i].parent == Some(map::ROOT) && board::is_heading(&t, i);
        let md = map::markdown(self.active_text(), &t, i, !is_col_head);
        tx.copy(md);
        self.toast("Copied", tx);
    }

    /// `Ctrl+Z` and `Ctrl+Shift+Z` / `Ctrl+Y` (BRD-027): note's shared undo/redo history.
    pub fn board_undo(&mut self, redo: bool, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let rel = self.active.clone();
        let old = self.map_tree();
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
        let s = map::diff(&before, self.active_text());
        let t = self.map_tree();
        if let Some(v) = self.map_view_mut() {
            v.follow(&old, &t, &s);
        }
        if let Some(rel) = rel {
            self.board_follow_expanded(&rel, &old, &t, &s);
        }
        self.board_enforce_kanban(Some(tx));
        if self.mode != Mode::Board {
            return;
        }
        let model = board::build(&t);
        let expanded = self.board_expanded();
        let changed = t.nodes.iter().position(|n| !n.own.is_empty() && s.written().contains(&n.own.start));
        let item = changed
            .and_then(|c| board::item_of(&t, &model, &expanded, c))
            .or_else(|| self.board_selected_index(&t));
        if let Some(item) = item {
            self.board_select_and_show(item, &t, tx);
        }
    }

    /// `Ctrl+Enter` (BRD-027): follows the selected card's first link.
    pub fn board_follow_link(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        let map::Body::Runs(runs) = &t.nodes[i].body else { return };
        match runs.iter().filter(|r| r.img.is_none()).find_map(|r| r.link.clone()) {
            Some(crate::blocks::Link::Wiki(w)) => self.open_link(w.trim(), tx),
            Some(crate::blocks::Link::Anchor(a)) => self.go_anchor(&a, tx),
            Some(crate::blocks::Link::Tag(tag)) => self.search_tag(&tag, tx),
            Some(crate::blocks::Link::Url(u)) => self.follow(&u, tx),
            None => {}
        }
    }

    /// `Esc` in Board mode (BRD-019, BRD-030): cancels an active hold, else dismisses notice,
    /// else closes Find, else runs `app.escape`.
    pub fn board_escape(&mut self, tx: &mut Tx) -> bool {
        if self.board_escape_hold(tx) {
            return true;
        }
        if self.notice.take().is_some() {
            return true;
        }
        if self.reader.find.open {
            self.close_find(tx);
            return true;
        }
        self.escape(tx)
    }

    /// Opens the card context menu at `at` (or below the card for keyboard Shift+F10, BRD-028).
    pub fn board_card_menu(&mut self, id: NodeId, at: Option<(f32, f32)>, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = t.find(id) else { return };
        let model = board::build(&t);
        if model.card(i).is_none() {
            return;
        }
        self.board_select(id);
        let pos = at.unwrap_or_else(|| {
            self.board
                .layout
                .borrow()
                .cards
                .get(&id)
                .map_or((300.0, 140.0), |r| (r[0] + 12.0, r[1] + r[3] + 4.0))
        });
        self.open_menu(crate::layer::MenuOf::BoardCard(id), pos, tx);
    }

    /// Find matches on the board: column titles and all task cards (including folded subtasks) in
    /// note order (BRD-030).
    pub fn board_hits(&self) -> Vec<crate::map_find::Hit> {
        let Some(q) = self.find_query() else { return Vec::new() };
        let t = self.map_tree();
        let model = board::build(&t);
        let mut out = Vec::new();
        let mut visit = |i: usize| {
            let n = &t.nodes[i];
            if let Some(text) = crate::map_find::label_text(n) {
                out.extend((0..crate::writer_app::find_in(&text, q).len()).map(|k| crate::map_find::Hit { node: i, pid: None, k }));
            }
        };
        for col in &model.columns {
            visit(col.heading);
            for c in col.all_cards() {
                visit(c);
            }
        }
        out
    }

    /// First board match at or after the selected board item (BRD-030).
    pub(crate) fn board_first_hit(&self, hits: &[crate::map_find::Hit]) -> usize {
        let t = self.map_tree();
        let sel = self.board_selected_index(&t).unwrap_or(0);
        hits.iter().position(|h| h.node >= sel).unwrap_or(0)
    }

    /// Selects and scrolls to the current Board Find match, unfolding ancestor cards if needed.
    pub(crate) fn board_show_hit(&mut self, tx: &mut Tx) {
        let hits = self.board_hits();
        if let Some(h) = hits.get(self.reader.find.cur) {
            let t = self.map_tree();
            let model = board::build(&t);
            if h.node < t.nodes.len() {
                self.board_expand_ancestors(h.node, &t, &model);
                self.board_select_and_show(h.node, &t, tx);
            }
        }
    }

    /// Outline click in Board mode (BRD-030): selects that heading's column header and scrolls it
    /// into view.
    pub fn board_jump(&mut self, line: usize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let expanded = self.board_expanded();
        if let Some(item) = board::item_of(&t, &model, &expanded, t.at_line(line)) {
            self.board_select_and_show(item, &t, tx);
            self.keys_to_note(tx);
        }
    }
}

/// Stable element key for a board column.
pub fn col_key(id: NodeId) -> String {
    format!("board-col:{id}")
}

/// Stable element key for a board column header.
pub fn head_key(id: NodeId) -> String {
    format!("board-head:{id}")
}

/// Stable element key for a board card (keyed by `NodeId`, never by source line).
pub fn card_key(id: NodeId) -> String {
    format!("board-card:{id}")
}
