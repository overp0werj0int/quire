//! The map canvas (MAP-032 to MAP-045, MAP-055, MAP-075): the note's tree on a canvas that
//! pans and zooms without bounds. One world element carries the camera as live values; every
//! node, link and fold circle sits in it at its world place. Only what meets the canvas and
//! its overscan is built.

mod card;
mod layouts;
mod lightbox;

use std::hash::{Hash, Hasher};

use rune::{Color, Drag, El, LineCap, MagnifyEvent, Path, PointerButton, Shape, TextHighlight, TextStyle, Tx, WheelEvent, el, text};

use super::preview::{Cx, Hl, Look, Rich, Tone, look_at};
pub(super) use layouts::layouts;
pub(super) use lightbox::lightbox;
use super::{V, styles};
use crate::app::App;
use crate::blocks::Run;
use crate::map::{Body, Node, NodeKind, ROOT, Tree};
use crate::map_app::{self, Tier};
use crate::map_geom::{Branch, FOLD_DOT_R, FOLD_HIT_BACK, Place, line_width};
use crate::map_layout::{DropMark, Laid};
use crate::page;
use crate::roles::Roles;
use crate::theme::normal_lh;

/// Half the world element's side. The world is centred on the canvas's top-left, so its live
/// offset is the camera's pan and nodes sit at their world coordinates plus this.
const W: f32 = 131_072.0;
/// The drag ghost's offset from the pointer and width cap, in screen points.
const GHOST_DX: f32 = 16.0;
const GHOST_DY: f32 = 18.0;
const GHOST_MAX_W: f32 = 260.0;
const GHOST_EDGE: f32 = 8.0;

/// A node's key: its first source line, 1-based, or the root or footnotes group by name.
pub fn node_key(t: &Tree, i: usize) -> String {
    match t.nodes[i].kind {
        NodeKind::Root => "map-node:root".into(),
        NodeKind::Footnotes => "map-node:footnotes".into(),
        _ => format!("map-node:{}", t.nodes[i].own.start + 1),
    }
}

fn key_tail(key: &str) -> &str {
    key.strip_prefix("map-node:").unwrap_or(key)
}

/// The contrast the map's lines and marks keep against the page: 3:1 with room for the
/// rounding to 8-bit colour (MAP-074).
const GRAPHIC: f32 = 3.1;

/// A branch's colour on the page: the accent at the root, else the palette (MAP-040).
fn branch_color(c: &Roles, branch: usize) -> Color {
    if branch == 0 {
        return Roles::readable_on(c.accent, c.bg, GRAPHIC);
    }
    let pal = [c.blue, c.green, c.orange, c.magenta, c.cyan, c.yellow];
    Roles::readable_on(pal[(branch - 1) % pal.len()], c.bg, GRAPHIC)
}

/// The look the map's labels and cards draw in: the page's, at a card's widest (MAP-038), with
/// dim and faint lifted to read on the canvas, cards and placeholders across all themes (MAP-074).
fn map_look(v: V) -> Look {
    let mut look = look_at(v, page::r(v.s.settings().size as f32 * 30.0));
    look.c.dim = look.c.readable_dim();
    let bg_card = look.c.on_page(look.c.card);
    let panel = look.c.on_page(look.c.panel);
    let tile_card = crate::roles::mix(crate::roles::mix(panel, look.c.text, look.c.raised.a), look.c.text, look.c.card.a);
    let mut faint = look.c.faint;
    for surf in [look.c.bg, bg_card, panel, tile_card] {
        faint = Roles::readable_on(faint, surf, 4.8);
    }
    look.c.faint = faint;
    look
}

/// Node `n`'s text as the reader sets it, in `st` (MAP-038): its inline runs after an ordered
/// item's number, or its label as it is.
fn label(cx: &Cx, n: &Node, st: TextStyle, color: Color) -> Rich {
    let plain = |t: String| Run { t, ..Run::default() };
    let runs = match (&n.body, &n.kind) {
        (Body::Runs(r), NodeKind::Item { marker, ordered: true, .. }) => std::iter::once(plain(format!("{marker} "))).chain(r.iter().cloned()).collect(),
        (Body::Runs(r), _) => r.clone(),
        _ => vec![plain(map_app::shown_label(n))],
    };
    cx.rich(&runs, Tone::top(color, st.size, 1.45))
}

/// Sizes nodes as `node` draws them, for the layout (`App::map_laid`).
pub(crate) struct Sizer<'a> {
    v: V<'a>,
    look: Look,
}

impl<'a> Sizer<'a> {
    pub(crate) fn new(s: &'a App) -> Sizer<'a> {
        let v = V::bare(s);
        Sizer { v, look: map_look(v) }
    }

    /// Node `i`'s content size: its card's, or its label wrapped at `max_w`, at least a line
    /// tall.
    pub(crate) fn size(&self, t: &Tree, i: usize, max_w: f32) -> [f32; 2] {
        let cx = Cx::map(self.v, &self.look, None);
        if let Body::Card(bl) = &t.nodes[i].body
            && let Some((_, size)) = card::card(&cx, bl, i, "", None)
        {
            return size;
        }
        let tier = map_app::tier(t, i);
        let st = map_app::style(tier, self.look.b, self.look.family, self.look.mono);
        let lh = page::line(st.size, if matches!(tier, Tier::Root | Tier::Head1) { 1.3 } else { 1.45 });
        let rich = label(&cx, &t.nodes[i], st, self.look.c.text);
        if rich.text.is_empty() {
            return [0.0, lh];
        }
        let anywhere = rune::text::RichLayoutOptions::new().wrap_anywhere();
        let (w, h) = rune::text::measure_with(&rich.text, &rich.spans, st, Some(max_w), anywhere);
        [w.min(max_w).ceil(), h.max(lh).ceil()]
    }
}

/// What a screen reader hears for node `i`, with `hidden` nodes folded away under it (MAP-075).
fn spoken(t: &Tree, i: usize, folded: bool, hidden: usize) -> String {
    let n = &t.nodes[i];
    let shown = map_app::shown_label(n);
    let name = if n.label.is_empty() || n.label == "\u{200b}" || shown == "\u{200b}" { "New node" } else { shown.as_str() };
    let mut s = format!("{name}, level {}", n.depth + 1);
    if !n.children.is_empty() {
        s += if folded { ", collapsed" } else { ", expanded" };
        if hidden > 0 {
            s += &format!(", {hidden} hidden");
        }
    }
    match n.kind {
        NodeKind::Item { task: crate::blocks::Task::Done, .. } => s += ", checked",
        NodeKind::Item { task: crate::blocks::Task::Todo, .. } => s += ", not checked",
        _ => {}
    }
    s
}

pub(super) fn canvas(v: V) -> El {
    let s = v.s;
    let look = map_look(v);
    let cx = Cx::map(v, &look, None);
    let c = &look.c;
    let t = s.map_tree();
    let drawn = s.map_drawn();
    let l = drawn.laid.clone();
    let cam = s.map_camera();
    let (nodes, links) = s.map_mounted();
    let sel = s.map_selected();
    let view = s.map_view();
    let rings = |i: usize| view.filter(|_| !t.nodes[i].children.is_empty()).map_or(Vec::new(), |v| l.rings(&t, i, &v.folds));
    let (b, prose, mono) = (look.b, look.family, look.mono);
    // A fold circle's or task box's target is at least 24 points on screen (MAP-041, MAP-073).
    // Pin detail level during active camera glides (`map_detail_zoom`) so zooming across
    // `LITE_ZOOM` or `hit_bucket` boundaries never mounts/rebuilds dozens of cards mid-glide.
    let dz = s.map_detail_zoom();
    let bucket_zoom = 2f32.powi(map_app::hit_bucket(dz));
    let target = 24.0 / bucket_zoom;
    let hit = target.max(2.0 * FOLD_HIT_BACK);
    // Below 0.65× zoom, card hover chips switch to compact square icon-only buttons sized for
    // ~20 pt on screen so they stay usable at Fit zoom without bloating at 0.8×.
    let chip_compact = map_app::chip_compact_side(dz);
    let lite = dz < map_app::LITE_ZOOM;
    let no_hover = dz < map_app::NO_HOVER_ZOOM;
    let plain_label = dz < map_app::PLAIN_LABEL_ZOOM;
    let mut world = el()
        .key("map-world")
        .pin(Some(-W), Some(-W), None, None)
        .size(2.0 * W, 2.0 * W)
        .transition(if s.mode_swapped { super::swap() } else { rune::Transition::none() })
        .child_exit(rune::Transition::none());
    if let Some(v) = view {
        world = world.live_offset(&v.live_pan).live_scale(&v.live_zoom);
    }
    if s.map_gliding() {
        world = world.instant_changes();
    }
    // The subtree a drag carries dims where it was (MAP-026).
    let drag = s.map.node_drag.as_ref().filter(|g| g.active);
    let moved = drag.and_then(|g| t.find(g.node)).map_or(0..0, |m| m..t.nodes[m].end);
    let dim = |i: usize| drawn.alpha(i) * if moved.contains(&i) { 0.35 } else { 1.0 };
    let editing = s.map_editing();
    // The field over the node being edited: its place, room left of the text, style and size.
    let field_at = editing.and_then(|i| {
        let (e, p) = (s.map.inline.as_ref()?, l.at[i]?);
        let room = map_app::task_room(&t.nodes[i], b);
        let st = map_app::style(map_app::tier(&t, i), b, prose, mono);
        Some((i, p, room, st, map_app::field_size(&e.edit.text, p.w - 2.0 * l.metrics.pad_x - room, st, l.metrics.max_w)))
    });
    // The selected node sits under a glass card (MAP-055). Only its content grows under the glass
    // (`node`); its underline, links and fold ring stay put, so no line moves or breaks there.
    let glass = nodes.contains(&sel).then(|| l.at[sel]).flatten().map(|g| {
        let lw = g.line_room();
        let [w, h] = match field_at {
            Some((i, _, room, _, [fw, fh])) if i == sel => [g.w.max(fw + room + 2.0 * l.metrics.pad_x), g.h.max(fh + l.metrics.pad_b)],
            _ => [g.w, g.h],
        };
        let [px, py] = crate::map_layout::GLASS_PAD;
        let r = [g.x - px, g.y - py, w + 2.0 * px, h + lw + 2.0 * py];
        (r, GLASS_R.min(r[3] / 2.0))
    });
    if let Some((r, rad)) = glass {
        world = world.child(glass_base(c, r, rad));
    }
    // The add toolbar over a new node's glass (MAP-087), at Source's size on screen.
    let bar = s.map_new_node().zip(glass).map(|(new, ([x, y, _, _], _))| {
        let k = 1.0 / s.map_bar_zoom();
        let h = map_app::KIND_BAR_H * k;
        (new, [x, y - map_app::KIND_BAR_GAP * k - h, KIND_BAR_W * k, h], k)
    });
    if plain_label {
        let mut static_links = Vec::new();
        for &i in &links {
            let a = dim(i);
            if a < 0.999 {
                world = world.child(link(&t, &l, i, c, a, true));
            } else {
                static_links.push(i);
            }
        }
        if let Some(batch) = links_batch(&t, &l, &static_links, c) {
            world = world.child(batch);
        }
    } else {
        for &i in &links {
            world = world.child(link(&t, &l, i, c, dim(i), false));
        }
    }
    let query = s.find_query();
    let cur = query.and_then(|_| s.map_hits().get(s.reader.find.cur).copied());
    for &i in &nodes {
        let found = query.map(|q| Found { q, cur: cur.filter(|h| h.node == i).map(|h| (h.pid, h.k)) });
        // Hover lights every element under the pointer, so a node under the add toolbar shows none.
        let covered = bar.and_then(|(_, r, _)| Some((r, l.at[i]?))).is_some_and(|([x, y, w, h], p)| {
            let [x0, y0, x1, y1] = p.rect();
            x0 < x + w && x < x1 && y0 < y + h && y < y1
        });
        let rings = rings(i);
        let folded = !rings.is_empty() && rings.iter().all(|r| r.folded);
        let hidden = rings.iter().filter(|r| r.folded).map(|r| r.side.nodes.len()).sum();
        let st = State { folded, hidden, selected: i == sel, editing: editing == Some(i), alpha: dim(i), lite, no_hover: no_hover || covered, plain_label, target, chip_compact, found };
        world = world.child(node(&cx, &t, &l, i, st));
    }
    // The glass's top-lit rim, over the node. Paint it BEFORE fold targets so hover-revealed fold
    // controls sit above it. The rim stays at least a pixel wide on screen when zoomed out.
    if let Some(([x, y, w, h], rad)) = glass {
        let lit = if s.map.canvas_focused { 1.0 } else { 0.6 };
        let rim = (1.0 / cam.1.max(map_app::ZOOM_MIN)).max(1.0);
        world = world.child(
            el().key("map-selection")
                .pin(Some(x + W), Some(y + W), None, None)
                .size(w, h)
                .radius(rad)
                .gradient(c.text.alpha(0.07 * lit), c.text.alpha(0.0))
                .border_gradient(rim, c.text.alpha(0.55 * lit), c.text.alpha(0.12 * lit))
                .box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 10.0, spread: 0.0, color: c.text.alpha(0.08 * lit), inset: true })
                .inert(),
        );
    }
    for &i in &nodes {
        let of = RingOf { t: &t, l: &l, i, editing: editing == Some(i), lite, hit, c, font: map_app::chip_font(b, mono) };
        for (nth, r) in rings(i).iter().enumerate() {
            world = world.children(fold(&of, r, nth).map(|e| e.opacity(dim(i))));
        }
    }
    if let Some((_, p, room, st, [w, _])) = field_at {
        world = world.child(field(v, p, room, st, w, &l.metrics));
    }
    if let Some((new, [x, y, _, _], k)) = bar {
        world = world.child(kind_bar(v, new, [x, y], k));
    }
    if let Some(g) = drag
        && let Some(at) = g.drop
    {
        world = world.child(drop_mark(&t, &l, at, c, cam.1));
    }
    let ghost_el = drag.and_then(|g| {
        let m = t.find(g.node)?;
        let child_bottom = match g.drop {
            Some(crate::map::Add::Child(j)) => l.child_mark_bottom(j).map(|y| cam.0[1] + y * cam.1),
            _ => None,
        };
        Some(ghost(&cx, &t, m, g.at, s.map_size(), child_bottom))
    });
    if nodes.contains(&ROOT)
        && let Some(p) = l.at[ROOT]
    {
        let font = map_app::chip_font(b, mono);
        let badge = (t.props > 0).then(|| map_app::badge(t.props, &l, font)).flatten();
        let badge_under = badge.is_some() && l.badge_under();
        if let Some((badge, [x, y, _, h])) = badge {
            world = world.child(
                rune::row()
                    .key("map-props")
                    .h(h)
                    .px(map_app::BADGE_PAD)
                    .center_y()
                    .radius(4.0)
                    .hover(c.hover)
                    .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
                    .role("button")
                    .label(badge.clone())
                    .cursor(rune::Cursor::Pointer)
                    .on_press_tx(|s: &mut App, tx: &mut Tx| s.map_props(tx))
                    .pin(Some(x + W), Some(y + W), None, None)
                    .child(text(badge).font(font).color(c.dim)),
            );
        }
        if t.nodes.len() == 1 {
            let y = p.bottom() + 6.0 + if badge_under { 28.0 } else { 0.0 };
            let hint_text = "Press Tab to add a branch, or F2 to name the root";
            let hint_font = styles::sans((b * 0.86).round(), 400.0);
            let hint_w = rune::text::measure(hint_text, &[], hint_font, None).0.ceil();
            let hint_x = p.x + p.w / 2.0 - hint_w / 2.0;
            world = world.child(
                text(hint_text)
                    .key("map-hint")
                    .font(hint_font)
                    .color(c.dim)
                    .pin(Some(hint_x + W), Some(y + W), None, None),
            );
        }
    }
    // The sheet rings itself while files hover over it (RD-97); the canvas would cover that.
    let ring = s.drop_over.then(|| {
        el().key("map-drop")
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .inert()
            .box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: v.p.acc, inset: true })
    });
    el().key("map-canvas")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .fill(c.bg)
        .clip_soft(0.0)
        .transition(rune::Transition::none())
        .role("tree")
        .label("Map")
        .child(world)
        .child(ring.unwrap_or_else(el))
        .children(ghost_el)
        .on_layout_tx(|s: &mut App, r, tx| s.map_laid_out(r, tx))
        .on_wheel(|s: &mut App, e: WheelEvent, tx: &mut Tx| {
            // The window sends Ctrl + wheel as a pinch (`on_magnify`); a wheel that still holds
            // Ctrl zooms the same way.
            if e.mods.cmd {
                s.map_zoom_about(e.x, e.y, 1.0 - e.dy / 200.0, tx);
                return true;
            }
            s.map_wheel(&e, tx);
            true
        })
        .on_magnify(|s: &mut App, e: MagnifyEvent, tx: &mut Tx| {
            s.map_zoom_about(e.x, e.y, 1.0 + e.delta, tx);
            true
        })
        .pointer_buttons([PointerButton::Middle])
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_pan_drag(d, tx))
}

/// Batches all opaque links at extreme overview zoom (`< PLAIN_LABEL_ZOOM`) into one vector
/// shape element, slashing hundreds of scene nodes and edges during 10% Fit fold/unfold frames.
fn links_batch(t: &Tree, l: &Laid, links: &[usize], c: &Roles) -> Option<El> {
    let segs: Vec<(Branch, Color)> = links.iter().filter_map(|&i| Some((l.branch(t, i)?, branch_color(c, t.nodes[i].branch)))).collect();
    if segs.is_empty() {
        return None;
    }
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (b, _) in &segs {
        bx0 = bx0.min(b.from[0].min(b.to[0]) - b.lw);
        by0 = by0.min(b.from[1].min(b.to[1]) - b.lw);
        bx1 = bx1.max(b.from[0].max(b.to[0]) + b.lw);
        by1 = by1.max(b.from[1].max(b.to[1]) + b.lw);
    }
    let w = (bx1 - bx0).max(1.0);
    let h = (by1 - by0).max(1.0);
    let rel = move |[x, y]: [f32; 2]| [x - bx0, y - by0];
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (w.to_bits(), h.to_bits(), segs.len()).hash(&mut hash);
    for (b, color) in &segs {
        (b.dir, rel(b.from).map(f32::to_bits), rel(b.to).map(f32::to_bits), b.lw.to_bits(), color_bits(*color)).hash(&mut hash);
    }
    let shape_el = rune::memo("map-links-batch".to_string(), hash.finish(), move || {
        let mut shape = Shape::new();
        for (b, color) in segs {
            let [a, c1, c2, z] = b.curve().map(rel);
            let path = if b.flat() { Path::new().move_to(a).line_to(z) } else { Path::new().move_to(a).cubic_to(c1, c2, z) };
            shape = shape.stroke_path(&path, b.lw, LineCap::Round, color);
        }
        rune::canvas(shape).size(w, h).inert()
    });
    Some(shape_el.pin(Some(bx0 + W), Some(by0 + W), None, None))
}

/// The branch into node `i` and, unless `plain` (where the node draws its own), the node's
/// underline as its last stroke, so the curve runs on into the line with nothing to meet.
fn link(t: &Tree, l: &Laid, i: usize, c: &Roles, alpha: f32, plain: bool) -> El {
    let Some(b) = l.branch(t, i) else { return el() };
    let end = if plain { None } else { b.end };
    let [x0, y0, x1, y1] = b.bbox(end);
    let (w, h) = (x1 - x0, y1 - y0);
    let tail = key_tail(&node_key(t, i)).to_string();
    let color = branch_color(c, t.nodes[i].branch);
    // Memoize the stroked Bézier shape by its geometry from its own corner, so rigid subtree
    // translations during fold/unfold relayout glides never re-tessellate unchanged links.
    let shape = rune::memo(format!("map-link-s:{tail}"), b.hash(end, color_bits(color)), move || {
        let rel = |[x, y]: [f32; 2]| [x - x0, y - y0];
        let mut shape = tapered_on(Shape::new(), b.curve().map(rel), b.lw, color);
        if let Some(e) = end {
            shape = shape.line(rel(b.to), rel(e), b.lw, LineCap::Butt, color);
        }
        rune::canvas(shape).size(w, h).inert()
    });
    el().key(format!("map-link:{tail}"))
        .pin(Some(x0 + W), Some(y0 + W), None, None)
        .size(w, h)
        .opacity(alpha)
        .inert()
        .child(shape)
}

/// The selection glass's corner radius (MAP-055); its padding is `map_layout::GLASS_PAD`.
const GLASS_R: f32 = 10.0;
/// How much the selected node's content grows under the glass, and at most how far each side
/// moves out, so a tall table or wide card still sits inside the glass's padding.
const GLASS_K: f32 = 1.035;
const GLASS_GROW: f32 = 4.0;

/// The scale the selected node's content takes under the glass, for content `w` by `h`.
fn glass_k(w: f32, h: f32) -> f32 {
    GLASS_K.min(1.0 + 2.0 * GLASS_GROW / w.max(h).max(1.0))
}

/// The glass's body, under the links and nodes: a faint sheen and the card's shadow.
fn glass_base(c: &Roles, [x, y, w, h]: [f32; 4], rad: f32) -> El {
    el().key("map-glass")
        .pin(Some(x + W), Some(y + W), None, None)
        .size(w, h)
        .radius(rad)
        .fill(crate::roles::mix(c.bg, c.text, 0.035))
        .box_shadow(rune::Shadow { x: 0.0, y: 6.0, blur: 22.0, spread: 0.0, color: Color::BLACK.alpha(0.35), inset: false })
        .inert()
}

/// How far one piece of a branch turns at most: 2 degrees, so a bend stays round at 400%, each
/// chord straying from the curve by under a 400th of its length.
const BRANCH_TURN: f32 = 2.0 * std::f32::consts::PI / 180.0;

/// After what `shape` already holds, a branch along the cubic `[a, c1, c2, z]`, from its
/// parent's fold ring to its node: 1 point
/// wide at the ring and widening to `lw` by halfway, so the branches fanning out of one ring
/// don't merge into a blot. Drawn as strokes, shorter where it bends (`BRANCH_TURN`), each
/// widening from the curve's width where it starts to its width where it ends (`Shape::taper`)
/// and starting where the last ended, so Rune joins them into one stroke with smooth edges: a
/// filled outline cut into thin triangles is stepped along its edges.
fn tapered_on(shape: Shape, [a, c1, c2, z]: [[f32; 2]; 4], lw: f32, color: Color) -> Shape {
    let at = |t: f32| {
        let u = 1.0 - t;
        let f = |i: usize| u * u * u * a[i] + 3.0 * u * u * t * c1[i] + 3.0 * u * t * t * c2[i] + t * t * t * z[i];
        [f(0), f(1)]
    };
    let tangent = |t: f32| {
        let u = 1.0 - t;
        let f = |i: usize| 3.0 * u * u * (c1[i] - a[i]) + 6.0 * u * t * (c2[i] - c1[i]) + 3.0 * t * t * (z[i] - c2[i]);
        [f(0), f(1)]
    };
    // The angle between two tangents, which a difference of headings gets wrong where they
    // wrap, on a curve running left.
    let turn = |t0: f32, t1: f32| {
        let ([x0, y0], [x1, y1]) = (tangent(t0), tangent(t1));
        (x0 * y1 - y0 * x1).atan2(x0 * x1 + y0 * y1).abs()
    };
    let width = |t: f32| {
        let e = (t / 0.5).min(1.0);
        1.0 + (lw - 1.0).max(0.0) * e * e * (3.0 - 2.0 * e)
    };
    fn split(t0: f32, t1: f32, depth: u32, turn: &dyn Fn(f32, f32) -> f32, out: &mut Vec<f32>) {
        if depth < 6 && turn(t0, t1) > BRANCH_TURN {
            let m = (t0 + t1) / 2.0;
            split(t0, m, depth + 1, turn, out);
            split(m, t1, depth + 1, turn, out);
        } else {
            out.push(t1);
        }
    }
    let mut ts = vec![0.0];
    for k in 0..16 {
        split(k as f32 / 16.0, (k + 1) as f32 / 16.0, 0, &turn, &mut ts);
    }
    ts.windows(2).fold(shape, |shape, t| shape.taper([at(t[0]), at(t[1])], width(t[0]), width(t[1]), LineCap::Round, color))
}

/// A wide curly bracket filling the box `[w, h]` at `[x, y]`: its point in the middle of the
/// top, or of the bottom, and its two ends at the other edge.
fn brace(shape: Shape, [x, y]: [f32; 2], [w, h]: [f32; 2], up: bool, lw: f32, color: Color) -> Shape {
    let (mid, cx, edge) = (h / 2.0, w / 2.0, lw / 2.0);
    let turn = 14f32.min(w / 4.0);
    let half = |s: f32| {
        let at = |px: f32, py: f32| [x + cx + s * (cx - px), y + if up { py } else { h - py }];
        Path::new()
            .move_to(at(edge, h - edge))
            .cubic_to(at(edge, mid), at(turn * 0.4, mid), at(turn, mid))
            .line_to(at(cx - turn, mid))
            .cubic_to(at(cx - turn * 0.4, mid), at(cx, mid), at(cx, 0.0))
    };
    [-1.0, 1.0].into_iter().fold(shape, |shape, s| shape.stroke_path(&half(s), lw, LineCap::Round, color))
}

/// The inline field over the node being edited, in the label's place and style (MAP-016).
fn field(v: V, p: Place, room: f32, st: rune::TextStyle, w: f32, m: &crate::map_layout::Metrics) -> El {
    let Some(e) = &v.s.map.inline else { return el() };
    let c = Roles::new(v.p);
    let surface = c.on_page(c.raised);
    rune::text_input(&e.edit, st)
        .key(map_app::FIELD)
        .label("Node text")
        .color(v.p.ink)
        .caret(v.p.ink, v.p.sel)
        .on_edit(|s: &mut App| match &mut s.map.inline {
            Some(e) => &mut e.edit,
            None => &mut s.map.spare,
        })
        .on_input_key(|s: &mut App, ev: &rune::KeyEvent, tx: &mut Tx| s.map_field_key(ev, tx))
        .pin(Some(p.x + p.label_pads(m.pad_x, room)[0] + W), Some(p.y + p.content(m.pad_b).0 + W), None, None)
        .w(w)
        .radius(3.0)
        .fill(surface)
        .wrap_anywhere()
}

/// The add toolbar over a new node's glass, its top-left corner at `at` (MAP-087): the types the
/// node can be written as, as Source's formatting toolbar draws them, its own marked and those
/// its place can't take greyed. Its sizes are points on screen times `k`, one over the map's
/// zoom, so it keeps Source's size at every zoom. A press keeps the keyboard in the field, which
/// neither commits nor closes.
/// The add toolbar's width: 5 buttons of 28, a separator of 11, 5 gaps of 2 and 4 of padding
/// at each end.
const KIND_BAR_W: f32 = 169.0;

fn kind_bar(v: V, new: map_app::NewNode, [x, y]: [f32; 2], k: f32) -> El {
    use super::icons::{BULLETS, HEADING, NUMBERS, PARAGRAPH, TASK};
    use crate::map::NewKind;
    let p = v.p;
    let tip = |name: &str, id: &str| crate::keymap::tip(v.s.preset(), name, id);
    let btn = |key: &str, glyph: &'static rune::Icon, help: String, kind: NewKind, on: bool, run: fn(&mut App, &mut Tx)| {
        let b = rune::row().key(key).size(28.0 * k, 28.0 * k).center().radius(6.0 * k).role("radio").checked(on).label(help.clone()).help(help);
        let glyph = rune::icon(glyph, 16.0 * k);
        if !new.kinds.has(kind) {
            return b.disabled(true).child(glyph.color(p.mute.alpha(0.35)));
        }
        let b = if on { b.fill(p.hover).border(k, p.line).child(glyph.color(p.ink)) } else { b.hover(p.hover).hover_group().child(glyph.color(p.mute).group_color(p.ink)) };
        b.cursor(rune::Cursor::Pointer).on_press_tx(move |s: &mut App, tx: &mut Tx| run(s, tx))
    };
    let level = match new.kind {
        NewKind::Heading(l) => Some(l),
        _ => new.kinds.heading,
    };
    let heading = level.map_or(NewKind::Heading(1), NewKind::Heading);
    let heading_tip = level.map_or_else(|| "Heading".to_string(), |l| tip(&format!("Heading {l}"), &format!("mapedit.heading-{l}")));
    let is = |kind: NewKind| new.kind == kind;
    rune::row()
        .key("map-kinds")
        .pin(Some(x + W), Some(y + W), None, None)
        .h(map_app::KIND_BAR_H * k)
        .px(4.0 * k)
        .gap(2.0 * k)
        .center_y()
        .fill(p.bg)
        .radius(super::widgets::BAR_RADIUS * k)
        .border(k, Roles::new(p).hairline)
        .box_shadow(rune::Shadow { x: 0.0, y: 6.0 * k, blur: 22.0 * k, spread: -4.0 * k, color: Color::BLACK.alpha(if p.dark { 0.5 } else { 0.12 }), inset: false })
        .role("toolbar")
        .label("New node type")
        // A press anywhere on it leaves the keyboard and the text in the field (MAP-017).
        .keeps_text_selection()
        .on_press(|_: &mut App| {})
        .child(btn("map-kind-paragraph", &PARAGRAPH, tip("Paragraph", "mapedit.paragraph"), NewKind::Paragraph, is(NewKind::Paragraph), |s, tx| s.map_pick_kind(NewKind::Paragraph, tx)))
        .child(btn("map-kind-heading", &HEADING, heading_tip, heading, matches!(new.kind, NewKind::Heading(_)), |s, tx| s.map_pick_heading(tx)))
        .child(el().key("map-kinds-sep").w(k).h(16.0 * k).mx(5.0 * k).fill(p.line))
        .child(btn("map-kind-bullets", &BULLETS, tip("Bulleted list", "mapedit.bulleted"), NewKind::Bullet, is(NewKind::Bullet), |s, tx| s.map_pick_kind(NewKind::Bullet, tx)))
        .child(btn("map-kind-numbers", &NUMBERS, tip("Numbered list", "mapedit.numbered"), NewKind::Number, is(NewKind::Number), |s, tx| s.map_pick_kind(NewKind::Number, tx)))
        .child(btn("map-kind-task", &TASK, tip("Task", "mapedit.task"), NewKind::Task, is(NewKind::Task), |s, tx| s.map_pick_kind(NewKind::Task, tx)))
}

#[allow(clippy::too_many_arguments)]
/// The find query and, when the current match is in this node, where: (card piece or None
/// for the label, match in it) (MAP-066).
#[derive(Clone, Copy, Hash)]
struct Found<'a> {
    q: &'a str,
    cur: Option<(Option<usize>, usize)>,
}

/// How node `i` draws, besides its place.
struct State<'a> {
    folded: bool,
    hidden: usize,
    selected: bool,
    editing: bool,
    alpha: f32,
    lite: bool,
    no_hover: bool,
    plain_label: bool,
    /// A task box's least target, in map points.
    target: f32,
    /// Target world-point side for compact icon-only card chips below 0.65× zoom.
    chip_compact: Option<f32>,
    found: Option<Found<'a>>,
}

/// Find's highlights over a label's text (MAP-066): the find colour, the current match solid.
fn label_marks(c: &Roles, text: &str, f: Found) -> Vec<TextHighlight> {
    let cur = f.cur.filter(|(pid, _)| pid.is_none()).map(|(_, k)| k);
    let mark = |(k, r)| if Some(k) == cur { TextHighlight::new(r, c.find_current).color(c.find_current_text).key(crate::reader::FIND_CUR) } else { TextHighlight::new(r, c.find) };
    crate::writer_app::find_in(text, f.q).into_iter().enumerate().map(mark).collect()
}

fn node(cx: &Cx, t: &Tree, l: &Laid, i: usize, st: State) -> El {
    let State { folded, hidden, selected, editing, alpha, lite, no_hover, plain_label, target, chip_compact, found } = st;
    let Some(p) = l.at[i] else { return el() };
    let (c, b) = (&cx.look.c, cx.look.b);
    let n = &t.nodes[i];
    let tier = map_app::tier(t, i);
    let color = match (tier, &n.kind) {
        (Tier::Root | Tier::Head1 | Tier::Head, _) => c.strong,
        (_, NodeKind::Item { task: crate::blocks::Task::Done, .. }) | (Tier::Card, _) => c.dim,
        _ => c.text,
    };
    let branch = branch_color(c, n.branch);
    let key = node_key(t, i);
    let room = map_app::task_room(n, b);
    let (pad_x, pad_b) = (l.metrics.pad_x, l.metrics.pad_b);
    let [label_x, label_end] = p.label_pads(pad_x, room);
    // The room the task box's target may take, from the node's own corner: its cell, off the
    // label.
    let cell = l.cell[i];
    let (box_x, [lo, hi]) = p.task_slot(pad_x, room, crate::page::r(b * 0.9));
    let lw = line_width(p.depth);
    let line_h = p.line_room();
    // The content sits on the line, or hangs under it.
    let ((top, content_h), line_top) = (p.content(pad_b), p.line_top());
    let room_at = [(cell[0] - p.x).max(lo), cell[1] - p.y - top, (cell[2] - p.x).min(hi), cell[3] - p.y - top];
    let tail = key_tail(&key).to_string();
    // Below `PLAIN_LABEL_ZOOM` (0.20×, where rows are <= 5 px tall), mount text nodes as a
    // single flat plain-text element with its line on `border_sides`, skipping rich inline
    // code-chip background spans, hover animations, and wrapper containers.
    if plain_label && !editing && !matches!(&n.body, Body::Card(_)) {
        let style = map_app::style(tier, b, cx.look.family, cx.look.mono);
        let raw = map_app::shown_label(n);
        let shown = if raw.is_empty() { " ".to_string() } else { raw };
        let mut e = text(shown.clone())
            .key(key)
            .font(style)
            .color(color)
            .wrap_anywhere()
            .pl(label_x)
            .pr(label_end)
            .pt(top)
            .pin(Some(p.x + W), Some(p.y + W), None, None)
            .size(p.w, p.h + line_h)
            .border_sides(if p.overlined() { [lw, 0.0, 0.0, 0.0] } else { [0.0, 0.0, if p.depth > 0 { lw } else { 0.0 }, 0.0] }, branch)
            .role("treeitem")
            .label(spoken(t, i, folded, hidden))
            .level(n.depth as u32 + 1)
            .focus_selected(selected)
            .opacity(alpha);
        if let Some(f) = found {
            e = e.text_highlights(label_marks(c, &shown, f));
        }
        if !n.children.is_empty() {
            e = e.expanded(!folded);
        }
        return e
            .children((room > 0.0).then(|| el().pin(Some(0.0), Some(top), None, None).size(p.w, content_h).child(task_box(n, i, b, box_x, target, room_at, c, true, true))))
            .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, false, tx))
            .on_double_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, true, tx))
            .on_secondary_press(move |s: &mut App, at, tx| s.map_node_menu(i, Some((at.x, at.y)), tx))
            .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_node_drag(i, d, tx));
    }
    let copied = matches!(&n.body, Body::Card(bl) if super::preview::code::copied(cx, bl.lines.start));
    // Memoize the node's inner body (task box, rich label / card, bracket) independently of
    // `(p.x, p.y, alpha)` so rigid translations during fold/unfold relayout glides hit the cache
    // on every unchanged branch without rebuilding labels, cards or task controls.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (i, n.src, n.depth, selected, editing, b.to_bits(), room.to_bits(), [p.w, p.h].map(f32::to_bits), color_bits(color), color_bits(branch)).hash(&mut h);
    let pics = if n.pictures() { cx.v.s.images.epoch.get() } else { 0 };
    (std::ptr::from_ref(cx.v.p) as usize, cx.v.s.names_gen, cx.look.family, cx.look.mono, copied, pics, (lite, plain_label), target.to_bits(), chip_compact.map(f32::to_bits), room_at.map(f32::to_bits), found).hash(&mut h);
    p.dir.hash(&mut h);
    let body_key = format!("map-body:{tail}");
    let body_tail = tail.clone();
    let grow = if selected && !lite { glass_k(p.w, p.h) } else { 1.0 };
    let body = rune::memo(body_key.clone(), h.finish(), move || {
        let st = map_app::style(tier, b, cx.look.family, cx.look.mono);
        let hl = found.map(|f| Hl::new(f.q, f.cur.and_then(|(pid, k)| Some((pid?, k)))));
        let cx = &Cx::map(cx.v, cx.look, hl.as_ref());
        let label_el = (!editing).then(|| {
            let shown = match &n.body {
                Body::Card(bl) if lite && !matches!(bl.kind, crate::blocks::Kind::Image { .. } | crate::blocks::Kind::Gallery { .. }) => {
                    Some(card::lite(cx, bl, p.w - 2.0 * pad_x - room, p.h - pad_b, &body_tail))
                }
                Body::Card(bl) => card::card(cx, bl, i, &body_tail, chip_compact).map(|(e, _)| e),
                _ => None,
            };
            let text = || {
                let mut r = label(cx, n, st, color);
                if let Some(f) = found {
                    r.marks = label_marks(c, &r.text, f);
                }
                r.el(st, color, None).wrap_anywhere().w(p.w - 2.0 * pad_x - room).clip_soft(0.0)
            };
            shown.unwrap_or_else(text).pin(Some(label_x), Some(0.0), None, None)
        });
        // The node's content, its task box with its label or card, grows as one piece under the
        // selection glass (MAP-055); the line stays put.
        let content = el()
            .pin(Some(0.0), Some(top), None, None)
            .size(p.w, content_h)
            .scale(grow)
            .children((room > 0.0).then(|| task_box(n, i, b, box_x, target, room_at, c, lite, plain_label)))
            .children(label_el);
        el().key(body_key)
            .pin(Some(0.0), Some(0.0), None, None)
            .size(p.w, p.h + line_h)
            .child(content)
            .children(p.braced().then(|| rune::canvas(brace(Shape::new(), [0.0, 0.0], [p.w, line_h], p.overlined(), lw, branch)).size(p.w, line_h).inert().pin(Some(0.0), Some(line_top), None, None)))
    });
    let mut e = el().key(key)
        .pin(Some(p.x + W), Some(p.y + W), None, None)
        .size(p.w, p.h + line_h)
        .role("treeitem")
        .label(spoken(t, i, folded, hidden))
        .level(n.depth as u32 + 1)
        .focus_selected(selected)
        .opacity(alpha);
    if !selected && !editing && !no_hover {
        e = e.radius(4.0).hover(c.hover).hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1));
    }
    if !n.children.is_empty() {
        e = e.expanded(!folded);
        if !folded && !editing && !lite {
            e = e.hover_group_shared(format!("map-row:{tail}"));
        }
    }
    e.on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, false, tx))
        .on_double_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, true, tx))
        .on_secondary_press(move |s: &mut App, at, tx| s.map_node_menu(i, Some((at.x, at.y)), tx))
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_node_drag(i, d, tx))
        .child(body)
}

/// A task's box beside its label, its left edge at `bx_x`, centred on the first line, in a
/// target `target` map points clamped to `cell`: its node's hit cell from the node's corner,
/// clear of its label text (MAP-053, MAP-073).
#[allow(clippy::too_many_arguments)]
fn task_box(n: &crate::map::Node, i: usize, b: f32, bx_x: f32, target: f32, cell: [f32; 4], c: &Roles, lite: bool, plain_label: bool) -> El {
    let done = matches!(n.kind, NodeKind::Item { task: crate::blocks::Task::Done, .. });
    let bx = crate::page::r(b * 0.9);
    let lh = crate::page::line(b, 1.45);
    let hit = target.max(bx);
    let bx_y = (lh - bx) / 2.0;
    let (vcx, vcy) = (bx_x + bx / 2.0, lh / 2.0);
    let x0 = (vcx - hit / 2.0).max(cell[0]).min(bx_x);
    let x1 = (vcx + hit / 2.0).min(cell[2]).max(bx_x + bx);
    let y0 = (vcy - hit / 2.0).max(cell[1]).min(bx_y);
    let y1 = (vcy + hit / 2.0).min(cell[3]).max(bx_y + bx);
    let (rad, edge) = (2f32.max(crate::page::r(bx * 0.24)), 1f32.max(crate::page::r(b / 12.0)));
    let mut bx_el = el().size(bx, bx).pin(Some(bx_x - x0), Some(bx_y - y0), None, None);
    if !plain_label {
        bx_el = bx_el.radius(rad);
    }
    if lite {
        bx_el = if done {
            bx_el
                .center()
                .fill(c.accent)
                .children((!plain_label).then(|| rune::icon(&super::icons::CHECK, (bx * 0.78).round()).color(c.bg)))
        } else {
            bx_el.border(edge, c.faint)
        };
        return el()
            .key(format!("map-task:{}", n.own.start + 1))
            .size(x1 - x0, y1 - y0)
            .role("checkbox")
            .checked(done)
            .label(if done { "Done" } else { "To do" })
            .cursor(rune::Cursor::Pointer)
            .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_task_click(i, tx))
            .pin(Some(x0), Some(y0), None, None)
            .child(bx_el);
    }
    let accent = Roles::readable_on(c.accent, c.bg, GRAPHIC);
    let halo = el()
        .size(bx + 6.0, bx + 6.0)
        .radius(rad + 2.0)
        .fill(accent.alpha(0.18))
        .group_opacity(0.0, 1.0)
        .pin(Some(bx_x - 3.0 - x0), Some(bx_y - 3.0 - y0), None, None)
        .inert();
    bx_el = if done {
        bx_el.center().fill(c.accent).child(rune::icon(&super::icons::CHECK, (bx * 0.78).round()).color(c.bg))
    } else {
        bx_el
            .border(edge, c.faint)
            .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(rad).border(edge, accent).group_opacity(0.0, 1.0).inert())
    };
    el().key(format!("map-task:{}", n.own.start + 1))
        .size(x1 - x0, y1 - y0)
        .role("checkbox")
        .checked(done)
        .label(if done { "Done" } else { "To do" })
        .cursor(rune::Cursor::Pointer)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
        .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_task_click(i, tx))
        .pin(Some(x0), Some(y0), None, None)
        .child(halo)
        .child(bx_el)
}

/// A node whose rings are drawn: how it is shown, and the size its targets aim for.
struct RingOf<'a> {
    t: &'a Tree,
    l: &'a Laid,
    i: usize,
    editing: bool,
    lite: bool,
    hit: f32,
    c: &'a Roles,
    font: rune::TextStyle,
}

/// A press on node `i`'s ring or pill on side `d`: fold or unfold what it hides, and everything
/// under that with Ctrl.
fn fold_press(i: usize, d: crate::map_geom::Dir) -> impl Fn(&mut App, &mut Tx) + 'static {
    move |s, tx| {
        let all = tx.mods().cmd;
        s.map_toggle(i, Some(d), all, tx);
        s.keys_to_note(tx);
    }
}

/// A node's `nth` fold ring and its hit target, or the `+N` pill in the ring's place.
fn fold(&RingOf { t, l, i, editing, lite, hit, c, font }: &RingOf, ring: &crate::map_layout::Ring, nth: usize) -> impl Iterator<Item = El> + use<> {
    let (mut dot, mut target, mut pill) = (None, None, None);
    let (d, folded) = (ring.side.dir, ring.folded);
    let tail = key_tail(&node_key(t, i)).to_string();
    let branch = branch_color(c, t.nodes[i].branch);
    let (Some(p), Some([x0, y0, x1, y1])) = (l.at[i], l.ring_hit(i, d, hit)) else { return dot.into_iter().chain(target).chain(pill) };
    let [cx, cy] = p.ring(d);
    // A node with a ring on each side keys the second by its number.
    let key = |name: &str| if nth == 0 { format!("{name}:{tail}") } else { format!("{name}:{tail}:{}", nth + 1) };
    if folded {
        let hidden = ring.side.nodes.len();
        let (count, size) = map_app::pill_size(hidden, font);
        let [x, y, w, h] = p.pill_box(d, size);
        pill = Some(
            el().key(key("map-pill"))
                .size(w, h)
                .center()
                .fill(c.bg)
                .hover(crate::roles::mix(c.bg, branch, 0.18))
                .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
                .border(1.5, branch)
                .radius(h / 2.0)
                .role("button")
                .label(format!("Unfold, {hidden} hidden"))
                .cursor(rune::Cursor::Pointer)
                .pin(Some(x + W), Some(y + W), None, None)
                .child(text(count).font(font).color(Roles::readable_on(c.dim, c.bg, 4.8)).inert())
                .on_press_tx(fold_press(i, d)),
        );
    } else {
        // The ring the branches leave from, at rest too; hovering adds its `−` and a halo.
        dot = Some(
            el().key(key("map-dot"))
                .pin(Some(cx - FOLD_DOT_R + W), Some(cy - FOLD_DOT_R + W), None, None)
                .size(2.0 * FOLD_DOT_R, 2.0 * FOLD_DOT_R)
                .radius(FOLD_DOT_R)
                .border(1.5, branch)
                .fill(c.bg)
                .inert(),
        );
    }
    if !editing {
        let mut hit_el = el()
            .key(key("map-fold"))
            .pin(Some(x0 + W), Some(y0 + W), None, None)
            .size(x1 - x0, y1 - y0)
            .role("button")
            .label(if folded { "Unfold" } else { "Fold" })
            .cursor(rune::Cursor::Pointer)
            .on_press_tx(fold_press(i, d));
        if !folded && !lite {
            let (fw, fh) = (x1 - x0, y1 - y0);
            let (rcx, rcy) = (cx - x0, cy - y0);
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (d, fw.to_bits(), fh.to_bits(), rcx.to_bits(), rcy.to_bits(), color_bits(branch)).hash(&mut hash);
            let vis_key = key("map-fold-v");
            let visuals = rune::memo(vis_key.clone(), hash.finish(), move || {
                let r = FOLD_DOT_R;
                let halo_r = r + 3.0;
                let halo_wrap = el()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .hover_group()
                    .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
                    .child(
                        el().size(2.0 * halo_r, 2.0 * halo_r)
                            .radius(halo_r)
                            .fill(branch.alpha(0.18))
                            .group_opacity(0.0, 1.0)
                            .pin(Some(rcx - halo_r), Some(rcy - halo_r), None, None)
                            .inert(),
                    );
                let minus = el()
                    .size(2.0 * r, 2.0 * r)
                    .center()
                    .group_opacity(0.0, 1.0)
                    .pin(Some(rcx - r), Some(rcy - r), None, None)
                    .inert()
                    .child(el().size(6.0, 1.5).radius(0.75).fill(branch).inert());
                el().key(vis_key)
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .child(halo_wrap)
                    .child(minus)
            });
            hit_el = hit_el
                .hover_group_shared(format!("map-row:{tail}"))
                .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
                .child(visuals);
        }
        target = Some(hit_el);
    }
    dot.into_iter().chain(target).chain(pill)
}

/// The node a drag carries, drawn as a screen-space overlay clamped inside `canvas_size` so it
/// stays bounded (<= `GHOST_MAX_W`), crisp at any zoom, clear of the Child drop stub and never
/// clipped behind docked chrome (MAP-026).
fn ghost(cx: &Cx, t: &Tree, i: usize, pointer: [f32; 2], canvas_size: [f32; 2], child_bottom: Option<f32>) -> El {
    let (c, b, mono) = (&cx.look.c, cx.look.b, cx.look.mono);
    let n = &t.nodes[i];
    let weight = match map_app::tier(t, i) {
        Tier::Root | Tier::Head1 | Tier::Head => 700.0,
        _ => 400.0,
    };
    let mut st = styles::note(cx.look.family, b, page::line(b, 1.35));
    st.weight = weight;
    let small = (b * 0.72).round();
    let pill_h = small + 6.0;
    let pill_font = styles::note(mono, small, normal_lh(mono, small));
    let below = n.end - i - 1;
    let surface = c.on_page(c.raised);
    let branch = branch_color(c, n.branch);
    let shadow = rune::Shadow {
        x: 0.0,
        y: 4.0,
        blur: 12.0,
        spread: 0.0,
        color: Color::BLACK.alpha(if cx.v.dark { 0.36 } else { 0.14 }),
        inset: false,
    };
    let bx = page::r(b * 0.9);
    let task_el = match n.kind {
        NodeKind::Item { task: crate::blocks::Task::Done, .. } if !n.card() => {
            let rad = 2f32.max(page::r(bx * 0.24));
            Some(el().size(bx, bx).radius(rad).center().fill(c.accent).child(rune::icon(&super::icons::CHECK, (bx * 0.78).round()).color(c.bg)))
        }
        NodeKind::Item { task: crate::blocks::Task::Todo, .. } if !n.card() => {
            let rad = 2f32.max(page::r(bx * 0.24));
            let edge = 1f32.max(page::r(b / 12.0));
            Some(el().size(bx, bx).radius(rad).border(edge, c.faint))
        }
        _ => None,
    };
    let gap = 6.0;
    let pad_x = 10.0;
    let task_w = if task_el.is_some() { bx + gap } else { 0.0 };
    let pill_text = (below > 0).then(|| format!("+{below}"));
    let pill_w = pill_text.as_deref().map_or(0.0, |pt| rune::text::measure(pt, &[], pill_font, None).0.ceil() + 10.0 + gap);
    let label_text = map_app::shown_label(n);
    let label_nat = rune::text::measure(&label_text, &[], st, None).0.ceil() + 2.0;
    let max_label_w = (GHOST_MAX_W - 2.0 * pad_x - task_w - pill_w).max(40.0);
    let label_w = label_nat.min(max_label_w);
    let ghost_w = (2.0 * pad_x + task_w + label_w + pill_w).ceil().min(GHOST_MAX_W);
    let ghost_h = (st.line_height + 10.0).ceil().max(28.0);
    let [cw, ch] = canvas_size;
    let mut x = pointer[0] + GHOST_DX;
    if x + ghost_w > cw - GHOST_EDGE {
        x = pointer[0] - GHOST_DX - ghost_w;
    }
    x = x.clamp(GHOST_EDGE, (cw - ghost_w - GHOST_EDGE).max(GHOST_EDGE));
    let mut y = pointer[1] + GHOST_DY;
    if let Some(cb) = child_bottom {
        y = y.max(cb + 8.0);
    }
    if y + ghost_h > ch - GHOST_EDGE {
        y = pointer[1] - GHOST_DY - ghost_h;
    }
    y = y.clamp(GHOST_EDGE, (ch - ghost_h - GHOST_EDGE).max(GHOST_EDGE));
    let label_el = text(label_text).font(st).color(c.text).truncate().w(label_w).min_w(0.0).clip_soft(0.0);
    rune::row()
        .key("map-ghost")
        .pin(Some(x), Some(y), None, None)
        .w(ghost_w)
        .max_w(GHOST_MAX_W)
        .center_y()
        .gap(gap)
        .px(pad_x)
        .py(5.0)
        .min_h(28.0)
        .radius(6.0)
        .fill(surface)
        .border(1.5, branch)
        .box_shadow(shadow)
        .opacity(0.92)
        .children(task_el)
        .child(label_el)
        .children(pill_text.map(|pt| {
            text(pt)
                .font(pill_font)
                .color(Roles::readable_on(c.dim, c.on_page(c.chip_fill), 4.8))
                .fill(c.chip_fill)
                .border(1.0, c.chip_edge)
                .radius(pill_h / 2.0)
                .px(5.0)
                .h(pill_h)
        }))
        .inert()
}

/// Where a drop would put the node: an accent bar above or below the target for Before and
/// After, or a tinted box round it with a stub of a branch for Child (MAP-026). Its label names
/// the zone and the target's key.
fn drop_mark(t: &Tree, l: &Laid, at: crate::map::Add, c: &Roles, zoom: f32) -> El {
    use crate::map::Add;
    let Some(mark) = l.drop_mark(at, zoom.max(map_app::ZOOM_MIN)) else { return el() };
    let accent = Roles::readable_on(c.accent, c.bg, GRAPHIC);
    let e = match mark {
        DropMark::Bar([x, y, w, h]) => el().pin(Some(x + W), Some(y + W), None, None).size(w, h).radius(w.min(h) / 2.0).fill(accent),
        DropMark::Child { frame: [x, y, w, h], stub: [sx, sy, sw, sh], edge } => {
            let wash = accent.alpha(if c.bg.r < 0.2 { 0.18 } else { 0.14 });
            el().pin(Some(x + W), Some(y + W), None, None)
                .size(w, h)
                .radius(4.0 + (edge - 2.0))
                .fill(wash)
                .border(edge, accent)
                .child(el().pin(Some(sx), Some(sy), None, None).size(sw, sh).fill(accent))
        }
    };
    let zone = match at {
        Add::Before(_) => "Before",
        Add::After(_) => "After",
        Add::Child(_) => "Child",
    };
    e.key("map-drop").label(format!("{zone} {}", node_key(t, at.node()))).inert()
}

/// Quiet bottom-right canvas zoom pill (`map-zoom-hud`, MAP-064): `−`, clickable tabular-mono
/// zoom percentage (resets to 100%), `+`, and `Fit`, with WCAG AA contrast on every theme.
/// Mounted in `doc-area` after `toast-band` so bottom-right controls stay visible while a toast shows.
pub(super) fn zoom_hud(v: V) -> Option<El> {
    let s = v.s;
    let [cw, _] = s.map_size();
    if s.active_locked() || cw < 360.0 || (s.foot_shows() && cw < 640.0) {
        return None;
    }
    let look = map_look(v);
    let c = &look.c;
    let mono = look.mono;
    let zoom = s.map_target_zoom();
    let preset = s.preset();
    let tip = |name: &str, id: &str| crate::keymap::tip(preset, name, id);
    let hud_bg = c.on_page(c.panel);
    let hud_edge = if v.dark { crate::roles::mix(c.bg, c.text, 0.14) } else { c.on_page(c.hairline_soft) };
    let sep_color = Roles::readable_on(hud_edge, hud_bg, 3.2);
    let hud_ink = Roles::readable_on(c.dim, hud_bg, 4.8);
    let hud_hover = crate::roles::mix(hud_bg, c.text, if v.dark { 0.10 } else { 0.06 });
    let shadow = rune::Shadow {
        x: 0.0,
        y: 1.0,
        blur: 6.0,
        spread: 0.0,
        color: Color::BLACK.alpha(if v.dark { 0.24 } else { 0.08 }),
        inset: false,
    };
    let icon_btn = |key: &'static str, glyph: &'static rune::Icon, label: &'static str, help: String, run: fn(&mut App, &mut Tx)| {
        rune::row()
            .key(key)
            .size(24.0, 24.0)
            .center()
            .radius(4.0)
            .hover(hud_hover)
            .hover_group()
            .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
            .cursor(rune::Cursor::Pointer)
            .role("button")
            .label(label)
            .help(help)
            .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                run(s, tx);
                s.keys_to_note(tx);
            })
            .child(rune::icon(glyph, 12.0).color(hud_ink).group_color(c.text))
    };
    let pct = format!("{}%", (zoom * 100.0).round() as i32);
    let pct_font = styles::note(mono, 11.0, normal_lh(mono, 11.0)).features("'tnum' 1");
    let reset = rune::row()
        .key("map-zoom-reset")
        .h(24.0)
        .min_w(42.0)
        .px(5.0)
        .center()
        .radius(4.0)
        .hover(hud_hover)
        .hover_group()
        .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(format!("Zoom {pct}, reset to 100%"))
        .help(tip("Zoom 100%", "map.zoom-reset"))
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            s.map_zoom_reset(tx);
            s.keys_to_note(tx);
        })
        .child(text(pct).font(pct_font).color(hud_ink).group_color(c.text));
    let sep = el().size(1.0, 14.0).mx(2.0).fill(sep_color).inert();
    Some(
        rune::row()
            .key("map-zoom-hud")
            .pin(None, None, Some(12.0), Some(12.0))
            .h(28.0)
            .px(3.0)
            .gap(1.0)
            .center_y()
            .radius(6.0)
            .fill(hud_bg)
            .border(1.0, hud_edge)
            .box_shadow(shadow)
            .child(icon_btn("map-zoom-out", &super::icons::MINUS, "Zoom out", tip("Zoom out", "map.zoom-out"), |s, tx| s.map_zoom_step(0.8, tx)))
            .child(reset)
            .child(icon_btn("map-zoom-in", &super::icons::PLUS, "Zoom in", tip("Zoom in", "map.zoom-in"), |s, tx| s.map_zoom_step(1.25, tx)))
            .child(sep)
            .child(icon_btn("map-zoom-fit", &super::icons::FIT, "Fit to view", tip("Fit to view", "map.fit"), |s, tx| s.map_fit(tx))),
    )
}

fn color_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// Where node `i` shows in the canvas's own points, for tests of the camera.
#[allow(dead_code)]
pub fn on_canvas(p: Place, cam: ([f32; 2], f32)) -> [f32; 2] {
    [cam.0[0] + p.x * cam.1, cam.0[1] + p.y * cam.1]
}
