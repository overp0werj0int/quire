//! Where each shown map node sits (MAP-037, MAP-039): a left-to-right tidy tree in world
//! points, with the root's top-left at the origin so folding never moves it.
//!
//! Each subtree is laid out on its own, then siblings stack top to bottom as close as their
//! profiles allow: a profile is the subtree's top and bottom edge over x, so a short branch
//! tucks under a long one where they do not share columns (d3-flextree's tidy tree).

use std::collections::HashSet;

use crate::map::{Add, NodeId, Options, Tree, ROOT};
pub use crate::map_geom::{Place, line_width};
use crate::map_geom::{Branch, Dir, FOLD_DOT_GAP, FOLD_DOT_R, FOLD_R};
use crate::page::r;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub sh: f32,
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
        let max_w = match o.max_width {
            Some(w) if w <= 0.0 => 1200.0,
            Some(w) => w,
            None => r(b * 22.0),
        };
        Metrics {
            sh: o.spacing_horizontal.unwrap_or_else(|| r(b * 80.0 / 14.0)),
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
}

/// One stretch of a profile: over `x0..x1`, the subtree's top edge and the node it belongs
/// to, and its bottom edge and that node.
#[derive(Clone, Copy, Debug)]
struct Seg {
    x0: f32,
    x1: f32,
    top: f32,
    top_n: usize,
    bot: f32,
    bot_n: usize,
}

/// Lays out the nodes `folded` leaves shown. `size` gives a node's content size, before
/// padding; it is asked only for shown nodes. `pill` gives the size of a folded node's `+N` count
/// pill, which takes room in its fold ring's place.
pub fn layout(tree: &Tree, folded: &HashSet<NodeId>, m: Metrics, mut size: impl FnMut(usize) -> [f32; 2], mut pill: impl FnMut(usize) -> [f32; 2]) -> Laid {
    let n = tree.nodes.len();
    let mut shown = Vec::new();
    let mut i = ROOT;
    while i < n {
        shown.push(i);
        i = if folded.contains(&tree.nodes[i].id) { tree.nodes[i].end } else { i + 1 };
    }
    let open = |i: usize| !folded.contains(&tree.nodes[i].id);
    let mut wh = vec![[0.0f32; 2]; n];
    for &i in &shown {
        let [cw, ch] = size(i);
        wh[i] = [cw + 2.0 * m.pad_x, ch.max(m.min_h) + m.pad_b];
    }
    let parent = |i: usize| tree.nodes[i].parent;
    let depth = |i: usize| tree.nodes[i].depth;
    // The gap below node `a` when node `b` sits under it: text leaves keep generous vertical
    // separation so each underline clearly belongs to its own row (R12) and parent groups read
    // distinctly (~1.7x cousin ratio), while framed cards keep enough gap for crisp drop bars
    // without pushing card-heavy maps below LITE_ZOOM at Fit. No gap is less than the selection
    // glass's reach plus a margin, so the glass round one node never covers its neighbour.
    let card_sv = r(m.sv * 0.58);
    let gap = |a: usize, b: usize| {
        let same = parent(a) == parent(b);
        let base = if same {
            if tree.nodes[a].card() { card_sv } else { m.sv }
        } else if tree.nodes[a].card() || tree.nodes[b].card() {
            r(m.sv * 0.72)
        } else {
            r(m.sv * 1.7)
        };
        base.max(GLASS_PAD[1] + GLASS_CLEAR) + line_width(depth(a))
    };
    let mut profile: Vec<Option<Vec<Seg>>> = vec![None; n];
    let mut dy = vec![0.0f32; n];
    for &v in shown.iter().rev() {
        let [w, h] = wh[v];
        let col = w + m.sh;
        let is_folded = !open(v) && !tree.nodes[v].children.is_empty();
        // Reserve room for a folded node's `+N` badge, centred on the node's middle where its
        // fold ring would be, so sibling Bézier links crossing `[w_sib, w_sib + sh]` never
        // slice through it and the sibling below clears its bottom edge.
        let mut prof = if is_folded {
            let [pill_w, pill_h] = pill(v);
            let [ext_w, half_h] = [FOLD_DOT_GAP + pill_w + 4.0, pill_h / 2.0];
            let pill_bot = (h / 2.0 + half_h).max(h);
            let split = (w - 6.0).max(0.0);
            if split > 0.0 {
                vec![
                    Seg { x0: 0.0, x1: split, top: 0.0, top_n: v, bot: h, bot_n: v },
                    Seg { x0: split, x1: col + ext_w, top: 0.0, top_n: v, bot: pill_bot, bot_n: v },
                ]
            } else {
                vec![Seg { x0: 0.0, x1: col + ext_w, top: 0.0, top_n: v, bot: pill_bot, bot_n: v }]
            }
        } else {
            vec![Seg { x0: 0.0, x1: col, top: 0.0, top_n: v, bot: h, bot_n: v }]
        };
        let kids: &[usize] = if open(v) { &tree.nodes[v].children } else { &[] };
        if let Some((&first, rest)) = kids.split_first() {
            let mut acc = profile[first].take().unwrap_or_default();
            dy[first] = 0.0;
            for &c in rest {
                let p = profile[c].take().unwrap_or_default();
                let off = clearance(&acc, &p, &gap);
                dy[c] = off;
                acc = merge(&acc, &shift(&p, 0.0, off));
            }
            let last = *kids.last().unwrap_or(&first);
            let mid = (wh[first][1] / 2.0 + dy[last] + wh[last][1] / 2.0) / 2.0;
            let lift = mid - h / 2.0;
            for &c in kids {
                dy[c] -= lift;
            }
            prof.extend(shift(&acc, col, -lift));
        }
        profile[v] = Some(prof);
    }
    let mut at: Vec<Option<Place>> = vec![None; n];
    let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for &v in &shown {
        let (x, y) = match parent(v).and_then(|p| at[p]) {
            Some(p) => (p.right() + m.sh, p.y + dy[v]),
            None => (0.0, 0.0),
        };
        let pl = Place { x, y, w: wh[v][0], h: wh[v][1], depth: depth(v), dir: Dir::Right };
        bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(pl.right()), bounds[3].max(pl.bottom())];
        at[v] = Some(pl);
    }
    let mut cell = vec![[f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY]; n];
    let mut by_y: Vec<(usize, Place)> = shown.iter().filter_map(|&v| at[v].map(|p| (v, p))).collect();
    by_y.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    const MAX_SCAN: f32 = 256.0;
    for &(v, p) in &by_y {
        cell[v][0] = p.x - m.sh / 2.0;
        cell[v][2] = p.right() + m.sh / 2.0;
    }
    for idx in 0..by_y.len() {
        let (a, pa) = by_y[idx];
        let (ax0, ax1) = (pa.x - m.sh / 2.0, pa.right() + m.sh / 2.0);
        for &(b, pb) in &by_y[idx + 1..] {
            if pb.y - pa.bottom() > MAX_SCAN {
                break;
            }
            if pb.y < pa.bottom() - 0.5 {
                continue;
            }
            let (bx0, bx1) = (pb.x - m.sh / 2.0, pb.right() + m.sh / 2.0);
            if ax0.max(bx0) < ax1.min(bx1) {
                let mid = (pa.bottom() + pb.y) / 2.0;
                cell[a][3] = cell[a][3].min(mid);
                cell[b][1] = cell[b][1].max(mid);
            }
        }
    }
    Laid { at, cell, shown, bounds, metrics: m }
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

    /// The sides node `i` has a fold ring on, or a `+N` pill when folded.
    pub fn rings(&self, i: usize) -> impl Iterator<Item = Dir> {
        self.at[i].map(|p| p.dir).into_iter()
    }

    /// The hit box of node `i`'s fold ring on side `d`, `[x0, y0, x1, y1]`: `hit` wide where
    /// there is room. It never reaches far over the node's own label, nor out of the node's
    /// cell into the rows beside it, the next level's task boxes or a folded node's pill, and
    /// it always holds the ring.
    pub fn ring_hit(&self, i: usize, d: Dir, hit: f32) -> Option<[f32; 4]> {
        let p = self.at[i]?;
        let [cx, cy] = p.ring(d);
        let [x0, y0, x1, y1] = self.cell[i];
        let r = FOLD_DOT_R;
        // A box that starts at `near`, `FOLD_R` back over the node, and grows away from it.
        let out = |c: f32, near: f32, far: f32| {
            let a = (c - hit / 2.0).max(near);
            (a, (a + hit).min(far).max(c + r))
        };
        let back = |c: f32, near: f32, far: f32| {
            let b = (c + hit / 2.0).min(near);
            ((b - hit).max(far).min(c - r), b)
        };
        let beside = |c: f32, lo: f32, hi: f32| ((c - hit / 2.0).max(lo).min(c - r), (c + hit / 2.0).min(hi).max(c + r));
        Some(match d {
            Dir::Right => {
                let ((a, b), (c, e)) = (out(cx, p.right() - FOLD_R, x1), beside(cy, y0, y1));
                [a, c, b, e]
            }
            Dir::Left => {
                let ((a, b), (c, e)) = (back(cx, p.x + FOLD_R, x0), beside(cy, y0, y1));
                [a, c, b, e]
            }
            Dir::Down => {
                let ((a, b), (c, e)) = (beside(cx, x0, x1), out(cy, p.bottom() - FOLD_R, y1));
                [a, c, b, e]
            }
            Dir::Up => {
                let ((a, b), (c, e)) = (beside(cx, x0, x1), back(cy, p.y + FOLD_R, y0));
                [a, c, b, e]
            }
        })
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
        let sibs: Vec<(usize, [f32; 2])> = t.nodes[j].parent.map_or(Vec::new(), |q| t.nodes[q].children.iter().filter_map(|&k| self.at[k].map(|s| (k, s.cross()))).collect());
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
                DropMark::Child { frame, stub: p.child_stub(24.0, edge, pad), edge }
            }
        })
    }

    /// The Properties badge of `size` under the root, `[x, y, w, h]`: right-aligned, clear of
    /// its fold ring.
    pub fn badge_box(&self, [w, h]: [f32; 2], pad_x: f32) -> Option<[f32; 4]> {
        let p = self.at[ROOT]?;
        Some([p.right() - pad_x.max(FOLD_R + 2.0) - w, p.bottom() + 6.0, w, h])
    }

    /// The pan that opens on node `i` at zoom `z` in the canvas's free part `free`: the node's
    /// back edge a little in from the side its branches grow away from, so they fill the
    /// view, and its middle half way across (MAP-065).
    pub fn opening_pan(&self, i: usize, [fx0, fy0, fx1, fy1]: [f32; 4], z: f32) -> Option<[f32; 2]> {
        let p = self.at[i]?;
        let inset = |room: f32| (room * 0.07).clamp(32.0, 56.0);
        let (mid_x, mid_y) = (fx0 + (fx1 - fx0) / 2.0 - (p.x + p.w / 2.0) * z, fy0 + (fy1 - fy0) / 2.0 - (p.y + p.h / 2.0) * z);
        Some(match p.dir {
            Dir::Right => [fx0 + inset(fx1 - fx0) - p.x * z, mid_y],
            Dir::Left => [fx1 - inset(fx1 - fx0) - p.right() * z, mid_y],
            Dir::Down => [mid_x, fy0 + inset(fy1 - fy0) - p.y * z],
            Dir::Up => [mid_x, fy1 - inset(fy1 - fy0) - p.bottom() * z],
        })
    }
}

fn shift(p: &[Seg], dx: f32, dy: f32) -> Vec<Seg> {
    p.iter().map(|s| Seg { x0: s.x0 + dx, x1: s.x1 + dx, top: s.top + dy, bot: s.bot + dy, ..*s }).collect()
}

/// How far down `b` must move to clear `a` wherever their columns overlap.
fn clearance(a: &[Seg], b: &[Seg], gap: &impl Fn(usize, usize) -> f32) -> f32 {
    let mut need = f32::MIN;
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let (sa, sb) = (a[i], b[j]);
        if sa.x0.max(sb.x0) < sa.x1.min(sb.x1) {
            need = need.max(sa.bot + gap(sa.bot_n, sb.top_n) - sb.top);
        }
        if sa.x1 <= sb.x1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    if need == f32::MIN { 0.0 } else { need }
}

/// The profile of `a` and `b` together.
fn merge(a: &[Seg], b: &[Seg]) -> Vec<Seg> {
    let mut xs: Vec<f32> = a.iter().chain(b).flat_map(|s| [s.x0, s.x1]).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let mut out: Vec<Seg> = Vec::with_capacity(xs.len());
    let (mut i, mut j) = (0, 0);
    for w in xs.windows(2) {
        let (x0, x1) = (w[0], w[1]);
        while i < a.len() && a[i].x1 <= x0 {
            i += 1;
        }
        while j < b.len() && b[j].x1 <= x0 {
            j += 1;
        }
        let sa = a.get(i).filter(|s| s.x0 <= x0);
        let sb = b.get(j).filter(|s| s.x0 <= x0);
        let s = match (sa, sb) {
            (Some(sa), Some(sb)) => {
                let (top, top_n) = if sb.top < sa.top { (sb.top, sb.top_n) } else { (sa.top, sa.top_n) };
                let (bot, bot_n) = if sb.bot > sa.bot { (sb.bot, sb.bot_n) } else { (sa.bot, sa.bot_n) };
                Seg { x0, x1, top, top_n, bot, bot_n }
            }
            (Some(s), None) | (None, Some(s)) => Seg { x0, x1, ..*s },
            (None, None) => continue,
        };
        match out.last_mut() {
            Some(l) if l.x1 == x0 && l.top == s.top && l.top_n == s.top_n && l.bot == s.bot && l.bot_n == s.bot_n => l.x1 = x1,
            _ => out.push(s),
        }
    }
    out
}
