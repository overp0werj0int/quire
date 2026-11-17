//! The reader's chrome over the page (`behavior/reader.md`): the find bar, the help sheet,
//! toasts, the reading progress line and the "reloaded" mark.

use rune::{Color, El, Key, KeyEvent, ScrollSpec, TextCase, Tx, col, el, icon, row, text, text_input};

use super::V;
use super::chip::Chip;
use super::panel::{BODY, BODY_SMALL, CAPTION, font};
use super::widgets::{CHEVRON_DOWN, CHEVRON_UP, CLOSE, SEARCH};
use crate::app::App;
use crate::keymap::{self, Ctx};
use crate::roles::Roles;
use crate::theme;

pub(super) fn popup(c: &Roles) -> El {
    el().fill(c.bg).border(1.0, c.hairline).radius(6.0).box_shadow(theme::menu_shadows()[0]).box_shadow(theme::menu_shadows()[1])
}

/// The find bar's size, and its room from the note area's right edge.
pub const FIND_H: f32 = 40.0;
pub const FIND_W: f32 = 380.0;
pub const FIND_RIGHT: f32 = 16.0;

/// The find bar (RD-52 to RD-61): 380 by 40, 16 px from the note area's right edge, 10 px
/// under its top, or on its own row under the formatting toolbar when it does not fit beside
/// it (`editor::find_top`).
pub fn find_bar(v: V) -> El {
    let s = v.s;
    let c = Roles::new(v.p);
    let f = &s.reader.find;
    if !f.open {
        return el();
    }
    let n = s.find_count();
    let (count, tone) = match (f.q.text.is_empty(), n) {
        (true, _) => (String::new(), c.faint),
        (false, 0) => ("No matches".to_string(), c.red),
        (false, n) => (format!("{} / {n}", f.cur.min(n - 1) + 1), c.faint),
    };
    let preset = s.preset();
    let tip = |name: &str, id: &str| keymap::tip(preset, name, id);
    let mono = theme::pick(&theme::CODE_FONTS, &s.settings().code_font).family;
    let button = |key: &'static str, glyph: &'static rune::Icon, size: f32, help: String, run: fn(&mut App, &mut Tx)| {
        Chip { key, glyph, size: BODY, icon: size, label: "", mono, tint: c.faint, active: false, fill: None }.el(&c).label(help.clone()).help(help).on_press_tx(run)
    };
    let field = text_input(&f.q, font(BODY, false))
        .key("find-input")
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(c.text)
        .placeholder("Find in document", c.ghost)
        .caret(c.text, c.selection)
        .on_edit(|s: &mut App| &mut s.reader.find.q)
        .on_input_change(|s: &mut App, tx| s.find_changed(tx))
        .on_input_key(on_find_key);
    popup(&c)
        .key("find-bar")
        .pin(None, Some(super::editor::find_top(v)), Some(FIND_RIGHT), None)
        .size(FIND_W, FIND_H)
        .paint_order(40)
        .child(
            row()
                .w_full()
                .h_full()
                .pl(12.0)
                .pr(6.0)
                .gap(8.0)
                .center_y()
                .child(icon(&SEARCH, 14.0).color(c.faint))
                .child(field)
                .child(text(count).key("find-count").font(font(BODY_SMALL, false)).color(tone))
                .child(button("find-prev", &CHEVRON_UP, 10.0, tip("Previous", "find.previous"), |s, tx| s.find_step(false, tx)))
                .child(button("find-next", &CHEVRON_DOWN, 10.0, tip("Next", "find.next"), |s, tx| s.find_step(true, tx)))
                .child(button("find-close", &CLOSE, 8.0, tip("Close", "find.close"), |s, tx| s.close_find(tx))),
        )
}

/// Keys in the find field: Enter, Shift+Enter, Esc and Ctrl+F (KEY items 77 to 80).
fn on_find_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if matches!(ev.key, Key::Escape) && !ev.mods.shift {
        s.close_find(tx);
        return true;
    }
    match keymap::lookup(s.preset(), Ctx::Find, ev).iter().find_map(|a| crate::keys::handler(a.id)) {
        Some(run) => {
            run(s, tx);
            true
        }
        None => false,
    }
}

/// One row of the help sheet: what it does and the actions whose keys it shows.
struct HelpRow {
    what: &'static str,
    ids: &'static [&'static str],
}

const fn h(what: &'static str, ids: &'static [&'static str]) -> HelpRow {
    HelpRow { what, ids }
}

/// The help sheet's groups, in omarky's order and wording (RD-90). The keys come from the
/// active keymap.
const HELP: &[(&str, &[HelpRow])] = &[
    (
        "Read",
        &[
            h("Scroll", &["reader.scroll-down", "reader.scroll-up"]),
            h("Page down / up", &["reader.page-down", "reader.page-up"]),
            h("Top / bottom", &["reader.top", "reader.bottom"]),
            h("Outline", &["reader.toggle-outline"]),
            h("Readable / full width", &["reader.toggle-wide"]),
            h("Zoom text", &["app.zoom-in", "app.zoom-out", "reader.zoom-reset"]),
        ],
    ),
    (
        "Find & copy",
        &[
            h("Find", &["reader.find"]),
            h("Next / previous match", &["reader.find-next", "reader.find-previous"]),
            h("Copy a block's Markdown", &["reader.copy-block-source"]),
            h("Copy the block under the mouse", &["reader.copy-block-under-pointer"]),
            h("Copy the whole file", &["reader.copy-all"]),
        ],
    ),
    (
        "Edit",
        &[
            h("Edit the block under the mouse", &["reader.edit-block"]),
            h("Edit its section", &["reader.edit-section"]),
            h("Save to the file / cancel", &["sheet.save", "sheet.cancel"]),
        ],
    ),
    ("File", &[h("Open a note", &["app.quick-switcher"]), h("Read from disk", &["reader.read-from-disk"]), h("Close the tab", &["reader.close-tab"]), h("This sheet", &["reader.help"])]),
];

/// The writer's help groups (WR-166), in omarky's order and wording.
const WRITER_HELP: &[(&str, &[HelpRow])] = &[
    (
        "Write",
        &[
            h("Bold / italic", &["writer.bold", "writer.italic"]),
            h("Inline code", &["writer.inline-code"]),
            h("Strikethrough", &["writer.strikethrough"]),
            h("Link", &["writer.link"]),
            h("Heading 1 to 6 / paragraph", &["writer.heading-1", "writer.heading-6", "writer.plain-paragraph"]),
            h("Indent / outdent", &["writer.indent", "writer.outdent"]),
            h("Follow the link", &["writer.follow-link"]),
        ],
    ),
    (
        "View",
        &[
            h("Preview", &["app.toggle-mode"]),
            h("Find", &["writer.find"]),
            h("Next / previous match", &["writer.find-next", "writer.find-previous"]),
            h("Paragraph focus", &["writer.paragraph-focus"]),
            h("Typewriter scrolling", &["writer.typewriter"]),
            h("Next font", &["writer.next-font"]),
            h("Outline", &["app.toggle-outline"]),
            h("Zoom text", &["app.zoom-in", "app.zoom-out", "reader.zoom-reset"]),
        ],
    ),
    (
        "File",
        &[
            h("Save", &["app.save"]),
            h("New note", &["app.new-note"]),
            h("Open a note", &["app.quick-switcher"]),
            h("Recent notes", &["writer.recent-notes"]),
            h("Close the tab", &["app.close-tab"]),
            h("This sheet", &["writer.help"]),
        ],
    ),
];

/// A key cap (RD-91): caption text in dim on a raised fill, hairline border, 12 by 6 px of
/// padding in all.
pub(super) fn cap(t: String, c: &Roles) -> El {
    row().px(6.0).py(3.0).radius(3.0).fill(c.raised).border(1.0, c.hairline).child(text(t).font(font(CAPTION, false)).color(c.dim))
}

/// The help sheet (RD-86 to RD-91; in the writer WR-158 to WR-166).
pub fn help_sheet(v: V) -> El {
    let c = Roles::new(v.p);
    let preset = v.s.preset();
    let writer = v.s.mode == crate::app::Mode::Source;
    let (groups, row_h, row_gap, keys_w, col_w) = if writer { (WRITER_HELP, 26.0, 7.0, 170.0, 470.0) } else { (HELP, 24.0, 0.0, 190.0 - 24.0, 520.0 - 48.0) };
    // The writer's sheet sets its groups in two columns on a wide window (WR-163).
    let cols = if writer && v.vw > 1040.0 { 2 } else { 1 };
    let content = col_w * cols as f32 + 40.0 * (cols - 1) as f32;
    let w = (v.vw - 48.0).min(content + 48.0);
    let mut card = col().key("help-body").w(w).px(24.0).py(22.0).gap(18.0).on_press(|_: &mut App| {});
    card = card.child(
        text(format!("Quire {}", env!("CARGO_PKG_VERSION")))
            .key("help-version")
            .font(font(CAPTION, false))
            .color(c.ghost)
            .pin(None, Some(22.0), Some(24.0), None),
    );
    let mut columns: Vec<El> = (0..cols).map(|i| col().key(format!("help-col-{i}")).w(col_w.min(w - 48.0)).gap(18.0)).collect();
    for (gi, (title, rows)) in groups.iter().enumerate() {
        let mut g = col().key(format!("help-{title}")).w_full().gap(row_gap).child(
            text(title.to_string()).font(font(CAPTION, true).tracking(1.0)).color(c.faint).text_case(TextCase::Uppercase).mb(6.0),
        );
        for (i, r) in rows.iter().enumerate() {
            let mut keys = row().gap(4.0).w(keys_w).center_y();
            for id in r.ids {
                let first = keymap::action(id).and_then(|a| a.chords(preset).first().copied());
                if let Some(k) = first {
                    keys = keys.child(cap(keymap::label_of(k), &c));
                }
            }
            g = g.child(row().key(format!("r-{i}")).h(row_h).w_full().center_y().child(keys).child(text(r.what.to_string()).font(font(BODY_SMALL, false)).color(c.text)));
        }
        // The first group fills the left column; the rest go to the right one.
        let k = if cols == 2 && gi > 0 { 1 } else { 0 };
        let slot = std::mem::replace(&mut columns[k], el());
        columns[k] = slot.child(g);
    }
    let mut body = row().key("help-cols").gap(40.0).items_start();
    for column in columns {
        body = body.child(column);
    }
    // The card fits the window, 24 px clear of each edge, and its rows scroll inside it.
    let card = popup(&c).key("help-card").child(col().key("help-scroll").max_h(v.vh - 48.0).scroll(ScrollSpec { stick_bottom: false }).child(card.child(body)));
    // Modal: the note behind takes no wheel or press while help is open.
    col()
        .key("help-scrim")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(v.p.scrim)
        .center()
        .paint_order(70)
        .role("dialog")
        .label("Keyboard help")
        .modal_tx(false, |s: &mut App, tx| s.close_help(tx))
        .on_press_tx(|s: &mut App, tx| s.close_help(tx))
        .child(card)
}

/// A toast (RD-98): a pill at the bottom centre of the note area, 28 px above its bottom edge.
pub fn toast(v: V, msg: &str) -> El {
    let c = Roles::new(v.p);
    row()
        .key("toast-row")
        .pin(Some(0.0), None, Some(0.0), Some(28.0))
        .justify_center()
        .paint_order(65)
        .child(popup(&c).key("toast").radius(999.0).px(14.0).py(7.0).child(text(msg.to_string()).font(font(BODY, false)).color(c.text)))
}

/// The reading progress line (RD-37, RD-38): 2 px of accent along the top of the note area.
pub fn progress(v: V) -> El {
    let c = Roles::new(v.p);
    el().key("progress").pin(Some(0.0), Some(0.0), None, None).h(2.0).w(0.0).fill(c.accent).paint_order(30).live_rect(&v.s.reader.bar)
}

/// The "reloaded" mark after the note's name (RD-33): a 6 px accent dot and caption text.
pub fn reloaded(v: V) -> El {
    let c = Roles::new(v.p);
    let accent: Color = c.accent;
    row()
        .key("reloaded")
        .gap(5.0)
        .ml(8.0)
        .center_y()
        .child(el().size(6.0, 6.0).radius(3.0).fill(accent))
        .child(text("reloaded").font(font(CAPTION, false)).color(accent))
}
