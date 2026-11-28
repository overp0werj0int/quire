//! Unified sliding glass selection outline (`MAP-055`, `BRD-012`).
//!
//! One persistent keyed glass rim (plus optional tinted base and drop shadow) pinned in the
//! container's coordinate space so Rune glides and morphs it smoothly between items whenever
//! selection moves, paired with Map mode's subtle content zoom (`SelectionGlass::scale_for`).
//!
//! Changing any constant here updates Map, Board, Map Layout Picker, Settings spatial pickers
//! (Light/Dark theme grids, Editor font cards, Sync provider cards), and Command Palette /
//! Quick Switcher in lockstep.

use rune::{El, Shadow, el};

use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::tokens::{Elevation, radius};

/// Default corner radius of the selection glass (`MAP-055`).
pub const GLASS_R: f32 = 10.0;
/// Maximum scale factor of selected spatial content under the glass.
pub const GLASS_K: f32 = 1.035;
/// Maximum pixel expansion per side when scaling large items under the glass, so tall cards or
/// wide panels stay neatly inside their glass margin.
pub const GLASS_GROW: f32 = 4.0;
/// Minimum `[x, y]` margin around a boxed spatial card/tile so the glass rim clears its border.
pub const SPATIAL_MIN_PAD: [f32; 2] = [3.0, 2.0];
/// Mix weight of `c.text` over `c.bg` for `GlassBackdrop::OpaqueBase`.
pub const BASE_MIX: f32 = 0.035;
/// Top-edge glass highlight gradient opacity at full lighting (`lit = 1.0`).
pub const SHEEN_TOP_ALPHA: f32 = 0.07;
/// Top-edge glass rim border gradient opacity at full lighting (`lit = 1.0`).
pub const RIM_TOP_ALPHA: f32 = 0.55;
/// Bottom-edge glass rim border gradient opacity at full lighting (`lit = 1.0`).
pub const RIM_BOT_ALPHA: f32 = 0.12;
/// Inset glow blur radius on the top-lit rim.
pub const INNER_GLOW_BLUR: f32 = 10.0;
/// Inset glow opacity at full lighting (`lit = 1.0`).
pub const INNER_GLOW_ALPHA: f32 = 0.08;

/// Backdrop layer painted alongside the top-lit glass rim.
///
/// Outset drop shadows must never paint above text; boxed cards and tiles apply the drop shadow
/// directly to their own opaque surface via [`SelectionGlass::lift`], while unboxed canvas items
/// (Map) mount [`SelectionGlass::base_el`] behind their nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassBackdrop {
    /// Translucent-tinted opaque base fill + `Elevation::SelectionGlass` shadow, mounted *behind*
    /// unboxed canvas items painted directly over `c.bg` (Map nodes).
    OpaqueBase,
    /// Top-lit glass rim, sheen, and inset glow with no outset drop shadow above the content
    /// (Board cards/headers, Layout picker tiles, Settings grids, Command Palette rows).
    RimOnly,
}

/// Parameterized sliding selection glass builder shared across every selectable view.
pub struct SelectionGlass<'a> {
    base_key: String,
    rim_key: String,
    p: &'a Pal,
    c: &'a Roles,
    rect: [f32; 4],
    radius: f32,
    rim: f32,
    lit: f32,
    backdrop: GlassBackdrop,
    base_order: Option<i16>,
    rim_order: Option<i16>,
}

impl<'a> SelectionGlass<'a> {
    /// Spatial glass selection with Map mode's exact default parameters (`GLASS_R`, `rim = 1.0`,
    /// `lit = 1.0`, `RimOnly` overlay so no shadow ever paints over card text; pair with
    /// [`SelectionGlass::lift`] on the selected opaque card/tile).
    pub fn spatial(prefix: &str, p: &'a Pal, c: &'a Roles, rect: [f32; 4]) -> Self {
        Self {
            base_key: format!("{prefix}-glass"),
            rim_key: format!("{prefix}-selection"),
            p,
            c,
            rect,
            radius: GLASS_R.min(rect[3] * 0.5),
            rim: 1.0,
            lit: 1.0,
            backdrop: GlassBackdrop::RimOnly,
            base_order: None,
            rim_order: Some(25),
        }
    }

    /// Flat list-row glass selection (`RimOnly`, `radius::CONTROL`, no drop-shadow bleed).
    pub fn list_row(prefix: &str, p: &'a Pal, c: &'a Roles, rect: [f32; 4]) -> Self {
        Self {
            base_key: format!("{prefix}-glass"),
            rim_key: format!("{prefix}-selection"),
            p,
            c,
            rect,
            radius: radius::CONTROL.min(rect[3] * 0.5),
            rim: 1.0,
            lit: 1.0,
            backdrop: GlassBackdrop::RimOnly,
            base_order: None,
            rim_order: Some(25),
        }
    }

    /// Overrides the element keys (e.g. `"map-glass"` / `"map-selection"`).
    pub fn keys(mut self, base: impl Into<String>, rim: impl Into<String>) -> Self {
        self.base_key = base.into();
        self.rim_key = rim.into();
        self
    }

    /// Sets the backdrop layer style (`OpaqueBase`, `ShadowOnly`, or `RimOnly`).
    pub fn backdrop(mut self, backdrop: GlassBackdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    /// Overrides corner radius (clamped to half the rect's height).
    pub fn radius(mut self, rad: f32) -> Self {
        self.radius = rad.min(self.rect[3] * 0.5);
        self
    }

    /// Overrides the rim stroke width (e.g. zoom-compensated on Map).
    pub fn rim(mut self, rim: f32) -> Self {
        self.rim = rim;
        self
    }

    /// Sets lighting intensity (`1.0` when focused/active, `0.6` when dimmed behind a layer).
    pub fn lit(mut self, lit: f32) -> Self {
        self.lit = lit;
        self
    }

    /// Overrides paint orders (`None` lets the caller place `base_el` and `rim_el` in tree order).
    pub fn orders(mut self, base: Option<i16>, rim: Option<i16>) -> Self {
        self.base_order = base;
        self.rim_order = rim;
        self
    }

    /// The subtle content zoom scale for an item of size `(w, h)` under the selection glass
    /// (Map's exact formula, returning `1.0` under reduced motion or lite mode).
    pub fn scale_for(w: f32, h: f32, no_zoom: bool) -> f32 {
        if no_zoom {
            return 1.0;
        }
        GLASS_K.min(1.0 + 2.0 * GLASS_GROW / w.max(h).max(1.0))
    }

    /// Expands a boxed spatial item's `[x, y, w, h]` symmetrically to hug its zoomed content.
    pub fn expanded_rect([x, y, w, h]: [f32; 4]) -> [f32; 4] {
        let k = Self::scale_for(w, h, false);
        let dx = (w * (k - 1.0) * 0.5).max(SPATIAL_MIN_PAD[0]);
        let dy = (h * (k - 1.0) * 0.5).max(SPATIAL_MIN_PAD[1]);
        [x - dx, y - dy, w + 2.0 * dx, h + 2.0 * dy]
    }

    /// Exact container-local `[x, y, w, h]` of item `idx` inside a uniform `cols`-wide grid.
    pub fn grid_item_rect(idx: usize, cols: usize, cell_w: f32, cell_h: f32, gap_x: f32, gap_y: f32) -> [f32; 4] {
        let cols = cols.max(1);
        let col_i = idx % cols;
        let row_i = idx / cols;
        [
            col_i as f32 * (cell_w + gap_x),
            row_i as f32 * (cell_h + gap_y),
            cell_w,
            cell_h,
        ]
    }

    /// Applies the selection glass lift shadow directly to a selected opaque card or tile so the
    /// drop shadow always paints behind the card's fill and never dims its text.
    pub fn lift(e: El, p: &Pal, selected: bool) -> El {
        if selected {
            Elevation::SelectionGlass.apply(e, p, 1.0)
        } else {
            e
        }
    }

    /// Builds the optional backdrop element (opaque base + shadow, mounted behind unboxed items).
    pub fn base_el(&self) -> Option<El> {
        let [x, y, w, h] = self.rect;
        let mut e = match self.backdrop {
            GlassBackdrop::OpaqueBase => el()
                .key(self.base_key.clone())
                .pin(Some(x), Some(y), None, None)
                .size(w, h)
                .radius(self.radius)
                .fill(crate::roles::mix(self.c.bg, self.c.text, BASE_MIX))
                .inert(),
            GlassBackdrop::RimOnly => return None,
        };
        e = Elevation::SelectionGlass.apply(e, self.p, 1.0);
        if let Some(order) = self.base_order {
            e = e.paint_order(order);
        }
        Some(e)
    }

    /// Builds the top-lit glass rim element (top sheen gradient, lit border gradient, inset glow).
    pub fn rim_el(&self) -> El {
        let [x, y, w, h] = self.rect;
        let lit = self.lit;
        let mut e = el()
            .key(self.rim_key.clone())
            .pin(Some(x), Some(y), None, None)
            .size(w, h)
            .radius(self.radius)
            .gradient(self.c.text.alpha(SHEEN_TOP_ALPHA * lit), self.c.text.alpha(0.0))
            .border_gradient(self.rim, self.c.text.alpha(RIM_TOP_ALPHA * lit), self.c.text.alpha(RIM_BOT_ALPHA * lit))
            .box_shadow(Shadow { x: 0.0, y: 0.0, blur: INNER_GLOW_BLUR, spread: 0.0, color: self.c.text.alpha(INNER_GLOW_ALPHA * lit), inset: true })
            .inert();
        if let Some(order) = self.rim_order {
            e = e.paint_order(order);
        }
        e
    }

    /// Mounts both backdrop (if any) and top-lit glass rim onto `container`.
    pub fn mount(self, mut container: El) -> El {
        if let Some(base) = self.base_el() {
            container = container.child(base);
        }
        container.child(self.rim_el())
    }
}
