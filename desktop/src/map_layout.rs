//! Where each shown map node sits (MAP-037, MAP-039, MAP-079): a tidy tree in world points,
//! growing the way the note's layout says, with the root's top-left at the origin so folding
//! never moves it.
//!
//! Each subtree is laid out on its own, then siblings stack in document order as close as
//! their profiles allow: a profile is the subtree's two edges across the branches over its
//! length along them, so a short branch tucks under a long one where they do not share
//! columns (d3-flextree's tidy tree).

use std::ops::Range;

use crate::map::{Add, Layout, Options, Tree, ROOT};
use crate::map_fold::Folds;
pub use crate::map_geom::Place;
use crate::map_geom::{line_room, line_width, Arrow, Branch, Dir, FOLD_DOT_GAP, FOLD_DOT_R, FOLD_HIT_BACK, Rel};
use crate::page::r;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The gap between a node and its children, along the branches.
    pub sh: f32,
    /// The gap between siblings, across the branches.
    pub sv: f32,
    pub pad_x: f32,
    pub pad_b: f32,
    pub min_h: f32,
    /// Where node text wraps.
    pub max_w: f32,
}

impl Metrics {
    /// The spacings at body size `b`, scaled by B / 14, with the note's own options.
    pub fn new(b: f32, o: &Options) -> Metrics {
        let rows = root_dir(o.layout).vertical();
        let max_w = match o.max_width {
            Some(w) if w <= 0.0 => 1200.0,
            Some(w) => w,
            None => r(b * if rows { ROW_WRAP } else { SIDE_WRAP }),
        };
        let level = r(b * 80.0 / 14.0);
        Metrics {
            sh: o.spacing_horizontal.unwrap_or(if rows { r(level * ROW_LEVEL) } else { level }),
            sv: o.spacing_vertical.unwrap_or_else(|| r(b * 14.0 / 14.0)),
            pad_x: r(b * 8.0 / 14.0),
            pad_b: r(b * 4.0 / 14.0),
            min_h: r(b * 16.0 / 14.0),
            max_w,
        }
    }
}

/// How far the selection glass (MAP-055) reaches past its node's box: left and right, then
/// above the node and below its underline.
pub const GLASS_PAD: [f32; 2] = [12.0, 10.0];
/// The least room between a node's glass and the node next to it.
const GLASS_CLEAR: f32 = 4.0;
/// Where branches grow down or up: the gap between siblings side by side, and between
/// neighbours with different parents, as multiples of the sibling spacing.
const ROW_GAP: f32 = 1.7;
const ROW_GAP_APART: f32 = 2.6;
/// The level gap where branches grow down or up, as a part of the one where they grow
/// sideways, and how many body sizes wide a node's text runs before it wraps there: a row of
/// siblings is as wide as its labels together, so both are tighter.
const ROW_LEVEL: f32 = 0.75;
const ROW_WRAP: f32 = 15.0;
/// How many body sizes wide a node's text runs before it wraps where branches grow sideways.
const SIDE_WRAP: f32 = 22.0;

pub struct Laid {
    /// Each node's box by index, None while a fold hides it.
    pub at: Vec<Option<Place>>,
    /// Each node's hit cell `[x0, y0, x1, y1]`: half the level gap past its ends along its
    /// branches, and half way to its neighbours across them, open where it has none. Task
    /// boxes, fold rings and drop bars stay inside it, so zoomed-out targets never overlap.
    pub cell: Vec<[f32; 4]>,
    /// The shown nodes in preorder.
    pub shown: Vec<usize>,
    /// `[x0, y0, x1, y1]` around every shown node.
    pub bounds: [f32; 4],
    pub metrics: Metrics,
    pub layout: Layout,
    pub sides: Sides,
}

/// What node leaves one side of a node: the way its branches grow and the nodes they hold, all
/// the way down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub dir: Dir,
    pub nodes: Range<usize>,
}

/// The root's sides, folded or not. Only Both's root has a second, and only when it has branches
/// for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sides {
    pub first: Side,
    pub second: Option<Side>,
}

impl Sides {
    fn each(&self) -> impl Iterator<Item = &Side> {
        std::iter::once(&self.first).chain(&self.second)
    }

    /// The way node `i`'s side of the root grows, None for the root.
    pub fn way(&self, i: usize) -> Option<Dir> {
        self.each().find(|s| s.nodes.contains(&i)).map(|s| s.dir)
    }

    /// Nodes `a` and `b` grow the same way: neither is across Both's root from the other.
    pub fn same(&self, a: usize, b: usize) -> bool {
        self.way(a) == self.way(b)
    }

    /// The side across the root from `d`.
    pub fn other(&self, d: Dir) -> Option<Dir> {
        self.second.as_ref()?;
        self.each().map(|s| s.dir).find(|&s| s != d)
    }
}

/// The sides of `t`'s root in the layout its note names. Both's root grows its first children
/// right and the rest left.
pub fn sides(t: &Tree) -> Sides {
    let all = Side { dir: root_dir(t.opts.layout), nodes: 1..t.nodes.len() };
    let left = (t.opts.layout == Layout::Both).then(|| both_split(t)).and_then(|k| t.nodes[ROOT].children.get(k));
    match left {
        Some(&k) => Sides { first: Side { nodes: 1..k, ..all.clone() }, second: Some(Side { dir: Dir::Left, nodes: k..all.nodes.end }) },
        None => Sides { first: all, second: None },
    }
}

/// A node's fold ring, or its `+N` pill when folded.
pub struct Ring {
    pub side: Side,
    pub folded: bool,
}

/// One stretch of a profile: over `from..to` along the branches, the subtree's leading edge
/// across them (towards earlier siblings) and the node it belongs to, and its trailing edge
/// and that node.
#[derive(Clone, Copy, Debug)]
struct Seg {
    from: f32,
    to: f32,
    lead: f32,
    lead_n: usize,
    trail: f32,
    trail_n: usize,
}

/// Lays out the nodes `folds` leave shown, the way `tree.opts.layout` says. `size` gives a
/// node's content size, before padding; it is asked only for shown nodes. `pill` gives the size
/// of a folded node's `+N` count pill, which takes room in its fold ring's place.
///
/// The tidy tree runs in a frame of its own: main is the axis the branches grow along and
/// cross the one siblings stack on, in document order. `to_world` turns that into the layout's
/// world, with the root's box at the origin in every layout.
pub fn layout(tree: &Tree, folds: &Folds, m: Metrics, mut size: impl FnMut(usize) -> [f32; 2], mut pill: impl FnMut(usize) -> [f32; 2]) -> Laid {
    let n = tree.nodes.len();
    let lay = tree.opts.layout;
    let vertical = root_dir(lay).vertical();
    let sides = sides(tree);
    let open = |i: usize| !folds.whole(tree.nodes[i].id);
    let mut shown = Vec::new();
    let mut seen = vec![false; n];
    let mut i = ROOT;
    while i < n {
        if folds.hides(tree, &sides, i) {
            i = tree.nodes[i].end;
            continue;
        }
        shown.push(i);
        seen[i] = true;
        i += 1;
    }
    let mut wh = vec![[0.0f32; 2]; n];
    for &i in &shown {
        let [cw, ch] = size(i);
        wh[i] = [cw + 2.0 * m.pad_x, ch.max(m.min_h) + m.pad_b];
    }
    let parent = |i: usize| tree.nodes[i].parent;
    let depth = |i: usize| tree.nodes[i].depth;
    // A node's length along main, its bracket included where that runs across the branches,
    // and its breadth across.
    let len = |v: usize| if vertical { wh[v][1] + line_room(depth(v), sides.first.dir) } else { wh[v][0] };
    let breadth = |v: usize| if vertical { wh[v][0] } else { wh[v][1] };
    // The gap below node `a` when node `b` sits under it: text leaves keep generous vertical
    // separation so each underline clearly belongs to its own row (R12) and parent groups read
    // distinctly (~1.7x cousin ratio), while framed cards keep enough gap for crisp drop bars
    // without pushing card-heavy maps below LITE_ZOOM at Fit. No gap is less than the selection
    // glass's reach plus a margin, so the glass round one node never covers its neighbour.
    // Where branches grow down or up the gap is between labels side by side, which need more
    // room to read apart than lines one above the other, and no line lies in it.
    let card_sv = r(m.sv * 0.58);
    let gap = |a: usize, b: usize| {
        let same = parent(a) == parent(b);
        let card = tree.nodes[a].card() || tree.nodes[b].card();
        if vertical {
            let base = r(m.sv * if same { ROW_GAP } else { ROW_GAP_APART });
            return (if card { r(base * 0.6) } else { base }).max(GLASS_PAD[0] + GLASS_CLEAR);
        }
        let base = if same {
            if tree.nodes[a].card() { card_sv } else { m.sv }
        } else if card {
            r(m.sv * 0.72)
        } else {
            r(m.sv * 1.7)
        };
        base.max(GLASS_PAD[1] + GLASS_CLEAR) + line_width(depth(a))
    };
    let mut profile: Vec<Option<Vec<Seg>>> = vec![None; n];
    let mut dy = vec![0.0f32; n];
    // Stacks `kids` in document order as close as their profiles allow, centred on the middle
    // of a parent `h` across, and gives their profile from the parent's corner, `col` along.
    let stack = |kids: &[usize], profile: &mut [Option<Vec<Seg>>], dy: &mut [f32], h: f32, col: f32| -> Vec<Seg> {
        let Some((&first, rest)) = kids.split_first() else { return Vec::new() };
        let mut acc = profile[first].take().unwrap_or_default();
        dy[first] = 0.0;
        for &c in rest {
            let p = profile[c].take().unwrap_or_default();
            let off = clearance(&acc, &p, &gap);
            dy[c] = off;
            acc = merge(&acc, &shift(&p, 0.0, off));
        }
        let last = *kids.last().unwrap_or(&first);
        let mid = (breadth(first) / 2.0 + dy[last] + breadth(last) / 2.0) / 2.0;
        let lift = mid - h / 2.0;
        for &c in kids {
            dy[c] -= lift;
        }
        shift(&acc, col, -lift)
    };
    for &v in shown.iter().rev() {
        let (w, h) = (len(v), breadth(v));
        let col = w + m.sh;
        let is_folded = !open(v) && !tree.nodes[v].children.is_empty();
        // Reserve room for a folded node's `+N` badge, centred on the node's middle where its
        // fold ring would be, so sibling Bézier links crossing `[w_sib, w_sib + sh]` never
        // slice through it and the sibling below clears its bottom edge.
        let mut prof = if is_folded {
            let [pill_w, pill_h] = pill(v);
            let (ext_w, half_h) = if vertical { (FOLD_DOT_GAP + pill_h + 4.0, pill_w / 2.0) } else { (FOLD_DOT_GAP + pill_w + 4.0, pill_h / 2.0) };
            let pill_bot = (h / 2.0 + half_h).max(h);
            // A pill is never taller than its node, and can be wider than a narrow one.
            let pill_top = if vertical { (h / 2.0 - half_h).min(0.0) } else { 0.0 };
            let split = (w - 6.0).max(0.0);
            if split > 0.0 {
                vec![
                    Seg { from: 0.0, to: split, lead: 0.0, lead_n: v, trail: h, trail_n: v },
                    Seg { from: split, to: col + ext_w, lead: pill_top, lead_n: v, trail: pill_bot, trail_n: v },
                ]
            } else {
                vec![Seg { from: 0.0, to: col + ext_w, lead: pill_top, lead_n: v, trail: pill_bot, trail_n: v }]
            }
        } else {
            vec![Seg { from: 0.0, to: col, lead: 0.0, lead_n: v, trail: h, trail_n: v }]
        };
        // Each of the root's sides stacks on its own, and nothing reads the root's profile.
        let kids = |of: &Range<usize>| -> Vec<usize> { tree.nodes[v].children.iter().copied().filter(|&c| seen[c] && of.contains(&c)).collect() };
        if v == ROOT {
            for s in sides.each() {
                stack(&kids(&s.nodes), &mut profile, &mut dy, h, col);
            }
        } else {
            prof.extend(stack(&kids(&(v..tree.nodes[v].end)), &mut profile, &mut dy, h, col));
        }
        profile[v] = Some(prof);
    }
    let root_len = [wh[ROOT][0], len(ROOT)];
    let mut at: Vec<Option<Place>> = vec![None; n];
    // Each shown node's corner in the local frame, `[main, cross]`.
    let mut local = vec![[0.0f32; 2]; n];
    let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for &v in &shown {
        let from = parent(v).and_then(|p| at[p].map(|q| (p, q)));
        let dir = sides.way(v).unwrap_or(sides.first.dir);
        local[v] = match from {
            Some((p, _)) => [local[p][0] + len(p) + m.sh, local[p][1] + dy[v]],
            None => [0.0, 0.0],
        };
        let [x, y] = to_world(dir, root_len, local[v], [wh[v][0], len(v)]);
        let pl = Place { x, y, w: wh[v][0], h: wh[v][1], depth: depth(v), dir };
        bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(pl.right()), bounds[3].max(pl.bottom())];
        at[v] = Some(pl);
    }
    Laid { cell: cells(&at, &shown, m.sh / 2.0), at, shown, bounds, metrics: m, layout: lay, sides }
}

/// The way the root's first branches grow in layout `l`.
fn root_dir(l: Layout) -> Dir {
    match l {
        Layout::Right | Layout::Both => Dir::Right,
        Layout::Left => Dir::Left,
        Layout::Down => Dir::Down,
        Layout::Up => Dir::Up,
    }
}

/// A node's corner in the world, from its corner `[main, cross]` in the local frame: `own` is
/// its width and its length along main, `root` the root's. The root's corner stays at the
/// origin whichever way the map grows.
fn to_world(dir: Dir, root: [f32; 2], [main, cross]: [f32; 2], own: [f32; 2]) -> [f32; 2] {
    let (k, fwd) = dir.axis();
    let main = if fwd { main } else { root[k] - main - own[k] };
    if k == 0 { [main, cross] } else { [cross, main] }
}

/// How many of the root's children grow right in Both; the rest grow left. The split that
/// best balances the leaves either side, counted in the whole tree so that a fold never moves
/// a branch across the root. A tie puts the extra branch on the right.
fn both_split(t: &Tree) -> usize {
    let kids = &t.nodes[ROOT].children;
    let leaves = |c: usize| t.nodes[c..t.nodes[c].end].iter().filter(|n| n.children.is_empty()).count() as i64;
    let weights: Vec<i64> = kids.iter().map(|&c| leaves(c)).collect();
    let total: i64 = weights.iter().sum();
    let mut before = 0;
    let mut best = (i64::MAX, 0);
    for k in 0..=kids.len() {
        let off = (2 * before - total).abs();
        if off <= best.0 {
            best = (off, k);
        }
        before += weights.get(k).copied().unwrap_or(0);
    }
    best.1
}

/// Each shown node's hit cell: `half` past its ends along its branches, and half way to the
/// nodes next to it across them wherever the two share a stretch along them.
fn cells(at: &[Option<Place>], shown: &[usize], half: f32) -> Vec<[f32; 4]> {
    const MAX_SCAN: f32 = 256.0;
    struct Span {
        v: usize,
        /// Along the branches, widened by `half`.
        main: [f32; 2],
        cross: [f32; 2],
        /// How far the cell reaches across.
        clamp: [f32; 2],
    }
    let mut by: Vec<Span> = shown
        .iter()
        .filter_map(|&v| at[v].map(|p| (v, p.main(), p.cross())))
        .map(|(v, [m0, m1], cross)| Span { v, main: [m0 - half, m1 + half], cross, clamp: [f32::NEG_INFINITY, f32::INFINITY] })
        .collect();
    by.sort_by(|a, b| a.cross[0].total_cmp(&b.cross[0]));
    for i in 0..by.len() {
        let ([a0, a1], a_end) = (by[i].main, by[i].cross[1]);
        for j in i + 1..by.len() {
            let ([b0, b1], b_start) = (by[j].main, by[j].cross[0]);
            if b_start - a_end > MAX_SCAN {
                break;
            }
            if b_start < a_end - 0.5 {
                continue;
            }
            if a0.max(b0) < a1.min(b1) {
                let mid = (a_end + b_start) / 2.0;
                by[i].clamp[1] = by[i].clamp[1].min(mid);
                by[j].clamp[0] = by[j].clamp[0].max(mid);
            }
        }
    }
    let mut cell = vec![[f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY]; at.len()];
    for Span { v, main: [m0, m1], clamp: [c0, c1], .. } in by {
        cell[v] = Dir::rect([m0, m1], [c0, c1], at[v].is_some_and(|p| p.dir.vertical()));
    }
    cell
}

/// A third of a node across its branches, for a drop (MAP-026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The side towards the sibling before.
    Lead,
    /// Between the two, and whether past the node's middle.
    Middle { late: bool },
    /// The side towards the sibling after.
    Trail,
}

impl Part {
    /// In the half towards the sibling after.
    pub fn late(self) -> bool {
        matches!(self, Part::Trail | Part::Middle { late: true })
    }
}

/// How long the stub of a branch a Child drop shows is.
const STUB: f32 = 24.0;

/// Where a drop mark draws, in world points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropMark {
    /// Before or After: a bar `[x, y, w, h]` between the target and its neighbour.
    Bar([f32; 4]),
    /// Child: a frame `[x, y, w, h]` round the target with a border `edge` wide, and the stub
    /// of a branch out of its growing side, `[x, y, w, h]` from the frame's corner.
    Child { frame: [f32; 4], stub: [f32; 4], edge: f32 },
}

impl Laid {
    /// The branch into node `i` from its parent.
    pub fn branch(&self, t: &Tree, i: usize) -> Option<Branch> {
        let p = self.at[i]?;
        let q = self.at[t.nodes[i].parent?]?;
        Some(Branch { from: q.branch_start(p.dir), to: p.entry(), end: p.underline_end(), dir: p.dir, lw: line_width(p.depth) })
    }

    /// The branch into node `i` where no fold ring is drawn: it leaves its parent's edge.
    pub fn ringless_branch(&self, t: &Tree, i: usize) -> Option<Branch> {
        let b = self.branch(t, i)?;
        let q = self.at[t.nodes[i].parent?]?;
        Some(Branch { from: q.side_mid(b.dir), ..b })
    }

    /// Node `i`'s fold rings: one on its growing side, and for the root one on each of its sides.
    pub fn rings(&self, t: &Tree, i: usize, folds: &Folds) -> Vec<Ring> {
        let whole = folds.whole(t.nodes[i].id);
        let half = folds.side(&self.sides);
        if i == ROOT {
            return self.sides.each().map(|s| Ring { folded: whole || half == Some(s.dir), side: s.clone() }).collect();
        }
        self.at[i].map(|p| Ring { side: Side { dir: p.dir, nodes: i + 1..t.nodes[i].end }, folded: whole }).into_iter().collect()
    }

    /// What arrow `a` means on node `i`. Both's root has children either side, so its Left and
    /// Right each go to the first child that way.
    pub fn toward(&self, i: usize, a: Arrow) -> Option<Rel> {
        let p = self.at[i]?;
        if i == ROOT && self.layout == Layout::Both {
            let d = match a {
                Arrow::Right => Dir::Right,
                Arrow::Left => Dir::Left,
                Arrow::Up | Arrow::Down => return None,
            };
            return self.sides.each().any(|s| s.dir == d).then_some(Rel::Child(d));
        }
        Some(p.dir.rel(a))
    }

    /// The hit box of node `i`'s fold ring on side `d`, `[x0, y0, x1, y1]`: `hit` wide where
    /// there is room. It never reaches far over the node's own label, nor out of the node's
    /// cell into the rows beside it, the next level's task boxes or a folded node's pill, and
    /// it always holds the ring.
    pub fn ring_hit(&self, i: usize, d: Dir, hit: f32) -> Option<[f32; 4]> {
        let p = self.at[i]?;
        let (c, edge) = (p.ring(d), p.side_mid(d));
        let [x0, y0, x1, y1] = self.cell[i];
        let (lo, hi, r) = ([x0, y0], [x1, y1], FOLD_DOT_R);
        let (k, fwd) = d.axis();
        // Along the branches the box starts `FOLD_HIT_BACK` back over the node and grows away from
        // it. Measured outwards, so one rule serves a ring on either end of the axis.
        let s = if fwd { 1.0 } else { -1.0 };
        let near = (s * c[k] - hit / 2.0).max(s * edge[k] - FOLD_HIT_BACK);
        let far = (near + hit).min(s * if fwd { hi[k] } else { lo[k] }).max(s * c[k] + r);
        let main = if fwd { [near, far] } else { [-far, -near] };
        let j = 1 - k;
        let cross = [(c[j] - hit / 2.0).max(lo[j]).min(c[j] - r), (c[j] + hit / 2.0).min(hi[j]).max(c[j] + r)];
        Some(Dir::rect(main, cross, d.vertical()))
    }

    /// The part of node `j` a drop at world point `w` is on, None when it is not on `j`. The
    /// node splits across its branches into a quarter, a half and a quarter, with the outer
    /// two at least `band` wide and reaching half way to the siblings either side.
    pub fn part_at(&self, t: &Tree, j: usize, w: [f32; 2], band: f32) -> Option<Part> {
        let p = self.at[j]?;
        let (m, c) = p.along(w);
        let [m0, m1] = p.main();
        if m < m0 || m > m1 {
            return None;
        }
        let [a, b] = p.cross();
        let half_gap = self.metrics.sv / 2.0;
        let sibs: Vec<(usize, [f32; 2])> = t.nodes[j].parent.map_or(Vec::new(), |q| t.nodes[q].children.iter().filter_map(|&k| self.at[k].filter(|s| s.dir == p.dir).map(|s| (k, s.cross()))).collect());
        let k = sibs.iter().position(|&(k, _)| k == j);
        let before = k.and_then(|k| k.checked_sub(1)).map_or(half_gap, |k| ((a - sibs[k].1[1]) / 2.0).max(0.0));
        let after = k.and_then(|k| sibs.get(k + 1)).map_or(half_gap, |s| ((s.1[0] - b) / 2.0).max(0.0));
        if c < a - before || c > b + after {
            return None;
        }
        let edge = ((b - a) / 4.0).max(band).min((b - a) / 2.0);
        Some(if c < a + edge {
            Part::Lead
        } else if c > b - edge {
            Part::Trail
        } else {
            Part::Middle { late: c >= a + (b - a) / 2.0 }
        })
    }

    /// Where the mark for a drop at `at` draws at zoom `zoom`: a bar across the target's
    /// branches for Before and After, centred in the half gap to its neighbour where it has
    /// one, or a frame round it with a stub of a branch for Child.
    pub fn drop_mark(&self, at: Add, zoom: f32) -> Option<DropMark> {
        let j = at.node();
        let p = self.at[j]?;
        Some(match at {
            Add::Before(_) | Add::After(_) => {
                let desired = (2.5 / zoom).clamp(3.0, 20.0);
                let [a, b] = p.cross();
                let [x0, y0, x1, y1] = self.cell[j];
                let [lo, hi] = if p.dir.vertical() { [x0, x1] } else { [y0, y1] };
                let fit = |half_gap: f32| desired.min((2.0 * half_gap - 2.0).max(2.5));
                let (thick, c) = if matches!(at, Add::Before(_)) {
                    if lo.is_finite() {
                        let half_gap = (a - lo).max(0.0);
                        let thick = fit(half_gap);
                        (thick, a - half_gap.min(thick / 2.0 + 3.0) - thick / 2.0)
                    } else {
                        (desired, a - desired - 2.0)
                    }
                } else if hi.is_finite() {
                    let half_gap = (hi - b).max(0.0);
                    let thick = fit(half_gap);
                    (thick, b + half_gap.min(thick / 2.0 + 3.0) - thick / 2.0)
                } else {
                    (desired, b + 2.0)
                };
                DropMark::Bar(p.bar(c, thick))
            }
            Add::Child(_) => {
                let edge = (2.0 / zoom).clamp(2.0, 16.0);
                let pad = 3.0 + (edge - 2.0);
                let frame = [p.x - pad, p.y - pad, p.w + 2.0 * pad, p.bottom() - p.y + 2.0 * pad];
                DropMark::Child { frame, stub: p.child_stub(STUB, edge, pad), edge }
            }
        })
    }

    /// The lowest a Child drop on node `j` marks: the bottom of the node, or of the stub out
    /// of it.
    pub fn child_mark_bottom(&self, j: usize) -> Option<f32> {
        let p = self.at[j]?;
        let [_, y, _, h] = p.child_stub(STUB, 0.0, 0.0);
        Some(p.bottom().max(p.y + y + h))
    }

    /// The Properties badge is under the root, not over it.
    pub fn badge_under(&self) -> bool {
        self.layout != Layout::Down
    }

    /// The Properties badge of `size` by the root, `[x, y, w, h]`, clear of its fold rings:
    /// under the root, at the end away from its ring or in the middle between two; above it
    /// where the ring is under it.
    pub fn badge_box(&self, [w, h]: [f32; 2], pad_x: f32) -> Option<[f32; 4]> {
        let p = self.at[ROOT]?;
        let inset = pad_x.max(FOLD_HIT_BACK + 2.0);
        let x = match self.layout {
            Layout::Right | Layout::Up | Layout::Down => p.right() - inset - w,
            Layout::Left => p.x + inset,
            Layout::Both => p.x + (p.w - w) / 2.0,
        };
        Some([x, if self.badge_under() { p.bottom() + 6.0 } else { p.y - 6.0 - h }, w, h])
    }

    /// The pan that opens on node `i` at zoom `z` in the canvas's free part `free`: the node's
    /// back edge a little in from the side its branches grow away from, so they fill the
    /// view, and its middle half way across; Both's root in the middle (MAP-065).
    pub fn opening_pan(&self, i: usize, [fx0, fy0, fx1, fy1]: [f32; 4], z: f32) -> Option<[f32; 2]> {
        let p = self.at[i]?;
        let inset = |room: f32| (room * 0.07).clamp(32.0, 56.0);
        let (mid_x, mid_y) = (fx0 + (fx1 - fx0) / 2.0 - (p.x + p.w / 2.0) * z, fy0 + (fy1 - fy0) / 2.0 - (p.y + p.h / 2.0) * z);
        if i == ROOT && self.layout == Layout::Both {
            return Some([mid_x, mid_y]);
        }
        let (k, fwd) = p.dir.axis();
        let ([lo, hi], [back, front]) = ([[fx0, fy0], [fx1, fy1]], p.main());
        let mut pan = [mid_x, mid_y];
        pan[k] = if fwd { lo[k] + inset(hi[k] - lo[k]) - back * z } else { hi[k] - inset(hi[k] - lo[k]) - front * z };
        Some(pan)
    }
}

fn shift(p: &[Seg], along: f32, across: f32) -> Vec<Seg> {
    p.iter().map(|s| Seg { from: s.from + along, to: s.to + along, lead: s.lead + across, trail: s.trail + across, ..*s }).collect()
}

/// How far across `b` must move to clear `a` wherever they share a stretch along.
fn clearance(a: &[Seg], b: &[Seg], gap: &impl Fn(usize, usize) -> f32) -> f32 {
    let mut need = f32::MIN;
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let (sa, sb) = (a[i], b[j]);
        if sa.from.max(sb.from) < sa.to.min(sb.to) {
            need = need.max(sa.trail + gap(sa.trail_n, sb.lead_n) - sb.lead);
        }
        if sa.to <= sb.to {
            i += 1;
        } else {
            j += 1;
        }
    }
    if need == f32::MIN { 0.0 } else { need }
}

/// The profile of `a` and `b` together.
fn merge(a: &[Seg], b: &[Seg]) -> Vec<Seg> {
    let mut cuts: Vec<f32> = a.iter().chain(b).flat_map(|s| [s.from, s.to]).collect();
    cuts.sort_by(f32::total_cmp);
    cuts.dedup();
    let mut out: Vec<Seg> = Vec::with_capacity(cuts.len());
    let (mut i, mut j) = (0, 0);
    for w in cuts.windows(2) {
        let (from, to) = (w[0], w[1]);
        while i < a.len() && a[i].to <= from {
            i += 1;
        }
        while j < b.len() && b[j].to <= from {
            j += 1;
        }
        let sa = a.get(i).filter(|s| s.from <= from);
        let sb = b.get(j).filter(|s| s.from <= from);
        let s = match (sa, sb) {
            (Some(sa), Some(sb)) => {
                let (lead, lead_n) = if sb.lead < sa.lead { (sb.lead, sb.lead_n) } else { (sa.lead, sa.lead_n) };
                let (trail, trail_n) = if sb.trail > sa.trail { (sb.trail, sb.trail_n) } else { (sa.trail, sa.trail_n) };
                Seg { from, to, lead, lead_n, trail, trail_n }
            }
            (Some(s), None) | (None, Some(s)) => Seg { from, to, ..*s },
            (None, None) => continue,
        };
        match out.last_mut() {
            Some(l) if l.to == from && l.lead == s.lead && l.lead_n == s.lead_n && l.trail == s.trail && l.trail_n == s.trail_n => l.to = to,
            _ => out.push(s),
        }
    }
    out
}
