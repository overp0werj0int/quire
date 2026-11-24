//! Where each shown map node sits (MAP-037, MAP-039): a left-to-right tidy tree in world
//! points, with the root's top-left at the origin so folding never moves it.
//!
//! Each subtree is laid out on its own, then siblings stack top to bottom as close as their
//! profiles allow: a profile is the subtree's top and bottom edge over x, so a short branch
//! tucks under a long one where they do not share columns (d3-flextree's tidy tree).

use std::collections::HashSet;

use crate::map::{NodeId, Options, Tree, ROOT};
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

/// A branch's stroke and underline width at `depth`: 4 at the root, then towards 1.
pub fn line_width(depth: usize) -> f32 {
    1.0 + 3.0 / 2f32.powi(depth.min(30) as i32)
}

/// A node's padded box in world points. Its underline runs below the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub depth: usize,
}

impl Place {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Where the underline and the links meet the node.
    pub fn line_y(&self) -> f32 {
        self.y + self.h + line_width(self.depth) / 2.0
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h + line_width(self.depth)
    }
}

pub struct Laid {
    /// Each node's box by index, None while a fold hides it.
    pub at: Vec<Option<Place>>,
    /// Vertical `[top, bottom]` hit-target bounds per node so zoomed-out checkboxes and fold
    /// circles never overlap adjacent rows in the same horizontal band.
    pub clear: Vec<[f32; 2]>,
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
/// padding; it is asked only for shown nodes. `pill_ext` gives `[width_ext, half_h]` right of
/// a folded node's right edge occupied by its fold circle and `+N` count pill.
pub fn layout(tree: &Tree, folded: &HashSet<NodeId>, m: Metrics, mut size: impl FnMut(usize) -> [f32; 2], mut pill_ext: impl FnMut(usize) -> [f32; 2]) -> Laid {
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
            let [ext_w, half_h] = pill_ext(v);
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
        let pl = Place { x, y, w: wh[v][0], h: wh[v][1], depth: depth(v) };
        bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(pl.right()), bounds[3].max(pl.bottom())];
        at[v] = Some(pl);
    }
    let mut clear = vec![[f32::NEG_INFINITY, f32::INFINITY]; n];
    let mut by_y: Vec<(usize, Place)> = shown.iter().filter_map(|&v| at[v].map(|p| (v, p))).collect();
    by_y.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    const MAX_SCAN: f32 = 256.0;
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
                clear[a][1] = clear[a][1].min(mid);
                clear[b][0] = clear[b][0].max(mid);
            }
        }
    }
    Laid { at, clear, shown, bounds, metrics: m }
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
