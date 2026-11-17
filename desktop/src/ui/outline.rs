//! The right panel: the outline, backlinks and, at its foot, the status line with the word
//! count and reading time.

use rune::{El, Live, ScrollSpec, col, el, row, text};

use super::{V, panel};
use super::editor::{group, link_at_caret};
use super::widgets::label;
use crate::app::{App, Mode};
use crate::keys::mod_click;
use crate::md;
use crate::roles::Roles;
use crate::theme::mono;

/// The panel's width, and the status line's side padding.
const STATUS_W: f32 = 236.0;
const STATUS_X: f32 = 16.0;

pub fn outline(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let c = Roles::new(p);
    let doc = s.active_text();
    let heads = md::headings(doc);
    let caret = s.active.as_ref().and_then(|a| s.edits.get(a)).map_or(0, |e| e.focus.min(doc.len()));
    let (line, _) = md::line_col(doc, caret);
    // omarky lists h1 to h4. Under a deeper heading, the h1 to h4 above it is current.
    let shown: Vec<usize> = (0..heads.len()).filter(|&i| heads[i].level <= 4).collect();
    let row_of = |i: usize| shown.iter().rposition(|&j| j <= i);
    // In the reader the scroll handler moves the highlight (`reader::PageMap::mark`), so a
    // scroll runs no view (VER-32).
    let mut map = s.reader.map.borrow_mut();
    let live = map.reading;
    let cur = if s.mode == Mode::Source {
        heads.iter().rposition(|h| h.line <= line).and_then(row_of)
    } else if live && !heads.is_empty() {
        let blocks = s.doc();
        map.heads = heads
            .iter()
            .enumerate()
            .filter_map(|(i, h)| Some((blocks.blocks.iter().position(|b| b.lines.start == h.line)?, row_of(i))))
            .collect();
        map.first_row = row_of(0);
        let cur = map.head_row(s.reader.scroll.get()[0]);
        map.marked = cur;
        cur
    } else {
        None
    };
    let min_lv = shown.iter().map(|&i| heads[i].level).min().unwrap_or(1);
    let mut list = col().key("outline-list").w_full().pb(12.0).child(panel::caption("Contents", &c).pl(panel::CAPTION_X).pt(panel::CAPTION_Y).pb(panel::CAPTION_GAP));
    for (n, &i) in shown.iter().enumerate() {
        let h = &heads[i];
        let on = cur == Some(n);
        let depth = h.level - min_lv;
        let line = h.line;
        let text_ = if h.text.is_empty() { "Untitled".to_string() } else { h.text.clone() };
        let size = if depth == 0 { panel::BODY } else { panel::BODY_SMALL };
        let mut name = text(text_).font(panel::row_font(depth)).color(panel::ink(&c, depth, on)).truncate().shrink().min_w(0.0);
        let mut shade = panel::shade(format!("ol-{i}"), size, depth, 0.0, &c, on);
        if live {
            let mark = crate::reader::Mark {
                on: [panel::shade_fill(&c, true), panel::ink(&c, depth, true)],
                off: [panel::shade_fill(&c, false), panel::ink(&c, depth, false)],
                fill: Live::new(panel::shade_fill(&c, on)),
                ink: Live::new(panel::ink(&c, depth, on)),
            };
            shade = shade.live_fill(&mark.fill);
            name = name.live_text_color(&mark.ink);
            map.marks.push(mark);
        }
        list = list.child(shade.role("button").on_press_tx(move |s: &mut App, tx| s.jump(line, tx)).child(name));
    }
    if shown.is_empty() {
        list = list.child(text("No headings in this note.").font(panel::font(panel::BODY_SMALL, false)).color(c.faint).pl(panel::TEXT_X));
    }
    list = list.child(panel::caption("Linked from", &c).pl(panel::CAPTION_X).pt(panel::CAPTION_Y + 6.0).pb(panel::CAPTION_GAP));
    let mut back = s.active.as_ref().map(|a| md::backlinks(s.files(), a)).unwrap_or_default();
    if let Some(vault) = &s.vault {
        back.sort_by_key(|(p, _)| (vault.born.get(p).copied().unwrap_or(u128::MAX), p.clone()));
    }
    let name_style = panel::font(panel::BODY, false);
    let small = panel::font(panel::CAPTION, false);
    let shift = super::baseline_shift(name_style, small);
    for (i, (bp, _)) in back.iter().enumerate() {
        let path = bp.clone();
        list = list.child(
            panel::shade(format!("bl-{i}"), panel::BODY, 0, 0.0, &c, false)
                .gap(8.0)
                .hover_group()
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, None, tx))
                .child(text(md::base(bp)).font(name_style).color(c.dim).group_color(c.text).truncate().shrink().min_w(0.0))
                .child(text(md::dir(bp)).font(small).color(c.faint).truncate().grow(1.0).shrink().min_w(0.0).mt(shift)),
        );
    }
    if back.is_empty() {
        list = list.child(text("No notes link here yet.").font(panel::font(panel::BODY_SMALL, false)).color(c.faint).pl(panel::TEXT_X));
    }
    let scroll = col().key("outline-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list);
    col()
        .key("outline")
        .w(STATUS_W)
        .h_full()
        .min_h(0.0)
        .fill(c.wash)
        .border_sides([0.0, 0.0, 0.0, 1.0], c.hairline_soft)
        .border_layout()
        .child(scroll)
        .child(status(v))
}

/// The status line at the panel's foot, centred, its parts split by dots: in the writer the
/// caret's line and column, or how many characters are selected; then the word count and
/// reading time together (WR-189), and the sync state. Parts that don't fit go on a centred line
/// below, so no line starts or ends with a dot. The link under the caret shows on a line above.
fn status(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let doc = s.active_text();
    let style = mono(11.0, 400.0);
    let edit = (s.mode == Mode::Source).then(|| s.active.as_ref().and_then(|a| s.edits.get(a))).flatten();
    let (a, b) = edit.map_or((0, 0), |e| (e.anchor.min(e.focus).min(doc.len()), e.anchor.max(e.focus).min(doc.len())));
    let sel = &doc[a..b];
    // Each part with its width: mono labels are 0.6 em a character (`widgets::label`).
    let w = |t: &str| t.chars().count() as f32 * 0.6 * style.size;
    let mut parts: Vec<(El, f32)> = Vec::new();
    let mut link = None;
    if let Some(e) = edit {
        let pos = e.focus.min(doc.len());
        let (line, ls) = md::line_col(doc, pos);
        let at = if sel.is_empty() {
            format!("Ln {}, Col {}", line + 1, doc[ls..pos].chars().count() + 1)
        } else {
            format!("{} selected", group(sel.chars().count()))
        };
        parts.push((label(at.clone(), style, p.mute).key("caret-pos"), w(&at)));
        link = link_at_caret(doc, pos).map(|t| {
            let verb = if s.names.resolve(&t).is_some() { " to open “" } else { " to create “" };
            text(format!("{}{verb}{t}”", mod_click())).font(style).color(p.acc).truncate().min_w(0.0).max_w(STATUS_W - 2.0 * STATUS_X)
        });
    }
    // The writer's count (WR-188): front matter, code fences and link targets left out.
    let count = s.word_count();
    let wc = count.words;
    let words = if sel.is_empty() {
        format!("{} {}", group(wc), if wc == 1 { "word" } else { "words" })
    } else {
        format!("{} of {} words", group(crate::writer::count_words(sel)), group(wc))
    };
    let read = format!("{} min read", count.minutes);
    let dot = || label("·", style, p.mark).mx(6.0);
    let sep = w("·") + 12.0;
    // The count and the reading time stay together on one line.
    let counts = row()
        .key("counts")
        .center_y()
        .child(label(words.clone(), style, p.mute).key("word-count").help(format!("{} characters", group(count.chars))))
        .child(dot())
        .child(label(read.clone(), style, p.mute).key("read-time"));
    parts.push((counts, w(&words) + sep + w(&read)));
    if let Some((lbl, busy, failed)) = s.sync_label() {
        let dot = if failed { p.err } else if busy { p.acc } else { p.mark };
        let width = 12.0 + w(&lbl);
        let mut sync = row()
            .key("sync-state")
            .role("button")
            .on_press(|s: &mut App| {
                s.open_settings();
                s.set_sec = crate::app::Sec::Sync;
            })
            .gap(6.0)
            .center_y()
            .child(el().size(6.0, 6.0).radius(3.0).fill(dot))
            .child(label(lbl, style, if failed { p.err } else { p.mute }));
        if let Some(e) = &s.sync.error {
            sync = sync.help(e.clone());
        }
        parts.push((sync, width));
    }
    let room = STATUS_W - 2.0 * STATUS_X;
    let mut lines: Vec<(Vec<El>, f32)> = Vec::new();
    for (part, pw) in parts {
        match lines.last_mut() {
            Some((line, used)) if *used + sep + pw <= room => {
                line.push(dot());
                line.push(part);
                *used += sep + pw;
            }
            _ => lines.push((vec![part], pw)),
        }
    }
    let lines = lines.into_iter().enumerate().map(|(i, (line, _))| row().key(format!("status-line-{i}")).center_y().children(line));
    col()
        .key("status")
        .w_full()
        .min_h(29.0)
        .py(7.0)
        .px(STATUS_X)
        .gap(4.0)
        .center()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .border_layout()
        .clip_rounded(0.0)
        .children(link)
        .children(lines)
}
