//! The edit sheet (`behavior/edit-sheet.md`, SHEET-015 to SHEET-031) and Board card mini window:
//! one shared modal layer (`Layer::Sheet`) built with `components::MiniCard`.
//!
//! - Standard Edit Sheet (`sh.card.is_none()`): opens directly in editing mode with optional faint
//!   neighbour context blocks and a hover-revealed top-right icon mini-bar (header & footer hover).
//! - Board Card Mini Window (`sh.card.is_some()`): opens on a card click / `Enter` in Preview mode
//!   showing the rendered content under the task node (`ui::preview::render_blocks`), or directly in
//!   Edit mode via `e`/`F2`, sharing the exact same `MiniCard` chrome, editor, and save pipeline.

use rune::{El, Key, KeyEvent, ScrollFade, ScrollSpec, TextHighlight, Tx, col, el, row, text, text_input};

use super::V;
use super::components::{Control, MiniCard, dots, error_line, mini_button, progress_dot};
use super::icons;
use super::layer::Card;
use super::map as ui_map;
use super::preview::{Cx, look_at};
use super::reader::cap;
use super::styles::{self, sans};
use super::tokens::{FocusRing, motion, radius, size, space};
use crate::app::App;
use crate::blocks::Task;
use crate::board;
use crate::keymap::Ctx;
use crate::map_app::{self, Tier};
use crate::page::r;
use crate::roles::Roles;
use crate::sheet::{CardView, EDITOR, Scope, Sheet};
use crate::theme;

const PAD_X: f32 = space::PAD_DIALOG;
const HEADER_H: f32 = 48.0;
const BODY_PY: f32 = space::PX_24;
const BODY_GAP: f32 = space::GAP_SECTION;
const FOOTER_H: f32 = 44.0;
const FADE: f32 = space::SCROLL_FADE_DIALOG;
/// Matches the 20 pt height of the footer's `kbd` key caps.
const KEY_ROW: f32 = 20.0;

/// The edit sheet or Board card mini window, a modal card in the middle of the window (`super::layer`).
pub(super) fn sheet(v: V, sh: &Sheet) -> Card {
    if let Some(card) = sh.card {
        return card_sheet(v, sh, card);
    }
    edit_sheet(v, sh)
}

/// Shared writer-styled editor scroll area used by both standard Edit Sheet and Card Mini Window
/// Edit mode.
fn sheet_editor(v: V, sh: &Sheet, measure: f32, gutter: f32, limit: f32) -> El {
    let s = v.s;
    let c = Roles::new(v.p);
    let style = super::editor::ed_style(v);
    let e = style.size;
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
    let input = super::editor::with_edit_menu(input, EDITOR).paragraph_styles(super::editor::paragraph_styles(&hangs, gutter));
    col()
        .key("sheet-scroll")
        .w_full()
        .min_h(90.0)
        .max_h(limit)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_fade(ScrollFade::strength(FADE).full_at(0.5).linear())
        .scroll_margin(FADE)
        .child(el().key("sheet-pad").w_full().py(space::PX_6).child(input))
}

/// Shared edit-mode footer (problem error line or Esc / Save key hints with right-aligned Modified state).
fn edit_footer(v: V, sh: &Sheet, dirty: bool, armed: bool, return_to_preview: bool) -> El {
    let c = Roles::new(v.p);
    let key_of = |id: &str| v.s.keymap.label(id);
    match sh.problem {
        Some(p) => row()
            .key("sheet-problem")
            .min_h(KEY_ROW)
            .w_full()
            .center_y()
            .child(error_line(v.p, p).grow(1.0)),
        None => {
            let (cancel, tone) = if armed {
                ("again to discard your edit", c.text)
            } else if return_to_preview {
                ("returns to preview", c.faint)
            } else {
                ("cancels", c.faint)
            };
            let mut hints = row()
                .key("sheet-keys")
                .min_h(KEY_ROW)
                .w_full()
                .gap(space::PX_8)
                .center_y()
                .child(cap(key_of("sheet.cancel"), v.p))
                .child(text(cancel).key("sheet-cancel").font(styles::small()).color(tone).truncate().shrink().min_w(0.0));
            if !armed {
                hints = hints
                    .child(el().w(space::PX_2))
                    .child(cap(key_of("sheet.save"), v.p))
                    .child(text("saves").font(styles::small()).color(c.faint));
            }
            if dirty {
                hints = hints
                    .child(el().grow(1.0))
                    .child(
                        text("Modified")
                            .key("sheet-dirty")
                            .font(styles::sans(styles::scale::SMALL, 500.0))
                            .color(c.accent),
                    );
            }
            hints
        }
    }
}

/// Shared icon-only mini-bar actions for edit mode: `(leading, close)` so `×` is always rightmost
/// (`[✓ Save] [× Cancel/Discard]` normally, `[Copy] [× Close]` on conflict).
fn edit_actions(v: V, sh: &Sheet, dirty: bool, armed: bool) -> (El, El) {
    let c = Roles::new(v.p);
    let key_of = |id: &str| v.s.keymap.label(id);
    let (rw, rg, rc) = super::focus_ring(v.p);
    let cancel_lbl = if armed {
        "Discard"
    } else if sh.problem.is_some() {
        "Close"
    } else {
        "Cancel"
    };
    let cancel_tip = format!("{cancel_lbl} · {}", key_of("sheet.cancel"));
    let cancel_tint = armed.then_some((c.readable_err(), c.readable_err()));
    let close_btn = mini_button(v.p, "sheet-btn-cancel", &icons::CLOSE, &cancel_tip, cancel_tint)
        .focus_ring(rw, rg, rc)
        .control(|s, tx| s.sheet_escape(tx));
    let leading_btn = match sh.problem {
        Some(_) => mini_button(v.p, "sheet-btn-copy", &icons::COPY, "Copy text", None)
            .focus_ring(rw, rg, rc)
            .control(|s, tx| s.sheet_copy(tx)),
        None => {
            let save_tip = format!("Save · {}", key_of("sheet.save"));
            let save_tint = dirty.then_some((c.accent, c.accent));
            let mut save_btn = mini_button(v.p, "sheet-btn-save", &icons::CHECK, &save_tip, save_tint)
                .focus_ring(rw, rg, rc)
                .control(|s, tx| s.sheet_save(tx));
            if !dirty {
                save_btn = save_btn.opacity(super::tokens::state::DISABLED_CONTENT);
            }
            save_btn
        }
    };
    (leading_btn, close_btn)
}

/// Board Card Mini Window: Preview or Edit mode built on the shared `MiniCard` component with
/// hover-revealed top-right icon actions (`[✎] [×]` in Preview, `[✓] [×]` in Edit).
fn card_sheet(v: V, sh: &Sheet, card: CardView) -> Card {
    let s = v.s;
    let c = Roles::new(v.p);
    let style = super::editor::ed_style(v);
    let e = style.size;
    let gutter = super::editor::hang_gutter(e);
    let colw = super::preview::column(v).width;
    let w = (v.vw - space::DIALOG_MARGIN_BOTH).min((colw + 2.0 * gutter + 2.0 * PAD_X).min(660.0)).max(360.0);
    let inner = (w - 2.0 * PAD_X).max(120.0);
    let avg = crate::page::avg_char(theme::editor_font(&s.settings().font).family);
    let measure = (inner - 2.0 * gutter).min(r(avg * e * 66.0)).max(0.0);

    let doc = s.doc();
    let tree = s.map_tree();
    let model = board::build(&tree);
    let node_idx = tree.find(card.node);
    let done = node_idx.and_then(|i| board::task_state(&tree, i)) == Some(Task::Done);
    let col_info = node_idx
        .and_then(|i| model.find_card_column(i))
        .and_then(|ci| model.columns.get(ci))
        .map(|col| {
            let hn = &tree.nodes[col.heading];
            (hn.label.clone(), ui_map::branch_color(&c, hn.branch))
        });
    let subtask_dots = node_idx
        .and_then(|i| model.card(i))
        .map(|bc| bc.subtask_dots(&tree))
        .unwrap_or_default();

    let dirty = sh.edit.text != sh.orig;
    let armed = sh.armed.as_ref().is_some_and(|(_, txt)| *txt == sh.edit.text);
    let reduced = crate::motion::reduced();
    let hover_anim = if reduced { rune::Anim::INSTANT } else { motion::hover_fade() };

    let look = look_at(v, inner);
    let cx = Cx::map(v, &look, None);
    let title_st = map_app::style(Tier::Head, look.b, look.family, look.mono);
    let title_ink = if done { c.readable_dim() } else { c.strong };
    let title_el = if let Some(i) = node_idx {
        ui_map::label(&cx, &tree.nodes[i], title_st, title_ink)
            .el(title_st, title_ink, None)
            .wrap_anywhere()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
    } else {
        text(crate::md::base(&sh.rel).to_string())
            .font(styles::title())
            .color(c.strong)
            .truncate()
            .grow(1.0)
            .shrink()
            .min_w(0.0)
    };

    let nid = card.node;
    let status_col = if done {
        Roles::readable_on(c.accent, c.bg, 3.1)
    } else {
        c.readable_dim()
    };
    let status_lbl = if done { "Done" } else { "Open" };
    let mut status_pill = row()
        .key("card-sheet-task")
        .h(size::CONTROL_BAR)
        .px(space::PX_6)
        .gap(space::PX_6)
        .center_y()
        .radius(radius::CONTROL)
        .role("checkbox")
        .checked(done)
        .label(if done { "Done, toggle task" } else { "Open, toggle task" })
        .child(progress_dot(status_col, done))
        .child(text(status_lbl).font(styles::small()).color(status_col));
    if !card.editing {
        status_pill = FocusRing::Tight.apply(
            status_pill
                .cursor(rune::Cursor::Pointer)
                .hover(c.hover)
                .hover_motion(hover_anim, hover_anim)
                .help("Toggle task · x")
                .control(move |s: &mut App, tx| s.board_toggle_task(nid, tx)),
            v.p,
        );
    } else {
        status_pill = status_pill.disabled(true).opacity(super::tokens::state::DISABLED_CONTENT);
    }

    // Hover-revealed top-right icon mini-bar (never text buttons or always-visible close `×`).
    let (rw, rg, rc) = super::focus_ring(v.p);
    let (leading_btn, close_btn) = if !card.editing {
        let edit_btn = mini_button(v.p, "card-sheet-edit", &icons::PENCIL, "Edit card · e", None)
            .focus_ring(rw, rg, rc)
            .control(|s: &mut App, tx| s.sheet_enter_edit(tx));
        let close_tip = format!("Close · {}", s.keymap.label("sheet.cancel"));
        let close_btn = mini_button(v.p, "sheet-btn-cancel", &icons::CLOSE, &close_tip, None)
            .focus_ring(rw, rg, rc)
            .control(|s: &mut App, tx| s.close_layer(tx));
        (edit_btn, close_btn)
    } else {
        edit_actions(v, sh, dirty, armed)
    };

    // Uniform spacing across column title, subtask dots, and status pill,
    // reserving right room for the top-right 2-icon mini-bar.
    let mut meta = row()
        .key("card-sheet-meta")
        .w_full()
        .h(size::CONTROL_BAR)
        .gap(space::PX_10)
        .pr(MiniCard::actions_reserve(2))
        .center_y();
    if let Some((col_name, branch_col)) = col_info {
        let dot_col = Roles::readable_on(branch_col, c.bg, 3.1);
        meta = meta.child(
            row()
                .gap(space::PX_6)
                .center_y()
                .shrink()
                .min_w(0.0)
                .child(el().size(3.0, 12.0).radius(1.5).fill(dot_col))
                .child(text(col_name).font(styles::small()).color(c.readable_dim()).truncate()),
        );
        if !subtask_dots.is_empty() {
            let done_n = subtask_dots.iter().filter(|&&d| d).count();
            let mut dots_row = row()
                .key("card-sheet-dots")
                .gap(space::GAP_TIGHT)
                .center_y()
                .label(format!("{done_n} of {} subtasks done", subtask_dots.len()));
            for d in subtask_dots {
                dots_row = dots_row.child(progress_dot(dot_col, d));
            }
            meta = meta.child(dots_row);
        }
    }
    meta = meta.child(status_pill);

    // Hidden keyboard focus anchor in Preview mode: `no_focus_ring` prevents the corner square
    // outline artefact when opened via keyboard (`Enter`).
    let focus_anchor = el()
        .key("card-sheet-focus")
        .pin(Some(0.0), Some(0.0), None, None)
        .size(1.0, 1.0)
        .focusable(true)
        .tab_index(-1)
        .no_focus_ring();

    let header = col()
        .key("card-sheet-header")
        .w_full()
        .px(PAD_X)
        .pt(space::PX_16)
        .pb(space::PX_8)
        .gap(space::PX_12)
        .child(meta)
        .children((!card.editing).then_some(title_el));

    // Body: switches smoothly with `ui::swap()` between Preview (`ui::preview::render_blocks`)
    // and Edit (`sheet_editor`), using `edit_sheet`'s open, divider-free breathing room.
    let chrome = 96.0 + FOOTER_H + 2.0 * space::PX_16;
    let limit = (r(0.72 * v.vh) - chrome).max(120.0);
    let body = if !card.editing {
        let blocks = node_idx
            .and_then(|i| super::preview::item_content_blocks(&doc.blocks, tree.nodes[i].own.start))
            .unwrap_or(&[]);
        let content = if blocks.is_empty() {
            el().key("card-sheet-empty")
                .w_full()
                .py(space::PX_8)
                .child(
                    text("No details under this card yet. Press e to add notes or subtasks.")
                        .font(styles::desc())
                        .color(c.readable_dim())
                        .wrap(),
                )
        } else {
            super::preview::render_blocks(v, &doc, blocks, inner)
        };
        col()
            .key("card-sheet-preview")
            .w_full()
            .min_h(100.0)
            .max_h(limit)
            .px(PAD_X)
            .pt(space::PX_8)
            .pb(space::PX_16)
            .scroll(ScrollSpec { stick_bottom: false })
            .scroll_fade(ScrollFade::strength(FADE).full_at(0.5).linear())
            .scroll_margin(FADE)
            .transition(super::swap())
            .child(content)
    } else {
        col()
            .key("card-sheet-editor")
            .w_full()
            .px(PAD_X)
            .py(space::PX_12)
            .transition(super::swap())
            .child(sheet_editor(v, sh, measure, gutter, limit))
    };

    let foot = if !card.editing {
        row()
            .key("card-sheet-keys")
            .min_h(KEY_ROW)
            .w_full()
            .gap(space::PX_8)
            .center_y()
            .child(cap("e".into(), v.p))
            .child(text("edits").font(styles::small()).color(c.faint))
            .child(el().w(space::PX_2))
            .child(cap("x".into(), v.p))
            .child(text("toggles task").font(styles::small()).color(c.faint))
            .child(el().w(space::PX_2))
            .child(cap(s.keymap.label("sheet.cancel"), v.p))
            .child(text("closes").font(styles::small()).color(c.faint))
    } else {
        edit_footer(v, sh, dirty, armed, card.return_to_preview)
    };

    let footer = row()
        .key("sheet-footer")
        .w_full()
        .min_h(FOOTER_H)
        .px(PAD_X)
        .py(space::PX_12)
        .center_y()
        .child(foot);

    let aria = node_idx.map_or_else(|| "Card details".to_string(), |i| tree.nodes[i].label.clone());
    let frame = MiniCard::new(v.p, "sheet-pop", "sheet-chrome", aria)
        .card_key("card-sheet")
        .width(w)
        .bar_inset(space::PX_16 - 2.0, space::PX_12)
        .pin_actions(card.editing && (armed || sh.problem.is_some()))
        .anchor(focus_anchor)
        .action(leading_btn)
        .close(close_btn)
        .build(header, body, footer);

    let drag = row()
        .key("sheet-drag")
        .pin(Some(0.0), Some(0.0), Some(0.0), None)
        .h(super::titlebar::BAR_H)
        .window_drag()
        .child(dots(v.p).ml(14.0).mt(super::titlebar::BAR_H / 2.0 - 6.0));
    Card { el: frame, plane: Some(col().center().child(drag)) }
}

/// Standard Edit Sheet over the reader, map, or board column heading (`SHEET-015` to `SHEET-031`).
fn edit_sheet(v: V, sh: &Sheet) -> Card {
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
    let title = row()
        .key("sheet-title")
        .gap(space::PX_8)
        .pr(MiniCard::actions_reserve(2))
        .center_y()
        .child(text(crate::md::base(&sh.rel).to_string()).font(sans(12.5, 600.0)).color(c.text).truncate().shrink().min_w(0.0))
        .child(text(span).key("sheet-span").font(sans(12.5, 400.0)).color(c.faint));

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
    let editor = sheet_editor(v, sh, measure, gutter, limit);

    // SHEET-030, SHEET-038, SHEET-047: keyboard hints in the footer; icon mini-bar floats
    // top-right on header or footer hover (`[✓ Save] [× Cancel/Discard]`, `×` rightmost).
    let armed = sh.armed.as_ref().is_some_and(|(_, txt)| *txt == sh.edit.text);
    let (leading_btn, close_btn) = edit_actions(v, sh, dirty, armed);
    let foot = edit_footer(v, sh, dirty, armed, false);

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

    let label = if sh.scope == Scope::Section { "Edit the section" } else { "Edit the block" };
    let card = MiniCard::new(v.p, "sheet-pop", "sheet-chrome", label)
        .card_key("sheet-card")
        .width(w)
        .pin_actions(armed || sh.problem.is_some())
        .action(leading_btn)
        .close(close_btn)
        .build(header, body, footer);

    // SHEET-015: the scrim takes every click and wheel, and only moving the window stays live.
    let drag = row()
        .key("sheet-drag")
        .pin(Some(0.0), Some(0.0), Some(0.0), None)
        .h(super::titlebar::BAR_H)
        .window_drag()
        .child(dots(v.p).ml(14.0).mt(super::titlebar::BAR_H / 2.0 - 6.0));
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
