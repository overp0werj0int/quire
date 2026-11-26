//! Unified icon and toggle buttons (`SZ-02`, `SZ-03`, `LK-10`, `LK-13`, `IX-12`).
//!
//! Button target size, icon glyph size and corner radius are coupled through `IconButtonSize`,
//! rest ink is contrast-lifted per `Zone` (`>= 3.2:1`), and accessible `role` is always set.

use rune::{Color, El, Icon, Tx, icon, row, text};

use super::Control;
use crate::app::App;
use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::focus;
use crate::ui::styles::nav;
use crate::ui::tokens::{FocusRing, Zone, radius, size, space, state};

/// Coupled target size, glyph size and corner radius for icon buttons (`SZ-02`, `SZ-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconButtonSize {
    /// Inside floating mini-bars, find bar, or bordered inputs: 24 × 24, 12 pt icon, radius 4.
    Mini,
    /// Inside the title bar and crumb: 24 × 24, 14 pt icon, radius 6.
    Bar,
    /// Standalone in a panel or dialog header: 28 × 28, 14 pt icon, radius 6.
    Panel,
    /// Formatting toolbar: 28 × 28, 16 pt icon, radius 6.
    Toolbar,
}

impl IconButtonSize {
    pub const fn target(self) -> f32 {
        match self {
            IconButtonSize::Mini | IconButtonSize::Bar => size::CONTROL_BAR,
            IconButtonSize::Panel | IconButtonSize::Toolbar => size::CONTROL_PANEL,
        }
    }

    pub const fn icon(self) -> f32 {
        match self {
            IconButtonSize::Mini => 12.0,
            IconButtonSize::Bar | IconButtonSize::Panel => 14.0,
            IconButtonSize::Toolbar => 16.0,
        }
    }

    pub const fn radius(self) -> f32 {
        match self {
            IconButtonSize::Mini => radius::INNER,
            IconButtonSize::Bar | IconButtonSize::Panel | IconButtonSize::Toolbar => radius::CONTROL,
        }
    }
}

/// Active-state visual treatment and accessibility role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleKind {
    /// Plain push button (`role="button"`).
    Button,
    /// Chrome toggle (`role="switch"`): hover fill + hairline + ink when active.
    Switch(bool),
    /// Mode choice (`role="radio"`): hover fill + hairline + ink when selected.
    Radio(bool),
    /// Panel footer toggle (`role="switch"`): panel `selected` fill + panel `text` when active.
    PanelSwitch(bool),
    /// Panel push button with open-layer highlight (`role="button"`): panel `selected` fill when active.
    PanelActive(bool),
    /// Formatting toolbar toggle (`role="switch"`): active wash + accent hairline + accent icon.
    AccentSwitch(bool),
}

/// Builder for Quire's unified icon button and icon toggle.
pub struct IconButton<'a> {
    p: &'a Pal,
    key: String,
    glyph: &'static Icon,
    help: String,
    aria_label: Option<String>,
    size: IconButtonSize,
    zone: Zone,
    toggle: ToggleKind,
    disabled: bool,
    tint: Option<(Color, Color)>,
    outlined: bool,
    count: Option<usize>,
    ring: FocusRing,
    focus_stop: bool,
    keeps_selection: bool,
    no_window_drag: bool,
}

impl<'a> IconButton<'a> {
    pub fn new(p: &'a Pal, key: impl Into<String>, glyph: &'static Icon, help: impl Into<String>) -> Self {
        Self {
            p,
            key: key.into(),
            glyph,
            help: help.into(),
            aria_label: None,
            size: IconButtonSize::Panel,
            zone: Zone::Chrome,
            toggle: ToggleKind::Button,
            disabled: false,
            tint: None,
            outlined: false,
            count: None,
            ring: FocusRing::Standard,
            focus_stop: false,
            keeps_selection: false,
            no_window_drag: false,
        }
    }

    /// Standard modal dialog close button (`SZ-02`, `SZ-08`): 28 × 28, 14 pt close glyph.
    pub fn close(p: &'a Pal, key: impl Into<String>, help: impl Into<String>) -> Self {
        Self::new(p, key, &crate::ui::icons::CLOSE, help)
            .size(IconButtonSize::Panel)
            .aria_label("Close")
    }

    pub fn size(mut self, size: IconButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn zone(mut self, zone: Zone) -> Self {
        self.zone = zone;
        self
    }

    pub fn toggle(mut self, kind: ToggleKind) -> Self {
        self.toggle = kind;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Overrides `(rest, hover)` icon colours (for example, accent tint on an unlocked lock icon).
    pub fn tint(mut self, rest: Color, hot: Color) -> Self {
        self.tint = Some((rest, hot));
        self
    }

    /// Renders a bordered surface button (such as the sidebar `+` New note button).
    pub fn outlined(mut self) -> Self {
        self.outlined = true;
        self
    }

    /// Optional numeric count shown beside the icon (11 pt panel font, `SZ-09`).
    pub fn count(mut self, count: Option<usize>) -> Self {
        self.count = count.filter(|&n| n > 0);
        self
    }

    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn ring(mut self, ring: FocusRing) -> Self {
        self.ring = ring;
        self
    }

    pub fn focus_stop(mut self) -> Self {
        self.focus_stop = true;
        self
    }

    pub fn keeps_text_selection(mut self) -> Self {
        self.keeps_selection = true;
        self
    }

    pub fn no_window_drag(mut self) -> Self {
        self.no_window_drag = true;
        self
    }

    fn build_shell(self) -> (El, bool) {
        let p = self.p;
        let c = Roles::panel(p);
        let interactive = !self.disabled;
        let target = self.size.target();
        let glyph_sz = self.size.icon();
        let rad = self.size.radius();
        let a11y = self.aria_label.unwrap_or_else(|| self.help.clone());
        let hover_bg = self.zone.hover(p);
        let (rest_ink, hot_ink) = self.tint.unwrap_or_else(|| {
            let hot = match self.zone {
                Zone::Panel => c.text,
                Zone::Chrome | Zone::Page => p.ink,
            };
            (self.zone.icon_rest(p), hot)
        });

        let base = if self.focus_stop && interactive { focus::stop(row()) } else { row() };
        let mut e = base.key(self.key).center().radius(rad).label(a11y).help(self.help);

        if self.count.is_some() {
            e = e.min_w(target).h(target).px(space::PX_6).gap(space::GAP_TIGHT);
        } else {
            e = e.size(target, target);
        }

        if self.no_window_drag {
            e = e.no_window_drag();
        }

        if self.disabled {
            return (
                e.disabled(true)
                    .child(icon(self.glyph, glyph_sz).color(rest_ink.alpha(state::DISABLED_CONTENT))),
                false,
            );
        }

        let (icon_el, count_ink) = match self.toggle {
            ToggleKind::Button => {
                e = e.role("button");
                if self.outlined {
                    e = e
                        .fill(hover_bg)
                        .hover(self.zone.selected(p))
                        .border(1.0, p.line)
                        .border_layout();
                    (icon(self.glyph, glyph_sz).color(hot_ink), c.faint)
                } else {
                    e = e.hover(hover_bg).hover_group();
                    (icon(self.glyph, glyph_sz).color(rest_ink).group_color(hot_ink), c.faint)
                }
            }
            ToggleKind::Switch(on) => {
                e = e.role("switch").checked(on);
                if on {
                    e = e.fill(hover_bg).border(1.0, p.line);
                    (icon(self.glyph, glyph_sz).color(hot_ink), hot_ink)
                } else {
                    e = e.hover(hover_bg).hover_group();
                    (icon(self.glyph, glyph_sz).color(rest_ink).group_color(hot_ink), c.faint)
                }
            }
            ToggleKind::Radio(on) => {
                e = e.role("radio").checked(on);
                if on {
                    e = e.fill(hover_bg).border(1.0, p.line);
                    (icon(self.glyph, glyph_sz).color(hot_ink), hot_ink)
                } else {
                    e = e.hover(hover_bg).hover_group();
                    (icon(self.glyph, glyph_sz).color(rest_ink).group_color(hot_ink), c.faint)
                }
            }
            ToggleKind::PanelSwitch(on) => {
                e = e
                    .role("switch")
                    .checked(on)
                    .fill(if on { c.selected } else { Color::TRANSPARENT })
                    .hover(c.hover)
                    .hover_group();
                let ic = if on {
                    icon(self.glyph, glyph_sz).color(c.text)
                } else {
                    icon(self.glyph, glyph_sz).color(rest_ink).group_color(c.text)
                };
                (ic, if on { c.text } else { c.faint })
            }
            ToggleKind::PanelActive(on) => {
                e = e
                    .role("button")
                    .fill(if on { c.selected } else { Color::TRANSPARENT })
                    .hover(c.hover)
                    .hover_group();
                let ic = if on {
                    icon(self.glyph, glyph_sz).color(c.text)
                } else {
                    icon(self.glyph, glyph_sz).color(rest_ink).group_color(c.text)
                };
                (ic, if on { c.text } else { c.faint })
            }
            ToggleKind::AccentSwitch(on) => {
                e = e.role("switch").checked(on);
                if on {
                    e = e
                        .fill(p.act)
                        .border(1.0, p.acc.alpha(if p.dark { 0.45 } else { 0.35 }));
                    (icon(self.glyph, glyph_sz).color(p.acc), p.acc)
                } else {
                    e = e.hover(hover_bg).hover_group();
                    (icon(self.glyph, glyph_sz).color(rest_ink).group_color(hot_ink), c.faint)
                }
            }
        };

        e = self.ring.apply(e, p);
        if self.keeps_selection {
            e = e.keeps_text_selection();
        }
        e = e.child(icon_el);
        if let Some(n) = self.count {
            e = e.child(text(n.to_string()).font(nav(11.0, false)).color(count_ink));
        }
        (e, true)
    }

    pub fn on_press(self, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
        let (e, interactive) = self.build_shell();
        if interactive { e.control(run) } else { e }
    }

    pub fn el(self) -> El {
        self.build_shell().0
    }
}
