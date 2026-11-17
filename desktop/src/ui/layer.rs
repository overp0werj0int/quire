//! The plane each open layer sits on (`crate::layer`). One wrapper for every layer: it makes
//! the layer's card modal, so the keyboard moves into it, Tab stays in it and nothing behind
//! takes a key, a press or the wheel; a press outside the card, with either button, or Esc
//! dismisses it (`App::dismiss`); and the plane under the card dims the page.
//!
//! The toast and the notice are not layers: they paint over every layer (`El::raise`) and stay
//! live to the pointer while one is open.

use rune::El;

use super::V;
use crate::app::App;
use crate::layer::{Kind, Layer};

/// What a layer's builder gives the plane.
pub(super) struct Card {
    /// The layer itself, which takes the modal.
    pub el: El,
    /// The plane laid out to hold the card, which the wrapper keys, pins over the window and
    /// dims; `None` for a card that places itself in the window.
    pub plane: Option<El>,
}

/// Every open layer, bottom first.
pub(super) fn layers(v: V) -> Vec<El> {
    v.s.open_layers().filter_map(|l| layer(v, l)).collect()
}

fn layer(v: V, l: &Layer) -> Option<El> {
    let kind = l.kind();
    let card = match l {
        Layer::Palette(pal) => super::palette::palette(v, pal)?,
        Layer::Settings => super::settings::settings(v),
        Layer::Lock(_) => super::lock::dialog(v),
        Layer::Help => super::reader::help_sheet(v),
        Layer::Sheet(sh) => super::sheet::sheet(v, sh),
    };
    // A press outside the edit sheet does nothing: only Esc and its keys close it (SHEET-015).
    let el = card.el.modal_tx(kind != Kind::Sheet, |s: &mut App, tx| s.dismiss(tx)).modal_keeps(keeps(kind));
    let (key, paint) = plane(kind);
    Some(match card.plane {
        Some(plane) => plane.key(key).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(paint).fill(v.p.scrim).child(el),
        None => el.paint_order(paint),
    })
}

/// The plane's key and paint order.
fn plane(kind: Kind) -> (&'static str, i16) {
    match kind {
        Kind::Palette => ("pal-layer", 50),
        Kind::Settings => ("set-layer", 50),
        Kind::Lock => ("lock-layer", 50),
        Kind::Help => ("help-scrim", 50),
        Kind::Sheet => ("sheet-scrim", 50),
    }
}

/// What stays live to the pointer outside the card: the toast and the notice, and over the
/// edit sheet the strip that moves the window (SHEET-015).
fn keeps(kind: Kind) -> Vec<&'static str> {
    let mut keys = vec!["toast-row", "notice"];
    if kind == Kind::Sheet {
        keys.push("sheet-drag");
    }
    keys
}
