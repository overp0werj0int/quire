//! The map canvas (MAP-032 to MAP-045, MAP-055, MAP-075): the note's tree on a canvas that
//! pans and zooms without bounds. One world element carries the camera as live values; every
//! node, link and fold circle sits in it at its world place. Only what meets the canvas and
//! its overscan is built.

mod card;
mod lightbox;

use std::hash::{Hash, Hasher};

use rune::{Color, Drag, El, LineCap, MagnifyEvent, Path, PointerButton, Shape, TextHighlight, TextStyle, Tx, WheelEvent, el, text};

use super::preview::{Cx, Hl, Look, Rich, Tone, look_at};
pub(super) use lightbox::lightbox;
use super::{V, styles};
use crate::app::App;
use crate::blocks::Run;
use crate::map::{Body, Node, NodeKind, ROOT, Tree};
use crate::map_app::{self, FOLD_DOT_R, FOLD_R, Tier};
use crate::map_layout::{Laid, Place, line_width};
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

/// What a screen reader hears for node `i` (MAP-075).
fn spoken(t: &Tree, i: usize, folded: bool) -> String {
    let n = &t.nodes[i];
    let shown = map_app::shown_label(n);
    let name = if n.label.is_empty() || n.label == "\u{200b}" || shown == "\u{200b}" { "New node" } else { shown.as_str() };
    let mut s = format!("{name}, level {}", n.depth + 1);
    if !n.children.is_empty() {
        if folded {
            s += &format!(", collapsed, {} hidden", n.end - i - 1);
        } else {
            s += ", expanded";
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
    let folded = |i: usize| view.is_some_and(|v| v.folded.contains(&t.nodes[i].id));
    let (b, prose, mono) = (look.b, look.family, look.mono);
    // A fold circle's or task box's target is at least 24 points on screen (MAP-041, MAP-073).
    // Pin detail level during active camera glides (`map_detail_zoom`) so zooming across
    // `LITE_ZOOM` or `hit_bucket` boundaries never mounts/rebuilds dozens of cards mid-glide.
    let dz = s.map_detail_zoom();
    let bucket_zoom = 2f32.powi(map_app::hit_bucket(dz));
    let target = 24.0 / bucket_zoom;
    let hit = target.max(2.0 * FOLD_R);
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
        Some((i, p, room, st, map_app::field_size(e, p, room, st, l.metrics)))
    });
    // The selected node sits under a glass card (MAP-055). Only its content grows under the glass
    // (`node`); its underline, links and fold ring stay put, so no line moves or breaks there.
    let glass = nodes.contains(&sel).then(|| l.at[sel]).flatten().map(|g| {
        let lw = line_width(g.depth);
        let [w, h] = match field_at {
            Some((i, _, room, _, [fw, fh])) if i == sel => [g.w.max(fw + room + 2.0 * l.metrics.pad_x), g.h.max(fh + l.metrics.pad_b)],
            _ => [g.w, g.h],
        };
        let (px, py) = (GLASS_PAD_X, GLASS_PAD_Y);
        let r = [g.x - px, g.y - py, w + 2.0 * px, h + lw + 2.0 * py];
        (r, GLASS_R.min(r[3] / 2.0))
    });
    if let Some((r, rad)) = glass {
        world = world.child(glass_base(c, r, rad));
    }
    if plain_label {
        let mut static_links = Vec::new();
        for &i in &links {
            let a = dim(i);
            if a < 0.999 {
                world = world.child(link(&t, &l, i, c, a));
            } else {
                static_links.push(i);
            }
        }
        if let Some(batch) = links_batch(&t, &l, &static_links, c) {
            world = world.child(batch);
        }
    } else {
        for &i in &links {
            world = world.child(link(&t, &l, i, c, dim(i)));
        }
    }
    let query = s.find_query();
    let cur = query.and_then(|_| s.map_hits().get(s.reader.find.cur).copied());
    for &i in &nodes {
        let found = query.map(|q| Found { q, cur: cur.filter(|h| h.node == i).map(|h| (h.pid, h.k)) });
        let st = State { folded: folded(i), selected: i == sel, editing: editing == Some(i), alpha: dim(i), lite, no_hover, plain_label, target, chip_compact, found };
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
        if !t.nodes[i].children.is_empty() {
            world = world.children(fold(&t, &l, i, folded(i), editing == Some(i), lite, hit, c, b, mono).into_iter().map(|e| e.opacity(dim(i))));
        }
    }
    if let Some((_, p, room, st, [w, _])) = field_at {
        world = world.child(field(v, p, room, st, w, l.metrics.pad_x));
    }
    if let Some(g) = drag
        && let Some(at) = g.drop
    {
        world = world.child(drop_mark(&t, &l, at, c, cam.1));
    }
    let ghost_el = drag.and_then(|g| {
        let m = t.find(g.node)?;
        let child_bottom = match g.drop {
            Some(crate::map::Add::Child(j)) => l.at[j].map(|p| cam.0[1] + p.bottom() * cam.1),
            _ => None,
        };
        Some(ghost(&cx, &t, m, g.at, s.map_size(), child_bottom))
    });
    if nodes.contains(&ROOT)
        && let Some(p) = l.at[ROOT]
    {
        let font = map_app::chip_font(b, mono);
        if t.props > 0 {
            let (badge, [x, y, _, h]) = map_app::badge(t.props, p, l.metrics.pad_x, font);
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
            let y = p.bottom() + 6.0 + if t.props > 0 { 28.0 } else { 0.0 };
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
    let mut segs: Vec<(f32, f32, f32, f32, f32, Color)> = Vec::with_capacity(links.len());
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &i in links {
        let (Some(p), Some(q)) = (l.at[i], t.nodes[i].parent.and_then(|q| l.at[q])) else { continue };
        let lw = line_width(p.depth);
        let [x0, ya] = map_app::branch_start(&q);
        let (x1, yb) = (p.x, p.line_y());
        let color = branch_color(c, t.nodes[i].branch);
        bx0 = bx0.min(x0.min(x1) - lw);
        by0 = by0.min(ya.min(yb) - lw);
        bx1 = bx1.max(x0.max(x1) + lw);
        by1 = by1.max(ya.max(yb) + lw);
        segs.push((x0, ya, x1, yb, lw, color));
    }
    if segs.is_empty() {
        return None;
    }
    let w = (bx1 - bx0).max(1.0);
    let h = (by1 - by0).max(1.0);
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (w.to_bits(), h.to_bits(), segs.len()).hash(&mut hash);
    for &(x0, ya, x1, yb, lw, color) in &segs {
        ((x0 - bx0).to_bits(), (ya - by0).to_bits(), (x1 - bx0).to_bits(), (yb - by0).to_bits(), lw.to_bits(), color_bits(color)).hash(&mut hash);
    }
    let shape_el = rune::memo("map-links-batch".to_string(), hash.finish(), move || {
        let mut shape = Shape::new();
        for (x0, ya, x1, yb, lw, color) in segs {
            let (lx0, ly0, lx1, ly1) = (x0 - bx0, ya - by0, x1 - bx0, yb - by0);
            let path = if (ly0 - ly1).abs() < 0.5 {
                let y = (ly0 + ly1) / 2.0;
                Path::new().move_to([lx0, y]).line_to([lx1, y])
            } else {
                let mid = (lx0 + lx1) / 2.0;
                Path::new().move_to([lx0, ly0]).cubic_to([mid, ly0], [mid, ly1], [lx1, ly1])
            };
            shape = shape.stroke_path(&path, lw, LineCap::Round, color);
        }
        rune::canvas(shape).size(w, h).inert()
    });
    Some(shape_el.pin(Some(bx0 + W), Some(by0 + W), None, None))
}

fn link(t: &Tree, l: &Laid, i: usize, c: &Roles, alpha: f32) -> El {
    let (Some(p), Some(q)) = (l.at[i], t.nodes[i].parent.and_then(|q| l.at[q])) else { return el() };
    let lw = line_width(p.depth);
    let [x0, ya] = map_app::branch_start(&q);
    let (x1, yb) = (p.x, p.line_y());
    let tail = key_tail(&node_key(t, i)).to_string();
    let key = format!("map-link:{tail}");
    let color = branch_color(c, t.nodes[i].branch);
    let w = (x1 - x0).max(1.0);
    let top = ya.min(yb) - lw;
    let h = (ya.max(yb) - top + lw).max(1.0);
    let (y0, y1) = (ya - top, yb - top);
    // Memoize the stroked Bézier shape by relative geometry `(w, y0, y1, lw, color)` so rigid
    // subtree translations during fold/unfold relayout glides never re-tessellate unchanged links.
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (w.to_bits(), h.to_bits(), y0.to_bits(), y1.to_bits(), lw.to_bits(), color_bits(color)).hash(&mut hash);
    let shape = rune::memo(format!("map-link-s:{tail}"), hash.finish(), move || {
        rune::canvas(Shape::new().fill(tapered([0.0, y0], [w, y1], lw), color)).size(w, h).inert()
    });
    el().key(key)
        .pin(Some(x0 + W), Some(top + W), None, None)
        .size(w, h)
        .opacity(alpha)
        .inert()
        .child(shape)
}

/// The selection glass's padding round its node and its corner radius (MAP-055).
const GLASS_PAD_X: f32 = 12.0;
const GLASS_PAD_Y: f32 = 10.0;
const GLASS_R: f32 = 10.0;
/// How much the selected node's content grows under the glass.
const GLASS_K: f32 = 1.035;

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

/// The outline of a branch from `a` (its parent's fold dot) to `z` (its node's underline): the
/// link's Bézier, 1 point wide at the dot and widening to `lw` by halfway, so the branches
/// fanning out of one dot don't merge into a blot.
fn tapered(a: [f32; 2], z: [f32; 2], lw: f32) -> Vec<[f32; 2]> {
    const STEPS: usize = 32;
    let mid = (a[0] + z[0]) / 2.0;
    let (c1, c2) = ([mid, a[1]], [mid, z[1]]);
    let mut left = Vec::with_capacity(2 * STEPS + 2);
    let mut right = Vec::with_capacity(STEPS + 1);
    for k in 0..=STEPS {
        let t = k as f32 / STEPS as f32;
        let u = 1.0 - t;
        let at = |i: usize| u * u * u * a[i] + 3.0 * u * u * t * c1[i] + 3.0 * u * t * t * c2[i] + t * t * t * z[i];
        let slope = |i: usize| 3.0 * u * u * (c1[i] - a[i]) + 6.0 * u * t * (c2[i] - c1[i]) + 3.0 * t * t * (z[i] - c2[i]);
        let (dx, dy) = (slope(0), slope(1));
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let e = (t / 0.5).min(1.0);
        let half = (1.0 + (lw - 1.0).max(0.0) * e * e * (3.0 - 2.0 * e)) / 2.0;
        let (nx, ny) = (-dy / len * half, dx / len * half);
        left.push([at(0) + nx, at(1) + ny]);
        right.push([at(0) - nx, at(1) - ny]);
    }
    left.extend(right.into_iter().rev());
    left
}

/// The inline field over the node being edited, in the label's place and style (MAP-016).
fn field(v: V, p: Place, room: f32, st: rune::TextStyle, w: f32, pad_x: f32) -> El {
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
        .pin(Some(p.x + pad_x + room + W), Some(p.y + W), None, None)
        .w(w)
        .radius(3.0)
        .fill(surface)
        .wrap_anywhere()
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
    let State { folded, selected, editing, alpha, lite, no_hover, plain_label, target, chip_compact, found } = st;
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
    let clear = l.clear[i];
    let sh = l.metrics.sh;
    let lw = line_width(p.depth);
    let tail = key_tail(&key).to_string();
    // Below `PLAIN_LABEL_ZOOM` (0.20×, where rows are <= 5 px tall), mount text nodes as a
    // single flat plain-text element with its underline on `border_sides`, skipping rich inline
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
            .pl(pad_x + room)
            .pr(pad_x)
            .pin(Some(p.x + W), Some(p.y + W), None, None)
            .size(p.w, p.h + lw)
            .border_sides([0.0, 0.0, lw, 0.0], branch)
            .role("treeitem")
            .label(spoken(t, i, folded))
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
            .children((room > 0.0).then(|| task_box(n, i, p, b, pad_x, room, sh, target, clear, c, true, true)))
            .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, false, tx))
            .on_double_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, true, tx))
            .on_secondary_press(move |s: &mut App, at, tx| s.map_node_menu(i, Some((at.x, at.y)), tx))
            .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_node_drag(i, d, tx));
    }
    let copied = matches!(&n.body, Body::Card(bl) if super::preview::code::copied(cx, bl.lines.start));
    let rel_clear = [clear[0] - p.y, clear[1] - p.y];
    // Memoize the node's inner body (task box, rich label / card, underline) independently of
    // `(p.x, p.y, alpha)` so rigid translations during fold/unfold relayout glides hit the cache
    // on every unchanged branch without rebuilding labels, cards or task controls.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (i, n.src, n.depth, selected, editing, b.to_bits(), room.to_bits(), [p.w, p.h].map(f32::to_bits), color_bits(color), color_bits(branch)).hash(&mut h);
    let pics = if n.pictures() { cx.v.s.images.epoch.get() } else { 0 };
    (std::ptr::from_ref(cx.v.p) as usize, cx.v.s.names_gen, cx.look.family, cx.look.mono, copied, pics, (lite, plain_label), target.to_bits(), chip_compact.map(f32::to_bits), rel_clear.map(f32::to_bits), sh.to_bits(), found).hash(&mut h);
    let body_key = format!("map-body:{tail}");
    let body_tail = tail.clone();
    let grow = if selected && !lite { GLASS_K } else { 1.0 };
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
            shown.unwrap_or_else(text).pin(Some(pad_x + room), Some(0.0), None, None)
        });
        // The node's content, its task box with its label or card, grows as one piece under the
        // selection glass (MAP-055); the underline below it stays put.
        let content = el()
            .pin(Some(0.0), Some(0.0), None, None)
            .size(p.w, p.h)
            .scale(grow)
            .children((room > 0.0).then(|| task_box(n, i, p, b, pad_x, room, sh, target, clear, c, lite, plain_label)))
            .children(label_el);
        el().key(body_key)
            .pin(Some(0.0), Some(0.0), None, None)
            .size(p.w, p.h + lw)
            .child(content)
            .child(el().pin(Some(0.0), Some(p.h), None, None).size(p.w, lw).fill(branch).inert())
    });
    let mut e = el().key(key)
        .pin(Some(p.x + W), Some(p.y + W), None, None)
        .size(p.w, p.h + lw)
        .role("treeitem")
        .label(spoken(t, i, folded))
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

/// A task's box left of its label, centred on the first line, in a target `target` map points
/// clamped to its non-overlapping row slot and clear of its label text (MAP-053, MAP-073).
#[allow(clippy::too_many_arguments)]
fn task_box(n: &crate::map::Node, i: usize, p: Place, b: f32, pad_x: f32, room: f32, sh: f32, target: f32, clear: [f32; 2], c: &Roles, lite: bool, plain_label: bool) -> El {
    let done = matches!(n.kind, NodeKind::Item { task: crate::blocks::Task::Done, .. });
    let bx = crate::page::r(b * 0.9);
    let lh = crate::page::line(b, 1.45);
    let hit = target.max(bx);
    let (bx_x, bx_y) = (pad_x, (lh - bx) / 2.0);
    let (vcx, vcy) = (bx_x + bx / 2.0, lh / 2.0);
    let x0 = (vcx - hit / 2.0).max(-sh / 2.0).min(bx_x);
    let x1 = (vcx + hit / 2.0).min(pad_x + room).max(bx_x + bx);
    let y0 = (vcy - hit / 2.0).max(clear[0] - p.y).min(bx_y);
    let y1 = (vcy + hit / 2.0).min(clear[1] - p.y).max(bx_y + bx);
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

#[allow(clippy::too_many_arguments)]
fn fold(t: &Tree, l: &Laid, i: usize, folded: bool, editing: bool, lite: bool, hit: f32, c: &Roles, b: f32, mono: rune::Family) -> Vec<El> {
    let Some(p) = l.at[i] else { return Vec::new() };
    let n = &t.nodes[i];
    let tail = key_tail(&node_key(t, i)).to_string();
    let branch = branch_color(c, n.branch);
    // An expanded node's ring sits beside the middle of its right side; a folded node keeps its
    // target at the junction, where its `+N` pill is.
    let (cx, cy) = if folded { (p.right(), p.line_y()) } else { let [x, y] = map_app::fold_dot(&p); (x, y) };
    let clear = l.clear[i];
    // Never let the fold hit box reach far over the node's own label text, nor cross into
    // adjacent rows, the next column's task boxes, or a folded node's `+N` pill.
    let x0 = (cx - hit / 2.0).max(p.right() - FOLD_R);
    let x1 = if folded {
        (x0 + hit).min(p.right() + 5.0).max(cx)
    } else {
        (x0 + hit).min(p.right() + l.metrics.sh / 2.0).max(cx + FOLD_DOT_R)
    };
    let y0 = (cy - hit / 2.0).max(clear[0]).min(cy - FOLD_DOT_R);
    let y1 = (cy + hit / 2.0).min(clear[1]).max(cy + FOLD_DOT_R);
    let toggle = move |s: &mut App, tx: &mut Tx| {
        let all = tx.mods().cmd;
        s.map_toggle(i, all, tx);
        s.keys_to_note(tx);
    };
    let mut out = Vec::new();
    // The ring the branches leave from, at rest too; hovering adds its `−` and a halo.
    if !folded {
        out.push(
            el().key(format!("map-dot:{tail}"))
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
            .key(format!("map-fold:{tail}"))
            .pin(Some(x0 + W), Some(y0 + W), None, None)
            .size(x1 - x0, y1 - y0)
            .role("button")
            .label(if folded { "Unfold" } else { "Fold" })
            .cursor(rune::Cursor::Pointer)
            .on_press_tx(toggle);
        if !folded && !lite {
            let (fw, fh) = (x1 - x0, y1 - y0);
            let (rcx, rcy) = (cx - x0, cy - y0);
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (fw.to_bits(), fh.to_bits(), rcx.to_bits(), rcy.to_bits(), color_bits(branch)).hash(&mut hash);
            let vis_key = format!("map-fold-v:{tail}");
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
        out.push(hit_el);
    }
    if folded {
        let hidden = n.end - i - 1;
        let font = map_app::chip_font(b, mono);
        let (pill, [x, y, _, h]) = map_app::pill(hidden, p, font);
        let pill_hover = crate::roles::mix(c.on_page(c.chip_fill), c.text, 0.08);
        out.push(
            text(pill)
                .key(format!("map-pill:{tail}"))
                .font(font)
                .color(Roles::readable_on(c.dim, c.on_page(c.chip_fill), 4.8))
                .fill(c.chip_fill)
                .hover(pill_hover)
                .hover_motion(rune::Curve::linear(0.1), rune::Curve::linear(0.1))
                .border(1.0, c.chip_edge)
                .radius(h / 2.0)
                .px(map_app::PILL_PAD)
                .h(h)
                .role("button")
                .label(format!("Unfold, {hidden} hidden"))
                .cursor(rune::Cursor::Pointer)
                .pin(Some(x + W), Some(y + W), None, None)
                .on_press_tx(toggle),
        );
    }
    out
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
    let j = at.node();
    let Some(p) = l.at[j] else { return el() };
    let accent = Roles::readable_on(c.accent, c.bg, GRAPHIC);
    let z = zoom.max(map_app::ZOOM_MIN);
    let (zone, e) = match at {
        Add::Before(_) | Add::After(_) => {
            let desired = (2.5 / z).clamp(3.0, 20.0);
            let clear = l.clear[j];
            let (name, bar_h, y) = if matches!(at, Add::Before(_)) {
                if clear[0].is_finite() {
                    let half_gap = (p.y - clear[0]).max(0.0);
                    let bar_h = desired.min((2.0 * half_gap - 2.0).max(2.5));
                    let cy = p.y - half_gap.min(bar_h / 2.0 + 3.0);
                    ("Before", bar_h, cy - bar_h / 2.0)
                } else {
                    ("Before", desired, p.y - desired - 2.0)
                }
            } else if clear[1].is_finite() {
                let half_gap = (clear[1] - p.bottom()).max(0.0);
                let bar_h = desired.min((2.0 * half_gap - 2.0).max(2.5));
                let cy = p.bottom() + half_gap.min(bar_h / 2.0 + 3.0);
                ("After", bar_h, cy - bar_h / 2.0)
            } else {
                ("After", desired, p.bottom() + 2.0)
            };
            (name, el().pin(Some(p.x + W), Some(y + W), None, None).size(p.w, bar_h).radius(bar_h / 2.0).fill(accent))
        }
        Add::Child(_) => {
            let bw = (2.0 / z).clamp(2.0, 16.0);
            let pad = 3.0 + (bw - 2.0);
            let wash = accent.alpha(if c.bg.r < 0.2 { 0.18 } else { 0.14 });
            let stub = el().pin(Some(p.w + 2.0 * pad), Some(p.line_y() - p.y + pad - bw / 2.0), None, None).size(24.0, bw).fill(accent);
            let e = el()
                .pin(Some(p.x - pad + W), Some(p.y - pad + W), None, None)
                .size(p.w + 2.0 * pad, p.bottom() - p.y + 2.0 * pad)
                .radius(4.0 + (bw - 2.0))
                .fill(wash)
                .border(bw, accent)
                .child(stub);
            ("Child", e)
        }
    };
    e.key("map-drop").label(format!("{zone} {}", node_key(t, j))).inert()
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
