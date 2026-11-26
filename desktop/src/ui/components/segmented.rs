//! Unified segmented control and compact pill-group selector (`SZ-06`, `LK-01`, `LK-14`).

use rune::{Color, El, Key, KeyEvent, Mods, TextStyle, Tx, row, text};

use super::{Control, label};
use crate::app::App;
use crate::roles::Roles;
use crate::theme::Pal;
use crate::ui::V;
use crate::ui::styles::{self, literal, sans};
use crate::ui::tokens::{Elevation, FocusRing, radius, size, space};

/// One button of a segmented control or pill group.
pub struct Seg {
    pub label: String,
    pub on: bool,
    pub run: Box<dyn Fn(&mut App, &mut Tx)>,
}

pub fn seg(label: impl Into<String>, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> Seg {
    Seg { label: label.into(), on, run: Box::new(run) }
}

/// Raised segmented control on a tinted track (`role="radiogroup"`).
pub fn segmented(p: &Pal, key: &str, segs: Vec<Seg>, h: f32, pad_x: f32, style: TextStyle, track: Color) -> El {
    let mut r = row()
        .key(key)
        .gap(space::PAD_TRACK)
        .padding(space::PAD_TRACK)
        .radius(radius::FIELD)
        .fill(track)
        .role("radiogroup")
        .focus_group();
    for (i, sg) in segs.into_iter().enumerate() {
        let on = sg.on;
        let run = sg.run;
        let mut b = row()
            .key(format!("{key}-{i}"))
            .h(h)
            .px(pad_x)
            .center()
            .shrink()
            .radius(radius::CONTROL)
            .fill(if on { p.bg } else { Color::TRANSPARENT })
            .label(sg.label.clone())
            .child(label(sg.label, style, if on { p.ink } else { p.mute }).truncate());
        if on {
            b = Elevation::Segment.apply(b, p, 1.0);
        }
        r = r.child(b.choice(on, move |s, tx| run(s, tx)));
    }
    r
}

/// Settings segmented control (28 pt tall, 12.5 pt Instrument Sans, Decision 7A).
pub fn settings_seg(v: V, key: &str, segs: Vec<Seg>) -> El {
    segmented(v.p, key, segs, size::CONTROL_PANEL, settings_seg_pad(v), styles::control(), v.p.side)
}

/// Settings segmented control for literal names (such as MCP transport).
pub fn settings_seg_literal(v: V, key: &str, segs: Vec<Seg>) -> El {
    segmented(v.p, key, segs, size::CONTROL_PANEL, settings_seg_pad(v), literal(12.0), v.p.side)
}

fn settings_seg_pad(v: V) -> f32 {
    if crate::ui::settings::stacked(v) { space::PX_8 } else { space::PX_12 }
}

/// Compact pill-group selector (24 pt tall pills inside a hairline track, radius 6 inside 8,
/// `SZ-06`), used for Help mode tabs.
pub struct PillItem {
    pub key: &'static str,
    pub label: &'static str,
    pub aria_label: String,
    pub on: bool,
    pub run: Box<dyn Fn(&mut App, &mut Tx)>,
    /// Optional follow-up action run after `run` when the user presses Enter on the pill
    /// (for example, dropping focus from the mode selector into the search field).
    pub on_enter: Option<Box<dyn Fn(&mut App, &mut Tx)>>,
}

pub fn pill_group(p: &Pal, key: &str, aria_label: &str, items: Vec<PillItem>) -> El {
    let muted = Roles::readable_on(p.mute, p.bg, 5.2);
    let mut group = row()
        .key(key)
        .gap(space::PAD_TRACK)
        .padding(space::PAD_TRACK)
        .radius(radius::FIELD)
        .border(1.0, p.line)
        .border_layout()
        .role("radiogroup")
        .focus_group()
        .label(aria_label);
    for it in items {
        let on = it.on;
        let run = std::rc::Rc::new(it.run);
        let run_enter = run.clone();
        let on_enter = it.on_enter;
        let lbl = text(it.label).font(sans(styles::scale::SMALL, if on { 500.0 } else { 400.0 }));
        let lbl = if on { lbl.color(p.ink) } else { lbl.color(muted).group_color(p.ink) };
        let pill = FocusRing::Standard
            .apply(
                row()
                    .key(it.key)
                    .h(size::CONTROL_BAR)
                    .px(space::PX_8)
                    .center()
                    .radius(radius::CONTROL)
                    .fill(if on { p.hover } else { Color::TRANSPARENT })
                    .hover(p.hover)
                    .hover_group()
                    .border(1.0, if on { p.line } else { Color::TRANSPARENT })
                    .border_layout()
                    .label(it.aria_label),
                p,
            )
            .role("radio")
            .checked(on)
            .focusable(true)
            .focus_selected(on)
            .on_press_tx(move |s: &mut App, tx| {
                s.touch_activity();
                run(s, tx);
            })
            .on_key(move |s: &mut App, ev: &KeyEvent, tx| {
                let enter = ev.key == Key::Enter && ev.mods == Mods::default();
                if enter {
                    s.touch_activity();
                    run_enter(s, tx);
                    if let Some(ref f) = on_enter {
                        f(s, tx);
                    }
                }
                enter
            })
            .child(lbl);
        group = group.child(pill);
    }
    group
}
