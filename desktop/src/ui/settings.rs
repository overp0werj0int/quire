//! The settings sheet.

use rune::{Color, El, Key, ScrollSpec, Tx, col, el, icon, row, text, text_input};

use super::V;
use super::components::{Button, Control, SCROLL_FADE, dialog_frame, faded, label, seg, settings_seg, switch};
use super::layer::Card;
use super::icons::{CLOSE, SEARCH};
use super::tokens::{radius, size, space};
use super::{settings_mcp, settings_sync};
use crate::app::{App, SECS, Sec};
use super::styles::{self, literal, note, sans};
use crate::theme::{self, EDITOR_FONTS};

/// Settings, a box in the middle of the window (`super::layer`).
pub(super) fn settings(v: V) -> Card {
    let p = v.p;
    let s = v.s;
    // The sections are tabs: Tab stops on the open one, and the arrow keys open the others.
    let mut tabs = col().key("set-tabs").gap(2.0).role("tablist").focus_group();
    for (id, lbl) in SECS {
        let on = s.set_sec == id;
        let mut tab = row().key(format!("nav-{lbl}"));
        // The open section's tab takes the focus, so no text field shows a caret unasked.
        if on {
            tab = tab.autofocus();
        }
        tabs = tabs.child(
            tab
                .h(30.0)
                .px(10.0)
                .center_y()
                .radius(6.0)
                .fill(if on { p.act } else { Color::TRANSPARENT })
                .hover_group()
                .role("tab")
                .control(move |s: &mut App, _: &mut Tx| {
                    if s.set_sec != id {
                        s.set_sec = id;
                        s.key_edit = None;
                    }
                })
                .focus_selected(on)
                .child(label(lbl, sans(13.5, if on { 500.0 } else { 400.0 }), if on { p.ink } else { p.ink2 }).group_color(p.ink)),
        );
    }
    let nav = col().key("set-nav").w(nav_w(v)).h_full().py(18.0).px(10.0).fill(p.side).child(styles::caption("settings", false, p).px(10.0).pb(10.0)).child(tabs);
    let title = SECS.iter().find(|x| x.0 == s.set_sec).map_or("", |x| x.1);
    let head = row()
        .key("set-head")
        .h(58.0)
        .w_full()
        .pl(pad(v))
        .pr(16.0)
        .center_y()
        .justify_between()
        .child(label(title, sans(17.0, 600.0), p.ink))
        .child(super::components::IconButton::close(p, "set-close", "Close").on_press(|s: &mut App, tx: &mut Tx| s.close_layer(tx)));
    let body = match s.set_sec {
        Sec::Appearance => appearance(v),
        Sec::Editor => editor(v),
        Sec::Files => files(v),
        Sec::Sync => settings_sync::sync(v),
        Sec::Mcp => settings_mcp::mcp(v),
        Sec::Keys => keys(v),
    };
    let content = col()
        .key("set-content")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .child(head)
        .child(
            col()
                .key("set-scroll")
                .shrink()
                .min_h(0.0)
                .w_full()
                .scroll(ScrollSpec { stick_bottom: false })
                .scroll_margin(SCROLL_FADE)
                .child(faded("set", v.p.bg, col().key(format!("set-body-{title}")).w_full().px(pad(v)).pb(28.0).child(body))),
        );
    // The shadow sits on an outer box: a rounded clip would cut it off.
    let panel = dialog_frame(p, "settings-frame", Some(size::DIALOG_W_SETTINGS))
        .max_h(max_h(v))
        .child(
            row()
                .key("settings")
                .w_full()
                .radius(radius::DIALOG)
                .clip_rounded(radius::DIALOG)
                .role("dialog")
                .label("Settings")
                .child(nav)
                .child(content),
        );
    Card { el: panel, plane: Some(col().px(space::DIALOG_MARGIN).pt((v.vh - max_h(v)) / 2.0).items_center()) }
}

/// The tallest the settings box grows: the window less a 24 point margin, up to 720 points.
fn max_h(v: V) -> f32 {
    (v.vh - space::DIALOG_MARGIN_BOTH).min(720.0)
}

/// Whether the window is small: the section list and the page's margins narrow in it.
fn small(v: V) -> bool {
    v.vw < size::NARROW_NOTE_W
}

/// The section list's width.
fn nav_w(v: V) -> f32 {
    if small(v) { 150.0 } else { 190.0 }
}

/// The page's side margin.
fn pad(v: V) -> f32 {
    if small(v) { space::PAD_DIALOG } else { space::PAD_DIALOG_WIDE }
}

/// The width of the settings body, for grids that fit columns to it.
pub fn body_width(v: V) -> f32 {
    (v.vw - space::DIALOG_MARGIN_BOTH).min(size::DIALOG_W_SETTINGS) - nav_w(v) - 2.0 * pad(v)
}

/// Whether a row stacks its control under its title and description: when the body is too
/// narrow for both side by side with a readable description.
pub fn stacked(v: V) -> bool {
    body_width(v) < 460.0
}

/// A settings row: a title and description on the left, a control on the right.
pub fn row_(v: V, key: &str, title: &str, desc: Option<&str>, control: El) -> El {
    let mut left = col().key("text").child(label(title, sans(14.0, 500.0), v.p.ink));
    if let Some(d) = desc {
        left = left.child(desc_text(v, d));
    }
    pair(v, key, left, control)
}

/// A settings row of `left` (its title and description) and `control`: side by side, or one
/// over the other when the body is narrow (`stacked`).
pub fn pair(v: V, key: &str, left: El, control: El) -> El {
    let r = if stacked(v) {
        col().gap(10.0).child(left.w_full()).child(control.self_start().max_w(body_width(v)))
    } else {
        row().gap(24.0).center_y().child(left.grow(1.0).shrink().min_w(0.0)).child(control)
    };
    r.key(key).w_full().py(16.0).border_sides([1.0, 0.0, 0.0, 0.0], v.p.line).border_layout()
}

pub fn desc_text(v: V, d: &str) -> El {
    text(d).font(super::styles::desc()).color(v.p.mute).wrap().mt(3.0).w_full()
}

/// A block with a title and description over its own content.
pub fn block(v: V, key: &str, title: &str, desc: Option<&str>) -> El {
    let p = v.p;
    let mut c = col().key(key).w_full().py(16.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout().child(label(title, super::styles::row_title(), p.ink));
    if let Some(d) = desc {
        c = c.child(desc_text(v, d));
    }
    c
}

fn appearance(v: V) -> El {
    let set = v.s.settings();
    let theme_seg = |id: &'static str, l: &str| seg(l, set.theme == id, move |s, _| s.set_theme(id));
    let prose_id = theme::pick(&theme::PROSE_FONTS, &set.preview_font).id;
    let prev = |f: &theme::FontChoice| {
        let id = f.id;
        seg(f.label, prose_id == id, move |s, _| s.settings_mut().preview_font = id.into())
    };
    let mode_desc = if theme::omarchy().is_some() {
        "Match system follows the light or dark mode of your Omarchy theme."
    } else {
        "Match system follows the light or dark setting of your OS."
    };
    col()
        .key("appearance")
        .w_full()
        .child(row_(v, "theme", "Mode", Some(mode_desc), settings_seg(v, "theme-seg", vec![theme_seg("light", "Light"), theme_seg("dark", "Dark"), theme_seg("system", "Match system")])))
        .child(theme_grid(v, false))
        .child(theme_grid(v, true))
        .child(accent_block(v))
        .child(row_(v, "prev-font", "Preview typeface", Some("Used when reading a note in preview."), settings_seg(v, "prev-seg", theme::PROSE_FONTS.iter().map(prev).collect())))
}

/// The themes for light or dark mode as cards, each with a small picture of its colours.
fn theme_grid(v: V, dark: bool) -> El {
    let p = v.p;
    let s = v.s;
    let picked = s.theme_id(dark);
    let accent = s.settings().accent.clone();
    let mut items: Vec<(&'static str, &'static str)> = theme::THEMES.iter().filter(|t| t.dark == dark).map(|t| (t.id, t.name)).collect();
    if theme::omarchy().is_some() {
        items.push(("omarchy", "Omarchy"));
    }
    let cols = if body_width(v) >= 380.0 { 3 } else { 2 };
    let cw = ((body_width(v) - 10.0 * (cols as f32 - 1.0)) / cols as f32).floor();
    let slot = if dark { "dark" } else { "light" };
    let mut grid = col().key(format!("{slot}-themes")).w_full().gap(10.0).mt(14.0).role("radiogroup").focus_group();
    let card_border = crate::roles::mix(p.bg, p.ink, if p.dark { 0.16 } else { 0.18 });
    for chunk in items.chunks(cols) {
        let mut r = row().w_full().gap(10.0);
        for &(id, name) in chunk {
            let tp = theme::themed(id, dark, &accent);
            let on = picked == id;
            let pic_border = crate::roles::mix(tp.bg, tp.ink, if tp.dark { 0.18 } else { 0.16 });
            let picture = row()
                .key("picture")
                .w_full()
                .h(54.0)
                .radius(6.0)
                .fill(tp.bg)
                .border(1.0, pic_border)
                .border_layout()
                .clip_rounded(6.0)
                .child(el().w(20.0).h_full().fill(tp.side))
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .px(10.0)
                        .pt(11.0)
                        .gap(6.0)
                        .child(el().w(44.0).h(5.0).radius(2.5).fill(tp.ink))
                        .child(el().w(62.0).h(4.0).radius(2.0).fill(tp.mute))
                        .child(row().gap(4.0).child(el().w(20.0).h(4.0).radius(2.0).fill(tp.acc)).child(el().w(28.0).h(4.0).radius(2.0).fill(tp.mark))),
                );
            let card = super::components::choice_card(p, format!("theme-{slot}-{id}"), name, on, Some(card_border), move |s: &mut App, _: &mut Tx| {
                let set = s.settings_mut();
                if dark {
                    set.dark_theme = Some(id.into());
                } else {
                    set.light_theme = Some(id.into());
                }
                // A pick in the other mode's list switches to that mode, so the change shows.
                if set.theme != "system" {
                    set.theme = if dark { "dark".into() } else { "light".into() };
                }
            })
            .w(cw)
            .gap(space::GAP_CONTROL)
            .padding(space::PX_6)
            .pb(space::PX_8)
            .hover(p.hover)
            .child(picture)
            .child(label(name, sans(12.5, if on { 500.0 } else { 400.0 }), if on { p.ink } else { p.ink2 }).truncate());
            r = r.child(card);
        }
        grid = grid.child(r);
    }
    let (title, desc) = if dark { ("Dark theme", "Used in dark mode.") } else { ("Light theme", "Used in light mode.") };
    block(v, &format!("{slot}-theme"), title, Some(desc)).child(grid)
}

/// The accent: the theme's own, a set of swatches, or any `#rrggbb` value.
fn accent_block(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let current = s.settings().accent.trim().to_lowercase();
    let own = theme::theme_pal(&s.theme_id(v.dark), v.dark).acc;
    let set_accent = |s: &mut App, hex: &str| {
        s.settings_mut().accent = hex.to_string();
        if let Some(e) = s.fields.get_mut("accent-hex") {
            e.replace_text(hex.to_string());
            let n = hex.len();
            e.set_selection(n, n);
        }
    };
    let mut swatches = row().key("swatches").w_full().gap(8.0).center_y().flex_wrap().role("radiogroup").focus_group();
    for &(hex, name) in theme::ACCENTS {
        let on = current == hex;
        if hex.is_empty() {
            swatches = swatches.child(
                row()
                    .key("accent-theme")
                    .h(28.0)
                    .pl(8.0)
                    .pr(11.0)
                    .gap(7.0)
                    .center_y()
                    .radius(14.0)
                    .border(1.0, if on { p.acc } else { p.line })
                    .border_layout()
                    .hover(p.hover)
                    .label(name)
                    .help(name)
                    .choice(on, move |s: &mut App, _: &mut Tx| set_accent(s, ""))
                    .child(el().size(14.0, 14.0).radius(7.0).fill(own))
                    .child(label("Theme", sans(12.5, if on { 500.0 } else { 400.0 }), if on { p.ink } else { p.ink2 })),
            );
            continue;
        }
        let mut dot = el()
            .key(format!("accent-{}", name.to_lowercase()))
            .size(24.0, 24.0)
            .radius(12.0)
            .fill(theme::hex(hex))
            .label(name)
            .help(format!("{name} {hex}"))
            .choice(on, move |s: &mut App, _: &mut Tx| set_accent(s, hex));
        if on {
            dot = dot.box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 4.0, color: p.ink2, inset: false }).box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: p.bg, inset: false });
        }
        swatches = swatches.child(dot);
    }
    let typed = s.fields.get("accent-hex").map_or_else(|| current.clone(), |e| e.text.clone());
    let bad = !typed.trim().is_empty() && theme::parse_accent(&typed).is_none();
    let field = super::components::literal_field(v, "accent-hex", "Accent colour", current.clone(), "#7a4fba", false, bad, 120.0, |s, t| {
        let t = t.trim().to_lowercase();
        if t.is_empty() {
            s.settings_mut().accent = String::new();
        } else if theme::parse_accent(&t).is_some() {
            s.settings_mut().accent = if t.starts_with('#') { t } else { format!("#{t}") };
        }
    });
    let custom = row().key("accent-custom").gap(10.0).center_y().mt(12.0).child(label("Custom", sans(12.5, 400.0), p.ink2)).child(el().w(120.0).child(field));
    block(v, "accent", "Accent colour", Some("Links, tags, buttons and the selection use it.")).child(swatches.mt(14.0)).child(custom)
}

fn editor(v: V) -> El {
    let p = v.p;
    let set = v.s.settings();
    let editor_id = theme::editor_font(&set.font).id;
    let mut cards = col().key("font-cards").w_full().gap(10.0).mt(14.0).role("radiogroup").focus_group();
    // As many columns as fit cards wide enough for their sample line, up to three.
    let cols = ((body_width(v) + 10.0) / 178.0).floor().clamp(1.0, 3.0) as usize;
    let mut card_row = row().w_full().gap(10.0);
    for (i, f) in EDITOR_FONTS.iter().enumerate() {
        let on = editor_id == f.id;
        let id = f.id;
        let b = super::components::choice_card(p, format!("font-{id}"), f.label, on, None, move |s: &mut App, _: &mut Tx| {
            s.settings_mut().font = id.into();
        })
        .grow(1.0)
        .min_w(0.0)
        .gap(space::GAP_CONTROL)
        .pt(space::PX_12)
        .px(14.0)
        .pb(14.0)
        .fill(p.bg)
        .child(label(f.label, super::styles::small(), if on { p.acc } else { p.mute }))
        .child(label("## Field notes", note(f.family, 15.0, 22.5), p.ink))
        .child(label("Rain until 14:00.", note(f.family, 13.0, 19.5), p.ink2));
        card_row = card_row.child(b);
        if i % cols == cols - 1 {
            cards = cards.child(std::mem::replace(&mut card_row, row().w_full().gap(10.0)));
        }
    }
    let code_id = theme::pick(&theme::CODE_FONTS, &set.code_font).id;
    let code = |f: &theme::FontChoice| {
        let id = f.id;
        seg(f.label, code_id == id, move |s: &mut App, tx: &mut Tx| {
            s.settings_mut().code_font = id.into();
            tx.set_font_families("Instrument Sans", theme::pick(&theme::CODE_FONTS, id).name);
        })
    };
    let size = set.size;
    let step = |key: &str, t: &str, d: i32| {
        row()
            .key(key)
            .size(28.0, 26.0)
            .center()
            .radius(6.0)
            .hover(p.bg)
            .role("button")
            .label(if d < 0 { "Smaller" } else { "Larger" })
            .control(move |s: &mut App, tx: &mut Tx| {
                let n = (s.settings().size as i32 + d).clamp(crate::store::SIZE_MIN as i32, crate::store::SIZE_MAX as i32) as u32;
                s.set_text_size(n, tx);
            })
            .child(label(t, sans(15.0, 400.0), p.ink2))
    };
    let size_ctl = row()
        .key("size-ctl")
        .gap(2.0)
        .padding(2.0)
        .radius(8.0)
        .fill(p.side)
        .center_y()
        .child(step("size-down", "−", -1))
        .child(row().key("size-label").min_w(48.0).justify_center().child(label(format!("{size} px"), sans(12.5, 400.0), p.ink)))
        .child(step("size-up", "+", 1));
    let width = |id: &'static str, l: &str| seg(l, set.width == id, move |s, _| s.settings_mut().width = id.into());
    let mut c = col()
        .key("editor-set")
        .w_full()
        .child(block(v, "font", "Editor font", Some("The font of a note in source mode.")).child(cards))
        .child(row_(v, "code-font", "Code font", Some("Code blocks and inline code, in both modes."), settings_seg(v, "code-seg", theme::CODE_FONTS.iter().map(code).collect())))
        .child(row_(v, "size", "Font size", Some("The size of the note's text. Headings and code scale with it."), size_ctl))
        .child(row_(v, "width", "Line width", Some("Readable keeps lines to about 72 characters."), settings_seg(v, "width-seg", vec![width("readable", "Readable"), width("full", "Full width")])));
    type Get = fn(&crate::store::Settings) -> bool;
    type Set = fn(&mut crate::store::Settings);
    let toggles: [(&str, &str, &str, Get, Set); 5] = [
        ("toolbar", "Formatting toolbar", "Buttons for bold, links, lists and images above the note.", |s| s.toolbar, |s| s.toolbar = !s.toolbar),
        ("spell", "Spell check", "Underline misspelled words while you type.", |s| s.spellcheck, |s| s.spellcheck = !s.spellcheck),
        ("lists", "Continue lists", "Pressing Enter in a list starts the next item.", |s| s.continue_lists, |s| s.continue_lists = !s.continue_lists),
        ("dim", "Paragraph focus", "Only the paragraph you are writing stays at full strength.", |s| s.paragraph_focus, |s| s.paragraph_focus = !s.paragraph_focus),
        ("typewriter", "Typewriter scrolling", "Keeps the line you are typing at the middle of the window.", |s| s.typewriter_scroll, |s| s.typewriter_scroll = !s.typewriter_scroll),
    ];
    for (key, t, d, get, flip) in toggles {
        c = c.child(row_(v, key, t, Some(d), switch(v, &format!("{key}-switch"), get(set), t, move |s, _| flip(s.settings_mut()))));
    }
    c
}

fn files(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let path = s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    let loc = pair(
        v,
        "location",
        col().key("text").child(label("Folder location", sans(14.0, 500.0), p.ink)).child(text(path).font(literal(12.0)).color(p.mute).wrap().wrap_anywhere().mt(4.0).w_full()),
        Button::outline(p, "switch", "Switch folder…").on_press(|s, tx| s.go_welcome(tx)),
    );
    let upd = s.settings().update_links;
    let al = s.settings().auto_lock_mins;
    let al_opt = |mins: u32, lbl: &str| seg(lbl, al == mins, move |s, _| s.settings_mut().auto_lock_mins = mins);
    col()
        .key("files-set")
        .w_full()
        .child(loc)
        .child(row_(
            v,
            "links",
            "Update links when renaming",
            Some("Rewrites [[links]] in other notes so they still point to the renamed file."),
            switch(v, "links-switch", upd, "Update links when renaming", |s, _| {
                let u = s.settings().update_links;
                s.settings_mut().update_links = !u;
            }),
        ))
        .child(row_(
            v,
            "autolock",
            "Auto-lock unlocked notes",
            Some("Re-locks unlocked notes after a period of inactivity."),
            settings_seg(v, "set-autolock", vec![al_opt(0, "Off"), al_opt(5, "5m"), al_opt(15, "15m"), al_opt(30, "30m")]),
        ))
        .child(row_(v, "sample", "Sample notes", Some("Put Field Notes back the way it started."), Button::outline(p, "restore", "Restore").on_press(|s, tx| s.reset_sample(tx))))
}

fn keys(v: V) -> El {
    use crate::keymap::{self, Ctx, Status};
    let p = v.p;
    let km = &v.s.keymap;
    let editing = v.s.key_edit.as_ref();
    let path_note = crate::store::keybindings_path()
        .map(|pp| format!("Saved in {}. Click any shortcut to rebind it.", crate::store::tilde(&pp)))
        .unwrap_or_else(|| "Click any shortcut to rebind it.".to_string());
    let reset_all = if km.is_empty() {
        el()
    } else {
        Button::outline(p, "keys-reset-all", "Reset all").sm().on_press(|s: &mut App, _: &mut Tx| s.reset_all_key_bindings())
    };
    let mut c = col().key("keys").w_full().child(row_(v, "keybindings-head", "Keybindings", Some(&path_note), reset_all));
    let query_empty = v.s.keys_query.text.is_empty();
    let clear_btn = (!query_empty).then(|| {
        super::components::IconButton::new(p, "keys-search-clear", &CLOSE, "Clear filter")
            .size(super::components::IconButtonSize::Mini)
            .aria_label("Clear shortcut filter")
            .ring(super::tokens::FocusRing::Inset)
            .on_press(|s: &mut App, tx: &mut Tx| {
                s.keys_query = rune::TextEdit::new("");
                s.keys_query.focused = true;
                tx.focus("keys-search", false);
            })
    });
    let search_bar = super::components::input_frame(p, "keys-search-box", super::components::InputSize::Dialog, false, v.s.keys_query.focused)
        .pl(super::tokens::space::PAD_INPUT_X)
        .pr(super::tokens::space::PX_4)
        .gap(super::tokens::space::GAP_CONTROL)
        .child(icon(&SEARCH, 14.0).color(p.mute))
        .child(
            text_input(&v.s.keys_query, super::styles::field())
            .key("keys-search")
            .single_line()
            .grow(1.0)
            .min_w(0.0)
            .color(p.ink)
            .placeholder("Filter shortcuts by name, group, or key…", super::styles::placeholder_ink(p))
            .caret(p.ink, p.sel)
            .label("Filter shortcuts")
            .on_edit(|s: &mut App| &mut s.keys_query)
            .on_input_change(|s: &mut App, _| {
                s.key_edit = None;
            })
            .on_input_key(|s: &mut App, ev: &rune::KeyEvent, tx: &mut Tx| {
                if crate::keys::esc_again(ev, tx) {
                    return true;
                }
                if ev.key == Key::Escape && !s.keys_query.text.is_empty() {
                    s.keys_query = rune::TextEdit::new("");
                    s.keys_query.focused = true;
                    return true;
                }
                false
            }),
    )
    .children(clear_btn);
    c = c.child(el().w_full().pb(6.0).child(search_bar));

    let q = v.s.keys_query.text.trim().to_lowercase();
    let mut total_shown = 0usize;
    let groups = [
        (Ctx::Global, "Everywhere"),
        (Ctx::Reader, "In preview"),
        (Ctx::Writer, "In the editor"),
        (Ctx::Sheet, "In the edit sheet"),
        (Ctx::Map, "In the map"),
        (Ctx::MapEdit, "Naming a map node"),
        (Ctx::Find, "In find"),
        (Ctx::Palette, "In the palette"),
        (Ctx::Field, "In name fields"),
        (Ctx::Help, "In keyboard help"),
    ];
    for (ctx, title) in groups {
        let title_hit = !q.is_empty() && title.to_lowercase().contains(&q);
        let shown: Vec<&'static keymap::Action> = keymap::TABLE
            .iter()
            .filter(|a| a.ctx.first() == Some(&ctx) && !matches!(a.status, Status::Deferred | Status::Dropped) && a.id != "app.escape")
            .filter(|a| {
                q.is_empty()
                    || title_hit
                    || a.name.to_lowercase().contains(&q)
                    || a.id.to_lowercase().contains(&q)
                    || km.display_labels(a.id).iter().any(|l| l.to_lowercase().contains(&q))
            })
            .collect();
        if shown.is_empty() {
            continue;
        }
        total_shown += shown.len();
        c = c.child(styles::caption(title, false, p).key(format!("keys-{title}")).pt(18.0).pb(6.0));
        for a in shown {
            let id = a.id;
            let mouse_only = a.chords.iter().all(|s| s.contains("click"));
            let is_editing = editing.is_some_and(|e| e.action_id == id);
            let err = editing.filter(|e| e.action_id == id).and_then(|e| e.error.as_deref());
            let custom = km.is_custom(id);
            let mut left = col().grow(1.0).shrink().min_w(0.0).gap(2.0);
            let mut title_row = row().gap(6.0).center_y().child(label(a.name, sans(13.5, 400.0), p.ink).wrap().shrink().min_w(0.0));
            if custom {
                title_row = title_row.child(label("custom", sans(11.0, 500.0), p.acc));
            }
            left = left.child(title_row);
            if let Some(msg) = err {
                left = left.child(super::components::error_line(p, msg));
            }
            let right = if is_editing {
                let (rw, rg, rc) = super::focus_ring(p);
                row()
                    .gap(6.0)
                    .center_y()
                    .child(
                        row()
                            .key(format!("key-rec-{id}"))
                            .h(26.0)
                            .px(10.0)
                            .center_y()
                            .radius(6.0)
                            .fill(p.soft)
                            .border(1.0, p.acc)
                            .focusable(true)
                            .focus_ring(rw, rg, rc)
                            .role("button")
                            .label(format!("Recording shortcut for {}", a.name))
                            .child(label("Press shortcut…", sans(12.0, 500.0), p.acc)),
                    )
                    .child(Button::outline(p, format!("key-clear-{id}"), "Clear").sm().on_press(move |s: &mut App, _: &mut Tx| s.clear_key_binding(id)))
                    .child(Button::outline(p, format!("key-cancel-{id}"), "Cancel").sm().on_press(|s: &mut App, _: &mut Tx| s.cancel_key_edit()))
            } else {
                let mut chips = row().gap(5.0).center_y().flex_wrap();
                let labels = km.display_labels(id);
                if labels.is_empty() {
                    chips = chips.child(label("Unbound", sans(12.0, 400.0), p.mute));
                } else {
                    for kb in labels {
                        chips = chips.child(super::components::kbd(p, kb, false));
                    }
                }
                let trigger = if mouse_only {
                    chips
                } else {
                    chips
                        .key(format!("key-edit-{id}"))
                        .px(6.0)
                        .py(3.0)
                        .radius(6.0)
                        .hover(p.hover)
                        .role("button")
                        .label(format!("Change shortcut for {}", a.name))
                        .help("Click to rebind")
                        .control(move |s: &mut App, tx: &mut Tx| s.start_key_edit(id, tx))
                };
                let mut r = row().gap(6.0).center_y().child(trigger);
                if custom {
                    r = r.child(Button::outline(p, format!("key-reset-{id}"), "Reset").sm().on_press(move |s: &mut App, _: &mut Tx| s.reset_key_binding(id)));
                }
                r
            };
            c = c.child(
                row()
                    .key(format!("key-{id}"))
                    .w_full()
                    .gap(16.0)
                    .py(10.0)
                    .center_y()
                    .justify_between()
                    .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
                    .border_layout()
                    .child(left)
                    .child(right),
            );
        }
    }
    // Reference section for sidebar tree and Open shelf keys handled by focus groups.
    const SIDEBAR_KEYS: &[(&str, &[&str])] = &[
        ("Move selection", &["Up", "Down"]),
        ("First / last row", &["Home", "End"]),
        ("Expand or collapse folder / tab outline", &["Right", "Left"]),
        ("Open focused row", &["Enter", "Space"]),
        ("Rename focused note in tree", &["F2"]),
        ("Move focused tab up / down", &["Alt+Up", "Alt+Down"]),
        ("Close focused tab in Open shelf", &["Delete", "Backspace"]),
        ("Row context menu", &["Shift+F10"]),
        ("Return keyboard to note", &["Esc"]),
    ];
    let sb_title = "Sidebar & Open shelf";
    let sb_title_hit = !q.is_empty() && sb_title.to_lowercase().contains(&q);
    let sb_shown: Vec<(usize, &str, &[&str])> = SIDEBAR_KEYS
        .iter()
        .enumerate()
        .filter(|&(_, &(name, chords))| {
            q.is_empty()
                || sb_title_hit
                || name.to_lowercase().contains(&q)
                || chords.iter().any(|ch| keymap::label_of(ch).to_lowercase().contains(&q))
        })
        .map(|(i, &(name, chords))| (i, name, chords))
        .collect();
    if !sb_shown.is_empty() {
        total_shown += sb_shown.len();
        c = c.child(styles::caption(sb_title, false, p).key("keys-sidebar").pt(18.0).pb(6.0));
        for (i, name, chords) in sb_shown {
            let mut chips = row().gap(5.0).center_y().flex_wrap();
            for ch in chords {
                chips = chips.child(super::components::kbd(p, keymap::label_of(ch), false));
            }
            c = c.child(
                row()
                    .key(format!("key-sidebar-{i}"))
                    .w_full()
                    .gap(16.0)
                    .py(10.0)
                    .center_y()
                    .justify_between()
                    .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
                    .border_layout()
                    .child(label(name, sans(13.5, 400.0), p.ink).wrap().shrink().min_w(0.0))
                    .child(chips),
            );
        }
    }
    if total_shown == 0 {
        c = c.child(text("No matching shortcuts.").key("keys-empty").font(sans(13.0, 400.0)).color(p.mute).py(18.0));
    }
    c
}
