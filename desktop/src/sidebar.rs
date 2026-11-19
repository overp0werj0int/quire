//! The sidebar's two lists as state: which one is folded and where the handle between them
//! sits (`docs/reader-writer/behavior/sidebar.md`, SB-33 to SB-47). Pure, with no drawing:
//! `ui::sidebar` draws from it.

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
}
