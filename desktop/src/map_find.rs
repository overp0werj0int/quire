//! Find on the map (MAP-066): the find bar's query matched in every node's label and card,
//! folded branches included, and the current match brought into view.

use rune::Tx;

use crate::app::App;
use crate::blocks::plain;
use crate::map::{Body, Node, NodeKind, Tree};
use crate::writer_app::find_in;

/// A match on the map: its node, the card piece it is in (None for the node's label), and which
/// match in that text it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub node: usize,
    pub pid: Option<usize>,
    pub k: usize,
}

/// The text node `n`'s label draws, which find searches, or None for a card, which is searched
/// by its pieces.
pub fn label_text(n: &Node) -> Option<String> {
    match (&n.body, &n.kind) {
        (Body::Card(_), _) => None,
        (Body::Runs(r), NodeKind::Item { marker, ordered: true, .. }) => Some(format!("{marker} {}", plain(r))),
        (Body::Runs(r), _) => Some(plain(r)),
        (Body::Plain, _) => Some(crate::map_app::shown_label(n)),
    }
}

/// Every match of `q` on map `t`, node by node in the note's order (RD-55).
pub fn hits(t: &Tree, q: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    let mut pieces = Vec::new();
    for (i, n) in t.nodes.iter().enumerate() {
        if let Body::Card(bl) = &n.body {
            pieces.clear();
            crate::blocks::pieces_in(bl, &mut pieces);
            for (pid, text) in &pieces {
                out.extend((0..find_in(text, q).len()).map(|k| Hit { node: i, pid: Some(*pid), k }));
            }
        } else if let Some(text) = label_text(n) {
            out.extend((0..find_in(&text, q).len()).map(|k| Hit { node: i, pid: None, k }));
        }
    }
    out
}

impl App {
    /// The find query's matches on the map shown.
    pub fn map_hits(&self) -> Vec<Hit> {
        self.find_query().map_or_else(Vec::new, |q| hits(&self.map_tree(), q))
    }

    /// Where a search starts: the first match at or after the selected node, else the first.
    pub(crate) fn map_first_hit(&self, hits: &[Hit]) -> usize {
        let sel = self.map_selected();
        hits.iter().position(|h| h.node >= sel).unwrap_or(0)
    }

    /// The current match's node selected, the folds above it open, and the camera gliding to
    /// put it in the middle of the canvas clear of the find bar.
    pub(crate) fn map_show_hit(&mut self, tx: &mut Tx) {
        if let Some(h) = self.map_hits().get(self.reader.find.cur) {
            self.map_go_node(h.node, tx);
        }
    }
}
