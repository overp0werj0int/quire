//! The lightbox (MAP-047): an image from the map as large as 90% of the window lets it be,
//! never larger than itself, built on the shared `MiniCard` component with header & footer hover
//! areas revealing the top-right icon-only `mini_bar` (`Prev`, `Next`, `Open externally`,
//! `Edit Markdown`, and rightmost `×` Close).

use rune::{el, row, text};

use super::map_look;
use crate::app::App;
use crate::keymap;
use crate::layer::Lightbox;
use crate::page::r;
use crate::roles::Roles;
use crate::ui::V;
use crate::ui::components::{Control, MiniCard, mini_button};
use crate::ui::icons;
use crate::ui::layer::Card;
use crate::ui::preview::{Cx, LOADING, Pic};
use crate::ui::reader::cap;
use crate::ui::styles::{self, sans};
use crate::ui::tokens::space;

/// The card's padding, the gap around the image, and the header and footer bar heights.
const PAD: f32 = 12.0;
const GAP: f32 = 10.0;
const HEAD_H: f32 = 28.0;
const BAR_H: f32 = 28.0;
/// Minimum card content width so the header title and mini-bar stay well separated.
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
    let (rw, rg, rc) = crate::ui::focus_ring(v.p);

    let total_btns = if multi { 5 } else { 3 };
    let mut header = row()
        .key("lightbox-header")
        .w_full()
        .h(PAD + HEAD_H + GAP)
        .pt(PAD)
        .pb(GAP)
        .pl(PAD)
        .pr(PAD + MiniCard::actions_reserve(total_btns))
        .gap(space::PX_8)
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

    let body = el()
        .key("lightbox-body")
        .w_full()
        .px(PAD)
        .center()
        .child(pic.key("lightbox-pic"));

    let caption = text(img.alt.clone())
        .key("lightbox-caption")
        .font(look.prose(r(b * 0.84), 1.5).italic())
        .color(look.c.dim)
        .truncate()
        .grow(1.0)
        .min_w(0.0);
    let mut hints = row().gap(space::PX_6).center_y();
    if multi {
        hints = hints
            .child(cap("←".into(), v.p))
            .child(cap("→".into(), v.p))
            .child(text("navigate").font(styles::small()).color(c.faint))
            .child(el().w(space::PX_2));
    }
    hints = hints
        .child(cap(esc_lbl.clone(), v.p))
        .child(text("closes").font(styles::small()).color(c.faint));
    let footer = row()
        .key("lightbox-bar")
        .w_full()
        .h(GAP + BAR_H + PAD)
        .pt(GAP)
        .pb(PAD)
        .px(PAD)
        .gap(space::PX_8)
        .center_y()
        .child(caption)
        .child(hints);

    let mut card = MiniCard::new(v.p, "lightbox", "lightbox-chrome", "Image")
        .card_key("lightbox-card")
        .actions_keys("lightbox-actions", "lightbox-actions-zone")
        .width(w + 2.0 * PAD)
        .bar_inset(PAD, PAD);
    if multi {
        let prev_btn = mini_button(v.p, "lightbox-prev", &icons::CHEVRON_LEFT, "Previous · ←", None)
            .focus_ring(rw, rg, rc)
            .control(|s: &mut App, _| s.lightbox_step(-1));
        let next_btn = mini_button(v.p, "lightbox-next", &icons::CHEVRON_RIGHT, "Next · →", None)
            .focus_ring(rw, rg, rc)
            .control(|s: &mut App, _| s.lightbox_step(1));
        card = card.action(prev_btn).action(next_btn);
    }
    let open_btn = mini_button(v.p, "lightbox-open", &icons::EXTERNAL, "Open externally", None)
        .focus_ring(rw, rg, rc)
        .control(|s: &mut App, tx| s.lightbox_open(tx));
    let edit_btn = mini_button(v.p, "lightbox-edit", &icons::PENCIL, "Edit Markdown", None)
        .focus_ring(rw, rg, rc)
        .control(|s: &mut App, tx| s.lightbox_edit(tx));
    let close_btn = mini_button(v.p, "lightbox-close", &icons::CLOSE, &format!("Close · {esc_lbl}"), None)
        .focus_ring(rw, rg, rc)
        .control(|s: &mut App, tx| s.close_layer(tx));

    let card = card.action(open_btn).action(edit_btn).close(close_btn).build(header, body, footer);
    Card { el: card, plane: Some(rune::col().center()) }
}
