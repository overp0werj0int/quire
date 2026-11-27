//! Board mode view (`docs/reader-writer/behavior/board.md`, decision 0015):
//! columns of cards laid out by Rune flow layout, reusing the reader's inline runs,
//! the map's block cards, and the design system's components and tokens.

use rune::{Color, Drag, El, ScrollSpec, Spring, TextHighlight, Tx, WheelEvent, col, el, row, text};

use super::components::{checkbox_box, count_pill, faded_h};
use super::map as ui_map;
use super::preview::{Cx, Hl, Look, look_at};
use super::tokens::{Elevation, radius, space};
use super::{V, styles};
use crate::app::App;
use crate::blocks::Task;
use crate::board::{self, Column, ColumnHead};
use crate::board_app::{card_key, col_key, head_key};
use crate::board_drag::{Held, Hold, SETTLE_SECS, TILT_SPRING, ease_out};
use crate::map::{Body, NodeId, NodeKind, Tree};
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
/// Card layout glide duration (180 ms, BRD-016).
const LAYOUT_GLIDE: f32 = 0.18;

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

/// Active Board Find query and current match, for painting match highlights (BRD-030).
#[derive(Clone, Copy)]
struct BoardFind<'a> {
    q: &'a str,
    cur: Option<crate::map_find::Hit>,
}

fn find_marks(c: &Roles, text: &str, f: BoardFind, node: usize) -> Vec<TextHighlight> {
    let cur_k = f.cur.filter(|h| h.node == node && h.pid.is_none()).map(|h| h.k);
    crate::writer_app::find_in(text, f.q)
        .into_iter()
        .enumerate()
        .map(|(k, r)| {
            if Some(k) == cur_k {
                TextHighlight::new(r, c.find_current).color(c.find_current_text).key(crate::reader::FIND_CUR)
            } else {
                TextHighlight::new(r, c.find)
            }
        })
        .collect()
}

/// Builds the full Board viewport filling the note area (BRD-007 to BRD-023).
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
    let find = s.find_query().map(|q| BoardFind { q, cur: s.board_hits().get(s.reader.find.cur).copied() });
    let hold = &s.board.hold;
    let holding = hold.active();
    let held_card = hold.held_card();
    let settling_card = hold.settling_card();
    let held_col = hold.held_column();
    let settling_col = hold.settling_column();
    let slot = hold.slot();
    let col_drag_slot = hold.col_slot();
    let slot_h = match hold {
        Hold::Held { size, .. } => size[1],
        _ => 40.0,
    };
    let reduced = crate::motion::reduced();
    let row_glide = if reduced { Spring::NONE } else { Spring::new(LAYOUT_GLIDE, 0.0) };

    let mut cols_row = row()
        .key("board-row")
        .items_start()
        .gap(col_gap)
        .padding(outer_pad)
        .h_full()
        .motion(row_glide)
        .child_exit(rune::Transition::none())
        .on_layout(|s: &mut App, r| s.board_row_laid_out(r[2]));

    let mut col_slot_placed = false;
    let mut heading_k = 0usize;
    for (ci, col_data) in model.columns.iter().enumerate() {
        if let ColumnHead::Heading(h) = col_data.head {
            let hid = t.nodes[h].id;
            if held_col == Some(hid) {
                // Keep the column header's drag host alive at the exact same tree path while
                // pinned out of flow so Rune keeps the drag capture and the column's old place closes.
                cols_row = cols_row.child(
                    el().key(format!("board-col-host:{hid}"))
                        .pin(Some(0.0), Some(0.0), None, None)
                        .size(0.0, 0.0)
                        .child(
                            el().key(format!("board-head-host:{hid}"))
                                .size(0.0, 0.0)
                                .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_col_drag(hid, d, tx)),
                        ),
                );
                continue;
            }
            if col_drag_slot == Some(heading_k) {
                cols_row = cols_row.child(col_slot_el(c, col_w, slot_h));
                col_slot_placed = true;
            }
            heading_k += 1;
        }
        let col_slot = slot.filter(|sl| sl.column == ci).map(|sl| (sl.index, slot_h));
        let col_settling = matches!(col_data.head, ColumnHead::Heading(h) if settling_col == Some(t.nodes[h].id));
        cols_row = cols_row.child(column_el(
            v,
            &cx,
            &t,
            col_data,
            col_w,
            sel,
            holding,
            held_card,
            settling_card,
            col_settling,
            col_slot,
            find,
        ));
    }
    if col_drag_slot.is_some() && !col_slot_placed {
        cols_row = cols_row.child(col_slot_el(c, col_w, slot_h));
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

    let overlay = hold_overlay(v, &cx, &t, &model, hold);

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
        .children(overlay)
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
#[allow(clippy::too_many_arguments)]
fn column_el(
    v: V,
    cx: &Cx,
    t: &Tree,
    col_data: &Column,
    col_w: f32,
    sel: Option<NodeId>,
    holding: bool,
    held_card: Option<NodeId>,
    settling_card: Option<NodeId>,
    col_settling: bool,
    col_slot: Option<(usize, f32)>,
    find: Option<BoardFind>,
) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let key_id = col_data.head.key_id(t);
    let c_key = if col_settling { format!("board-col-seat:{key_id}") } else { col_key(col_data.head, t) };
    let h_key = head_key(col_data.head, t);
    let head_selected = match col_data.head {
        ColumnHead::Heading(h) => sel == Some(t.nodes[h].id) && !col_settling,
        ColumnHead::Loose => false,
    };

    let header = column_header(v, cx, t, col_data, &h_key, head_selected, holding, key_id, find);

    let reduced = crate::motion::reduced();
    let glide = if reduced { Spring::NONE } else { Spring::new(LAYOUT_GLIDE, 0.0) };
    let mut cards_stack = col()
        .key(format!("board-cards:{key_id}"))
        .w_full()
        .gap(CARD_GAP)
        .pt(space::PX_6)
        .motion(glide)
        .child_exit(rune::Transition::none())
        .on_layout(move |s: &mut App, r| s.board_col_content_laid_out(key_id, r[3]));

    let mut slot_placed = false;
    let mut rest_k = 0usize;
    for &ci in &col_data.cards {
        let cid = t.nodes[ci].id;
        if held_card == Some(cid) {
            // Keep the drag-capturing host alive at the exact same tree path while pinned out of
            // flow so Rune keeps the active drag capture and the card's old place closes cleanly.
            cards_stack = cards_stack.child(
                el().key(format!("board-card-host:{cid}"))
                    .pin(Some(0.0), Some(0.0), None, None)
                    .size(0.0, 0.0)
                    .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_card_drag(cid, d, tx)),
            );
            continue;
        }
        if let Some((slot_idx, slot_h)) = col_slot
            && rest_k == slot_idx
        {
            cards_stack = cards_stack.child(slot_el(c, slot_h));
            slot_placed = true;
        }
        let selected = sel == Some(cid);
        let settling = settling_card == Some(cid);
        cards_stack = cards_stack.child(card_el(v, cx, t, ci, selected, holding, settling, find));
        rest_k += 1;
    }
    if let Some((_, slot_h)) = col_slot
        && !slot_placed
    {
        cards_stack = cards_stack.child(slot_el(c, slot_h));
    }

    let faded_cards = faded_h(&format!("board-col-{key_id}"), c.on_page(c.panel), COL_FADE, cards_stack);
    let cards_scroll = col()
        .key(format!("board-col-scroll:{key_id}"))
        .w_full()
        .shrink()
        .min_h(0.0)
        .scroll(ScrollSpec::default())
        .scroll_margin(COL_FADE)
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

    let host_key = match col_data.head {
        ColumnHead::Loose => "board-col-host:loose".to_string(),
        ColumnHead::Heading(_) => format!("board-col-host:{key_id}"),
    };
    let bounds_marker = el()
        .key(c_key)
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .inert()
        .on_layout(move |s: &mut App, r| s.board_col_laid_out(key_id, r));

    let mut column = col()
        .key(host_key)
        .w(col_w)
        .max_h(avail_h)
        .padding(COL_PAD)
        .radius(COL_RADIUS)
        .fill(c.panel)
        .child(bounds_marker)
        .child(header)
        .child(cards_scroll);

    if col_settling {
        column = column.opacity(0.0);
    }
    column
}

/// The dashed accent drop slot that opens where a dropped card would land (BRD-016).
fn slot_el(c: &Roles, h: f32) -> El {
    el().key("board-slot")
        .w_full()
        .h(h.max(24.0))
        .radius(radius::FIELD)
        .fill(c.accent.alpha(0.10))
        .border(1.5, c.accent.alpha(0.55))
        .border_dash(6.0, 4.0)
        .inert()
}

/// The dashed accent column-wide slot that opens between columns during a column drag (BRD-021).
fn col_slot_el(c: &Roles, w: f32, h: f32) -> El {
    el().key("board-col-slot")
        .size(w, h.max(80.0))
        .radius(COL_RADIUS)
        .fill(c.accent.alpha(0.10))
        .border(1.5, c.accent.alpha(0.55))
        .border_dash(6.0, 4.0)
        .inert()
}

/// Overlay for the lifted/settling card or column (BRD-014, BRD-015, BRD-017, BRD-019, BRD-021, BRD-022).
fn hold_overlay(v: V, cx: &Cx, t: &Tree, model: &board::Board, hold: &Hold) -> Option<El> {
    let c = &cx.look.c;
    let reduced = crate::motion::reduced();
    match *hold {
        Hold::Held { what: Held::Card(id), grab, at, size, refused, lean, .. } => {
            let i = t.find(id)?;
            let pos = [at[0] - grab[0], at[1] - grab[1]];
            let ox = (grab[0] / size[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / size[1].max(1.0)).clamp(0.0, 1.0);
            let body = card_body(v, cx, t, i, cx.look.width, None);
            let mut card = el()
                .key(card_key(id))
                .size(size[0], size[1])
                .py(CARD_PY)
                .px(CARD_PX)
                .radius(radius::FIELD)
                .fill(c.on_page(c.card))
                .border(1.5, c.accent)
                .border_layout()
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(if reduced { Spring::NONE } else { Spring::new(TILT_SPRING, 0.0) })
                .description(format!("tilt={lean:.4}"))
                .role("listitem")
                .label(t.nodes[i].label.clone())
                .focus_selected(true)
                .child(body);
            card = Elevation::Dialog.apply(card, v.p, 1.0);
            if let Some(reason) = refused {
                let wash = el()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .radius(radius::FIELD)
                    .fill(c.panel.alpha(0.55))
                    .inert();
                card = card.child(wash);
                let cue = text(reason)
                    .key("board-refusal")
                    .font(styles::sans(styles::CAPTION, 500.0))
                    .color(styles::caption_ink(v.p))
                    .mt(space::PX_4);
                return Some(
                    col()
                        .key("board-held-wrap")
                        .pin(Some(pos[0]), Some(pos[1]), None, None)
                        .paint_order(40)
                        .inert()
                        .child(card)
                        .child(cue),
                );
            }
            Some(card.pin(Some(pos[0]), Some(pos[1]), None, None).paint_order(40).inert())
        }
        Hold::Settling { what: Held::Card(id), from, to, size, grab, from_lean, start } => {
            let i = t.find(id)?;
            let frac = ((v.s.now.get() - start) / SETTLE_SECS).clamp(0.0, 1.0) as f32;
            let k = ease_out(frac);
            let pos = [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k];
            let lean = from_lean * (1.0 - k);
            let ox = (grab[0] / size[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / size[1].max(1.0)).clamp(0.0, 1.0);
            let body = card_body(v, cx, t, i, cx.look.width, None);
            let mut card = el()
                .key(card_key(id))
                .pin(Some(pos[0]), Some(pos[1]), None, None)
                .size(size[0], size[1])
                .py(CARD_PY)
                .px(CARD_PX)
                .radius(radius::FIELD)
                .fill(c.on_page(c.card))
                .border(1.0, c.hairline)
                .border_layout()
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(Spring::NONE)
                .description(format!("tilt={lean:.4}"))
                .role("listitem")
                .label(t.nodes[i].label.clone())
                .focus_selected(true)
                .paint_order(40)
                .inert()
                .child(body);
            card = Elevation::Dialog.apply(card, v.p, 1.0 - k);
            Some(card)
        }
        Hold::Held { what: Held::Column(id), grab, at, size, refused, lean, .. } => {
            let h = t.find(id)?;
            let ci = model.find_column(h)?;
            let col_data = &model.columns[ci];
            let pos = [at[0] - grab[0], at[1] - grab[1]];
            let ox = (grab[0] / size[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / size[1].max(1.0)).clamp(0.0, 1.0);
            let mut lifted = lifted_column_el(v, cx, t, col_data, size, true)
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(if reduced { Spring::NONE } else { Spring::new(TILT_SPRING, 0.0) })
                .description(format!("tilt={lean:.4}"));
            lifted = Elevation::Dialog.apply(lifted, v.p, 1.0);
            if let Some(reason) = refused {
                let wash = el()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .radius(COL_RADIUS)
                    .fill(c.panel.alpha(0.55))
                    .inert();
                lifted = lifted.child(wash);
                let cue = text(reason)
                    .key("board-refusal")
                    .font(styles::sans(styles::CAPTION, 500.0))
                    .color(styles::caption_ink(v.p))
                    .mt(space::PX_4);
                return Some(
                    col()
                        .key("board-held-wrap")
                        .pin(Some(pos[0]), Some(pos[1]), None, None)
                        .paint_order(40)
                        .inert()
                        .child(lifted)
                        .child(cue),
                );
            }
            Some(lifted.pin(Some(pos[0]), Some(pos[1]), None, None).paint_order(40).inert())
        }
        Hold::Settling { what: Held::Column(id), from, to, size, grab, from_lean, start } => {
            let h = t.find(id)?;
            let ci = model.find_column(h)?;
            let col_data = &model.columns[ci];
            let frac = ((v.s.now.get() - start) / SETTLE_SECS).clamp(0.0, 1.0) as f32;
            let k = ease_out(frac);
            let pos = [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k];
            let lean = from_lean * (1.0 - k);
            let ox = (grab[0] / size[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / size[1].max(1.0)).clamp(0.0, 1.0);
            let mut lifted = lifted_column_el(v, cx, t, col_data, size, false)
                .pin(Some(pos[0]), Some(pos[1]), None, None)
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(Spring::NONE)
                .description(format!("tilt={lean:.4}"))
                .paint_order(40)
                .inert();
            lifted = Elevation::Dialog.apply(lifted, v.p, 1.0 - k);
            Some(lifted)
        }
        _ => None,
    }
}

/// Visual representation of a lifted or settling column in the drag overlay (BRD-021).
fn lifted_column_el(v: V, cx: &Cx, t: &Tree, col_data: &Column, size: [f32; 2], accent_border: bool) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let ColumnHead::Heading(h) = col_data.head else {
        return el();
    };
    let n = &t.nodes[h];
    let id = n.id;
    let st = map_app::style(Tier::Head, b, cx.look.family, cx.look.mono);
    let bar_col = ui_map::branch_color(c, n.branch);
    let bar = el().size(BAR_W, BAR_H).radius(1.5).fill(bar_col);
    let rich = ui_map::label(cx, n, st, c.strong);
    let title = rich.el(st, c.strong, None).truncate().grow(1.0).shrink().min_w(0.0);
    let header = row()
        .w_full()
        .px(space::PX_4)
        .py(space::PX_4)
        .gap(space::PX_8)
        .center_y()
        .radius(radius::CONTROL)
        .child(bar)
        .child(title)
        .child(count_pill(v.p, format!("board-lifted-count:{id}"), col_data.cards.len()));

    let mut stack = col().w_full().gap(CARD_GAP).pt(space::PX_6);
    for &ci in &col_data.cards {
        let body = card_body(v, cx, t, ci, cx.look.width, None);
        let card = el()
            .w_full()
            .py(CARD_PY)
            .px(CARD_PX)
            .radius(radius::FIELD)
            .fill(c.on_page(c.card))
            .border(1.0, c.hairline)
            .border_layout()
            .child(body);
        stack = stack.child(card);
    }

    let mut frame = col()
        .key(col_key(col_data.head, t))
        .size(size[0], size[1])
        .padding(COL_PAD)
        .radius(COL_RADIUS)
        .fill(c.on_page(c.panel))
        .clip_soft(0.0)
        .role("region")
        .label(n.label.clone())
        .focus_selected(true)
        .child(header)
        .child(el().w_full().grow(1.0).min_h(0.0).clip_soft(0.0).child(stack));

    frame = if accent_border {
        frame.border(1.5, c.accent).border_layout()
    } else {
        frame.border(1.0, c.hairline).border_layout()
    };
    frame
}

/// Column header row (BRD-009, BRD-021): 3 × 14 px branch colour bar (omitted on Loose), bold `B`
/// title truncated with ellipsis, and right-aligned `count_pill`.
#[allow(clippy::too_many_arguments)]
fn column_header(v: V, cx: &Cx, t: &Tree, col_data: &Column, h_key: &str, selected: bool, holding: bool, key_id: u64, find: Option<BoardFind>) -> El {
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
            let mut rich = ui_map::label(cx, n, st, c.strong);
            if let Some(f) = find {
                rich.marks = find_marks(c, &rich.text, f, h);
            }
            let title = rich.el(st, c.strong, None).truncate().grow(1.0).shrink().min_w(0.0);
            head_row = head_row
                .role("button")
                .label(n.label.clone())
                .child(bar)
                .child(title)
                .child(count_pill(v.p, format!("board-count:{id}"), col_data.cards.len()));
            if !holding {
                head_row = head_row.hover(c.hover);
            }
            if selected {
                head_row = apply_selection_glass(head_row, c);
            }
            el().key(format!("board-head-host:{id}"))
                .w_full()
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx| s.board_click(id, false, tx))
                .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, true, tx))
                .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_col_drag(id, d, tx))
                .child(head_row)
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
            head_row
                .child(title)
                .child(count_pill(v.p, "board-count:loose", col_data.cards.len()))
        }
    }
}

/// One card on the board (BRD-010, BRD-011, BRD-012, BRD-014, BRD-028). Keyed by `NodeId`.
#[allow(clippy::too_many_arguments)]
fn card_el(v: V, cx: &Cx, t: &Tree, i: usize, selected: bool, holding: bool, settling: bool, find: Option<BoardFind>) -> El {
    let c = &cx.look.c;
    let n = &t.nodes[i];
    let id = n.id;
    let key = if settling { format!("board-card-seat:{id}") } else { card_key(id) };
    let inner_w = cx.look.width;

    let body = card_body(v, cx, t, i, inner_w, find);
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
        .focus_selected(selected && !settling)
        .on_layout(move |s: &mut App, r| s.board_card_laid_out(id, r))
        .child(body);

    if settling {
        card = card.opacity(0.0);
    } else {
        if !holding {
            card = card.hover(c.hover);
        }
        if selected {
            card = apply_selection_glass(card, c);
        }
    }

    el().key(format!("board-card-host:{id}"))
        .w_full()
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(move |s: &mut App, tx| s.board_click(id, false, tx))
        .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, true, tx))
        .on_secondary_press(move |s: &mut App, at, tx| s.board_card_menu(id, Some((at.x, at.y)), tx))
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_card_drag(id, d, tx))
        .child(card)
}

/// Renders a card's inner content: section card, block card, task card, or inline text + child
/// summary caption (BRD-004, BRD-010, BRD-011, BRD-030).
fn card_body(v: V, cx: &Cx, t: &Tree, i: usize, inner_w: f32, find: Option<BoardFind>) -> El {
    let c = &cx.look.c;
    let b = cx.look.b;
    let n = &t.nodes[i];
    let id = n.id;

    // Section card: heading under a column (BRD-004).
    if matches!(n.kind, NodeKind::Heading { .. }) {
        let st = map_app::style(Tier::Head, b, cx.look.family, cx.look.mono);
        let mut rich = ui_map::label(cx, n, st, c.strong);
        if let Some(f) = find {
            rich.marks = find_marks(c, &rich.text, f, i);
        }
        let cap_font = map_app::chip_font(b, cx.look.mono);
        return col()
            .w_full()
            .gap(space::PX_4)
            .child(rich.el(st, c.strong, None).wrap_anywhere().w_full())
            .child(text(board::section_summary(t, i)).font(cap_font).color(c.dim));
    }

    // Block card: code, diff, table, quote, alert, HTML, image, gallery (BRD-010).
    if let Body::Card(bl) = &n.body {
        let hl = find.map(|f| Hl::new(f.q, f.cur.filter(|h| h.node == i).and_then(|h| h.pid.map(|pid| (pid, h.k)))));
        let card_cx = Cx::map(v, cx.look, hl.as_ref());
        if let Some((card_el, _)) = ui_map::block_card(&card_cx, bl, i, &format!("b{id}")) {
            return card_el;
        }
    }

    let st = map_app::style(Tier::Body, b, cx.look.family, cx.look.mono);
    let task = match n.kind {
        NodeKind::Item { task, .. } if task != Task::None => Some(task),
        _ => None,
    };
    let ink = if task == Some(Task::Done) { c.dim } else { c.text };
    let mut rich = ui_map::label(cx, n, st, ink);
    if let Some(f) = find {
        rich.marks = find_marks(c, &rich.text, f, i);
    }
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
