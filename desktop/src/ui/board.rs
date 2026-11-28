//! Board mode view (`docs/reader-writer/behavior/board.md`, decision 0015):
//! heading columns of task cards with recursive subtask folding, colored subtask progress dots,
//! tilted drag/settle overlays, and Map-grade polish and animations, composed strictly from
//! `ui/tokens.rs`, `ui/styles.rs`, `ui/components/`, and the reader's inline runs.

use std::collections::HashSet;

use rune::{Anim, Drag, El, ScrollSpec, Spring, SpringPair, TextHighlight, Transition, Tx, col, el, icon, row, text};

use super::components::{GLASS_R, SelectionGlass, count_pill, faded_h, progress_dot};
use super::icons;
use super::map as ui_map;
use super::preview::{Cx, Look, look_at};
use super::tokens::{Elevation, FocusRing, motion, radius, size, space};
use super::{V, styles};
use crate::app::App;
use crate::blocks::Task;
use crate::board::{self, Card, Column, Slot, SlotParent};
use crate::board_app::{card_key, col_key, head_key};
use crate::board_drag::{Held, Hold, SETTLE_SECS, ease_out};
use crate::map::{NodeId, Tree};
use crate::map_app::{self, Tier};
use crate::page;
use crate::roles::Roles;

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
/// Subtle scale boost on a lifted card, matching Map mode's glass lift.
const LIFT_SCALE: f32 = 1.02;

/// Board's page look at `width`, with `dim` and `faint` lifted to readable contrast as Map does.
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

/// Shared rendering parameters threaded down column and recursive card builders.
struct RenderCtx<'a> {
    v: V<'a>,
    cx: &'a Cx<'a>,
    t: &'a Tree,
    expanded: &'a HashSet<NodeId>,
    sel: Option<NodeId>,
    holding: bool,
    held_card: Option<NodeId>,
    settling_card: Option<NodeId>,
    slot: Option<Slot>,
    slot_h: f32,
    find: Option<BoardFind<'a>>,
    reduced: bool,
}

impl RenderCtx<'_> {
    fn hover_anim(&self) -> Anim {
        if self.reduced { Anim::INSTANT } else { motion::hover_fade() }
    }

    fn glide_spring(&self) -> Spring {
        if self.reduced { Spring::NONE } else { motion::layout_glide_spring() }
    }

    fn slot_transition(&self) -> Transition {
        if self.reduced {
            Transition::none()
        } else {
            let s = motion::modal_spring();
            Transition::opacity().springs(SpringPair::new(s, s))
        }
    }

    fn sub_transition(&self) -> Transition {
        if self.reduced {
            Transition::none()
        } else {
            let s = motion::layout_glide_spring();
            let mut t = Transition::move_edge(rune::Edge::Top).springs(SpringPair::new(s, s));
            t.insertion.opacity = 0.0;
            t.removal.opacity = 0.0;
            t
        }
    }
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
    let expanded = s.board_expanded();
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
    let rc = RenderCtx {
        v,
        cx: &cx,
        t: &t,
        expanded: &expanded,
        sel,
        holding,
        held_card,
        settling_card,
        slot,
        slot_h,
        find,
        reduced,
    };

    let mut cols_row = row()
        .key("board-row")
        .items_start()
        .gap(col_gap)
        .padding(outer_pad)
        .h_full()
        .motion(rc.glide_spring())
        .child_exit(Transition::none())
        .on_layout_tx(|s: &mut App, r, tx| s.board_row_laid_out(r, tx));

    let mut col_slot_placed = false;
    let mut heading_k = 0usize;
    for (ci, col_data) in model.columns.iter().enumerate() {
        let hid = col_data.key_id(&t);
        if held_col == Some(hid) {
            // Keep the column header's drag host alive at the exact same tree path while pinned
            // out of flow so Rune keeps the drag capture and the column's old place closes.
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
            cols_row = cols_row.child(col_slot_el(&rc, col_w, slot_h));
            col_slot_placed = true;
        }
        heading_k += 1;
        let col_settling = settling_col == Some(hid);
        cols_row = cols_row.child(column_el(&rc, ci, col_data, col_w, col_settling));
    }
    if col_drag_slot.is_some() && !col_slot_placed {
        cols_row = cols_row.child(col_slot_el(&rc, col_w, slot_h));
    }

    if !model.has_headings() {
        cols_row = cols_row.child(empty_hint(v, outer_pad));
    }

    // Shared sliding glass selection outline (`components::SelectionGlass`), pinned in `board-row`
    // so it glides smoothly between cards and across columns on Rune's spring while moving in
    // lockstep with horizontal board scroll (via `board-row`) and vertical column scroll (via
    // `Live` offset/opacity updated directly from `on_scroll_offset`, zero view commits).
    s.board.sel_scroll_offset.set([0.0, 0.0]);
    if !holding
        && let Some(sid) = sel
        && settling_card != Some(sid)
        && settling_col != Some(sid)
        && let Some((glass_r, glass_rad, anchor, alpha)) = selection_glass_rect(s, sid)
    {
        s.board.sel_col_anchor.set(anchor);
        s.board.sel_scroll_opacity.set(alpha);
        let lit = if s.layer().is_none() { 1.0 } else { 0.6 };
        let rim = SelectionGlass::spatial("board", v.p, c, glass_r)
            .keys("board-glass", "board-selection")
            .radius(glass_rad)
            .lit(lit)
            .rim_el()
            .live_offset(&s.board.sel_scroll_offset)
            .live_opacity(&s.board.sel_scroll_opacity);
        cols_row = cols_row.child(rim);
    } else {
        s.board.sel_col_anchor.set(None);
        s.board.sel_scroll_opacity.set(1.0);
    }

    // Native Rune horizontal scroll viewport: touchpad swipes, inertial throw, fling boost,
    // touchpad hold stop, and Shift+wheel all flow through Rune's single built-in scroll/throw
    // engine — identical to Preview, Source, wide code blocks, and tables (decision 0012).
    let scroll = el()
        .key("board-scroll")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .h_full()
        .scroll_x()
        .scroll_margin(outer_pad + COL_PAD + 1.0)
        .on_scroll_offset({
            let sx = s.board.scroll_x.clone();
            move |at| sx.set(at.offset[0])
        })
        .child(cols_row);

    let overlay = hold_overlay(&rc, &model, hold);

    el().key("board-canvas")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .h_full()
        .fill(c.bg)
        .clip_soft(0.0)
        .transition(if s.mode_swapped { super::swap() } else { Transition::none() })
        .child_exit(Transition::none())
        .role("region")
        .label("Board")
        .on_layout_tx(|s: &mut App, r, tx| s.board_view_laid_out(r, tx))
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_pan_drag(d, tx))
        .child(scroll)
        .children(overlay)
}

/// Opacity of a card's selection glass relative to its column's scroll viewport so it fades out
/// cleanly inside the column's scroll-edge fade and never bleeds over the column header.
fn sel_card_alpha(vis_top: f32, card_h: f32, col_h: f32) -> f32 {
    let top_a = ((vis_top + 2.0) / COL_FADE).clamp(0.0, 1.0);
    let bot_a = ((col_h - (vis_top + card_h) + 2.0) / COL_FADE).clamp(0.0, 1.0);
    top_a.min(bot_a)
}

/// Computes the `board-row`-local rect, corner radius, optional vertical column-scroll anchor,
/// and current opacity for the sliding selection glass around selected card or header `sid`.
fn selection_glass_rect(s: &App, sid: NodeId) -> Option<([f32; 4], f32, Option<crate::board_app::SelColAnchor>, f32)> {
    let layout = s.board.layout.borrow();
    if layout.row_w <= 0.0 {
        return None;
    }
    let [rx, ry] = layout.row_origin;
    if let Some(&r) = layout.cards.get(&sid) {
        let mut y = r[1];
        let mut anchor = None;
        let mut alpha = 1.0;
        if let Some(&(cid, laid_scroll)) = layout.card_scroll.get(&sid) {
            let cur_scroll = s.board.col_scroll.borrow().get(&cid).copied().unwrap_or(0.0);
            y -= cur_scroll - laid_scroll;
            if let Some(&cv) = layout.col_views.get(&cid) {
                let vis_top = y - cv[1];
                alpha = sel_card_alpha(vis_top, r[3], cv[3]);
                anchor = Some(crate::board_app::SelColAnchor {
                    col_id: cid,
                    base_scroll: cur_scroll,
                    vis_top,
                    card_h: r[3],
                    col_h: cv[3],
                });
            }
        }
        let rect = SelectionGlass::expanded_rect([r[0] - rx, y - ry, r[2], r[3]]);
        return Some((rect, GLASS_R.min(rect[3] * 0.5), anchor, alpha));
    }
    if let Some(&r) = layout.heads.get(&sid) {
        let rect = SelectionGlass::expanded_rect([r[0] - rx, r[1] - ry, r[2], r[3]]);
        return Some((rect, (radius::CONTROL + 2.0).min(rect[3] * 0.5), None, 1.0));
    }
    None
}

/// Quiet empty-board hint when a kanban note has no column headings yet (BRD-006).
fn empty_hint(v: V, pad: f32) -> El {
    el().key("board-empty-hint")
        .pt(pad * 0.5)
        .px(space::PX_8)
        .child(
            text("Headings become columns and tasks become cards. Add one in Source to start a board.")
                .font(styles::desc())
                .color(styles::caption_ink(v.p)),
        )
}

/// One heading column: header row + scrollable task card stack capped to the board height.
fn column_el(rc: &RenderCtx, ci: usize, col_data: &Column, col_w: f32, col_settling: bool) -> El {
    let c = &rc.cx.look.c;
    let b = rc.cx.look.b;
    let t = rc.t;
    let key_id = col_data.key_id(t);
    let col_branch = t.nodes[col_data.heading].branch;
    let c_key = if col_settling { format!("board-col-seat:{key_id}") } else { col_key(key_id) };
    let h_key = head_key(key_id);
    let head_selected = rc.sel == Some(key_id) && !col_settling;

    let header = column_header(rc, col_data, &h_key, head_selected, key_id);

    let mut cards_stack = col()
        .key(format!("board-cards:{key_id}"))
        .w_full()
        .gap(CARD_GAP)
        .pt(space::PX_6)
        .pb(space::PX_4)
        .motion(rc.glide_spring())
        .child_exit(Transition::none())
        .on_layout_tx(move |s: &mut App, r, tx| s.board_col_content_laid_out(key_id, r[3], tx));

    let col_slot = rc.slot.filter(|sl| sl.parent == SlotParent::Column(ci)).map(|sl| sl.index);
    let mut slot_placed = false;
    let mut rest_k = 0usize;
    for card in &col_data.cards {
        let cid = t.nodes[card.node].id;
        if rc.held_card == Some(cid) {
            cards_stack = cards_stack.child(held_card_host(cid));
            continue;
        }
        if col_slot == Some(rest_k) {
            cards_stack = cards_stack.child(el().w_full().px(space::PX_4).child(slot_el(rc, "board-slot")));
            slot_placed = true;
        }
        cards_stack = cards_stack.child(card_tree_el(rc, card, key_id, col_branch, 0));
        rest_k += 1;
    }
    if col_slot.is_some() && !slot_placed {
        cards_stack = cards_stack.child(el().w_full().px(space::PX_4).child(slot_el(rc, "board-slot")));
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
            let map = rc.v.s.board.col_scroll.clone();
            let anchor = rc.v.s.board.sel_col_anchor.clone();
            let sel_off = rc.v.s.board.sel_scroll_offset.clone();
            let sel_vis = rc.v.s.board.sel_scroll_opacity.clone();
            move |at| {
                let off = at.offset[1];
                map.borrow_mut().insert(key_id, off);
                if let Some(a) = anchor.get()
                    && a.col_id == key_id
                {
                    let dy = -(off - a.base_scroll);
                    sel_off.set([0.0, dy]);
                    sel_vis.set(sel_card_alpha(a.vis_top + dy, a.card_h, a.col_h));
                }
            }
        })
        .on_layout_tx(move |s: &mut App, r, tx| s.board_col_view_laid_out(key_id, r, tx))
        .child(faded_cards);

    // Column is as tall as its cards and never taller than the viewport area (BRD-008).
    let outer_pad = page::r(b * 16.0 / 14.0);
    let avail_h = (rc.v.s.board.layout.borrow().view[3] - 2.0 * outer_pad).max(160.0);

    let bounds_marker = el()
        .key(c_key)
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .inert()
        .on_layout_tx(move |s: &mut App, r, tx| s.board_col_laid_out(key_id, r, tx));

    let mut column = col()
        .key(format!("board-col-host:{key_id}"))
        .w(col_w)
        .max_h(avail_h)
        .py(COL_PAD)
        .px(space::PX_4)
        .radius(radius::COLUMN)
        .fill(c.panel)
        .border(1.0, c.hairline)
        .border_layout()
        .child(bounds_marker)
        .child(el().w_full().px(space::PX_4).child(header))
        .child(cards_scroll);

    if col_settling {
        column = column.opacity(0.0);
    }
    column
}

/// Out-of-flow drag host keeping Rune's drag capture alive while a card is lifted.
fn held_card_host(cid: NodeId) -> El {
    el().key(format!("board-card-host:{cid}"))
        .pin(Some(0.0), Some(0.0), None, None)
        .size(0.0, 0.0)
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_card_drag(cid, d, tx))
}

/// Renders a task card and, when it has subtasks, a persistent disclosure container underneath it
/// so toggling fold/unfold never remounts (flickers) the parent card and sub-cards smoothly slide
/// out from under the parent card's bottom edge.
fn card_tree_el(rc: &RenderCtx, card: &Card, col_id: u64, col_branch: usize, depth: usize) -> El {
    let t = rc.t;
    let id = t.nodes[card.node].id;
    let selected = rc.sel == Some(id);
    let settling = rc.settling_card == Some(id);
    let has_subs = !card.subtasks.is_empty();
    let is_expanded = has_subs && rc.expanded.contains(&id);
    let card_ui = card_el(rc, card, col_id, col_branch, selected, settling, is_expanded, depth == 0);

    if !has_subs {
        return card_ui;
    }

    let sub_t = rc.sub_transition();
    let sub_wrap = (is_expanded && !settling).then(|| {
        let c = &rc.cx.look.c;
        let rail_col = Roles::readable_on(ui_map::branch_color(c, col_branch), c.on_page(c.panel), 3.1);
        let sub_slot = rc.slot.filter(|sl| sl.parent == SlotParent::Card(card.node)).map(|sl| sl.index);
        let mut sub_stack = col()
            .key(format!("board-subs-stack:{id}"))
            .w_full()
            .gap(CARD_GAP)
            .motion(rc.glide_spring())
            .child_exit(Transition::none());

        let mut slot_placed = false;
        let mut rest_k = 0usize;
        for sub in &card.subtasks {
            let sid = t.nodes[sub.node].id;
            if rc.held_card == Some(sid) {
                sub_stack = sub_stack.child(held_card_host(sid));
                continue;
            }
            if sub_slot == Some(rest_k) {
                sub_stack = sub_stack.child(slot_el(rc, "board-slot"));
                slot_placed = true;
            }
            sub_stack = sub_stack.child(card_tree_el(rc, sub, col_id, col_branch, depth + 1));
            rest_k += 1;
        }
        if sub_slot.is_some() && !slot_placed {
            sub_stack = sub_stack.child(slot_el(rc, "board-slot"));
        }

        let rail_x = if depth == 0 { space::PX_4 + space::PX_4 } else { space::PX_4 };
        let rail = el()
            .pin(Some(rail_x), Some(space::PX_6), None, Some(space::PX_4))
            .w(space::PX_2)
            .radius(1.0)
            .fill(rail_col.alpha(0.45))
            .inert();

        let pl = if depth == 0 { space::PX_12 + space::PX_4 } else { space::PX_12 };
        let pr = if depth == 0 { space::PX_4 } else { 0.0 };
        el().key(format!("board-subs:{id}"))
            .w_full()
            .pl(pl)
            .pr(pr)
            .pt(space::PX_6)
            .transition(sub_t)
            .on_layout_tx(move |s: &mut App, r, tx| s.board_sub_list_laid_out(id, r, tx))
            .child(rail)
            .child(sub_stack)
    });

    let clip_box = el()
        .key(format!("board-subs-clip:{id}"))
        .w_full()
        .clip_soft(0.0)
        .motion(rc.glide_spring())
        .child_exit(sub_t)
        .children(sub_wrap);

    col()
        .key(format!("board-card-group:{id}"))
        .w_full()
        .motion(rc.glide_spring())
        .child_exit(Transition::none())
        .child(card_ui.paint_order(if selected { 10 } else { 5 }))
        .child(clip_box)
}

/// The dashed accent drop slot that opens where a dropped card would land (BRD-016).
fn slot_el(rc: &RenderCtx, key: &'static str) -> El {
    let c = &rc.cx.look.c;
    el().key(key)
        .w_full()
        .h(rc.slot_h.max(size::CONTROL_BAR))
        .radius(radius::FIELD)
        .fill(c.accent.alpha(0.10))
        .border(1.5, c.accent.alpha(0.55))
        .border_dash(6.0, 4.0)
        .transition(rc.slot_transition())
        .inert()
}

/// The dashed accent column-wide slot that opens between columns during a column drag (BRD-021).
fn col_slot_el(rc: &RenderCtx, w: f32, h: f32) -> El {
    let c = &rc.cx.look.c;
    el().key("board-col-slot")
        .size(w, h.max(80.0))
        .radius(radius::COLUMN)
        .fill(c.accent.alpha(0.10))
        .border(1.5, c.accent.alpha(0.55))
        .border_dash(6.0, 4.0)
        .transition(rc.slot_transition())
        .inert()
}

/// Overlay for the lifted/settling card or column (BRD-014, BRD-015, BRD-017, BRD-019, BRD-021, BRD-022).
fn hold_overlay(rc: &RenderCtx, model: &board::Board, hold: &Hold) -> Option<El> {
    let c = &rc.cx.look.c;
    let t = rc.t;
    let v = rc.v;
    match hold {
        &Hold::Held { what: Held::Card(id), grab, at, size: sz, refused, lean, .. } => {
            let i = t.find(id)?;
            let card_data = model.card(i)?;
            let col_branch = model.find_card_column(i).map_or(0, |ci| t.nodes[model.columns[ci].heading].branch);
            let pos = [at[0] - grab[0], at[1] - grab[1]];
            let ox = (grab[0] / sz[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / sz[1].max(1.0)).clamp(0.0, 1.0);
            let body = card_body(rc, card_data, col_branch, false, false);
            let mut card = el()
                .key(card_key(id))
                .size(sz[0], sz[1])
                .py(CARD_PY)
                .px(CARD_PX)
                .radius(radius::FIELD)
                .fill(c.on_page(c.card))
                .border(1.5, c.accent)
                .border_layout()
                .origin(ox, oy)
                .scale(if rc.reduced { 1.0 } else { LIFT_SCALE })
                .rotate(lean)
                .rotate_motion(if rc.reduced { Spring::NONE } else { motion::tilt_spring() })
                .role("listitem")
                .label(t.nodes[i].label.clone())
                .focus_selected(true)
                .child(body);
            card = Elevation::Dialog.apply(card, v.p, 1.0);
            if let Some(reason) = refused {
                let wash = el()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .radius(radius::FIELD)
                    .fill(c.dim.alpha(0.22))
                    .inert();
                card = card.child(wash);
                let cue = text(reason)
                    .key("board-refusal")
                    .font(styles::small())
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
        &Hold::Settling { what: Held::Card(id), from, to, size: sz, grab, from_lean, start, .. } => {
            let i = t.find(id)?;
            let card_data = model.card(i)?;
            let col_branch = model.find_card_column(i).map_or(0, |ci| t.nodes[model.columns[ci].heading].branch);
            let frac = ((v.s.now.get() - start) / SETTLE_SECS).clamp(0.0, 1.0) as f32;
            let k = ease_out(frac);
            let pos = [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k];
            let lean = from_lean * (1.0 - k);
            let scale = if rc.reduced { 1.0 } else { 1.0 + (LIFT_SCALE - 1.0) * (1.0 - k) };
            let ox = (grab[0] / sz[0].max(1.0)).clamp(0.0, 1.0);
            let oy = (grab[1] / sz[1].max(1.0)).clamp(0.0, 1.0);
            let body = card_body(rc, card_data, col_branch, false, false);
            let mut card = el()
                .key(card_key(id))
                .pin(Some(pos[0]), Some(pos[1]), None, None)
                .size(sz[0], sz[1])
                .py(CARD_PY)
                .px(CARD_PX)
                .radius(radius::FIELD)
                .fill(c.on_page(c.card))
                .border(1.0, c.hairline)
                .border_layout()
                .origin(ox, oy)
                .scale(scale)
                .rotate(lean)
                .rotate_motion(Spring::NONE)
                .role("listitem")
                .label(t.nodes[i].label.clone())
                .focus_selected(true)
                .paint_order(40)
                .inert()
                .child(body);
            card = Elevation::Dialog.apply(card, v.p, 1.0 - k);
            Some(card)
        }
        &Hold::Held { what: Held::Column(id), grab, at, size: sz, refused, lean, .. } => {
            let h = t.find(id)?;
            let ci = model.find_column(h)?;
            let col_data = &model.columns[ci];
            let bw = 1.5;
            let pos = [at[0] - grab[0] - bw, at[1] - grab[1] - bw];
            let ox = ((grab[0] + bw) / (sz[0] + 2.0 * bw).max(1.0)).clamp(0.0, 1.0);
            let oy = ((grab[1] + bw) / (sz[1] + 2.0 * bw).max(1.0)).clamp(0.0, 1.0);
            let mut lifted = lifted_column_el(rc, col_data, sz, true)
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(if rc.reduced { Spring::NONE } else { motion::tilt_spring() });
            lifted = Elevation::Dialog.apply(lifted, v.p, 1.0);
            if let Some(reason) = refused {
                let wash = el()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .radius(radius::COLUMN)
                    .fill(c.dim.alpha(0.22))
                    .inert();
                lifted = lifted.child(wash);
                let cue = text(reason)
                    .key("board-refusal")
                    .font(styles::small())
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
        &Hold::Settling { what: Held::Column(id), from, to, size: sz, grab, from_lean, start, .. } => {
            let h = t.find(id)?;
            let ci = model.find_column(h)?;
            let col_data = &model.columns[ci];
            let bw = 1.0;
            let frac = ((v.s.now.get() - start) / SETTLE_SECS).clamp(0.0, 1.0) as f32;
            let k = ease_out(frac);
            let pos = [from[0] + (to[0] - from[0]) * k - bw, from[1] + (to[1] - from[1]) * k - bw];
            let lean = from_lean * (1.0 - k);
            let ox = ((grab[0] + bw) / (sz[0] + 2.0 * bw).max(1.0)).clamp(0.0, 1.0);
            let oy = ((grab[1] + bw) / (sz[1] + 2.0 * bw).max(1.0)).clamp(0.0, 1.0);
            let mut lifted = lifted_column_el(rc, col_data, sz, false)
                .pin(Some(pos[0]), Some(pos[1]), None, None)
                .origin(ox, oy)
                .rotate(lean)
                .rotate_motion(Spring::NONE)
                .paint_order(40)
                .inert();
            lifted = Elevation::Dialog.apply(lifted, v.p, 1.0 - k);
            Some(lifted)
        }
        _ => None,
    }
}

/// Visual representation of a lifted or settling column in the drag overlay (BRD-021).
fn lifted_column_el(rc: &RenderCtx, col_data: &Column, sz: [f32; 2], accent_border: bool) -> El {
    let c = &rc.cx.look.c;
    let b = rc.cx.look.b;
    let t = rc.t;
    let n = &t.nodes[col_data.heading];
    let id = n.id;
    let col_branch = n.branch;
    let st = map_app::style(Tier::Head, b, rc.cx.look.family, rc.cx.look.mono);
    let bar_col = ui_map::branch_color(c, col_branch);
    let bar = el().size(BAR_W, BAR_H).radius(1.5).fill(bar_col);
    let rich = ui_map::label(rc.cx, n, st, c.strong);
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
        .child(count_pill(rc.v.p, format!("board-lifted-count:{id}"), col_data.cards.len()));

    let mut stack = col().w_full().gap(CARD_GAP).pt(space::PX_6);
    for card_data in &col_data.cards {
        let body = card_body(rc, card_data, col_branch, false, false);
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

    let bw = if accent_border { 1.5 } else { 1.0 };
    let scroll_y = rc.v.s.board.col_scroll.borrow().get(&id).copied().unwrap_or(0.0);
    let mut frame = col()
        .key(col_key(id))
        .size(sz[0] + 2.0 * bw, sz[1] + 2.0 * bw)
        .padding(COL_PAD)
        .radius(radius::COLUMN)
        .fill(c.on_page(c.panel))
        .clip_soft(0.0)
        .role("region")
        .label(n.label.clone())
        .focus_selected(true)
        .child(header)
        .child(el().w_full().grow(1.0).min_h(0.0).clip_soft(0.0).child(stack.pin(Some(0.0), Some(-scroll_y), Some(0.0), None)));

    frame = if accent_border {
        frame.border(bw, c.accent).border_layout()
    } else {
        frame.border(bw, c.hairline).border_layout()
    };
    frame
}

/// Column header row (BRD-009, BRD-021): 3 × 14 px branch colour bar, bold `B` title truncated
/// with ellipsis, and right-aligned `count_pill`.
fn column_header(rc: &RenderCtx, col_data: &Column, h_key: &str, selected: bool, key_id: u64) -> El {
    let c = &rc.cx.look.c;
    let b = rc.cx.look.b;
    let t = rc.t;
    let n = &t.nodes[col_data.heading];
    let id = n.id;
    let st = map_app::style(Tier::Head, b, rc.cx.look.family, rc.cx.look.mono);
    let bar_col = ui_map::branch_color(c, n.branch);
    let bar = el().key(format!("board-bar:{id}")).size(BAR_W, BAR_H).radius(1.5).fill(bar_col);
    let mut rich = ui_map::label(rc.cx, n, st, c.strong);
    if let Some(f) = rc.find {
        rich.marks = find_marks(c, &rich.text, f, col_data.heading);
    }
    let title = rich.el(st, c.strong, None).focusable(false).truncate().grow(1.0).shrink().min_w(0.0);
    let hover_anim = rc.hover_anim();

    let mut head_row = row()
        .key(h_key)
        .w_full()
        .px(space::PX_4)
        .py(space::PX_4)
        .gap(space::PX_8)
        .center_y()
        .radius(radius::CONTROL)
        .focus_selected(selected)
        .role("button")
        .label(n.label.clone())
        .on_layout_tx(move |s: &mut App, r, tx| s.board_head_laid_out(key_id, r, tx))
        .child(bar)
        .child(title)
        .child(count_pill(rc.v.p, format!("board-count:{id}"), col_data.cards.len()));

    if !rc.holding && !selected {
        head_row = head_row.hover(c.hover).hover_motion(hover_anim, hover_anim);
    }
    if selected && !rc.holding {
        let sz = rc.v.s.board.layout.borrow().heads.get(&key_id).map(|r| [r[2], r[3]]);
        let grow = sz.map_or(if rc.reduced { 1.0 } else { 1.025 }, |[w, h]| SelectionGlass::scale_for(w, h, rc.reduced));
        head_row = SelectionGlass::lift(
            head_row.fill(c.on_page(c.panel)).scale(grow).paint_order(10),
            rc.v.p,
            true,
        );
    }
    el().key(format!("board-head-host:{id}"))
        .w_full()
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(move |s: &mut App, tx| s.board_click(id, true, false, tx))
        .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, true, true, tx))
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_col_drag(id, d, tx))
        .child(head_row)
}

/// One task card on the board (top-level or nested sub-card). Keyed by `NodeId`.
fn card_el(rc: &RenderCtx, card_data: &Card, col_id: u64, col_branch: usize, selected: bool, settling: bool, is_expanded: bool, top_level: bool) -> El {
    let c = &rc.cx.look.c;
    let n = &rc.t.nodes[card_data.node];
    let id = n.id;
    let key = if settling { format!("board-card-seat:{id}") } else { card_key(id) };
    let hover_anim = rc.hover_anim();

    let body = card_body(rc, card_data, col_branch, is_expanded, true);
    let mut card = el()
        .key(key)
        .w_full()
        .py(CARD_PY)
        .px(CARD_PX)
        .radius(radius::FIELD)
        .fill(c.on_page(c.card))
        .border(1.0, c.hairline)
        .border_layout()
        .role("listitem")
        .label(n.label.clone())
        .focus_selected(selected && !settling)
        .on_layout_tx(move |s: &mut App, r, tx| s.board_card_laid_out(id, col_id, r, tx))
        .child(body);

    if settling {
        card = card.opacity(0.0);
    } else {
        if !rc.holding && !selected {
            card = card.hover(c.hover).hover_motion(hover_anim, hover_anim);
        }
        if selected && !rc.holding {
            let sz = rc.v.s.board.layout.borrow().cards.get(&id).map(|r| [r[2], r[3]]);
            let grow = sz.map_or(if rc.reduced { 1.0 } else { 1.025 }, |[w, h]| SelectionGlass::scale_for(w, h, rc.reduced));
            card = SelectionGlass::lift(
                card.scale(grow).paint_order(10),
                rc.v.p,
                true,
            );
        }
    }

    let mut host = el()
        .key(format!("board-card-host:{id}"))
        .w_full()
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(move |s: &mut App, tx| s.board_click(id, false, false, tx))
        .on_double_press_tx(move |s: &mut App, tx| s.board_click(id, false, true, tx))
        .on_secondary_press(move |s: &mut App, at, tx| s.board_card_menu(id, Some((at.x, at.y)), tx))
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.board_card_drag(id, d, tx))
        .child(card);
    if top_level {
        host = host.px(space::PX_4);
    }
    host
}

/// Renders a task card's inner content: inline-run label (dimmed when done; every board card is
/// already a task so cards omit checkboxes), and (when subtasks or body notes exist) the folding
/// handle, colored solid/outlined subtask progress dots, and subtle details icon.
fn card_body(rc: &RenderCtx, card_data: &Card, col_branch: usize, is_expanded: bool, interactive: bool) -> El {
    let c = &rc.cx.look.c;
    let b = rc.cx.look.b;
    let t = rc.t;
    let i = card_data.node;
    let n = &t.nodes[i];
    let id = n.id;
    let hover_anim = rc.hover_anim();

    let st = map_app::style(Tier::Body, b, rc.cx.look.family, rc.cx.look.mono);
    let checked = board::task_state(t, i) == Some(Task::Done);
    let ink = if checked { c.dim } else { c.text };
    let mut rich = ui_map::label(rc.cx, n, st, ink);
    if let Some(f) = rc.find {
        rich.marks = find_marks(c, &rich.text, f, i);
    }
    let label_el = rich.el(st, ink, None).focusable(false).wrap_anywhere().w_full().min_w(0.0);

    let has_subs = !card_data.subtasks.is_empty();
    if !has_subs && !card_data.has_body {
        return label_el;
    }

    let card_surf = c.on_page(c.card);
    let dot_col = Roles::readable_on(ui_map::branch_color(c, col_branch), card_surf, 3.1);
    let mut meta_row = row().key(format!("board-meta:{id}")).w_full().gap(space::PX_6).center_y();

    if has_subs {
        let fold_tip = if is_expanded { "Fold subtasks · Space" } else { "Unfold subtasks · Space" };
        let chev = icon(&icons::CHEVRON_RIGHT, 10.0)
            .color(dot_col)
            .rotate(if is_expanded { std::f32::consts::FRAC_PI_2 } else { 0.0 })
            .rotate_motion(rc.glide_spring());
        let mut handle = row()
            .key(format!("board-fold:{id}"))
            .size(size::CONTROL_BAR, size::CONTROL_BAR)
            .ml(-space::PX_6)
            .radius(radius::INNER)
            .center()
            .role("button")
            .label(if is_expanded { "Fold subtasks" } else { "Unfold subtasks" })
            .help(fold_tip)
            .child(chev);
        if interactive {
            handle = FocusRing::Tight.apply(
                handle
                    .cursor(rune::Cursor::Pointer)
                    .hover(dot_col.alpha(0.16))
                    .hover_motion(hover_anim, hover_anim)
                    .on_press_tx(move |s: &mut App, tx| s.board_toggle_fold(id, tx)),
                rc.v.p,
            );
        }
        meta_row = meta_row.child(handle);

        let dots_states = card_data.subtask_dots(t);
        let done_n = dots_states.iter().filter(|&&d| d).count();
        let total_n = dots_states.len();
        let mut dots_wrap = row()
            .key(format!("board-dots:{id}"))
            .flex_wrap()
            .gap(space::GAP_TIGHT)
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .label(format!("{done_n} of {total_n} subtasks done"));
        for done in dots_states {
            dots_wrap = dots_wrap.child(progress_dot(dot_col, done));
        }
        meta_row = meta_row.child(dots_wrap);
    } else {
        meta_row = meta_row.child(el().grow(1.0));
    }

    if card_data.has_body {
        meta_row = meta_row.child(
            el().key(format!("board-body-mark:{id}"))
                .help("Has details · click card to preview")
                .child(icon(&icons::FILE_TEXT, 11.0).color(c.dim)),
        );
    }

    col()
        .w_full()
        .gap(space::PX_4)
        .child(label_el)
        .child(meta_row)
}
