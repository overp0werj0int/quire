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
    let drawn = s.map_drawn();
    let l = drawn.laid.clone();
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
    // The subtree a drag carries dims where it was (MAP-026).
    let drag = s.map.node_drag.as_ref().filter(|g| g.active);
    let moved = drag.and_then(|g| t.find(g.node)).map_or(0..0, |m| m..t.nodes[m].end);
    let dim = |i: usize| drawn.alpha(i) * if moved.contains(&i) { 0.35 } else { 1.0 };
    for &i in &links {
        world = world.child(link(&t, &l, i, &c).opacity(dim(i)));
    }
    let editing = s.map_editing();
    for &i in &nodes {
        world = world.child(node(&t, &l, i, folded(i), i == sel, editing == Some(i), dim(i), &c, b, prose, mono));
        if !t.nodes[i].children.is_empty() {
            world = world.children(fold(&t, &l, i, folded(i), hit, &c, b, mono).into_iter().map(|e| e.opacity(dim(i))));
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
    if let Some(i) = editing
        && let Some(p) = l.at[i]
    {
        let room = map_app::task_room(&t.nodes[i], b);
        world = world.child(field(v, p, room, map_app::style(map_app::tier(&t, i), b, prose, mono), l.metrics));
    }
    if let Some(g) = drag {
        if let Some(at) = g.drop {
            world = world.child(drop_mark(&t, &l, at, &c));
        }
        if let Some(m) = t.find(g.node)
            && let Some(p) = l.at[m]
        {
            let ([px, py], z) = cam;
            let at = [(g.at[0] - px) / z - g.grab[0], (g.at[1] - py) / z - g.grab[1]];
            world = world.child(ghost(&t, m, p, at, &c, b, prose, mono, l.metrics.pad_x));
        }
    }
    if nodes.contains(&ROOT)
        && let Some(p) = l.at[ROOT]
    {
        let below = p.bottom() + 6.0;
        let small = (b * 0.72).round();
        if t.props > 0 {
            // Right-aligned under the root: its links leave from its right edge, and nothing
            // is left of it, so the badge never meets one.
            let badge = format!("Properties · {}", t.props);
            let font = styles::note(mono, small, normal_lh(mono, small));
            let w = rune::text::measure(&badge, &[], font, None).0.ceil();
            world = world.child(
                text(badge)
                    .key("map-props")
                    .font(font)
                    .color(c.faint)
                    .role("button")
                    .cursor(rune::Cursor::Pointer)
                    .on_press_tx(|s: &mut App, tx: &mut Tx| s.map_props(tx))
                    .pin(Some(p.right() - l.metrics.pad_x - w + W), Some(below + W), None, None),
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
        .on_layout_tx(|s: &mut App, r, _| s.map_laid_out(r))
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
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_pan_drag(d, tx))
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

/// The inline field over the node being edited, in the label's place and style (MAP-016).
fn field(v: V, p: Place, room: f32, st: rune::TextStyle, m: crate::map_layout::Metrics) -> El {
    let Some(e) = &v.s.map.inline else { return el() };
    let shown = if e.edit.text.is_empty() { "M" } else { e.edit.text.as_str() };
    let w = rune::text::measure(shown, &[], st, Some(m.max_w)).0.ceil() + 2.0;
    let mut f = rune::text_input(&e.edit, st)
        .key(map_app::FIELD)
        .label("Node text")
        .color(v.p.ink)
        .caret(v.p.ink, v.p.sel)
        .on_edit(|s: &mut App| match &mut s.map.inline {
            Some(e) => &mut e.edit,
            None => &mut s.map.spare,
        })
        .on_input_key(|s: &mut App, ev: &rune::KeyEvent, tx: &mut Tx| s.map_field_key(ev, tx))
        .pin(Some(p.x + m.pad_x + room + W), Some(p.y + W), None, None)
        .w(w.max(p.w - 2.0 * m.pad_x - room).min(m.max_w))
        .fill(Roles::new(v.p).bg);
    if !e.multi {
        f = f.single_line();
    }
    f
}

#[allow(clippy::too_many_arguments)]
fn node(t: &Tree, l: &Laid, i: usize, folded: bool, selected: bool, editing: bool, alpha: f32, c: &Roles, b: f32, prose: rune::Family, mono: rune::Family) -> El {
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
    let room = map_app::task_room(n, b);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (i, &n.label, n.depth, folded, selected, editing, alpha.to_bits(), b.to_bits(), room.to_bits(), [p.x, p.y, p.w, p.h].map(f32::to_bits), color_bits(color), color_bits(branch)).hash(&mut h);
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
            .focus_selected(selected)
            .opacity(alpha);
        if !n.children.is_empty() {
            e = e.expanded(!folded);
        }
        e.on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, false, tx))
            .on_double_press_tx(move |s: &mut App, tx: &mut Tx| s.map_click(i, true, tx))
            .on_secondary_press(move |s: &mut App, at, tx| s.map_node_menu(i, Some((at.x, at.y)), tx))
            .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| s.map_node_drag(i, d, tx))
            .children((room > 0.0).then(|| task_box(n, i, b, pad_x, c)))
            .children((!editing).then(|| {
                text(label)
                    .font(map_app::style(tier, b, prose, mono))
                    .color(color)
                    .wrap()
                    .pin(Some(pad_x + room), Some(0.0), None, None)
                    .w(p.w - 2.0 * pad_x - room)
                    .clip_soft(0.0)
            }))
            .child(el().pin(Some(0.0), Some(p.h), None, None).size(p.w, lw).fill(branch).inert())
    })
}

/// A task's box left of its label, in a 24-point target, centred on the first line (MAP-053).
fn task_box(n: &crate::map::Node, i: usize, b: f32, pad_x: f32, c: &Roles) -> El {
    let done = matches!(n.kind, NodeKind::Item { task: crate::blocks::Task::Done, .. });
    let bx = crate::page::r(b * 0.9);
    let lh = crate::page::line(b, 1.45);
    let hit = 24f32.max(bx);
    let (rad, edge) = (2f32.max(crate::page::r(bx * 0.24)), 1f32.max(crate::page::r(b / 12.0)));
    let mut bx_el = el().size(bx, bx).radius(rad);
    bx_el = if done { bx_el.center().fill(c.accent).child(rune::icon(&super::icons::CHECK, (bx * 0.78).round()).color(c.bg)) } else { bx_el.border(edge, c.faint) };
    el().key(format!("map-task:{}", n.own.start + 1))
        .size(hit, hit)
        .center()
        .role("checkbox")
        .checked(done)
        .label(if done { "Done" } else { "To do" })
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_task_click(i, tx))
        .pin(Some(pad_x + (bx - hit) / 2.0), Some((lh - hit) / 2.0), None, None)
        .child(bx_el)
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

/// The node a drag carries, at the pointer: a translucent copy with a count of what goes
/// with it (MAP-026).
#[allow(clippy::too_many_arguments)]
fn ghost(t: &Tree, i: usize, p: Place, at: [f32; 2], c: &Roles, b: f32, prose: rune::Family, mono: rune::Family, pad_x: f32) -> El {
    let n = &t.nodes[i];
    let small = (b * 0.72).round();
    let pill_h = small + 6.0;
    let below = n.end - i - 1;
    el().key("map-ghost")
        .pin(Some(at[0] + W), Some(at[1] + W), None, None)
        .size(p.w, p.h)
        .radius(4.0)
        .fill(c.bg)
        .border(1.0, branch_color(c, n.branch))
        .opacity(0.8)
        .child(
            text(map_app::shown_label(n))
                .font(map_app::style(map_app::tier(t, i), b, prose, mono))
                .color(c.text)
                .wrap()
                .pin(Some(pad_x), Some(0.0), None, None)
                .w((p.w - 2.0 * pad_x).max(1.0))
                .clip_soft(0.0),
        )
        .children((below > 0).then(|| {
            text(format!("+{below}"))
                .font(styles::note(mono, small, normal_lh(mono, small)))
                .color(c.readable_dim())
                .fill(c.chip_fill)
                .border(1.0, c.chip_edge)
                .radius(pill_h / 2.0)
                .px(5.0)
                .h(pill_h)
                .pin(Some(p.w + 6.0), Some((p.h - pill_h) / 2.0), None, None)
        }))
        .inert()
}

/// Where a drop would put the node: an accent bar above or below the target for Before and
/// After, or a box round it with a stub of a branch for Child (MAP-026). Its label names the
/// zone and the target's key.
fn drop_mark(t: &Tree, l: &Laid, at: crate::map::Add, c: &Roles) -> El {
    use crate::map::Add;
    let j = at.node();
    let Some(p) = l.at[j] else { return el() };
    let accent = Roles::readable_on(c.accent, c.bg, 3.0);
    let (zone, e) = match at {
        Add::Before(_) | Add::After(_) => {
            let y = if matches!(at, Add::Before(_)) { p.y - 4.0 } else { p.bottom() + 1.0 };
            let name = if matches!(at, Add::Before(_)) { "Before" } else { "After" };
            (name, el().pin(Some(p.x + W), Some(y + W), None, None).size(p.w, 3.0).radius(1.5).fill(accent))
        }
        Add::Child(_) => {
            let stub = el().pin(Some(p.w + 6.0), Some(p.line_y() - p.y + 2.0), None, None).size(24.0, 2.0).fill(accent);
            let e = el().pin(Some(p.x - 3.0 + W), Some(p.y - 3.0 + W), None, None).size(p.w + 6.0, p.bottom() - p.y + 6.0).radius(4.0).border(2.0, accent).child(stub);
            ("Child", e)
        }
    };
    e.key("map-drop").label(format!("{zone} {}", node_key(t, j))).inert()
}

fn color_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// Where node `i` shows in the canvas's own points, for tests of the camera.
#[allow(dead_code)]
pub fn on_canvas(p: Place, cam: ([f32; 2], f32)) -> [f32; 2] {
    [cam.0[0] + p.x * cam.1, cam.0[1] + p.y * cam.1]
}
