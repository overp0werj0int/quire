//! Board mode state and actions on `App` (`docs/reader-writer/behavior/board.md`, decision 0015).
//!
//! Selection is shared with Map mode (`MapView::selected`); edits go through `map.rs` and
//! `App::map_apply` so undo, save, locked notes and sync work identically.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use rune::{Drag, Phase, Tx};

use crate::app::{App, Mode};
use crate::board::{self, Board, ColumnHead};
use crate::board_drag::Hold;
use crate::map::{self, NodeId};

/// Layout rectangles reported by Rune (`El::on_layout`) during the last frame, in window points.
#[derive(Default)]
pub struct BoardLayout {
    /// Board viewport `[x, y, w, h]` in window points.
    pub view: [f32; 4],
    /// Total laid-out width of `board-row` for clamping horizontal scroll.
    pub row_w: f32,
    /// Column outer rects by column key id (`0` for Loose, heading `NodeId` otherwise).
    pub cols: HashMap<u64, [f32; 4]>,
    /// Column header rects by column key id.
    pub heads: HashMap<u64, [f32; 4]>,
    /// Column scroll-area rects by column key id.
    pub col_views: HashMap<u64, [f32; 4]>,
    /// Column card-stack content height by column key id, for clamping vertical scroll.
    pub col_content_h: HashMap<u64, f32>,
    /// Card rects by `NodeId`.
    pub cards: HashMap<NodeId, [f32; 4]>,
}

/// Board mode state held on `App`.
pub struct BoardState {
    /// Last linear mode (`Source` or `Preview`), for `Ctrl+Shift+K` toggle back (BRD-029).
    pub back: Mode,
    /// Active drag / settle state machine.
    pub hold: Hold,
    /// Suppresses the synthetic click that follows a dropped drag.
    pub dropped: bool,
    /// Layout reports from Rune (`El::on_layout`).
    pub layout: RefCell<BoardLayout>,
    /// Horizontal pan state while dragging empty board space `(start_scroll_x, start_pointer_x)`.
    pub pan_drag: Option<(f32, f32)>,
    /// Current horizontal scroll offset of `board-scroll`.
    pub scroll_x: Rc<Cell<f32>>,
    /// Current vertical scroll offset per column (keyed by column key id).
    pub col_scroll: Rc<RefCell<HashMap<u64, f32>>>,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState {
            back: Mode::Source,
            hold: Hold::None,
            dropped: false,
            layout: RefCell::new(BoardLayout::default()),
            pan_drag: None,
            scroll_x: Rc::new(Cell::new(0.0)),
            col_scroll: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl App {
    /// True when Board mode shows an open, unlocked note.
    pub fn board_editable(&self) -> bool {
        self.mode == Mode::Board && self.active.is_some() && !self.active_locked()
    }

    /// Builds the current board projection (including an empty Loose column while a compatible
    /// card is held, BRD-005).
    pub fn board_model(&self) -> (std::rc::Rc<map::Tree>, Board) {
        let t = self.map_tree();
        let held = self.board.hold.held_card().and_then(|id| t.find(id));
        let b = board::build_with_held(&t, held);
        (t, b)
    }

    /// `Ctrl+Shift+K` (`app.mode-board`, BRD-029): switches to Board from any mode, and from Board
    /// back to the last linear mode (`Source` or `Preview`).
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
        } else {
            self.show_mode(Mode::Board, tx);
        }
    }

    /// Enters Board mode, keeping selection synced with Map/Source/Preview (BRD-024).
    pub fn enter_board(&mut self, tx: &mut Tx) {
        if self.mode == Mode::Board {
            return;
        }
        tx.clear_text_selection();
        let from = self.mode;
        if matches!(from, Mode::Source | Mode::Preview) {
            self.board.back = from;
        }
        let line = if !self.active_locked() && matches!(from, Mode::Source | Mode::Preview) {
            Some(self.linear_place_line())
        } else {
            None
        };
        self.mode = Mode::Board;
        self.board.hold = Hold::None;
        self.board.pan_drag = None;
        self.close_find(tx);
        self.keys_to_note(tx);
        if self.active_locked() {
            return;
        }
        let t = self.map_tree();
        let b = board::build(&t);
        if let Some(l) = line {
            let at = t.at_line(l);
            if let Some(item) = board::item_of(&t, at).or_else(|| board::default_selection(&b))
                && let Some(v) = self.map_view_mut()
            {
                v.selected = t.nodes[item].id;
            }
        } else {
            // Coming from Map: keep `MapView::selected`, projecting nested nodes to their card.
            self.board_ensure_selection();
        }
    }

    /// Leaves Board mode for `to`, keeping the selected node's line (BRD-024).
    pub fn leave_board(&mut self, to: Mode, tx: &mut Tx) {
        if self.mode != Mode::Board || to == Mode::Board {
            return;
        }
        self.board.hold = Hold::None;
        self.board.pan_drag = None;
        let line = if self.active_locked() {
            0
        } else {
            let t = self.map_tree();
            let sel = self.board_selected_index(&t).unwrap_or(map::ROOT);
            t.nodes[sel].own.start
        };
        if to == Mode::Map {
            self.mode = Mode::Map;
            self.close_find(tx);
            self.keys_to_note(tx);
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

    /// Selected board item index in `t`, reading `MapView::selected` and projecting any nested
    /// node to its owning card (BRD-024), falling back to the first card or column header.
    pub fn board_selected_index(&self, t: &map::Tree) -> Option<usize> {
        if let Some(v) = self.map_view()
            && let Some(i) = t.find(v.selected)
            && let Some(item) = board::item_of(t, i)
        {
            return Some(item);
        }
        let b = board::build(t);
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

    /// Clicks a card or column header (ignoring the synthetic press right after a drop).
    pub fn board_click(&mut self, id: NodeId, double: bool, tx: &mut Tx) {
        if self.board.dropped {
            self.board.dropped = false;
            return;
        }
        self.board_select(id);
        self.keys_to_note(tx);
        if double {
            self.board_edit_selected(tx);
        }
    }

    /// Opens the Edit Sheet on the selected card's `subtree_lines` or column header's own line
    /// (BRD-027).
    pub fn board_edit_selected(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        let n = &t.nodes[i];
        let is_col_head = n.parent == Some(map::ROOT) && board::is_heading(&t, i);
        let lines = if is_col_head { n.own.clone() } else { n.span.clone() };
        if !lines.is_empty() {
            self.open_sheet_lines(lines, crate::sheet::Scope::Block, tx);
        }
    }

    /// Toggles task state on node `i` (BRD-011, BRD-027).
    pub fn board_toggle_task(&mut self, i: usize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        if let Some(s) = map::toggle_task(self.active_text(), &t, i) {
            self.board_apply(&t, &s, tx);
        }
    }

    /// Applies a `map::Splice` from Board mode through the map's single-undo-step pipeline
    /// (BRD-019, MAP-013).
    pub fn board_apply(&mut self, old: &map::Tree, s: &map::Splice, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        let base = self.active_text().to_string();
        self.splice_into(&rel, &base, s);
        let t = self.map_tree();
        if let Some(v) = self.map_view_mut() {
            v.follow(old, &t, s);
        }
        let focus = s.focus.map_or(map::ROOT, |l| t.at_line(l));
        if let Some(item) = board::item_of(&t, focus)
            && let Some(v) = self.map_view_mut()
        {
            v.selected = t.nodes[item].id;
        }
        let _ = tx;
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
                    let next = (from_x - (d.window_point()[0] - start_px)).max(0.0);
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

    /// Records the board viewport rect from `El::on_layout`.
    pub fn board_view_laid_out(&self, r: [f32; 4]) {
        self.board.layout.borrow_mut().view = r;
    }

    /// Records the board row's total width from `El::on_layout`.
    pub fn board_row_laid_out(&self, w: f32) {
        self.board.layout.borrow_mut().row_w = w;
    }

    /// Records a column's outer rect from `El::on_layout`.
    pub fn board_col_laid_out(&self, col_key: u64, r: [f32; 4]) {
        self.board.layout.borrow_mut().cols.insert(col_key, r);
    }

    /// Records a column header's rect from `El::on_layout`.
    pub fn board_head_laid_out(&self, col_key: u64, r: [f32; 4]) {
        self.board.layout.borrow_mut().heads.insert(col_key, r);
    }

    /// Records a column's cards scroll viewport rect from `El::on_layout`.
    pub fn board_col_view_laid_out(&self, col_key: u64, r: [f32; 4]) {
        self.board.layout.borrow_mut().col_views.insert(col_key, r);
    }

    /// Records a column's card-stack content height from `El::on_layout`.
    pub fn board_col_content_laid_out(&self, col_key: u64, h: f32) {
        self.board.layout.borrow_mut().col_content_h.insert(col_key, h);
    }

    /// Records a card's rect from `El::on_layout`.
    pub fn board_card_laid_out(&self, id: NodeId, r: [f32; 4]) {
        self.board.layout.borrow_mut().cards.insert(id, r);
    }

    /// Cancels any active board hold when the note changes outside the app (BRD-023).
    pub fn board_cancel_hold(&mut self) {
        self.board.hold = Hold::None;
    }

    /// Scrolls the board and column by the least that shows board item `i` (BRD-025, BRD-030).
    pub fn board_scroll_to_item(&self, i: usize, t: &map::Tree, tx: &mut Tx) {
        let n = &t.nodes[i];
        if n.parent == Some(map::ROOT) && board::is_heading(t, i) {
            tx.scroll_into_view(head_key(ColumnHead::Heading(i), t));
        } else {
            tx.scroll_into_view(card_key(n.id));
        }
    }

    /// Selects item `i` in `t` and scrolls it into view (without stealing focus from Find).
    fn board_select_and_show(&mut self, i: usize, t: &map::Tree, tx: &mut Tx) {
        let id = t.nodes[i].id;
        self.board_select(id);
        self.board_scroll_to_item(i, t, tx);
    }

    /// `Up`/`k` (`dir = -1`) and `Down`/`j` (`dir = 1`) in Board mode (BRD-025).
    pub fn board_move_sel_v(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        if let Some((ci, ki)) = model.find_card(cur) {
            let col = &model.columns[ci];
            if dir < 0 {
                if ki > 0 {
                    self.board_select_and_show(col.cards[ki - 1], &t, tx);
                } else if let ColumnHead::Heading(h) = col.head {
                    self.board_select_and_show(h, &t, tx);
                }
            } else if ki + 1 < col.cards.len() {
                self.board_select_and_show(col.cards[ki + 1], &t, tx);
            }
        } else if let Some(ci) = model.find_column(cur)
            && dir > 0
            && let Some(&first) = model.columns[ci].cards.first()
        {
            self.board_select_and_show(first, &t, tx);
        }
    }

    /// `Left`/`h` (`dir = -1`) and `Right`/`l` (`dir = 1`) in Board mode (BRD-025):
    /// from a card picks the card at the same vertical height in the adjacent column (else its last
    /// card, else its header); from a header moves to the adjacent column's header (or card if Loose).
    pub fn board_move_sel_h(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        let (ci, card_pos) = if let Some((ci, ki)) = model.find_card(cur) {
            (ci, Some(ki))
        } else if let Some(ci) = model.find_column(cur) {
            (ci, None)
        } else {
            return;
        };
        let Some(next_ci) = ci.checked_add_signed(dir) else { return };
        let Some(next_col) = model.columns.get(next_ci) else { return };

        let target = if let Some(ki) = card_pos {
            if next_col.cards.is_empty() {
                match next_col.head {
                    ColumnHead::Heading(h) => Some(h),
                    ColumnHead::Loose => None,
                }
            } else {
                let layout = self.board.layout.borrow();
                let cur_id = t.nodes[cur].id;
                if let Some(&cur_r) = layout.cards.get(&cur_id) {
                    let cur_y = cur_r[1] + cur_r[3] * 0.5;
                    // Prefer a card whose vertical span contains cur_y, else nearest center
                    // (which naturally selects the last card when cur_y is below all cards).
                    let hit = next_col
                        .cards
                        .iter()
                        .copied()
                        .find(|&c| layout.cards.get(&t.nodes[c].id).is_some_and(|r| cur_y >= r[1] && cur_y <= r[1] + r[3]))
                        .or_else(|| {
                            next_col
                                .cards
                                .iter()
                                .copied()
                                .filter_map(|c| {
                                    let r = layout.cards.get(&t.nodes[c].id)?;
                                    Some((c, ((r[1] + r[3] * 0.5) - cur_y).abs()))
                                })
                                .min_by(|a, b| a.1.total_cmp(&b.1))
                                .map(|(c, _)| c)
                        })
                        .or_else(|| next_col.cards.last().copied());
                    hit
                } else {
                    next_col.cards.get(ki).or_else(|| next_col.cards.last()).copied()
                }
            }
        } else {
            match next_col.head {
                ColumnHead::Heading(h) => Some(h),
                ColumnHead::Loose => next_col.cards.first().copied(),
            }
        };
        if let Some(target) = target {
            self.board_select_and_show(target, &t, tx);
        }
    }

    /// `Home` (BRD-025): selects the first column's header (or first loose card if no headings).
    pub fn board_home(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let target = model
            .columns
            .iter()
            .find_map(|c| match c.head {
                ColumnHead::Heading(h) => Some(h),
                ColumnHead::Loose => None,
            })
            .or_else(|| board::default_selection(&model));
        if let Some(target) = target {
            self.board_select_and_show(target, &t, tx);
        }
    }

    /// `Alt+Up` (`dir = -1`) and `Alt+Down` (`dir = 1`): moves the selected card one place in its
    /// column (BRD-026).
    pub fn board_move_card_v(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        let Some((ci, ki)) = model.find_card(cur) else { return };
        let Some(next_ki) = ki.checked_add_signed(dir) else { return };
        if next_ki >= model.columns[ci].cards.len() {
            return;
        }
        let slot = board::Slot { column: ci, index: next_ki };
        match board::slot_add(&model, &t, cur, slot) {
            Ok(add) => self.board_run_move(&t, cur, add, tx),
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `Alt+Left` (`dir = -1`) and `Alt+Right` (`dir = 1`): moves the selected card to the end of
    /// the adjacent column, or moves the selected column left/right (BRD-026).
    pub fn board_move_h(&mut self, dir: isize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let model = board::build(&t);
        let Some(cur) = self.board_selected_index(&t) else { return };
        if let Some(ci) = model.find_column(cur) {
            // Column header move among heading columns.
            let heads: Vec<usize> = model
                .columns
                .iter()
                .filter_map(|c| match c.head {
                    ColumnHead::Heading(h) => Some(h),
                    ColumnHead::Loose => None,
                })
                .collect();
            let Some(hpos) = heads.iter().position(|&h| h == cur) else { return };
            let Some(next_hpos) = hpos.checked_add_signed(dir) else { return };
            if next_hpos >= heads.len() {
                return;
            }
            let _ = ci;
            match board::column_add(&model, &t, cur, next_hpos) {
                Ok(add) => self.board_run_move(&t, cur, add, tx),
                Err(Some(why)) => self.toast(why, tx),
                Err(None) => {}
            }
            return;
        }
        let Some((ci, _)) = model.find_card(cur) else { return };
        let Some(next_ci) = ci.checked_add_signed(dir) else { return };
        let Some(target_col) = model.columns.get(next_ci) else { return };
        let want = board::Slot { column: next_ci, index: target_col.cards.len() };
        if let Some(allowed) = board::nearest_allowed(&model, &t, cur, want) {
            match board::slot_add(&model, &t, cur, allowed) {
                Ok(add) => self.board_run_move(&t, cur, add, tx),
                Err(Some(why)) => self.toast(why, tx),
                Err(None) => {}
            }
        } else if let Some(why) = board::column_refusal(&model, &t, cur, want) {
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
        let Some(ci) = model.columns.iter().position(|c| c.head.key_id(&t) == target_col_key_id) else { return };
        let want = board::Slot { column: ci, index: model.columns[ci].cards.len() };
        if let Some(allowed) = board::nearest_allowed(&model, &t, cur, want) {
            if !board::is_same_slot(&model, cur, allowed) {
                match board::slot_add(&model, &t, cur, allowed) {
                    Ok(add) => self.board_run_move(&t, cur, add, tx),
                    Err(Some(why)) => self.toast(why, tx),
                    Err(None) => {}
                }
            }
        } else if let Some(why) = board::column_refusal(&model, &t, cur, want) {
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
        let t = self.map_tree();
        if let Some(i) = self.board_selected_index(&t) {
            self.board_toggle_task(i, tx);
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
        let changed = t.nodes.iter().position(|n| !n.own.is_empty() && s.written().contains(&n.own.start));
        let item = changed
            .and_then(|c| board::item_of(&t, c))
            .or_else(|| self.board_selected_index(&t));
        if let Some(item) = item {
            self.board_select_and_show(item, &t, tx);
        }
    }

    /// `Ctrl+Enter` (BRD-027): opens lightbox on an image card, else follows the card's first link.
    pub fn board_follow_link(&mut self, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        let Some(i) = self.board_selected_index(&t) else { return };
        if self.open_lightbox(i, 0, tx) {
            return;
        }
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
        if model.find_card(i).is_none() {
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

    /// Find matches on the board: column titles and card labels/blocks in note order (BRD-030).
    pub fn board_hits(&self) -> Vec<crate::map_find::Hit> {
        let Some(q) = self.find_query() else { return Vec::new() };
        let t = self.map_tree();
        let model = board::build(&t);
        let mut out = Vec::new();
        let mut pieces = Vec::new();
        let mut visit = |i: usize| {
            let n = &t.nodes[i];
            if let map::Body::Card(bl) = &n.body {
                pieces.clear();
                crate::blocks::pieces_in(bl, &mut pieces);
                for (pid, text) in &pieces {
                    out.extend((0..crate::writer_app::find_in(text, q).len()).map(|k| crate::map_find::Hit { node: i, pid: Some(*pid), k }));
                }
            } else if let Some(text) = crate::map_find::label_text(n) {
                out.extend((0..crate::writer_app::find_in(&text, q).len()).map(|k| crate::map_find::Hit { node: i, pid: None, k }));
            }
        };
        for col in &model.columns {
            if let ColumnHead::Heading(h) = col.head {
                visit(h);
            }
            for &c in &col.cards {
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

    /// Selects and scrolls to the current Board Find match (BRD-030).
    pub(crate) fn board_show_hit(&mut self, tx: &mut Tx) {
        let hits = self.board_hits();
        if let Some(h) = hits.get(self.reader.find.cur) {
            let t = self.map_tree();
            if h.node < t.nodes.len() {
                self.board_select_and_show(h.node, &t, tx);
            }
        }
    }

    /// Outline click in Board mode (BRD-030): selects that heading's column header or section card
    /// and scrolls it into view.
    pub fn board_jump(&mut self, line: usize, tx: &mut Tx) {
        if !self.board_editable() {
            return;
        }
        let t = self.map_tree();
        if let Some(item) = board::item_of(&t, t.at_line(line)) {
            self.board_select_and_show(item, &t, tx);
            self.keys_to_note(tx);
        }
    }
}

/// Stable element key for a board column.
pub fn col_key(head: ColumnHead, t: &map::Tree) -> String {
    match head {
        ColumnHead::Loose => "board-col:loose".into(),
        ColumnHead::Heading(h) => format!("board-col:{}", t.nodes[h].id),
    }
}

/// Stable element key for a board column header.
pub fn head_key(head: ColumnHead, t: &map::Tree) -> String {
    match head {
        ColumnHead::Loose => "board-head:loose".into(),
        ColumnHead::Heading(h) => format!("board-head:{}", t.nodes[h].id),
    }
}

/// Stable element key for a board card (keyed by `NodeId`, never by source line).
pub fn card_key(id: NodeId) -> String {
    format!("board-card:{id}")
}
