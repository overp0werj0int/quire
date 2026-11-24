//! Moving a map node by dragging it (MAP-026): the drag's state, the drop zone under the
//! pointer, and the pan at the canvas's edges. The move itself is `map::move_to`, the same one
//! the keys run.

use std::time::Duration;

use rune::{Drag, Phase, Tx};

use crate::app::App;
use crate::map::{self, Add, NodeId, NodeKind};
use crate::map_layout::Part;

/// How far the pointer travels before a press on a node becomes a drag, in points.
const THRESHOLD: f32 = 6.0;
/// The band along the canvas's edges that pans while a drag holds the pointer there.
const EDGE: f32 = 36.0;
/// The fastest that band pans, in points a second.
const EDGE_SPEED: f32 = 480.0;
/// The least width of a Before or After band, in screen points.
const BAND: f32 = 8.0;

/// A press on a node, which turns into a move once the pointer travels.
pub struct NodeDrag {
    pub node: NodeId,
    /// The pointer at the press and now, in the canvas's points.
    start: [f32; 2],
    pub at: [f32; 2],
    /// Past the threshold: a move, not a click.
    pub active: bool,
    /// Where a drop now would put the node.
    pub drop: Option<Add>,
    /// When the edge pan last stepped.
    tick: f64,
}

impl App {
    /// The canvas point under a drag's pointer.
    fn map_point(&self, d: &Drag) -> [f32; 2] {
        let [x, y] = d.window_point();
        let [ox, oy] = self.map.origin.get();
        [x - ox, y - oy]
    }

    /// The world point under canvas point `c`.
    fn map_world(&self, c: [f32; 2]) -> [f32; 2] {
        let ([px, py], z) = self.map_camera();
        [(c[0] - px) / z, (c[1] - py) / z]
    }

    /// A drag that pans the map: on the empty canvas, with the middle button, or on a node
    /// that can't move (MAP-033).
    pub fn map_pan_drag(&mut self, d: &Drag, tx: &mut Tx) {
        let (pan, z) = self.map_camera();
        match d.phase {
            Phase::Down => {
                self.map.drag_from = Some(pan);
                tx.live_only();
            }
            Phase::Move => {
                if let Some([x, y]) = self.map.drag_from {
                    self.map_move([x + d.dx, y + d.dy], z, tx);
                }
            }
            Phase::Up => {
                if let Some(from) = self.map.drag_from.take()
                    && d.cancelled()
                {
                    self.map_move(from, z, tx);
                }
            }
        }
    }

    /// A drag that starts on node `i` (MAP-026): past 6 points it moves the node, and the drop
    /// zone under the pointer says where to. Esc, or a drop on no zone, puts nothing anywhere.
    pub fn map_node_drag(&mut self, i: usize, d: &Drag, tx: &mut Tx) {
        let here = self.map_point(d);
        match d.phase {
            Phase::Down => {
                self.map.dropped = false;
                self.map.node_drag = None;
                let t = self.map_tree();
                let fixed = i == map::ROOT || matches!(t.nodes[i].kind, NodeKind::Footnotes | NodeKind::Footnote { .. });
                if fixed || !self.map_editable() {
                    return self.map_pan_drag(d, tx);
                }
                self.map.node_drag = Some(NodeDrag { node: t.nodes[i].id, start: here, at: here, active: false, drop: None, tick: 0.0 });
                tx.live_only();
            }
            Phase::Move => {
                let Some(g) = &mut self.map.node_drag else { return self.map_pan_drag(d, tx) };
                g.at = here;
                if !g.active {
                    if (here[0] - g.start[0]).hypot(here[1] - g.start[1]) <= THRESHOLD {
                        tx.live_only();
                        return;
                    }
                    // A field still open on any node goes in first; the node is found again in
                    // the tree that leaves.
                    let Some(i) = self.map_press_node(i, tx) else {
                        self.map.node_drag = None;
                        return;
                    };
                    let id = self.map_tree().nodes[i].id;
                    if let Some(g) = &mut self.map.node_drag {
                        g.node = id;
                        g.active = true;
                    }
                }
                self.map_aim();
                if self.map_edge_pan() != [0.0, 0.0] {
                    if let Some(g) = &mut self.map.node_drag {
                        g.tick = self.now.get();
                    }
                    tx.wake_after(Duration::from_millis(16));
                }
            }
            Phase::Up => {
                let Some(g) = self.map.node_drag.take() else { return self.map_pan_drag(d, tx) };
                if !g.active {
                    return;
                }
                self.map.dropped = true;
                if d.cancelled() {
                    return;
                }
                let t = self.map_tree();
                let Some(s) = t.find(g.node) else { return };
                if let Some(at) = self.map_drop_at(s, self.map_world(here)) {
                    self.map_move_node(s, at, tx);
                }
            }
        }
    }

    /// The drop zone under the drag's pointer, again.
    fn map_aim(&mut self) {
        let Some(g) = &self.map.node_drag else { return };
        let t = self.map_tree();
        let drop = t.find(g.node).and_then(|s| self.map_drop_at(s, self.map_world(g.at)));
        if let Some(g) = &mut self.map.node_drag {
            g.drop = drop;
        }
    }

    /// Where a drop at world point `w` puts node `s` (MAP-026): the zone of the node under it,
    /// among those the move rules allow. A container's box splits into Before, Child and
    /// After (a quarter, a half, a quarter), a leaf's into Before and After; Before and After
    /// reach half way to the siblings either side and are never under 8 screen points. A zone
    /// the rules refuse gives its room to the ones they allow.
    pub fn map_drop_at(&self, s: usize, w: [f32; 2]) -> Option<Add> {
        let t = self.map_tree();
        let l = self.map_laid();
        let band = BAND / self.map_camera().1;
        let moved = s..t.nodes[s].end;
        for &j in l.shown.iter().filter(|j| !moved.contains(j)) {
            let Some(part) = l.part_at(&t, j, w, band) else { continue };
            let ok = |a: Add| map::zone(&t, s, a).is_ok();
            let (before, child, after) = (ok(Add::Before(j)), ok(Add::Child(j)), ok(Add::After(j)));
            let at = match part {
                Part::Lead if child && before => Add::Before(j),
                Part::Trail if child && after => Add::After(j),
                _ if child => Add::Child(j),
                _ if before && (!part.late() || !after) => Add::Before(j),
                _ if after => Add::After(j),
                _ => return None,
            };
            return Some(at);
        }
        None
    }

    /// The edge pan's velocity for the drag's pointer, in points a second.
    fn map_edge_pan(&self) -> [f32; 2] {
        let Some(g) = self.map.node_drag.as_ref().filter(|g| g.active) else { return [0.0, 0.0] };
        let [w, h] = self.map_size();
        let speed = |d: f32| EDGE_SPEED * ((EDGE - d) / EDGE).clamp(0.0, 1.0);
        [speed(g.at[0]) - speed(w - g.at[0]), speed(g.at[1]) - speed(h - g.at[1])]
    }

    /// Before each frame: while a drag holds the pointer near the canvas's edge, the map pans
    /// that way and the drop zone follows (MAP-026).
    pub(crate) fn poll_drag(&mut self, now: f64, tx: &mut Tx) -> bool {
        let [vx, vy] = self.map_edge_pan();
        let Some(g) = &mut self.map.node_drag else { return false };
        if vx == 0.0 && vy == 0.0 {
            return false;
        }
        let dt = (now - g.tick).clamp(0.0, 0.05) as f32;
        g.tick = now;
        let ([px, py], z) = self.map_camera();
        self.map.glide = None;
        self.set_camera([px + vx * dt, py + vy * dt], z);
        self.map_aim();
        tx.wake_after(Duration::from_millis(16));
        true
    }
}
