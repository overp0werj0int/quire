//! The window's view.

pub(crate) mod editor;
mod chip;
mod focus;
mod layer;
mod lists;
mod lock;
mod menu;
mod outline;
mod icons;
mod panel;
pub(crate) mod styles;
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
    /// The writer's caret line in window points, as last drawn, read only while a toast shows
    /// (`editor::doc_scroll`): reading it runs the view again at each caret move.
    pub caret: Option<[f32; 4]>,
    /// The current find match's highlight where the page lays it out, unscrolled, in either
    /// mode, read only while a toast shows with find open (`App::band_laid_out`).
    pub find_at: Option<[f32; 4]>,
    /// The current match waiting to be brought into view, as its `place` number, where the
    /// page laid it out, unscrolled, and where the view is (`App::place_match`).
    pub place_at: Option<(u32, [f32; 4], [f32; 4])>,
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
        self.is_vault() && self.s.drawer() == Some(crate::app::Drawer::Sidebar) && !self.focus() && self.vw < crate::app::SIDEBAR_DOCK_W
    }
    /// The outline docked beside the page, which makes room for it.
    pub fn right_visible(&self) -> bool {
        self.is_vault() && self.s.persist.right_open && !self.focus() && self.has_doc() && self.vw >= crate::app::OUTLINE_DOCK_W
    }
    /// The outline over the page's right edge, in a window too narrow to dock it.
    pub fn right_over(&self) -> bool {
        self.is_vault() && self.s.drawer() == Some(crate::app::Drawer::Outline) && !self.focus() && self.has_doc() && self.vw < crate::app::OUTLINE_DOCK_W
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
/// comes in at 40% and rises to full over 160 ms. So the two never show together, and the place
/// never looks empty (WR-109). Under reduced motion the new one shows whole at once.
pub(crate) fn swap() -> rune::Transition {
    let mut t = rune::Transition::opacity().springs(rune::SpringPair::new(rune::Spring::new(0.16, 0.0), rune::Spring::NONE));
    t.insertion = rune::Effect::opacity(SWAP_FROM);
    t
}

/// The opacity a swapped-in element starts at.
pub(crate) const SWAP_FROM: f32 = 0.4;

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

#[cfg(test)]
thread_local! {
    /// How many times the view has run on this thread: a test's proof that something moved
    /// with no view run (VER-32).
    pub static VIEWS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    #[cfg(test)]
    VIEWS.with(|n| n.set(n.get() + 1));
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
    let caret = (s.writing() && s.foot_shows()).then(|| cx.caret("editor")).flatten();
    let find_at = (s.foot_shows() && s.find_query().is_some()).then(|| cx.layout(crate::reader::FIND_CUR)).flatten();
    let place_at = s.reader.find.place.and_then(|p| Some((p.n, cx.layout(&crate::reader::place_key(p.n))?, cx.layout(crate::reader::SCROLL)?)));
    let v = V { s, p, dark, vw, vh, trailing, caret, find_at, place_at };
    cx.caption_color(p.ink2);
    let (w, gap, c) = focus_ring(p);
    cx.focus_ring(w, gap, c);
    // Tooltips in the panel family, so a name and its key read as one line (KT). The system
    // look otherwise, which on macOS replaces AppKit's own tooltips with Rune's.
    // Wide enough that a long note name shows whole in two or three lines (C7).
    cx.help_look(rune::HelpLook::system(dark).font(styles::tooltip()).max_width(Some(widgets::TIP_WRAP)));

    let focus = v.focus();
    let win_bg = if v.is_vault() && !focus { p.side } else { p.bg };
    let mut root = col().key("root").w_full().h_full().fill(win_bg);
    root = match s.screen {
        Screen::Welcome => root.child(welcome::welcome(v)),
        Screen::Vault => {
            // The sidebar runs the full height; the title bar and the page share the rest. Zen
            // mode's bar spans the window from the frame it shows, so its pill never rides the
            // page's widening as the sidebar goes.
            let slide = rune::Spring::new(0.16, 0.0);
            let slide_pair = rune::SpringPair::new(slide, slide);
            let side = v.left_visible().then(|| sidebar::sidebar(v).transition(rune::Transition::move_edge(rune::Edge::Leading).springs(slide_pair)));
            let bar = if focus { el().key("titlebar-room").h(titlebar::BAR_H) } else { titlebar::titlebar(v) };
            let main = col().key("main").grow(1.0).min_w(0.0).h_full().motion(slide).child(bar).child(editor::body(v));
            let frame = row().key("frame").w_full().h_full().children(side).child(main);
            let root = root.child(frame);
            if focus {
                root.child(titlebar::titlebar(v).pin(Some(0.0), Some(0.0), Some(0.0), None)).children(editor::progress(v))
            } else {
                root.child(titlebar::corner(v))
            }
        }
    };
    if s.dragging.as_ref().is_some_and(|d| d.active) {
        root = root.child(sidebar::drag_ghost(v));
    }
    // In the folder's window the notice sits in the note area, with the toast (`editor::body`).
    if let Some(n) = s.notice.as_deref().filter(|_| !v.is_vault()) {
        root = root.child(reader::notice(v, n));
    }
    root = root.children(reader::find_placed(v));
    root = root.children(layer::layers(v));
    if s.layer_swap {
        // The palette giving way to Settings: the two cards never show together.
        root = root.child_exit(rune::Transition::none());
    }
    // Under reduced motion Rune still fades what opens, closes and changes opacity, for 180 ms
    // (Apple's reduced motion). PAGE-84 wants none: everything in the window lands at once.
    if reduced { root.instant_changes() } else { root }
}

/// A drawer (`crate::layer`): in a window too narrow to dock it, the sidebar over the page's
/// left edge on the sidebar's colour, or the outline over its right edge on the page's, so the
/// page does not reflow under it. Both slide in from their edge over 160 ms (WR-99). The window
/// corner's buttons stay over the sidebar's top. The title bar and the window corner stay live
/// to the pointer: their buttons close the drawer and act (`titlebar`), so both drawers answer
/// them by one rule.
fn drawer(v: V) -> Option<layer::Card> {
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
    let panel = panel.role("dialog").transition(rune::Transition::move_edge(edge).springs(slide));
    Some(layer::Card { el: panel, plane: None })
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
    map.outline.clear();
    map.shelf.clear();
    if !map.reading {
        // The writer's progress takes the reader's measure (WR-178): it needs the line count.
        map.lines = s.active.as_ref().map_or(0, |_| s.active_text().split('\n').count());
        return;
    }
    let doc = s.doc();
    map.lines = doc.line_count;
    map.starts.extend(doc.blocks.iter().map(|b| b.lines.start));
    let b = s.settings().size as f32;
    let keys = || doc.blocks.iter().map(crate::reader::block_key).collect();
    map.set_rows(s.doc_hash(), keys, crate::page::gaps(b, &doc.blocks), crate::page::row_estimate(b));
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
