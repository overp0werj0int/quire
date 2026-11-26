//! The welcome screen: create or open a folder, or pick a recent one.

use rune::{Color, El, ScrollSpec, col, el, row, text, text_input};

use super::components::Button;
use super::components::{Control, MARK, MARK_ON_INK, dots, label};
use super::{V, baseline_shift};
use crate::app::App;
use crate::store::tilde;
use super::styles::{caption, literal, sans};

pub fn welcome(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let bar = row()
        .key("welcome-bar")
        .h(super::titlebar::BAR_H)
        .w_full()
        .pl(14.0)
        .center_y()
        .window_drag()
        .child(dots(p))
        .child(el().grow(1.0))
        .child(el().w(v.trailing));

    // The logo's horizontal lockup: the mark, a gap of a quarter of its width, the wordmark.
    let lockup = row()
        .key("lockup")
        .gap(14.0)
        .center_y()
        .label("Quire")
        .child(rune::icon(if p.dark { &MARK_ON_INK } else { &MARK }, 48.0).a11y_hidden())
        .child(label("quire", sans(52.0, 600.0).lh(52.0).tracking(-0.045 * 52.0), p.ink).a11y_hidden());
    let title = col()
        .key("brand")
        .gap(12.0)
        .child(lockup)
        .child(label("A plain editor for folders of Markdown files.", sans(14.0, 400.0).lh(20.0), p.mute));

    let actions = match &s.creating {
        None => col()
            .key("actions")
            .gap(8.0)
            .child(card(v, "create", "Create a new folder", "Start an empty notebook anywhere on your disk.", |s, tx| {
                let mut e = rune::TextEdit::new("");
                e.focused = true;
                s.creating = Some(e);
                tx.focus("new-vault", false);
            }))
            .child(card(v, "open", "Open a folder…", "Use any folder of .md files you already have.", |s, tx| s.open_folder_dialog(tx))),
        Some(edit) => {
            let path = tilde(&s.new_vault_path());
            col()
                .key("creating")
                .gap(12.0)
                .padding(16.0)
                .border(1.0, p.line)
                .border_layout()
                .radius(super::tokens::radius::FIELD)
                .child(label("Folder name", sans(12.5, 500.0), p.ink2))
                .child(
                    super::components::input_frame(p, "new-vault-box", super::components::InputSize::Dialog, false, edit.focused)
                        .px(super::tokens::space::PAD_INPUT_X)
                        .child(
                            text_input(edit, super::styles::field())
                                .key("new-vault")
                                .single_line()
                                .grow(1.0)
                                .color(p.ink)
                                .label("Folder name")
                                .placeholder("My notes", super::styles::placeholder_ink(p))
                                .caret(p.ink, p.sel)
                                .on_edit(|s: &mut App| s.creating.get_or_insert_with(|| rune::TextEdit::new("")))
                                .on_input_key(|s: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx| {
                                    for a in s.keymap.lookup(crate::keymap::Ctx::Field, ev) {
                                        match a.id {
                                            "field.submit" => {
                                                s.create_vault(tx);
                                                return true;
                                            }
                                            "field.cancel" => return s.escape(tx),
                                            _ => {}
                                        }
                                    }
                                    if matches!(ev.key, rune::Key::Escape) && !ev.mods.logo && s.keymap.chords("field.cancel").contains(&"Esc") {
                                        return s.escape(tx);
                                    }
                                    false
                                }),
                        ),
                )
                .child(label(path, literal(11.5), p.mute))
                .child(
                    row()
                        .key("create-buttons")
                        .gap(8.0)
                        .justify_end()
                        .child(Button::outline(p, "cancel", "Cancel").on_press(|s: &mut App, tx| {
                            s.creating = None;
                            tx.focus("create", tx.by_keys());
                        }))
                        .child(Button::primary(p, "create-go", "Create").on_press(|s, tx| s.create_vault(tx))),
                )
        }
    };

    let mut recent = col().key("recent").gap(2.0).child(caption("recent", false, p).pb(8.0));
    let name_style = sans(14.0, 500.0);
    let small = literal(11.5);
    let shift = baseline_shift(name_style, small);
    for (i, r) in s.persist.vaults.iter().enumerate() {
        let n = s.counts.get(&r.path).copied().unwrap_or(0);
        let path = r.path.clone();
        let forget = r.path.clone();
        recent = recent.child(
            row()
                .key(format!("recent-{i}"))
                .gap(12.0)
                .py(9.0)
                .px(10.0)
                .mx(-10.0)
                .radius(8.0)
                .hover(p.hover)
                .role("button")
                .label(r.name.clone())
                .control(move |s: &mut App, tx| s.open_vault(path.clone(), tx))
                .on_secondary_press(move |s: &mut App, at, tx| {
                    s.open_menu(crate::layer::MenuOf::RecentVault(forget.clone()), (at.x, at.y), tx);
                })
                .child(label(r.name.clone(), name_style, p.ink))
                .child(text(tilde(&r.path)).font(small).color(p.mute).truncate().grow(1.0).shrink().mt(shift))
                .child(label(format!("{n} {}", if n == 1 { "note" } else { "notes" }), sans(12.0, 400.0), p.mute).mt(baseline_shift(name_style, sans(12.0, 400.0)))),
        );
    }

    let inner = col().key("welcome-inner").w_full().max_w(440.0).gap(40.0).child(title).child(actions).child(recent);
    let content = col().key("welcome-content").w_full().min_h((v.vh - super::titlebar::BAR_H).max(0.0)).pt(24.0).px(24.0).pb(72.0).center().child(inner);
    col()
        .key("welcome")
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .fill(p.bg)
        .child(bar)
        .child(col().key("welcome-scroll").grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(content))
}

fn card(v: V, key: &str, title: &str, desc: &str, run: fn(&mut App, &mut rune::Tx)) -> El {
    let p = v.p;
    row()
        .key(key)
        .gap(16.0)
        .py(14.0)
        .px(16.0)
        .center_y()
        .border(1.0, p.line)
        .border_layout()
        .radius(super::tokens::radius::FIELD)
        .fill(p.bg)
        .hover_group()
        .role("button")
        .label(title)
        .control(move |s: &mut App, tx| run(s, tx))
        .child(
            col()
                .grow(1.0)
                .gap(3.0)
                .child(label(title, sans(14.5, 500.0), p.ink))
                .child(label(desc, sans(12.5, 400.0), p.mute)),
        )
        .child(label("→", sans(15.0, 400.0), p.mute))
        // The hover border: CSS changes the border colour; this ring fades in over it.
        .child(el().key("ring").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(super::tokens::radius::FIELD).border(1.0, p.mark).fill(Color::TRANSPARENT).group_opacity(0.0, 1.0))
}
