//! The lightbox (MAP-047): an image from the map as large as 90% of the window lets it be,
//! never larger than itself, with its caption, Open externally and Edit Markdown.

use rune::{el, icon, row, text};

use super::map_look;
use crate::app::App;
use crate::keymap;
use crate::layer::Lightbox;
use crate::page::r;
use crate::roles::Roles;
use crate::ui::V;
use crate::ui::icons::CLOSE;
use crate::ui::layer::Card;
use crate::ui::preview::{Cx, LOADING, Pic};
use crate::ui::styles::sans;
use crate::ui::widgets::{Control, kbd, outline_button};

/// The card's padding, the gap around the image, and the header and footer bar heights.
const PAD: f32 = 12.0;
const GAP: f32 = 10.0;
const HEAD_H: f32 = 28.0;
const BAR_H: f32 = 28.0;
/// The bar's buttons and a little caption need this much width.
const MIN_W: f32 = 360.0;

pub(in crate::ui) fn lightbox(v: V, lb: &Lightbox) -> Card {
    let look = map_look(v);
    let cx = Cx::map(v, &look, None);
    let c = Roles::new(v.p);
    let img = lb.current();
    let multi = lb.images.len() > 1;
    let room = [v.vw * 0.9 - 2.0 * PAD, v.vh * 0.9 - 2.0 * PAD - 2.0 * GAP - HEAD_H - BAR_H];
    let natural = match cx.pic(img) {
        Pic::Ready(im) => im.size().filter(|[w, h]| *w > 0 && *h > 0).map(|[w, h]| (im, [w as f32, h as f32])).ok_or_else(|| LOADING.to_string()),
        Pic::Loading => Err(LOADING.to_string()),
        Pic::Missing(msg) => Err(msg),
    };
    let (pic, pw) = match natural {
        Ok((im, [nw, nh])) => {
            let k = 1f32.min(room[0] / nw).min(room[1] / nh);
            let [w, h] = [(nw * k).round().max(1.0), (nh * k).round().max(1.0)];
            let alt = if img.alt.is_empty() { "Image".to_string() } else { img.alt.clone() };
            (el().pixels(im.pixels()).size(w, h).radius(4.0).clip_rounded(4.0).role("img").label(alt), w)
        }
        Err(msg) => {
            let w = cx.placeholder_w(&msg).min(room[0]);
            (cx.placeholder(msg, None).w(w), w)
        }
    };
    let min_w = if multi { 480.0 } else { MIN_W };
    let w = pw.max(min_w).min(room[0].max(1.0));
    let b = look.b;
    let esc_lbl = keymap::label_of("Esc");
    let mut header = row()
        .key("lightbox-header")
        .w(w)
        .h(HEAD_H)
        .gap(8.0)
        .center_y()
        .child(text("Image").font(sans(12.5, 600.0)).color(c.text));
    if multi {
        header = header.child(
            text(format!("{} / {}", lb.index + 1, lb.images.len()))
                .key("lightbox-pos")
                .font(sans(11.5, 500.0))
                .color(c.dim),
        );
    }
    let header = header
        .child(el().grow(1.0))
        .child(kbd(v.p, esc_lbl.clone(), false))
        .child(
            row()
                .key("lightbox-close")
                .size(24.0, 24.0)
                .center()
                .radius(6.0)
                .hover(v.p.hover)
                .hover_group()
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .label("Close")
                .help(format!("Close · {esc_lbl}"))
                .control(|s: &mut App, tx| s.close_layer(tx))
                .child(icon(&CLOSE, 13.0).color(v.p.mute).group_color(v.p.ink)),
        );
    let caption = text(img.alt.clone()).key("lightbox-caption").font(look.prose(r(b * 0.84), 1.5).italic()).color(look.c.dim).truncate().grow(1.0).min_w(0.0);
    let mut bar = row()
        .key("lightbox-bar")
        .w(w)
        .h(BAR_H)
        .gap(8.0)
        .center_y();
    if multi {
        bar = bar
            .child(outline_button(v.p, "lightbox-prev", "Prev", BAR_H, |s: &mut App, _| s.lightbox_step(-1)))
            .child(outline_button(v.p, "lightbox-next", "Next", BAR_H, |s: &mut App, _| s.lightbox_step(1)));
    }
    let bar = bar
        .child(caption)
        .child(outline_button(v.p, "lightbox-open", "Open externally", BAR_H, |s: &mut App, tx| s.lightbox_open(tx)))
        .child(outline_button(v.p, "lightbox-edit", "Edit Markdown", BAR_H, |s: &mut App, tx| s.lightbox_edit(tx)));
    let mut card = crate::ui::reader::popup(&c)
        .key("lightbox")
        .role("dialog")
        .label("Image")
        .padding(PAD)
        .child(rune::col().gap(GAP).items_center().child(header).child(pic.key("lightbox-pic")).child(bar));
    if v.p.dark {
        card = card.border(1.0, crate::roles::mix(c.bg, c.text, 0.20));
    }
    Card { el: card, plane: Some(rune::col().center()) }
}
