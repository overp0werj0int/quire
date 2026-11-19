//! Search's results, in place of the shelf and the vault while the field holds text (SB-57 to
//! SB-77): Titles, Text and Tags, each under its label with its count, then for a whole tag
//! the notes that carry it. Title and text rows are two rows, as a tab is; the matched letters
//! are in the accent as text. One row is filled, the one Enter opens: the row with the
//! keyboard, else the first. The rows come from `App::results`, worked out when the query or
//! the notes change, not here.

use rune::{El, Span, col, icon, rich, row, text};

use super::super::V;
use super::super::lists;
use super::super::panel;
use super::super::styles::{nav, sans};
use super::super::widgets::TAG;
use super::parts::{caption_style, label_row, two_rows};
use super::tree::last_part;
use super::{BUTTON_END, kbd};
use crate::app::App;
use crate::md;
use crate::roles::Roles;
use crate::search::{Results, Row};

/// The results list, one Tab stop that enters at the filled row (SB-81).
pub(super) fn results(v: V, r: &Results) -> El {
    let c = Roles::panel(v.p);
    let mut out = lists::list(col().key("results").w_full(), "list");
    let Some(first) = r.first() else {
        return out.child(text("No matches").font(nav(panel::BODY_SMALL, false)).color(c.faint).py(6.0).pl(panel::TEXT_X).pr(panel::RIGHT));
    };
    let first_key = first.key();
    let filled = v.s.res_focus.clone().filter(|k| r.rows().any(|row| row.key() == *k)).unwrap_or(first_key.clone());
    let ql = r.query.to_lowercase();
    let tag_q = ql.strip_prefix('#').map(|t| t.trim().to_string());
    let section = |key: &str, name: &str, n: usize| label_row(v.p, key, None, name, n.to_string()).mt(2.0);
    let mut last = None;
    for row in r.rows() {
        let kind = std::mem::discriminant(&row);
        if last != Some(kind) {
            last = Some(kind);
            out = out.child(match row {
                Row::Title(_) => section("sec-titles", "Titles", r.titles.len()),
                Row::Text(_) => section("sec-text", "Text", r.text.len()),
                Row::Tag(..) => section("sec-tags", "Tags", r.tags.len()),
                Row::Tagged(_) => section("sec-tagged", "Tagged", r.tagged.len()),
            });
        }
        let key = row.key();
        let on = key == filled;
        let el = match row {
            Row::Title(p) => note_row(v, &key, p, marked(md::base(p), &ql, &c), None, on),
            Row::Tagged(p) => note_row(v, &key, p, (md::base(p).to_string(), Vec::new()), None, on),
            Row::Text(h) => note_row(v, &key, &h.path, (md::base(&h.path).to_string(), Vec::new()), Some(snip(v, h, ql.len())), on),
            Row::Tag(t, n) => tag_row(v, &key, t, n, tag_q.as_deref().unwrap_or(&ql), on),
        };
        let el = if on { el.child(kbd(v, "Enter".into())) } else { el };
        out = out.child(choose(v, el, &row, on, key == first_key));
    }
    out
}

/// What choosing `row` does, by a click, Enter or Space (SB-63, SB-64, SB-77). A note's row
/// opens it in the current mode, or switches to its tab, and the keyboard goes to the note. A
/// tag lists its notes; chosen by keys, the keyboard goes to the first of them. Up from the
/// first row goes back to the field (SB-66).
fn choose(v: V, el: El, row: &Row, on: bool, first: bool) -> El {
    let el = match *row {
        Row::Tag(t, _) => {
            let t = t.to_string();
            lists::item(el, v.p, on, move |s: &mut App, tx| {
                s.search_tag(&t, tx);
                if tx.by_keys() {
                    if let Some(k) = s.results().and_then(|r| r.tagged.first().map(|p| Row::Tagged(p).key())) {
                        tx.focus(k, true);
                    }
                }
            })
        }
        _ => {
            let path = row.note().unwrap_or_default().to_string();
            let at = match row {
                Row::Text(h) => Some((h.first.pos, h.first.line)),
                _ => None,
            };
            lists::opener(el, v.p, on, move |s: &mut App, tx| s.open_result(&path, at, tx))
        }
    };
    if !first {
        return el;
    }
    el.on_key(|_: &mut App, ev: &rune::KeyEvent, tx| {
        let up = ev.key == rune::Key::Up && ev.mods == rune::Mods::default();
        if up {
            tx.focus("search", false);
        }
        up
    })
}

/// A note's two rows: its name over its folder, or under a text result over the line of its
/// first match. A note with a tab says " · open" (SB-61, SB-63).
fn note_row(v: V, key: &str, path: &str, (name, marks): (String, Vec<Span>), line: Option<(String, Vec<Span>)>, on: bool) -> El {
    let s = v.s;
    let c = Roles::panel(v.p);
    let small = caption_style();
    let title = rich(name, marks).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().w_full();
    let open = s.tabs.iter().any(|t| t == path);
    let caption = match line {
        Some((t, marks)) => rich(t, marks).font(small).color(c.faint).truncate().shrink().min_w(0.0),
        None => {
            let dir = md::dir(path);
            let folder = if dir.is_empty() { s.vault.as_ref().map_or_else(String::new, |v| v.name.clone()) } else { dir.to_string() };
            let room = last_part(&folder, small);
            text(folder).font(small).color(c.faint).truncate_head().shrink().min_w(room)
        }
    };
    let caption = row().w_full().child(caption).children(open.then(|| text(" · open").font(small).color(c.faint)));
    let el = two_rows(key, title, caption).pr(BUTTON_END + 4.0).role("button").label(md::base(path).to_string());
    if on { el.fill(c.selected) } else { el.hover(c.hover) }
}

/// A tag's row: the tag icon, the tag with the query's letters marked, and how many notes
/// carry it (SB-62).
fn tag_row(v: V, key: &str, tag: &str, n: usize, q: &str, on: bool) -> El {
    let c = Roles::panel(v.p);
    let (name, marks) = marked(tag, q, &c);
    let notes = format!("{n} {}", if n == 1 { "note" } else { "notes" });
    let el = row()
        .key(key)
        .h(panel::row_h(panel::BODY))
        .mx(panel::INSET)
        .pl(panel::TEXT_X - panel::INSET)
        .pr(BUTTON_END + 4.0)
        .gap(6.0)
        .center_y()
        .radius(super::parts::RADIUS)
        .role("button")
        .label(format!("#{tag}"))
        .child(icon(&TAG, 12.0).color(c.faint))
        .child(rich(name, marks).font(nav(panel::BODY, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0))
        .child(text(notes).font(sans(10.0, 500.0)).color(c.faint));
    if on { el.fill(c.selected) } else { el.hover(c.hover) }
}

/// `name` with the first place it holds the lowered query `q` in the accent as text, where
/// lowering keeps its offsets.
fn marked(name: &str, q: &str, c: &Roles) -> (String, Vec<Span>) {
    let low = name.to_lowercase();
    let at = (!q.is_empty() && low.len() == name.len()).then(|| low.find(q)).flatten();
    let marks = at.map(|i| vec![Span { color: Some(c.accent_text), ..Span::new(i, i + q.len()) }]).unwrap_or_default();
    (name.to_string(), marks)
}

/// A text result's line: its first match's line as text, cut at words to one caption line,
/// the match in the accent as text (SB-61, `md::snippet`).
fn snip(v: V, h: &crate::search::TextHit, len: usize) -> (String, Vec<Span>) {
    let c = Roles::panel(v.p);
    let line = v.s.files().get(&h.path).and_then(|t| t.split('\n').nth(h.first.line)).unwrap_or("");
    let (snip, m) = md::snippet(line, h.first.col, len, SNIP_BEFORE, SNIP_COLS, 1);
    let marks = if m.is_empty() { Vec::new() } else { vec![Span { color: Some(c.accent_text), ..Span::new(m.start, m.end) }] };
    (snip, marks)
}

/// A snippet's room: at most this many characters ahead of the match, and about the
/// characters a caption line holds. The caption cuts what still does not fit.
const SNIP_BEFORE: usize = 12;
const SNIP_COLS: usize = 36;
