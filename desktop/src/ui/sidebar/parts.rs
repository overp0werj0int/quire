//! Pieces the sidebar's lists share: the two-row row of a tab and of a search result, and the
//! label over a section (SB-05, SB-15, SB-61).

use rune::{El, TextStyle, col, el, row, text};

use super::super::panel;
use super::super::styles::{caption, sans};
use crate::roles::Roles;
use crate::theme::Pal;

/// A two-row row's height: its two rows and 4 pt above and below (SB-07).
pub(super) const TWO_ROWS_H: f32 = 36.0;
/// A two-row row's corners.
pub(super) const RADIUS: f32 = 6.0;
/// A section label's row (SB-05).
pub(super) const LABEL_H: f32 = 26.0;

/// The caption's style under a two-row title: Instrument Sans, smaller (SB-07).
pub(super) fn caption_style() -> TextStyle {
    sans(10.5, 400.0)
}

/// A row of two rows keyed `key`: `title` over `caption`, at the rows' text edge, the room
/// after them for what the caller adds (SB-15, SB-61).
pub(super) fn two_rows(key: impl Into<String>, title: El, caption: El) -> El {
    let names = col().grow(1.0).shrink().min_w(0.0).gap(1.0).child(title).child(row().w_full().child(caption));
    row().key(key.into()).h(TWO_ROWS_H).mx(panel::INSET).pl(panel::TEXT_X - panel::INSET).gap(2.0).center_y().radius(RADIUS).child(names)
}

/// A section's label keyed `key`: `lead` (a chevron, for a label that folds), then `name` in
/// small capitals and `count` at the right, in faint (SB-05). A label with no lead starts its
/// name at the rows' text edge.
pub(super) fn label_row(p: &Pal, key: &str, lead: Option<El>, name: &str, count: String) -> El {
    let c = Roles::panel(p);
    // The chevron at 8 pt and the name at the rows' text edge.
    let out = row().key(key).h(LABEL_H).mx(6.0).pr(panel::RIGHT - 6.0).gap(4.0).center_y().radius(4.0);
    let out = match lead {
        Some(lead) => out.pl(2.0).child(lead),
        None => out.pl(panel::TEXT_X - 6.0),
    };
    out.child(caption(name, false, p).truncate().shrink().min_w(0.0)).child(el().grow(1.0)).child(text(count).font(sans(10.0, 500.0)).color(c.faint))
}
