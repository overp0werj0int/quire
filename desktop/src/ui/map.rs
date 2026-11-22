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
use crate::map_app::{self, FOLD_R, Tier};
use crate::map_layout::{Laid, Place, line_width};
use crate::page;
use crate::roles::Roles;
use crate::theme::normal_lh;

/// Half the world element's side. The world is centred on the canvas's top-left, so its live
/// offset is the camera's pan and nodes sit at their world coordinates plus this.
const W: f32 = 131_072.0;
/// The drag ghost's offset from the pointer, in screen points (`ui::sidebar::tree`'s).
const GHOST_DX: f32 = 14.0;
const GHOST_DY: f32 = 10.0;

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
/// dim lifted to read on the canvas and on cards, where Preview's dim text, such as a quote's,
/// can fall short (MAP-074).
fn map_look(v: V) -> Look {
    let mut look = look_at(v, page::r(v.s.settings().size as f32 * 30.0));
    look.c.dim = look.c.readable_dim();
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
            && let Some((_, size)) = card::card(&cx, bl, i, "")
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
        let (w, h) = rune::text::measure(&rich.text, &rich.spans, st, Some(max_w));
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
    let (nodes, links) = s.map_mounted(cam);
    let sel = s.map_selected();
    let view = s.map_view();
    let folded = |i: usize| view.is_some_and(|v| v.folded.contains(&t.nodes[i].id));
    let (b, prose, mono) = (look.b, look.family, look.mono);
    // A fold circle's or task box's target is at least 24 points on screen (MAP-041, MAP-073).
    let target = 24.0 / 2f32.powi(map_app::hit_bucket(cam.1));
    let hit = target.max(2.0 * FOLD_R);
    let mut world = el()
        .key("map-world")
        .pin(Some(-W), Some(-W), None, None)
        .size(2.0 * W, 2.0 * W)
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
    for &i in &links {
        world = world.child(link(&t, &l, i, c).opacity(dim(i)));
    }
    let editing = s.map_editing();
    let query = s.find_query();
    let cur = query.and_then(|_| s.map_hits().get(s.reader.find.cur).copied());
    for &i in &nodes {
        let found = query.map(|q| Found { q, cur: cur.filter(|h| h.node == i).map(|h| (h.pid, h.k)) });
        let st = State { folded: folded(i), selected: i == sel, editing: editing == Some(i), alpha: dim(i), lite: cam.1 < map_app::LITE_ZOOM, target, found };
        world = world.child(node(&cx, &t, &l, i, st));
        if !t.nodes[i].children.is_empty() {
            world = world.children(fold(&t, &l, i, folded(i), hit, c, b, mono).into_iter().map(|e| e.opacity(dim(i))));
        }
    }
    // The field over the node being edited: its place, room left of the text, style and size.
    let field_at = editing.and_then(|i| {
        let (e, p) = (s.map.inline.as_ref()?, l.at[i]?);
        let room = map_app::task_room(&t.nodes[i], b);
        let st = map_app::style(map_app::tier(&t, i), b, prose, mono);
        Some((i, p, room, st, map_app::field_size(e, p, room, st, l.metrics)))
    });
    if nodes.contains(&sel)
        && let Some(p) = l.at[sel]
    {
        let [w, h] = match field_at {
            Some((i, _, room, _, [fw, fh])) if i == sel => [p.w.max(fw + room + 2.0 * l.metrics.pad_x), p.h.max(fh + l.metrics.pad_b)],
            _ => [p.w, p.h],
        };
        world = world.child(
            el().key("map-selection")
                .pin(Some(p.x - 3.0 + W), Some(p.y - 3.0 + W), None, None)
                .size(w + 6.0, h + line_width(p.depth) + 6.0)
                .radius(4.0)
                .border(2.0, Roles::readable_on(c.accent, c.bg, GRAPHIC))
                .inert(),
        );
    }
    if let Some((_, p, room, st, [w, _])) = field_at {
        world = world.child(field(v, p, room, st, w, l.metrics.pad_x));
    }
    if let Some(g) = drag {
        if let Some(at) = g.drop {
            world = world.child(drop_mark(&t, &l, at, c));
        }
        if let Some(m) = t.find(g.node)
            && let Some(p) = l.at[m]
        {
            let ([px, py], z) = cam;
            // Below and right of the pointer, as the tree's ghost sits, so the target under the
            // pointer stays readable.
            let at = [(g.at[0] - px + GHOST_DX) / z, (g.at[1] - py + GHOST_DY) / z];
            world = world.child(ghost(&cx, &t, m, p, at, l.metrics.pad_x));
        }
    }
    if nodes.contains(&ROOT)
        && let Some(p) = l.at[ROOT]
    {
        let font = map_app::chip_font(b, mono);
        if t.props > 0 {
            let (badge, [x, y, ..]) = map_app::badge(t.props, p, l.metrics.pad_x, font);
            world = world.child(
                text(badge)
                    .key("map-props")
                    .font(font)
                    .color(c.dim)
                    .role("button")
                    .cursor(rune::Cursor::Pointer)
                    .on_press_tx(|s: &mut App, tx: &mut Tx| s.map_props(tx))
                    .pin(Some(x + W), Some(y + W), None, None),
            );
        }
        if t.nodes.len() == 1 {
            let y = p.bottom() + 6.0 + if t.props > 0 { font.size * 1.6 } else { 0.0 };
            world = world.child(
                text("Press Tab to add a branch, or F2 to name the root")
                    .key("map-hint")
                    .font(styles::sans(12.0, 400.0))
                    .color(c.dim)
                    .pin(Some(p.x + l.metrics.pad_x + W), Some(y + W), None, None),
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
        .role("tree")
        .label("Map")
        .child(world)
        .child(ring.unwrap_or_else(el))
        .on_layout_tx(|s: &mut App, r, _| s.map_laid_out(r))
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
fn field(v: V, p: Place, room: f32, st: rune::TextStyle, w: f32, pad_x: f32) -> El {
    let Some(e) = &v.s.map.inline else { return el() };
    let c = Roles::new(v.p);
    let surface = c.on_page(c.raised);
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
        .pin(Some(p.x + pad_x + room + W), Some(p.y + W), None, None)
        .w(w)
        .radius(3.0)
        .fill(surface);
    if !e.multi {
        f = f.single_line();
    }
    f
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
    /// A task box's least target, in map points.
    target: f32,
    found: Option<Found<'a>>,
}

/// Find's highlights over a label's text (MAP-066): the find colour, the current match solid.
fn label_marks(c: &Roles, text: &str, f: Found) -> Vec<TextHighlight> {
    let cur = f.cur.filter(|(pid, _)| pid.is_none()).map(|(_, k)| k);
    let mark = |(k, r)| if Some(k) == cur { TextHighlight::new(r, c.find_current).color(c.find_current_text).key(crate::reader::FIND_CUR) } else { TextHighlight::new(r, c.find) };
    crate::writer_app::find_in(text, f.q).into_iter().enumerate().map(mark).collect()
}

fn node(cx: &Cx, t: &Tree, l: &Laid, i: usize, st: State) -> El {
    let State { folded, selected, editing, alpha, lite, target, found } = st;
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
    let copied = matches!(&n.body, Body::Card(bl) if super::preview::code::copied(cx, bl.lines.start));
    let (pad_x, pad_b) = (l.metrics.pad_x, l.metrics.pad_b);
    let clear = l.clear[i];
    let sh = l.metrics.sh;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // The palette and the names a wiki link resolves against colour the runs.
    (i, n.src, n.depth, folded, selected, editing, alpha.to_bits(), b.to_bits(), room.to_bits(), [p.x, p.y, p.w, p.h].map(f32::to_bits), color_bits(color), color_bits(branch)).hash(&mut h);
    let pics = if n.pictures() { cx.v.s.images.epoch.get() } else { 0 };
    (std::ptr::from_ref(cx.v.p) as usize, cx.v.s.names_gen, cx.look.family, cx.look.mono, copied, pics, lite, target.to_bits(), clear.map(f32::to_bits), sh.to_bits(), found).hash(&mut h);
    rune::memo(key.clone(), h.finish(), move || {
        let lw = line_width(p.depth);
        let st = map_app::style(tier, b, cx.look.family, cx.look.mono);
        let hl = found.map(|f| Hl::new(f.q, f.cur.and_then(|(pid, k)| Some((pid?, k)))));
        let cx = &Cx::map(cx.v, cx.look, hl.as_ref());
        let tail = key_tail(&key).to_string();
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
            .children((room > 0.0).then(|| task_box(n, i, p, b, pad_x, room, sh, target, clear, c)))
            .children((!editing).then(|| {
                let shown = match &n.body {
                    Body::Card(_) if lite => Some(card::lite(c, p.w - 2.0 * pad_x - room, p.h - pad_b, &tail)),
                    Body::Card(bl) => card::card(cx, bl, i, &tail).map(|(e, _)| e),
                    _ => None,
                };
                let text = || {
                    let mut r = label(cx, n, st, color);
                    if let Some(f) = found {
                        r.marks = label_marks(c, &r.text, f);
                    }
                    r.el(st, color, None).w(p.w - 2.0 * pad_x - room).clip_soft(0.0)
                };
                shown.unwrap_or_else(text).pin(Some(pad_x + room), Some(0.0), None, None)
            }))
            .child(el().pin(Some(0.0), Some(p.h), None, None).size(p.w, lw).fill(branch).inert())
    })
}

/// A task's box left of its label, centred on the first line, in a target `target` map points
/// clamped to its non-overlapping row slot and clear of its label text (MAP-053, MAP-073).
#[allow(clippy::too_many_arguments)]
fn task_box(n: &crate::map::Node, i: usize, p: Place, b: f32, pad_x: f32, room: f32, sh: f32, target: f32, clear: [f32; 2], c: &Roles) -> El {
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
    let mut bx_el = el().size(bx, bx).radius(rad).pin(Some(bx_x - x0), Some(bx_y - y0), None, None);
    bx_el = if done { bx_el.center().fill(c.accent).child(rune::icon(&super::icons::CHECK, (bx * 0.78).round()).color(c.bg)) } else { bx_el.border(edge, c.faint) };
    el().key(format!("map-task:{}", n.own.start + 1))
        .size(x1 - x0, y1 - y0)
        .role("checkbox")
        .checked(done)
        .label(if done { "Done" } else { "To do" })
        .cursor(rune::Cursor::Pointer)
        .on_press_tx(move |s: &mut App, tx: &mut Tx| s.map_task_click(i, tx))
        .pin(Some(x0), Some(y0), None, None)
        .child(bx_el)
}

#[allow(clippy::too_many_arguments)]
fn fold(t: &Tree, l: &Laid, i: usize, folded: bool, hit: f32, c: &Roles, b: f32, mono: rune::Family) -> Vec<El> {
    let Some(p) = l.at[i] else { return Vec::new() };
    let n = &t.nodes[i];
    let tail = key_tail(&node_key(t, i)).to_string();
    let branch = branch_color(c, n.branch);
    let (cx, cy) = (p.right(), p.line_y());
    let clear = l.clear[i];
    // Never let the fold hit box invade left of the fold dot over the node's own label text,
    // nor cross into adjacent rows or the next column's task boxes.
    let x0 = (cx - hit / 2.0).max(p.right() - FOLD_R);
    let x1 = (x0 + hit).min(p.right() + l.metrics.sh / 2.0).max(cx + FOLD_R);
    let y0 = (cy - hit / 2.0).max(clear[0]).min(cy - FOLD_R);
    let y1 = (cy + hit / 2.0).min(clear[1]).max(cy + FOLD_R);
    let toggle = move |s: &mut App, tx: &mut Tx| {
        let all = tx.mods().cmd;
        s.map_toggle(i, all, tx);
        s.keys_to_note(tx);
    };
    let dot = el()
        .size(2.0 * FOLD_R, 2.0 * FOLD_R)
        .radius(FOLD_R)
        .border(1.5, branch)
        .fill(if folded { branch } else { c.bg })
        .pin(Some(cx - FOLD_R - x0), Some(cy - FOLD_R - y0), None, None);
    let mut out = vec![
        el().key(format!("map-fold:{tail}"))
            .pin(Some(x0 + W), Some(y0 + W), None, None)
            .size(x1 - x0, y1 - y0)
            .role("button")
            .label(if folded { "Unfold" } else { "Fold" })
            .cursor(rune::Cursor::Pointer)
            .on_press_tx(toggle)
            .child(dot),
    ];
    if folded {
        let hidden = n.end - i - 1;
        let font = map_app::chip_font(b, mono);
        let (pill, [x, y, _, h]) = map_app::pill(hidden, p, font);
        out.push(
            text(pill)
                .key(format!("map-pill:{tail}"))
                .font(font)
                .color(Roles::readable_on(c.dim, c.on_page(c.chip_fill), 4.8))
                .fill(c.chip_fill)
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

/// The node a drag carries, at the pointer: a translucent copy with a count of what goes
/// with it (MAP-026).
fn ghost(cx: &Cx, t: &Tree, i: usize, p: Place, at: [f32; 2], pad_x: f32) -> El {
    let (c, b, mono) = (&cx.look.c, cx.look.b, cx.look.mono);
    let n = &t.nodes[i];
    let st = map_app::style(map_app::tier(t, i), b, cx.look.family, mono);
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
            label(cx, n, st, c.text)
                .el(st, c.text, None)
                .pin(Some(pad_x), Some(0.0), None, None)
                .w((p.w - 2.0 * pad_x).max(1.0))
                .clip_soft(0.0),
        )
        .children((below > 0).then(|| {
            text(format!("+{below}"))
                .font(styles::note(mono, small, normal_lh(mono, small)))
                .color(Roles::readable_on(c.dim, c.on_page(c.chip_fill), 4.8))
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
    let accent = Roles::readable_on(c.accent, c.bg, GRAPHIC);
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
