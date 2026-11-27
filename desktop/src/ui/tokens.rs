//! Unified desktop design tokens: spacing, corner radii, control heights, elevation shadows,
//! motion durations, interaction states, focus rings and surface roles.
//!
//! Every component under `ui/components/` reads its geometry and interaction style from here so
//! a change in one place updates every screen.

use rune::{Anim, Color, Curve, El, Shadow, Spring};

use crate::roles::Roles;
use crate::theme::{Pal, rgba};

/// Spacing and distancing scale: 4 pt grid with compact desktop half-steps (`6` and `10`) plus
/// `1` and `2` micro steps.
pub mod space {
    pub const PX_1: f32 = 1.0;
    pub const PX_2: f32 = 2.0;
    pub const PX_4: f32 = 4.0;
    pub const PX_6: f32 = 6.0;
    pub const PX_8: f32 = 8.0;
    pub const PX_10: f32 = 10.0;
    pub const PX_12: f32 = 12.0;
    pub const PX_16: f32 = 16.0;
    pub const PX_24: f32 = 24.0;
    pub const PX_32: f32 = 32.0;

    /// Between adjacent list rows or mini-bar icon buttons.
    pub const GAP_MICRO: f32 = PX_1;
    /// Between an icon and its count, or sibling controls in a tight bar.
    pub const GAP_TIGHT: f32 = PX_4;
    /// Between a field label and its input, or a dialog title and description.
    pub const GAP_FIELD: f32 = PX_6;
    /// Between adjacent buttons or inputs in a row.
    pub const GAP_CONTROL: f32 = PX_8;
    /// Between form groups or items in a header.
    pub const GAP_GROUP: f32 = PX_12;
    /// Between sections inside a dialog or settings page.
    pub const GAP_SECTION: f32 = PX_16;

    /// Track inset inside segmented controls, mini-bars and switches.
    pub const PAD_TRACK: f32 = PX_2;
    /// Horizontal padding inside single-line text inputs and menu rows.
    pub const PAD_INPUT_X: f32 = PX_10;
    /// Horizontal padding inside text buttons.
    pub const PAD_BUTTON_X: f32 = PX_12;
    /// Padding inside modal dialogs.
    pub const PAD_DIALOG: f32 = PX_24;
    /// Wide body padding in Settings.
    pub const PAD_DIALOG_WIDE: f32 = PX_32;

    /// Minimum clearance between a modal dialog and one window edge.
    pub const DIALOG_MARGIN: f32 = PX_24;
    /// Total horizontal or vertical window clearance (`2 * DIALOG_MARGIN`).
    pub const DIALOG_MARGIN_BOTH: f32 = DIALOG_MARGIN * 2.0;

    /// Edge fade height for scrolling side panels.
    pub const SCROLL_FADE_PANEL: f32 = 14.0;
    /// Edge fade height for scrolling modal sheets (Settings, Help).
    pub const SCROLL_FADE_DIALOG: f32 = 28.0;
}

/// Concentric corner radius scale: four steps plus pill (`outer = inner + container_pad`).
pub mod radius {
    /// Controls inside a `CONTROL`-radius bar, row or field, key caps, inline tags.
    pub const INNER: f32 = 4.0;
    /// Icon buttons, list rows, floating bars, segments, sidebar search.
    pub const CONTROL: f32 = 6.0;
    /// Text buttons (primary, outline, destructive), dialog inputs, segmented tracks, cards, menus.
    pub const FIELD: f32 = 8.0;
    /// Board columns (BRD-008).
    pub const COLUMN: f32 = 10.0;
    /// Modal dialogs (Settings, Lock, Trash, Palette, Help, Edit sheet).
    pub const DIALOG: f32 = 12.0;
    /// Fully rounded pills (toasts, notices, count pills).
    pub const PILL: f32 = 999.0;
}

/// Interactive control heights (4 pt steps, all `>= 24 pt` pointer targets) and shared layout
/// dimensions.
pub mod size {
    /// Compact buttons inside a bar (title bar, mini-bar, find bar, toast actions, mode pills).
    pub const CONTROL_BAR: f32 = 24.0;
    /// Standalone panel controls (panel icon buttons, row buttons, segments, switch, search box).
    pub const CONTROL_PANEL: f32 = 28.0;
    /// Dialog buttons and single-line dialog inputs.
    pub const CONTROL_DIALOG: f32 = 32.0;

    /// Menu row height.
    pub const MENU_ROW: f32 = 30.0;
    /// Window title bar height.
    pub const TITLEBAR_H: f32 = 40.0;
    /// Shared left/right panel footer height (`CONTROL_PANEL` + 6 pt top/bottom pad + 1 pt border).
    pub const FOOTER_H: f32 = CONTROL_PANEL + 2.0 * super::space::PX_6 + 1.0;
    /// Docked outline panel width.
    pub const OUTLINE_W: f32 = 236.0;

    /// Narrow modal dialog max width (Lock, Confirm).
    pub const DIALOG_W_SM: f32 = 400.0;
    /// Medium modal dialog max width (Trash).
    pub const DIALOG_W_MD: f32 = 460.0;
    /// Command palette / quick switcher max width.
    pub const DIALOG_W_PALETTE: f32 = 600.0;
    /// Settings dialog max width.
    pub const DIALOG_W_SETTINGS: f32 = 780.0;
    /// Note-area narrow breakpoint.
    pub const NARROW_NOTE_W: f32 = 720.0;

    /// Floating mini-bar geometry.
    pub const MINI_BTN: f32 = CONTROL_BAR;
    pub const MINI_ICON: f32 = 12.0;
    pub const MINI_BAR_PX: f32 = super::space::PAD_TRACK;
    pub const MINI_BAR_GAP: f32 = super::space::GAP_MICRO;
    pub const MINI_BAR_H: f32 = MINI_BTN + 2.0 * MINI_BAR_PX;

    /// Switch geometry.
    pub const SWITCH_W: f32 = 50.0;
    pub const SWITCH_H: f32 = CONTROL_PANEL;
    pub const KNOB_INSET: f32 = super::space::PAD_TRACK;
    pub const KNOB: f32 = SWITCH_H - 2.0 * KNOB_INSET;
}

/// Interaction state opacities: disabled is distinct from busy.
pub mod state {
    /// Disabled icons and plain text labels.
    pub const DISABLED_CONTENT: f32 = 0.38;
    /// Disabled bordered buttons.
    pub const DISABLED_BORDERED: f32 = 0.45;
    /// Active async busy buttons (`Unlocking…`, `Locking…`, `Working…`).
    pub const BUSY: f32 = 0.65;
}

/// Named motion durations and springs.
pub mod motion {
    use super::*;

    /// Hover colour fade duration in seconds (100 ms).
    pub const HOVER_FADE: f32 = 0.1;
    /// Modal dialog and scrim fade spring.
    pub const MODAL_SPRING: f32 = 0.08;
    /// Held board card/column tilt spring (120 ms, BRD-015).
    pub const TILT_SPRING: f32 = 0.12;
    /// In-place swap transition spring (160 ms).
    pub const SWAP_SPRING: f32 = 0.16;
    /// Sidebar and outline drawer slide spring (160 ms).
    pub const DRAWER_SPRING: f32 = 0.16;
    /// Tab switch slide spring (180 ms).
    pub const TAB_SLIDE_SPRING: f32 = 0.18;
    /// Board card/column layout glide and settle duration (180 ms, BRD-016, BRD-019).
    pub const LAYOUT_GLIDE_SPRING: f32 = 0.18;
    /// Zen bar and HUD reveal spring (250 ms).
    pub const ZEN_REVEAL_SPRING: f32 = 0.25;
    /// How long an armed destructive button stays armed before resetting.
    pub const ARMED_LAPSE_SECS: u64 = 2;

    pub fn hover_fade() -> Anim {
        Curve::linear(HOVER_FADE).into()
    }

    pub fn modal_spring() -> Spring {
        Spring::new(MODAL_SPRING, 0.0)
    }

    pub fn tilt_spring() -> Spring {
        Spring::new(TILT_SPRING, 0.0)
    }

    pub fn swap_spring() -> Spring {
        Spring::new(SWAP_SPRING, 0.0)
    }

    pub fn drawer_spring() -> Spring {
        Spring::new(DRAWER_SPRING, 0.0)
    }

    pub fn tab_spring() -> Spring {
        Spring::new(TAB_SLIDE_SPRING, 0.0)
    }

    pub fn layout_glide_spring() -> Spring {
        Spring::new(LAYOUT_GLIDE_SPRING, 0.0)
    }

    pub fn zen_spring() -> Spring {
        Spring::new(ZEN_REVEAL_SPRING, 0.0)
    }
}

/// Theme-aware elevations. Light themes cast warm brown shadows; dark themes cast deep neutral
/// shadows so popups and dialogs separate cleanly on dark backgrounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Floating mini-bars, formatting toolbar, toasts and canvas HUDs.
    Toolbar,
    /// Menus, find bar and lightweight popups.
    Popup,
    /// Modal dialogs: Settings, Lock, Trash, Palette, Help and Edit sheet.
    Dialog,
    /// Raised active segment inside a segmented track.
    Segment,
    /// Switch knob.
    Knob,
}

impl Elevation {
    pub fn shadows(self, p: &Pal, k: f32) -> Vec<Shadow> {
        let ring = if p.dark { Color::BLACK.alpha(0.32) } else { rgba("#1e1405", 0.10) };
        match self {
            Elevation::Toolbar => vec![Shadow {
                x: 0.0,
                y: 6.0 * k,
                blur: 22.0 * k,
                spread: -4.0 * k,
                color: Color::BLACK.alpha(if p.dark { 0.5 } else { 0.12 }),
                inset: false,
            }],
            Elevation::Popup => vec![
                Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0 * k, color: ring, inset: false },
                Shadow {
                    x: 0.0,
                    y: 14.0 * k,
                    blur: 34.0 * k,
                    spread: -10.0 * k,
                    color: if p.dark { Color::BLACK.alpha(0.55) } else { rgba("#281c0a", 0.30) },
                    inset: false,
                },
            ],
            Elevation::Dialog => vec![
                Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0 * k, color: ring, inset: false },
                Shadow {
                    x: 0.0,
                    y: 24.0 * k,
                    blur: 60.0 * k,
                    spread: -16.0 * k,
                    color: if p.dark { Color::BLACK.alpha(0.65) } else { rgba("#281c0a", 0.38) },
                    inset: false,
                },
            ],
            Elevation::Segment => vec![
                Shadow {
                    x: 0.0,
                    y: 1.0 * k,
                    blur: 2.0 * k,
                    spread: 0.0,
                    color: if p.dark { Color::BLACK.alpha(0.35) } else { rgba("#281c0a", 0.14) },
                    inset: false,
                },
                Shadow {
                    x: 0.0,
                    y: 0.0,
                    blur: 0.0,
                    spread: 1.0 * k,
                    color: if p.dark { Color::BLACK.alpha(0.20) } else { rgba("#281c0a", 0.05) },
                    inset: false,
                },
            ],
            Elevation::Knob => vec![Shadow {
                x: 0.0,
                y: 1.0 * k,
                blur: 2.0 * k,
                spread: 0.0,
                color: Color::BLACK.alpha(0.25),
                inset: false,
            }],
        }
    }

    pub fn apply(self, mut e: El, p: &Pal, k: f32) -> El {
        for sh in self.shadows(p, k) {
            e = e.box_shadow(sh);
        }
        e
    }
}

/// Side-drawer shadow matching `Elevation::Toolbar` along the horizontal axis.
pub fn drawer_shadow(p: &Pal, x: f32) -> Shadow {
    Shadow {
        x,
        y: 0.0,
        blur: 22.0,
        spread: -4.0,
        color: Color::BLACK.alpha(if p.dark { 0.5 } else { 0.12 }),
        inset: false,
    }
}

/// Named keyboard focus rings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusRing {
    /// Standard focus glass on controls Tab reaches (2 wide, 3 out, ink).
    Standard,
    /// Tight ring for closely spaced controls such as find-bar buttons and task boxes (2 wide, 1 out).
    Tight,
    /// Single-line text input focus-within ring (1.5 wide, 1 inset, readable faint).
    Input,
    /// Control seated inside a bordered input box, such as search-clear (2 wide, 2 inset, ink).
    Inset,
}

impl FocusRing {
    pub fn spec(self, p: &Pal) -> (f32, f32, Color) {
        match self {
            FocusRing::Standard => (2.0, 3.0, p.ink),
            FocusRing::Tight => (2.0, 1.0, p.ink),
            FocusRing::Input => (1.5, -1.0, Roles::panel(p).faint),
            FocusRing::Inset => (2.0, -2.0, p.ink),
        }
    }

    pub fn apply(self, e: El, p: &Pal) -> El {
        let (w, gap, c) = self.spec(p);
        match self {
            FocusRing::Input => e.focus_within_ring(w, gap, c),
            _ => e.focus_ring(w, gap, c),
        }
    }

    pub fn apply_when(self, e: El, p: &Pal, active: bool) -> El {
        if active { self.apply(e, p) } else { e }
    }
}

/// Background zone that determines hover, selection and contrast-lifted icon inks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Title bar, dialogs, floating bars and welcome screen (`p.bg`).
    Chrome,
    /// Sidebar and outline panels (`p.side`).
    Panel,
    /// Reader page and map canvas (`Roles::new(p).bg`).
    Page,
}

impl Zone {
    pub fn bg(self, p: &Pal) -> Color {
        match self {
            Zone::Chrome | Zone::Page => p.bg,
            Zone::Panel => p.side,
        }
    }

    pub fn hover(self, p: &Pal) -> Color {
        match self {
            Zone::Chrome => p.hover,
            Zone::Panel => Roles::panel(p).hover,
            Zone::Page => Roles::new(p).hover,
        }
    }

    pub fn selected(self, p: &Pal) -> Color {
        match self {
            Zone::Chrome => p.act,
            Zone::Panel => Roles::panel(p).selected,
            Zone::Page => Roles::new(p).selected,
        }
    }

    /// Rest ink for icon buttons on this zone, lifted to at least 3.2:1 graphic contrast on every
    /// theme.
    pub fn icon_rest(self, p: &Pal) -> Color {
        let base = match self {
            Zone::Chrome | Zone::Page => p.mute,
            Zone::Panel => Roles::panel(p).dim,
        };
        Roles::readable_on(base, self.bg(p), 3.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{THEMES, contrast, omarchy_pal, theme_pal};

    fn all_palettes() -> Vec<(&'static str, Pal)> {
        let om = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"").unwrap();
        let mut out: Vec<(&'static str, Pal)> = THEMES.iter().map(|t| (t.id, *theme_pal(t.id, t.dark))).collect();
        out.push(("omarchy", om));
        out
    }

    #[test]
    fn icons_and_focus_rings_meet_contrast_on_every_theme() {
        for (id, p) in all_palettes() {
            for zone in [Zone::Chrome, Zone::Panel, Zone::Page] {
                let cr = contrast(zone.icon_rest(&p), zone.bg(&p));
                assert!(cr >= 3.2, "{id} {zone:?}: icon_rest {cr:.2}:1 (< 3.2)");
            }
            let (_, _, rc) = FocusRing::Standard.spec(&p);
            for fill in [p.bg, p.side] {
                assert!(contrast(rc, fill) >= 3.0, "{id}: focus ring contrast < 3.0");
            }
            let (_, _, ic) = FocusRing::Input.spec(&p);
            for fill in [p.bg, p.side] {
                assert!(contrast(ic, fill) >= 4.5, "{id}: input ring contrast < 4.5");
            }
            assert!(
                contrast(p.on_acc, p.button) >= 4.5,
                "{id}: primary button on_acc contrast {:.2}:1 (< 4.5)",
                contrast(p.on_acc, p.button)
            );
        }
    }

    #[test]
    fn dark_elevations_use_neutral_black_shadows() {
        let ink = theme_pal("ink", true);
        for elev in [Elevation::Toolbar, Elevation::Popup, Elevation::Dialog, Elevation::Segment, Elevation::Knob] {
            for sh in elev.shadows(ink, 1.0) {
                assert_eq!((sh.color.r, sh.color.g, sh.color.b), (0.0, 0.0, 0.0), "{elev:?} dark shadow should be neutral black");
            }
        }
    }

    #[test]
    fn control_heights_meet_minimum_pointer_target() {
        for h in [size::CONTROL_BAR, size::CONTROL_PANEL, size::CONTROL_DIALOG, size::MENU_ROW, size::MINI_BTN] {
            assert!(h >= 24.0, "control height {h} is below 24 pt pointer target");
        }
    }

    fn walk_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                walk_rs(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }

    /// Guardrail: no screen outside `tokens.rs` and the map canvas constructs raw outset drop
    /// shadows or calls the retired `theme::*_shadows` helpers.
    #[test]
    fn no_raw_drop_shadows_outside_tokens() {
        let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let mut files = Vec::new();
        walk_rs(&ui_dir, &mut files);
        let mut hits = Vec::new();
        for path in files {
            let rel = path.strip_prefix(&ui_dir).unwrap().to_string_lossy().replace('\\', "/");
            if rel == "tokens.rs" || rel == "map.rs" {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for (i, line) in src.lines().enumerate() {
                let t = line.trim();
                if t.starts_with("//") {
                    continue;
                }
                if t.contains("_shadows(") {
                    hits.push(format!("{rel}:{}: retired shadow helper: {t}", i + 1));
                }
                if t.contains("Shadow {") && !t.contains("blur: 0.0") && !t.contains("inset: true") {
                    hits.push(format!("{rel}:{}: raw drop shadow outside tokens.rs: {t}", i + 1));
                }
            }
        }
        assert!(hits.is_empty(), "drop shadows must come from ui::tokens::Elevation:\n{}", hits.join("\n"));
    }

    /// Guardrail: `ui/widgets.rs` and `ui/chip.rs` stay deleted and no source file references them.
    #[test]
    fn no_legacy_widgets_or_chip_modules() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert!(!src_dir.join("ui/widgets.rs").exists(), "legacy ui/widgets.rs must stay deleted");
        assert!(!src_dir.join("ui/chip.rs").exists(), "legacy ui/chip.rs must stay deleted");
        let mut files = Vec::new();
        walk_rs(&src_dir, &mut files);
        let mut hits = Vec::new();
        for path in files {
            let rel = path.strip_prefix(&src_dir).unwrap().to_string_lossy().replace('\\', "/");
            if rel == "ui/tokens.rs" {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for (i, line) in src.lines().enumerate() {
                if line.contains("ui::widgets")
                    || line.contains("super::widgets")
                    || line.contains("mod widgets;")
                    || line.contains("ui::chip::")
                    || line.contains("super::chip::")
                    || line.contains("mod chip;")
                {
                    hits.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                }
            }
        }
        assert!(hits.is_empty(), "legacy widgets/chip references found:\n{}", hits.join("\n"));
    }

    /// Guardrail: every modal dialog builder uses the unified `dialog_frame` helper.
    #[test]
    fn dialogs_use_shared_dialog_frame() {
        let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        for rel in ["lock.rs", "trash.rs", "settings.rs", "palette.rs", "reader.rs", "sheet.rs", "map/lightbox.rs"] {
            let src = std::fs::read_to_string(ui_dir.join(rel)).unwrap();
            assert!(src.contains("dialog_frame("), "{rel} must build its modal frame via components::dialog_frame");
        }
    }

    /// Guardrail: form dialogs (`lock.rs`, `welcome.rs`) use `components::input` rather than
    /// hand-rolling styled text input wrappers.
    #[test]
    fn form_dialogs_use_shared_input_component() {
        let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let lock = std::fs::read_to_string(ui_dir.join("lock.rs")).unwrap();
        assert!(lock.contains("password_field("), "lock.rs must use components::password_field");
        assert!(!lock.contains("text_input("), "lock.rs must not hand-roll raw text_input");
        let welcome = std::fs::read_to_string(ui_dir.join("welcome.rs")).unwrap();
        assert!(welcome.contains("input_frame("), "welcome.rs must use components::input_frame");
    }
}
