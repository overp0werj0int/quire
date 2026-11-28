//! Board mode's pure model: a `kanban-plugin` note's `map::Tree` projected into heading columns
//! of task cards with recursive subtask trees (`docs/reader-writer/behavior/board.md`, decision 0015).
//!
//! Pure, with no `App` or UI. Every move maps a board `Slot` to a `map::Add` and asks
//! `map::zone` whether the note's Markdown rules allow it.

use std::collections::HashSet;

use crate::blocks::{Doc, Kind, Task};
use crate::map::{self, Add, NodeId, NodeKind, ROOT, Refused, Tree};

/// True when `doc` declares `kanban-plugin` in its front matter (and does not explicitly disable
/// it with `false`, `no`, or `off`).
pub fn is_kanban(doc: &Doc) -> bool {
    doc.blocks.iter().any(|bl| match &bl.kind {
        Kind::FrontMatter { fields, .. } => fields.iter().any(|f| {
            f.key.eq_ignore_ascii_case("kanban-plugin")
                && !matches!(f.value.trim().to_ascii_lowercase().as_str(), "false" | "no" | "off")
        }),
        _ => false,
    })
}

/// True when tree node `i` is a task list item (`- [ ]` or `- [x]`).
pub fn is_task(t: &Tree, i: usize) -> bool {
    matches!(t.nodes[i].kind, NodeKind::Item { task, .. } if task != Task::None)
}

/// Task state of node `i`, or `None` when it is not a task item.
pub fn task_state(t: &Tree, i: usize) -> Option<Task> {
    match t.nodes[i].kind {
        NodeKind::Item { task, .. } if task != Task::None => Some(task),
        _ => None,
    }
}

pub fn is_heading(t: &Tree, i: usize) -> bool {
    matches!(t.nodes[i].kind, NodeKind::Heading { .. })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Heading node index in `Tree::nodes`.
    pub heading: usize,
    /// Top-level task cards directly under this column heading, in note order.
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// Task item node index in `Tree::nodes`.
    pub node: usize,
    /// Direct child task items, recursively modeled with the exact same rules.
    pub subtasks: Vec<Card>,
    /// True when this task node has any non-task child blocks (prose, code, images, bullets, etc.).
    pub has_body: bool,
}

/// Where a card drop slot lives: in a column's top-level card list or in an expanded card's
/// subtask list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotParent {
    /// Column index in `Board::columns`.
    Column(usize),
    /// Parent task card's node index in `Tree::nodes`.
    Card(usize),
}

/// A drop position on the board: `index` is the insertion index among remaining direct task cards
/// under `parent` (excluding the held card if it comes from there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub parent: SlotParent,
    pub index: usize,
}

impl Card {
    fn build(t: &Tree, node: usize) -> Card {
        let mut subtasks = Vec::new();
        let mut has_body = false;
        for &c in &t.nodes[node].children {
            if is_task(t, c) {
                subtasks.push(Card::build(t, c));
            } else if !matches!(t.nodes[c].kind, NodeKind::Footnotes) {
                has_body = true;
            }
        }
        Card { node, subtasks, has_body }
    }

    /// Completion state (`true` = done, `false` = todo) of each direct subtask, in note order.
    pub fn subtask_dots(&self, t: &Tree) -> Vec<bool> {
        self.subtasks.iter().map(|c| task_state(t, c.node) == Some(Task::Done)).collect()
    }

    /// Finds `node` anywhere inside this card subtree.
    pub fn find(&self, node: usize) -> Option<&Card> {
        if self.node == node {
            return Some(self);
        }
        for sub in &self.subtasks {
            if let Some(found) = sub.find(node) {
                return Some(found);
            }
        }
        None
    }

    fn find_parent(&self, target: usize) -> Option<(SlotParent, usize)> {
        for (idx, sub) in self.subtasks.iter().enumerate() {
            if sub.node == target {
                return Some((SlotParent::Card(self.node), idx));
            }
            if let Some(found) = sub.find_parent(target) {
                return Some(found);
            }
        }
        None
    }

    fn collect_visible(&self, t: &Tree, expanded: &HashSet<NodeId>, held: Option<usize>, depth: usize, out: &mut Vec<(usize, usize)>) {
        if Some(self.node) == held {
            return;
        }
        out.push((self.node, depth));
        if !self.subtasks.is_empty() && expanded.contains(&t.nodes[self.node].id) {
            for sub in &self.subtasks {
                sub.collect_visible(t, expanded, held, depth + 1, out);
            }
        }
    }

    fn collect_all(&self, out: &mut Vec<usize>) {
        out.push(self.node);
        for sub in &self.subtasks {
            sub.collect_all(out);
        }
    }
}

impl Column {
    /// Stable key id for this column (its heading's `NodeId`).
    pub fn key_id(&self, t: &Tree) -> u64 {
        t.nodes[self.heading].id
    }

    /// Visible cards `(node_idx, depth)` in visual preorder, respecting `expanded` and omitting
    /// `held` (and its subtree).
    pub fn visible_cards(&self, t: &Tree, expanded: &HashSet<NodeId>, held: Option<usize>) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for card in &self.cards {
            card.collect_visible(t, expanded, held, 0, &mut out);
        }
        out
    }

    /// Every task card in this column (including folded subtasks), in note order.
    pub fn all_cards(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for card in &self.cards {
            card.collect_all(&mut out);
        }
        out
    }
}

impl Board {
    /// Whether the board has any heading columns.
    pub fn has_headings(&self) -> bool {
        !self.columns.is_empty()
    }

    /// Finds the `Card` for tree node `i` anywhere on the board (top-level or subtask).
    pub fn card(&self, i: usize) -> Option<&Card> {
        self.columns.iter().find_map(|col| col.cards.iter().find_map(|c| c.find(i)))
    }

    /// Finds the column index that contains card `i` (top-level or subtask).
    pub fn find_card_column(&self, i: usize) -> Option<usize> {
        self.columns.iter().position(|col| col.cards.iter().any(|c| c.find(i).is_some()))
    }

    /// Finds the direct `(SlotParent, index)` owning card `i`.
    pub fn find_card_slot(&self, i: usize) -> Option<(SlotParent, usize)> {
        for (ci, col) in self.columns.iter().enumerate() {
            for (ki, card) in col.cards.iter().enumerate() {
                if card.node == i {
                    return Some((SlotParent::Column(ci), ki));
                }
                if let Some(found) = card.find_parent(i) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Finds which column has heading node `h`.
    pub fn find_column(&self, h: usize) -> Option<usize> {
        self.columns.iter().position(|c| c.heading == h)
    }

    /// Direct task cards under `parent`, excluding `held` if present.
    pub fn rest(&self, parent: SlotParent, held: Option<usize>) -> Vec<usize> {
        let slice = match parent {
            SlotParent::Column(ci) => self.columns.get(ci).map(|c| c.cards.as_slice()),
            SlotParent::Card(node) => self.card(node).map(|c| c.subtasks.as_slice()),
        };
        slice
            .unwrap_or(&[])
            .iter()
            .map(|c| c.node)
            .filter(|&n| Some(n) != held)
            .collect()
    }

    /// Tree node index of `parent`'s owning node (the column heading or parent task card).
    pub fn parent_node(&self, parent: SlotParent) -> Option<usize> {
        match parent {
            SlotParent::Column(ci) => self.columns.get(ci).map(|c| c.heading),
            SlotParent::Card(node) => self.card(node).map(|c| c.node),
        }
    }
}

/// Builds the task-only kanban board from `t`:
/// - Root's heading children become columns in note order.
/// - Each column's direct task-item children become cards, with direct child tasks nested
///   recursively as `Card::subtasks`.
/// - Non-heading root blocks, non-task children, subheadings, and `Footnotes` are excluded.
pub fn build(t: &Tree) -> Board {
    let mut columns = Vec::new();
    for &c in &t.root().children {
        if matches!(t.nodes[c].kind, NodeKind::Heading { .. }) {
            let cards = t.nodes[c]
                .children
                .iter()
                .copied()
                .filter(|&k| is_task(t, k))
                .map(|k| Card::build(t, k))
                .collect();
            columns.push(Column { heading: c, cards });
        }
    }
    Board { columns }
}

/// Maps `slot` on `board` for held task card `held` to a `map::Add` and checks `map::zone`.
pub fn slot_add(board: &Board, t: &Tree, held: usize, slot: Slot) -> Result<Add, Refused> {
    let parent_node = board.parent_node(slot.parent).ok_or(None)?;
    let rest = board.rest(slot.parent, Some(held));
    let idx = slot.index.min(rest.len());
    let add = if idx < rest.len() {
        Add::Before(rest[idx])
    } else if let Some(&last) = rest.last() {
        Add::After(last)
    } else {
        // Empty task list in this column/card: append as child, or before the first subheading
        // if a heading column has subheadings under it.
        let child_add = Add::Child(parent_node);
        if map::zone(t, held, child_add).is_ok() {
            return Ok(child_add);
        }
        if let SlotParent::Column(_) = slot.parent
            && let Some(&first_head) = t.nodes[parent_node].children.iter().find(|&&k| is_heading(t, k))
        {
            let before_head = Add::Before(first_head);
            map::zone(t, held, before_head)?;
            return Ok(before_head);
        }
        map::zone(t, held, child_add)?;
        return Ok(child_add);
    };
    map::zone(t, held, add)?;
    Ok(add)
}

/// Nearest slot under `slot.parent` that `slot_add` allows for `held`, or `None` if disallowed.
pub fn nearest_allowed(board: &Board, t: &Tree, held: usize, slot: Slot) -> Option<Slot> {
    board.parent_node(slot.parent)?;
    let len = board.rest(slot.parent, Some(held)).len();
    let want = slot.index.min(len);
    if slot_add(board, t, held, Slot { parent: slot.parent, index: want }).is_ok() {
        return Some(Slot { parent: slot.parent, index: want });
    }
    for d in 1..=len {
        if let Some(lo) = want.checked_sub(d)
            && slot_add(board, t, held, Slot { parent: slot.parent, index: lo }).is_ok()
        {
            return Some(Slot { parent: slot.parent, index: lo });
        }
        let hi = want + d;
        if hi <= len && slot_add(board, t, held, Slot { parent: slot.parent, index: hi }).is_ok() {
            return Some(Slot { parent: slot.parent, index: hi });
        }
    }
    None
}

/// Refusal reason when `nearest_allowed` returns `None` for `slot.parent`.
pub fn slot_refusal(board: &Board, t: &Tree, held: usize, slot: Slot) -> Option<&'static str> {
    slot_add(board, t, held, slot).err().flatten()
}

/// True when dropping `held` at `slot` leaves it in the exact place it came from.
pub fn is_same_slot(board: &Board, held: usize, slot: Slot) -> bool {
    let Some((parent, idx)) = board.find_card_slot(held) else { return false };
    parent == slot.parent && idx == slot.index
}

/// Maps moving column heading `held` to target index `target_col` among the remaining heading
/// columns.
pub fn column_add(board: &Board, t: &Tree, held: usize, target_col: usize) -> Result<Add, Refused> {
    let heads: Vec<usize> = board.columns.iter().map(|c| c.heading).filter(|&h| h != held).collect();
    if heads.is_empty() {
        return Err(None);
    }
    let idx = target_col.min(heads.len());
    let add = if idx < heads.len() { Add::Before(heads[idx]) } else { Add::After(*heads.last().unwrap()) };
    map::zone(t, held, add)?;
    Ok(add)
}

/// True when dropping column heading `held` at `target_col` leaves it in place.
pub fn is_same_column_slot(board: &Board, held: usize, target_col: usize) -> bool {
    board.find_column(held) == Some(target_col)
}

/// True when every card ancestor of task card `i` is expanded in `expanded` (so `i` is visible
/// on the board right now).
pub fn is_card_visible(t: &Tree, board: &Board, expanded: &HashSet<NodeId>, i: usize) -> bool {
    if board.card(i).is_none() {
        return false;
    }
    let mut cur = i;
    while let Some(p) = t.nodes[cur].parent {
        if board.find_column(p).is_some() {
            return true;
        }
        if !expanded.contains(&t.nodes[p].id) {
            return false;
        }
        cur = p;
    }
    false
}

/// Resolves any tree node `i` to the deepest visible board item (card or column heading) that owns
/// it, respecting `expanded` for subtasks.
pub fn item_of(t: &Tree, board: &Board, expanded: &HashSet<NodeId>, i: usize) -> Option<usize> {
    let mut cur = i;
    loop {
        if cur == ROOT || matches!(t.nodes[cur].kind, NodeKind::Footnotes | NodeKind::Footnote { .. }) {
            return None;
        }
        if board.find_column(cur).is_some() {
            return Some(cur);
        }
        if is_card_visible(t, board, expanded, cur) {
            return Some(cur);
        }
        cur = t.nodes[cur].parent?;
    }
}

/// Ancestor card `NodeId`s that must be expanded so task card `i` becomes visible (outermost first).
pub fn ancestors_to_expand(t: &Tree, board: &Board, i: usize) -> Vec<NodeId> {
    if board.card(i).is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cur = i;
    while let Some(p) = t.nodes[cur].parent {
        if board.find_column(p).is_some() {
            break;
        }
        if board.card(p).is_some() {
            out.push(t.nodes[p].id);
        }
        cur = p;
    }
    out.reverse();
    out
}

/// First selectable board item: first top-level card of the first column that has one, else first
/// column heading.
pub fn default_selection(board: &Board) -> Option<usize> {
    for col in &board.columns {
        if let Some(first) = col.cards.first() {
            return Some(first.node);
        }
        return Some(col.heading);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_and_tree(src: &str) -> (Doc, Tree) {
        let doc = crate::blocks::build(src);
        let t = map::build(&doc, src, "Board.md");
        (doc, t)
    }

    #[test]
    fn kanban_metadata_gate() {
        let (d1, _) = doc_and_tree("---\nkanban-plugin: board\n---\n\n## Todo\n\n- [ ] One\n");
        assert!(is_kanban(&d1));
        let (d2, _) = doc_and_tree("---\nkanban-plugin: basic\n---\n\n## Todo\n");
        assert!(is_kanban(&d2));
        let (d3, _) = doc_and_tree("---\ntitle: Regular\n---\n\n## Todo\n\n- [ ] One\n");
        assert!(!is_kanban(&d3));
        let (d4, _) = doc_and_tree("## Todo\n\n- [ ] One\n");
        assert!(!is_kanban(&d4));
        let (d5, _) = doc_and_tree("---\nkanban-plugin: false\n---\n\n## Todo\n");
        assert!(!is_kanban(&d5));
    }

    #[test]
    fn builds_task_cards_only_with_recursive_subtasks_and_dots() {
        let src = "---\nkanban-plugin: board\n---\n\nLoose para\n\n## Todo\n\nIntro text\n\n- Plain bullet\n- [ ] Parent\n\n  Notes under parent\n\n  - [x] Done sub\n  - [ ] Todo sub\n    - [x] Deep done\n- [x] Second\n\n### Subheading\n\n- [ ] Hidden under subheading\n\n## Done\n\n[^1]: Footnote\n";
        let (_, t) = doc_and_tree(src);
        let b = build(&t);
        assert_eq!(b.columns.len(), 2);
        assert_eq!(b.columns[0].cards.len(), 2);
        assert_eq!(b.columns[1].cards.len(), 0);

        let parent = &b.columns[0].cards[0];
        assert!(parent.has_body, "non-task paragraph under parent sets has_body");
        assert_eq!(parent.subtasks.len(), 2);
        assert_eq!(parent.subtask_dots(&t), vec![true, false]);

        let nested = &parent.subtasks[1];
        assert_eq!(nested.subtasks.len(), 1);
        assert_eq!(nested.subtask_dots(&t), vec![true]);
        assert!(!nested.has_body);

        // Visible cards respect expanded set.
        let empty = HashSet::new();
        assert_eq!(b.columns[0].visible_cards(&t, &empty, None).len(), 2);
        let mut exp = HashSet::new();
        exp.insert(t.nodes[parent.node].id);
        let vis = b.columns[0].visible_cards(&t, &exp, None);
        assert_eq!(vis.len(), 4);
        assert_eq!(vis[0], (parent.node, 0));
        assert_eq!(vis[1], (parent.subtasks[0].node, 1));
        assert_eq!(vis[2], (parent.subtasks[1].node, 1));
        assert_eq!(vis[3], (b.columns[0].cards[1].node, 0));
    }

    #[test]
    fn subcard_slot_and_column_with_subheading_allowed() {
        let src = "---\nkanban-plugin: board\n---\n\n## Todo\n\n- [ ] Parent\n  - [ ] Sub A\n  - [x] Sub B\n- [ ] Other\n\n## Done\n\n### Archive\n\n- [x] Old\n";
        let (_, t) = doc_and_tree(src);
        let b = build(&t);
        let parent = b.columns[0].cards[0].node;
        let sub_a = b.columns[0].cards[0].subtasks[0].node;
        let other = b.columns[0].cards[1].node;

        // Reorder Sub A after Sub B inside Parent.
        let add = slot_add(&b, &t, sub_a, Slot { parent: SlotParent::Card(parent), index: 1 });
        assert!(add.is_ok(), "reorder subtask inside parent: {add:?}");

        // Cannot drop Parent into its own subtask list (cycle).
        let self_nest = slot_add(&b, &t, parent, Slot { parent: SlotParent::Card(parent), index: 0 });
        assert!(self_nest.is_err(), "parent cannot drop into itself");

        // Drop Other into Done (which has no direct tasks, only a subheading).
        let into_done = slot_add(&b, &t, other, Slot { parent: SlotParent::Column(1), index: 0 });
        assert!(into_done.is_ok(), "drop task into heading column with subheading: {into_done:?}");
    }
}
