//! The layout picker (MAP-089): a card under the title bar's layout button with one tile for
//! each way the map's branches can grow. Each tile's schematic is the real layout pass on a
//! small sample tree, so it cannot drift from what picking it does. The title bar's 16 pt
//! glyphs (`icons::LAYOUT_*`) are drawn by hand and can.

use rune::{Color, El, LineCap, Shape, col, el, icon, row, text};

use super::{brace, branch_color, tapered_on};
use crate::app::App;
use crate::keymap;
use crate::layer::tile_key;
use crate::map::{Layout, ROOT, Tree};
use crate::map_geom::line_width;
use crate::map_fold::Folds;
use crate::map_layout::{Metrics, layout};
use crate::roles::Roles;
use crate::ui::V;
use crate::ui::icons::CHECK;
use crate::ui::layer::Card;
use crate::ui::styles::sans;
use crate::ui::titlebar::BAR_H;
use crate::ui::components::{Control, kbd, popup_frame};
use crate::ui::tokens::{motion, radius};

const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
const TILE_W: f32 = 112.0;
const THUMB: [f32; 2] = [100.0, 66.0];
/// A tile's room round its schematic, and the card's from the window's edge.
const TILE_PAD: f32 = 5.0;
const EDGE: f32 = 8.0;
/// A sample's room inside its plate.
const INSET: f32 = 7.0;

/// The tree a one-sided tile draws: a root, three branches and two leaves on the first.
const SAMPLE: &str = "# R\n\n## A\n\n- a\n- b\n\n## B\n\n## C\n";
/// Both sides draws six branches, three a side, so its plate is no wider than the others' and
/// its drawing is not shrunk to fit.
const SAMPLE_BOTH: &str = "# R\n\n## A\n\n## B\n\n## C\n\n## D\n\n## E\n\n## F\n";

pub(in crate::ui) fn layouts(v: V) -> Card {
    let c = Roles::new(v.p);
    let now = v.s.map_tree().opts.layout;
    let across = ((v.vw - 2.0 * EDGE - 2.0 * PAD + GAP) / (TILE_W + GAP)).floor().clamp(1.0, 5.0) as usize;
    let per = if across >= 5 { 5 } else { across.min(3) };
    let mut grid = col().key("layout-tiles").gap(GAP).role("group").label("Layout").focus_group();
    for (row_idx, chunk) in Layout::ALL.chunks(per).enumerate() {
        let mut line = row().gap(GAP);
        for (col_idx, &l) in chunk.iter().enumerate() {
            let idx = row_idx * per + col_idx;
            line = line.child(tile(v, &c, l, idx, l == now));
        }
        grid = grid.child(line);
    }
    let w = per as f32 * TILE_W + (per - 1) as f32 * GAP + 2.0 * PAD;
    let x = (v.s.map.layout_button_right - w).max(EDGE);
    let esc = keymap::label_of("Esc");
    let header = row()
        .key("layouts-header")
        .h(crate::ui::tokens::size::CONTROL_BAR)
        .gap(crate::ui::tokens::space::GAP_CONTROL)
        .center_y()
        .child(text("Layout").font(sans(12.5, 600.0)).color(c.text))
        .child(el().grow(1.0))
        .child(kbd(v.p, esc, false));
    let foot = text("Saved in this note, under markmap").font(sans(11.0, 400.0)).color(v.p.mute);
    let card = popup_frame(col(), v.p)
        .key("layouts")
        .role("dialog")
        .label("Layout")
        .absolute(x, BAR_H + 6.0)
        .w(w)
        .padding(PAD)
        .gap(10.0)
        .radius(radius::FIELD)
        .child(header)
        .child(grid)
        .child(foot)
        .on_key(|s: &mut App, ev: &rune::KeyEvent, tx| {
            let rune::Key::Text(t) = &ev.key else { return false };
            let plain = ev.mods == rune::Mods::default();
            match t.parse::<usize>().ok().and_then(|n| Layout::ALL.get(n.wrapping_sub(1))) {
                Some(&l) if plain => {
                    s.map_set_layout(l, tx);
                    true
                }
                _ => false,
            }
        });
    Card { el: fade(card), plane: None }
}

/// The card comes in over 120 ms and goes at once; under reduced motion it just shows.
fn fade(card: El) -> El {
    if crate::motion::reduced() {
        return card;
    }
    let s = motion::modal_spring();
    card.transition(rune::Transition::opacity().springs(rune::SpringPair::new(s, s)))
}

fn tile(v: V, c: &Roles, l: Layout, idx: usize, current: bool) -> El {
    let (rw, rg, rc) = crate::ui::focus_ring(v.p);
    let plate = el().w(THUMB[0]).h(THUMB[1]).radius(radius::CONTROL).clip_rounded(radius::CONTROL).fill(c.sunken).child(schematic(v.p, l));
    let digit = idx + 1;
    let key_badge = row()
        .key(format!("layout-key-{digit}"))
        .min_w(15.0)
        .h(15.0)
        .px(3.0)
        .center()
        .radius(radius::INNER)
        .fill(c.raised)
        .border(1.0, c.hairline)
        .border_layout()
        .child(text(digit.to_string()).font(crate::ui::styles::keys(10.0).lh(10.0)).color(Roles::panel(v.p).dim));
    let mut name = row()
        .gap(5.0)
        .center_y()
        .child(key_badge)
        .child(text(l.name()).font(sans(12.0, if current { 600.0 } else { 400.0 })).color(if current { c.text } else { v.p.mute }));
    if current {
        name = name.child(icon(&CHECK, 10.0).color(c.accent_text));
    }
    col()
        .key(tile_key(l))
        .w(TILE_W)
        .padding(TILE_PAD)
        .gap(6.0)
        .items_center()
        .radius(radius::FIELD)
        .border(1.5, if current { c.accent } else { Color::TRANSPARENT })
        .border_layout()
        .hover(v.p.hover)
        .hover_group()
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .checked(current)
        .label(l.name())
        .help(l.name())
        .focus_ring(rw, rg, rc)
        .control(move |s: &mut App, tx| s.map_set_layout(l, tx))
        .focus_selected(current)
        .child(plate)
        .child(name)
}

/// The sample under layout `l`, laid out by the map's own pass with sizes of a label's bar, and
/// drawn scaled into a plate: branches through the map's curve code in the branch colours,
/// labels as bars, the root's the strongest.
fn schematic(p: &'static crate::theme::Pal, l: Layout) -> El {
    // A palette is made once and kept (`theme::themed`), so its address stands for its colours.
    let key = (l, std::ptr::from_ref(p) as usize);
    rune::memo(format!("layout-thumb:{}", l.word()), key, move || {
        let c = Roles::new(p);
        let sample = if l == Layout::Both { SAMPLE_BOTH } else { SAMPLE };
        let src = format!("---\nmarkmap:\n  layout: {}\n---\n{sample}", l.word());
        let tree: Tree = crate::map::build(&crate::blocks::build(&src), &src, "");
        // A bar sits on its underline, and clear of its bracket.
        let pad_b = if matches!(l, Layout::Down | Layout::Up) { 3.0 } else { 0.0 };
        let m = Metrics { sh: 26.0, sv: 7.0, pad_x: 0.0, pad_b, min_h: 0.0, max_w: 1000.0 };
        let size = |i: usize| match tree.nodes[i].depth {
            0 => [44.0, 13.0],
            1 => [30.0, 9.0],
            _ => [22.0, 6.0],
        };
        let laid = layout(&tree, &Folds::default(), m, size, |_| [0.0, 0.0]);
        let [x0, y0, x1, y1] = laid.bounds;
        let [pw, ph] = THUMB;
        let k = ((pw - 2.0 * INSET) / (x1 - x0)).min((ph - 2.0 * INSET) / (y1 - y0));
        let at = |[x, y]: [f32; 2]| [pw / 2.0 + (x - (x0 + x1) / 2.0) * k, ph / 2.0 + (y - (y0 + y1) / 2.0) * k];
        let mut shape = Shape::new();
        for &i in &laid.shown {
            let color = branch_color(&c, tree.nodes[i].branch);
            if let Some(b) = laid.ringless_branch(&tree, i) {
                shape = tapered_on(shape, b.curve().map(at), (b.lw * k * 1.6).max(1.0), color);
            }
        }
        for &i in &laid.shown {
            let Some(p) = laid.at[i] else { continue };
            let bar = if i == ROOT { c.strong } else { c.dim };
            let h = p.h - pad_b;
            let t = (h * k).max(2.0);
            let [bx0, by] = at([p.x, p.y + p.content(pad_b).0 + h / 2.0]);
            let [bx1, _] = at([p.right(), p.y]);
            shape = shape.line([bx0 + t / 2.0, by], [bx1 - t / 2.0, by], t, LineCap::Round, bar);
            let color = branch_color(&c, tree.nodes[i].branch);
            let lw = (line_width(p.depth) * k * 1.6).max(1.0);
            if p.braced() {
                let lw = (lw * 0.75).max(1.0);
                shape = brace(shape, at([p.x, p.y + p.line_top()]), [p.w * k, p.line_room() * k], p.overlined(), lw, color);
            } else if i != ROOT {
                shape = shape.line(at([p.x, p.line_y()]), at([p.right(), p.line_y()]), lw, LineCap::Round, color);
            }
        }
        rune::canvas(shape).size(pw, ph).inert()
    })
}
