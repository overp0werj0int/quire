//! Browseable `.trash` recovery modal: lists deleted notes and folders with Restore and armed Empty Trash.

use rune::{ScrollSpec, col, el, icon, row, text};

use super::V;
use super::components::{Button, ButtonVariant, dialog_frame, dialog_plane, label};
use super::icons::{FILE_TEXT, FOLDER, TRASH};
use super::layer::Card;
use super::styles::{self, nav, sans};
use super::tokens::{radius, size, space};
use crate::app::App;
use crate::layer::TrashModal;
use crate::md;
use crate::roles::Roles;

fn relative_time(deleted_at_ns: u128) -> String {
    if deleted_at_ns == 0 {
        return "just now".to_string();
    }
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let secs = now_ns.saturating_sub(deleted_at_ns) / 1_000_000_000;
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 172_800 {
        "yesterday".to_string()
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

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
            let mut list = col().w_full().gap(space::PX_4).role("list").label("Trashed items").focus_group();
            for (idx, entry) in entries.iter().enumerate() {
            let display = if entry.is_dir {
                entry.original_rel.rsplit('/').next().unwrap_or(&entry.name).to_string()
            } else {
                md::base(&entry.original_rel).to_string()
            };
            let when = relative_time(entry.deleted_at);
            let sub = if entry.is_dir {
                let parent = md::dir(&entry.original_rel);
                if parent.is_empty() { format!("Folder · {when}") } else { format!("Folder · {parent} · {when}") }
            } else {
                let parent = md::dir(&entry.original_rel);
                if parent.is_empty() { when } else { format!("{parent} · {when}") }
            };
            let trash_name = entry.name.clone();
            let trash_name_del = entry.name.clone();
            let safe_key = entry.name.replace('/', "|");
            let restore_key = format!("trash-restore:{safe_key}");
            let delete_key = format!("trash-delete:{safe_key}");
            let armed_del = modal.armed_delete.as_deref() == Some(entry.name.as_str());
            let glyph = if entry.is_dir { &FOLDER } else { &FILE_TEXT };
            let mut info = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(display.clone()).font(sans(13.0, 500.0)).color(p.ink).truncate())
                .child(text(sub).font(nav(11.0, false)).color(c.faint).truncate());
            if !entry.snippet.is_empty() {
                info = info.child(text(entry.snippet.clone()).font(sans(11.5, 400.0)).color(p.mute).truncate());
            }
            let mut restore = Button::outline(p, restore_key, "Restore")
                .sm()
                .aria_label(format!("Restore {display}"))
                .help(format!("Restore {display} to {}", entry.original_rel));
            if idx == 0 {
                restore = restore.autofocus();
            }
            let restore = restore.on_press(move |s: &mut App, tx| s.restore_trash(&trash_name, tx));
            let del_btn = Button::new(p, delete_key, if armed_del { "Delete?" } else { "Delete" })
                .variant(if armed_del { ButtonVariant::ArmedDestructive } else { ButtonVariant::Destructive })
                .sm()
                .aria_label(if armed_del {
                    format!("Confirm permanently delete {display}")
                } else {
                    format!("Permanently delete {display}")
                })
                .help(if armed_del {
                    format!("Click again to permanently delete {display}")
                } else {
                    format!("Permanently delete {display} from .trash")
                })
                .on_press(move |s: &mut App, tx| s.delete_trash_item_press(&trash_name_del, tx));
            list = list.child(
                row()
                    .w_full()
                    .px(10.0)
                    .py(7.0)
                    .gap(8.0)
                    .center_y()
                    .radius(radius::FIELD)
                    .hover(p.hover)
                    .child(icon(glyph, 15.0).color(p.mute))
                    .child(info)
                    .child(restore)
                    .child(del_btn),
            );
        }
        col()
            .key("trash-list")
            .w_full()
            .max_h(320.0)
            .scroll(ScrollSpec { stick_bottom: false })
            .child(list)
    };
    let empty_btn = if entries.is_empty() {
        Button::outline(p, "trash-empty", "Empty trash").disabled(true).el()
    } else if modal.armed_empty {
        Button::new(p, "trash-empty", "Confirm empty?")
            .variant(ButtonVariant::ArmedDestructive)
            .aria_label("Confirm empty trash")
            .help("Click again to permanently delete every item in .trash")
            .on_press(|s: &mut App, tx| s.empty_trash_press(tx))
    } else {
        Button::destructive(p, "trash-empty", "Empty trash")
            .help("Permanently delete every item in .trash")
            .on_press(|s: &mut App, tx| s.empty_trash_press(tx))
    };
    let head_title = if entries.is_empty() { "Trash".to_string() } else { format!("Trash ({})", entries.len()) };
    let close_btn = if entries.is_empty() {
        Button::outline(p, "trash-close", "Close").autofocus().on_press(|s, tx| s.close_layer(tx))
    } else {
        Button::outline(p, "trash-close", "Close").on_press(|s, tx| s.close_layer(tx))
    };
    let panel = dialog_frame(p, "trash-dialog", Some(size::DIALOG_W_MD))
        .role("dialog")
        .label("Trash")
        .child(
            col()
                .w_full()
                .padding(space::PAD_DIALOG)
                .gap(space::GAP_SECTION)
                .child(
                    col()
                        .gap(space::GAP_FIELD)
                        .child(
                            row()
                                .gap(space::GAP_CONTROL)
                                .center_y()
                                .child(icon(&TRASH, 18.0).color(p.mute))
                                .child(label(head_title, styles::title(), p.ink)),
                        )
                        .child(
                            label(
                                "Deleted notes and folders stay in .trash until you restore them or empty the trash.",
                                styles::desc(),
                                p.mute,
                            )
                            .wrap(),
                        ),
                )
                .child(body)
                .child(
                    row()
                        .w_full()
                        .gap(space::GAP_CONTROL)
                        .mt(space::PX_4)
                        .center_y()
                        .child(empty_btn)
                        .child(el().grow(1.0))
                        .child(close_btn),
                ),
        );
    Card { el: panel, plane: Some(dialog_plane()) }
}
