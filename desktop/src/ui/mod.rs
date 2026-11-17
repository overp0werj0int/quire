//! The window's view.

pub(crate) mod editor;
mod chip;
mod lock;
mod outline;
mod octicons;
mod panel;
mod styles;
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

use rune::{Cx, El, TextStyle, col, el, row};

use crate::app::{App, Screen};
use crate::roles::Roles;
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
        self.is_vault() && self.s.persist.left_open && !self.focus() && self.vw >= crate::app::SIDEBAR_DOCK_W
    }
    /// The sidebar over the page's left edge, in a window too narrow to dock it.
    pub fn left_over(&self) -> bool {
        self.is_vault() && self.s.drawer == Some(crate::app::Drawer::Sidebar) && !self.focus() && self.vw < crate::app::SIDEBAR_DOCK_W
    }
    /// The outline docked beside the page, which makes room for it.
    pub fn right_visible(&self) -> bool {
        self.is_vault() && self.s.persist.right_open && !self.focus() && self.has_doc() && self.vw >= crate::app::OUTLINE_DOCK_W
    }
    /// The outline over the page's right edge, in a window too narrow to dock it.
    pub fn right_over(&self) -> bool {
        self.is_vault() && self.s.drawer == Some(crate::app::Drawer::Outline) && !self.focus() && self.has_doc() && self.vw < crate::app::OUTLINE_DOCK_W
    }
}

/// The keyboard's focus ring on every control Tab reaches (FR): 2 points of the text colour,
/// 3 points outside the control. A selected card or choice wears the accent at most 1 point
/// outside its edge, so focus sits clear of it in another colour, and the two never look
/// alike.
pub(crate) fn focus_ring(p: &Pal) -> (f32, f32, rune::Color) {
    (2.0, 3.0, p.ink)
}

/// How one thing gives way to another in its place: the old one goes at once and the new one
/// fades in over 160 ms, so the two never show together.
pub(crate) fn swap() -> rune::Transition {
    rune::Transition::opacity().springs(rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::NONE))
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
    s.view_w.set(vw);
    map_page(s);
    let v = V { s, p, dark, vw, vh, trailing };
    cx.caption_color(p.ink2);
    let (w, gap, c) = focus_ring(p);
    cx.focus_ring(w, gap, c);

    let focus = v.focus();
    let win_bg = if v.is_vault() && !focus { p.side } else { p.bg };
    let mut root = col().key("root").w_full().h_full().fill(win_bg);
    root = match s.screen {
        Screen::Welcome => root.child(welcome::welcome(v)),
        Screen::Vault => {
            // The sidebar runs the full height; the title bar and the page share the rest. Zen
            // mode's bar spans the window from the frame it shows, so its pill never rides the
            // page's widening as the sidebar goes.
            let bar = if focus { el().key("titlebar-room").h(titlebar::BAR_H) } else { titlebar::titlebar(v) };
            let main = col().key("main").grow(1.0).min_w(0.0).h_full().child(bar).child(editor::body(v));
            let frame = row().key("frame").w_full().h_full().children(v.left_visible().then(|| sidebar::sidebar(v))).child(main);
            let mut root = root.child(frame);
            if let Some(drawer) = drawer(v) {
                root = root.child(drawer_scrim(v)).child(drawer);
            }
            if focus {
                root.child(titlebar::titlebar(v).pin(Some(0.0), Some(0.0), Some(0.0), None)).children(editor::progress(v))
            } else {
                root.child(titlebar::corner(v))
            }
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
    // Under reduced motion Rune still fades what opens, closes and changes opacity, for 180 ms
    // (Apple's reduced motion). PAGE-84 wants none: everything in the window lands at once.
    if reduced { root.instant_changes() } else { root }
}

/// The drawer on screen, if any (`App::shown_drawer`): in a window too narrow to dock it, the
/// sidebar over the page's left edge on the sidebar's colour, or the outline over its right
/// edge on the page's, so the page does not reflow under it. Both slide in from their edge over
/// 160 ms (WR-99). The window corner's buttons stay over the sidebar's top.
fn drawer(v: V) -> Option<El> {
    let slide = rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::new(0.16, 0.0));
    let shadow = |x: f32| rune::Shadow { x, y: 0.0, blur: 22.0, spread: -4.0, color: rune::Color::BLACK.alpha(if v.p.dark { 0.5 } else { 0.12 }), inset: false };
    let (panel, edge) = if v.left_over() {
        let panel = el().key("sidebar-over").pin(Some(0.0), Some(0.0), None, Some(0.0)).fill(v.p.side).box_shadow(shadow(6.0)).label("Sidebar").child(sidebar::sidebar(v));
        (panel, rune::Edge::Leading)
    } else if v.right_over() {
        // Under the title bar and the page's top border, where the docked outline sits.
        let panel = el().key("outline-over").pin(None, Some(titlebar::BAR_H + 1.0), Some(0.0), Some(0.0)).fill(v.p.bg).box_shadow(shadow(-6.0)).label("Outline").child(outline::outline(v));
        (panel, rune::Edge::Trailing)
    } else {
        return None;
    };
    Some(panel.paint_order(35).role("dialog").transition(rune::Transition::move_edge(edge).springs(slide)))
}

/// The scrim behind an open drawer: the modal layers' colour over the page, under the title
/// bar. A press on it closes the drawer and does nothing else. The title bar stays live: its
/// buttons close the drawer and act (`titlebar`), so both drawers answer an outside press by
/// one rule.
fn drawer_scrim(v: V) -> El {
    let fade = rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::new(0.16, 0.0));
    el().key("drawer-scrim")
        .pin(Some(0.0), Some(titlebar::BAR_H), Some(0.0), Some(0.0))
        .paint_order(34)
        .fill(v.p.scrim)
        .on_press_tx(|s: &mut App, tx| s.close_drawer(tx))
        .transition(rune::Transition::opacity().springs(fade))
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
    widgets::floating(row(), &Roles::new(p))
        .key("notice")
        .pin(None, None, Some(16.0), Some(40.0))
        .w(360.0)
        .items_start()
        .gap(12.0)
        .padding(12.0)
        .radius(9.0)
        .paint_order(60)
        .child(widgets::label(msg, styles::sans(12.5, 400.0).lh(18.0), p.err).wrap().grow(1.0).shrink())
        .child(
            widgets::label("dismiss", styles::sans(11.5, 400.0), p.mute)
                .key("notice-x")
                .on_press(|s: &mut App| s.notice = None),
        )
}

#[cfg(test)]
mod tests {
    use crate::theme::{THEMES, contrast, theme_pal};

    /// FR: keyboard focus reads on every theme, against the window and the sidebar (3:1, as
    /// WCAG asks of a focus indicator), differs from the accent that marks a selected choice,
    /// and leaves a gap outside the selected ring, which reaches 1 point past the edge.
    #[test]
    fn focus_differs_from_selection() {
        for t in THEMES {
            let p = theme_pal(t.id, t.dark);
            let (w, gap, c) = super::focus_ring(p);
            for fill in [p.bg, p.side] {
                assert!(contrast(c, fill) >= 3.0, "{}: focus ring {:.2}:1", t.id, contrast(c, fill));
            }
            assert!(contrast(c, p.acc) >= 1.4, "{}: focus ring {:.2}:1 against the accent", t.id, contrast(c, p.acc));
            assert!(w >= 2.0 && gap - 1.0 >= 2.0, "{}: focus ring {w} wide, {gap} out", t.id);
        }
    }
}
