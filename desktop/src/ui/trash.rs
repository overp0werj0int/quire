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
    let r = Roles::new(p);
    let (rw, rg, rc) = super::focus_ring(p);
    let entries = &v.s.trash_entries;
    let body = if entries.is_empty() {
        row()
            .key("trash-list")
            .w_full()
            .py(24.0)
            .center()
            .child(label("Trash is empty.", sans(13.0, 400.0), p.mute))
    } else {
        let mut list = col().w_full().gap(4.0).role("list").label("Trashed items").focus_group();
        for entry in entries {
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
                .focus_ring(rw, rg, rc)
                .control(move |s: &mut App, tx| s.restore_trash(&trash_name, tx))
                .child(label("Restore", sans(12.0, 500.0), p.ink));
            let del_btn = if armed_del {
                row()
                    .key(delete_key)
                    .h(26.0)
                    .px(10.0)
                    .center()
                    .radius(6.0)
                    .fill(p.err)
                    .role("button")
                    .label(format!("Confirm permanently delete {display}"))
                    .help(format!("Click again to permanently delete {display}"))
                    .focus_ring(rw, rg, rc)
                    .control(move |s: &mut App, tx| s.delete_trash_item_press(&trash_name_del, tx))
                    .child(label("Delete?", sans(12.0, 600.0), p.bg))
            } else {
                row()
                    .key(delete_key)
                    .h(26.0)
                    .px(10.0)
                    .center()
                    .radius(6.0)
                    .border(1.0, p.err.alpha(0.45))
                    .border_layout()
                    .hover(p.hover)
                    .role("button")
                    .label(format!("Permanently delete {display}"))
                    .help(format!("Permanently delete {display} from .trash"))
                    .focus_ring(rw, rg, rc)
                    .control(move |s: &mut App, tx| s.delete_trash_item_press(&trash_name_del, tx))
                    .child(label("Delete", sans(12.0, 500.0), r.readable_err()))
            };
            list = list.child(
                row()
                    .w_full()
                    .px(10.0)
                    .py(7.0)
                    .gap(8.0)
                    .center_y()
                    .radius(7.0)
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
        row()
            .key("trash-empty")
            .h(32.0)
            .px(12.0)
            .center()
            .radius(7.0)
            .border(1.0, p.line)
            .border_layout()
            .role("button")
            .disabled(true)
            .opacity(0.5)
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
