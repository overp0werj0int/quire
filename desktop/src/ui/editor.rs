//! The editor body: the sheet with the note and the outline.
//!
//! Source mode works as the design does: highlighted lines draw underneath, and a text field
//! with transparent text lies over them and takes the typing. Both lay out the same text in the
//! same font at the same width, so the caret and selection land on the drawn glyphs.

use rune::{Color, El, Key, KeyEvent, ScrollFade, ScrollSpec, TextEdit, TextHighlight, TextStyle, Tx, col, el, icon, row, text, text_input};

use super::widgets::label;
use super::{V, outline, preview};
use crate::app::{App, Mode};
use crate::keymap::{self, Ctx};
use crate::roles::Roles;
use crate::md::{self, Tok};
use super::styles::{keys, literal, note, sans};
use crate::theme;

pub fn body(v: V) -> El {
    let p = v.p;
    let focus = v.focus();
    let mut sheet = col().key("sheet").grow(1.0).min_w(0.0).min_h(0.0).fill(p.bg);
    if v.has_doc() {
        // Image files dropped on the note link at the caret. Wayland gives no file drops yet,
        // so there a copied file or image goes in with paste.
        sheet = sheet
            .on_drop_files(|s: &mut App, files, tx| {
                s.drop_over = false;
                s.insert_images(files, tx);
            })
            .accepts_extensions(&md::IMAGE_EXT)
            .on_drop_hover(|s: &mut App, over, _| s.drop_over = over);
        if v.s.drop_over {
            sheet = sheet.box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: p.acc, inset: true });
        }
    }
    if !focus {
        sheet = sheet.border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout();
    }
    let mut area = col().key("doc-area").grow(1.0).min_w(0.0).min_h(0.0).h_full().child(doc_scroll(v));
    // In zen mode the progress bar runs along the window's top edge (`ui::view`).
    let progress = if focus { None } else { progress(v) };
    // Text fades under the title bar and into the sheet's foot (WR-177). The bands let presses
    // through to the text.
    let c = Roles::new(v.p);
    let pin = |top: bool| el().pin(Some(0.0), top.then_some(0.0), Some(0.0), (!top).then_some(0.0)).inert();
    let fade_top = || pin(true).key("fade-top").h(28.0).gradient(c.bg, c.bg.alpha(0.0));
    let foot = [(0.0, c.bg.alpha(0.0)), (0.6, c.bg), (1.0, c.bg)];
    let fade_foot = || pin(false).key("fade-foot").h(64.0).linear_gradient(std::f32::consts::PI, foot);
    if super::reading(v.s) {
        // The reader has the writer's fades in zen mode, where no bar sits over the page.
        if focus {
            area = area.child(fade_top()).child(fade_foot());
        }
        area = area.children(progress).child(super::reader::find_bar(v));
    } else if v.has_doc() && v.s.mode == Mode::Source && v.s.active.as_deref().is_some_and(|a| !v.s.note_locked(a)) {
        // The thumb fades over 300 ms (WR-93), and shows and hides at once under reduced motion
        // (PAGE-84).
        let shown = &v.s.reader.thumb_shown;
        let shown = if crate::motion::reduced() { shown.into() } else { shown.smooth(rune::Spring::new(0.3, 0.0)) };
        let thumb = el()
            .key("thumb")
            .pin(Some(0.0), Some(0.0), None, None)
            .radius(1.5)
            .fill(c.faint)
            .paint_order(30)
            .live_rect(&v.s.reader.thumb)
            .live_opacity(shown);
        // While the toolbar shows, its own fade does the top.
        if !shows_toolbar(v) {
            area = area.child(fade_top());
        }
        area = area.child(fade_foot());
        area = area.children(progress).child(thumb).child(super::reader::find_bar(v));
    }
    if let Some(d) = &v.s.deleted {
        area = area.child(super::reader::undo_toast(v, &d.name));
    } else if let Some((msg, _)) = &v.s.reader.toast {
        area = area.child(super::reader::toast(v, msg));
    }
    let mut inner = row().key("sheet-row").grow(1.0).min_h(0.0).w_full().child(area);
    // The outline slides in from the right and out to it over 160 ms (WR-99).
    let slide = rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::new(0.16, 0.0));
    let t = rune::Transition::move_edge(rune::Edge::Trailing).springs(slide);
    if v.right_visible() {
        // Docked, its slot closes as it leaves.
        inner = inner.child(outline::outline(v).transition(t.hold_layout()));
    }
    sheet = sheet.child(inner);
    row().key("body").grow(1.0).min_h(0.0).w_full().child(sheet)
}

/// The progress bar: in both modes the share of the note's lines above the view, full at its
/// end (RD-38, WR-178). It is drawn from the scroll in the frame it moves.
pub(super) fn progress(v: V) -> Option<El> {
    let writing = v.has_doc() && v.s.mode == Mode::Source && v.s.active.as_deref().is_some_and(|a| !v.s.note_locked(a));
    (super::reading(v.s) || writing).then(|| super::reader::progress(v))
}

/// The writer's scroll thumb (WR-93) from the viewport's [offset, range, height, width]: 3
/// wide, 4 in from the right edge, as tall as the view is of the content but at least 24, and
/// none when the content is less than 1.05 views tall.
fn thumb_rect([offset, max, view, width]: [f32; 4]) -> [f32; 4] {
    let content = max + view;
    if view <= 0.0 || content <= view * 1.05 {
        return [0.0; 4];
    }
    let h = 24f32.max(view * view / content);
    let k = if max > 0.0 { (offset / max).clamp(0.0, 1.0) } else { 0.0 };
    [width - 4.0 - 3.0, (view - h) * k, 3.0, h]
}

fn clamp(lo: f32, x: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

fn doc_scroll(v: V) -> El {
    let s = v.s;
    let focus = v.focus();
    // The page map places the reader's rows against the content's top (`reader::PageMap`).
    let mut content = col().key("doc-content").w_full().items_center().on_layout_tx(|s: &mut App, [_, y, _, _], tx| {
        s.reader.map.borrow_mut().content_laid_out(y);
        tx.live_only();
    });
    if v.has_doc() {
        let pad_x = if focus { clamp(20.0, v.vw * 0.06, 64.0) } else { clamp(20.0, v.vw * 0.05, 56.0) };
        // The toolbar floats TOOLBAR_TOP below the title bar. The note's name and folder sit in
        // the title bar, so the text starts TOOLBAR_GAP under the toolbar, or PAGE-28's room
        // under the title bar.
        let bar = shows_toolbar(v);
        let locked = s.active.as_deref().is_some_and(|a| s.note_locked(a));
        // The reader page sets its own column, gutter and room below (PAGE-21 to PAGE-29), and
        // the writer the same column measured in its own font (WR-171).
        let reader = s.mode == Mode::Preview && !locked;
        let writer = s.mode == Mode::Source && !locked;
        let page = if writer { source_column(v) } else { preview::column(v) };
        // Above the text: the toolbar's room, or with typewriter scrolling half the view
        // (WR-86, WR-87). Below it: half the view (WR-86).
        let pad_t = if focus { v.vh * 0.16 } else if bar { TOOLBAR_TOP } else { crate::page::TOP_ROOM };
        let pad_b = if reader {
            crate::page::bottom_room(v.vh)
        } else if writer {
            v.vh * 0.5
        } else if focus {
            v.vh * 0.48
        } else {
            v.vh * 0.38
        };
        let mut column = col()
            .key("doc-col")
            .w_full()
            .pt(pad_t)
            .pb(pad_b)
            .px(if reader || writer { 0.0 } else { pad_x })
            .on_press_tx(|s: &mut App, tx| {
                // A click below the text puts the caret at the end, as the design's column does.
                if s.mode == Mode::Source {
                    let e = s.cur_edit();
                    let n = e.text.len();
                    e.set_selection(n, n);
                    s.focus_editor(tx);
                }
            });
        if reader || writer {
            column = column.pl(page.gutter);
        }
        if bar {
            column = column.child(toolbar(v).transition(super::swap()));
            // A find bar too wide to sit beside the toolbar takes its own row under it, and the
            // text starts below that row.
            if find_row(v) {
                column = column.child(el().key("find-row").h(FIND_GAP + super::reader::FIND_H));
            }
            column = column.child(el().key("toolbar-gap").h(TOOLBAR_GAP));
        }
        // On a mode switch the old text and toolbar go at once and the new ones fade in from 40%
        // (WR-109), so the two texts, at their different line pitches, never show together, and
        // the page never looks empty.
        column = column.child(
            if locked {
                super::lock::panel(v)
            } else if s.mode == Mode::Source {
                source(v, page.width, page.gutter)
            } else {
                preview::preview(v).w(page.width)
            }
            .transition(super::swap()),
        );
        content = content.child(column);
    } else if s.files().is_empty() {
        content = content.min_h(v.vh - super::titlebar::BAR_H - 1.0).justify_center().padding(40.0).child(empty_vault(v));
    } else {
        content = content.min_h(v.vh - super::titlebar::BAR_H - 1.0).justify_center().padding(40.0).child(no_tab(v));
    }
    // A scroll viewport is a column: in a row its content would stretch to the viewport's
    // height and leave nothing to scroll.
    let live = s.reader.scroll.clone();
    let bar = s.reader.bar.clone();
    let map = s.reader.map.clone();
    let (thumb, shown) = (s.reader.thumb.clone(), s.reader.thumb_shown.clone());
    let mut scroll = col()
        .key("doc-scroll")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| {
            let now = [at.offset[1], at.max[1], at.viewport[1], at.viewport[0]];
            let moved = live.get()[0] != now[0];
            live.set(now);
            // The progress line (RD-38, WR-178).
            map.borrow_mut().follow(now, &bar);
            thumb.set(thumb_rect(now));
            if moved {
                // Each scroll shows the thumb for 700 ms past the last move (WR-93).
                shown.pulse(0.5, 0.0, std::time::Duration::from_millis(700));
            }
        });
    if super::reading(s) {
        // The page holds its top block in place when text above it changes (RD-11, RD-13).
        scroll = scroll.scroll_anchor();
    }
    if shows_toolbar(v) {
        // Text fades out at the top, so the toolbar, and the find bar on its own row under it,
        // float on a clear band. The caret stays below them while you type.
        let below = if find_row(v) { find_top(v) + super::reader::FIND_H } else { TOOLBAR_TOP + TOOLBAR_H };
        // The caret, and a match find shows, stay below the band's fade.
        let clear = if find_row(v) { FIND_FADE_OUT } else { 12.0 };
        scroll = scroll.scroll_fade(ScrollFade::strength(below + 8.0).with_end(ScrollFade::strength(0.0))).scroll_margin(below + clear);
    }
    scroll.child(content)
}

/// Where the find bar's top sits in the note area: 10 under the title bar, which is the
/// formatting toolbar's top when it fits beside it, else on its own row under the toolbar
/// (RD-53, WR-122).
pub(super) fn find_top(v: V) -> f32 {
    if find_row(v) {
        TOOLBAR_TOP + TOOLBAR_H + FIND_GAP
    } else {
        TOOLBAR_TOP
    }
}

/// The open find bar takes its own row under the formatting toolbar: the window is too narrow
/// for it to sit beside the toolbar with `FIND_CLEAR` between them.
fn find_row(v: V) -> bool {
    use super::reader::{FIND_RIGHT, FIND_W};
    if !v.s.reader.find.open || !shows_toolbar(v) {
        return false;
    }
    let toolbar_right = source_column(v).gutter + TOOLBAR_X + TOOLBAR_W;
    let (left, right) = docked(v);
    toolbar_right + FIND_CLEAR > v.vw - left - right - FIND_RIGHT - FIND_W
}

/// The least room between the toolbar and a find bar on its row.
const FIND_CLEAR: f32 = 16.0;
/// From the toolbar to the find bar under it.
const FIND_GAP: f32 = 8.0;
/// The fade at the foot of the band that covers a find bar on its own row: it starts
/// `FIND_FADE_IN` above the bar's foot and ends `FIND_FADE_OUT` below it, longer than a heading's
/// line, so a line crossing it dims gradually instead of showing only its lower half.
const FIND_FADE_IN: f32 = 12.0;
const FIND_FADE_OUT: f32 = 40.0;

/// The space above the floating toolbar, at rest and while it sticks.
const TOOLBAR_TOP: f32 = 10.0;
/// From the toolbar to the text.
const TOOLBAR_GAP: f32 = 24.0;
/// The height of the floating toolbar: the find bar's, so the two share one row edge to edge.
const TOOLBAR_H: f32 = super::reader::FIND_H;
/// The toolbar's width: 13 buttons of 28, 4 separators of 11, 16 gaps of 2 and 4 of padding
/// at each end.
pub(crate) const TOOLBAR_W: f32 = 448.0;
/// From the text column's left edge to the toolbar's: its 4 points of padding and the 6 from a
/// button's edge to its icon, so the first icon's box starts on the column's edge (C4).
pub(crate) const TOOLBAR_X: f32 = -10.0;

/// Whether the formatting toolbar shows: source mode, the setting on, a note open and not
/// locked, and not in focus mode.
fn shows_toolbar(v: V) -> bool {
    let s = v.s;
    let locked = s.active.as_deref().is_some_and(|a| s.note_locked(a));
    v.has_doc() && !v.focus() && s.mode == Mode::Source && s.settings().toolbar && !locked
}

/// The toolbar's lock: a plain note gets a password, and an unlocked note locks again. The
/// accent marks a note that is locked and open now.
fn lock_button(v: V) -> El {
    use super::widgets::LOCK;
    let p = v.p;
    let open = v.s.active.as_deref().is_some_and(|a| v.s.note_unlocked(a));
    let help = crate::commands::lock_name(open);
    row()
        .key("fmt-lock")
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .hover(p.hover)
        .hover_group()
        .role("button")
        .label(help)
        .help(help)
        .on_press_tx(|s: &mut App, tx| s.ask_lock(tx))
        .child(icon(&LOCK, 16.0).color(if open { p.acc } else { p.mute }).group_color(if open { p.acc } else { p.ink }))
}

/// The formatting toolbar: a pill at the top of the note column, its first icon's box on the
/// column's left edge, as the text is. It scrolls with the note
/// until it reaches TOOLBAR_TOP below the title bar, then stays there, and the note scrolls
/// under it.
fn toolbar(v: V) -> El {
    use super::widgets::{BOLD, BULLETS, CODE, HEADING, IMAGE, ITALIC, LINK, NUMBERS, QUOTE, STRIKE, TASK, WIKI, icon_button};
    use crate::format::Fmt;
    let p = v.p;
    let tip = |name: &str, id: &str| crate::keymap::tip(v.s.preset(), name, id);
    let btn = |key: &str, glyph: &'static rune::Icon, help: String, f: Fmt| icon_button(p, key, glyph, 16.0, &help).on_press_tx(move |s: &mut App, tx| s.format(f, tx));
    let sep = |key: &str| el().key(key).w(1.0).h(16.0).mx(5.0).fill(p.line);
    // At rest the toolbar has no background and no border, as the text around it. The
    // background, its shadow and the find bar's corner and hairline come in while the note
    // scrolls under the toolbar, so the two bars on one row look alike (RAD).
    let shadow = if p.dark { 0.5 } else { 0.12 };
    let back = super::widgets::bar_edge(el(), &Roles::new(p))
        .key("toolbar-back")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(p.bg.alpha(0.88))
        .backdrop_blur(12.0)
        .box_shadow(rune::Shadow { x: 0.0, y: 6.0, blur: 22.0, spread: -4.0, color: Color::BLACK.alpha(shadow), inset: false })
        .stuck_reveal(8.0);
    // With the find bar on its own row, the stuck toolbar hangs a band of the page's colour
    // from the view's top edge to that row's foot, the width of the note area, which eases out
    // across the row's foot. Text scrolling up goes under the band, never beside the bar.
    let band = find_row(v).then(|| {
        let (left, right) = docked(v);
        let below = find_top(v) + super::reader::FIND_H;
        let h = below + FIND_FADE_OUT;
        let start = below - FIND_FADE_IN;
        let mut stops = vec![(0.0, p.bg)];
        stops.extend((0..=8).map(|k| {
            let t = k as f32 / 8.0;
            let y = start + t * (FIND_FADE_IN + FIND_FADE_OUT);
            (y / h, p.bg.alpha(1.0 - t * t * (3.0 - 2.0 * t)))
        }));
        el().key("find-band")
            .pin(Some(-(source_column(v).gutter + TOOLBAR_X)), Some(-TOOLBAR_TOP), None, None)
            .size(v.vw - left - right, h)
            .linear_gradient(std::f32::consts::PI, stops)
            .inert()
            .stuck_reveal(8.0)
    });
    row()
        .key("toolbar")
        .self_start()
        .h(TOOLBAR_H)
        .mb(64.0 - TOOLBAR_TOP - TOOLBAR_H)
        // The first icon lines up with the text: 4 points of padding, then the 6 points from
        // a button's edge to its icon.
        .ml(TOOLBAR_X)
        .px(4.0)
        .gap(2.0)
        .center_y()
        .sticky_top(TOOLBAR_TOP)
        // A press between the buttons stays on the toolbar. The column under it would move
        // the caret to the end of the note.
        .on_press(|_: &mut App| {})
        .role("toolbar")
        .label("Formatting")
        .children(band)
        .child(back)
        .child(btn("fmt-bold", &BOLD, tip("Bold", "writer.bold"), Fmt::Bold))
        .child(btn("fmt-italic", &ITALIC, tip("Italic", "writer.italic"), Fmt::Italic))
        .child(btn("fmt-strike", &STRIKE, tip("Strikethrough", "writer.strikethrough"), Fmt::Strike))
        .child(btn("fmt-heading", &HEADING, "Heading".into(), Fmt::Heading))
        .child(sep("sep-1"))
        .child(btn("fmt-link", &LINK, tip("Insert link", "writer.link"), Fmt::Link))
        .child(btn("fmt-wiki", &WIKI, "Insert note link".into(), Fmt::Wiki))
        .child(sep("sep-2"))
        .child(btn("fmt-bullets", &BULLETS, "Bulleted list".into(), Fmt::Bullet))
        .child(btn("fmt-numbers", &NUMBERS, "Numbered list".into(), Fmt::Number))
        .child(btn("fmt-task", &TASK, "Task".into(), Fmt::Task))
        .child(btn("fmt-quote", &QUOTE, "Quote".into(), Fmt::Quote))
        .child(btn("fmt-code", &CODE, tip("Inline code", "writer.inline-code"), Fmt::Code))
        .child(sep("sep-3"))
        .child(icon_button(p, "fmt-image", &IMAGE, 16.0, &tip(crate::commands::INSERT_IMAGE, "app.insert-image")).on_press_tx(|s: &mut App, tx| s.pick_image(tx)))
        .child(sep("sep-4"))
        .child(lock_button(v))
}

/// The note's folder and name, in the title bar; a click on the name renames it.
pub(super) fn crumb(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let active = s.active.clone().unwrap_or_default();
    let dir = md::dir(&active).to_string();
    let style = literal(12.0);
    // The separator and the extension read on every theme: "/" at 3:1, ".md" at 4.5:1.
    let quiet = Roles::panel(p);
    // A long folder or name is cut to the bar's room: the folder keeps its end, the name its start.
    let mut r = row().key("crumb").h(24.0).shrink().min_w(0.0).gap(6.0).center_y();
    if !dir.is_empty() {
        r = r.child(text(dir).font(style).color(p.mute).truncate_head().shrink().min_w(0.0)).child(label("/", style, quiet.ghost));
    }
    let r = match &s.renaming {
        None => r.child(
            row()
                .key("crumb-file")
                .shrink()
                .min_w(0.0)
                .px(5.0)
                .py(2.0)
                .mx(-5.0)
                .radius(5.0)
                .hover(p.hover)
                .no_window_drag()
                .role("button")
                .label("Rename note")
                .help("Rename note")
                .cursor(rune::Cursor::Text)
                .on_press_tx(super::titlebar::act(|s, tx| s.start_rename(tx)))
                .child(text(md::base(&active).to_string()).font(style).color(p.ink2).truncate().shrink().min_w(0.0))
                .child(label(".md", style, quiet.faint)),
        ),
        Some(edit) => {
            let w = (edit.text.chars().count() as f32 + 1.0).max(8.0) * 12.0 * 0.6 + 10.0;
            r.child(
                row()
                    .key("rename-row")
                    .center_y()
                    .child(
                        el().key("rename-box").no_window_drag().w(w).px(5.0).py(2.0).mx(-5.0).radius(5.0).fill(p.soft).child(
                            text_input(edit, style)
                                .key("rename")
                                .single_line()
                                .w_full()
                                .color(p.ink)
                                .caret(p.ink, p.sel)
                                .on_edit(|s: &mut App| s.renaming.get_or_insert_with(|| TextEdit::new("")))
                                .on_submit(|s: &mut App, tx| s.commit_rename(tx))
                                .on_input_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| {
                                    if ev.key == Key::Escape {
                                        s.cancel_rename(tx);
                                        return true;
                                    }
                                    false
                                })
                                .on_window_focus(|_s: &mut App, _f, _tx| {}),
                        ),
                    )
                    .child(label(".md", style, quiet.faint).ml(5.0)),
            )
        }
    };
    // After an outside change (RD-33).
    if s.reader.reloaded.is_some() && s.mode == Mode::Preview { r.child(super::reader::reloaded(v)) } else { r }
}

/// The source font at size `E`. Each line is 155 percent of the iA fonts' natural 1.3 em
/// (WR-07); a heading's span keeps that ratio at its own size.
pub fn ed_style(v: V) -> TextStyle {
    let set = v.s.settings();
    let size = set.size as f32;
    note(theme::editor_font(&set.font).family, size, line_h(size)).features("'liga' 0, 'clig' 0, 'calt' 0")
}

/// The source's line height at size `e` (WR-07).
pub fn line_h(e: f32) -> f32 {
    (e * 1.3 * 1.55).round()
}

/// The source column (WR-171, WR-172): the preview's rule with the source font's measure.
pub fn source_column(v: V) -> crate::page::Column {
    let set = v.s.settings();
    let (left, right) = docked(v);
    let avg = crate::page::avg_char(theme::editor_font(&set.font).family);
    crate::page::column(v.vw - left - right, set.size as f32, avg, v.s.wide(), left - right)
}

/// The widths of the docked sidebar and outline, 0 for one that is not docked.
fn docked(v: V) -> (f32, f32) {
    (if v.left_visible() { 249.0 } else { 0.0 }, if v.right_visible() { 236.0 } else { 0.0 })
}

/// The writer (`behavior/writer.md`): one field, painted with the formatting spans of
/// `crate::writer`. The text is the note's Markdown, character for character (WR-01).
fn source(v: V, width: f32, room: f32) -> El {
    let p = v.p;
    let s = v.s;
    let doc = s.active_text();
    let style = ed_style(v);
    let e = style.size;
    let c = Roles::new(p);
    let ink = crate::writer::Ink::new(&c);
    let edit = s.active.as_ref().and_then(|a| s.edits.get(a));
    let caret = edit.map_or(0, |e| e.focus.min(doc.len()));
    let focus = s.settings().paragraph_focus.then(|| md::para_range(doc, caret));
    let (spans, strikes, hangs) = s.source_spans(e, &ink, focus);
    // The field reaches a gutter left of the column, and every line starts at the column, so
    // marks hang in the gutter (WR-26).
    let gutter = hang_gutter(e).min(room);
    let mut marks = Vec::new();
    // Typewriter scrolling centres this character's line (WR-88).
    let at = if caret < doc.len() {
        Some(caret..caret + doc[caret..].chars().next().map_or(1, char::len_utf8))
    } else {
        doc[..caret].chars().next_back().map(|ch| caret - ch.len_utf8()..caret)
    };
    if let Some(r) = at {
        marks.push(TextHighlight::new(r, Color::TRANSPARENT).key(crate::writer_app::CARET));
    }
    if let Some((line, _)) = s.wstate.top_line {
        let a = md::line_start(doc, line);
        if let Some(ch) = doc[a..].chars().next() {
            marks.push(TextHighlight::new(a..a + ch.len_utf8(), Color::TRANSPARENT).key(crate::writer_app::LINE_AT));
        }
    }
    // Find paints every match as the reader does (RD-59, WR-118): the find background, and the
    // current one solid accent with background-coloured letters.
    if s.find_query().is_some() {
        let cur = s.reader.find.cur;
        for (k, r) in s.source_matches().into_iter().enumerate() {
            marks.push(if k == cur { TextHighlight::new(r, c.find_current).color(c.find_current_text).key(crate::reader::FIND_CUR) } else { TextHighlight::new(r, c.find) });
        }
    }
    let empty = TextEdit::new("");
    let pad_top = if s.settings().typewriter_scroll { v.vh * 0.5 } else { 0.0 };
    let input = text_input(edit.unwrap_or(&empty), style)
        .key("editor")
        .w_full()
        .color(ink.text);
    let input = writer_caret(input, &c)
        .placeholder("Start writing…", c.ghost)
        .spellcheck(s.settings().spellcheck)
        .label("Note")
        .spans(spans)
        .strikes(strikes)
        .text_highlights(marks)
        .on_edit(|s: &mut App| s.cur_edit())
        .on_input_change(|s: &mut App, _: &mut Tx| s.after_edit())
        .on_input_paste(|s: &mut App, content, tx| s.paste_content(content, tx))
        .on_input_key(on_editor_key);
    // Each line is laid out on its own, so a keystroke reshapes only its line (WR-224), in the
    // direction of its own first strong character.
    let input = with_edit_menu(input, "editor").paragraph_styles(paragraph_styles(&hangs, gutter)).on_layout_tx(|s: &mut App, r, tx| s.field_laid_out(r, tx));
    el().key("source").w(width + gutter).ml(-gutter).pt(pad_top).child(input)
}

/// The gutter marks hang in: round(E × 3.2), 45 at 14 (WR-26, SHEET-028).
pub(crate) fn hang_gutter(e: f32) -> f32 {
    (e * 3.2).round()
}

/// Each line's layout in a field whose lines start `gutter` in: its mark hung before that, a
/// list item's leading whitespace kept in the column (WR-16, WR-19, WR-22, WR-25, WR-225).
pub(super) fn paragraph_styles(hangs: &[(usize, usize)], gutter: f32) -> Vec<rune::ParagraphStyle> {
    let base = rune::ParagraphStyle::new().indent(gutter);
    hangs.iter().map(|&(lead, end)| if end > 0 { base.hang(end).lead(lead) } else { base }).collect()
}

/// Quire's own menu on a right-click in field `key` where Rune has no native one, as on Linux
/// (WR-135, SHEET-032). Elsewhere the field keeps Rune's menu, spelling included.
pub fn with_edit_menu(input: rune::El, key: &'static str) -> rune::El {
    if rune::popup_menu_supported() {
        return input;
    }
    input.on_secondary_press(move |s: &mut App, at, tx| s.open_menu(crate::layer::MenuOf::Edit(key), (at.x, at.y), tx))
}

/// The writer's caret and selection, shared with the edit sheet: an accent caret in Rune's own
/// shape, a 1.5 point bar that doesn't blink (WR-175, SHEET-029, adapted), and selected text in
/// the strong colour over the selection (WR-176).
pub(super) fn writer_caret(input: El, c: &Roles) -> El {
    let sel = rune::SelectionStyle::new(c.selection).ink(c.strong).inactive(c.selection, Some(c.strong));
    input.caret(c.accent, c.selection).selection_style(sel)
}

/// The `[[link]]` around the caret, if any.
pub fn link_at_caret(text: &str, caret: usize) -> Option<String> {
    let (_, ls) = md::line_col(text, caret);
    let le = text[ls..].find('\n').map_or(text.len(), |n| ls + n);
    let line = &text[ls..le];
    let col = caret - ls;
    for (t, r) in md::inline_tokens(line) {
        // `[[cat.png]]` names an image, not a note, so there is no note to open or make.
        if t == Tok::Wiki && col >= r.start && col <= r.end && !md::is_image(&md::link_target(&line[r.start + 2..r.end - 2])) {
            return Some(md::link_target(&line[r.start + 2..r.end - 2]).to_string());
        }
    }
    None
}

pub fn follow_link_at_caret(s: &mut App, tx: &mut Tx) {
    let text = s.active_text().to_string();
    let caret = s.cur_edit().focus.min(text.len());
    if let Some(t) = link_at_caret(&text, caret) {
        s.open_link(&t, tx);
    }
}

fn on_editor_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // A caret move that follows a key is not a click (see `App::check_link_click`).
    s.key_moved = true;
    // The field would keep Escape, so the writer's Esc order runs here (WR-77).
    if matches!(ev.key, Key::Escape) && !ev.mods.shift {
        return s.writer_escape(tx);
    }
    // The writer's editing keys come from the keymap, so a preset changes them here too.
    match keymap::lookup(s.preset(), Ctx::Writer, ev).iter().find_map(|a| crate::keys::handler(a.id)) {
        Some(run) => {
            run(s, tx);
            true
        }
        None => false,
    }
}

/// 1234 as "1,234", as `toLocaleString` writes it in English.
pub fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn no_tab(v: V) -> El {
    let p = v.p;
    let hints = [("Open a note", "app.quick-switcher"), ("New note", "app.new-note"), ("All commands", "app.command-palette"), ("Search", "app.search-notes")];
    let mut c = col().key("hints").w(260.0).gap(12.0);
    for (i, (l, id)) in hints.into_iter().enumerate() {
        let Some(run) = crate::keys::handler(id) else { continue };
        c = c.child(
            row()
                .key(format!("hint-{i}"))
                .gap(24.0)
                .justify_between()
                .hover_group()
                .role("button")
                .on_press_tx(move |s: &mut App, tx| run(s, tx))
                .child(label(l, sans(13.5, 400.0), p.ink2).group_color(p.acc))
                .child(label(crate::keymap::label(v.s.preset(), id), keys(12.0), p.mute)),
        );
    }
    c
}

fn empty_vault(v: V) -> El {
    let p = v.p;
    let path = v.s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    col()
        .key("empty-vault")
        .max_w(380.0)
        .gap(14.0)
        .items_start()
        .child(label("Nothing here yet.", sans(32.0, 500.0).lh(32.0 * 1.15).tracking(-0.02 * 32.0), p.ink))
        .child(text(md::pretty(&format!("Notes you write here are saved as plain .md files in {path}."))).font(sans(14.0, 400.0).lh(14.0 * 1.6)).color(p.ink2).wrap())
        .child(
            row()
                .key("first-note")
                .h(34.0)
                .mt(6.0)
                .px(14.0)
                .gap(12.0)
                .center_y()
                .radius(8.0)
                .fill(p.button)
                .hover(p.button_hover)
                .role("button")
                .on_press_tx(|s: &mut App, tx| s.new_note(None, tx))
                .child(label("Create your first note", sans(13.5, 500.0), p.on_acc))
                .child(label(crate::keymap::label(v.s.preset(), "app.new-note"), keys(11.0), p.on_acc.alpha(0.8))),
        )
}
