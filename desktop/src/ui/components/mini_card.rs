//! Unified modal mini-card (`MiniCard`): Edit Sheet, Kanban Card Preview/Edit, and Map Lightbox.
//!
//! Enforces Quire's mini-card design-system rules structurally:
//! - Built on `dialog_frame` (`radius::DIALOG` 12, `Elevation::Dialog`).
//! - Three borderless bands: **header** (hover area), **body** (not a hover area, so reading and
//!   typing keep chrome invisible), and **footer** (hover area).
//! - Header and footer share one `hover_group_shared` group; hovering anywhere in either band (or
//!   over the bar itself) reveals the single floating top-right `mini_bar` on `motion::zen_spring()`.
//!   Callers may set `pin_actions(true)` while a destructive discard is armed or an error is shown.
//! - Every action is an icon-only `mini_button` (`24 × 24`, `12 pt` icon, `"Action · Key"` tooltip);
//!   text `Button`s and permanently visible standalone close buttons are never used.
//! - The close/cancel (`×`) icon is passed separately and always appended as the **rightmost** button.

use rune::{El, col, row};

use super::dialog::dialog_frame;
use super::mini_bar::{MINI_BAR_H, mini_bar, mini_bar_w};
use crate::theme::Pal;
use crate::ui::tokens::{motion, space};

/// Standard right inset for the top-right floating mini-bar inside a mini-card.
pub const MINI_CARD_BAR_PR: f32 = space::PX_12;
/// Standard top inset for the top-right floating mini-bar inside a 48 pt mini-card header.
pub const MINI_CARD_BAR_PT: f32 = (48.0 - MINI_BAR_H) / 2.0;

/// Builder for a Quire modal mini-card with shared header/footer hover zones and a top-right
/// icon-only `mini_bar` whose close/cancel (`×`) icon is structurally guaranteed to sit rightmost.
pub struct MiniCard<'a> {
    p: &'a Pal,
    frame_key: String,
    card_key: String,
    hover_group: String,
    actions_key: String,
    actions_zone_key: String,
    label: String,
    width: Option<f32>,
    bar_top: f32,
    bar_right: f32,
    pin_actions: bool,
    leading: Vec<El>,
    close: Option<El>,
    extra: Vec<El>,
}

impl<'a> MiniCard<'a> {
    /// Starts a modal mini-card keyed `frame_key` (e.g. `"sheet-pop"`, `"lightbox"`) with a shared
    /// hover group for its header, footer, and top-right mini-bar.
    pub fn new(p: &'a Pal, frame_key: impl Into<String>, hover_group: impl Into<String>, label: impl Into<String>) -> Self {
        let frame_key = frame_key.into();
        let hover_group = hover_group.into();
        Self {
            p,
            frame_key,
            card_key: "sheet-card".into(),
            hover_group,
            actions_key: "sheet-actions".into(),
            actions_zone_key: "sheet-actions-zone".into(),
            label: label.into(),
            width: None,
            bar_top: MINI_CARD_BAR_PT,
            bar_right: MINI_CARD_BAR_PR,
            pin_actions: false,
            leading: Vec::new(),
            close: None,
            extra: Vec::new(),
        }
    }

    /// Sets the inner card column's key (e.g. `"sheet-card"`, `"card-sheet"`, `"lightbox-card"`).
    pub fn card_key(mut self, key: impl Into<String>) -> Self {
        self.card_key = key.into();
        self
    }

    /// Sets the keys for the floating mini-bar and its pinned wrapper.
    pub fn actions_keys(mut self, bar: impl Into<String>, zone: impl Into<String>) -> Self {
        self.actions_key = bar.into();
        self.actions_zone_key = zone.into();
        self
    }

    /// Fixed width for the inner card column (when callers compute an exact dialog width).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Custom top/right inset for the floating mini-bar (defaults to vertically centered in a 48 pt
    /// header, 12 pt from the right edge).
    pub fn bar_inset(mut self, top: f32, right: f32) -> Self {
        self.bar_top = top;
        self.bar_right = right;
        self
    }

    /// Keeps the mini-bar visible without hover (used when discard is armed or on conflict error).
    pub fn pin_actions(mut self, pin: bool) -> Self {
        self.pin_actions = pin;
        self
    }

    /// Appends an icon-only `mini_button` before the rightmost close/cancel (`×`) icon.
    pub fn action(mut self, btn: El) -> Self {
        self.leading.push(btn);
        self
    }

    /// Sets the close/cancel (`×`) `mini_button`, always placed as the rightmost icon in the bar.
    pub fn close(mut self, btn: El) -> Self {
        self.close = Some(btn);
        self
    }

    /// Adds an extra child inside the card column ahead of header/body/footer (e.g. a hidden focus anchor).
    pub fn anchor(mut self, el: El) -> Self {
        self.extra.push(el);
        self
    }

    /// Right padding a header row should reserve so its title/metadata never collides with a
    /// top-right mini-bar holding `total_buttons` icons (including `×`).
    pub fn actions_reserve(total_buttons: usize) -> f32 {
        mini_bar_w(total_buttons) + space::PX_8
    }

    /// Builds the complete `dialog_frame` mini-card from `header`, `body`, and `footer`.
    /// Both `header` and `footer` are automatically wired into the shared hover group so hovering
    /// anywhere in either band reveals the top-right `mini_bar`; `body` stays outside the group.
    pub fn build(self, header: El, body: El, footer: El) -> El {
        let zen_s = motion::zen_spring();
        let header = header.hover_group_shared(self.hover_group.clone()).hover_motion(zen_s, zen_s);
        let footer = footer.hover_group_shared(self.hover_group.clone()).hover_motion(zen_s, zen_s);

        let mut bar = mini_bar(self.p, &self.actions_key);
        for btn in self.leading {
            bar = bar.child(btn);
        }
        if let Some(close) = self.close {
            bar = bar.child(close);
        }
        let bar = bar.group_opacity(if self.pin_actions { 1.0 } else { 0.0 }, 1.0);

        let actions_zone = row()
            .key(self.actions_zone_key)
            .pin(None, Some(self.bar_top), Some(self.bar_right), None)
            .hover_group_shared(self.hover_group)
            .hover_motion(zen_s, zen_s)
            .child(bar);

        let mut inner = col().key(self.card_key);
        if let Some(w) = self.width {
            inner = inner.w(w);
        }
        for a in self.extra {
            inner = inner.child(a);
        }
        inner = inner.child(header).child(body).child(footer);

        dialog_frame(self.p, self.frame_key, None)
            .role("dialog")
            .label(self.label)
            .child(inner)
            .child(actions_zone)
    }
}
