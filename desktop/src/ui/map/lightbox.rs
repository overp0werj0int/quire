//! The lightbox (MAP-047): an image from the map as large as 90% of the window lets it be,
//! never larger than itself, with its caption, Open externally and Edit Markdown.

use rune::{el, row, text};

use super::map_look;
use crate::app::App;
use crate::layer::Lightbox;
use crate::page::r;
use crate::roles::Roles;
use crate::ui::V;
use crate::ui::layer::Card;
use crate::ui::preview::{Cx, LOADING, Pic};
use crate::ui::widgets::outline_button;

/// The card's padding, the gap over the bar, and the bar's height.
const PAD: f32 = 12.0;
const GAP: f32 = 12.0;
const BAR_H: f32 = 28.0;
/// The bar's buttons and a little caption need this much width.
const MIN_W: f32 = 360.0;

pub(in crate::ui) fn lightbox(v: V, lb: &Lightbox) -> Card {
    let look = map_look(v);
    let cx = Cx::map(v, &look, None);
    let c = Roles::new(v.p);
    let room = [v.vw * 0.9 - 2.0 * PAD, v.vh * 0.9 - 2.0 * PAD - GAP - BAR_H];
    let natural = match cx.pic(&lb.img) {
        Pic::Ready(im) => im.size().filter(|[w, h]| *w > 0 && *h > 0).map(|[w, h]| (im, [w as f32, h as f32])).ok_or_else(|| LOADING.to_string()),
        Pic::Loading => Err(LOADING.to_string()),
        Pic::Missing(msg) => Err(msg),
    };
    let (pic, pw) = match natural {
        Ok((im, [nw, nh])) => {
            let k = 1f32.min(room[0] / nw).min(room[1] / nh);
            let [w, h] = [(nw * k).round().max(1.0), (nh * k).round().max(1.0)];
            let alt = if lb.img.alt.is_empty() { "Image".to_string() } else { lb.img.alt.clone() };
            (el().pixels(im.pixels()).size(w, h).radius(4.0).clip_rounded(4.0).role("img").label(alt), w)
        }
        Err(msg) => {
            let w = cx.placeholder_w(&msg).min(room[0]);
            (cx.placeholder(msg, None).w(w), w)
        }
    };
    let w = pw.max(MIN_W).min(room[0].max(1.0));
    let b = look.b;
    let caption = text(lb.img.alt.clone()).key("lightbox-caption").font(look.prose(r(b * 0.84), 1.5).italic()).color(look.c.dim).truncate().grow(1.0).min_w(0.0);
    let bar = row()
        .key("lightbox-bar")
        .w(w)
        .h(BAR_H)
        .gap(8.0)
        .center_y()
        .child(caption)
        .child(outline_button(v.p, "lightbox-open", "Open externally", BAR_H, |s: &mut App, tx| s.lightbox_open(tx)))
        .child(outline_button(v.p, "lightbox-edit", "Edit Markdown", BAR_H, |s: &mut App, tx| s.lightbox_edit(tx)));
    let mut card = crate::ui::reader::popup(&c)
        .key("lightbox")
        .role("dialog")
        .label("Image")
        .padding(PAD)
        .child(rune::col().gap(GAP).items_center().child(pic.key("lightbox-pic")).child(bar));
    if v.p.dark {
        card = card.border(1.0, crate::roles::mix(c.bg, c.text, 0.20));
    }
    Card { el: card, plane: Some(rune::col().center()) }
}
