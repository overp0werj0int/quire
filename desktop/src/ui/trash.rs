//! Browseable `.trash` recovery modal: lists deleted notes and folders with Restore and armed Empty Trash.

use rune::{ScrollSpec, col, el, icon, row, text};

use super::V;
use super::icons::{FILE_TEXT, FOLDER, TRASH};
use super::layer::Card;
use super::styles::{nav, sans};
use super::widgets::{Control, label, outline_button};
use crate::app::App;
use crate::layer::TrashModal;
use crate::md;
use crate::roles::Roles;
use crate::theme;

pub(super) fn dialog(v: V, modal: &TrashModal) -> Card {
    let p = v.p;
    let c = Roles::panel(p);
    let entries = &v.s.trash_entries;
    let body = if entries.is_empty() {
        row()
            .key("trash-list")
            .w_full()
            .py(24.0)
            .center()
            .child(label("Trash is empty.", sans(13.0, 400.0), p.mute))
    } else {
        let mut list = col().w_full().gap(4.0);
        for entry in entries {
            let display = if entry.is_dir {
                entry.original_rel.rsplit('/').next().unwrap_or(&entry.name).to_string()
            } else {
                md::base(&entry.original_rel).to_string()
            };
            let sub = if entry.is_dir {
                let parent = md::dir(&entry.original_rel);
                if parent.is_empty() { "Folder".to_string() } else { format!("Folder · {parent}") }
            } else {
                md::dir(&entry.original_rel).to_string()
            };
            let trash_name = entry.name.clone();
            let restore_key = format!("trash-restore:{}", entry.name.replace('/', "|"));
            let glyph = if entry.is_dir { &FOLDER } else { &FILE_TEXT };
            let mut info = col().grow(1.0).min_w(0.0).gap(1.0).child(text(display.clone()).font(sans(13.0, 500.0)).color(p.ink).truncate());
            if !sub.is_empty() {
                info = info.child(text(sub).font(nav(11.0, false)).color(c.faint).truncate());
            }
            let restore = row()
                .key(restore_key)
                .h(26.0)
                .px(10.0)
                .center()
                .radius(6.0)
                .border(1.0, p.line)
                .border_layout()
                .hover(p.hover)
                .role("button")
                .label(format!("Restore {display}"))
                .help(format!("Restore {display} to {}", entry.original_rel))
                .control(move |s: &mut App, tx| s.restore_trash(&trash_name, tx))
                .child(label("Restore", sans(12.0, 500.0), p.ink));
            list = list.child(
                row()
                    .w_full()
                    .px(10.0)
                    .py(7.0)
                    .gap(10.0)
                    .center_y()
                    .radius(7.0)
                    .hover(p.hover)
                    .child(icon(glyph, 15.0).color(p.mute))
                    .child(info)
                    .child(restore),
            );
        }
        col()
            .key("trash-list")
            .w_full()
            .max_h(300.0)
            .scroll(ScrollSpec { stick_bottom: false })
            .child(list)
    };
    let empty_btn = if entries.is_empty() {
        row()
            .key("trash-empty")
            .h(32.0)
            .px(12.0)
            .center()
            .radius(7.0)
            .border(1.0, p.line)
            .border_layout()
            .role("button")
            .label("Empty trash")
            .child(label("Empty trash", sans(12.5, 500.0), p.mute))
    } else if modal.armed_empty {
        row()
            .key("trash-empty")
            .h(32.0)
            .px(12.0)
            .center()
            .radius(7.0)
            .fill(p.err)
            .role("button")
            .label("Confirm empty trash")
            .help("Click again to permanently delete every item in .trash")
            .control(|s: &mut App, tx| s.empty_trash_press(tx))
            .child(label("Confirm empty?", sans(12.5, 600.0), p.bg))
    } else {
        row()
            .key("trash-empty")
            .h(32.0)
            .px(12.0)
            .center()
            .radius(7.0)
            .border(1.0, p.err)
            .border_layout()
            .hover(p.hover)
            .role("button")
            .label("Empty trash")
            .help("Permanently delete every item in .trash")
            .control(|s: &mut App, tx| s.empty_trash_press(tx))
            .child(label("Empty trash", sans(12.5, 500.0), p.err))
    };
    let head_title = if entries.is_empty() { "Trash".to_string() } else { format!("Trash ({})", entries.len()) };
    let panel = col()
        .key("trash-dialog")
        .w_full()
        .max_w(460.0)
        .padding(24.0)
        .gap(16.0)
        .radius(12.0)
        .fill(p.bg)
        .border(1.0, Roles::new(p).hairline)
        .box_shadow(theme::settings_shadows()[0])
        .box_shadow(theme::settings_shadows()[1])
        .role("dialog")
        .label("Trash")
        .child(
            col()
                .gap(6.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(icon(&TRASH, 18.0).color(p.mute))
                        .child(label(head_title, sans(17.0, 600.0), p.ink)),
                )
                .child(
                    label(
                        "Deleted notes and folders stay in .trash until you restore them or empty the trash.",
                        sans(12.5, 400.0).lh(12.5 * 1.45),
                        p.mute,
                    )
                    .wrap(),
                ),
        )
        .child(body)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .mt(4.0)
                .center_y()
                .child(empty_btn)
                .child(el().grow(1.0))
                .child(outline_button(p, "trash-close", "Close", 32.0, |s, tx| s.close_layer(tx))),
        );
    Card { el: panel, plane: Some(col().center().px(16.0)) }
}
