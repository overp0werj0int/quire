//! The edit sheet over the reader (`behavior/edit-sheet.md`, SHEET-015 to SHEET-031): a scrim
//! over the whole window, and a card with the note's name, the block's neighbours faint, the
//! writer's editor and the key row. Its behaviour is `crate::sheet`.

use rune::{Key, KeyEvent, ScrollFade, ScrollSpec, TextHighlight, Tx, col, el, row, text, text_input};

use super::V;
use super::components::{Control, MINI_BAR_H, dialog_frame, dots, mini_bar, mini_bar_w, mini_button};
use super::icons;
use super::layer::Card;
use super::styles::sans;
use super::reader::cap;
use super::tokens::{motion, space};
use crate::app::App;
use crate::keymap::Ctx;
use crate::page::r;
use crate::roles::Roles;
use crate::sheet::{EDITOR, Scope, Sheet};
use crate::theme;

const PAD_X: f32 = space::PAD_DIALOG;
const HEADER_H: f32 = 48.0;
const BODY_PY: f32 = space::PX_24;
const BODY_GAP: f32 = space::GAP_SECTION;
const FOOTER_H: f32 = 44.0;
const FADE: f32 = space::SCROLL_FADE_DIALOG;
/// Matches the 20 pt height of the footer's `kbd` key caps.
const KEY_ROW: f32 = 20.0;

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
    let w = (v.vw - space::DIALOG_MARGIN_BOTH).min(colw + 2.0 * gutter + 2.0 * PAD_X);
    let inner = w - 2.0 * PAD_X;
    let avg = crate::page::avg_char(theme::editor_font(&s.settings().font).family);
    let measure = (inner - 2.0 * gutter).min(r(avg * e * 66.0)).max(0.0);
    let ctx_w = (inner - gutter).min(colw).max(0.0);
    let ctx_h = r(b * 1.6 * 3.0);

    // SHEET-020: the note's name and the line span, counted from 1.
    let (a, z) = (sh.pick.lines.start + 1, sh.pick.lines.end.max(sh.pick.lines.start + 1));
    let span = if a == z { format!("line {a}") } else { format!("lines {a} to {z}") };
    let dirty = sh.edit.text != sh.orig;
    let mut title = row()
        .key("sheet-title")
        .gap(8.0)
        .pr(mini_bar_w(2) + 8.0)
        .center_y()
        .child(text(crate::md::base(&sh.rel).to_string()).font(sans(12.5, 600.0)).color(c.text).truncate().shrink().min_w(0.0))
        .child(text(span).key("sheet-span").font(sans(12.5, 400.0)).color(c.faint));
    if dirty {
        title = title.child(
            text("Modified")
                .key("sheet-dirty")
                .font(sans(11.0, 500.0))
                .color(c.accent),
        );
    }

    // SHEET-022 to SHEET-026: a neighbour, clipped to three reading lines, fading at the edge
    // where it is cut.
    let context = |key: &str, i: usize, before: bool| {
        let fade = ScrollFade::strength(FADE).full_at(0.5).linear();
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
    let chrome = HEADER_H + FOOTER_H + 2.0 * BODY_PY + shown as f32 * (ctx_h + BODY_GAP);
    let limit = (r(0.7 * v.vh) - chrome).max(90.0);
    let ink = crate::writer::Ink::new(&c);
    let (spans, strikes, hangs) = s.sheet_spans(&sh.edit.text, e, &ink);
    let input = text_input(&sh.edit, style)
        .key(EDITOR)
        .w(measure + gutter)
        .color(ink.text);
    let input = super::editor::writer_caret(input, &c)
        .label("Edit the block")
        .spellcheck(s.settings().spellcheck)
        .spans(spans)
        .strikes(strikes)
        .text_highlights(Vec::<TextHighlight>::new())
        .on_edit(|s: &mut App| s.sheet_field())
        .on_input_paste(|s: &mut App, content, tx| s.paste_content(content, tx))
        .on_input_key(on_sheet_key);
    // The field starts at the card's padding and its lines a gutter in, so marks hang there.
    let input = super::editor::with_edit_menu(input, EDITOR).paragraph_styles(super::editor::paragraph_styles(&hangs, gutter));
    let editor = col()
        .key("sheet-scroll")
        .w_full()
        .min_h(90.0)
        .max_h(limit)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_fade(ScrollFade::strength(FADE).full_at(0.5).linear())
        .scroll_margin(FADE)
        .child(el().key("sheet-pad").w_full().py(6.0).child(input));

    // SHEET-030, SHEET-038, SHEET-047: keyboard hints in the footer; icon actions float top-right on hover.
    let key_of = |id: &str| s.keymap.label(id);
    let armed = sh.armed.as_ref().is_some_and(|(_, txt)| *txt == sh.edit.text);
    let (rw, rg, rc) = super::focus_ring(v.p);
    let cancel_lbl = if armed { "Discard" } else if sh.problem.is_some() { "Close" } else { "Cancel" };
    let cancel_tip = format!("{cancel_lbl} · {}", key_of("sheet.cancel"));
    let cancel_tint = armed.then_some((c.readable_err(), c.readable_err()));
    let cancel_btn = mini_button(v.p, "sheet-btn-cancel", &icons::CLOSE, &cancel_tip, cancel_tint)
        .focus_ring(rw, rg, rc)
        .control(|s, tx| s.sheet_escape(tx));
    let actions = match sh.problem {
        Some(_) => {
            let copy_btn = mini_button(v.p, "sheet-btn-copy", &icons::COPY, "Copy text", None)
                .focus_ring(rw, rg, rc)
                .control(|s, tx| s.sheet_copy(tx));
            mini_bar(v.p, "sheet-actions").child(copy_btn).child(cancel_btn)
        }
        None => {
            let save_tip = format!("Save · {}", key_of("sheet.save"));
            let save_tint = dirty.then_some((c.accent, c.accent));
            let mut save_btn = mini_button(v.p, "sheet-btn-save", &icons::CHECK, &save_tip, save_tint)
                .focus_ring(rw, rg, rc)
                .control(|s, tx| s.sheet_save(tx));
            if !dirty {
                save_btn = save_btn.opacity(super::tokens::state::DISABLED_CONTENT);
            }
            mini_bar(v.p, "sheet-actions").child(cancel_btn).child(save_btn)
        }
    };
    let bar_top = (HEADER_H - MINI_BAR_H) / 2.0;
    let zen_s = motion::zen_spring();
    let actions_zone = row()
        .key("sheet-actions-zone")
        .pin(None, Some(0.0), Some(0.0), None)
        .pt(bar_top)
        .pb(bar_top)
        .pl(space::PX_24)
        .pr(space::PX_12)
        .hover_group()
        .hover_motion(zen_s, zen_s)
        .child(actions.group_opacity(if armed || sh.problem.is_some() { 1.0 } else { 0.0 }, 1.0));

    let foot = match sh.problem {
        Some(p) => row()
            .key("sheet-problem")
            .min_h(KEY_ROW)
            .w_full()
            .center_y()
            .child(super::components::error_line(v.p, p).grow(1.0)),
        None => {
            let (cancel, tone) = if armed { ("again to discard your edit", c.text) } else { ("cancels", c.faint) };
            let mut hints = row()
                .key("sheet-keys")
                .min_h(KEY_ROW)
                .w_full()
                .gap(8.0)
                .center_y()
                .child(cap(key_of("sheet.cancel"), v.p))
                .child(text(cancel).key("sheet-cancel").font(sans(11.5, 400.0)).color(tone).truncate().shrink().min_w(0.0));
            if !armed {
                hints = hints
                    .child(el().w(10.0 - 8.0))
                    .child(cap(key_of("sheet.save"), v.p))
                    .child(text("saves").font(sans(11.5, 400.0)).color(c.faint));
            }
            hints
        }
    };

    let header = row()
        .key("sheet-header")
        .w_full()
        .h(HEADER_H)
        .px(PAD_X)
        .center_y()
        .child(title);
    let mut body = col().key("sheet-body").w_full().px(PAD_X).py(BODY_PY).gap(BODY_GAP);
    if let Some(i) = sh.pick.before {
        body = body.child(context("sheet-before", i, true));
    }
    body = body.child(editor);
    if let Some(i) = sh.pick.after {
        body = body.child(context("sheet-after", i, false));
    }
    let footer = row()
        .key("sheet-footer")
        .w_full()
        .min_h(FOOTER_H)
        .px(PAD_X)
        .py(space::PX_12)
        .center_y()
        .child(foot);
    let card = col()
        .key("sheet-card")
        .w(w)
        .child(header)
        .child(body)
        .child(footer);
    let label = if sh.scope == Scope::Section { "Edit the section" } else { "Edit the block" };
    let card = dialog_frame(v.p, "sheet-pop", None)
        .role("dialog")
        .label(label)
        .child(card)
        .child(actions_zone);

    // SHEET-015: the scrim takes every click and wheel, and only moving the window stays live.
    let drag = row().key("sheet-drag").pin(Some(0.0), Some(0.0), Some(0.0), None).h(super::titlebar::BAR_H).window_drag().child(dots(v.p).ml(14.0).mt(super::titlebar::BAR_H / 2.0 - 6.0));
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
