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
use super::super::styles::nav;
use super::super::icons::TAG;
use super::parts::{COUNT_TEXT, GAP, INSET, RADIUS, ROW_H, ROW_PX, ROW_TEXT, TAB_TITLE_LH, TEXT_X, caption_style, section_label, two_rows};
use super::tree::last_part;
use super::kbd;
use crate::app::App;
use crate::md;
use crate::roles::Roles;
use crate::search::{Results, Row};

/// The results list, built again only when something it draws from changes: the results,
/// which follow the query and the notes, the filled row, the tabs, the vault's name, the
/// theme and the panel font. A frame that changes none of them, a keystroke in the writer
/// while the results show, reuses the rows, however many there are.
pub(super) fn results(v: V, r: &Results) -> El {
    let s = v.s;
    let version = (
        s.vault.as_ref().map(|x| (x.id(), x.writes(), &x.name)),
        &r.query,
        &s.res_focus,
        &s.tabs,
        format!("{:?}", v.p),
        &s.settings().panel_font,
    );
    rune::memo("results", version, || list(v, r))
}

/// The list itself, one Tab stop that enters at the filled row (SB-81).
fn list(v: V, r: &Results) -> El {
    let c = Roles::panel(v.p);
    let mut out = lists::list(col().key("results").w_full().gap(GAP), "list");
    let Some(first) = r.first() else {
        return out.child(text("No matches").font(nav(panel::BODY_SMALL, false)).color(c.faint).py(6.0).pl(TEXT_X).pr(TEXT_X));
    };
    let first_key = first.key();
    let filled = v.s.res_focus.clone().filter(|k| r.rows().any(|row| row.key() == *k)).unwrap_or(first_key.clone());
    let ql = r.query.to_lowercase();
    let tag_q = ql.strip_prefix('#').map(|t| t.trim().to_string());
    let section = |key: &str, name: &str, n: usize| section_label(v.p, key, name, n.to_string());
    let vault = v.s.vault.as_ref();
    let look = (format!("{:?}", v.p), &v.s.settings().panel_font, vault.map(|x| &x.name), &r.query);
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
        let first = key == first_key;
        // Each row is built again only when what it draws changes, so an edit to one note
        // builds that note's row and no other.
        let note = row.note();
        let what = match row {
            Row::Text(h) => (h.count, h.first.pos),
            Row::Tag(_, n) => (n, 0),
            _ => (0, 0),
        };
        let version = (&look, on, first, what, note.map(|p| (vault.map_or(0, |x| x.ver(p)), v.s.tabs.iter().any(|t| t == p))));
        let k = key.clone();
        out = out.child(rune::memo(key, version, || {
            let el = match row {
                Row::Title(p) => note_row(v, &k, p, marked(md::base(p), &ql, &c), None, on),
                Row::Tagged(p) => note_row(v, &k, p, (md::base(p).to_string(), Vec::new()), None, on),
                Row::Text(h) => note_row(v, &k, &h.path, (md::base(&h.path).to_string(), Vec::new()), Some(snip(v, h, ql.len())), on),
                Row::Tag(t, n) => tag_row(v, &k, t, n, tag_q.as_deref().unwrap_or(&ql), on),
            };
            let el = if on { el.child(kbd(v, "Enter".into())) } else { el };
            choose(v, el, k.clone(), on, first)
        }));
    }
    out
}

/// A result row's keys: a click, Enter or Space chooses it (`App::choose_result`). Esc goes
/// by `App::search_escape`, so it clears the search and lands in the field (SB-90); Up from
/// the first row goes back to the field (SB-66). A row that goes away with the keyboard, as a
/// note edited out of the results does, hands it to the field (SB-72).
fn choose(v: V, el: El, key: String, on: bool, first: bool) -> El {
    let k = key.clone();
    lists::item(el, v.p, on, move |s: &mut App, tx| s.choose_result(&k, tx)).focus_return("search").on_key(move |s: &mut App, ev: &rune::KeyEvent, tx| {
        if crate::keys::esc_again(ev, tx) {
            return true;
        }
        if ev.mods != rune::Mods::default() {
            return false;
        }
        match ev.key {
            rune::Key::Escape => s.search_escape(tx),
            rune::Key::Up if first => {
                tx.focus("search", false);
                true
            }
            _ => false,
        }
    })
}

/// A note's two rows: its name over its folder, or under a text result over the line of its
/// first match. A note with a tab says " · open" (SB-61, SB-63).
fn note_row(v: V, key: &str, path: &str, (name, marks): (String, Vec<Span>), line: Option<(String, Vec<Span>)>, on: bool) -> El {
    let s = v.s;
    let c = Roles::panel(v.p);
    let small = caption_style();
    let title = rich(name, marks).font(nav(ROW_TEXT, false).lh(TAB_TITLE_LH)).color(if on { c.text } else { c.dim }).truncate().w_full();
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
    let el = two_rows(key, title, caption).pr(ROW_PX).gap(7.0).role("button").label(md::base(path).to_string());
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
        .h(ROW_H)
        .mx(INSET)
        .px(ROW_PX)
        .gap(7.0)
        .center_y()
        .radius(RADIUS)
        .role("button")
        .label(format!("#{tag}"))
        .child(icon(&TAG, 12.0).color(c.faint))
        .child(rich(name, marks).font(nav(ROW_TEXT, false)).color(if on { c.text } else { c.dim }).truncate().grow(1.0).shrink().min_w(0.0))
        .child(text(notes).font(nav(COUNT_TEXT, false)).color(c.faint));
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
