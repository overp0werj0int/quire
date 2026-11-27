//! Board mode view (`docs/reader-writer/behavior/board.md`, decision 0015):
//! columns of cards laid out by Rune flow layout, reusing the reader's inline runs,
//! the map's block cards, and the design system's components and tokens.

use rune::{Color, Drag, El, ScrollSpec, Tx, WheelEvent, col, el, row, text};

use super::components::{checkbox_box, count_pill, faded_h};
use super::map as ui_map;
use super::preview::{Cx, Look, look_at};
use super::tokens::{radius, space};
use super::{V, styles};
use crate::app::App;
use crate::blocks::Task;
use crate::board::{self, Column, ColumnHead};
use crate::board_app::{card_key, col_key, head_key};
use crate::map::{Body, NodeKind, Tree};
use crate::map_app::{self, Tier};
use crate::page;
use crate::roles::Roles;

/// Column radius in points (BRD-008).
const COL_RADIUS: f32 = 10.0;
/// Column inner padding (BRD-008).
const COL_PAD: f32 = space::PX_8;
/// Gap between cards inside a column (BRD-010).
const CARD_GAP: f32 = space::PX_6;
/// Card padding: 8 px top/bottom, 10 px left/right (BRD-010).
const CARD_PY: f32 = space::PX_8;
const CARD_PX: f32 = space::PX_10;
/// Branch bar in column header: 3 × 14 px (BRD-009).
const BAR_W: f32 = 3.0;
const BAR_H: f32 = 14.0;
/// Scroll-edge fade height inside columns (matching Edit Sheet / panels).
const COL_FADE: f32 = space::SCROLL_FADE_PANEL;

/// Board's page look at `width`, with `dim` lifted to `readable_dim` as Map mode does.
fn board_look(v: V, width: f32) -> Look {
    let mut look = look_at(v, width);
    look.c.dim = look.c.readable_dim();
    let bg_card = look.c.on_page(look.c.card);
    let panel = look.c.on_page(look.c.panel);
    let mut faint = look.c.faint;
    for surf in [look.c.bg, bg_card, panel] {
        faint = Roles::readable_on(faint, surf, 4.8);
    }
    look.c.faint = faint;
    look
}

/// Builds the full Board viewport filling the note area (BRD-007 to BRD-013).
pub(super) fn board(v: V) -> El {
    let s = v.s;
    let b = s.settings().size as f32;
    let col_w = page::r(b * 20.0);
    let col_gap = page::r(b * 12.0 / 14.0);
    let outer_pad = page::r(b * 16.0 / 14.0);
    let card_inner_w = (col_w - 2.0 * COL_PAD - 2.0 * CARD_PX - 2.0).max(60.0);
    let look = board_look(v, card_inner_w);
    let cx = Cx::map(v, &look, None);
    let c = &look.c;
    let (t, model) = s.board_model();
    let sel = s.board_selected();
    let holding = s.board.hold.active();

    let mut cols_row = row()
        .key("board-row")
        .items_start()
        .gap(col_gap)
        .padding(outer_pad)
        .h_full();

    for col_data in &model.columns {
        cols_row = cols_row.child(column_el(v, &cx, &t, col_data, col_w, sel, holding));
    }

    if !model.has_headings() {
        cols_row = cols_row.child(empty_hint(v, outer_pad));
    }

    let scroll = el()
        .key("board-scroll")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .h_full()
        .scroll_x()
        .on_scroll_offset({
            let sx = s.board.scroll_x.clone();
            move |at| sx.set(at.offset[0])
        })
        .on_wheel(|s: &mut App, e: WheelEvent, tx: &mut Tx| {
            // Shift+wheel scrolls the board row horizontally (BRD-007).
            if e.mods.shift && e.dx == 0.0 && e.dy != 0.0 {
                let next = (s.board.scroll_x.get() - e.dy).max(0.0);
                s.board.scroll_x.set(next);
                tx.scroll_to_instant("board-scroll", next, 0.0);
                return true;
            }
            false
        })
        .child(cols_row);

    el().key("board-canvas")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .h_full()
        .fill(c.bg)
        .clip_soft(0.0)
        .transition(if s.mode_swapped { super::swap() } else { rune::Transition::none() })
        .child_exit(rune::Transition::none())
        .role("region")
        .label("Board")
        .on_layout(|s: &mut App, r| s.board_view_laid_out(r))
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_pan_drag(d, tx))
        .child(scroll)
}

/// Quiet empty-board hint when a note has no column headings (BRD-006).
fn empty_hint(v: V, pad: f32) -> El {
    el().key("board-empty-hint")
        .pt(pad * 0.5)
        .px(space::PX_8)
        .child(
            text("Headings become columns. Add one in Source to start a board.")
                .font(styles::sans(styles::CAPTION, 500.0))
                .color(styles::caption_ink(v.p)),
        )
}

/// One column: header row + scrollable card stack capped to the board height (BRD-008, BRD-009).
fn column_el(v: V, cx: &Cx, t: &Tree, col_data: &Column, col_w: f32, sel: Option<u64>, holding: bool) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let key_id = col_data.head.key_id(t);
    let c_key = col_key(col_data.head, t);
    let h_key = head_key(col_data.head, t);
    let head_selected = match col_data.head {
        ColumnHead::Heading(h) => sel == Some(t.nodes[h].id),
        ColumnHead::Loose => false,
    };

    let header = column_header(v, cx, t, col_data, &h_key, head_selected, holding, key_id);

    let mut cards_stack = col()
        .key(format!("board-cards:{key_id}"))
        .w_full()
        .gap(CARD_GAP)
        .pt(space::PX_6);

    for &ci in &col_data.cards {
        let cid = t.nodes[ci].id;
        let selected = sel == Some(cid);
        cards_stack = cards_stack.child(card_el(v, cx, t, ci, selected, holding));
    }

    let faded_cards = faded_h(&format!("board-col-{key_id}"), c.on_page(c.panel), COL_FADE, cards_stack);
    let cards_scroll = el()
        .key(format!("board-col-scroll:{key_id}"))
        .w_full()
        .shrink()
        .min_h(0.0)
        .scroll(ScrollSpec::default())
        .on_scroll_offset({
            let map = v.s.board.col_scroll.clone();
            move |at| {
                map.borrow_mut().insert(key_id, at.offset[1]);
            }
        })
        .on_layout(move |s: &mut App, r| s.board_col_view_laid_out(key_id, r))
        .child(faded_cards);

    // Column is as tall as its cards and never taller than the viewport area (BRD-008).
    let outer_pad = page::r(b * 16.0 / 14.0);
    let avail_h = (v.s.board.layout.borrow().view[3] - 2.0 * outer_pad).max(160.0);

    col()
        .key(c_key)
        .w(col_w)
        .max_h(avail_h)
        .padding(COL_PAD)
        .radius(COL_RADIUS)
        .fill(c.panel)
        .on_layout(move |s: &mut App, r| s.board_col_laid_out(key_id, r))
        .child(header)
        .child(cards_scroll)
}

/// Column header row (BRD-009): 3 × 14 px branch colour bar (omitted on Loose), bold `B` title
/// truncated with ellipsis, and right-aligned `count_pill`.
fn column_header(v: V, cx: &Cx, t: &Tree, col_data: &Column, h_key: &str, selected: bool, holding: bool, key_id: u64) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let st = map_app::style(Tier::Head, b, cx.look.family, cx.look.mono);
    let mut head_row = row()
        .key(h_key)
        .w_full()
        .px(space::PX_4)
        .py(space::PX_4)
        .gap(space::PX_8)
        .center_y()
        .radius(radius::CONTROL)
        .focus_selected(selected)
        .on_layout(move |s: &mut App, r| s.board_head_laid_out(key_id, r));

    match col_data.head {
        ColumnHead::Heading(h) => {
            let n = &t.nodes[h];
            let id = n.id;
            let bar_col = ui_map::branch_color(c, n.branch);
            let bar = el().key(format!("board-bar:{id}")).size(BAR_W, BAR_H).radius(1.5).fill(bar_col);
            let rich = ui_map::label(cx, n, st, c.strong);
            let title = rich.el(st, c.strong, None).truncate().grow(1.0).shrink().min_w(0.0);
            head_row = head_row
                .role("button")
                .label(n.label.clone())
                .cursor(rune::Cursor::Pointer)
                .child(bar)
                .child(title)
                .child(count_pill(v.p, format!("board-count:{id}"), col_data.cards.len()))
                .on_press_tx(move |s: &mut App, tx| s.board_click(id, false, tx))
                .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, true, tx));
            if !holding {
                head_row = head_row.hover(c.hover);
            }
            if selected {
                head_row = apply_selection_glass(head_row, c);
            }
        }
        ColumnHead::Loose => {
            let root_label = if t.root().label.is_empty() { "Loose".to_string() } else { t.root().label.clone() };
            let title = text(root_label)
                .font(st)
                .color(c.dim)
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0);
            head_row = head_row
                .child(title)
                .child(count_pill(v.p, "board-count:loose", col_data.cards.len()));
        }
    }
    head_row
}

/// One card on the board (BRD-010, BRD-011, BRD-012). Keyed by `NodeId`.
fn card_el(v: V, cx: &Cx, t: &Tree, i: usize, selected: bool, holding: bool) -> El {
    let c = &cx.look.c;
    let n = &t.nodes[i];
    let id = n.id;
    let key = card_key(id);
    let inner_w = cx.look.width;

    let body = card_body(v, cx, t, i, inner_w);
    let mut card = el()
        .key(key)
        .w_full()
        .py(CARD_PY)
        .px(CARD_PX)
        .radius(radius::FIELD)
        .fill(c.card)
        .border(1.0, c.hairline)
        .border_layout()
        .role("listitem")
        .label(n.label.clone())
        .focus_selected(selected)
        .cursor(rune::Cursor::Pointer)
        .on_layout(move |s: &mut App, r| s.board_card_laid_out(id, r))
        .on_press_tx(move |s: &mut App, tx| s.board_click(id, false, tx))
        .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, true, tx))
        .child(body);

    if !holding {
        card = card.hover(c.hover);
    }
    if selected {
        card = apply_selection_glass(card, c);
    }
    card
}

/// Renders a card's inner content: section card, block card, task card, or inline text + child
/// summary caption (BRD-004, BRD-010, BRD-011).
fn card_body(v: V, cx: &Cx, t: &Tree, i: usize, inner_w: f32) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let n = &t.nodes[i];
    let id = n.id;

    // Section card: heading under a column (BRD-004).
    if matches!(n.kind, NodeKind::Heading { .. }) {
        let st = map_app::style(Tier::Head, b, cx.look.family, cx.look.mono);
        let rich = ui_map::label(cx, n, st, c.strong);
        let cap_font = map_app::chip_font(b, cx.look.mono);
        return col()
            .w_full()
            .gap(space::PX_4)
            .child(rich.el(st, c.strong, None).wrap_anywhere().w_full())
            .child(text(board::section_summary(t, i)).font(cap_font).color(c.dim));
    }

    // Block card: code, diff, table, quote, alert, HTML, image, gallery (BRD-010).
    if let Body::Card(bl) = &n.body
        && let Some((card_el, _)) = ui_map::block_card(cx, bl, i, &format!("b{id}"))
    {
        return card_el;
    }

    let st = map_app::style(Tier::Body, b, cx.look.family, cx.look.mono);
    let task = match n.kind {
        NodeKind::Item { task, .. } if task != Task::None => Some(task),
        _ => None,
    };
    let ink = if task == Some(Task::Done) { c.dim } else { c.text };
    let rich = ui_map::label(cx, n, st, ink);
    let label_el = rich.el(st, ink, None).wrap_anywhere().grow(1.0).shrink().min_w(0.0);

    let top = if let Some(task) = task {
        let checked = task == Task::Done;
        let box_top = ((st.line_height - space::PX_16) / 2.0).max(0.0);
        let toggle = el()
            .key(format!("board-task:{id}"))
            .pt(box_top)
            .cursor(rune::Cursor::Pointer)
            .role("checkbox")
            .checked(checked)
            .label(if checked { "Checked" } else { "Not checked" })
            .on_press_tx(move |s: &mut App, tx| s.board_toggle_task(i, tx))
            .child(checkbox_box(v.p, checked));
        row().w_full().items_start().gap(space::PX_8).child(toggle).child(label_el)
    } else {
        el().w(inner_w).w_full().child(label_el)
    };

    if let Some(summary) = board::child_summary(t, i) {
        let cap_font = map_app::chip_font(b, cx.look.mono);
        col()
            .w_full()
            .gap(space::PX_4)
            .child(top)
            .child(text(summary).key(format!("board-sub:{id}")).font(cap_font).color(c.dim))
    } else {
        top
    }
}

/// Focus glass selection treatment matching `Cx::focus_glass` and Map's selection rim (BRD-012).
fn apply_selection_glass(e: El, c: &Roles) -> El {
    e.gradient(c.text.alpha(0.07), Color::TRANSPARENT)
        .border_gradient(1.5, c.text.alpha(0.55), c.text.alpha(0.12))
        .box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 10.0, spread: 0.0, color: c.text.alpha(0.08), inset: true })
}
