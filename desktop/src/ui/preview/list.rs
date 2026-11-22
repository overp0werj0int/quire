//! Lists (BLK-18 to BLK-32): markers hang in the margin at the top level and step in when
//! nested, each centred on the x-height of its item's first line.

use rune::{El, TextStyle, col, el, icon, text};

use super::{Cx, Tone};
use crate::app::App;
use crate::blocks::{Block, Item, Kind, List, Task};
use crate::page::{line, r};
use crate::ui::icons;
use crate::ui::widgets::Control;

/// The indent left of each item's body and the vertical gap between items for list `l` in `tone`.
pub(super) fn layout_metrics(cx: &Cx, l: &List, tone: Tone) -> (f32, f32, f32, f32) {
    let look = cx.look;
    let t = tone.size;
    let lh = line(t, tone.lead);
    let gap = r(t * 0.55);
    let bx = r(t * 0.9);
    let numbers = TextStyle::new(t, 400.0).family(look.family).lh(lh).features("'tnum' 1");
    let mut mw = t * 0.9;
    if l.ordered {
        let last = l.items.iter().map(|i| i.number).max().unwrap_or(l.start);
        mw = rune::text::advance(&format!("{last}."), numbers);
    }
    if l.tasks {
        mw = mw.max(bx);
    }
    let mw = mw.ceil();
    let indent = if (l.depth > 0 && l.depth <= 10) || (tone.contained && l.depth == 0) { (mw + gap).max(r(t * 1.5)) } else { 0.0 };
    let item_gap = if l.tight { r(t * 0.3) } else { r(t * 0.75) };
    (mw, gap, indent, item_gap)
}

pub(super) fn list(cx: &Cx, l: &List, tone: Tone) -> El {
    let look = cx.look;
    let c = &look.c;
    let t = tone.size;
    let lh = line(t, tone.lead);
    let bx = r(t * 0.9);
    let numbers = TextStyle::new(t, 400.0).family(look.family).lh(lh).features("'tnum' 1");
    let (mw, gap, indent, item_gap) = layout_metrics(cx, l, tone);
    // The marker box's left edge, from the item's text edge (negative: in the margin).
    let mx = indent - gap - mw;
    let xc = look.x_mid(t, lh);
    let mut out = col().w_full().gap(item_gap).role("list");
    for it in &l.items {
        let done = it.task == Task::Done;
        let marker = match it.task {
            Task::None if l.ordered => text(format!("{}.", it.number)).font(numbers).color(c.readable_dim()).align(rune::Align::End).w(mw).pin(Some(mx), Some(0.0), None, None),
            Task::None => match l.depth % 3 {
                0 => {
                    let d = r(t * 0.3);
                    el().size(d, d).radius(d / 2.0).fill(c.dim).pin(Some(mx + mw - r(t * 0.12) - d), Some(r(xc - d / 2.0)), None, None)
                }
                1 => {
                    let d = r(t * 0.32);
                    el().size(d, d).radius(d / 2.0).border(1f32.max(r(t / 14.0)), c.faint).pin(Some(mx + mw - r(t * 0.12) - d), Some(r(xc - d / 2.0)), None, None)
                }
                _ => {
                    let (w, h) = (r(t * 0.36), 1f32.max(r(t / 12.0)));
                    el().size(w, h).radius(h / 2.0).fill(c.faint).pin(Some(mx + mw - r(t * 0.1) - w), Some(r(xc - h / 2.0)), None, None)
                }
            },
            _ => {
                // The drawn box keeps BLK-29's size, centred inside a >= 24 pt wide hit target
                // (24×24 pt at default body size, clamped vertically to the row stride so
                // zoomed-out task checkboxes never overlap, BLK-32).
                let (hit_w, hit_h) = task_hit(t, lh, item_gap);
                let pad_x = (hit_w - bx) / 2.0;
                let pad_y = (hit_h - bx) / 2.0;
                let (rad, edge) = (2f32.max(r(bx * 0.24)), 1f32.max(r(t / 12.0)));
                let mut b = el().size(bx, bx).radius(rad);
                b = if done {
                    b.center().fill(c.accent).child(icon(&icons::CHECK, (bx * 0.78).round()).color(c.bg))
                } else {
                    // Accent under the pointer, at once as omarky's border colour has no fade.
                    let hot = el().key("hot").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(rad).border(edge, c.accent).group_opacity(0.0, 1.0);
                    b.border(edge, c.faint).child(hot)
                };
                let line_no = it.line;
                let lbl = task_label(it);
                let mut hit = el()
                    .size(hit_w, hit_h)
                    .radius(4.0)
                    .center()
                    .hover_group()
                    .hover_motion(rune::Anim::INSTANT, rune::Anim::INSTANT)
                    .role("checkbox")
                    .checked(done)
                    .label(lbl);
                if cx.selectable {
                    let (_, _, rc) = crate::ui::focus_ring(cx.v.p);
                    hit = crate::ui::focus::stop(hit)
                        .cursor(rune::Cursor::Pointer)
                        .focus_ring(2.0, 1.0, rc)
                        .keeps_text_selection()
                        .preserve_focus_on_press(true)
                        .control(move |s: &mut App, _| s.toggle_task(line_no, !done));
                }
                hit.child(b).pin(Some(mx + mw - bx - pad_x), Some(r(xc - bx / 2.0 - t * 0.04) - pad_y), None, None)
            }
        };
        // A done item's text is dim (BLK-31).
        let inner = if done { Tone { color: c.dim, ..tone } } else { tone };
        let body = cx.stack(&it.blocks, inner, l.tight).key("body").w_full().pl(indent);
        out = out.child(el().key(format!("li-{}", it.line)).w_full().child(body).child(marker.key("marker")));
    }
    out
}

/// Descriptive accessible label for a task checkbox from its item text (`WORDSEL`).
fn task_label(it: &Item) -> String {
    fn collect(blocks: &[Block], out: &mut String) {
        for b in blocks {
            match &b.kind {
                Kind::Paragraph { runs } => {
                    for run in runs {
                        if run.br {
                            out.push(' ');
                        } else {
                            out.push_str(&run.t);
                        }
                    }
                }
                Kind::Heading { text, .. } => out.push_str(text),
                _ => continue,
            }
            if !out.trim().is_empty() {
                break;
            }
        }
    }
    let mut out = String::new();
    collect(&it.blocks, &mut out);
    let t = out.trim();
    if t.is_empty() { "Task".to_string() } else { t.to_string() }
}

/// Hit target dimensions `(width, height)` for a task checkbox at text size `t`, line box `lh`,
/// and inter-item `item_gap` (BLK-32). Width stays `>= 24 pt`; height clamps to the row stride
/// `floor(lh + item_gap)` so adjacent zoomed-out task targets never overlap vertically.
pub(super) fn task_hit(t: f32, lh: f32, item_gap: f32) -> (f32, f32) {
    let bx = r(t * 0.9);
    let w = (bx + r(t * 0.5)).max(24.0);
    let h = w.min((lh + item_gap).floor()).max(bx);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blk_32_task_hit_clamps_height_to_row_stride() {
        assert_eq!(task_hit(14.0, line(14.0, 1.6), r(14.0 * 0.3)), (24.0, 24.0), "default body keeps 24x24");
        assert_eq!(task_hit(10.0, line(10.0, 1.6), r(10.0 * 0.3)), (24.0, 19.0), "zoomed out clamps to 19 pt stride");
        assert_eq!(task_hit(9.0, line(9.0, 1.6), r(9.0 * 0.3)), (24.0, 17.0), "minimum body clamps to 17 pt stride");
        assert_eq!(task_hit(20.0, line(20.0, 1.6), r(20.0 * 0.3)), (28.0, 28.0), "zoomed in grows beyond 24");
    }
}
