//! The reader's chrome over the page (`behavior/reader.md`): the find bar, the help sheet,
//! toasts, the reading progress line and the "reloaded" mark.

use rune::{Color, El, Key, KeyEvent, ScrollSpec, Tx, col, el, icon, row, text, text_input};

use super::V;
use super::layer::Card;
use super::chip::Chip;
use super::panel::BODY;
use super::styles::{self, caption, keys, sans};
use super::icons::{CHEVRON_DOWN, CHEVRON_UP, CLOSE, SEARCH};
use super::widgets::Control;
use crate::app::App;
use crate::keymap::{self, Ctx};
use crate::roles::Roles;
use crate::theme;

pub(super) fn popup(c: &Roles) -> El {
    super::widgets::floating(el(), c).radius(6.0)
}

/// The find bar's size, and its room from the note area's right edge.
pub const FIND_H: f32 = 40.0;
pub const FIND_W: f32 = 380.0;
pub const FIND_RIGHT: f32 = 16.0;

/// Bottom cushion below the find bar inside the top Find band when no toolbar shows.
pub(super) const FIND_BACKING_PAD: f32 = 8.0;

/// Total height of the top Find header slot for a given note-area width when Find is open
/// without a formatting toolbar (`FINDMODEL`, `FINDSTATE`).
pub(crate) fn find_slot_h_for_width(doc_w: f32) -> f32 {
    let narrow = doc_w <= super::editor::NARROW_FIND_MAX;
    let top = if narrow { 8.0 } else { super::editor::TOOLBAR_TOP };
    let h = if narrow { super::editor::FIND_NARROW_H } else { FIND_H };
    top + h + FIND_BACKING_PAD
}

/// Total height of the top Find header slot for `s`'s current note-area width, used by
/// scroll-anchor compensation when toggling Find open or closed (`FINDSTATE`).
pub(crate) fn find_slot_h_for_app(s: &App) -> f32 {
    let width = s.reader.scroll.get()[3];
    let doc_w = if width > 0.0 {
        width
    } else {
        let (left, right) = s.docked();
        s.view_w.get() - if left { super::sidebar::DOCKED_W } else { 0.0 } - if right { super::outline::PANEL_W } else { 0.0 }
    };
    find_slot_h_for_width(doc_w)
}

/// Total height of the top Find header slot when Find is open without a formatting toolbar.
pub(super) fn find_backing_h(v: V) -> f32 {
    find_slot_h_for_width(super::editor::doc_width(v))
}

/// Full-width page-coloured top slot placed in flow above `doc-scroll` whenever Find is open
/// without a formatting toolbar (Preview at any width, or toolbar-hidden Source / zen mode).
/// Keeps `doc-scroll` clipped strictly below `find-bar` so scrolled headings, prose, and table
/// headers never collide with or peek beneath `find-bar` at any width, while painting beneath the
/// reading progress line (`paint_order(38)`) (`FINDMODEL`).
pub fn find_backing(v: V) -> El {
    let bg = Roles::new(v.p).bg;
    let key = if super::editor::narrow_find(v) { "find-narrow-band" } else { "find-band" };
    el().key(key)
        .w_full()
        .h(find_backing_h(v))
        .fill(bg)
        .inert()
        .paint_order(35)
}

/// The find bar (RD-52 to RD-61): 380 by 40, 16 px from the note area's right edge, 10 px
/// under its top, or on its own row under the formatting toolbar when it does not fit beside
/// it (`editor::find_top`). At narrow note-area widths (`<= 680 px`), spans the full usable
/// width with 12 px side margins (`FINDPOLISH`).
pub fn find_bar(v: V) -> El {
    let s = v.s;
    let c = Roles::new(v.p);
    let quiet = Roles::panel(v.p);
    let f = &s.reader.find;
    if !f.open {
        return el();
    }
    let state = s.find_state();
    let (count, tone, steppers_enabled) = match state {
        crate::reader::FindState::Empty => (String::new(), quiet.faint, false),
        crate::reader::FindState::Pending => (String::new(), quiet.faint, true),
        crate::reader::FindState::NoMatches => ("No matches".to_string(), c.readable_err(), false),
        crate::reader::FindState::Matches(n) => (format!("{} / {n}", f.cur.min(n - 1) + 1), quiet.faint, true),
    };
    let preset = s.preset();
    let tip = |name: &str, id: &str| keymap::tip(preset, name, id);
    let mono = theme::pick(&theme::CODE_FONTS, &s.settings().code_font).family;
    let (_, _, rc) = super::focus_ring(v.p);
    let button = |key: &'static str, glyph: &'static rune::Icon, size: f32, help: String, enabled: bool, run: fn(&mut App, &mut Tx)| {
        if !enabled {
            return row()
                .key(key)
                .size(24.0, 24.0)
                .center()
                .radius(3.0)
                .disabled(true)
                .label(help.clone())
                .help(help)
                .child(icon(glyph, size).color(quiet.faint.alpha(0.38)));
        }
        // Tight 1 pt outer gap keeps the 2 pt ring clear of both 36 pt narrow and 40 pt wide bar borders.
        super::focus::stop(Chip { key, glyph, size: BODY, icon: size, label: "", mono, tint: quiet.faint, active: false, fill: None }.el(&c))
            .label(help.clone())
            .help(help)
            .focus_ring(2.0, 1.0, rc)
            .keeps_text_selection()
            .control(run)
    };
    let field = text_input(&f.q, sans(13.0, 400.0))
        .key("find-input")
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(c.text)
        .placeholder("Find in note", quiet.faint)
        .label("Find in note")
        .caret(c.text, c.selection)
        .on_edit(|s: &mut App| &mut s.reader.find.q)
        .on_input_change(|s: &mut App, tx| s.find_changed(tx))
        .on_input_key(on_find_key);
    let narrow = super::editor::narrow_find(v);
    let top = super::editor::find_top(v);
    let h = super::editor::find_h(v);
    let mut bar = super::widgets::input_focus_ring_when(super::widgets::bar_edge(popup(&c), &c), v.p, f.q.focused)
        .key("find-bar")
        .paint_order(40)
        .on_key(|s: &mut App, ev: &KeyEvent, tx| {
            if matches!(ev.key, Key::Escape) && !ev.mods.logo {
                if s.notice.take().is_none() {
                    s.close_find(tx);
                }
                return true;
            }
            false
        });
    bar = if narrow {
        bar.pin(Some(super::editor::FIND_NARROW_X), Some(top), Some(super::editor::FIND_NARROW_X), None).h(h)
    } else {
        bar.pin(None, Some(top), Some(FIND_RIGHT), None).size(FIND_W, h)
    };
    bar.child(
        row()
            .w_full()
            .h_full()
            .pl(12.0)
            .pr(6.0)
            .gap(8.0)
            .center_y()
            .child(icon(&SEARCH, 14.0).color(quiet.faint))
            .child(field)
            .child(text(count).key("find-count").font(sans(12.0, 400.0)).color(tone))
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .preserve_focus_on_press(true)
                    .child(button("find-prev", &CHEVRON_UP, 12.0, tip("Previous match", "find.previous"), steppers_enabled, |s, tx| s.find_step(false, tx)))
                    .child(button("find-next", &CHEVRON_DOWN, 12.0, tip("Next match", "find.next"), steppers_enabled, |s, tx| s.find_step(true, tx)))
                    .child(button("find-close", &CLOSE, 12.0, tip("Close find", "find.close"), true, |s, tx| s.close_find(tx))),
            ),
    )
}

/// Keys in the find field: Enter, Shift+Enter, Esc and Ctrl+F (KEY items 77 to 80). Esc
/// dismisses the notice first, as on the page (RD-85, WR-77). Esc with a modifier does the
/// same, since the field gives up the keyboard on any Esc.
fn on_find_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if matches!(ev.key, Key::Escape) {
        if s.notice.take().is_none() {
            s.close_find(tx);
        }
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
            h("Copy or edit a block", &["reader.block-menu"]),
            h("Copy the block under the mouse", &["reader.copy-block-under-pointer"]),
            h("Copy the whole note", &["reader.copy-all"]),
        ],
    ),
    (
        "Edit",
        &[
            h("Edit the whole note", &["app.toggle-mode"]),
            h("Edit the block under the mouse", &["reader.edit-block"]),
            h("Edit its section", &["reader.edit-section"]),
            h("Save to the file / cancel", &["sheet.save", "sheet.cancel"]),
        ],
    ),
    (
        "File",
        &[
            h("Open a note", &["app.quick-switcher"]),
            h("Next / previous tab", &["app.next-tab", "app.previous-tab"]),
            h("Switch tab 1 to 9", &["app.tab-1", "app.tab-9"]),
            h("Read from disk", &["reader.read-from-disk"]),
            h("Close the tab", &["reader.close-tab"]),
            h("This sheet", &["reader.help"]),
        ],
    ),
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
            h("Next / previous tab", &["app.next-tab", "app.previous-tab"]),
            h("Switch tab 1 to 9", &["app.tab-1", "app.tab-9"]),
            h("Close the tab", &["app.close-tab"]),
            h("This sheet", &["writer.help"]),
        ],
    ),
];

/// A key cap (RD-91): delegates to the shared `<kbd>` helper (`widgets::kbd`, `SCROLLFADE`).
pub(super) fn cap(t: String, p: &crate::theme::Pal) -> El {
    super::widgets::kbd(p, t, false)
}

/// The help sheet (RD-86 to RD-91; in the writer WR-158 to WR-166).
/// The keyboard help, a card in the middle of the window (`super::layer`).
pub(super) fn help_sheet(v: V) -> Card {
    let c = Roles::new(v.p);
    let preset = v.s.preset();
    let writer = v.s.mode == crate::app::Mode::Source;
    let (groups, row_h, row_gap, keys_w, col_w) = if writer { (WRITER_HELP, 26.0, 7.0, 170.0, 470.0) } else { (HELP, 24.0, 0.0, 190.0 - 24.0, 520.0 - 48.0) };
    // The writer's sheet sets its groups in two columns on a wide window (WR-163).
    let cols = if writer && v.vw > 1040.0 { 2 } else { 1 };
    let content = col_w * cols as f32 + 40.0 * (cols - 1) as f32;
    let w = (v.vw - 48.0).min(content + 48.0);
    let header_h = 44.0;
    let header = row()
        .key("help-header")
        .w(w)
        .h(header_h)
        .pl(24.0)
        .pr(14.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], c.hairline)
        .border_layout()
        .child(text("Keyboard shortcuts").font(sans(13.0, 600.0)).color(c.text))
        .child(
            text(format!("Quire {}", env!("CARGO_PKG_VERSION")))
                .key("help-version")
                .font(sans(11.0, 400.0))
                .color(super::styles::caption_ink(v.p)),
        )
        .child(el().grow(1.0))
        .child(cap(keymap::label_of("Esc"), v.p))
        .child(
            row()
                .key("help-close")
                .size(24.0, 24.0)
                .center()
                .radius(6.0)
                .hover(v.p.hover)
                .hover_group()
                .role("button")
                .label("Close help")
                .help(keymap::tip(preset, "Close help", "help.close"))
                .control(|s: &mut App, tx| s.close_layer(tx))
                .child(icon(&CLOSE, 13.0).color(v.p.mute).group_color(v.p.ink)),
        );
    let card = col().key("help-body").w(w).px(24.0).py(18.0).gap(18.0);
    let mut columns: Vec<El> = (0..cols).map(|i| col().key(format!("help-col-{i}")).w(col_w.min(w - 48.0)).gap(18.0)).collect();
    for (gi, (title, rows)) in groups.iter().enumerate() {
        let mut g = col().key(format!("help-{title}")).w_full().gap(row_gap).child(
            caption(title, false, v.p).mb(6.0),
        );
        for (i, r) in rows.iter().enumerate() {
            let mut keys = row().gap(4.0).w(keys_w).center_y();
            for id in r.ids {
                if let Some(a) = keymap::action(id) {
                    let chords = a.chords(preset);
                    let show = if *id == "reader.help" { chords } else { &chords[..chords.len().min(1)] };
                    for k in show {
                        keys = keys.child(cap(keymap::label_of(k), v.p));
                    }
                }
            }
            g = g.child(row().key(format!("r-{i}")).h(row_h).w_full().center_y().child(keys).child(text(r.what.to_string()).font(sans(12.5, 400.0)).color(c.text)));
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
    let live = v.s.reader.help_scroll.clone();
    // The card fits the window, 24 px clear of each edge, and its rows scroll inside it, fading
    // at the card's edges as Settings' page does.
    let scroll = col()
        .key(crate::reader::HELP_SCROLL)
        .w(w)
        .max_h((v.vh - 48.0 - header_h).max(80.0))
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_margin(super::widgets::SCROLL_FADE)
        .on_scroll_offset(move |at| {
            let (by_hand, _, t) = rune::scroll_watch_state();
            let mut st = live.get();
            st.on_offset([at.offset[1], at.max[1], at.viewport[1], at.viewport[0]], by_hand, t);
            live.set(st);
        })
        .child(super::widgets::faded("help", c.bg, card.child(body)));
    // Keep clip_rounded on the inner wrapper so popup(&c)'s outer drop shadow renders outside
    // the card bounds, matching Settings and the Command Palette. On dark themes, raise the card
    // edge above the dark scrim (`RASTERCR`).
    let mut card = popup(&c)
        .key("help-card")
        .role("dialog")
        .label("Keyboard help")
        .child(col().w(w).radius(6.0).clip_rounded(6.0).child(header).child(scroll));
    if v.p.dark {
        card = card.border(1.0, crate::roles::mix(c.bg, c.text, 0.20));
    }
    Card { el: card, plane: Some(col().center()) }
}

/// A toast's place: this far above the note area's bottom edge (RD-98).
const TOAST_LIFT: f32 = 28.0;

/// A pill's height at the note area's foot: one line of popup text, 7 px above and below it.
/// The toast with Undo and the notice hold their 24 px buttons in the same 30 px.
const PILL_H: f32 = 30.0;

/// The room between the notice and the toast under it.
const PILL_GAP: f32 = 8.0;

/// The width a pill at the note area's foot may take: the part of the note area no drawer
/// covers, 16 px clear of its edges (`foot_row`).
fn foot_room(v: V) -> f32 {
    let (left, right) = if v.is_vault() { super::editor::docked(v) } else { (0.0, 0.0) };
    let over = if v.left_over() { super::sidebar::W } else { 0.0 } + if v.right_over() { super::outline::PANEL_W } else { 0.0 };
    (v.vw - left - right - over - 32.0).max(0.0)
}

fn width(t: &str, style: rune::TextStyle) -> f32 {
    rune::text::measure(t, &[], style, None).0
}

/// A toast (RD-98): a pill at the bottom centre of the note area, `TOAST_LIFT` above its bottom
/// edge, over `toast_band`.
pub fn toast(v: V, msg: &str) -> El {
    let c = Roles::new(v.p);
    foot_row(v, "toast-row", TOAST_LIFT, popup(&c).key("toast").radius(999.0).px(14.0).py(7.0).child(text(msg.to_string()).font(styles::popup()).color(c.text).truncate().shrink().min_w(0.0)))
}

/// Where the notice sits: in the toast's place, or just above the toast while one shows.
fn notice_lift(v: V) -> f32 {
    if v.is_vault() && v.s.toast_shows() { TOAST_LIFT + PILL_H + PILL_GAP } else { TOAST_LIFT }
}

/// The row that holds a pill at the note area's foot, `lift` above it: a toast, a toast with
/// Undo, or the notice. The pill centres in the part of the note area no drawer covers, 16 px
/// clear of the drawer's edge and of the window's (`foot_room`).
fn foot_row(v: V, key: &'static str, lift: f32, pill: El) -> El {
    let left = if v.left_over() { super::sidebar::W } else { 0.0 };
    let right = if v.right_over() { super::outline::PANEL_W } else { 0.0 };
    row().key(key).pin(Some(left), None, Some(right), Some(lift)).px(16.0).justify_center().raise().child(pill.shrink().min_w(0.0))
}

/// The room above the top pill that the band keeps solid.
const BAND_CLEAR: f32 = 16.0;

/// The fade above the band's solid part.
const BAND_FADE: f32 = 32.0;

/// How far the toast band, fade included, reaches up from the note area's bottom edge, or None
/// when no pill shows: 16 px above the top pill (a toast, the notice, or the notice over a
/// toast), then the fade.
pub(super) fn band_h(v: V) -> Option<f32> {
    let toast = v.s.toast_shows().then_some(PILL_H);
    let notice = v.s.notice.as_deref().map(|m| notice_fit(v, m).1);
    let stack = match (toast, notice) {
        (Some(t), Some(n)) => t + PILL_GAP + n,
        (t, n) => t.or(n)?,
    };
    Some(TOAST_LIFT + stack + BAND_CLEAR + BAND_FADE)
}

/// Reports the fresh caret rect to the unified reveal controller after an editor key/edit while a
/// foot band is visible, so a deferred caret reveal glides smoothly above the band (`REVEALMOD`).
pub fn caret_reveal(v: V) -> Option<El> {
    let seq = v.s.reader.reveal.awaiting_caret()?;
    let caret = v.caret?;
    let view = v.caret_view?;
    let band = band_h(v)?;
    if caret[1] >= view[1] + view[3] || caret[1] + caret[3] <= view[1] {
        return None;
    }
    let key = format!("caret-reveal-{seq}-{}", (caret[1] * 2.0).round() as i64);
    Some(el().key(key).pin(Some(0.0), Some(0.0), None, None).w(0.0).h(0.0).on_layout_tx(move |s: &mut App, _, tx| s.reveal_caret(seq, caret, view, band, tx)))
}

/// Reports where the match find brings into view was laid out, with the bands it goes
/// between (`App::place_match`): an empty point at the window's root, keyed by where the match
/// lies, so a new one reports each time the match moves, and the band going meanwhile does not
/// stop it.
pub fn find_placed(v: V) -> Option<El> {
    let (n, r, view) = v.place_at?;
    let (roof, band) = (super::editor::match_roof(v), band_h(v).unwrap_or(0.0));
    let key = format!("find-placed-{n}-{}", (r[1] * 2.0).round() as i64);
    Some(el().key(key).pin(Some(0.0), Some(0.0), None, None).w(0.0).h(0.0).on_layout_tx(move |s: &mut App, _, tx| s.place_match(n, r, view, roof, band, tx)))
}

/// While a pill shows at the note area's foot, the page fades out under it, in both modes, so
/// no line of the note reads behind or beside it: the page's colour from the note area's bottom
/// edge to 16 px above the top pill, then a 32 px fade above that (`band_h`). Presses go
/// through it to the page, and it fades in and out over 140 ms. As it appears or grows, what
/// the user is on comes above it (`App::band_laid_out`).
pub fn toast_band(v: V) -> Option<El> {
    let h = band_h(v)?;
    let (caret, find_at, roof) = (v.caret, v.find_at, super::editor::match_roof(v));
    let bg = Roles::new(v.p).bg;
    let stops = [(0.0, bg.alpha(0.0)), (BAND_FADE / h, bg), (1.0, bg)];
    let fade = rune::SpringPair::new(rune::Spring::new(0.14, 0.0), rune::Spring::new(0.14, 0.0));
    Some(
        el().key("toast-band")
            .pin(Some(0.0), None, Some(0.0), Some(0.0))
            .h(h)
            .linear_gradient(std::f32::consts::PI, stops)
            .inert()
            .transition(rune::Transition::opacity().springs(fade))
            .on_layout_tx(move |s: &mut App, r, tx| s.band_laid_out(r, caret, find_at, roof, tx)),
    )
}

/// A pill's button: Undo on the toast, Dismiss on the notice. 24 px tall, so it takes a press
/// over the pill's height less 3 px at each edge.
fn pill_button(c: &Roles, key: &str) -> El {
    // A press leaves the keyboard where it was: the pill is not part of what is being done.
    row().key(key).h(24.0).px(10.0).gap(8.0).center_y().radius(999.0).hover(c.hover).role("button").keeps_text_selection()
}

/// What the toast with Undo shows of its sentence in `room`. When the room is short it gives
/// up parts in this order, so the text never runs under the button: the key hint, then the
/// end of the name, down to its first eight letters and "…", then the name ("Moved to the
/// trash"). Undo always shows whole.
#[derive(Debug, PartialEq)]
enum UndoFit {
    /// The name, or its start and "…", and whether the key hint shows.
    Named(String, bool),
    /// "Moved to the trash".
    Short,
}

/// The longest start of `name` that fits `w` with "…" after it, keeping at least `least`
/// letters; None when even that does not fit.
fn cut(name: &str, w: f32, least: usize, style: rune::TextStyle) -> Option<String> {
    if width(name, style) <= w {
        return Some(name.to_string());
    }
    let chars: Vec<char> = name.chars().collect();
    let with = |n: usize| format!("{}…", chars[..n].iter().collect::<String>().trim_end());
    (1..chars.len()).rev().map(with).take_while(|t| t.chars().count() > least.min(chars.len() - 1)).find(|t| width(t, style) <= w)
}

fn undo_fit(room: f32, name: &str, key: &str) -> UndoFit {
    let body = styles::popup();
    let button = |hint: bool| 20.0 + width("Undo", body) + if hint { 8.0 + width(key, keys(11.0)) } else { 0.0 };
    // The pill's 14 px and 4 px of padding, and the 6 px between the sentence and the button.
    let free = |hint: bool| room - 24.0 - button(hint) - width("Moved ", body) - width(" to the trash", body);
    if width(name, body) <= free(true) {
        return UndoFit::Named(name.to_string(), true);
    }
    cut(name, free(false), 8, body).map_or(UndoFit::Short, |t| UndoFit::Named(t, false))
}

/// After a delete, the toast's place holds what went and an Undo button with its key, until
/// the button or the key is pressed or `folders::UNDO_FOR` is up (`undo_fit`).
pub fn undo_toast(v: V, name: &str) -> El {
    let c = Roles::new(v.p);
    let body = styles::popup();
    let key = keymap::label(v.s.preset(), "app.undo-delete");
    let fit = undo_fit(foot_room(v), name, &key);
    let hint = matches!(fit, UndoFit::Named(_, true));
    let undo = pill_button(&c, "toast-undo")
        .label(format!("Undo ({key})"))
        .on_press_tx(|s: &mut App, tx| s.undo_delete(tx))
        .child(text("Undo").font(body).color(c.accent))
        .children(hint.then(|| text(key).font(keys(11.0)).color(c.faint)));
    let words = |t: &str| text(t.to_string()).font(body).color(c.text);
    let msg = row().key("toast-msg").shrink().min_w(0.0).center_y();
    let msg = match fit {
        UndoFit::Named(shown, _) => msg.child(words("Moved ")).child(words(&shown)).child(words(" to the trash")),
        // Below the short sentence's own width (a window narrower than Quire allows) it is cut.
        UndoFit::Short => msg.child(words("Moved to the trash").truncate().shrink().min_w(0.0)),
    };
    let pill = row().gap(6.0).center_y().shrink().min_w(0.0).child(msg).child(undo);
    foot_row(v, "toast-row", TOAST_LIFT, popup(&c).key("toast").radius(999.0).pl(14.0).pr(4.0).py(3.0).child(pill))
}

/// The notice's text width and its pill's height in its room: on one line the toast's 30 px,
/// else wrapped at the widest the room allows, and as wide as its longest line (HUG).
fn notice_fit(v: V, msg: &str) -> (f32, f32) {
    let body = styles::popup();
    let room = foot_room(v) - 24.0 - 20.0 - width("Dismiss", body);
    let whole = width(msg, body);
    if whole <= room {
        return (whole, PILL_H);
    }
    // The least whole-point width that breaks the lines as the room does: the longest line's,
    // or a few points more where a line needs its trailing space to fit. Whole points, as Rune
    // lays a text box out on the pixel grid.
    let room = room.max(0.0);
    let at_room = rune::text::measure(msg, &[], body, Some(room));
    let w = (0..8).map(|d| at_room.0.ceil() + d as f32).find(|&w| w <= room && rune::text::measure(msg, &[], body, Some(w)) == at_room).unwrap_or(room);
    (w, at_room.1 + 14.0)
}

/// The notice: what went wrong, in the toast's place, or just above the toast while one shows.
/// It is a pill in the toast's family: on one line the toast's shape, with "Dismiss" where
/// Undo sits; wrapped, the same padding and type with corners of half the one-line height.
/// Esc dismisses it too (RD-85, WR-77).
pub(super) fn notice(v: V, msg: &str) -> El {
    let c = Roles::new(v.p);
    let body = styles::popup();
    let (w, h) = notice_fit(v, msg);
    let dismiss = pill_button(&c, "notice-x").label("Dismiss").on_press(|s: &mut App| s.notice = None).child(text("Dismiss").font(body).color(v.p.mute));
    // The text's first line centres on the button's 24 px.
    let words = row().py(4.0).child(text(msg.to_string()).font(body).color(c.readable_err()).wrap().w(w));
    let pill = super::widgets::floating(row(), &c)
        .key("notice")
        .radius(if h > PILL_H { PILL_H / 2.0 } else { 999.0 })
        .pl(14.0)
        .pr(4.0)
        .py(3.0)
        .gap(6.0)
        .items_start()
        .child(words)
        .child(dismiss);
    foot_row(v, "notice-row", notice_lift(v), pill)
}

/// The progress line (RD-37, RD-38, WR-178): 2 px of accent along the top of the note area,
/// one line for both modes. Paint order sits above `find-narrow-band` (35) so reading progress
/// stays visible when narrow Find is open (`FINDPOLISH`).
pub fn progress(v: V) -> El {
    let c = Roles::new(v.p);
    el().key("progress").pin(Some(0.0), Some(0.0), None, None).h(2.0).w(0.0).fill(c.accent).paint_order(38).live_rect(&v.s.reader.bar)
}

/// The width the "reloaded" mark takes, its 8 px of margin included.
pub fn reloaded_w() -> f32 {
    8.0 + 6.0 + 5.0 + width("reloaded", sans(11.0, 400.0))
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
        .child(text("reloaded").font(sans(11.0, 400.0)).color(accent))
}
