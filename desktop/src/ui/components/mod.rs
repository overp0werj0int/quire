//! Unified desktop UI components (one module per component).
//!
//! Every component reads its geometry, radius, elevation, motion and interaction states from
//! `super::tokens` and `super::styles`, so changing a token or component updates every screen.

pub mod badge;
pub mod button;
pub mod choice;
pub mod chrome;
pub mod dialog;
pub mod icon_button;
pub mod input;
pub mod kbd;
pub mod mini_bar;
pub mod mini_card;
pub mod segmented;
pub mod selection_glass;
pub mod switch;

pub use badge::{count_pill, error_line, progress_dot, status_dot, tag_badge};
pub use button::{Button, ButtonVariant};
pub use choice::{checkbox_box, choice_card, choice_pill};
pub use chrome::{Control, DOTS_W, MARK, MARK_ON_INK, TIP_WRAP, bar_sep, dots, label, name_tip, padded};
pub use dialog::{SCROLL_FADE, bar_edge, dialog_frame, dialog_plane, faded, faded_h, popup_frame};
pub use icon_button::{IconButton, IconButtonSize, ToggleKind};
pub use input::{InputSize, area_frame, input_frame, literal_field, password_field, secret_display};
pub use kbd::{kbd, key_hint};
pub use mini_bar::{
    MINI_BAR_GAP, MINI_BAR_H, MINI_BAR_PX, MINI_BAR_RADIUS, MINI_BTN, MINI_BTN_RADIUS, MINI_DISABLED_ALPHA, MINI_ICON,
    mini_bar, mini_bar_at, mini_bar_w, mini_button, mini_button_at, toolbar_surface,
};
pub use mini_card::MiniCard;
pub use segmented::{PillItem, pill_group, seg, settings_seg, settings_seg_literal};
pub use selection_glass::{GLASS_R, GlassBackdrop, SelectionGlass};
pub use switch::switch;

/// Consistent focus-within ring for single-line text inputs (`SB-95`, `KBDA11Y`).
pub fn input_focus_ring(e: rune::El, p: &crate::theme::Pal) -> rune::El {
    super::tokens::FocusRing::Input.apply(e, p)
}

/// Scoped `SB-95` focus-within ring for compound input bars (`find-bar`, `search-box`).
pub fn input_focus_ring_when(e: rune::El, p: &crate::theme::Pal, active: bool) -> rune::El {
    super::tokens::FocusRing::Input.apply_when(e, p, active)
}
