//! Unified floating toolbar background and mini-bar buttons (`SP-02`, `LK-10`, `LK-13`).
//!
//! Shared by preview block actions, map cards, the edit sheet, the map zoom HUD, and the map
//! add-toolbar. Every caller shares `MINI_BAR_PX` (2 pt), `MINI_BAR_GAP` (1 pt), `MINI_BTN`
//! (24 pt), `MINI_ICON` (12 pt), and `Elevation::Toolbar`.

use rune::{Color, El, Icon, icon, row};

use crate::theme::Pal;
use crate::ui::tokens::{Elevation, Zone, radius, size, state};

pub const MINI_BTN: f32 = size::MINI_BTN;
pub const MINI_ICON: f32 = size::MINI_ICON;
pub const MINI_BAR_H: f32 = size::MINI_BAR_H;
pub const MINI_BAR_PX: f32 = size::MINI_BAR_PX;
pub const MINI_BAR_GAP: f32 = size::MINI_BAR_GAP;
pub const MINI_BAR_RADIUS: f32 = radius::CONTROL;
pub const MINI_BTN_RADIUS: f32 = radius::INNER;

/// Width of a floating action mini-bar holding `n` uniform icon buttons.
pub fn mini_bar_w(n: usize) -> f32 {
    2.0 * MINI_BAR_PX + n as f32 * MINI_BTN + n.saturating_sub(1) as f32 * MINI_BAR_GAP
}

/// Shared floating toolbar backing (mini-bars, formatting toolbar, toasts, notices and canvas
/// HUD): 94% opaque page background, 1 pt border, and `Elevation::Toolbar` scaled by `k`.
/// Intentionally omits `backdrop_blur` to keep partial-damage redraws and Vulkan stability.
pub fn toolbar_surface(e: El, p: &Pal, k: f32) -> El {
    Elevation::Toolbar.apply(e.fill(p.bg.alpha(0.94)).border(k, p.line), p, k)
}

/// Shared floating mini-toolbar container (preview blocks, map cards, edit sheet, HUD), scaled by `k`.
pub fn mini_bar_at(p: &Pal, key: &str, k: f32) -> El {
    toolbar_surface(
        row()
            .key(key)
            .h(MINI_BAR_H * k)
            .px(MINI_BAR_PX * k)
            .gap(MINI_BAR_GAP * k)
            .center_y()
            .radius(MINI_BAR_RADIUS * k),
        p,
        k,
    )
}

/// Shared floating mini-toolbar container at 1× scale.
pub fn mini_bar(p: &Pal, key: &str) -> El {
    mini_bar_at(p, key, 1.0)
}

/// Uniform icon-only button for a floating mini-bar at scale `k`, with tooltip and accessible
/// label. Uses `Zone::Chrome.icon_rest(p)` (`>= 3.2:1`) by default.
pub fn mini_button_at(p: &Pal, key: &str, glyph: &'static Icon, help: &str, tint: Option<(Color, Color)>, k: f32) -> El {
    let (rest, hot) = tint.unwrap_or_else(|| (Zone::Page.icon_rest(p), p.ink));
    row()
        .key(key)
        .size(MINI_BTN * k, MINI_BTN * k)
        .center()
        .radius(MINI_BTN_RADIUS * k)
        .hover(p.hover)
        .hover_group()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(help)
        .help(help)
        .child(icon(glyph, MINI_ICON * k).color(rest).group_color(hot))
}

/// Uniform `24 × 24` icon-only button for a floating mini-bar, with tooltip.
pub fn mini_button(p: &Pal, key: &str, glyph: &'static Icon, help: &str, tint: Option<(Color, Color)>) -> El {
    mini_button_at(p, key, glyph, help, tint, 1.0)
}

/// Disabled opacity for mini-bar buttons (matches `state::DISABLED_CONTENT`, 38%).
pub const MINI_DISABLED_ALPHA: f32 = state::DISABLED_CONTENT;
