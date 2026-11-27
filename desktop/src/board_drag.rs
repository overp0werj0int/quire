//! Board mode drag state machine and tilt physics (`docs/reader-writer/behavior/board.md`,
//! BRD-014 to BRD-023).
//!
//! One state machine (`Hold`) owns the lifecycle of pressing, lifting, carrying with tilt, and
//! settling a card or a column. The tilt rule is a pure function so tests and probes read the
//! exact angle in radians.

use std::time::Duration;

use rune::{Drag, Phase, Tx};

use crate::app::{App, Mode};
use crate::board::{self, Board, ColumnHead, Slot};
use crate::board_app::{card_key, head_key};
use crate::map::{self, NodeId};

/// Pointer travel in points before a press lifts a card or column (BRD-014, BRD-021).
pub const THRESHOLD: f32 = 6.0;
/// Base tilt angle on lift and at rest, in degrees (BRD-015).
pub const BASE_DEG: f32 = 3.0;
/// Maximum tilt angle at full sideways speed, in degrees (BRD-015).
pub const MAX_DEG: f32 = 6.0;
/// Horizontal pointer speed in px/s at which tilt reaches `MAX_DEG` (BRD-015).
pub const MAX_SPEED: f32 = 1200.0;
/// Horizontal travel under this many points between frames keeps the last sign (BRD-015).
pub const SIGN_EPS: f32 = 4.0;
/// Tilt spring duration in seconds (120 ms, BRD-015).
pub const TILT_SPRING: f32 = 0.12;
/// Settle glide duration in seconds (180 ms, BRD-019).
pub const SETTLE_SECS: f64 = 0.18;
/// Edge auto-scroll band width in points (BRD-018).
pub const EDGE: f32 = 36.0;
/// Maximum edge auto-scroll speed in points per second (BRD-018).
pub const EDGE_SPEED: f32 = 480.0;

/// What a board hold carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Card(NodeId),
    Column(NodeId),
}

impl Held {
    pub fn is_column(self) -> bool {
        matches!(self, Held::Column(_))
    }
}

/// The single state machine for dragging a card or column on the board.
#[derive(Clone, Debug, Default)]
pub enum Hold {
    #[default]
    None,
    /// Pointer is down on a card or column header, not yet past `THRESHOLD`.
    Pressed {
        what: Held,
        /// Note identity and text revision when the press began (A2, A3).
        origin: crate::app::TextKey,
        start: [f32; 2],
        grab: [f32; 2],
        size: [f32; 2],
        /// Original card/column top-left in board-viewport coordinates, for cancel settle.
        origin_pos: [f32; 2],
    },
    /// Lifted and following the pointer with a tilt and live drop slot.
    Held {
        what: Held,
        /// Note identity and text revision when the hold began (A2, A3).
        origin: crate::app::TextKey,
        /// Offset from the held card/column's top-left corner to the pressed point.
        grab: [f32; 2],
        /// Pointer position in board-viewport coordinates.
        at: [f32; 2],
        /// Held element's laid-out `[w, h]`.
        size: [f32; 2],
        /// Original top-left before lift, in board-viewport coordinates.
        origin_pos: [f32; 2],
        /// Target slot for a card drag.
        slot: Option<Slot>,
        /// Target column index among remaining heading columns for a column drag.
        col_slot: Option<usize>,
        /// Refusal reason when the hovered column allows no slot.
        refused: Option<&'static str>,
        /// Signed tilt in radians (clockwise positive).
        lean: f32,
        /// Last non-zero horizontal travel sign (`+1.0` clockwise, `-1.0` counter-clockwise).
        sign: f32,
        /// Last pointer `x` and clock time for velocity calculation.
        last_x: f32,
        last_t: f64,
        /// Last time edge scroll stepped.
        tick: f64,
    },
    /// Gliding from release position to target slot while turning upright over `SETTLE_SECS`.
    Settling {
        what: Held,
        /// Note identity and text revision this settle belongs to (A2).
        origin: crate::app::TextKey,
        from: [f32; 2],
        to: [f32; 2],
        size: [f32; 2],
        grab: [f32; 2],
        from_lean: f32,
        start: f64,
    },
}

impl Hold {
    pub fn active(&self) -> bool {
        matches!(self, Hold::Held { .. })
    }

    pub fn held_card(&self) -> Option<NodeId> {
        match self {
            Hold::Held { what: Held::Card(id), .. } => Some(*id),
            _ => None,
        }
    }

    pub fn settling_card(&self) -> Option<NodeId> {
        match self {
            Hold::Settling { what: Held::Card(id), .. } => Some(*id),
            _ => None,
        }
    }

    pub fn held_column(&self) -> Option<NodeId> {
        match self {
            Hold::Held { what: Held::Column(id), .. } => Some(*id),
            _ => None,
        }
    }

    pub fn settling_column(&self) -> Option<NodeId> {
        match self {
            Hold::Settling { what: Held::Column(id), .. } => Some(*id),
            _ => None,
        }
    }

    pub fn slot(&self) -> Option<Slot> {
        match self {
            Hold::Held { slot, .. } => *slot,
            _ => None,
        }
    }

    pub fn col_slot(&self) -> Option<usize> {
        match self {
            Hold::Held { col_slot, .. } => *col_slot,
            _ => None,
        }
    }
}

/// Pure tilt rule (BRD-015, BRD-021, BRD-022):
/// Given the previous sign (`+1.0` or `-1.0`), frame horizontal displacement `dx` in px,
/// horizontal speed `vx` in px/s, `reduced` motion flag, and `half` (true for columns, BRD-021),
/// returns `(signed_radians, next_sign)`.
pub fn tilt_radians(last_sign: f32, dx: f32, vx: f32, reduced: bool, half: bool) -> (f32, f32) {
    let scale = if half { 0.5 } else { 1.0 };
    if reduced {
        return ((BASE_DEG * scale).to_radians(), 1.0);
    }
    let sign = if dx.abs() >= SIGN_EPS {
        dx.signum()
    } else if last_sign == 0.0 {
        1.0
    } else {
        last_sign
    };
    let frac = (vx.abs() / MAX_SPEED).clamp(0.0, 1.0);
    let deg = (BASE_DEG + (MAX_DEG - BASE_DEG) * frac) * scale;
    (sign * deg.to_radians(), sign)
}

/// Ease-out cubic curve for settle glide (0..=1 -> 0..=1).
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

impl App {
    /// Pointer position relative to `board-canvas`'s top-left corner.
    fn board_point(&self, d: &Drag) -> [f32; 2] {
        let [wx, wy] = d.window_point();
        let v = self.board.layout.borrow().view;
        [wx - v[0], wy - v[1]]
    }

    /// True when the active hold's page context has changed (note, text revision, vault, lock
    /// state, mode, screen, or an open layer) and the hold must end with nothing written (A2, A3).
    pub(crate) fn board_hold_stale(&self) -> bool {
        match &self.board.hold {
            Hold::None => false,
            Hold::Pressed { origin, .. } | Hold::Held { origin, .. } => {
                !self.board_editable() || !self.layers.is_empty() || !origin.matches(self.active_text_key())
            }
            Hold::Settling { origin, .. } => {
                !self.board_editable() || !origin.matches(self.active_text_key())
            }
        }
    }

    /// Drag handler on a board card (BRD-014 to BRD-020, BRD-022, BRD-023).
    pub fn board_card_drag(&mut self, id: NodeId, d: &Drag, tx: &mut Tx) {
        let pt = self.board_point(d);
        let now = self.now.get();
        match d.phase {
            Phase::Down => {
                if !self.board_editable() || !self.layers.is_empty() {
                    return;
                }
                let origin = self.active_text_key().to_owned();
                let layout = self.board.layout.borrow();
                let view = layout.view;
                let rect = layout.cards.get(&id).copied().unwrap_or([view[0] + pt[0], view[1] + pt[1], 264.0, 40.0]);
                let local = [rect[0] - view[0], rect[1] - view[1]];
                let grab = [(pt[0] - local[0]).clamp(0.0, rect[2]), (pt[1] - local[1]).clamp(0.0, rect[3])];
                let size = [rect[2], rect[3]];
                drop(layout);
                self.board.hold = Hold::Pressed { what: Held::Card(id), origin, start: pt, grab, size, origin_pos: local };
                tx.live_only();
            }
            Phase::Move => {
                if self.board_hold_stale() {
                    self.board_cancel_hold();
                    return;
                }
                let reduced = crate::motion::reduced();
                match &mut self.board.hold {
                    Hold::Pressed { what: Held::Card(cid), origin, start, grab, size, origin_pos } if *cid == id => {
                        if (pt[0] - start[0]).hypot(pt[1] - start[1]) <= THRESHOLD {
                            tx.live_only();
                            return;
                        }
                        let (origin, grab, size, origin_pos) = (origin.clone(), *grab, *size, *origin_pos);
                        let (lean, sign) = (BASE_DEG.to_radians(), 1.0);
                        self.board.hold = Hold::Held {
                            what: Held::Card(id),
                            origin,
                            grab,
                            at: pt,
                            size,
                            origin_pos,
                            slot: None,
                            col_slot: None,
                            refused: None,
                            lean,
                            sign,
                            last_x: pt[0],
                            last_t: now,
                            tick: now,
                        };
                        if let Some(v) = self.map_view_mut() {
                            v.selected = id;
                        }
                        self.board_aim_card();
                        tx.wake_after(Duration::from_millis(16));
                    }
                    Hold::Held { what: Held::Card(cid), at, lean, sign, last_x, last_t, .. } if *cid == id => {
                        let dx = pt[0] - *last_x;
                        let dt = (now - *last_t).max(1.0 / 240.0) as f32;
                        let vx = dx / dt;
                        let (new_lean, new_sign) = tilt_radians(*sign, dx, vx, reduced, false);
                        *at = pt;
                        *lean = new_lean;
                        *sign = new_sign;
                        *last_x = pt[0];
                        *last_t = now;
                        self.board_aim_card();
                        tx.wake_after(Duration::from_millis(16));
                    }
                    _ => {}
                }
            }
            Phase::Up => {
                // Leave `Hold::Settling` untouched so a mouse-up after Esc lets the glide finish (A5).
                match &self.board.hold {
                    Hold::Pressed { what: Held::Card(cid), .. } if *cid == id => {
                        self.board.hold = Hold::None;
                        return;
                    }
                    Hold::Held { what: Held::Card(cid), .. } if *cid == id => {}
                    _ => return,
                }
                let Hold::Held { what: Held::Card(cid), origin, grab, at, size, origin_pos, slot, refused, lean, .. } = std::mem::take(&mut self.board.hold) else {
                    return;
                };
                if !self.board_editable() || !self.layers.is_empty() || !origin.matches(self.active_text_key()) {
                    return;
                }
                let from = [at[0] - grab[0], at[1] - grab[1]];
                if d.cancelled() {
                    // Rune's `queue_key` intercepts Escape during a drag and emits a cancelled
                    // `Phase::Up` instead of reaching `keys::on_key`; when the page underneath is
                    // unchanged, glide back over 180 ms (BRD-019). Stale cancels (tab/mode/note/layer
                    // changes) already exited above in `Hold::None` (A2).
                    self.start_settle(Held::Card(cid), from, origin_pos, size, grab, lean, tx);
                    return;
                }
                let t = self.map_tree();
                let Some(held_idx) = t.find(cid) else { return };
                let model = board::build_with_held(&t, Some(held_idx));
                if let Some(slot) = slot {
                    let target_to = self.slot_local_pos(&model, &t, held_idx, slot).unwrap_or(origin_pos);
                    let mut final_id = cid;
                    if !board::is_same_slot(&model, held_idx, slot)
                        && let Ok(add) = board::slot_add(&model, &t, held_idx, slot)
                    {
                        match map::move_to(self.active_text(), &t, held_idx, add) {
                            Ok(splice) => {
                                self.board_apply(&t, &splice, tx);
                                if let Some(sel) = self.board_selected() {
                                    final_id = sel;
                                    tx.scroll_into_view(card_key(sel));
                                }
                            }
                            Err(Some(why)) => {
                                self.toast(why, tx);
                                self.start_settle(Held::Card(cid), from, origin_pos, size, grab, lean, tx);
                                return;
                            }
                            Err(None) => {}
                        }
                    }
                    self.start_settle(Held::Card(final_id), from, target_to, size, grab, lean, tx);
                } else {
                    if let Some(reason) = refused {
                        self.toast(reason, tx);
                    }
                    self.start_settle(Held::Card(cid), from, origin_pos, size, grab, lean, tx);
                }
            }
        }
    }

    fn start_settle(&mut self, what: Held, from: [f32; 2], to: [f32; 2], size: [f32; 2], grab: [f32; 2], from_lean: f32, tx: &mut Tx) {
        if crate::motion::reduced() {
            self.board.hold = Hold::None;
            return;
        }
        let origin = self.active_text_key().to_owned();
        self.board.hold = Hold::Settling { what, origin, from, to, size, grab, from_lean, start: self.now.get() };
        tx.wake_after(Duration::from_millis(16));
    }

    /// Cancels any active hold on Esc, settling back with nothing written (BRD-019).
    pub fn board_escape_hold(&mut self, tx: &mut Tx) -> bool {
        let hold = std::mem::take(&mut self.board.hold);
        match hold {
            Hold::Held { what, grab, at, size, origin_pos, lean, .. } => {
                let from = [at[0] - grab[0], at[1] - grab[1]];
                self.start_settle(what, from, origin_pos, size, grab, lean, tx);
                true
            }
            Hold::Pressed { .. } => true,
            other => {
                self.board.hold = other;
                false
            }
        }
    }

    /// Updates `slot` and `refused` for the currently held card (BRD-017).
    pub(crate) fn board_aim_card(&mut self) {
        let &Hold::Held { what: Held::Card(id), grab, at, size, .. } = &self.board.hold else { return };
        let t = self.map_tree();
        let Some(held_idx) = t.find(id) else { return };
        let model = board::build_with_held(&t, Some(held_idx));
        if model.columns.is_empty() {
            return;
        }
        let layout = self.board.layout.borrow();
        let view = layout.view;
        let px = view[0] + at[0];
        let mid_y = view[1] + (at[1] - grab[1]) + size[1] * 0.5;

        // Pick column under pointer's x, or nearest when between or past columns (BRD-017).
        let b = self.settings().size as f32;
        let col_w = crate::page::r(b * 20.0);
        let col_gap = crate::page::r(b * 12.0 / 14.0);
        let outer_pad = crate::page::r(b * 16.0 / 14.0);
        let col_rect = |ci: usize, col: &board::Column| -> [f32; 4] {
            if let Some(&r) = layout.cols.get(&col.head.key_id(&t)) {
                return r;
            }
            // Fallback for a just-appeared empty Loose column before its first layout report.
            let x = view[0] + outer_pad - self.board.scroll_x.get() + ci as f32 * (col_w + col_gap);
            [x, view[1] + outer_pad, col_w, 80.0]
        };

        let mut best_col = 0usize;
        let mut best_dist = f32::MAX;
        for (ci, col) in model.columns.iter().enumerate() {
            let r = col_rect(ci, col);
            let d = if px < r[0] {
                r[0] - px
            } else if px > r[0] + r[2] {
                px - (r[0] + r[2])
            } else {
                0.0
            };
            if d < best_dist {
                best_dist = d;
                best_col = ci;
            }
        }

        // In that column, pick the first remaining card whose middle is below the held card's middle.
        let rest = model.rest(best_col, Some(held_idx));
        let mut idx = rest.len();
        for (k, &ci) in rest.iter().enumerate() {
            if let Some(&cr) = layout.cards.get(&t.nodes[ci].id) {
                let cy = cr[1] + cr[3] * 0.5;
                if cy > mid_y {
                    idx = k;
                    break;
                }
            }
        }
        drop(layout);

        let want = Slot { column: best_col, index: idx };
        let allowed = board::nearest_allowed(&model, &t, held_idx, want);
        let refused = if allowed.is_none() { board::column_refusal(&model, &t, held_idx, want) } else { None };
        if let Hold::Held { slot, refused: r, .. } = &mut self.board.hold {
            *slot = allowed;
            *r = refused;
        }
    }

    /// Estimates the board-local `[x, y]` of `slot` for the settle glide target.
    fn slot_local_pos(&self, model: &Board, t: &map::Tree, held_idx: usize, slot: Slot) -> Option<[f32; 2]> {
        let layout = self.board.layout.borrow();
        let view = layout.view;
        let col = model.columns.get(slot.column)?;
        let key_id = col.head.key_id(t);
        let rest = model.rest(slot.column, Some(held_idx));
        if let Some(&after_idx) = rest.get(slot.index)
            && let Some(&r) = layout.cards.get(&t.nodes[after_idx].id)
        {
            return Some([r[0] - view[0], r[1] - view[1]]);
        }
        if let Some(&before_idx) = slot.index.checked_sub(1).and_then(|k| rest.get(k))
            && let Some(&r) = layout.cards.get(&t.nodes[before_idx].id)
        {
            return Some([r[0] - view[0], r[1] + r[3] + 6.0 - view[1]]);
        }
        if let Some(&cv) = layout.col_views.get(&key_id) {
            return Some([cv[0] - view[0], cv[1] + 6.0 - view[1]]);
        }
        if let Some(&cr) = layout.cols.get(&key_id) {
            return Some([cr[0] + 8.0 - view[0], cr[1] + 40.0 - view[1]]);
        }
        None
    }

    /// Drag handler on a heading column's header (BRD-021, BRD-022, BRD-023).
    pub fn board_col_drag(&mut self, id: NodeId, d: &Drag, tx: &mut Tx) {
        let pt = self.board_point(d);
        let now = self.now.get();
        match d.phase {
            Phase::Down => {
                if !self.board_editable() || !self.layers.is_empty() {
                    return;
                }
                let origin = self.active_text_key().to_owned();
                let layout = self.board.layout.borrow();
                let view = layout.view;
                let rect = layout.cols.get(&id).copied().unwrap_or([view[0] + pt[0], view[1] + pt[1], 280.0, 200.0]);
                let local = [rect[0] - view[0], rect[1] - view[1]];
                let grab = [(pt[0] - local[0]).clamp(0.0, rect[2]), (pt[1] - local[1]).clamp(0.0, rect[3])];
                let size = [rect[2], rect[3]];
                drop(layout);
                self.board.hold = Hold::Pressed { what: Held::Column(id), origin, start: pt, grab, size, origin_pos: local };
                tx.live_only();
            }
            Phase::Move => {
                if self.board_hold_stale() {
                    self.board_cancel_hold();
                    return;
                }
                let reduced = crate::motion::reduced();
                match &mut self.board.hold {
                    Hold::Pressed { what: Held::Column(cid), origin, start, grab, size, origin_pos } if *cid == id => {
                        if (pt[0] - start[0]).hypot(pt[1] - start[1]) <= THRESHOLD {
                            tx.live_only();
                            return;
                        }
                        let (origin, grab, size, origin_pos) = (origin.clone(), *grab, *size, *origin_pos);
                        let (lean, sign) = ((BASE_DEG * 0.5).to_radians(), 1.0);
                        self.board.hold = Hold::Held {
                            what: Held::Column(id),
                            origin,
                            grab,
                            at: pt,
                            size,
                            origin_pos,
                            slot: None,
                            col_slot: None,
                            refused: None,
                            lean,
                            sign,
                            last_x: pt[0],
                            last_t: now,
                            tick: now,
                        };
                        if let Some(v) = self.map_view_mut() {
                            v.selected = id;
                        }
                        self.board_aim_column();
                        tx.wake_after(Duration::from_millis(16));
                    }
                    Hold::Held { what: Held::Column(cid), at, lean, sign, last_x, last_t, .. } if *cid == id => {
                        let dx = pt[0] - *last_x;
                        let dt = (now - *last_t).max(1.0 / 240.0) as f32;
                        let vx = dx / dt;
                        let (new_lean, new_sign) = tilt_radians(*sign, dx, vx, reduced, true);
                        *at = pt;
                        *lean = new_lean;
                        *sign = new_sign;
                        *last_x = pt[0];
                        *last_t = now;
                        self.board_aim_column();
                        tx.wake_after(Duration::from_millis(16));
                    }
                    _ => {}
                }
            }
            Phase::Up => {
                // Leave `Hold::Settling` untouched so a mouse-up after Esc lets the glide finish (A5).
                match &self.board.hold {
                    Hold::Pressed { what: Held::Column(cid), .. } if *cid == id => {
                        self.board.hold = Hold::None;
                        return;
                    }
                    Hold::Held { what: Held::Column(cid), .. } if *cid == id => {}
                    _ => return,
                }
                let Hold::Held { what: Held::Column(cid), origin, grab, at, size, origin_pos, col_slot, refused, lean, .. } = std::mem::take(&mut self.board.hold) else {
                    return;
                };
                if !self.board_editable() || !self.layers.is_empty() || !origin.matches(self.active_text_key()) {
                    return;
                }
                let from = [at[0] - grab[0], at[1] - grab[1]];
                if d.cancelled() {
                    self.start_settle(Held::Column(cid), from, origin_pos, size, grab, lean, tx);
                    return;
                }
                let t = self.map_tree();
                let Some(held_idx) = t.find(cid) else { return };
                let model = board::build(&t);
                if let Some(target_col) = col_slot {
                    let target_to = self.col_slot_local_pos(&model, &t, held_idx, target_col).unwrap_or(origin_pos);
                    let mut final_id = cid;
                    if !board::is_same_column_slot(&model, held_idx, target_col)
                        && let Ok(add) = board::column_add(&model, &t, held_idx, target_col)
                    {
                        match map::move_to(self.active_text(), &t, held_idx, add) {
                            Ok(splice) => {
                                self.board_apply(&t, &splice, tx);
                                if let Some(sel) = self.board_selected() {
                                    final_id = sel;
                                    let t2 = self.map_tree();
                                    if let Some(hi) = t2.find(sel) {
                                        tx.scroll_into_view(head_key(ColumnHead::Heading(hi), &t2));
                                    }
                                }
                            }
                            Err(Some(why)) => {
                                self.toast(why, tx);
                                self.start_settle(Held::Column(cid), from, origin_pos, size, grab, lean, tx);
                                return;
                            }
                            Err(None) => {}
                        }
                    }
                    self.start_settle(Held::Column(final_id), from, target_to, size, grab, lean, tx);
                } else {
                    if let Some(reason) = refused {
                        self.toast(reason, tx);
                    }
                    self.start_settle(Held::Column(cid), from, origin_pos, size, grab, lean, tx);
                }
            }
        }
    }

    /// Updates `col_slot` and `refused` for the currently held column (BRD-021).
    pub(crate) fn board_aim_column(&mut self) {
        let &Hold::Held { what: Held::Column(id), grab, at, size, .. } = &self.board.hold else { return };
        let t = self.map_tree();
        let Some(held_idx) = t.find(id) else { return };
        let model = board::build(&t);
        let heads: Vec<usize> = model
            .columns
            .iter()
            .filter_map(|c| match c.head {
                ColumnHead::Heading(h) if h != held_idx => Some(h),
                _ => None,
            })
            .collect();
        if heads.is_empty() {
            if let Hold::Held { col_slot, refused, .. } = &mut self.board.hold {
                *col_slot = Some(0);
                *refused = None;
            }
            return;
        }
        let layout = self.board.layout.borrow();
        let view = layout.view;
        let mid_x = view[0] + (at[0] - grab[0]) + size[0] * 0.5;
        let mut target = heads.len();
        for (k, &h) in heads.iter().enumerate() {
            if let Some(&cr) = layout.cols.get(&t.nodes[h].id) {
                let cx = cr[0] + cr[2] * 0.5;
                if cx > mid_x {
                    target = k;
                    break;
                }
            }
        }
        drop(layout);

        let (allowed, why) = if board::is_same_column_slot(&model, held_idx, target)
            || board::column_add(&model, &t, held_idx, target).is_ok()
        {
            (Some(target), None)
        } else {
            let reason = board::column_add(&model, &t, held_idx, target).err().flatten();
            (None, reason)
        };
        if let Hold::Held { col_slot, refused, .. } = &mut self.board.hold {
            *col_slot = allowed;
            *refused = why;
        }
    }

    /// Estimates the board-local `[x, y]` of column slot `target_col` for the settle glide target.
    fn col_slot_local_pos(&self, model: &Board, t: &map::Tree, held_idx: usize, target_col: usize) -> Option<[f32; 2]> {
        let layout = self.board.layout.borrow();
        let view = layout.view;
        let b = self.settings().size as f32;
        let col_gap = crate::page::r(b * 12.0 / 14.0);
        let outer_pad = crate::page::r(b * 16.0 / 14.0);
        let heads: Vec<usize> = model
            .columns
            .iter()
            .filter_map(|c| match c.head {
                ColumnHead::Heading(h) if h != held_idx => Some(h),
                _ => None,
            })
            .collect();
        if let Some(&after_h) = heads.get(target_col)
            && let Some(&r) = layout.cols.get(&t.nodes[after_h].id)
        {
            return Some([r[0] - view[0], r[1] - view[1]]);
        }
        if let Some(&before_h) = target_col.checked_sub(1).and_then(|k| heads.get(k))
            && let Some(&r) = layout.cols.get(&t.nodes[before_h].id)
        {
            return Some([r[0] + r[2] + col_gap - view[0], r[1] - view[1]]);
        }
        if let Some(&loose_r) = layout.cols.get(&0) {
            return Some([loose_r[0] + loose_r[2] + col_gap - view[0], loose_r[1] - view[1]]);
        }
        Some([outer_pad, outer_pad])
    }

    /// Per-frame board animation step: settle glide, edge auto-scroll (BRD-018), and velocity
    /// tilt relaxation when the hand pauses (BRD-015).
    pub(crate) fn poll_board(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.board_hold_stale() {
            self.board_cancel_hold();
            return true;
        }
        if self.mode != Mode::Board {
            return false;
        }
        let mut need_reaim: Option<Held> = None;
        let mut changed = false;
        let mut keep_waking = false;

        match &mut self.board.hold {
            Hold::Settling { start, .. } => {
                if crate::motion::reduced() || now - *start >= SETTLE_SECS {
                    self.board.hold = Hold::None;
                    return true;
                }
                tx.wake_after(Duration::from_millis(16));
                return true;
            }
            Hold::Held { what, at, lean, sign, last_t, tick, .. } => {
                let what = *what;
                let at = *at;
                let dt = (now - *tick).clamp(0.0, 0.05) as f32;
                *tick = now;

                // 1. Velocity relaxation when hand pauses (BRD-015).
                let reduced = crate::motion::reduced();
                if !reduced {
                    let rest = *sign * BASE_DEG.to_radians() * if what.is_column() { 0.5 } else { 1.0 };
                    if (*lean - rest).abs() > 0.001 {
                        if now - *last_t > 0.08 {
                            *lean = rest;
                            changed = true;
                        } else {
                            keep_waking = true;
                        }
                    }
                }

                // 2. Edge auto-scroll (BRD-018): within 36 px of board left/right or column top/bottom.
                let layout = self.board.layout.borrow();
                let view = layout.view;
                let max_x = (layout.row_w - view[2]).max(0.0);
                let vx = if view[2] > 0.0 && at[0] < EDGE {
                    -EDGE_SPEED * ((EDGE - at[0]) / EDGE).clamp(0.0, 1.0)
                } else if view[2] > 0.0 && at[0] > view[2] - EDGE {
                    EDGE_SPEED * ((at[0] - (view[2] - EDGE)) / EDGE).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let mut h_scroll: Option<f32> = None;
                if vx != 0.0 && max_x > 0.0 {
                    let cur = self.board.scroll_x.get();
                    let can_move = (vx < 0.0 && cur > 0.01) || (vx > 0.0 && cur < max_x - 0.01);
                    if can_move {
                        keep_waking = true;
                        if dt > 0.0 {
                            let next = (cur + vx * dt).clamp(0.0, max_x);
                            if (next - cur).abs() > 0.01 {
                                h_scroll = Some(next);
                            }
                        }
                    }
                }

                let mut v_scroll: Option<(u64, f32)> = None;
                if !what.is_column() {
                    let px = view[0] + at[0];
                    let py = view[1] + at[1];
                    let hovered_col = layout
                        .cols
                        .iter()
                        .find(|&(_, cr)| px >= cr[0] && px <= cr[0] + cr[2])
                        .map(|(&k, _)| k);
                    if let Some(key_id) = hovered_col
                        && let Some(&cv) = layout.col_views.get(&key_id)
                        && let Some(&content_h) = layout.col_content_h.get(&key_id)
                    {
                        let max_y = (content_h - cv[3]).max(0.0);
                        if max_y > 0.0 && py >= cv[1] - 48.0 && py <= cv[1] + cv[3] + 48.0 {
                            let vy = if py < cv[1] + EDGE {
                                -EDGE_SPEED * ((cv[1] + EDGE - py) / EDGE).clamp(0.0, 1.0)
                            } else if py > cv[1] + cv[3] - EDGE {
                                EDGE_SPEED * ((py - (cv[1] + cv[3] - EDGE)) / EDGE).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            if vy != 0.0 {
                                let cur = self.board.col_scroll.borrow().get(&key_id).copied().unwrap_or(0.0);
                                let can_move = (vy < 0.0 && cur > 0.01) || (vy > 0.0 && cur < max_y - 0.01);
                                if can_move {
                                    keep_waking = true;
                                    if dt > 0.0 {
                                        let next = (cur + vy * dt).clamp(0.0, max_y);
                                        if (next - cur).abs() > 0.01 {
                                            v_scroll = Some((key_id, next));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                drop(layout);

                if let Some(next_x) = h_scroll {
                    self.board.scroll_x.set(next_x);
                    tx.scroll_to_instant("board-scroll", next_x, 0.0);
                    changed = true;
                    need_reaim = Some(what);
                }
                if let Some((key_id, next_y)) = v_scroll {
                    self.board.col_scroll.borrow_mut().insert(key_id, next_y);
                    tx.scroll_to_instant(format!("board-col-scroll:{key_id}"), 0.0, next_y);
                    changed = true;
                    need_reaim = Some(what);
                }
            }
            _ => return false,
        }

        if let Some(what) = need_reaim {
            match what {
                Held::Card(_) => self.board_aim_card(),
                Held::Column(_) => self.board_aim_column(),
            }
        }
        if keep_waking {
            tx.wake_after(Duration::from_millis(16));
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilt_rule_matches_brd_015_021_022() {
        // Rest: 3 deg clockwise with last sign +1.
        let (rad, sign) = tilt_radians(1.0, 0.0, 0.0, false, false);
        assert!((rad - 3.0f32.to_radians()).abs() < 1e-5);
        assert_eq!(sign, 1.0);
        // Full speed right: 6 deg clockwise.
        let (rad, sign) = tilt_radians(1.0, 20.0, 1500.0, false, false);
        assert!((rad - 6.0f32.to_radians()).abs() < 1e-5);
        assert_eq!(sign, 1.0);
        // Full speed left: -6 deg counter-clockwise.
        let (rad, sign) = tilt_radians(1.0, -20.0, -1500.0, false, false);
        assert!((rad + 6.0f32.to_radians()).abs() < 1e-5);
        assert_eq!(sign, -1.0);
        // Small jitter under 4 px keeps last sign.
        let (rad, sign) = tilt_radians(-1.0, 2.0, 0.0, false, false);
        assert!((rad + 3.0f32.to_radians()).abs() < 1e-5);
        assert_eq!(sign, -1.0);
        // Column uses half angles (1.5..3 deg).
        let (rad, _) = tilt_radians(1.0, 20.0, 1500.0, false, true);
        assert!((rad - 3.0f32.to_radians()).abs() < 1e-5);
        // Reduced motion: fixed 3 deg for card, 1.5 deg for column.
        let (card_red, _) = tilt_radians(-1.0, -20.0, -1500.0, true, false);
        assert!((card_red - 3.0f32.to_radians()).abs() < 1e-5);
        let (col_red, _) = tilt_radians(-1.0, -20.0, -1500.0, true, true);
        assert!((col_red - 1.5f32.to_radians()).abs() < 1e-5);
    }
}
