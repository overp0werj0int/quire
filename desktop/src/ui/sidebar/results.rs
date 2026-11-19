//! What replaces the two lists while the search field holds text, or while the tags show.
//! Today's search results and tags list, until stage 3 of the sidebar's plan rebuilds them
//! (SB-57 to SB-77).

use rune::{El, Span, col, el, rich, text};

use super::super::V;
use super::super::lists;
use super::super::panel;
use super::super::styles::nav;
use super::W;
use crate::app::{App, Sel};
use crate::md;
use crate::roles::{Roles, mix};

/// The notes that hold the query, each with its first four hits, under a count.
pub(super) fn results(v: V) -> El {
    let p = v.p;
    let c = Roles::panel(p);
    let s = v.s;
    let q = s.query.text.trim().to_string();
    let results = md::search(s.files(), &q);
    let total: usize = results.iter().map(|r| r.hits.len()).sum();
    let summary = if results.is_empty() {
        "No matches".to_string()
    } else {
        format!("{total} {} in {} {}", if total == 1 { "match" } else { "matches" }, results.len(), if results.len() == 1 { "note" } else { "notes" })
    };
    let out = col().key("search-panel").w_full().child(text(summary).font(nav(panel::BODY_SMALL, false)).color(c.faint).wrap().w_full().pt(6.0).pl(panel::TEXT_X).pr(panel::TEXT_X).pb(4.0));
    let ql = q.len();
    let mut found = lists::list(col().key("search-results").w_full(), "list");
    for (ri, r) in results.iter().enumerate() {
        let lines: Vec<&str> = s.files().get(&r.path).map(|t| t.split('\n').collect()).unwrap_or_default();
        // A match opens in the writer with the caret at its start and nothing selected, as find
        // leaves it, so the next key types there rather than replacing it.
        let first = r.hits.first().map(|h| Sel { start: h.pos, end: h.pos, line: Some(h.line), force_source: true });
        let path = r.path.clone();
        let name_style = nav(panel::BODY, true);
        let small = nav(panel::CAPTION, false);
        let shift = super::super::baseline_shift(name_style, small);
        let mut head = panel::shade(format!("res:{}", r.path), panel::BODY, 0, 0.0, &c, false).gap(8.0).role("button").label(md::base(&r.path).to_string());
        if ri == 0 {
            // Up from the first result goes back to the field (FOCUS).
            head = head.on_key(|_: &mut App, ev: &rune::KeyEvent, tx| {
                let up = ev.key == rune::Key::Up && ev.mods == rune::Mods::default();
                if up {
                    tx.focus("search", false);
                }
                up
            });
        }
        let mut block = col().key(format!("res-block:{}", r.path)).py(4.0).child(
            lists::opener(head, p, false, move |s: &mut App, tx| s.open_file(&path, first, tx))
                .child(text(md::base(&r.path)).font(name_style).color(c.dim).truncate().shrink())
                .child(text(md::dir(&r.path)).font(small).color(c.faint).truncate_head().grow(1.0).shrink().min_w(super::tree::last_part(md::dir(&r.path), small)).mt(shift))
                .child(text(if r.hits.is_empty() { String::new() } else { r.hits.len().to_string() }).font(small).color(c.faint).mt(shift)),
        );
        for (hi, h) in r.hits.iter().take(4).enumerate() {
            let ln = lines.get(h.line).copied().unwrap_or("");
            // The line as text, cut at words to two of the panel's lines (C17).
            let (snip, m) = md::snippet(ln, h.col, ql, SNIP_BEFORE, SNIP_COLS, 2);
            let (text_, spans) = if m.is_empty() {
                (snip, Vec::new())
            } else {
                // `<mark>` has 1 px of padding on each side: a mono space 1 px wide takes it.
                let text_ = format!("{} {} {}", &snip[..m.start], &snip[m.clone()], &snip[m.end..]);
                let (m0, m1) = (m.start, m.end + 2);
                let pad = Span { mono: true, size: Some(1.0 / 0.6), bg: Some(p.soft), ..Span::default() };
                let spans = vec![
                    Span { start: m0 as u32, end: (m0 + 1) as u32, ..pad },
                    Span { bg: Some(p.soft), color: Some(p.ink), ..Span::new(m0 + 1, m1 - 1) },
                    Span { start: (m1 - 1) as u32, end: m1 as u32, ..pad },
                ];
                (text_, spans)
            };
            let path = r.path.clone();
            let sel = Sel { start: h.pos, end: h.pos, line: Some(h.line), force_source: true };
            let hit = el().key(format!("hit:{}:{hi}", r.path)).label(snip_label(ln)).mx(panel::INSET).pl(panel::TEXT_X - panel::INSET).pr(panel::RIGHT - panel::INSET).py(4.0).radius(3.0).hover(c.hover).role("button");
            block = block.child(
                lists::opener(hit, p, false, move |s: &mut App, tx| s.open_file(&path, Some(sel), tx))
                    .child(rich(text_, spans).font(nav(panel::BODY_SMALL, false).lh(16.0)).color(mix(c.dim, c.faint, 0.5)).max_lines(2).w_full()),
            );
        }
        if r.hits.len() > 4 {
            block = block.child(text(format!("+ {} more", r.hits.len() - 4)).font(nav(panel::CAPTION, false)).color(c.faint).py(2.0).pl(panel::TEXT_X));
        }
        found = found.child(block);
    }
    out.child(found)
}

/// The key of the first result, where Down from the search field goes.
pub(super) fn first_result(v: V) -> Option<String> {
    let q = v.s.query.text.trim();
    if q.is_empty() {
        return None;
    }
    md::search(v.s.files(), q).into_iter().next().map(|r| format!("res:{}", r.path))
}

/// A hit's name for screen readers and type-select: its line, without the Markdown marks
/// that lead it.
fn snip_label(line: &str) -> String {
    line.trim_start_matches(|ch: char| ch == '#' || ch == '>' || ch == '-' || ch == '*' || ch.is_whitespace()).to_string()
}

/// A search snippet's room: at most this many characters ahead of the match, and the characters
/// a line of the panel holds at 0.6 em a character.
const SNIP_BEFORE: usize = 16;
const SNIP_COLS: usize = ((W - panel::TEXT_X - panel::RIGHT) / (panel::BODY_SMALL * 0.6)) as usize - 1;

/// Every tag with its note count. Choosing one searches for it.
pub(super) fn tags(v: V) -> El {
    let c = Roles::panel(v.p);
    let tags = md::tags(v.s.files());
    let mut out = lists::list(col().key("tags").w_full(), "list");
    if tags.is_empty() {
        return out.child(text("No tags yet. Write #something in a note.").font(nav(panel::BODY_SMALL, false)).color(c.faint).py(6.0).pl(panel::TEXT_X).pr(panel::RIGHT).wrap());
    }
    for (tag, n) in tags {
        let t = tag.clone();
        let shade = panel::shade(format!("tag-row:{tag}"), panel::BODY, 0, 0.0, &c, false).justify_between().role("button").label(tag.clone());
        out = out.child(
            lists::item(shade, v.p, false, move |s: &mut App, tx| {
                s.search_tag(&t, tx);
                // From the keyboard, the keyboard follows the tag to its notes.
                if tx.by_keys() {
                    tx.focus("search-results", true);
                }
            })
            .child(text(format!("#{tag}")).font(nav(panel::BODY, false)).color(c.accent))
            .child(text(n.to_string()).font(nav(panel::CAPTION, false)).color(c.faint)),
        );
    }
    out
}
