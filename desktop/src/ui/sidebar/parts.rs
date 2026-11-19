//! Pieces the sidebar's lists share: the two-row row of a tab and of a search result, and the
//! label over a section (SB-05, SB-15, SB-61).

use rune::{El, TextStyle, col, el, row, text};

use super::super::styles::sans;
use crate::roles::Roles;
use crate::theme::Pal;

/// Outer margin of every sidebar row and section label, matching the top bar and footer (SB-07).
pub(super) const INSET: f32 = 8.0;
/// Left and right padding inside a sidebar row or label.
pub(super) const ROW_PX: f32 = 8.0;
/// Where a root row's text or icon starts: outer margin plus row padding.
pub(super) const TEXT_X: f32 = INSET + ROW_PX;
/// Room between adjacent rows in a sidebar list.
pub(super) const GAP: f32 = 1.0;
/// A single-line row's height: 12.5 pt text at ~1.3 leading plus 5 pt top and bottom (SB-07).
pub(super) const ROW_H: f32 = 27.0;
/// A two-row row's height: 12.5 pt title and 10.5 pt caption at 1.15 leading plus 4 pt top and
/// bottom (SB-07).
pub(super) const TWO_ROWS_H: f32 = 35.0;
/// A row's corners.
pub(super) const RADIUS: f32 = 6.0;
/// A section label's row: the least pointer target (SB-05, SB-08).
pub(super) const LABEL_H: f32 = 24.0;
/// Single-line row and tab title size (SB-07).
pub(super) const ROW_TEXT: f32 = 12.5;
/// Folder and tag count size (`.r .m`).
pub(super) const COUNT_TEXT: f32 = 11.0;
/// Outline entry size (SB-07, SB-30).
pub(super) const OUT_TEXT: f32 = 11.5;
/// Line height of a two-row title (`12.5 × 1.15 ≈ 14.4`, giving exact 4 pt top and bottom padding
/// in `TWO_ROWS_H`).
pub(super) const TAB_TITLE_LH: f32 = 14.0;
/// Line height of a two-row caption (`10.5 × 1.15 ≈ 12.1`).
pub(super) const TAB_CAP_LH: f32 = 12.0;

/// The caption's style under a two-row title: Instrument Sans, 10.5 pt at 1.15 leading (SB-07).
pub(super) fn caption_style() -> TextStyle {
    sans(10.5, 400.0).lh(TAB_CAP_LH)
}

/// A row of two rows keyed `key`: `title` over `caption`, at the rows' text edge, the room
/// after them for what the caller adds (SB-15, SB-61).
pub(super) fn two_rows(key: impl Into<String>, title: El, caption: El) -> El {
    let names = col().grow(1.0).shrink().min_w(0.0).gap(1.0).child(title).child(row().w_full().child(caption));
    row().key(key.into()).h(TWO_ROWS_H).mx(INSET).pl(ROW_PX).gap(2.0).center_y().radius(RADIUS).child(names)
}

/// A section's label text and count: 10 pt sans at line-height 1, uppercase 600 with 0.1 em
/// tracking on the left, 500 not uppercase on the right, both in faint (SB-05, SB-07).
fn label_parts(p: &Pal, out: El, name: &str, count: String) -> El {
    let c = Roles::panel(p);
    out.child(text(name.to_uppercase()).font(sans(10.0, 600.0).lh(10.0).tracking(1.0)).color(c.faint).truncate().shrink().min_w(0.0))
        .child(el().grow(1.0))
        .child(text(count).font(sans(10.0, 500.0).lh(10.0)).color(c.faint))
}

/// A section's label keyed `key`: `lead` (a chevron, for a label that folds), then `name` in
/// small capitals and `count` at the right, in faint (SB-05). Clickable labels keep `LABEL_H`
/// (24 pt, SB-08).
pub(super) fn label_row(p: &Pal, key: &str, lead: Option<El>, name: &str, count: String) -> El {
    let out = row().key(key).h(LABEL_H).mx(INSET).px(ROW_PX).gap(6.0).center_y().radius(RADIUS);
    let out = match lead {
        Some(lead) => out.child(lead),
        None => out,
    };
    label_parts(p, out, name, count)
}

/// A non-interactive section heading in search results (Titles, Text, Tags, Tagged): the spec's
/// 10 pt top and 5 pt bottom padding over 10 pt text (25 pt tall, SB-05).
pub(super) fn section_label(p: &Pal, key: &str, name: &str, count: String) -> El {
    let out = row().key(key).mx(INSET).px(ROW_PX).pt(10.0).pb(5.0).gap(6.0).center_y();
    label_parts(p, out, name, count)
}
