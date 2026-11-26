//! The right panel: the outline and backlinks, the word count and reading time under them, and
//! in the writer a footer with the caret's line and column.

use rune::{El, Live, ScrollSpec, col, row, text};

use super::{V, lists, panel, styles};
use super::editor::{CaretLink, group, link_at_caret};
use super::components::{label, name_tip};
use crate::app::{App, Mode};
use crate::keys::mod_click;
use crate::md;
use crate::roles::Roles;

/// The panel's width, and the side padding of its counts and footer.
pub(super) const PANEL_W: f32 = super::tokens::size::OUTLINE_W;
const PANEL_X: f32 = 16.0;

/// Section caption matching the left sidebar's OPEN / NOTES headings (10 pt Instrument Sans 600).
fn section_caption(title: &str, c: &Roles) -> El {
    text(title.to_uppercase()).font(styles::sans(panel::CAPTION, 600.0).lh(panel::CAPTION).tracking(1.0)).color(c.faint)
}

pub fn outline(v: V) -> El {
    use std::hash::{Hash, Hasher};
    let p = v.p;
    let s = v.s;
    let c = Roles::panel(p);
    let doc = s.active_text();
    let (heads, _, heads_ver) = s.headings_cached();
    let edit = s.active.as_ref().and_then(|a| s.edits.get(a));
    let pos = edit.map_or(0, |e| e.focus.min(doc.len()));
    let line_col = (s.mode == Mode::Source).then(|| md::line_col(doc, pos));
    // omarky lists h1 to h4. Under a deeper heading, the h1 to h4 above it is current.
    let shown: Vec<usize> = (0..heads.len()).filter(|&i| heads[i].level <= 4).collect();
    let row_of = |i: usize| shown.iter().rposition(|&j| j <= i);
    // In both reader and writer the scroll handler moves the highlight (`reader::PageMap::mark`),
    // so a scroll runs no view (VER-32). In source mode, caret moves also update the live mark
    // without rebuilding the outline memo.
    let (live, reading, outline_gen) = {
        let mut map = s.reader.map.borrow_mut();
        let caret_key = (s.doc_hash(), pos);
        if map.writing && map.source_caret != Some(caret_key) {
            map.source_caret = Some(caret_key);
            map.source_from_caret = true;
            if let Some((line, _)) = line_col {
                map.mark_line(line);
            }
        }
        (map.reading || map.writing, map.reading, map.outline_gen)
    };
    // On the map the row of the heading holding the selected node is marked (MAP-067).
    let on_map = super::mapping(s).then(|| s.map_heading_line(4).and_then(|l| shown.iter().position(|&j| heads[j].line == l)));
    let pal_id = p as *const crate::theme::Pal as usize;
    let panel_font = &s.settings().panel_font;
    let heads_version = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (heads_ver, live, reading, outline_gen, if reading { s.doc_hash() } else { None }, pal_id, panel_font, on_map).hash(&mut h);
        h.finish()
    };
    let contents = rune::memo("outline-heads", heads_version, || {
        let mut map = s.reader.map.borrow_mut();
        let cur = if live {
            map.outline.clear();
            if !heads.is_empty() {
                if map.reading {
                    let blocks = s.doc();
                    map.outline.heads = heads
                        .iter()
                        .enumerate()
                        .filter_map(|(i, h)| {
                            let k = blocks.blocks.partition_point(|b| b.lines.start < h.line);
                            (blocks.blocks.get(k).is_some_and(|b| b.lines.start == h.line)).then_some((k, row_of(i)))
                        })
                        .collect();
                } else {
                    map.outline.heads = heads.iter().enumerate().map(|(i, h)| (h.line, row_of(i))).collect();
                }
                map.outline.first_row = row_of(0);
                let cur = if map.writing && map.source_from_caret {
                    crate::reader::PageMap::head_row_line(&map.outline, line_col.map_or(0, |(l, _)| l))
                } else {
                    map.head_row(&map.outline, s.reader.scroll.get())
                };
                map.outline.marked = cur;
                cur
            } else {
                None
            }
        } else {
            on_map.flatten()
        };
        let min_lv = shown.iter().map(|&i| heads[i].level).min().unwrap_or(1);
        let mut contents = lists::list(col().w_full().child_exit(rune::Transition::none()), "list");
        for (n, &i) in shown.iter().enumerate() {
            let h = &heads[i];
            let on = cur == Some(n);
            let depth = h.level - min_lv;
            let line = h.line;
            let text_ = if h.text.is_empty() { "Untitled".to_string() } else { h.text.clone() };
            let size = if depth == 0 { panel::BODY } else { panel::BODY_SMALL };
            // A heading cut short offers its whole text; one shown in full has no tooltip (C7).
            let mut name = text(text_.clone()).font(panel::row_font(depth)).color(panel::ink(&c, depth, on)).truncate().shrink().min_w(0.0).help_when_truncated(text_.clone());
            let mut shade = panel::shade(format!("ol-{i}"), size, depth, 0.0, &c, on).label(text_);
            // The current row is marked by its shade and its ink, nothing else (RD-47).
            if live {
                let mark = crate::reader::Mark {
                    on: [panel::shade_fill(&c, true), panel::ink(&c, depth, true)],
                    off: [panel::shade_fill(&c, false), panel::ink(&c, depth, false)],
                    fill: Live::new(panel::shade_fill(&c, on)),
                    ink: Live::new(panel::ink(&c, depth, on)),
                };
                shade = shade.live_fill(&mark.fill);
                name = name.live_text_color(&mark.ink);
                map.outline.marks.push(mark);
            }
            contents = contents.child(lists::opener(shade.role("button"), v.p, on, move |s: &mut App, tx| s.jump(line, tx)).child(name));
        }
        contents
    });
    let mut list = col().key("outline-list").w_full().pb(12.0).child_exit(rune::Transition::none()).child(section_caption("Contents", &c).pl(panel::TEXT_X).pt(panel::CAPTION_Y).pb(panel::CAPTION_GAP)).child(contents);
    if shown.is_empty() {
        list = list.child(text("No headings in this note.").font(styles::nav(panel::BODY_SMALL, false)).color(c.faint).pl(panel::TEXT_X));
    }
    list = list.child(section_caption("Linked from", &c).pl(panel::TEXT_X).pt(panel::CAPTION_Y + 6.0).pb(panel::CAPTION_GAP));
    let (back, back_ver) = s.backlinks_cached();
    let links_version = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (back_ver, pal_id, panel_font).hash(&mut h);
        h.finish()
    };
    let links = rune::memo("outline-links", links_version, || {
        let name_style = styles::nav(panel::BODY, false);
        let small = styles::nav(panel::CAPTION, false);
        let shift = super::baseline_shift(name_style, small);
        let mut links = lists::list(col().w_full().gap(2.0), "list");
        for (i, bl) in back.iter().enumerate() {
            let bp = &bl.path;
            let path = bp.clone();
            let mut head = row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(text(md::base(bp)).font(name_style).color(c.dim).group_color(c.text).truncate().shrink().min_w(0.0).help_when_truncated(name_tip(bp)))
                .child(text(md::dir(bp)).font(small).color(c.faint).truncate().grow(1.0).shrink().min_w(0.0).mt(shift).help_when_truncated(name_tip(bp)));
            if bl.count > 1 {
                head = head.child(super::components::count_pill(v.p, format!("bl-count-{i}"), bl.count));
            }
            let mut item_col = col().w_full().gap(2.0).child(head);
            if !bl.snippet.is_empty() {
                item_col = item_col.child(
                    text(bl.snippet.clone())
                        .key(format!("backlink-snip:{}", bp.replace('/', "|")))
                        .font(small)
                        .color(c.faint)
                        .truncate()
                        .w_full()
                        .help_when_truncated(bl.snippet.clone()),
                );
            }
            let shade = col()
                .key(format!("bl-{i}"))
                .mx(panel::INSET)
                .px(panel::TEXT_X - panel::INSET)
                .py(5.0)
                .radius(6.0)
                .hover(c.hover)
                .hover_group()
                .role("button")
                .label(md::base(bp).to_string())
                .child(item_col);
            links = links.child(lists::opener(shade, v.p, false, move |s: &mut App, tx| s.open_file(&path, None, tx)));
        }
        links
    });
    list = list.child(links);
    if back.is_empty() {
        list = list.child(text("No notes link here yet.").font(styles::nav(panel::BODY_SMALL, false)).color(c.faint).pl(panel::TEXT_X));
    }
    let scroll = col()
        .key("outline-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_fade(rune::ScrollFade::strength(super::tokens::space::SCROLL_FADE_PANEL).full_at(1.0).linear())
        .scroll_margin(panel::RING_ROOM)
        .child(list);
    super::focus::pane(col(), crate::focus::Pane::Outline)
        .key("outline")
        .w(PANEL_W)
        .h_full()
        .min_h(0.0)
        .fill(c.wash)
        .border_sides([0.0, 0.0, 0.0, 1.0], v.p.line)
        .border_layout()
        .child(scroll)
        .child(counts(v))
        .children(footer(v, line_col))
}

/// The word count and reading time, left-aligned under the outline's list at the same inset as
/// the footer when it shows (WR-189). The writer's count (WR-188): front matter, code fences
/// and link targets left out.
fn counts(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let c = Roles::panel(p);
    let doc = s.active_text();
    let style = styles::nav(panel::BODY_SMALL, false);
    let sel_range = (s.mode == Mode::Source)
        .then(|| s.active.as_ref().and_then(|a| s.edits.get(a)))
        .flatten()
        .map(|e| (e.anchor.min(e.focus).min(doc.len()), e.anchor.max(e.focus).min(doc.len())))
        .filter(|&(a, b)| a < b);
    let count = s.word_count();
    let wc = count.words;
    let words = match sel_range {
        None => format!("{} {}", group(wc), if wc == 1 { "word" } else { "words" }),
        Some((a, b)) => format!("{} of {} words", group(crate::writer::count_words_range(doc, a, b)), group(wc)),
    };
    let mut r = row()
        .key("counts")
        .w_full()
        .px(PANEL_X)
        .center_y()
        .child(label(words, style, c.faint).key("word-count").help(format!("{} characters", group(count.chars))))
        .child(label("·", style, c.faint).mx(6.0))
        .child(label(format!("{} min read", count.minutes), style, c.faint).key("read-time"));
    if s.mode == Mode::Source {
        r = r.py(10.0);
    } else {
        // In preview there is no status footer beneath counts; match side-foot's 41 pt height
        // and top hairline so the left and right panels align across the window's foot.
        r = r.h(super::tokens::size::FOOTER_H).border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout();
    }
    r
}

/// The writer's footer, one line of fixed height: the caret's line and column, or how many
/// characters are selected, and the link under the caret. None in preview.
fn footer(v: V, line_col: Option<(usize, usize)>) -> Option<El> {
    let p = v.p;
    let s = v.s;
    if s.mode != Mode::Source {
        return None;
    }
    let c = Roles::panel(p);
    let doc = s.active_text();
    let style = styles::nav(panel::BODY_SMALL, false);
    let edit = s.active.as_ref().and_then(|a| s.edits.get(a));
    let (a, b) = edit.map_or((0, 0), |e| (e.anchor.min(e.focus).min(doc.len()), e.anchor.max(e.focus).min(doc.len())));
    let pos = edit.map_or(0, |e| e.focus.min(doc.len()));
    let (line, ls) = line_col.unwrap_or_else(|| md::line_col(doc, pos));
    let at = if a == b {
        format!("Ln {}, Col {}", line + 1, doc[ls..pos].chars().count() + 1)
    } else {
        format!("{} selected", group(doc[a..b].chars().count()))
    };
    let mut r = row()
        .key("status")
        .h(super::tokens::size::FOOTER_H)
        .w_full()
        .gap(12.0)
        .px(PANEL_X)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .border_layout()
        .clip_rounded(0.0)
        .child(label(at, style, c.faint).key("caret-pos"));
    match link_at_caret(doc, pos) {
        Some(CaretLink::Wiki(t)) => {
            let (name, frag) = crate::md::wiki_parts(&t);
            let label = if name.is_empty() {
                format!("{} to jump to “#{}”", mod_click(), frag.unwrap_or_default())
            } else if s.names.resolve(&name).is_some() {
                format!("{} to open “{t}”", mod_click())
            } else {
                format!("{} to create “{name}”", mod_click())
            };
            r = r.child(text(label).font(style).color(c.accent_text).truncate().shrink().min_w(0.0));
        }
        Some(CaretLink::Url(u)) => {
            r = r.child(text(format!("{} to open {u}", mod_click())).font(style).color(c.accent_text).truncate().shrink().min_w(0.0));
        }
        None => {}
    }
    Some(r)
}
