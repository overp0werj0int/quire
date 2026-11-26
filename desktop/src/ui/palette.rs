//! The quick switcher and command palette.

use crate::roles::Roles;
use rune::{Color, Key, KeyEvent, ScrollSpec, Span, Tx, col, el, icon, rich, row, text, text_input};

use super::V;
use super::components::{dialog_frame, faded_h, kbd, key_hint, label, tag_badge};
use super::icons::{LOCK, WARNING};
use super::layer::Card;
use super::tokens::{radius, size, space};
use crate::app::{App, PalMode, Palette};
use crate::commands::{self, pal_data};
use crate::keymap::Ctx;
use super::styles::{keys, sans};

const HEAD_H: f32 = 53.0;
const FOOT_H: f32 = 35.0;
const LAYER_BOTTOM: f32 = 20.0;
const ROW_H: f32 = 36.0;
const LIST_PAD: f32 = 6.0;
/// Slim edge fade that stays below the 14 pt text inside a 36 pt palette row (9.5 pt bottom room),
/// so the last whole visible command row keeps full contrast instead of being washed out.
const PAL_FADE: f32 = 8.0;
/// The widest a row's folder or key runs: about two fifths of the row, so the name keeps the rest.
const SUB_W: f32 = 220.0;

/// The quick switcher or the command palette, near the window's top (`super::layer`).
pub(super) fn palette(v: V, pal: &Palette) -> Option<Card> {
    let d = pal_data(v.s, v.dark)?;
    Some(palette_card(v, d, &pal.q))
}

/// Offscreen command palette built inside `Tx::prepare_text` to warm Instrument Sans font loading
/// and command-row text layouts at exact palette width before the first `Ctrl+Shift+K` (`COLDPACE`).
pub(super) fn prepare_commands(v: V) -> rune::El {
    let d = commands::pal_data_for(v.s, v.dark, PalMode::Commands, "", 0, None);
    let q = rune::TextEdit::new("");
    let card = palette_card(v, d, &q);
    col().w_full().items_center().px(20.0).child(card.el)
}

fn palette_card(v: V, d: commands::PalData, q: &rune::TextEdit) -> Card {
    let p = v.p;
    let cmds = d.mode == PalMode::Commands;
    let moving = d.mode == PalMode::MoveFolder;
    let badge_lbl = if moving { "move" } else if cmds { "run" } else { "open" };
    let input_lbl = if moving { "Move to folder" } else if cmds { "Command palette" } else { "Quick switcher" };
    let input_ph = if moving { "Choose destination folder…" } else if cmds { "Type a command…" } else { "Find a note by name…" };
    let list_lbl = if moving { "Folders" } else if cmds { "Commands" } else { "Notes" };
    let head = row()
        .key("pal-head")
        .h(HEAD_H)
        .w_full()
        .gap(space::GAP_GROUP)
        .px(18.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.line)
        .border_layout()
        .child(tag_badge(p, badge_lbl))
        .child(
            text_input(q, super::styles::palette_input())
                .key("pal-input")
                .single_line()
                .px(2.0)
                .py(1.0)
                .grow(1.0)
                .min_w(0.0)
                .color(p.ink)
                .label(input_lbl)
                .placeholder(input_ph, super::styles::placeholder_ink(p))
                .caret(p.ink, p.sel)
                .on_edit(pal_field)
                .on_input_change(|s: &mut App, _| {
                    if let Some(p) = s.pal_mut() {
                        p.idx = 0;
                    }
                })
                .on_input_key(on_pal_key),
        );

    let mut list = col()
        .key("pal-items")
        .w_full()
        .padding(LIST_PAD)
        .role("listbox")
        .label(list_lbl);
    let roles = Roles::new(p);
    for (i, it) in d.items.iter().enumerate() {
        let on = i == d.idx;
        let mut t = String::new();
        let mut spans = Vec::new();
        for (seg, hit) in &it.segs {
            let a = t.len();
            t.push_str(seg);
            if *hit {
                spans.push(Span { color: Some(p.acc), weight: Some(600.0), ..Span::new(a, t.len()) });
            }
        }
        let action = it.action.clone();
        let mut row_label = t.clone();
        if !it.sub.is_empty() {
            row_label.push_str(" — ");
            row_label.push_str(&it.sub);
        }
        if it.locked {
            row_label.push_str(" (locked)");
        } else if it.unlocked {
            row_label.push_str(" (unlocked)");
        }
        if it.conflict {
            row_label.push_str(" (sync conflict)");
        }
        let lock_badge = (it.locked || it.unlocked).then(|| {
            let (col, tip) = if it.locked { (p.mute, "Locked note") } else { (p.acc, "Unlocked in this session") };
            icon(&LOCK, 13.0).color(col).label(tip).help(tip)
        });
        let conflict_badge = it.conflict.then(|| {
            icon(&WARNING, 13.0).color(roles.readable_err()).label("Sync conflict copy").help("Sync conflict copy")
        });
        list = list.child(
            row()
                .key(format!("pal-{i}"))
                .h(ROW_H)
                .w_full()
                .gap(8.0)
                .px(12.0)
                .center_y()
                .radius(radius::CONTROL)
                .fill(if on { p.act } else { Color::TRANSPARENT })
                .role("option")
                .checked(on)
                .label(row_label)
                .on_hover(move |s: &mut App| {
                    if let Some(p) = s.pal_mut()
                        && p.idx != i
                    {
                        p.idx = i;
                    }
                })
                .on_press_tx(move |s: &mut App, tx| commands::run(s, action.clone(), tx))
                .children(lock_badge)
                .children(conflict_badge)
                .child(rich(t.clone(), spans).font(sans(14.0, 400.0)).color(p.ink).truncate().grow(1.0).shrink().min_w(0.0).help_when_truncated(t))
                // The note's folder, or the command's key cap (`components::kbd`, `SCROLLFADE`).
                .children((!it.sub.is_empty()).then(|| {
                    if cmds {
                        kbd(p, it.sub.clone(), true)
                    } else {
                        text(it.sub.clone()).font(keys(11.5)).color(p.mute).truncate_head().shrink().min_w(0.0).max_w(SUB_W).help_when_truncated(it.sub.clone())
                    }
                })),
        );
    }
    if d.items.is_empty() {
        list = list.child(label("Nothing matches.", sans(13.0, 400.0), p.mute).py(14.0).px(12.0));
    }
    // The list is at most 392 tall, and less in a short window, snapped to whole rows so no
    // command row is sliced in half across the bottom scrollport edge on open.
    let top = (v.vh * 0.11).round();
    let avail = (v.vh - top - LAYER_BOTTOM - HEAD_H - FOOT_H).clamp(0.0, 392.0);
    let visible_rows = ((avail - LIST_PAD) / ROW_H).floor().max(0.0);
    let scroll_max_h = if d.items.is_empty() {
        avail
    } else if d.items.len() as f32 > visible_rows {
        (LIST_PAD + visible_rows * ROW_H).min(avail)
    } else {
        (2.0 * LIST_PAD + visible_rows * ROW_H).min(avail)
    };
    let scroller = col()
        .key("pal-scroll")
        .w_full()
        .max_h(scroll_max_h)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_margin(PAL_FADE)
        .child(faded_h("pal", p.bg, PAL_FADE, list));

    // A key cap in the shared `<kbd>` style, then what it does (C14, SCROLLFADE).
    let hint = |k: String, what: &str| key_hint(p, k, what, p.mute);
    let km = &v.s.keymap;
    let prev_k = km.label("palette.previous");
    let next_k = km.label("palette.next");
    let nav_k = match (prev_k.as_str(), next_k.as_str()) {
        ("", "") => String::new(),
        (a, "") => a.to_string(),
        ("", b) => b.to_string(),
        (a, b) if a.chars().count() == 1 && b.chars().count() == 1 => format!("{a}{b}"),
        (a, b) => format!("{a}/{b}"),
    };
    let run_k = km.label("palette.run");
    let close_k = km.label("palette.close");
    let foot = row()
        .key("pal-foot")
        .h(FOOT_H)
        .w_full()
        .gap(16.0)
        .px(18.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .border_layout()
        .clip_rounded(0.0)
        .children((!nav_k.is_empty()).then(|| hint(nav_k, "select")))
        .children((!run_k.is_empty()).then(|| hint(run_k, if moving { "move" } else if cmds { "run" } else { "open" })))
        .children((!close_k.is_empty()).then(|| hint(close_k, "close")))
        .child(el().grow(1.0))
        .child(if moving {
            el()
        } else if !cmds && d.total > 50 {
            label(format!("Showing 50 of {}", d.total), sans(11.5, 400.0), p.mute)
        } else if cmds {
            hint(km.label("app.quick-switcher"), "for notes")
        } else {
            row()
                .gap(5.0)
                .center_y()
                .child(label("type", sans(11.5, 400.0), p.mute))
                .child(kbd(p, ">", true))
                .child(label("for commands", sans(11.5, 400.0), p.mute))
        });

    let panel = dialog_frame(p, "palette-frame", Some(size::DIALOG_W_PALETTE)).child(
        col()
            .key("palette")
            .w_full()
            .radius(radius::DIALOG)
            .clip_rounded(radius::DIALOG)
            .role("dialog")
            .label(if moving { "Move to folder" } else if cmds { "Commands" } else { "Quick switcher" })
            .child(head)
            .child(scroller)
            .child(foot),
    );
    Card { el: panel, plane: Some(col().items_center().pt(top).px(space::DIALOG_MARGIN).pb(LAYER_BOTTOM)) }
}

/// The palette's field, for its edit hook, which can still run in the frame the palette
/// closes; that last edit lands in a scratch field.
fn pal_field(s: &mut App) -> &mut rune::TextEdit {
    if s.pal().is_some() {
        return &mut s.pal_mut().expect("checked above").q;
    }
    s.field("pal-closed", String::new)
}

fn on_pal_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let dark = s.sys_dark.get();
    let dark = s.is_dark(dark);
    let Some(d) = pal_data(s, dark) else { return false };
    let n = d.items.len().max(1);
    for a in s.keymap.lookup(Ctx::Palette, ev) {
        match a.id {
            "palette.next" => {
                if let Some(p) = s.pal_mut() {
                    p.idx = (d.idx + 1) % n;
                    tx.scroll_into_view(format!("pal-{}", p.idx));
                }
                return true;
            }
            "palette.previous" => {
                if let Some(p) = s.pal_mut() {
                    p.idx = (d.idx + n - 1) % n;
                    tx.scroll_into_view(format!("pal-{}", p.idx));
                }
                return true;
            }
            "palette.run" => {
                if let Some(it) = d.items.into_iter().nth(d.idx) {
                    commands::run(s, it.action, tx);
                }
                return true;
            }
            "palette.back-to-notes" => {
                let Some(p) = s.pal_mut() else { return false };
                if p.q.text.is_empty() && p.mode == PalMode::Commands {
                    p.mode = PalMode::Files;
                    p.idx = 0;
                    return true;
                }
                return false;
            }
            "palette.close" => {
                // IX-03: Esc clears a non-empty filter first; the next Esc closes the palette.
                if let Some(p) = s.pal_mut()
                    && !p.q.text.is_empty()
                {
                    p.q = rune::TextEdit::new("");
                    p.idx = 0;
                    return true;
                }
                s.dismiss(tx);
                return true;
            }
            _ => {}
        }
    }
    // Esc with Shift/Ctrl/Alt still dismisses when palette.close is bound to Esc (MODESC2).
    if matches!(ev.key, Key::Escape) && !ev.mods.logo && s.keymap.chords("palette.close").contains(&"Esc") {
        if let Some(p) = s.pal_mut()
            && !p.q.text.is_empty()
        {
            p.q = rune::TextEdit::new("");
            p.idx = 0;
            return true;
        }
        s.dismiss(tx);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_dispatches_via_keymap_and_respects_modifiers() {
        let dir = std::env::temp_dir().join(format!("quire-pal-km-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Alpha.md"), "# Alpha\n").unwrap();
        std::fs::write(dir.join("Beta.md"), "# Beta\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.keymap.set("palette.next", vec!["Ctrl+N".into()]);
            s.open_palette(PalMode::Files, tx);
        });
        assert_eq!(rt.state.pal().unwrap().idx, 0);

        // Shift+Down must NOT advance when palette.next is rebound to Ctrl+N (nor on default Down).
        rt.update(|s, tx| {
            let shift_down = KeyEvent { key: Key::Down, mods: rune::Mods { shift: true, ..rune::Mods::default() } };
            assert!(!on_pal_key(s, &shift_down, tx));
        });
        assert_eq!(rt.state.pal().unwrap().idx, 0);

        // Custom Ctrl+N binding advances selection!
        rt.update(|s, tx| {
            let ctrl_n = KeyEvent { key: Key::Command('n'), mods: rune::Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..rune::Mods::default() } };
            assert!(on_pal_key(s, &ctrl_n, tx));
        });
        assert_eq!(rt.state.pal().unwrap().idx, 1);

        // IX-03: Esc clears a non-empty query first, then closes the palette on the next Esc.
        rt.update(|s, _| {
            if let Some(p) = s.pal_mut() {
                p.q = rune::TextEdit::new("al");
            }
        });
        let esc = KeyEvent { key: Key::Escape, mods: rune::Mods::default() };
        rt.update(|s, tx| assert!(on_pal_key(s, &esc, tx)));
        assert!(rt.state.pal().is_some(), "first Esc keeps palette open when clearing query");
        assert!(rt.state.pal().unwrap().q.text.is_empty(), "first Esc clears the query");
        assert_eq!(rt.state.pal().unwrap().idx, 0, "clearing query resets selection to 0");
        rt.update(|s, tx| assert!(on_pal_key(s, &esc, tx)));
        assert!(rt.state.pal().is_none(), "second Esc closes the palette");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
