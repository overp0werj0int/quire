//! The reader's chrome over the page (`behavior/reader.md`): the find bar, the help sheet,
//! toasts, the reading progress line and the "reloaded" mark.

use rune::{Color, El, Key, KeyEvent, TextCase, Tx, col, el, icon, row, text, text_input};

use super::V;
use super::panel::{BODY, BODY_SMALL, CAPTION, font};
use super::widgets::{CHEVRON_DOWN, CHEVRON_UP, CLOSE, SEARCH};
use crate::app::App;
use crate::keymap::{self, Ctx};
use crate::roles::Roles;
use crate::theme;

fn popup(c: &Roles) -> El {
    el().fill(c.bg).border(1.0, c.hairline).radius(6.0).box_shadow(theme::menu_shadows()[0]).box_shadow(theme::menu_shadows()[1])
}

/// The find bar (RD-52 to RD-61): 380 by 40, 16 px from the note area's right edge and 10 px
/// under its top.
pub fn find_bar(v: V) -> El {
    let s = v.s;
    let c = Roles::new(v.p);
    let f = &s.reader.find;
    if !f.open {
        return el();
    }
    let n = s.find_matches().len();
    let (count, tone) = match (f.q.text.is_empty(), n) {
        (true, _) => (String::new(), c.faint),
        (false, 0) => ("No matches".to_string(), c.red),
        (false, n) => (format!("{} / {n}", f.cur.min(n - 1) + 1), c.faint),
    };
    let preset = s.preset();
    let tip = |name: &str, id: &str| format!("{name}  {}", keymap::label(preset, id));
    let button = |key: &str, glyph: &'static rune::Icon, size: f32, help: String, run: fn(&mut App, &mut Tx)| {
        row()
            .key(key)
            .size(26.0, 26.0)
            .center()
            .radius(4.0)
            .hover(c.hover)
            .hover_group()
            .role("button")
            .label(help.clone())
            .help(help)
            .on_press_tx(run)
            .child(icon(glyph, size).color(c.faint).group_color(c.text))
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
        .pin(None, Some(10.0), Some(16.0), None)
        .size(380.0, 40.0)
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
            h("Reading / wide measure", &["reader.toggle-wide"]),
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

/// A key cap (RD-91): caption text in dim on a raised fill, hairline border, 12 by 6 px of
/// padding in all.
fn cap(t: String, c: &Roles) -> El {
    row().px(6.0).py(3.0).radius(3.0).fill(c.raised).border(1.0, c.hairline).child(text(t).font(font(CAPTION, false)).color(c.dim))
}

/// The help sheet (RD-86 to RD-91).
pub fn help_sheet(v: V) -> El {
    let c = Roles::new(v.p);
    let preset = v.s.preset();
    let w = (v.vw - 48.0).min(520.0);
    let mut card = col().key("help-card").w(w).px(24.0).py(22.0).gap(18.0).on_press(|_: &mut App| {});
    card = card.child(
        text(format!("Quire {}", env!("CARGO_PKG_VERSION")))
            .key("help-version")
            .font(font(CAPTION, false))
            .color(c.ghost)
            .pin(None, Some(22.0), Some(24.0), None),
    );
    for (title, rows) in HELP {
        let mut g = col().key(format!("help-{title}")).w_full().child(
            text(title.to_string()).font(font(CAPTION, true).tracking(1.0)).color(c.faint).text_case(TextCase::Uppercase).mb(6.0),
        );
        for (i, r) in rows.iter().enumerate() {
            let mut keys = row().gap(4.0).w(190.0 - 24.0).center_y();
            for id in r.ids {
                let first = keymap::action(id).and_then(|a| a.chords(preset).first().copied());
                if let Some(k) = first {
                    keys = keys.child(cap(keymap::label_of(k), &c));
                }
            }
            g = g.child(row().key(format!("r-{i}")).h(24.0).w_full().center_y().child(keys).child(text(r.what.to_string()).font(font(BODY_SMALL, false)).color(c.text)));
        }
        card = card.child(g);
    }
    let card = popup(&c).child(card);
    col()
        .key("help-scrim")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(c.bg.alpha(0.72))
        .center()
        .paint_order(70)
        .on_press(|s: &mut App| s.reader.help = false)
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
pub fn progress(v: V, k: f32) -> El {
    let c = Roles::new(v.p);
    el().key("progress").pin(Some(0.0), Some(0.0), None, None).h(2.0).w_frac(k.clamp(0.0, 1.0)).fill(c.accent).paint_order(30)
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
