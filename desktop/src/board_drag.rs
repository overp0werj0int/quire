//! Board mode drag state machine and tilt physics (`docs/reader-writer/behavior/board.md`,
//! BRD-014 to BRD-023).
//!
//! One state machine (`Hold`) owns the lifecycle of pressing, lifting, carrying with tilt, and
//! settling a card or a column. The tilt rule is a pure function so tests and probes read the
//! exact angle in radians.

use std::time::Duration;

use rune::{Drag, Phase, Tx};

use crate::app::{App, Mode};
use crate::board::{self, Board, Slot};
use crate::board_app::card_key;
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
    pub fn id(self) -> NodeId {
        match self {
            Held::Card(id) | Held::Column(id) => id,
        }
    }

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
        start: [f32; 2],
        grab: [f32; 2],
        size: [f32; 2],
        /// Original card/column top-left in board-viewport coordinates, for cancel settle.
        origin_pos: [f32; 2],
    },
    /// Lifted and following the pointer with a tilt and live drop slot.
    Held {
        what: Held,
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

    pub fn slot(&self) -> Option<Slot> {
        match self {
            Hold::Held { slot, .. } => *slot,
            _ => None,
        }
    }

    /// The tilt in radians if currently held, or `0.0` otherwise.
    pub fn lean(&self) -> f32 {
        match self {
            Hold::Held { lean, .. } => *lean,
            _ => 0.0,
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

    /// Drag handler on a board card (BRD-014 to BRD-020, BRD-022, BRD-023).
    pub fn board_card_drag(&mut self, id: NodeId, d: &Drag, tx: &mut Tx) {
        let pt = self.board_point(d);
        let now = self.now.get();
        match d.phase {
            Phase::Down => {
                self.board.dropped = false;
                if !self.board_editable() {
                    return;
                }
                let layout = self.board.layout.borrow();
                let view = layout.view;
                let rect = layout.cards.get(&id).copied().unwrap_or([view[0] + pt[0], view[1] + pt[1], 264.0, 40.0]);
                let local = [rect[0] - view[0], rect[1] - view[1]];
                let grab = [(pt[0] - local[0]).clamp(0.0, rect[2]), (pt[1] - local[1]).clamp(0.0, rect[3])];
                let size = [rect[2], rect[3]];
                drop(layout);
                self.board.hold = Hold::Pressed { what: Held::Card(id), start: pt, grab, size, origin_pos: local };
                tx.live_only();
            }
            Phase::Move => {
                let reduced = crate::motion::reduced();
                match &mut self.board.hold {
                    Hold::Pressed { what: Held::Card(cid), start, grab, size, origin_pos } if *cid == id => {
                        if (pt[0] - start[0]).hypot(pt[1] - start[1]) <= THRESHOLD {
                            tx.live_only();
                            return;
                        }
                        let (grab, size, origin_pos) = (*grab, *size, *origin_pos);
                        let (lean, sign) = (BASE_DEG.to_radians(), 1.0);
                        self.board.hold = Hold::Held {
                            what: Held::Card(id),
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
                        if !reduced {
                            tx.wake_after(Duration::from_millis(16));
                        }
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
                        if !reduced {
                            tx.wake_after(Duration::from_millis(16));
                        }
                    }
                    _ => {}
                }
            }
            Phase::Up => {
                let hold = std::mem::take(&mut self.board.hold);
                let Hold::Held { what: Held::Card(cid), grab, at, size, origin_pos, slot, refused, lean, .. } = hold else {
                    return;
                };
                self.board.dropped = true;
                let from = [at[0] - grab[0], at[1] - grab[1]];
                if d.cancelled() {
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
        self.board.hold = Hold::Settling { what, from, to, size, grab, from_lean, start: self.now.get() };
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
        let Hold::Held { what: Held::Card(id), grab, at, size, .. } = self.board.hold else { return };
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

    /// Per-frame board animation step (settle glide + velocity relaxation when hand pauses).
    pub(crate) fn poll_board(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.mode != Mode::Board {
            return false;
        }
        match &mut self.board.hold {
            Hold::Settling { start, .. } => {
                if crate::motion::reduced() || now - *start >= SETTLE_SECS {
                    self.board.hold = Hold::None;
                    return true;
                }
                tx.wake_after(Duration::from_millis(16));
                true
            }
            Hold::Held { what, lean, sign, last_t, .. } => {
                let reduced = crate::motion::reduced();
                if !reduced && now - *last_t > 0.08 {
                    let rest = *sign * BASE_DEG.to_radians() * if what.is_column() { 0.5 } else { 1.0 };
                    if (*lean - rest).abs() > 0.001 {
                        *lean = rest;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}
