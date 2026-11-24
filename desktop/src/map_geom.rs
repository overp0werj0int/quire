//! The geometry beside one map node: its fold ring, where its branches leave and land, its
//! `+N` pill, the room culling keeps for it, and the branch between two nodes. A node's place
//! carries the way its branches grow (`Place::dir`), and everything here follows it. Only this
//! file and `map_layout.rs` match on `Dir`; the rest of the map asks a `Place`, a `Branch` or
//! the `Laid`.

use std::hash::{Hash, Hasher};

/// Room past a node's growing side for its fold ring and count pill, in world points.
pub const FOLD_ROOM: f32 = 64.0;
/// How far a fold ring's hit box may reach back over its node (MAP-041).
pub const FOLD_R: f32 = 6.0;
/// An expanded node's fold ring: beside the middle of the node's growing side, just outside
/// its outline, where its branches leave (MAP-041). Its radius, and the gap between the node
/// and the ring, wide enough to clear the selection outline.
pub const FOLD_DOT_R: f32 = 7.0;
pub const FOLD_DOT_GAP: f32 = 6.0;
/// How far above a node's top a branch growing down stops.
const ENTRY_GAP: f32 = 3.0;

/// A branch's stroke and underline width at `depth`: 4 at the root, then towards 1.
pub fn line_width(depth: usize) -> f32 {
    1.0 + 3.0 / 2f32.powi(depth.min(30) as i32)
}

/// The way a node's branches grow: away from its parent, towards its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(dead_code, reason = "every map is laid out Right until the other layouts land")]
pub enum Dir {
    Right,
    Left,
    Down,
    Up,
}

impl Dir {
    pub fn vertical(self) -> bool {
        matches!(self, Dir::Down | Dir::Up)
    }

    /// One step this way.
    fn unit(self) -> [f32; 2] {
        match self {
            Dir::Right => [1.0, 0.0],
            Dir::Left => [-1.0, 0.0],
            Dir::Down => [0.0, 1.0],
            Dir::Up => [0.0, -1.0],
        }
    }
}

/// A node's padded box in world points, and the way its branches grow. Its underline runs
/// below the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub depth: usize,
    pub dir: Dir,
}

impl Place {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// The middle of the underline's thickness.
    pub fn line_y(&self) -> f32 {
        self.y + self.h + line_width(self.depth) / 2.0
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h + line_width(self.depth)
    }

    /// The box with its underline, `[x0, y0, x1, y1]`.
    pub fn rect(&self) -> [f32; 4] {
        [self.x, self.y, self.right(), self.bottom()]
    }

    /// The fold ring's centre on side `d`: beyond the middle of that side.
    pub fn ring(&self, d: Dir) -> [f32; 2] {
        let out = FOLD_DOT_GAP + FOLD_DOT_R;
        match d {
            Dir::Right => [self.right() + out, self.y + self.h / 2.0],
            Dir::Left => [self.x - out, self.y + self.h / 2.0],
            Dir::Down => [self.x + self.w / 2.0, self.bottom() + out],
            Dir::Up => [self.x + self.w / 2.0, self.y - out],
        }
    }

    /// Where branches growing `d` leave the node: the ring's far edge.
    pub fn branch_start(&self, d: Dir) -> [f32; 2] {
        let ([x, y], [ux, uy]) = (self.ring(d), d.unit());
        [x + FOLD_DOT_R * ux, y + FOLD_DOT_R * uy]
    }

    /// Where the branch into this node lands.
    pub fn entry(&self) -> [f32; 2] {
        match self.dir {
            Dir::Right => [self.x, self.line_y()],
            Dir::Left => [self.right(), self.line_y()],
            Dir::Down => [self.x + self.w / 2.0, self.y - ENTRY_GAP],
            Dir::Up => [self.x + self.w / 2.0, self.line_y()],
        }
    }

    /// The far end of the underline, when the branch into this node runs on as it. None where
    /// the branch lands on the node's middle, and the node draws its own underline.
    pub fn underline_end(&self) -> Option<[f32; 2]> {
        match self.dir {
            Dir::Right => Some([self.right(), self.line_y()]),
            Dir::Left => Some([self.x, self.line_y()]),
            Dir::Down | Dir::Up => None,
        }
    }

    /// A `+N` pill of `size` in the ring's place on side `d`, `[x, y, w, h]`: it starts where
    /// the ring's near edge would be and is centred on the ring across.
    pub fn pill_box(&self, d: Dir, [w, h]: [f32; 2]) -> [f32; 4] {
        let [cx, cy] = self.ring(d);
        match d {
            Dir::Right => [self.right() + FOLD_DOT_GAP, cy - h / 2.0, w, h],
            Dir::Left => [self.x - FOLD_DOT_GAP - w, cy - h / 2.0, w, h],
            Dir::Down => [cx - w / 2.0, self.bottom() + FOLD_DOT_GAP, w, h],
            Dir::Up => [cx - w / 2.0, self.y - FOLD_DOT_GAP - h, w, h],
        }
    }

    /// What culling must keep to show the node, `[x0, y0, x1, y1]`: its box with `FOLD_ROOM`
    /// on its growing side for the ring and the pill.
    pub fn reach(&self) -> [f32; 4] {
        let [x0, y0, x1, y1] = self.rect();
        match self.dir {
            Dir::Right => [x0, y0, x1 + FOLD_ROOM, y1],
            Dir::Left => [x0 - FOLD_ROOM, y0, x1, y1],
            Dir::Down => [x0, y0, x1, y1 + FOLD_ROOM],
            Dir::Up => [x0, y0 - FOLD_ROOM, x1, y1],
        }
    }

    /// This place moved so its branch lands at `pt`.
    pub fn landed_at(self, pt: [f32; 2]) -> Place {
        let [ex, ey] = self.entry();
        Place { x: pt[0] - (ex - self.x), y: pt[1] - (ey - self.y), ..self }
    }

    /// The node's span along the axis its siblings stack on, underline included: top to bottom
    /// where branches grow sideways, left to right where they grow down or up. It increases in
    /// document order.
    pub(crate) fn cross(&self) -> [f32; 2] {
        if self.dir.vertical() { [self.x, self.right()] } else { [self.y, self.bottom()] }
    }

    /// The node's span along the axis its branches grow on.
    pub(crate) fn main(&self) -> [f32; 2] {
        if self.dir.vertical() { [self.y, self.bottom()] } else { [self.x, self.right()] }
    }

    /// World point `w` as `(main, cross)` on this node's axes.
    pub(crate) fn along(&self, [x, y]: [f32; 2]) -> (f32, f32) {
        if self.dir.vertical() { (y, x) } else { (x, y) }
    }

    /// A bar as long as the node, `thick` wide, starting at `c` across, `[x, y, w, h]`.
    pub(crate) fn bar(&self, c: f32, thick: f32) -> [f32; 4] {
        if self.dir.vertical() { [c, self.y, thick, self.bottom() - self.y] } else { [self.x, c, self.w, thick] }
    }

    /// The stub of a branch a Child drop shows, `len` long and `thick` wide, out of the growing
    /// side: `[x, y, w, h]` from the corner of the node's box grown by `pad` all round.
    pub(crate) fn child_stub(&self, len: f32, thick: f32, pad: f32) -> [f32; 4] {
        let line = self.line_y() - self.y + pad - thick / 2.0;
        let mid = self.w / 2.0 + pad - thick / 2.0;
        match self.dir {
            Dir::Right => [self.w + 2.0 * pad, line, len, thick],
            Dir::Left => [-len, line, len, thick],
            Dir::Down => [mid, self.bottom() - self.y + 2.0 * pad, thick, len],
            Dir::Up => [mid, -len, thick, len],
        }
    }
}

/// One branch: from the parent's ring edge to the child's entry, and on along its underline.
#[derive(Clone, Copy, Debug)]
pub struct Branch {
    pub from: [f32; 2],
    pub to: [f32; 2],
    /// The far end of the child's underline, where the branch draws it.
    pub end: Option<[f32; 2]>,
    pub dir: Dir,
    pub lw: f32,
}

impl Branch {
    /// The cubic from `from` to `to`, leaving and landing along `dir`: an S.
    pub fn curve(&self) -> [[f32; 2]; 4] {
        let (a, z) = (self.from, self.to);
        if self.dir.vertical() {
            let mid = (a[1] + z[1]) / 2.0;
            [a, [a[0], mid], [z[0], mid], z]
        } else {
            let mid = (a[0] + z[0]) / 2.0;
            [a, [mid, a[1]], [mid, z[1]], z]
        }
    }

    /// The curve is as good as a straight line: its ends are under half a point apart across.
    pub fn flat(&self) -> bool {
        let k = if self.dir.vertical() { 0 } else { 1 };
        (self.from[k] - self.to[k]).abs() < 0.5
    }

    /// The box round the curve and the underline to `end`, `[x0, y0, x1, y1]`: widened by the
    /// stroke across the branch, and at least a point each way.
    pub fn bbox(&self, end: Option<[f32; 2]>) -> [f32; 4] {
        let pts = [self.from, self.to, end.unwrap_or(self.to)];
        let lo = |k: usize| pts.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
        let hi = |k: usize| pts.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
        let [wx, wy] = if self.dir.vertical() { [self.lw, 0.0] } else { [0.0, self.lw] };
        let (x0, y0) = (lo(0) - wx, lo(1) - wy);
        [x0, y0, (hi(0) + wx).max(x0 + 1.0), (hi(1) + wy).max(y0 + 1.0)]
    }

    /// What a drawing of this branch in `color` depends on, wherever the branch sits: its
    /// shape from its box's corner, its direction, its width and the colour.
    pub fn hash(&self, end: Option<[f32; 2]>, color: impl Hash) -> u64 {
        let [x0, y0, ..] = self.bbox(end);
        let rel = |[x, y]: [f32; 2]| [(x - x0).to_bits(), (y - y0).to_bits()];
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.dir, self.curve().map(rel), end.map(rel), self.lw.to_bits(), color).hash(&mut h);
        h.finish()
    }
}
