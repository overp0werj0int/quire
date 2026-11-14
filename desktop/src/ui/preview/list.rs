//! Lists (BLK-18 to BLK-32): markers hang in the margin at the top level and step in when
//! nested, each centred on the x-height of its item's first line.

use rune::{El, TextStyle, col, el, icon, text};

use super::{Cx, Tone};
use crate::app::App;
use crate::blocks::{List, Task};
use crate::page::{line, r};
use crate::ui::octicons;

pub(super) fn list(cx: &Cx, l: &List, tone: Tone) -> El {
    let look = cx.look;
    let c = &look.c;
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
    // The marker box's left edge, from the item's text edge (negative: in the margin).
    let mx = indent - gap - mw;
    let xc = look.x_mid(t, lh);
    let mut out = col().w_full().gap(if l.tight { r(t * 0.3) } else { r(t * 0.75) }).role("list");
    for it in &l.items {
        let done = it.task == Task::Done;
        let marker = match it.task {
            Task::None if l.ordered => text(format!("{}.", it.number)).font(numbers).color(c.faint).align(rune::Align::End).w(mw).pin(Some(mx), Some(0.0), None, None),
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
                // The box, inside a hit area round(T × 0.5) larger (BLK-29, BLK-30, BLK-32).
                let pad = r(t * 0.5) / 2.0;
                let mut b = el().size(bx, bx).radius(2f32.max(r(bx * 0.24)));
                b = if done { b.fill(c.accent).child(icon(&octicons::TICK, bx).color(c.bg)) } else { b.border(1f32.max(r(t / 12.0)), c.faint) };
                let line_no = it.line;
                el().padding(pad)
                    .role("checkbox")
                    .checked(done)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut App| s.toggle_task(line_no, !done))
                    .child(b)
                    .pin(Some(mx + mw - bx - pad), Some(r(xc - bx / 2.0 - t * 0.04) - pad), None, None)
            }
        };
        // A done item's text is dim (BLK-31).
        let inner = if done { Tone { color: c.dim, ..tone } } else { tone };
        let body = cx.stack(&it.blocks, inner, false, l.tight).key("body").w_full().pl(indent);
        out = out.child(el().key(format!("li-{}", it.line)).w_full().child(body).child(marker.key("marker")));
    }
    out
}
