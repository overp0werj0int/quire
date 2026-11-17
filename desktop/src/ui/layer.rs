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
    let mut out = Vec::new();
    for l in v.s.open_layers() {
        layer(v, l, &mut out);
    }
    out
}

fn layer(v: V, l: &Layer, out: &mut Vec<El>) {
    let kind = l.kind();
    let card = match l {
        Layer::Palette(pal) => super::palette::palette(v, pal),
        Layer::Settings => Some(super::settings::settings(v)),
        Layer::Lock(_) => Some(super::lock::dialog(v)),
        Layer::Help => Some(super::reader::help_sheet(v)),
        Layer::Sheet(sh) => Some(super::sheet::sheet(v, sh)),
        Layer::Drawer(_) => super::drawer(v),
        Layer::Menu(m) => Some(super::menu::menu(v, m)),
    };
    let Some(card) = card else { return };
    // A press outside the edit sheet does nothing: only Esc and its keys close it (SHEET-015).
    let el = card.el.modal_tx(kind != Kind::Sheet, |s: &mut App, tx| s.dismiss(tx)).modal_keeps(keeps(kind));
    let (key, paint) = plane(kind);
    match card.plane {
        Some(plane) => out.push(plane.key(key).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(paint).fill(v.p.scrim).child(el)),
        None => {
            if matches!(kind, Kind::Drawer(_)) {
                out.push(page_scrim(v, paint - 1));
            }
            out.push(el.paint_order(paint));
        }
    }
}

/// The scrim behind a drawer: the layers' colour over the page, under the title bar, which
/// stays live. It fades in and out with the drawer's slide.
fn page_scrim(v: V, paint: i16) -> El {
    let fade = rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::new(0.16, 0.0));
    rune::el()
        .key("drawer-scrim")
        .pin(Some(0.0), Some(super::titlebar::BAR_H), Some(0.0), Some(0.0))
        .paint_order(paint)
        .fill(v.p.scrim)
        .transition(rune::Transition::opacity().springs(fade))
}

/// The plane's key and paint order.
fn plane(kind: Kind) -> (&'static str, i16) {
    match kind {
        Kind::Palette => ("pal-layer", 50),
        Kind::Settings => ("set-layer", 50),
        Kind::Lock => ("lock-layer", 50),
        Kind::Help => ("help-scrim", 50),
        Kind::Sheet => ("sheet-scrim", 50),
        Kind::Drawer(_) => ("", 35),
        Kind::Menu => ("", 80),
    }
}

/// What stays live to the pointer outside the card: the toast and the notice; over the edit
/// sheet the strip that moves the window (SHEET-015); over a drawer the title bar and the
/// window corner.
fn keeps(kind: Kind) -> Vec<&'static str> {
    let mut keys = vec!["toast-row", "notice"];
    match kind {
        Kind::Sheet => keys.push("sheet-drag"),
        Kind::Drawer(_) => keys.extend(["titlebar", "left-zone"]),
        _ => {}
    }
    keys
}
