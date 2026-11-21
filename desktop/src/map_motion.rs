//! The map's nodes gliding from one layout to the next (MAP-043): after a fold, an edit or a
//! move, each node goes from where it was drawn to its new place over 180 ms, ease-out cubic,
//! so a moved branch travels from where it was. Nodes a fold shows grow out of their parent's
//! right anchor and nodes it hides go back into it. Under reduced motion nothing glides.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use rune::Tx;

use crate::app::{App, Mode};
use crate::map::{self, NodeId, ROOT, Tree};
use crate::map_layout::{Laid, Place};

const RELAYOUT: f64 = 0.18;

pub(crate) struct Relayout {
    rel: String,
    /// Where each node of the new tree was drawn when the change came.
    from: HashMap<NodeId, Place>,
    start: f64,
}

/// The map as one frame draws it.
pub struct Drawn {
    pub laid: Rc<Laid>,
    /// Each node's opacity while it fades in or out; empty when none does.
    fade: Vec<f32>,
}

impl Drawn {
    pub fn alpha(&self, i: usize) -> f32 {
        self.fade.get(i).copied().unwrap_or(1.0)
    }
}

/// The last `Drawn`, with what it was drawn from: the view draws through `map_drawn`, so this
/// is what is on screen, which the next change glides from.
pub(crate) struct Frame {
    rel: String,
    now: f64,
    start: Option<f64>,
    target: Rc<Laid>,
    tree: Rc<Tree>,
    drawn: Rc<Drawn>,
}

impl App {
    /// Where the nodes are drawn now: their places in the layout, or on the way there from
    /// the last change's (MAP-043).
    pub fn map_drawn(&self) -> Rc<Drawn> {
        let laid = self.map_laid();
        let tree = self.map_tree();
        let now = self.now.get();
        let rel = self.active.clone().unwrap_or_default();
        let r = self.map.relayout.as_ref().filter(|r| r.rel == rel && now - r.start < RELAYOUT);
        let start = r.map(|r| r.start);
        if let Some(f) = &*self.map.frame.borrow()
            && Rc::ptr_eq(&f.target, &laid)
            && Rc::ptr_eq(&f.tree, &tree)
            && (f.now, f.start) == (now, start)
            && f.rel == rel
        {
            return f.drawn.clone();
        }
        let drawn = Rc::new(match r {
            Some(r) => {
                let u = ((now - r.start) / RELAYOUT).max(0.0) as f32;
                blend(&tree, &laid, &r.from, 1.0 - (1.0 - u).powi(3))
            }
            None => Drawn { laid: laid.clone(), fade: Vec::new() },
        });
        *self.map.frame.borrow_mut() = Some(Frame { rel, now, start, target: laid, tree, drawn: drawn.clone() });
        drawn
    }

    /// After a change to the map shown, its text or its folds: the nodes glide from where the
    /// last frame drew them. `change` is the tree the text was and the splice that made it
    /// what it is, which says where a moved node went.
    pub(crate) fn map_relayout(&mut self, change: Option<(&Tree, &map::Splice)>, tx: &mut Tx) {
        let last = self.map.frame.borrow().as_ref().filter(|f| Some(f.rel.as_str()) == self.active.as_deref()).map(|f| (f.tree.clone(), f.drawn.clone()));
        let Some((old, drawn)) = last else { return };
        if crate::motion::reduced() || self.mode != Mode::Map {
            self.map.relayout = None;
            return;
        }
        let t = self.map_tree();
        let laid = self.map_laid();
        let s = change.filter(|(o, _)| std::ptr::eq(*o, &*old)).map(|(_, s)| s);
        let from: HashMap<NodeId, Place> = pair(&old, &t, s).into_iter().filter_map(|(id, k)| drawn.laid.at[k].map(|p| (id, p))).collect();
        if t.nodes.iter().zip(&laid.at).all(|(n, p)| from.get(&n.id) == p.as_ref()) {
            return;
        }
        self.map.relayout = Some(Relayout { rel: self.active.clone().unwrap_or_default(), from, start: self.now.get() });
        tx.wake_after(Duration::from_millis(16));
    }

    /// A relayout glide runs on the map shown. Rune's own layout motion stays off the map's
    /// world meanwhile: it pairs elements by key, and a node's key is its source line, which a
    /// move or an insertion changes.
    pub fn map_gliding(&self) -> bool {
        self.map.relayout.as_ref().is_some_and(|r| Some(r.rel.as_str()) == self.active.as_deref())
    }

    /// Before each frame: the glide's next step. True while it runs, and on its last frame.
    pub(crate) fn poll_relayout(&mut self, t: f64, tx: &mut Tx) -> bool {
        let Some(r) = &self.map.relayout else { return false };
        if t - r.start >= RELAYOUT {
            self.map.relayout = None;
        } else {
            tx.wake_after(Duration::from_millis(16));
        }
        true
    }
}

/// Each node of `new` by the index of the node of `old` it was: the same id; else the node
/// splice `s` moved there; else, under the same parent and in the same place, a node whose id
/// went because its text changed (a rename, or a new node's first name).
fn pair(old: &Tree, new: &Tree, s: Option<&map::Splice>) -> HashMap<NodeId, usize> {
    let ids: HashSet<NodeId> = new.nodes.iter().map(|n| n.id).collect();
    let gone = |k: usize| !ids.contains(&old.nodes[k].id);
    let mut was: HashMap<NodeId, usize> = old.nodes.iter().enumerate().filter(|&(k, _)| !gone(k)).map(|(k, n)| (n.id, k)).collect();
    for k in (0..old.nodes.len()).filter(|&k| gone(k)) {
        if let Some(id) = s.and_then(|s| map::follow(old, new, s, old.nodes[k].id)) {
            was.entry(id).or_insert(k);
        }
    }
    was.entry(new.nodes[ROOT].id).or_insert(ROOT);
    let mut used: HashSet<usize> = was.values().copied().collect();
    for (i, n) in new.nodes.iter().enumerate() {
        let Some(p) = n.parent.filter(|_| !was.contains_key(&n.id)) else { continue };
        let Some(&op) = was.get(&new.nodes[p].id) else { continue };
        let k = new.nodes[p].children.iter().position(|&c| c == i);
        if let Some(&oc) = k.and_then(|k| old.nodes[op].children.get(k))
            && gone(oc)
            && used.insert(oc)
        {
            was.insert(n.id, oc);
        }
    }
    was
}

/// The layout `e` of the way from the places in `from` to `laid`'s. A node with no place to
/// come from grows out of its nearest ancestor's right anchor; a node the layout hides goes
/// into its nearest shown ancestor's.
fn blend(t: &Tree, laid: &Laid, from: &HashMap<NodeId, Place>, e: f32) -> Drawn {
    let mut at = laid.at.clone();
    let mut fade = vec![1.0; t.nodes.len()];
    let mut shown = laid.shown.clone();
    let up = |i: usize| std::iter::successors(t.nodes[i].parent, |&a| t.nodes[a].parent);
    for (i, n) in t.nodes.iter().enumerate() {
        match (from.get(&n.id).copied(), laid.at[i]) {
            (Some(a), Some(b)) => at[i] = Some(mix(a, b, e)),
            (None, Some(b)) => {
                if let Some(a) = up(i).find_map(|a| from.get(&t.nodes[a].id)) {
                    at[i] = Some(mix(anchored(a, b), b, e));
                    fade[i] = e;
                }
            }
            (Some(a), None) => {
                if let Some(b) = up(i).find_map(|a| laid.at[a]) {
                    at[i] = Some(mix(a, anchored(&b, a), e));
                    fade[i] = 1.0 - e;
                    shown.push(i);
                }
            }
            (None, None) => {}
        }
    }
    shown.sort_unstable();
    Drawn { laid: Rc::new(Laid { at, shown, bounds: laid.bounds, metrics: laid.metrics }), fade }
}

/// Place `p` moved so its underline starts where node `a`'s branches leave it.
fn anchored(a: &Place, p: Place) -> Place {
    Place { x: a.right(), y: a.line_y() - (p.line_y() - p.y), ..p }
}

fn mix(a: Place, b: Place, e: f32) -> Place {
    let m = |x: f32, y: f32| x + (y - x) * e;
    Place { x: m(a.x, b.x), y: m(a.y, b.y), w: m(a.w, b.w), h: m(a.h, b.h), depth: b.depth }
}
