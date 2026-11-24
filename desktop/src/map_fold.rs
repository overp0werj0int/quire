//! A tab's folds (MAP-041): which nodes show nothing below them. Both's root folds a side at
//! a time (MAP-088), so it can also have one side folded and the other open.

use std::collections::HashSet;

use crate::map::{NodeId, ROOT, Tree};
use crate::map_geom::Dir;
use crate::map_layout::Sides;

#[derive(Clone, Debug, Default)]
pub struct Folds {
    /// Nodes with everything below them folded. Both's root is here when both sides are.
    whole: HashSet<NodeId>,
    /// The one side of Both's root folded alone. It says nothing about a root with one side.
    side: Option<Dir>,
}

impl FromIterator<NodeId> for Folds {
    fn from_iter<I: IntoIterator<Item = NodeId>>(ids: I) -> Folds {
        Folds { whole: ids.into_iter().collect(), side: None }
    }
}

impl Folds {
    /// Nothing below node `id` shows.
    pub fn whole(&self, id: NodeId) -> bool {
        self.whole.contains(&id)
    }

    /// The side of a two-sided root that is folded alone.
    pub fn side(&self, sides: &Sides) -> Option<Dir> {
        self.side.filter(|_| sides.second.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.whole.is_empty() && self.side.is_none()
    }

    pub fn clear(&mut self) {
        *self = Folds::default();
    }

    /// Folds node `i` or opens it: the branches growing `d` where it has another side, else all
    /// of it. Whether anything changed.
    pub fn set(&mut self, t: &Tree, sides: &Sides, i: usize, d: Option<Dir>, fold: bool) -> bool {
        let id = t.nodes[i].id;
        let pair = d.filter(|_| i == ROOT).and_then(|d| Some((d, sides.other(d)?)));
        let Some((d, other)) = pair else {
            let half = i == ROOT && self.side.take().is_some();
            return (if fold { self.whole.insert(id) } else { self.whole.remove(&id) }) || half;
        };
        let was = (self.whole(id), self.side);
        if !fold {
            let half = Some(other).filter(|_| self.whole.remove(&id));
            self.side = half.or(self.side.filter(|&s| s != d));
        } else if self.side == Some(other) {
            self.side = None;
            self.whole.insert(id);
        } else if !self.whole(id) {
            self.side = Some(d);
        }
        was != (self.whole(id), self.side)
    }

    /// Node `i`'s parent is folded, or the side of the root it is on.
    pub fn hides(&self, t: &Tree, sides: &Sides, i: usize) -> bool {
        let Some(p) = t.nodes[i].parent else { return false };
        self.whole(t.nodes[p].id) || (p == ROOT && self.side(sides).is_some_and(|d| sides.way(i) == Some(d)))
    }

    /// Opens what hides node `i`. Whether anything did.
    pub fn open_to(&mut self, t: &Tree, sides: &Sides, i: usize) -> bool {
        let mut opened = false;
        let mut a = i;
        while let Some(p) = t.nodes[a].parent {
            opened |= self.set(t, sides, p, sides.way(a), false);
            a = p;
        }
        opened
    }

    /// The folds after a change to the note: `keep` follows a folded node, None when it has
    /// gone or has nothing left to fold. A root that lost a side opens the other, and the side
    /// that now holds node `sel` opens.
    pub fn follow(&mut self, sides: &Sides, sel: usize, keep: impl FnMut(NodeId) -> Option<NodeId>) {
        self.whole = std::mem::take(&mut self.whole).into_iter().filter_map(keep).collect();
        self.side = self.side(sides).filter(|&d| sides.way(sel) != Some(d));
    }
}
