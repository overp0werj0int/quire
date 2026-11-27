//! Board mode drag state machine and tilt physics (`docs/reader-writer/behavior/board.md`,
//! BRD-014 to BRD-023).
//!
//! One state machine (`Hold`) owns the lifecycle of pressing, lifting, carrying with tilt, and
//! settling a card or a column. The tilt rule is a pure function so tests and probes read the
//! exact angle in radians.

use crate::board::Slot;
use crate::map::NodeId;

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

    pub fn held_column(&self) -> Option<NodeId> {
        match self {
            Hold::Held { what: Held::Column(id), .. } => Some(*id),
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
