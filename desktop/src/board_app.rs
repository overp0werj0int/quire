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
    /// Column outer rects by column key id (`0` for Loose, heading `NodeId` otherwise).
    pub cols: HashMap<u64, [f32; 4]>,
    /// Column header rects by column key id.
    pub heads: HashMap<u64, [f32; 4]>,
    /// Column scroll-area rects by column key id.
    pub col_views: HashMap<u64, [f32; 4]>,
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

    /// Selects a card or column header by its `NodeId` (BRD-012, BRD-024).
    pub fn board_select(&mut self, id: NodeId, tx: &mut Tx) {
        if let Some(v) = self.map_view_mut() {
            v.selected = id;
        }
        self.keys_to_note(tx);
    }

    /// Clicks a card or column header (ignoring the synthetic press right after a drop).
    pub fn board_click(&mut self, id: NodeId, double: bool, tx: &mut Tx) {
        if self.board.dropped {
            self.board.dropped = false;
            return;
        }
        self.board_select(id, tx);
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

    /// Records a card's rect from `El::on_layout`.
    pub fn board_card_laid_out(&self, id: NodeId, r: [f32; 4]) {
        self.board.layout.borrow_mut().cards.insert(id, r);
    }

    /// Cancels any active board hold when the note changes outside the app (BRD-023).
    pub fn board_cancel_hold(&mut self) {
        self.board.hold = Hold::None;
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
