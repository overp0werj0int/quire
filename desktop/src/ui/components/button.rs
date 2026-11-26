//! Unified text button (`SZ-01`, `SP-04`, `LK-02`, `LK-10`).
//!
//! One radius (`radius::FIELD`), one horizontal padding (`space::PAD_BUTTON_X`), two heights
//! (`ButtonSize::Sm` 28 pt for inline rows and bars, `ButtonSize::Dialog` 32 pt for dialogs and
//! primary page actions), and typed variants including destructive and busy states.

use rune::{Color, El, Tx, row};

use super::{Control, label};
use crate::app::App;
use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::focus;
use crate::ui::styles::{self, error_ink};
use crate::ui::tokens::{FocusRing, radius, size, space, state};

/// Visual intent of a text button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Solid accent fill (`p.button`) with high-contrast `p.on_acc` label.
    Primary,
    /// Hairline border (`p.line`) with `p.ink` label.
    Outline,
    /// Accent border and readable accent label (for highlighted secondary actions).
    AccentOutline,
    /// Red outline (`p.err`) with WCAG-readable error ink.
    Destructive,
    /// Armed two-step destructive confirmation: solid `p.err` fill with `p.bg` label.
    ArmedDestructive,
}

/// Height preset of a text button (Decision 7A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    /// Compact button inside a floating bar such as Find & Replace (24 pt, inner radius 4, tight ring).
    Bar,
    /// Inline row, banner or compact bar button (28 pt).
    Sm,
    /// Dialog footer or standalone page action button (32 pt).
    Dialog,
}

impl ButtonSize {
    pub const fn height(self) -> f32 {
        match self {
            ButtonSize::Bar => size::CONTROL_BAR,
            ButtonSize::Sm => size::CONTROL_PANEL,
            ButtonSize::Dialog => size::CONTROL_DIALOG,
        }
    }

    const fn px(self) -> f32 {
        match self {
            ButtonSize::Bar => space::PX_8,
            ButtonSize::Sm | ButtonSize::Dialog => space::PAD_BUTTON_X,
        }
    }

    const fn radius(self) -> f32 {
        match self {
            ButtonSize::Bar => radius::INNER,
            ButtonSize::Sm | ButtonSize::Dialog => radius::FIELD,
        }
    }
}

/// Builder for Quire's unified text button.
pub struct Button<'a> {
    p: &'a Pal,
    key: String,
    text: String,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    busy: bool,
    busy_label_key: Option<&'static str>,
    aria_label: Option<String>,
    help: Option<String>,
    trailing: Option<El>,
    fill_override: Option<Color>,
    focus_stop: bool,
    keeps_selection: bool,
    autofocus: bool,
}

impl<'a> Button<'a> {
    pub fn new(p: &'a Pal, key: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            p,
            key: key.into(),
            text: text.into(),
            variant: ButtonVariant::Outline,
            size: ButtonSize::Dialog,
            disabled: false,
            busy: false,
            busy_label_key: None,
            aria_label: None,
            help: None,
            trailing: None,
            fill_override: None,
            focus_stop: false,
            keeps_selection: false,
            autofocus: false,
        }
    }

    pub fn primary(p: &'a Pal, key: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(p, key, text).variant(ButtonVariant::Primary)
    }

    pub fn outline(p: &'a Pal, key: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(p, key, text).variant(ButtonVariant::Outline)
    }

    pub fn destructive(p: &'a Pal, key: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(p, key, text).variant(ButtonVariant::Destructive)
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn bar(mut self) -> Self {
        self.size = ButtonSize::Bar;
        self
    }

    pub fn sm(mut self) -> Self {
        self.size = ButtonSize::Sm;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Optional element key attached to the inner label while `busy` is true.
    pub fn busy_label_key(mut self, key: &'static str) -> Self {
        self.busy_label_key = Some(key);
        self
    }

    pub fn aria_label(mut self, lbl: impl Into<String>) -> Self {
        self.aria_label = Some(lbl.into());
        self
    }

    pub fn help(mut self, tip: impl Into<String>) -> Self {
        self.help = Some(tip.into());
        self
    }

    pub fn trailing(mut self, el: El) -> Self {
        self.trailing = Some(el);
        self
    }

    /// Explicit background fill for outline buttons sitting on tinted banners.
    pub fn fill_bg(mut self, fill: Color) -> Self {
        self.fill_override = Some(fill);
        self
    }

    /// Marks the button as a page/pane focus stop (`focus::stop`) with a visible focus ring.
    pub fn focus_stop(mut self) -> Self {
        self.focus_stop = true;
        self
    }

    pub fn keeps_text_selection(mut self) -> Self {
        self.keeps_selection = true;
        self
    }

    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    fn build_shell(self) -> (El, bool) {
        let p = self.p;
        let interactive = !self.disabled && !self.busy;
        let a11y = self.aria_label.unwrap_or_else(|| self.text.clone());

        let (text_style, text_color) = match self.variant {
            ButtonVariant::Primary => (styles::sans(styles::scale::BODY, 500.0), p.on_acc),
            ButtonVariant::Outline => (
                if self.size == ButtonSize::Bar { styles::sans(styles::scale::SMALL, 500.0) } else { styles::control() },
                if self.disabled { p.mute } else { p.ink },
            ),
            ButtonVariant::AccentOutline => (
                styles::control_medium(),
                if self.disabled { p.mute } else { Roles::readable_on(p.acc, p.bg, 4.5) },
            ),
            ButtonVariant::Destructive => (
                styles::control_medium(),
                if self.disabled { p.mute } else { error_ink(p) },
            ),
            ButtonVariant::ArmedDestructive => (styles::sans(styles::scale::CONTROL, 600.0), p.bg),
        };

        let mut text_el = label(self.text, text_style, text_color);
        if self.busy
            && let Some(k) = self.busy_label_key
        {
            text_el = text_el.key(k);
        }

        let base = if self.focus_stop { focus::stop(row()) } else { row() };
        let mut e = base
            .key(self.key)
            .h(self.size.height())
            .px(self.size.px())
            .gap(space::GAP_CONTROL)
            .center()
            .radius(self.size.radius())
            .role("button")
            .label(a11y);

        if let Some(tip) = self.help {
            e = e.help(tip);
        }

        e = match self.variant {
            ButtonVariant::Primary => {
                let fill = if self.busy { p.button.alpha(state::BUSY) } else { p.button };
                let e = e.fill(fill);
                if interactive { e.hover(p.button_hover) } else { e }
            }
            ButtonVariant::Outline => {
                let mut e = e.border(1.0, p.line).border_layout();
                if let Some(bg) = self.fill_override {
                    e = e.fill(bg);
                }
                if interactive { e.hover(p.hover) } else { e }
            }
            ButtonVariant::AccentOutline => {
                let border = if self.disabled { p.line } else { p.acc.alpha(0.55) };
                let mut e = e.border(1.0, border).border_layout();
                if let Some(bg) = self.fill_override {
                    e = e.fill(bg);
                }
                if interactive { e.hover(p.hover) } else { e }
            }
            ButtonVariant::Destructive => {
                let border = if self.disabled { p.line } else { p.err.alpha(0.65) };
                let mut e = e.border(1.0, border).border_layout();
                if let Some(bg) = self.fill_override {
                    e = e.fill(bg);
                }
                if interactive { e.hover(p.hover) } else { e }
            }
            ButtonVariant::ArmedDestructive => e.fill(p.err),
        };

        if self.disabled {
            let alpha = match self.variant {
                ButtonVariant::Primary | ButtonVariant::ArmedDestructive => state::DISABLED_CONTENT,
                ButtonVariant::Outline | ButtonVariant::AccentOutline | ButtonVariant::Destructive => state::DISABLED_BORDERED,
            };
            e = e.disabled(true).opacity(alpha);
        } else if self.busy {
            e = e.disabled(true);
        }

        if interactive {
            let ring = if self.size == ButtonSize::Bar { FocusRing::Tight } else { FocusRing::Standard };
            e = ring.apply(e, p);
        }
        if self.keeps_selection {
            e = e.keeps_text_selection();
        }
        if self.autofocus {
            e = e.autofocus();
        }

        e = e.child(text_el);
        if let Some(tail) = self.trailing {
            e = e.child(tail);
        }
        (e, interactive)
    }

    /// Finishes an interactive button with its click/Space/Enter handler.
    pub fn on_press(self, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
        let (e, interactive) = self.build_shell();
        if interactive { e.control(run) } else { e }
    }

    /// Finishes a non-interactive (disabled or busy) button.
    pub fn el(self) -> El {
        self.build_shell().0
    }
}
