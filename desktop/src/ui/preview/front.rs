//! The front-matter card (BLK-85 to BLK-98) and pills (BLK-99).

use rune::{Color, El, Icon, TextCase, TextStyle, col, el, icon, row, text};

use super::{Cx, Rich};
use crate::blocks::{Field, FieldKind, Link, Run};
use crate::page::{line, r, step};
use crate::roles::mix;
use crate::ui::octicons;

/// The tone of a `status` or `state` value (BLK-94).
fn status_tone(cx: &Cx, key: &str, value: &str) -> Option<Color> {
    if !matches!(key.to_ascii_lowercase().as_str(), "status" | "state") {
        return None;
    }
    let v = value.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| v.contains(w));
    let c = &cx.look.c;
    if any(&["accept", "approv", "done", "complete", "shipped", "merged", "final", "publish", "active", "ready", "resolved", "live"]) {
        Some(c.green)
    } else if any(&["draft", "propos", "wip", "progress", "pending", "review", "open", "todo", "planned", "exploring"]) {
        Some(c.yellow)
    } else if any(&["reject", "deprecat", "supersed", "block", "abandon", "closed", "cancel", "obsolete", "failed"]) {
        Some(c.red)
    } else {
        None
    }
}

/// YYYY-MM-DD, optionally followed by a time (BLK-95).
fn is_date(v: &str) -> bool {
    let b = v.trim().as_bytes();
    b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, ch)| if i == 4 || i == 7 { *ch == b'-' } else { ch.is_ascii_digit() })
        && (b.len() == 10 || matches!(b[10], b' ' | b'T'))
}

/// A pill: mono, twice its size tall, fully rounded, neutral or toned (BLK-99).
pub(super) fn pill(cx: &Cx, label: &str, tone: Option<Color>, glyph: Option<&'static Icon>) -> El {
    let look = cx.look;
    let c = &look.c;
    let size = r(look.b * 0.76);
    let h = 2.0 * size;
    let (fill, edge, ink, icon_ink) = match tone {
        None => (c.raised, c.hairline, c.dim, c.faint),
        Some(t) => (t.alpha(look.ld(0.12, 0.14)), t.alpha(look.ld(0.4, 0.35)), mix(t, c.text, look.ld(0.25, 0.1)), t),
    };
    let mut p = row().h(h).px(r(size * 1.3) / 2.0).gap(r(size * 0.4)).center_y().radius(h / 2.0).fill(fill).border(1.0, edge);
    if let Some(g) = glyph {
        p = p.child(icon(g, size).key("icon").color(icon_ink));
    }
    p.child(text(label.to_string()).key("label").font(look.mono_at(size, 1.3)).color(ink))
}

pub(super) fn card(cx: &Cx, title: &str, fields: &[Field]) -> El {
    let look = cx.look;
    let c = &look.c;
    let b = look.b;
    let pad = r(b * 1.2);
    let ks = r(b * 0.74);
    let key_style = TextStyle::new(ks, 400.0).family(look.mono).lh(line(ks, 1.3)).tracking(0.8);
    let widest = fields.iter().map(|f| rune::text::advance(&f.key.to_uppercase(), key_style)).fold(0.0, f32::max);
    let key_w = widest.min(9.0 * b).ceil() + r(b * 1.2);
    let vs = r(b * 0.9);
    let ls = r(b * 0.66);
    let label = text("front matter")
        .key("label")
        .font(TextStyle::new(ls, 400.0).family(look.mono).lh(line(ls, 1.3)).tracking(0.8))
        .color(c.ghost)
        .text_case(TextCase::Uppercase)
        .pin(None, Some(pad - r(b * 0.35)), Some(pad), None);
    let mut body = col().key("body").w_full();
    if !title.is_empty() {
        let ts = step(b, 1.5);
        body = body.child(text(title.to_string()).key("title").font(TextStyle::new(ts, 700.0).family(look.family).lh(line(ts, 1.3))).color(c.strong).wrap().w_full().pr(r(b * 6.0)).mb(r(b * 0.6)));
    }
    let mut rows = col().key("rows").w_full().gap(r(b * 0.55));
    for (i, f) in fields.iter().enumerate() {
        let value: El = match f.kind {
            FieldKind::Pill => {
                let glyph = is_date(&f.value).then_some(&octicons::CALENDAR);
                pill(cx, &f.value, status_tone(cx, &f.key, &f.value), glyph)
            }
            FieldKind::List => {
                let mut r_ = row().w_full().flex_wrap().gap(r(b * 0.35));
                for (j, it) in f.items.iter().enumerate() {
                    r_ = r_.child(pill(cx, it, None, None).key(format!("v-{j}")));
                }
                r_
            }
            FieldKind::Link => {
                let mut rich = Rich::default();
                let run = Run { t: f.value.clone(), link: Some(Link::Url(f.value.clone())), ..Run::default() };
                rich.runs(&[run], cx, super::Tone { color: c.link, size: vs, lead: 1.5, contained: true, quote_depth: 0 });
                rich.el(look.prose(vs, 1.5), c.link).w_full()
            }
            FieldKind::Empty => text("\u{2014}").font(look.prose(vs, 1.5)).color(c.ghost),
            FieldKind::Text => text(f.value.clone()).font(look.prose(vs, 1.5)).color(c.text).wrap().w_full(),
        };
        let key = text(f.key.clone()).key("key").font(key_style).color(c.faint).text_case(TextCase::Uppercase).w(key_w);
        rows = rows.child(row().key(format!("f-{i}")).w_full().min_h(r(b * 1.55)).center_y().child(key).child(el().key("value").grow(1.0).min_w(0.0).child(value)));
    }
    body = body.child(rows);
    el().w_full().padding(pad).radius(6.0).fill(c.card).border(1.0, c.hairline).child(body).child(label)
}

#[cfg(test)]
mod tests {
    use super::is_date;

    #[test]
    fn blk_95_dates() {
        assert!(is_date("2026-09-30") && is_date("2026-09-30 12:00") && is_date("2026-09-30T12:00"));
        assert!(!is_date("2026-9-30") && !is_date("Sep 30") && !is_date("2026-09-301"));
    }
}
