//! Unified toggle switch (`SWITCH_W × SWITCH_H`, 50 × 28 pt, knob 24 pt).

use rune::{Color, El, Spring, Tx, el, row};

use super::Control;
use crate::app::App;
use crate::ui::V;
use crate::ui::tokens::{Elevation, FocusRing, motion, size};

/// How far an off switch's track moves toward the ink under the pointer.
const SWITCH_HOVER: f32 = 0.2;

/// A switch drawn 50 by 28, its knob 24 across. The pointer anywhere on it gives the track its
/// hover fill, which lands at once and fades over 100 ms; Tab rings the track.
pub fn switch(v: V, key: &str, on: bool, label_text: &str, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let p = v.p;
    let hovered = v.s.hover_switch.as_deref() == Some(key);
    let fill = match (on, hovered) {
        (true, false) => p.button,
        (true, true) => p.button_hover,
        (false, false) => p.mark,
        (false, true) => p.mark.mix(p.ink, SWITCH_HOVER),
    };
    let knob = Elevation::Knob.apply(
        el().key("knob")
            .size(size::KNOB, size::KNOB)
            .radius(size::KNOB / 2.0)
            .fill(Color::WHITE)
            .absolute(
                if on { size::SWITCH_W - size::KNOB - size::KNOB_INSET } else { size::KNOB_INSET },
                size::KNOB_INSET,
            ),
        p,
        1.0,
    );
    let track = FocusRing::Standard
        .apply(
            el().key("track")
                .size(size::SWITCH_W, size::SWITCH_H)
                .radius(size::SWITCH_H / 2.0)
                .fill(fill),
            p,
        )
        .child(knob);
    let own = key.to_string();
    let e = row()
        .key(key)
        .size(size::SWITCH_W, size::SWITCH_H)
        .center_y()
        .role("switch")
        .checked(on)
        .label(label_text)
        .on_hover_change(move |s: &mut App, inside, tx| {
            if inside {
                s.hover_switch = Some(own.clone());
                tx.instant();
            } else if s.hover_switch.as_deref() == Some(own.as_str()) {
                s.hover_switch = None;
                tx.color_motion(Spring::new(motion::HOVER_FADE, 0.0));
            }
        });
    e.control(run).child(track)
}
