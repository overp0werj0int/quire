//! The map canvas (MAP-032 to MAP-045, MAP-055, MAP-075): the note's tree on a canvas that
//! pans and zooms without bounds. One world element carries the camera as live values; every
//! node, link and fold circle sits in it at its world place. Only what meets the canvas and
//! its overscan is built.

use std::hash::{Hash, Hasher};

use rune::{Color, Drag, El, LineCap, MagnifyEvent, Path, PointerButton, Shape, Tx, WheelEvent, el, text};

use super::{V, styles};
use crate::app::App;
use crate::map::{NodeKind, ROOT, Tree};
use crate::map_app::{self, Tier};
use crate::map_layout::{Laid, Place, line_width};
use crate::roles::Roles;
use crate::theme::normal_lh;

/// Half the world element's side. The world is centred on the canvas's top-left, so its live
/// offset is the camera's pan and nodes sit at their world coordinates plus this.
const W: f32 = 131_072.0;
const FOLD_R: f32 = 6.0;

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

/// A branch's colour on the page: the accent at the root, else the palette, lifted to 3:1
/// (MAP-040).
fn branch_color(c: &Roles, branch: usize) -> Color {
    if branch == 0 {
        return Roles::readable_on(c.accent, c.bg, 3.0);
    }
    let pal = [c.blue, c.green, c.orange, c.magenta, c.cyan, c.yellow];
    Roles::readable_on(pal[(branch - 1) % pal.len()], c.bg, 3.0)
}

/// What a screen reader hears for node `i` (MAP-075).
fn spoken(t: &Tree, i: usize, folded: bool) -> String {
    let n = &t.nodes[i];
    let mut s = format!("{}, level {}", map_app::shown_label(n), n.depth + 1);
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
    let c = Roles::new(v.p);
    let t = s.map_tree();
    let l = s.map_laid();
    let cam = s.map_camera();
    let (nodes, links) = s.map_mounted(cam);
    let sel = s.map_selected();
    let view = s.map_view();
    let folded = |i: usize| view.is_some_and(|v| v.folded.contains(&t.nodes[i].id));
    let b = s.settings().size as f32;
    let set = s.settings();
    let prose = crate::theme::pick(&crate::theme::PROSE_FONTS, &set.preview_font).family;
    let mono = crate::theme::pick(&crate::theme::CODE_FONTS, &set.code_font).family;
    // A fold circle's target is at least 24 points on screen (MAP-041).
    let hit = (24.0 / 2f32.powi(map_app::hit_bucket(cam.1))).max(2.0 * FOLD_R);
    let mut world = el().key("map-world").pin(Some(-W), Some(-W), None, None).size(2.0 * W, 2.0 * W);
    if let Some(v) = view {
        world = world.live_offset(&v.live_pan).live_scale(&v.live_zoom);
    }
    for &i in &links {
        world = world.child(link(&t, &l, i, &c));
    }
    for &i in &nodes {
        world = world.child(node(&t, &l, i, folded(i), i == sel, &c, b, prose, mono));
        if !t.nodes[i].children.is_empty() {
            world = world.children(fold(&t, &l, i, folded(i), hit, &c, b, mono));
        }
    }
    if nodes.contains(&sel)
        && let Some(p) = l.at[sel]
    {
        world = world.child(
            el().key("map-selection")
                .pin(Some(p.x - 3.0 + W), Some(p.y - 3.0 + W), None, None)
                .size(p.w + 6.0, p.h + line_width(p.depth) + 6.0)
                .radius(4.0)
                .border(2.0, c.accent)
                .inert(),
        );
    }
    if nodes.contains(&ROOT)
        && let Some(p) = l.at[ROOT]
    {
        let below = p.bottom() + 6.0;
        let small = (b * 0.72).round();
        if t.props > 0 {
            world = world.child(
                text(format!("Properties · {}", t.props))
                    .key("map-props")
                    .font(styles::note(mono, small, normal_lh(mono, small)))
                    .color(c.faint)
                    .pin(Some(p.x + l.metrics.pad_x + W), Some(below + W), None, None),
            );
        }
        if t.nodes.len() == 1 {
            let y = below + if t.props > 0 { small * 1.6 } else { 0.0 };
            world = world.child(
                text("Press Tab to add a branch, or F2 to name the root")
                    .key("map-hint")
                    .font(styles::sans(12.0, 400.0))
                    .color(c.faint)
                    .pin(Some(p.x + l.metrics.pad_x + W), Some(y + W), None, None),
            );
        }
    }
    el().key("map-canvas")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .w_full()
        .fill(c.bg)
        .clip_soft(0.0)
        .role("tree")
        .label("Map")
        .child(world)
        .on_layout_tx(|s: &mut App, [_, _, w, h], _| s.map_laid_out([w, h]))
        .on_wheel(|s: &mut App, e: WheelEvent, tx: &mut Tx| {
            // The window sends Ctrl + wheel as a pinch (`on_magnify`); a wheel that still holds
            // Ctrl zooms the same way.
            if e.mods.cmd {
                s.map_zoom_about(e.x, e.y, 1.0 - e.dy / 200.0, tx);
                return true;
            }
            let ([px, py], z) = s.map_camera();
            let (dx, dy) = if e.mods.shift && e.dx == 0.0 { (e.dy, 0.0) } else { (e.dx, e.dy) };
            s.map_move([px + dx, py + dy], z, tx);
            true
        })
        .on_magnify(|s: &mut App, e: MagnifyEvent, tx: &mut Tx| {
            s.map_zoom_about(e.x, e.y, 1.0 + e.delta, tx);
            true
        })
        .pointer_buttons([PointerButton::Middle])
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| {
            let (pan, z) = s.map_camera();
            match d.phase {
                rune::Phase::Down => {
                    s.map.drag_from = Some(pan);
                    tx.live_only();
                }
                rune::Phase::Move => {
                    if let Some([x, y]) = s.map.drag_from {
                        s.map_move([x + d.dx, y + d.dy], z, tx);
                    }
                }
                rune::Phase::Up => {
                    if let Some(from) = s.map.drag_from.take()
                        && d.cancelled()
                    {
                        s.map_move(from, z, tx);
                    }
                }
            }
        })
}

fn link(t: &Tree, l: &Laid, i: usize, c: &Roles) -> El {
    let (Some(p), Some(q)) = (l.at[i], t.nodes[i].parent.and_then(|q| l.at[q])) else { return el() };
    let lw = line_width(p.depth);
    let (x0, x1) = (q.right(), p.x);
    let (ya, yb) = (q.line_y(), p.line_y());
    let top = ya.min(yb) - lw;
    let (w, h) = ((x1 - x0).max(1.0), (ya.max(yb) - top + lw).max(1.0));
    let mid = w / 2.0;
    let (a, z) = ([0.0, ya - top], [w, yb - top]);
    let path = Path::new().move_to(a).cubic_to([mid, a[1]], [mid, z[1]], z);
    rune::canvas(Shape::new().stroke_path(&path, lw, LineCap::Round, branch_color(c, t.nodes[i].branch)))
        .key(format!("map-link:{}", key_tail(&node_key(t, i))))
        .pin(Some(x0 + W), Some(top + W), None, None)
        .size(w, h)
        .inert()
}

#[allow(clippy::too_many_arguments)]
fn node(t: &Tree, l: &Laid, i: usize, folded: bool, selected: bool, c: &Roles, b: f32, prose: rune::Family, mono: rune::Family) -> El {
    let Some(p) = l.at[i] else { return el() };
    let n = &t.nodes[i];
    let tier = map_app::tier(t, i);
    let color = match (tier, &n.kind) {
        (Tier::Root | Tier::Head1 | Tier::Head, _) => c.strong,
        (_, NodeKind::Item { task: crate::blocks::Task::Done, .. }) => c.dim,
        (Tier::Card, _) => c.dim,
        _ => c.text,
    };
    let branch = branch_color(c, n.branch);
    let key = node_key(t, i);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&n.label, n.depth, folded, selected, b.to_bits(), [p.x, p.y, p.w, p.h].map(f32::to_bits), color_bits(color), color_bits(branch)).hash(&mut h);
    let pad_x = l.metrics.pad_x;
    rune::memo(key.clone(), h.finish(), move || {
        let lw = line_width(p.depth);
        let label = map_app::shown_label(n);
        let mut e = el().key(key)
            .pin(Some(p.x + W), Some(p.y + W), None, None)
            .size(p.w, p.h + lw)
            .role("treeitem")
            .label(spoken(t, i, folded))
            .level(n.depth as u32 + 1)
            .focus_selected(selected);
        if !n.children.is_empty() {
            e = e.expanded(!folded);
        }
        e.on_press_tx(move |s: &mut App, tx: &mut Tx| {
                s.map_click(i);
                s.keys_to_note(tx);
            })
            .child(
                text(label)
                    .font(map_app::style(tier, b, prose, mono))
                    .color(color)
                    .wrap()
                    .pin(Some(pad_x), Some(0.0), None, None)
                    .w(p.w - 2.0 * pad_x)
                    .clip_soft(0.0),
            )
            .child(el().pin(Some(0.0), Some(p.h), None, None).size(p.w, lw).fill(branch).inert())
    })
}

#[allow(clippy::too_many_arguments)]
fn fold(t: &Tree, l: &Laid, i: usize, folded: bool, hit: f32, c: &Roles, b: f32, mono: rune::Family) -> Vec<El> {
    let Some(p) = l.at[i] else { return Vec::new() };
    let n = &t.nodes[i];
    let tail = key_tail(&node_key(t, i)).to_string();
    let branch = branch_color(c, n.branch);
    let (cx, cy) = (p.right(), p.line_y());
    let toggle = move |s: &mut App, tx: &mut Tx| {
        let all = tx.mods().cmd;
        s.map_toggle(i, all, tx);
        s.keys_to_note(tx);
    };
    let dot = el().size(2.0 * FOLD_R, 2.0 * FOLD_R).radius(FOLD_R).border(1.5, branch).fill(if folded { branch } else { c.bg });
    let mut out = vec![
        el().key(format!("map-fold:{tail}"))
            .pin(Some(cx - hit / 2.0 + W), Some(cy - hit / 2.0 + W), None, None)
            .size(hit, hit)
            .center()
            .role("button")
            .label(if folded { "Unfold" } else { "Fold" })
            .on_press_tx(toggle)
            .child(dot),
    ];
    if folded {
        let size = (b * 0.72).round();
        let pill_h = size + 6.0;
        out.push(
            text(format!("+{}", n.end - i - 1))
                .key(format!("map-pill:{tail}"))
                .font(styles::note(mono, size, normal_lh(mono, size)))
                .color(c.readable_dim())
                .fill(c.chip_fill)
                .border(1.0, c.chip_edge)
                .radius(pill_h / 2.0)
                .px(5.0)
                .h(pill_h)
                .pin(Some(cx + FOLD_R + 6.0 + W), Some(cy - pill_h / 2.0 + W), None, None)
                .on_press_tx(toggle),
        );
    }
    out
}

fn color_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// Where node `i` shows in the canvas's own points, for tests of the camera.
#[allow(dead_code)]
pub fn on_canvas(p: Place, cam: ([f32; 2], f32)) -> [f32; 2] {
    [cam.0[0] + p.x * cam.1, cam.0[1] + p.y * cam.1]
}
