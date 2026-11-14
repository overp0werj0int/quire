//! The window's view.

pub(crate) mod editor;
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
mod titlebar;
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
    /// The heading at the top of the preview, for the outline's dot.
    pub prev_head: usize,
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
    let prev_head = preview_head(s, cx);
    s.reader.top.set(reader_top(s, cx));
    let v = V { s, p, dark, vw, vh, trailing, prev_head };
    cx.caption_color(p.ink2);

    let focus = v.focus();
    let win_bg = if v.is_vault() && !focus { p.side } else { p.bg };
    let mut root = col().key("root").w_full().h_full().fill(win_bg);
    root = match s.screen {
        Screen::Welcome => root.child(welcome::welcome(v)),
        Screen::Vault => {
            // The title bar steps aside while you type (WR-185), and any pointer move brings
            // it back (WR-187).
            let (o, fade) = s.chrome_fade();
            root.child(titlebar::titlebar(v).opacity(o).opacity_motion(fade)).child(editor::body(v)).on_pointer_move_local(|s: &mut App, _, tx| {
                if !s.pointer_moved() {
                    tx.live_only();
                }
            })
        }
    };
    if s.tree_menu.is_some() {
        root = root.child(sidebar::tree_menu(v));
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
fn reading(s: &App) -> bool {
    s.screen == Screen::Vault && s.mode == crate::app::Mode::Preview && s.active.as_deref().is_some_and(|a| !s.note_locked(a))
}

/// The index of the top-level block at the top of the view, found by halving over the blocks'
/// laid-out tops: the last whose top is at or above 21 points under the viewport's top.
fn reader_top(s: &App, cx: &mut Cx<App>) -> usize {
    if !reading(s) {
        return 0;
    }
    let Some(top) = cx.window_layout("doc-scroll").map(|r| r[1] + 21.0) else { return 0 };
    let doc = s.doc();
    let (mut lo, mut hi) = (0usize, doc.blocks.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        match cx.window_layout(&crate::reader::block_key(&doc.blocks[mid])) {
            Some(r) if r[1] <= top => lo = mid + 1,
            _ => hi = mid,
        }
    }
    lo.saturating_sub(1)
}

/// The last preview heading whose top is above the viewport's top plus 100 points.
fn preview_head(s: &App, cx: &mut Cx<App>) -> usize {
    if s.screen != Screen::Vault || s.mode != crate::app::Mode::Preview || !s.persist.right_open {
        return 0;
    }
    let Some(top) = cx.window_layout("doc-scroll").map(|r| r[1]) else { return 0 };
    let doc = s.doc();
    let mut cur = 0;
    for (i, h) in crate::md::headings(s.active_text()).iter().enumerate() {
        let Some(b) = doc.blocks.iter().find(|b| b.lines.start == h.line) else { continue };
        if let Some(r) = cx.window_layout(&crate::reader::block_key(b)) {
            if r[1] <= top + 100.0 {
                cur = i;
            }
        }
    }
    cur
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
