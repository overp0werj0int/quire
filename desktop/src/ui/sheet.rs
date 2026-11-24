//! The edit sheet over the reader (`behavior/edit-sheet.md`, SHEET-015 to SHEET-031): a scrim
//! over the whole window, and a card with the note's name, the block's neighbours faint, the
//! writer's editor and the key row. Its behaviour is `crate::sheet`.

use rune::{Key, KeyEvent, ScrollFade, ScrollSpec, TextHighlight, Tx, col, el, row, text, text_input};

use super::V;
use super::layer::Card;
use super::styles::sans;
use super::reader::{cap, popup};
use crate::app::App;
use crate::keymap::Ctx;
use crate::page::r;
use crate::roles::Roles;
use crate::sheet::{EDITOR, Scope, Sheet};
use crate::theme;

const PAD: f32 = 22.0;
const GAP: f32 = 10.0;
const FADE: f32 = 28.0;
const KEY_ROW: f32 = 24.0;
/// The title row's height: one line of bodySmall.
const TITLE: f32 = 18.0;

/// The edit sheet, a card in the middle of the window over the reader (`super::layer`).
pub(super) fn sheet(v: V, sh: &Sheet) -> Card {
    let s = v.s;
    let c = Roles::new(v.p);
    let b = s.settings().size as f32;
    let style = super::editor::ed_style(v);
    let e = style.size;
    // SHEET-016, SHEET-028: the editor's gutter, the reading column and the editor's measure.
    let gutter = super::editor::hang_gutter(e);
    let colw = super::preview::column(v).width;
    let w = (v.vw - 48.0).min(colw + 2.0 * gutter + 2.0 * PAD);
    let inner = w - 2.0 * PAD;
    let avg = crate::page::avg_char(theme::editor_font(&s.settings().font).family);
    let measure = (inner - 2.0 * gutter).min(r(avg * e * 66.0)).max(0.0);
    let ctx_w = (inner - gutter).min(colw).max(0.0);
    let ctx_h = r(b * 1.6 * 3.0);

    // SHEET-020: the note's name and the line span, counted from 1.
    let (a, z) = (sh.pick.lines.start + 1, sh.pick.lines.end.max(sh.pick.lines.start + 1));
    let span = if a == z { format!("line {a}") } else { format!("lines {a} to {z}") };
    let title = row()
        .key("sheet-title")
        .gap(8.0)
        .center_y()
        .child(text(crate::md::base(&sh.rel).to_string()).font(sans(12.5, 600.0)).color(c.text).truncate().shrink().min_w(0.0))
        .child(text(span).key("sheet-span").font(sans(12.5, 400.0)).color(c.faint));

    // SHEET-022 to SHEET-026: a neighbour, clipped to three reading lines, fading at the edge
    // where it is cut.
    let context = |key: &str, i: usize, before: bool| {
        let fade = if before { ScrollFade::strength(FADE).full_at(0.5).linear() } else { ScrollFade::strength(0.0).with_end(ScrollFade::strength(FADE).full_at(0.5).linear()) };
        col()
            .key(key)
            .w(ctx_w)
            .ml(gutter)
            .max_h(ctx_h)
            .min_h(0.0)
            .scroll(ScrollSpec { stick_bottom: before })
            .scroll_fade(fade)
            .opacity(0.55)
            .child(super::preview::context(v, &sh.doc, i))
    };
    let shown = sh.pick.before.is_some() as u8 + sh.pick.after.is_some() as u8;

    // SHEET-019: the card grows with its text to 70% of the window, then the editor scrolls.
    let chrome = 2.0 * PAD + TITLE + GAP + KEY_ROW + 14.0 + shown as f32 * (ctx_h + GAP);
    let limit = (r(0.7 * v.vh) - chrome).max(90.0);
    let ink = crate::writer::Ink::new(&c);
    let (spans, strikes, hangs) = s.sheet_spans(&sh.edit.text, e, &ink);
    let input = text_input(&sh.edit, style)
        .key(EDITOR)
        .w(measure + gutter)
        .color(ink.text);
    let input = super::editor::writer_caret(input, &c)
        .label("Edit the block")
        .spans(spans)
        .strikes(strikes)
        .text_highlights(Vec::<TextHighlight>::new())
        .on_edit(|s: &mut App| s.sheet_field())
        .on_input_key(on_sheet_key);
    // The field starts at the card's padding and its lines a gutter in, so marks hang there.
    let input = super::editor::with_edit_menu(input, EDITOR).paragraph_styles(super::editor::paragraph_styles(&hangs, gutter));
    let editor = col()
        .key("sheet-scroll")
        .w_full()
        .min_h(90.0)
        .max_h(limit)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_margin(6.0)
        .child(el().key("sheet-pad").w_full().py(6.0).child(input));

    // SHEET-030, SHEET-038, SHEET-047: the key row, or the problem in its place.
    let key_of = |id: &str| s.keymap.label(id);
    let foot = match sh.problem {
        Some(p) => row().key("sheet-problem").h(KEY_ROW).w_full().center_y().child(text(p).font(sans(11.5, 400.0)).color(c.red).truncate().shrink().min_w(0.0)),
        None => {
            let armed = sh.armed.is_some();
            let (cancel, tone) = if armed { ("again to discard your edit", c.text) } else { ("cancels", c.faint) };
            row()
                .key("sheet-keys")
                .h(KEY_ROW)
                .gap(8.0)
                .center_y()
                .child(cap(key_of("sheet.cancel"), v.p))
                .child(text(cancel).key("sheet-cancel").font(sans(11.5, 400.0)).color(tone))
                .child(el().w(10.0 - 8.0))
                .child(cap(key_of("sheet.save"), v.p))
                .child(text("saves").font(sans(11.5, 400.0)).color(c.faint))
        }
    };

    let mut card = col().key("sheet-card").w(w).padding(PAD).gap(GAP).child(title);
    if let Some(i) = sh.pick.before {
        card = card.child(context("sheet-before", i, true));
    }
    card = card.child(editor);
    if let Some(i) = sh.pick.after {
        card = card.child(context("sheet-after", i, false));
    }
    card = card.child(foot.mt(14.0 - GAP));
    let label = if sh.scope == Scope::Section { "Edit the section" } else { "Edit the block" };
    let card = popup(&c).key("sheet-pop").radius(6.0).role("dialog").label(label).child(card);

    // SHEET-015: the scrim takes every click and wheel, and only moving the window stays live.
    let drag = row().key("sheet-drag").pin(Some(0.0), Some(0.0), Some(0.0), None).h(super::titlebar::BAR_H).window_drag().child(super::widgets::dots(v.p).ml(14.0).mt(super::titlebar::BAR_H / 2.0 - 6.0));
    Card { el: card, plane: Some(col().center().child(drag)) }
}

/// Keys in the sheet's editor (SHEET-033 to SHEET-037): the sheet's own, then the writer's
/// editing keys; every other key is the field's.
fn on_sheet_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if crate::keys::esc_again(ev, tx) {
        return true;
    }
    if matches!(ev.key, Key::Escape) {
        s.sheet_escape(tx);
        return true;
    }
    match s.keymap.lookup(Ctx::Sheet, ev).iter().find_map(|a| crate::keys::handler(a.id)) {
        Some(run) => {
            run(s, tx);
            true
        }
        None => false,
    }
}
