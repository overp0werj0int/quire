//! Board mode's pure model: a note's `map::Tree` projected into columns of cards
//! (`docs/reader-writer/behavior/board.md`, decision 0015).
//!
//! Pure, with no `App` or UI. Every move maps a board `Slot` to a `map::Add` and asks
//! `map::zone` whether the note's Markdown rules allow it.

use crate::blocks::Task;
use crate::map::{self, Add, NodeKind, ROOT, Refused, Tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub head: ColumnHead,
    /// Indices into `Tree::nodes`, in note order.
    pub cards: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColumnHead {
    /// Non-heading children of the root before the first column heading (BRD-005).
    Loose,
    /// A heading child of the root, by its index in `Tree::nodes` (BRD-002).
    Heading(usize),
}

/// A drop position on the board: `index` is the insertion index among the column's remaining
/// cards (excluding the held card if it comes from that column).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub column: usize,
    pub index: usize,
}

impl ColumnHead {
    /// The `Tree` node index that owns cards in this column (`ROOT` for Loose, `h` for Heading).
    pub fn parent(self) -> usize {
        match self {
            ColumnHead::Loose => ROOT,
            ColumnHead::Heading(h) => h,
        }
    }

    /// Stable key id for this column (`0` for Loose, else the heading's `NodeId`).
    pub fn key_id(self, t: &Tree) -> u64 {
        match self {
            ColumnHead::Loose => 0,
            ColumnHead::Heading(h) => t.nodes[h].id,
        }
    }
}

impl Board {
    /// Whether the board has any heading columns (false triggers the BRD-006 empty hint).
    pub fn has_headings(&self) -> bool {
        self.columns.iter().any(|c| matches!(c.head, ColumnHead::Heading(_)))
    }

    /// Finds which column and card index hold node `i`.
    pub fn find_card(&self, i: usize) -> Option<(usize, usize)> {
        for (ci, col) in self.columns.iter().enumerate() {
            if let Some(ki) = col.cards.iter().position(|&c| c == i) {
                return Some((ci, ki));
            }
        }
        None
    }

    /// Finds which column has heading node `h`.
    pub fn find_column(&self, h: usize) -> Option<usize> {
        self.columns.iter().position(|c| c.head == ColumnHead::Heading(h))
    }

    /// Cards in column `ci`, excluding `held` if present.
    pub fn rest(&self, ci: usize, held: Option<usize>) -> Vec<usize> {
        self.columns[ci].cards.iter().copied().filter(|&c| Some(c) != held).collect()
    }
}

/// Builds the board from `t` (BRD-001 to BRD-005):
/// - Root's non-heading children (excluding `Footnotes`) form the leading `Loose` column when
///   non-empty.
/// - Root's heading children become columns in note order, each holding all of its direct children
///   (non-headings first, then section-heading cards) in note order.
pub fn build(t: &Tree) -> Board {
    let mut loose = Vec::new();
    let mut columns = Vec::new();
    for &c in &t.root().children {
        match t.nodes[c].kind {
            NodeKind::Footnotes => {}
            NodeKind::Heading { .. } => {
                let cards = t.nodes[c]
                    .children
                    .iter()
                    .copied()
                    .filter(|&k| !matches!(t.nodes[k].kind, NodeKind::Footnotes))
                    .collect();
                columns.push(Column { head: ColumnHead::Heading(c), cards });
            }
            _ => loose.push(c),
        }
    }
    if !loose.is_empty() {
        columns.insert(0, Column { head: ColumnHead::Loose, cards: loose });
    }
    Board { columns }
}

/// Like [`build`], prepending an empty `Loose` column when card `held` is lifted and `map::zone`
/// allows dropping it into the loose area (BRD-005).
pub fn build_with_held(t: &Tree, held: Option<usize>) -> Board {
    let mut b = build(t);
    let Some(s) = held else { return b };
    let has_loose = b.columns.first().is_some_and(|c| c.head == ColumnHead::Loose);
    if !has_loose && !is_heading(t, s) && map::zone(t, s, Add::Child(ROOT)).is_ok() {
        b.columns.insert(0, Column { head: ColumnHead::Loose, cards: Vec::new() });
    }
    b
}

pub fn is_heading(t: &Tree, i: usize) -> bool {
    matches!(t.nodes[i].kind, NodeKind::Heading { .. })
}

/// Maps `slot` on `board` for held card `held` to a `map::Add` and checks `map::zone` (BRD-017).
pub fn slot_add(board: &Board, t: &Tree, held: usize, slot: Slot) -> Result<Add, Refused> {
    let col = board.columns.get(slot.column).ok_or(None)?;
    // A section heading card can never land in the loose column: a heading child of ROOT would
    // become a column itself rather than a loose card (BRD-004, BRD-005).
    if col.head == ColumnHead::Loose && is_heading(t, held) {
        return Err(Some(map::HEADINGS_ONLY));
    }
    let rest = board.rest(slot.column, Some(held));
    let idx = slot.index.min(rest.len());
    let add = if idx < rest.len() {
        // When placing a non-heading card directly before a section heading and after a
        // non-heading card, anchor after the preceding card so a list item joins its list
        // cleanly instead of landing after the blank line before the subheading.
        if idx > 0 && !is_heading(t, held) && is_heading(t, rest[idx]) && !is_heading(t, rest[idx - 1]) {
            Add::After(rest[idx - 1])
        } else {
            Add::Before(rest[idx])
        }
    } else if let Some(&last) = rest.last() {
        // Putting a section card at the end of a column that has no section cards yet uses
        // `Add::Child(heading)` rather than `Add::After(non_heading)` (MAP-027).
        if is_heading(t, held) && !is_heading(t, last) {
            Add::Child(col.head.parent())
        } else {
            Add::After(last)
        }
    } else {
        Add::Child(col.head.parent())
    };
    map::zone(t, held, add)?;
    Ok(add)
}

/// Nearest slot in `slot.column` that `slot_add` allows for `held`, or `None` if the column
/// allows no slot at all (BRD-017). Ties prefer the earlier slot (higher in the column).
pub fn nearest_allowed(board: &Board, t: &Tree, held: usize, slot: Slot) -> Option<Slot> {
    if slot.column >= board.columns.len() {
        return None;
    }
    let len = board.rest(slot.column, Some(held)).len();
    let want = slot.index.min(len);
    if slot_add(board, t, held, Slot { column: slot.column, index: want }).is_ok() {
        return Some(Slot { column: slot.column, index: want });
    }
    for d in 1..=len {
        if let Some(lo) = want.checked_sub(d)
            && slot_add(board, t, held, Slot { column: slot.column, index: lo }).is_ok()
        {
            return Some(Slot { column: slot.column, index: lo });
        }
        let hi = want + d;
        if hi <= len && slot_add(board, t, held, Slot { column: slot.column, index: hi }).is_ok() {
            return Some(Slot { column: slot.column, index: hi });
        }
    }
    None
}

/// Refusal reason when `nearest_allowed` returns `None` for `slot.column`.
pub fn column_refusal(board: &Board, t: &Tree, held: usize, slot: Slot) -> Option<&'static str> {
    slot_add(board, t, held, slot).err().flatten()
}

/// True when dropping `held` at `slot` leaves it in the exact place it came from (BRD-019).
pub fn is_same_slot(board: &Board, held: usize, slot: Slot) -> bool {
    let Some((ci, ki)) = board.find_card(held) else { return false };
    ci == slot.column && ki == slot.index
}

/// Maps moving column heading `held` to target index `target_col` among the remaining heading
/// columns (BRD-021).
pub fn column_add(board: &Board, t: &Tree, held: usize, target_col: usize) -> Result<Add, Refused> {
    let heads: Vec<usize> = board
        .columns
        .iter()
        .filter_map(|c| match c.head {
            ColumnHead::Heading(h) if h != held => Some(h),
            _ => None,
        })
        .collect();
    if heads.is_empty() {
        return Err(None);
    }
    let idx = target_col.min(heads.len());
    let add = if idx < heads.len() { Add::Before(heads[idx]) } else { Add::After(*heads.last().unwrap()) };
    map::zone(t, held, add)?;
    Ok(add)
}

/// True when dropping column heading `held` at `target_col` (among remaining heading columns)
/// leaves it in place.
pub fn is_same_column_slot(board: &Board, held: usize, target_col: usize) -> bool {
    let mut pos = 0;
    for c in &board.columns {
        if let ColumnHead::Heading(h) = c.head {
            if h == held {
                return pos == target_col;
            }
            pos += 1;
        }
    }
    false
}

/// Resolves any tree node `i` to the board item (column heading or card) that owns it (BRD-024).
/// Returns `None` for the root or footnotes when they have no board item.
pub fn item_of(t: &Tree, i: usize) -> Option<usize> {
    let mut cur = i;
    loop {
        if cur == ROOT || matches!(t.nodes[cur].kind, NodeKind::Footnotes | NodeKind::Footnote { .. }) {
            return None;
        }
        let p = t.nodes[cur].parent?;
        if p == ROOT {
            // Direct child of root: either a column heading or a loose card.
            return Some(cur);
        }
        if t.nodes[p].parent == Some(ROOT) && is_heading(t, p) {
            // Direct child of a column heading: a card (including a section card).
            return Some(cur);
        }
        cur = p;
    }
}

/// First selectable board item: first card of the first column that has one, else first column
/// heading (BRD-012).
pub fn default_selection(board: &Board) -> Option<usize> {
    for col in &board.columns {
        if let Some(&first) = col.cards.first() {
            return Some(first);
        }
        if let ColumnHead::Heading(h) = col.head {
            return Some(h);
        }
    }
    None
}

/// Caption under a list-item card that has children (BRD-011): `done/all` when any descendant is
/// a task, else `N items` (`1 item`).
pub fn child_summary(t: &Tree, i: usize) -> Option<String> {
    let n = &t.nodes[i];
    if !matches!(n.kind, NodeKind::Item { .. }) || n.children.is_empty() {
        return None;
    }
    let mut tasks = 0usize;
    let mut done = 0usize;
    for k in (i + 1)..n.end {
        if let NodeKind::Item { task, .. } = t.nodes[k].kind
            && task != Task::None
        {
            tasks += 1;
            if task == Task::Done {
                done += 1;
            }
        }
    }
    if tasks > 0 {
        Some(format!("{done}/{tasks}"))
    } else {
        let count = n.end - i - 1;
        Some(if count == 1 { "1 item".into() } else { format!("{count} items") })
    }
}

/// Caption under a section card (a heading under a column, BRD-004): how many nodes it holds.
pub fn section_summary(t: &Tree, i: usize) -> String {
    let count = t.nodes[i].end - i - 1;
    if count == 1 { "1 node".into() } else { format!("{count} nodes") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(src: &str) -> Tree {
        let doc = crate::blocks::build(src);
        map::build(&doc, src, "Note.md")
    }

    #[test]
    fn builds_loose_columns_section_cards_and_skips_footnotes() {
        let t = tree("# Top\n\nLoose para\n\n## Todo\n\n- [ ] One\n- Two\n  - [x] Sub\n\n### Sub\n\n- Inner\n\n## Done\n\n[^1]: Foot\n");
        let b = build(&t);
        assert_eq!(b.columns.len(), 3);
        assert_eq!(b.columns[0].head, ColumnHead::Loose);
        assert_eq!(b.columns[0].cards.len(), 1);
        assert!(matches!(b.columns[1].head, ColumnHead::Heading(_)));
        assert_eq!(b.columns[1].cards.len(), 3); // One, Two, Sub (section card)
        assert_eq!(child_summary(&t, b.columns[1].cards[1]).as_deref(), Some("1/1"));
        assert_eq!(section_summary(&t, b.columns[1].cards[2]), "1 node");
        assert!(matches!(b.columns[2].head, ColumnHead::Heading(_)));
        assert_eq!(b.columns[2].cards.len(), 0); // footnote excluded
    }

    #[test]
    fn nearest_allowed_clamps_text_above_section_and_refuses_section_in_loose() {
        let t = tree("# Top\n\nLoose\n\n## Todo\n\n- Task\n\n### Sub\n\n- Inner\n");
        let b = build(&t);
        let task = b.columns[1].cards[0];
        let section = b.columns[1].cards[1];
        // Dropping text card below section card clamps above section card (index 0 in rest).
        let clamped = nearest_allowed(&b, &t, task, Slot { column: 1, index: 1 });
        assert_eq!(clamped, Some(Slot { column: 1, index: 0 }));
        // Section card in loose column is refused with HEADINGS_ONLY.
        assert_eq!(nearest_allowed(&b, &t, section, Slot { column: 0, index: 0 }), None);
        assert_eq!(column_refusal(&b, &t, section, Slot { column: 0, index: 0 }), Some(map::HEADINGS_ONLY));
    }

    #[test]
    fn build_with_held_reveals_empty_loose_for_non_heading_only() {
        let t = tree("# Top\n\n## Todo\n\n- Task\n\n### Sub\n\n- Inner\n");
        let base = build(&t);
        assert_eq!(base.columns.len(), 1);
        let task = base.columns[0].cards[0];
        let section = base.columns[0].cards[1];
        let with_task = build_with_held(&t, Some(task));
        assert_eq!(with_task.columns[0].head, ColumnHead::Loose);
        let with_section = build_with_held(&t, Some(section));
        assert!(matches!(with_section.columns[0].head, ColumnHead::Heading(_)));
    }
}
