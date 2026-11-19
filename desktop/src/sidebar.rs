//! The sidebar's two lists as state: which one is folded and where the handle between them
//! sits, which tab is active after a close, and a tab dragged to a new place
//! (`docs/reader-writer/behavior/sidebar.md`, SB-20, SB-21, SB-33 to SB-47). Pure, with no
//! drawing: `ui::sidebar` draws from it.

use serde::{Deserialize, Serialize};

/// One of the sidebar's two lists: the Open shelf above, the vault below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Shelf,
    Vault,
}

/// What `state.json` keeps of the sidebar, once for the app (SB-46, SB-47).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// The folded section. One field, so both cannot be folded at once (SB-40).
    pub folded: Option<Section>,
    /// The shelf's height after a handle drag, in points. None: the shelf fits its tabs (SB-35).
    pub shelf_h: Option<f32>,
}

impl Layout {
    /// Folds `s`, or unfolds it when it is folded. Folding one unfolds the other (SB-39, SB-40).
    pub fn toggle(&mut self, s: Section) {
        self.folded = if self.folded == Some(s) { None } else { Some(s) };
    }

    /// Unfolds `s` if it is folded (SB-111).
    pub fn unfold(&mut self, s: Section) {
        if self.folded == Some(s) {
            self.folded = None;
        }
    }
}

/// The shelf's height in `room`, the height the shelf and the vault share. A stored height
/// wins; without one the shelf fits its tabs (`fit`) up to half the room (SB-35). Then the
/// shelf keeps `min_shelf` and the vault `min_vault` (SB-36). A room too short for both gives
/// the shelf what the vault leaves, down to nothing, so the vault's label stays (SB-132). The
/// stored height is never written back: a taller window puts the handle back.
pub fn split(stored: Option<f32>, room: f32, fit: f32, min_shelf: f32, min_vault: f32) -> f32 {
    stored.unwrap_or(fit.min(room / 2.0)).max(min_shelf).min((room - min_vault).max(0.0))
}

/// Which tab is active after the active one, at `i`, closed with `len` tabs left: the one
/// above it, or the one below when it was first (SB-20). None when no tab is left.
pub fn next_active(i: usize, len: usize) -> Option<usize> {
    (len > 0).then(|| i.saturating_sub(1).min(len - 1))
}

/// Points the pointer travels before a press on a tab lifts it: Rune's click slop, so a
/// press that does not lift is a click (SB-21).
const SLOP: f32 = 4.0;

/// A tab held by the pointer, ported from rune-kit's `shell/reorder.rs`. A press, then past
/// `SLOP` a lift that carries it from its place `from` to `to`, `dy` from the press, tabs
/// `pitch` apart. `len` is how many tabs the shelf had at the press: a drag over a shelf whose
/// tabs changed since is stale. `lead` is how far its place moved up at the lift, when the
/// outline above it folded back (SB-22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reorder {
    from: usize,
    to: usize,
    dy: f32,
    pitch: f32,
    lifted: bool,
    len: usize,
    lead: f32,
}

impl Reorder {
    /// The pointer went down on tab `from` of `len`, tabs `pitch` apart.
    pub fn press(from: usize, len: usize, pitch: f32) -> Self {
        Reorder { from, to: from, dy: 0.0, pitch: pitch.max(1.0), lifted: false, len, lead: 0.0 }
    }

    /// The pointer moved `dy` since the press. Past the slop the tab lifts, and stays lifted.
    pub fn drag(&mut self, dy: f32) {
        self.lifted |= dy.abs() >= SLOP;
        if !self.lifted {
            return;
        }
        self.dy = dy;
        let to = self.from as f32 + ((dy + self.lead) / self.pitch).round();
        self.to = to.clamp(0.0, self.len.saturating_sub(1) as f32) as usize;
    }

    /// The lifted tab's place moved up `by`, as an outline above it folded back: it stays under
    /// the pointer (SB-22).
    pub fn lead(&mut self, by: f32) {
        self.lead = by;
        self.drag(self.dy);
    }

    /// The lifted tab, by its place before the drag.
    pub fn lifted(&self) -> Option<usize> {
        self.lifted.then_some(self.from)
    }

    /// How many tabs the shelf had at the press.
    pub fn len(&self) -> usize {
        self.len
    }

    /// How far tab `i` sits from its place: the lifted tab with the pointer, and the tabs
    /// between its place and its target a step toward the gap it left.
    pub fn shift(&self, i: usize) -> f32 {
        let Reorder { from, to, dy, pitch, lifted, lead, .. } = *self;
        if !lifted {
            0.0
        } else if i == from {
            dy + lead
        } else if from < i && i <= to {
            -pitch
        } else if to <= i && i < from {
            pitch
        } else {
            0.0
        }
    }

    /// The drop: `(from, to)` when the tab lifted and lands somewhere new.
    pub fn end(self) -> Option<(usize, usize)> {
        (self.lifted && self.from != self.to).then_some((self.from, self.to))
    }
}

/// Moves the item at `from` to `to`, the others keeping their order.
pub fn move_tab<T>(tabs: &mut Vec<T>, from: usize, to: usize) {
    if from < tabs.len() && to < tabs.len() {
        let t = tabs.remove(from);
        tabs.insert(to, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SB-39, SB-40: a label folds its section and unfolds it again, and folding one section
    /// unfolds the other.
    #[test]
    fn one_section_folds_at_a_time() {
        let mut l = Layout::default();
        l.toggle(Section::Vault);
        assert_eq!(l.folded, Some(Section::Vault));
        l.toggle(Section::Shelf);
        assert_eq!(l.folded, Some(Section::Shelf), "folding the shelf unfolds the vault");
        l.toggle(Section::Shelf);
        assert_eq!(l.folded, None);
        l.toggle(Section::Vault);
        l.unfold(Section::Shelf);
        assert_eq!(l.folded, Some(Section::Vault), "unfolding the other section changes nothing");
        l.unfold(Section::Vault);
        assert_eq!(l.folded, None);
    }

    /// SB-35, SB-36: the shelf fits its tabs up to half the room, a drag's height wins, and
    /// neither section goes below its minimum.
    #[test]
    fn the_split_keeps_both_minimums() {
        assert_eq!(split(None, 600.0, 200.0, 70.0, 100.0), 200.0, "a few tabs: the shelf fits them");
        assert_eq!(split(None, 600.0, 500.0, 70.0, 100.0), 300.0, "many tabs: half the room");
        assert_eq!(split(Some(420.0), 600.0, 100.0, 70.0, 100.0), 420.0, "a dragged height stays");
        assert_eq!(split(Some(560.0), 600.0, 100.0, 70.0, 100.0), 500.0, "the vault keeps its minimum");
        assert_eq!(split(Some(10.0), 600.0, 100.0, 70.0, 100.0), 70.0, "the shelf keeps its minimum");
    }

    /// SB-132: a window too short for the stored place moves the handle up and keeps the
    /// stored place for a taller window. Too short for both minimums, the vault's room wins.
    #[test]
    fn a_short_window_keeps_the_stored_split() {
        let stored = Some(400.0);
        assert_eq!(split(stored, 300.0, 100.0, 70.0, 100.0), 200.0);
        assert_eq!(split(stored, 800.0, 100.0, 70.0, 100.0), 400.0, "a taller window puts it back");
        assert_eq!(split(stored, 150.0, 100.0, 70.0, 100.0), 50.0, "the vault keeps its minimum, the shelf takes the rest");
        assert_eq!(split(stored, 60.0, 100.0, 70.0, 100.0), 0.0, "nothing left for the shelf");
    }

    /// SB-20: the tab above becomes active, or the one below when the closed tab was first;
    /// none with no tab left.
    #[test]
    fn closing_takes_the_tab_above() {
        assert_eq!(next_active(2, 3), Some(1), "the tab above");
        assert_eq!(next_active(0, 3), Some(0), "the first: the one below, now first");
        assert_eq!(next_active(3, 3), Some(2), "the last: the one above, now last");
        assert_eq!(next_active(0, 0), None, "no tab left");
    }

    /// SB-21: a lifted tab follows the pointer, and the tabs between its place and its target
    /// step a pitch toward the gap it left; the drop moves it there.
    #[test]
    fn reorder_shifts_the_passed_tabs() {
        let mut r = Reorder::press(1, 5, 37.0);
        r.drag(80.0);
        assert_eq!(r.lifted(), Some(1));
        let shifts: Vec<f32> = (0..5).map(|i| r.shift(i)).collect();
        assert_eq!(shifts, [0.0, 80.0, -37.0, -37.0, 0.0], "down two places");
        r.drag(-500.0);
        assert_eq!((r.shift(0), r.shift(2)), (37.0, 0.0), "up past the top: clamped to the first place");
        assert_eq!(r.end(), Some((1, 0)));
        let mut r = Reorder::press(3, 5, 37.0);
        r.drag(900.0);
        assert_eq!(r.end(), Some((3, 4)), "down past the end: the last place");
        let mut tabs = vec!["a", "b", "c", "d"];
        move_tab(&mut tabs, 0, 2);
        assert_eq!(tabs, ["b", "c", "a", "d"]);
        move_tab(&mut tabs, 3, 0);
        assert_eq!(tabs, ["d", "b", "c", "a"]);
    }

    /// SB-22: a tab lifted from under an outline that folds back keeps under the pointer, and
    /// lands by where the pointer is over the folded shelf.
    #[test]
    fn a_lead_keeps_the_lifted_tab_under_the_pointer() {
        let mut r = Reorder::press(3, 5, 37.0);
        r.drag(-10.0);
        r.lead(96.0);
        assert_eq!(r.shift(3), 86.0, "its place moved up 96: it sits 86 below it, under the pointer");
        assert_eq!(r.end(), Some((3, 4)), "and lands a place down");
    }

    /// SB-21: a press that moves less than the slop is a click: nothing lifts or moves, and a
    /// drag that comes back to its place moves nothing either.
    #[test]
    fn a_press_that_does_not_move_is_a_click() {
        let mut r = Reorder::press(2, 4, 37.0);
        r.drag(3.0);
        assert_eq!((r.lifted(), r.shift(2), r.shift(3)), (None, 0.0, 0.0));
        assert_eq!(r.end(), None);
        let mut r = Reorder::press(2, 4, 37.0);
        r.drag(40.0);
        r.drag(2.0);
        assert_eq!(r.lifted(), Some(2), "once lifted it stays lifted");
        assert_eq!(r.end(), None, "dropped in its own place");
    }
}
