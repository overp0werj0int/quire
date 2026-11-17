//! The window's view.

pub(crate) mod editor;
mod chip;
mod lock;
mod outline;
mod octicons;
mod panel;
mod palette;
mod preview;
mod reader;
mod settings;
mod sheet;
mod settings_mcp;
pub mod settings_sync;
mod sidebar;
pub(crate) mod titlebar;
mod welcome;
pub mod widgets;

pub use editor::follow_link_at_caret;

use rune::{Cx, El, TextStyle, col, row};

use crate::app::{App, Screen};
use crate::theme::{self, Pal};

/// What every builder reads: the state, the palette and the window size.
#[derive(Clone, Copy)]
pub struct V<'a> {
    pub s: &'a App,
    pub p: &'static Pal,
    pub dark: bool,
    pub vw: f32,
    pub vh: f32,
    /// Space the platform's own window buttons take at the trailing edge (Windows).
    pub trailing: f32,
}

impl V<'_> {
    pub fn is_vault(&self) -> bool {
        self.s.screen == Screen::Vault
    }
    pub fn has_doc(&self) -> bool {
        self.is_vault() && self.s.active.as_ref().is_some_and(|a| self.s.files().contains_key(a))
    }
    pub fn focus(&self) -> bool {
        self.is_vault() && self.s.focus && self.has_doc()
    }
    pub fn left_visible(&self) -> bool {
        self.is_vault() && self.s.persist.left_open && !self.focus() && self.vw >= 680.0
    }
    pub fn right_visible(&self) -> bool {
        self.is_vault() && self.s.persist.right_open && !self.focus() && self.has_doc() && self.vw >= 1100.0
    }
}

/// Where a text's baseline sits below its box top, for CSS `align-items: baseline` rows.
pub fn baseline(style: TextStyle) -> f32 {
    let (asc, desc) = theme::metrics(style.family);
    let (a, d) = (theme::px(asc * style.size), theme::px(desc * style.size));
    (style.line_px() - a - d) / 2.0 + a
}

/// The top margin that puts `small`'s baseline on `big`'s.
pub fn baseline_shift(big: TextStyle, small: TextStyle) -> f32 {
    (baseline(big) - baseline(small)).max(0.0)
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    theme::set_scale(cx.env().scale);
    let reduced = cx.system_reduced_motion() || crate::motion::forced();
    cx.reduced_motion(reduced);
    crate::motion::set(reduced);
    let p = s.palette(cx.system_dark());
    let dark = p.dark;
    s.sys_dark.set(cx.system_dark());
    let [vw, vh] = cx.layout_size("root").unwrap_or([1280.0, 800.0]);
    let trailing = cx.chrome_insets().trailing;
    s.view_h.set(vh);
    map_page(s);
    let v = V { s, p, dark, vw, vh, trailing };
    cx.caption_color(p.ink2);

    let focus = v.focus();
    let win_bg = if v.is_vault() && !focus { p.side } else { p.bg };
    let mut root = col().key("root").w_full().h_full().fill(win_bg);
    root = match s.screen {
        Screen::Welcome => root.child(welcome::welcome(v)),
        Screen::Vault => {
            // The sidebar runs the full height; the title bar and the page share the rest.
            let main = col().key("main").grow(1.0).min_w(0.0).h_full().child(titlebar::titlebar(v)).child(editor::body(v));
            let frame = row().key("frame").w_full().h_full().children(v.left_visible().then(|| sidebar::sidebar(v))).child(main);
            let root = root.child(frame);
            if v.focus() { root.children(editor::progress(v)) } else { root }
        }
    };
    if s.tree_menu.is_some() {
        root = root.child(sidebar::tree_menu(v));
    }
    if s.edit_menu.is_some() {
        root = root.child(sidebar::edit_menu(v));
    }
    if s.lock_dialog.is_some() {
        root = root.child(lock::dialog(v));
    }
    if s.dragging.as_ref().is_some_and(|d| d.active) {
        root = root.child(sidebar::drag_ghost(v));
    }
    if s.palette.is_some() {
        root = root.child(palette::palette(v));
    }
    if s.settings_open && v.is_vault() {
        root = root.child(settings::settings(v));
    }
    if let Some(n) = &s.notice {
        root = root.child(notice(v, n));
    }
    if v.is_vault() && s.reader.help {
        root = root.child(reader::help_sheet(v));
    }
    if v.is_vault() && s.sheet.is_some() {
        root = root.child(sheet::sheet(v));
    }
    root
}

/// The reader is on screen: a note open in preview, not locked.
pub(crate) fn reading(s: &App) -> bool {
    s.screen == Screen::Vault && s.mode == crate::app::Mode::Preview && s.active.as_deref().is_some_and(|a| !s.note_locked(a))
}

/// What the page map (`reader::PageMap`) takes from the view: whether the page shows, each
/// block's first line, row key and gap above it. The rows' heights come in as they are laid
/// out, so a scroll runs no view (VER-32).
fn map_page(s: &App) {
    let mut map = s.reader.map.borrow_mut();
    map.reading = reading(s);
    map.starts.clear();
    map.heads.clear();
    map.marks.clear();
    map.marked = None;
    if !map.reading {
        return;
    }
    let doc = s.doc();
    map.lines = doc.line_count;
    map.starts.extend(doc.blocks.iter().map(|b| b.lines.start));
    let b = s.settings().size as f32;
    let keys = || doc.blocks.iter().map(crate::reader::block_key).collect();
    map.set_rows(s.doc_hash(), keys, crate::page::gaps(b, &doc.blocks), crate::page::row_estimate(b));
}

fn notice(v: V, msg: &str) -> El {
    let p = v.p;
    row()
        .key("notice")
        .pin(None, None, Some(16.0), Some(40.0))
        .w(360.0)
        .items_start()
        .gap(12.0)
        .padding(12.0)
        .radius(9.0)
        .fill(p.bg)
        .box_shadow(theme::menu_shadows()[0])
        .box_shadow(theme::menu_shadows()[1])
        .paint_order(60)
        .child(widgets::label(msg, theme::sans(12.5, 400.0).lh(18.0), p.err).wrap().grow(1.0).shrink())
        .child(
            widgets::label("dismiss", theme::mono(11.0, 400.0), p.mute)
                .key("notice-x")
                .on_press(|s: &mut App| s.notice = None),
        )
}
