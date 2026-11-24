//! The geometry beside one map node: its fold ring, where its branches leave and land, its
//! `+N` pill, the room culling keeps for it, and the branch between two nodes. A node's place
//! carries the way its branches grow (`Place::dir`), and everything here follows it. Only this
//! file and `map_layout.rs` match on `Dir`; the rest of the map asks a `Place`, a `Branch` or
//! the `Laid`.

use std::hash::{Hash, Hasher};

/// Room past a node's growing side for its fold ring and count pill, in world points.
pub const FOLD_ROOM: f32 = 64.0;
/// How far a fold ring's hit box may reach back over its node (MAP-041).
pub const FOLD_HIT_BACK: f32 = 6.0;
/// An expanded node's fold ring: beside the middle of the node's growing side, just outside
/// its outline, where its branches leave (MAP-041). Its radius, and the gap between the node
/// and the ring, wide enough to clear the selection outline.
pub const FOLD_DOT_R: f32 = 7.0;
pub const FOLD_DOT_GAP: f32 = 6.0;

/// The stroke width of the branch into a node at `depth` and of the node's line: 2.5 under
/// the root, then towards 1.
pub fn line_width(depth: usize) -> f32 {
    1.0 + 3.0 / 2f32.powi(depth.min(30) as i32)
}

/// How tall the curly bracket along a label is, where branches grow down or up.
const BRACE_H: f32 = 12.0;

/// The room a node's line takes beside its label: the bracket's height where branches grow
/// `dir` down or up, else the underline's width. The root has no line.
pub fn line_room(depth: usize, dir: Dir) -> f32 {
    match depth {
        0 => 0.0,
        _ if dir.vertical() => BRACE_H,
        d => line_width(d),
    }
}

/// The way a node's branches grow: away from its parent, towards its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Right,
    Left,
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

/// What an arrow means on a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Parent,
    /// The first child growing this way.
    Child(Dir),
    Prev,
    Next,
}

impl Dir {
    pub fn vertical(self) -> bool {
        matches!(self, Dir::Down | Dir::Up)
    }

    /// The arrow along the branches goes to a child and the one against them to the parent.
    /// Across them, the arrow towards earlier siblings (up, or left in a vertical layout) is
    /// Prev and the other Next.
    pub fn rel(self, a: Arrow) -> Rel {
        match (self, a) {
            (Dir::Right, Arrow::Right) | (Dir::Left, Arrow::Left) | (Dir::Down, Arrow::Down) | (Dir::Up, Arrow::Up) => Rel::Child(self),
            (Dir::Right, Arrow::Left) | (Dir::Left, Arrow::Right) | (Dir::Down, Arrow::Up) | (Dir::Up, Arrow::Down) => Rel::Parent,
            (Dir::Right | Dir::Left, Arrow::Up) | (Dir::Down | Dir::Up, Arrow::Left) => Rel::Prev,
            (Dir::Right | Dir::Left, Arrow::Down) | (Dir::Down | Dir::Up, Arrow::Right) => Rel::Next,
        }
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

    /// The axis the branches run along, 0 for x and 1 for y, and whether they run towards
    /// its greater end.
    pub(crate) fn axis(self) -> (usize, bool) {
        (self.vertical() as usize, matches!(self, Dir::Right | Dir::Down))
    }

    /// `[x0, y0, x1, y1]` from a span along the branches and one across them.
    pub(crate) fn rect([m0, m1]: [f32; 2], [c0, c1]: [f32; 2], vertical: bool) -> [f32; 4] {
        if vertical { [c0, m0, c1, m1] } else { [m0, c0, m1, c1] }
    }

    /// A bar from `from` running this way: `len` long and `thick` across, `[x, y, w, h]`.
    fn bar_from(self, [x, y]: [f32; 2], len: f32, thick: f32) -> [f32; 4] {
        let [ux, uy] = self.unit();
        if self.vertical() { [x - thick / 2.0, y + len * uy.min(0.0), thick, len] } else { [x + len * ux.min(0.0), y - thick / 2.0, len, thick] }
    }
}

/// A node's box in world points, and the way its branches grow. `h` is its content with the
/// padding to its line; the line takes `line_room` more. The line is an underline where
/// branches grow sideways. Where they grow down or up it is a curly bracket along the edge the
/// branch into the node comes to, over the label or under it, its point towards the branch.
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

    /// The node's line is a curly bracket.
    pub fn braced(&self) -> bool {
        self.depth > 0 && self.dir.vertical()
    }

    /// The line is over the label, not under it.
    pub fn overlined(&self) -> bool {
        self.depth > 0 && self.dir == Dir::Down
    }

    pub fn line_room(&self) -> f32 {
        line_room(self.depth, self.dir)
    }

    /// The top of the line, from the node's top.
    pub fn line_top(&self) -> f32 {
        if self.overlined() { 0.0 } else { self.h }
    }

    /// The middle of the underline's thickness.
    pub fn line_y(&self) -> f32 {
        self.y + self.line_top() + self.line_room() / 2.0
    }

    /// The content's top from the node's, and its height, with `pad` between it and the line.
    /// Content over its line keeps the padding in its height, and the root, with no line, has
    /// half of it above and half below.
    pub fn content(&self, pad: f32) -> (f32, f32) {
        if self.depth == 0 {
            (pad / 2.0, self.h - pad)
        } else if self.overlined() {
            (self.line_room() + pad, self.h - pad)
        } else {
            (0.0, self.h)
        }
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h + self.line_room()
    }

    /// The box with its line, `[x0, y0, x1, y1]`.
    pub fn rect(&self) -> [f32; 4] {
        [self.x, self.y, self.right(), self.bottom()]
    }

    /// The middle of side `d` of the box with its line. What sits beside a node is
    /// placed from here, out along `d`.
    pub fn side_mid(&self, d: Dir) -> [f32; 2] {
        match d {
            Dir::Right => [self.right(), self.y + self.h / 2.0],
            Dir::Left => [self.x, self.y + self.h / 2.0],
            Dir::Down => [self.x + self.w / 2.0, self.bottom()],
            Dir::Up => [self.x + self.w / 2.0, self.y],
        }
    }

    /// The point `by` out from the middle of side `d`.
    fn out(&self, d: Dir, by: f32) -> [f32; 2] {
        let ([x, y], [ux, uy]) = (self.side_mid(d), d.unit());
        [x + by * ux, y + by * uy]
    }

    /// The fold ring's centre on side `d`: beyond the middle of that side.
    pub fn ring(&self, d: Dir) -> [f32; 2] {
        self.out(d, FOLD_DOT_GAP + FOLD_DOT_R)
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
            Dir::Down => [self.x + self.w / 2.0, self.y],
            Dir::Up => [self.x + self.w / 2.0, self.bottom()],
        }
    }

    /// The far end of the underline, when the branch into this node runs on as it. None where
    /// the branch lands on the point of the node's bracket, which the node draws.
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
        let (len, thick) = if d.vertical() { (h, w) } else { (w, h) };
        d.bar_from(self.out(d, FOLD_DOT_GAP), len, thick)
    }

    /// What culling must keep to show the node, `[x0, y0, x1, y1]`: its box with `FOLD_ROOM`
    /// on its growing side for the ring and the pill.
    pub fn reach(&self) -> [f32; 4] {
        let ([x0, y0, x1, y1], [ux, uy]) = (self.rect(), self.dir.unit());
        [x0 + FOLD_ROOM * ux.min(0.0), y0 + FOLD_ROOM * uy.min(0.0), x1 + FOLD_ROOM * ux.max(0.0), y1 + FOLD_ROOM * uy.max(0.0)]
    }

    /// This place moved so its branch lands at `pt`.
    pub fn landed_at(self, pt: [f32; 2]) -> Place {
        let [ex, ey] = self.entry();
        Place { x: pt[0] - (ex - self.x), y: pt[1] - (ey - self.y), ..self }
    }

    /// A task's box sits at the end of its label away from the fold ring (MAP-053): the far
    /// end where branches grow left, the near end in every other layout.
    fn task_far(&self) -> bool {
        self.dir == Dir::Left
    }

    /// Where a task's box `bx` wide sits in a node `room` wider for it, from the node's corner:
    /// its left edge, and the least and most x its hit target may take, which stays off the
    /// label.
    pub fn task_slot(&self, pad_x: f32, room: f32, bx: f32) -> (f32, [f32; 2]) {
        if self.task_far() { (self.w - pad_x - bx, [self.w - pad_x - room, f32::INFINITY]) } else { (pad_x, [f32::NEG_INFINITY, pad_x + room]) }
    }

    /// The world x span of the `room` a task's box takes beside the label.
    pub fn task_span(&self, pad_x: f32, room: f32) -> [f32; 2] {
        if self.task_far() { [self.right() - pad_x - room, self.right() - pad_x] } else { [self.x + pad_x, self.x + pad_x + room] }
    }

    /// The room left and right of the label in a node `room` wider for a task's box.
    pub fn label_pads(&self, pad_x: f32, room: f32) -> [f32; 2] {
        if self.task_far() { [pad_x, pad_x + room] } else { [pad_x + room, pad_x] }
    }

    /// Where a menu opened on the node from the keyboard hangs: under it, and under its fold
    /// ring where that is below it.
    pub fn menu_at(&self) -> [f32; 2] {
        let clear = if self.dir == Dir::Down { FOLD_DOT_GAP + 2.0 * FOLD_DOT_R } else { FOLD_HIT_BACK };
        [self.x, self.bottom() + clear + 4.0]
    }

    /// The node's span along the axis its siblings stack on, line included: top to bottom
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
    /// side: `[x, y, w, h]` from the corner of the node's box grown by `pad` all round. It
    /// leaves the frame at the underline of a node that has one.
    pub(crate) fn child_stub(&self, len: f32, thick: f32, pad: f32) -> [f32; 4] {
        let framed = Place { x: 0.0, y: 0.0, w: self.w + 2.0 * pad, h: self.h + 2.0 * pad, ..*self };
        let [x, y] = framed.side_mid(self.dir);
        let from = if self.dir.vertical() || self.depth == 0 { [x, y] } else { [x, self.line_y() - self.y + pad] };
        self.dir.bar_from(from, len, thick)
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
