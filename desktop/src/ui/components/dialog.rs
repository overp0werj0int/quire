//! Unified modal dialog frames, popup cards, floating bar edges and scroll-edge fades
//! (`LK-03`, `LK-14`, `SP-03`, `SP-08`, `SZ-08`).
//!
//! Every modal dialog (Settings, Lock, Trash, Palette, Help, Edit sheet, Lightbox) takes
//! `radius::DIALOG` (12 pt), `Elevation::Dialog` (theme-aware light/dark shadows) and a readable
//! border edge on dark scrims. Every lightweight popup (menus, find bar, layout picker) takes
//! `Elevation::Popup`.

use rune::{Color, El, col, el};

use crate::roles::{Roles, mix};
use crate::theme::Pal;
use crate::ui::tokens::{Elevation, radius, space};

/// Edge fade height for scrolling modal sheets (`Settings`, `Help`).
pub const SCROLL_FADE: f32 = space::SCROLL_FADE_DIALOG;

/// Outer modal dialog frame: `radius::DIALOG` (12 pt), page background, hairline border lifted on
/// dark themes so the card edge separates cleanly from the scrim, and `Elevation::Dialog`.
pub fn dialog_frame(p: &Pal, key: impl Into<String>, max_w: Option<f32>) -> El {
    let c = Roles::new(p);
    let edge = if p.dark { mix(c.bg, c.text, 0.20) } else { c.hairline };
    let mut e = el()
        .key(key.into())
        .radius(radius::DIALOG)
        .fill(p.bg)
        .border(1.0, edge);
    if let Some(mw) = max_w {
        e = e.w_full().max_w(mw);
    }
    Elevation::Dialog.apply(e, p, 1.0)
}

/// Centered modal plane with uniform `space::DIALOG_MARGIN` (24 pt) horizontal clearance.
pub fn dialog_plane() -> El {
    col().center().px(space::DIALOG_MARGIN)
}

/// Lightweight popup backing (menus, find bar, layout picker): page background, hairline border,
/// and theme-aware `Elevation::Popup`.
pub fn popup_frame(e: El, p: &Pal) -> El {
    let c = Roles::new(p);
    Elevation::Popup.apply(e.fill(c.bg).border(1.0, c.hairline), p, 1.0)
}

/// Floating bar corner radius and 1 pt hairline (formatting toolbar and find bar).
pub fn bar_edge(e: El, c: &Roles) -> El {
    e.radius(radius::CONTROL).border(1.0, c.hairline)
}

/// Scrolling sheet page with top and bottom edge fades (`Settings`, `Help`).
pub fn faded(name: &str, bg: Color, page: El) -> El {
    faded_h(name, bg, SCROLL_FADE, page)
}

/// Like [`faded`], with a custom edge-fade height `height`.
pub fn faded_h(name: &str, bg: Color, height: f32, page: El) -> El {
    let fade = |key: String, top: bool| {
        let stops = if top { [(0.0, bg), (1.0, bg.alpha(0.0))] } else { [(0.0, bg.alpha(0.0)), (1.0, bg)] };
        el().key(key)
            .pin(Some(0.0), Some(if top { 0.0 } else { -height }), Some(0.0), None)
            .h(height)
            .linear_gradient(std::f32::consts::PI, stops)
            .inert()
            .stuck_reveal(12.0)
    };
    let top = el()
        .key(format!("{name}-fade-top-hold"))
        .w_full()
        .h(0.0)
        .sticky_top(0.0)
        .paint_order(1)
        .child(fade(format!("{name}-fade-top"), true));
    let bottom = el()
        .key(format!("{name}-fade-hold"))
        .w_full()
        .h(0.0)
        .sticky_bottom(0.0)
        .child(fade(format!("{name}-fade"), false));
    col().key(format!("{name}-page")).w_full().child(top).child(page).child(bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{THEMES, contrast, theme_pal};

    /// D4: popup and dialog borders show clearly against every theme background.
    #[test]
    fn floating_edge_shows_on_every_theme() {
        for t in THEMES {
            let p = theme_pal(t.id, t.dark);
            let c = Roles::new(p);
            let edge = c.bg.mix(c.hairline.alpha(1.0), c.hairline.a);
            assert!(contrast(edge, c.bg) >= 1.1, "{}: popup edge {:.2}:1", t.id, contrast(edge, c.bg));
            let ring = Elevation::Popup.shadows(p, 1.0)[0].color;
            let old = c.bg.mix(ring.alpha(1.0), ring.a);
            assert!(contrast(edge, c.bg) >= contrast(old, c.bg) * 0.9, "{}: edge visibility regressed", t.id);
        }
    }
}
