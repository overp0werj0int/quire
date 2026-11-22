//! Map mode on `App`: each tab's camera, folds and selection, the tree and layout of the note
//! shown, and the actions the map's keys and pointer run (decision 0014, `behavior/map.md`).
//!
//! The camera is `pan + world × zoom` in the canvas's own points. Each tab keeps it twice: as
//! plain numbers, which the view reads to choose what to mount, and as the `Live` values the
//! world element draws with, so a pan or zoom that mounts nothing new runs no view.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::Duration;

use rune::{Family, Live, TextEdit, TextStyle, Tx};

use crate::app::{App, Mode};
use crate::blocks::Link;
use crate::map::{self, Body, NodeId, NodeKind, ROOT, Tree};
use crate::map_layout::{self, Laid, Metrics};
use crate::page;

pub const ZOOM_MIN: f32 = 0.1;
pub const ZOOM_MAX: f32 = 4.0;
/// How far past the canvas's edges nodes stay mounted (MAP-045).
pub const OVERSCAN: f32 = 160.0;
/// Below this zoom a card draws as its bare box (MAP-045).
pub const LITE_ZOOM: f32 = 0.35;
/// What `ensureVisible` keeps between a node and the canvas's edges (MAP-036).
const MARGIN: f32 = 40.0;
/// Camera glides (MAP-043).
const GLIDE: f64 = 0.2;
/// Room right of a node for its fold circle and count pill, in world points.
pub const FOLD_ROOM: f32 = 64.0;
/// A fold circle's radius, at a branching node's right anchor (MAP-041).
pub const FOLD_R: f32 = 6.0;

/// One tab's map: kept for the session, dropped with the tab (MAP-044).
pub struct MapView {
    pub pan: [f32; 2],
    pub zoom: f32,
    pub live_pan: Live<[f32; 2]>,
    pub live_zoom: Live<f32>,
    pub folded: HashSet<NodeId>,
    /// Bumped on every fold change, so the layout runs again.
    pub fold_ver: u64,
    pub selected: NodeId,
    /// The camera has been placed once (MAP-065).
    pub placed: bool,
}

impl MapView {
    /// After change `s` turned map `old` into `t`: the folds and the selection follow their
    /// nodes, the selection else to the node at the change (MAP-010, MAP-068).
    fn follow(&mut self, old: &Tree, t: &Tree, s: &map::Splice) {
        let shut = |id: &NodeId| t.find(*id).is_some_and(|i| !t.nodes[i].children.is_empty());
        self.folded = std::mem::take(&mut self.folded).into_iter().filter_map(|id| map::follow(old, t, s, id)).filter(shut).collect();
        self.fold_ver += 1;
        self.selected = map::follow(old, t, s, self.selected).unwrap_or_else(|| t.nodes[t.at_line(s.lines.start)].id);
    }
}

pub(crate) struct Glide {
    from: ([f32; 2], f32),
    to: ([f32; 2], f32),
    start: f64,
}

/// The typography tier of a node (MAP-038).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    Root,
    Head1,
    Head,
    Body,
    Card,
}

/// A node's source hash (`Node::src`), tier, body size, wrap width and, for pictures, the
/// images epoch.
type SizeKey = (u64, Tier, u32, u32, u64);
/// The fold version is None before the tab has a view: a new view's first folds, from the
/// note's fold comments, are version 0 and need a layout of their own.
type LaidKey = (String, u64, Option<u64>, u32, u64, &'static str, &'static str, u64);
/// Label sizes, kept while the font epoch and the two fonts stay the same.
type Sizes = (u64, &'static str, &'static str, HashMap<SizeKey, [f32; 2]>);

pub struct MapState {
    pub views: HashMap<String, MapView>,
    /// The linear mode the map goes back to (MAP-057).
    pub back: Mode,
    /// The canvas's size as last laid out, 0 before it has been.
    pub size: Cell<[f32; 2]>,
    /// The canvas's top-left corner in the window.
    pub(crate) origin: Cell<[f32; 2]>,
    tree: RefCell<Option<(u64, Rc<Tree>)>>,
    laid: RefCell<Option<(LaidKey, Rc<Laid>)>>,
    sizes: RefCell<Sizes>,
    pub(crate) glide: Option<Glide>,
    /// A touchpad swipe's coast after the fingers lift (`map_throw`).
    pub(crate) throw: crate::map_throw::Throw,
    pub(crate) relayout: Option<crate::map_motion::Relayout>,
    pub(crate) frame: RefCell<Option<crate::map_motion::Frame>>,
    /// The pan when the canvas's drag began.
    pub drag_from: Option<[f32; 2]>,
    pub node_drag: Option<crate::map_drag::NodeDrag>,
    /// A drag just dropped, so the press its release makes is not a click.
    pub dropped: bool,
    pub inline: Option<Inline>,
    /// What the field edits for the moment between its edit ending and its view going.
    pub spare: TextEdit,
    /// True while keyboard focus sits on the map canvas (`note-keys` or `map-inline-edit`).
    pub canvas_focused: bool,
    /// Pan saved before nudging the selected node clear of an overlay drawer.
    pub(crate) drawer_return_pan: Option<[f32; 2]>,
}

/// The field over a node while its text is edited in place (MAP-016 to MAP-019, MAP-021).
pub struct Inline {
    pub rel: String,
    /// The note's text when the edit began: a commit splices into this text only.
    base: String,
    /// `base`'s map, whose indices the edit names.
    tree: Rc<Tree>,
    what: Editing,
    pub edit: TextEdit,
    /// Shift+Enter starts a new line, rather than committing.
    pub multi: bool,
    /// The note's text with the new node in it, and that text's hash: the map shows it until
    /// the first commit, and the file never holds it.
    preview: Option<(u64, String)>,
    /// The node selected before a new one, which Esc goes back to.
    anchor: NodeId,
    /// The field has had focus, so losing it commits (MAP-017).
    held: bool,
    /// Camera saved when auto-zooming from overview zoom (< 0.85) on `Editing::Node`, restored
    /// on Enter commit or Esc cancel unless the user manually moved the camera.
    edit_return_cam: Option<([f32; 2], f32)>,
    /// Hash of `edit.text` last checked for live field visibility.
    last_hash: u64,
}

impl Inline {
    /// The change the field's text makes to `base`, or None when it makes none (MAP-018).
    fn splice(&self) -> Option<map::Splice> {
        match self.what {
            Editing::Node(i) => map::rename(&self.base, &self.tree, i, &self.edit.text),
            Editing::New(at) => map::add(&self.base, &self.tree, at, &self.edit.text).ok(),
        }
    }
}

#[derive(Clone, Copy)]
enum Editing {
    Node(usize),
    New(map::Add),
}

impl Default for MapState {
    fn default() -> Self {
        MapState {
            views: HashMap::new(),
            back: Mode::Source,
            size: Cell::new([0.0, 0.0]),
            origin: Cell::new([0.0, 0.0]),
            tree: RefCell::new(None),
            laid: RefCell::new(None),
            sizes: RefCell::new((0, "", "", HashMap::new())),
            glide: None,
            throw: Default::default(),
            relayout: None,
            frame: RefCell::new(None),
            drag_from: None,
            node_drag: None,
            dropped: false,
            inline: None,
            spare: TextEdit::new(""),
            canvas_focused: true,
            drawer_return_pan: None,
        }
    }
}

pub fn tier(t: &Tree, i: usize) -> Tier {
    let n = &t.nodes[i];
    match &n.kind {
        _ if i == ROOT => Tier::Root,
        NodeKind::Heading { .. } if n.depth == 1 => Tier::Head1,
        NodeKind::Heading { .. } | NodeKind::Footnotes => Tier::Head,
        _ if n.card() => Tier::Card,
        _ => Tier::Body,
    }
}

/// A tier's text style at body size `b`.
pub fn style(tier: Tier, b: f32, prose: Family, mono: Family) -> TextStyle {
    let (size, weight, lead, family) = match tier {
        Tier::Root => (page::step(b, 1.5), 700.0, 1.3, prose),
        Tier::Head1 => (page::step(b, 1.0), 700.0, 1.3, prose),
        Tier::Head => (b, 700.0, 1.35, prose),
        Tier::Body => (b, 400.0, 1.45, prose),
        Tier::Card => (page::code_size(b), 400.0, 1.45, mono),
    };
    TextStyle::new(size, weight).family(family).lh(page::line(size, lead))
}

/// The room a task's box takes left of its label at body size `b` (MAP-053): the box and a gap.
pub fn task_room(n: &map::Node, b: f32) -> f32 {
    match n.kind {
        NodeKind::Item { task: crate::blocks::Task::Todo | crate::blocks::Task::Done, .. } if !n.card() => page::r(b * 0.9) + page::r(b * 0.45),
        _ => 0.0,
    }
}

/// The text of the Properties badge and the `+N` pills at body size `b`.
pub fn chip_font(b: f32, mono: Family) -> TextStyle {
    let size = (b * 0.72).round();
    crate::ui::styles::note(mono, size, crate::theme::normal_lh(mono, size))
}

/// The Properties badge under the root's box `p` (MAP-006): its text and `[x, y, w, h]`.
/// Right-aligned under the root: its links leave from its right edge, and nothing is left of
/// it, so the badge never meets one.
pub fn badge(props: usize, p: map_layout::Place, pad_x: f32, font: TextStyle) -> (String, [f32; 4]) {
    let text = format!("Properties · {props}");
    let w = rune::text::measure(&text, &[], font, None).0.ceil();
    (text, [p.right() - pad_x - w, p.bottom() + 6.0, w, font.line_height])
}

/// The `+N` pill right of a folded node's circle, for box `p` (MAP-041): its text and
/// `[x, y, w, h]`.
pub fn pill(hidden: usize, p: map_layout::Place, font: TextStyle) -> (String, [f32; 4]) {
    let text = format!("+{hidden}");
    let h = font.size + 6.0;
    let w = rune::text::measure(&text, &[], font, None).0.ceil() + 2.0 * PILL_PAD;
    (text, [p.right() + FOLD_R + 6.0, p.line_y() - h / 2.0, w, h])
}

/// A pill's padding either side of its text.
pub const PILL_PAD: f32 = 5.0;

/// The inline field's size for its text: as wide as the text, at least the label's width and
/// at most a node's widest, and as tall as the text wraps to at that width (breaking inside
/// unbreakable tokens wider than `max_w`).
pub fn field_size(e: &Inline, p: map_layout::Place, room: f32, st: TextStyle, m: Metrics) -> [f32; 2] {
    let shown = if e.edit.text.is_empty() { "M" } else { e.edit.text.as_str() };
    let anywhere = rune::text::RichLayoutOptions::new().wrap_anywhere();
    let w = (rune::text::measure_with(shown, &[], st, Some(m.max_w), anywhere).0.ceil() + 2.0)
        .max(p.w - 2.0 * m.pad_x - room)
        .min(m.max_w);
    [w, rune::text::measure_with(shown, &[], st, e.multi.then_some(w), anywhere).1.ceil()]
}

fn edit_hash(e: &TextEdit) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (e.text.as_str(), e.focus).hash(&mut h);
    h.finish()
}

/// Minimum zoom at which inline editing is legible without auto-zooming to 1.0.
const EDIT_ZOOM_MIN: f32 = 0.85;

/// What a node shows: an ordered item's number before its text.
pub fn shown_label(n: &map::Node) -> String {
    match &n.kind {
        NodeKind::Item { marker, ordered: true, .. } if !n.card() => format!("{marker} {}", n.label),
        _ => n.label.clone(),
    }
}

impl App {
    fn fonts(&self) -> (Family, Family, &'static str, &'static str) {
        let set = self.settings();
        let prose = crate::theme::pick(&crate::theme::PROSE_FONTS, &set.preview_font);
        let mono = crate::theme::pick(&crate::theme::CODE_FONTS, &set.code_font);
        (prose.family, mono.family, prose.id, mono.id)
    }

    /// The note shown, as a map tree, built once per text: with a new node in it while one is
    /// being named.
    pub fn map_tree(&self) -> Rc<Tree> {
        let name = self.active.as_deref().unwrap_or("");
        let preview = self.map.inline.as_ref().filter(|e| e.rel == name).and_then(|e| e.preview.as_ref());
        let (doc, ver) = match preview {
            Some((h, text)) => (Rc::new(crate::blocks::build(text)), *h),
            None => (self.doc(), self.doc_hash().unwrap_or(0)),
        };
        if let Some((v, t)) = &*self.map.tree.borrow()
            && *v == ver
        {
            return t.clone();
        }
        let text = preview.map_or(self.active_text(), |p| p.1.as_str());
        let t = Rc::new(map::build(&doc, text, name));
        *self.map.tree.borrow_mut() = Some((ver, t.clone()));
        t
    }

    /// The version of the text `map_tree` was built from.
    fn map_ver(&self) -> u64 {
        self.map.tree.borrow().as_ref().map_or(0, |(v, _)| *v)
    }

    /// The active tab's map, or None with no note shown.
    pub fn map_view(&self) -> Option<&MapView> {
        self.map.views.get(self.active.as_deref()?)
    }

    /// The active tab's map, made with the note's opening folds on first use.
    pub fn map_view_mut(&mut self) -> Option<&mut MapView> {
        let a = self.active.clone()?;
        if !self.map.views.contains_key(&a) {
            let t = self.map_tree();
            let v = MapView { pan: [0.0, 0.0], zoom: 1.0, live_pan: Live::new([0.0, 0.0]), live_zoom: Live::new(1.0), folded: t.initial_folds().into_iter().collect(), fold_ver: 0, selected: t.root().id, placed: false };
            self.map.views.insert(a.clone(), v);
        }
        self.map.views.get_mut(&a)
    }

    /// The selected node's index: the root when the selection is gone.
    pub fn map_selected(&self) -> usize {
        let t = self.map_tree();
        self.map_view().and_then(|v| t.find(v.selected)).unwrap_or(ROOT)
    }

    /// The first line of the nearest heading of level `max` or above that holds the selected
    /// node, or is it: what the outline marks on the map (MAP-067). While a provisional new node
    /// is being named, resolve from the base tree so Outline stays on the real owning heading.
    pub fn map_heading_line(&self, max: usize) -> Option<usize> {
        let name = self.active.as_deref().unwrap_or("");
        let (t, mut i) = match self.map.inline.as_ref().filter(|e| e.rel == name && e.preview.is_some()) {
            Some(e) => {
                let start = match e.what {
                    Editing::New(map::Add::Child(j)) => Some(j),
                    Editing::New(map::Add::Before(j) | map::Add::After(j)) => e.tree.nodes[j].parent.or(Some(j)),
                    Editing::Node(j) => Some(j),
                };
                (e.tree.clone(), start)
            }
            None => (self.map_tree(), Some(self.map_selected())),
        };
        while let Some(k) = i {
            if matches!(t.nodes[k].kind, NodeKind::Heading { level, .. } if level <= max) {
                return Some(t.nodes[k].own.start);
            }
            i = t.nodes[k].parent;
        }
        None
    }

    /// The layout of the note shown, run again only when the text, the folds, the text size or
    /// the fonts change (MAP-045).
    pub fn map_laid(&self) -> Rc<Laid> {
        let t = self.map_tree();
        let b = self.settings().size as f32;
        let (_, mono, prose_id, mono_id) = self.fonts();
        let epoch = rune::text::font_epoch();
        let empty = HashSet::new();
        let (folded, fold_ver) = self.map_view().map_or((&empty, None), |v| (&v.folded, Some(v.fold_ver)));
        let pics = self.images.epoch.get();
        let key: LaidKey = (self.active.clone().unwrap_or_default(), self.map_ver(), fold_ver, b.to_bits(), epoch, prose_id, mono_id, pics);
        if let Some((k, l)) = &*self.map.laid.borrow()
            && *k == key
        {
            return l.clone();
        }
        let m = Metrics::new(b, &t.opts);
        let mut sizes = self.map.sizes.borrow_mut();
        if (sizes.0, sizes.1, sizes.2) != (epoch, prose_id, mono_id) {
            *sizes = (epoch, prose_id, mono_id, HashMap::new());
        }
        let cache = &mut sizes.3;
        let sizer = crate::ui::MapSizer::new(self);
        let chip = chip_font(b, mono);
        let laid = Rc::new(crate::map_layout::layout(
            &t,
            folded,
            m,
            |i| {
                let n = &t.nodes[i];
                let key = (n.src, tier(&t, i), b.to_bits(), m.max_w.to_bits(), if n.pictures() { pics } else { 0 });
                let [w, h] = *cache.entry(key).or_insert_with(|| sizer.size(&t, i, m.max_w));
                [w + task_room(n, b), h]
            },
            |i| {
                let hidden = t.nodes[i].end - i - 1;
                let text = format!("+{hidden}");
                let pw = rune::text::measure(&text, &[], chip, None).0.ceil() + 2.0 * PILL_PAD;
                let half_h = ((chip.size + 6.0) / 2.0).max(FOLD_R);
                [FOLD_R + 6.0 + pw + 4.0, half_h]
            },
        ));
        *self.map.laid.borrow_mut() = Some((key, laid.clone()));
        laid
    }

    /// The canvas's size, or a guess from the window before it has been laid out.
    pub fn map_size(&self) -> [f32; 2] {
        match self.map.size.get() {
            [w, h] if w > 0.0 && h > 0.0 => [w, h],
            _ => [self.view_w.get().max(1.0), self.view_h.get().max(1.0)],
        }
    }

    /// The camera the view draws with.
    pub fn map_camera(&self) -> ([f32; 2], f32) {
        self.map_view().map_or(([0.0, 0.0], 1.0), |v| (v.pan, v.zoom))
    }

    /// The nodes and links the view mounts at camera `cam`: whatever meets the canvas and its
    /// overscan. Links are by child.
    pub fn map_mounted(&self, cam: ([f32; 2], f32)) -> (Vec<usize>, Vec<usize>) {
        let t = self.map_tree();
        let l = self.map_drawn().laid.clone();
        let ([px, py], z) = cam;
        let [w, h] = self.map_size();
        let (x0, y0, x1, y1) = ((-px - OVERSCAN) / z, (-py - OVERSCAN) / z, (w - px + OVERSCAN) / z, (h - py + OVERSCAN) / z);
        let meets = |a0: f32, b0: f32, a1: f32, b1: f32| a1 >= x0 && a0 <= x1 && b1 >= y0 && b0 <= y1;
        let mut nodes = Vec::new();
        let mut links = Vec::new();
        for &i in &l.shown {
            let Some(p) = l.at[i] else { continue };
            if meets(p.x, p.y, p.right() + FOLD_ROOM, p.bottom()) {
                nodes.push(i);
            }
            if let Some(q) = t.nodes[i].parent.and_then(|q| l.at[q])
                && meets(q.right(), p.line_y().min(q.line_y()), p.x, p.line_y().max(q.line_y()))
            {
                links.push(i);
            }
        }
        (nodes, links)
    }

    fn map_signature(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let cam = self.map_camera();
        self.map_mounted(cam).hash(&mut h);
        hit_bucket(cam.1).hash(&mut h);
        (cam.1 < LITE_ZOOM).hash(&mut h);
        h.finish()
    }

    /// Moves the camera at once, running no view unless what is mounted changes.
    pub fn map_move(&mut self, pan: [f32; 2], zoom: f32, tx: &mut Tx) {
        let before = self.map_signature();
        self.map.glide = None;
        self.map.throw.stop();
        self.map.drawer_return_pan = None;
        if let Some(e) = &mut self.map.inline {
            e.edit_return_cam = None;
        }
        self.set_camera(pan, zoom);
        if self.map_signature() == before {
            tx.live_only();
        }
    }

    pub(crate) fn set_camera(&mut self, pan: [f32; 2], zoom: f32) {
        let zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        if let Some(v) = self.map_view_mut() {
            v.pan = pan;
            v.zoom = zoom;
            v.live_pan.set(pan);
            v.live_zoom.set(zoom);
        }
    }

    /// Glides the camera to `to` over 200 ms, or jumps under reduced motion (MAP-043).
    fn map_glide(&mut self, to: ([f32; 2], f32), tx: &mut Tx) {
        let to = (to.0, to.1.clamp(ZOOM_MIN, ZOOM_MAX));
        let from = self.map_camera();
        self.map.throw.stop();
        if crate::motion::reduced() || from == to {
            self.set_camera(to.0, to.1);
            self.map.glide = None;
            return;
        }
        self.map.glide = Some(Glide { from, to, start: self.now.get() });
        tx.wake_after(Duration::from_millis(16));
    }

    /// Before each frame: a camera glide's next step. True when it mounts something new.
    pub(crate) fn poll_map(&mut self, t: f64, tx: &mut Tx) -> bool {
        let mut committed = self.place_due() | self.poll_field(tx) | self.poll_drag(t, tx) | self.poll_relayout(t, tx);
        let node_menu = matches!(self.menu(), Some(m) if matches!(m.of, crate::layer::MenuOf::Node(_)));
        let focused = node_menu || tx.focused().is_none_or(|k| k == crate::focus::NOTE || k == FIELD);
        if self.map.canvas_focused != focused {
            self.map.canvas_focused = focused;
            committed = true;
        }
        if let Some(([dx, dy], running)) = self.map.throw.step(t) {
            let before = self.map_signature();
            let ([px, py], z) = self.map_camera();
            self.set_camera([px + dx, py + dy], z);
            if running {
                tx.wake_after(Duration::from_millis(16));
            }
            committed |= self.map_signature() != before;
        }
        let Some(g) = &self.map.glide else { return committed };
        let u = ((t - g.start) / GLIDE).clamp(0.0, 1.0) as f32;
        let e = 1.0 - (1.0 - u).powi(3);
        let (from, to) = (g.from, g.to);
        let mix = |a: f32, b: f32| a + (b - a) * e;
        let before = self.map_signature();
        self.set_camera([mix(from.0[0], to.0[0]), mix(from.0[1], to.0[1])], mix(from.1, to.1));
        if u < 1.0 {
            tx.wake_after(Duration::from_millis(16));
        } else {
            self.map.glide = None;
        }
        committed || self.map_signature() != before
    }

    /// A wheel or touchpad scroll over the canvas pans the camera by its deltas; the fingers'
    /// lift throws it on (`map_throw`).
    pub fn map_wheel(&mut self, e: &rune::WheelEvent, tx: &mut Tx) {
        let t = e.time.unwrap_or_else(|| tx.clock());
        let (dx, dy) = if e.mods.shift && e.dx == 0.0 { (e.dy, 0.0) } else { (e.dx, e.dy) };
        if e.phase == rune::GesturePhase::Ended && (dx, dy) == (0.0, 0.0) {
            if self.map.throw.released(t, tx.clock(), self.map_size()) {
                tx.wake_after(Duration::from_millis(0));
            }
            return;
        }
        let ([px, py], z) = self.map_camera();
        self.map_move([px + dx, py + dy], z, tx);
        if e.precise {
            self.map.throw.moved((dx, dy), t);
        } else {
            self.map.throw.stepped();
        }
    }

    /// Scales the camera by `f` about canvas point `(x, y)`: the world point under it stays.
    pub fn map_zoom_about(&mut self, x: f32, y: f32, f: f32, tx: &mut Tx) {
        let ([px, py], z) = self.map_camera();
        let nz = (z * f).clamp(ZOOM_MIN, ZOOM_MAX);
        let (wx, wy) = ((x - px) / z, (y - py) / z);
        self.map_move([x - wx * nz, y - wy * nz], nz, tx);
    }

    /// `+`, `=` and `-`: about the canvas's centre (MAP-034).
    pub fn map_zoom_step(&mut self, f: f32, tx: &mut Tx) {
        let [w, h] = self.map_size();
        self.map_zoom_about(w / 2.0, h / 2.0, f, tx);
    }

    /// What the map draws, `[x0, y0, x1, y1]`: every shown node with its fold circle, its
    /// `+N` pill when folded, and the Properties badge.
    fn map_extent(&self) -> [f32; 4] {
        let t = self.map_tree();
        let l = self.map_laid();
        let font = chip_font(self.settings().size as f32, self.fonts().1);
        let folded = |i: usize| self.map_view().is_some_and(|v| v.folded.contains(&t.nodes[i].id));
        let mut boxes = Vec::new();
        for &i in l.shown.iter().filter(|&&i| !t.nodes[i].children.is_empty()) {
            let Some(p) = l.at[i] else { continue };
            boxes.push([p.right() - FOLD_R, p.line_y() - FOLD_R, 2.0 * FOLD_R, 2.0 * FOLD_R]);
            if folded(i) {
                boxes.push(pill(t.nodes[i].end - i - 1, p, font).1);
            }
        }
        if t.props > 0
            && let Some(p) = l.at[ROOT]
        {
            boxes.push(badge(t.props, p, l.metrics.pad_x, font).1);
        }
        boxes.into_iter().fold(l.bounds, |[x0, y0, x1, y1], [x, y, w, h]| [x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h)])
    }

    /// The part of the canvas that an open overlay drawer, find bar and foot pill leave clear,
    /// `[x0, y0, x1, y1]` (MAP-035).
    fn map_free(&self) -> [f32; 4] {
        let [w, h] = self.map_size();
        let left = if self.drawer() == Some(crate::app::Drawer::Sidebar) && !self.focus && self.view_w.get() < crate::app::SIDEBAR_DOCK_W {
            crate::ui::SIDEBAR_W
        } else {
            0.0
        };
        let right = if self.drawer() == Some(crate::app::Drawer::Outline) && !self.focus && self.view_w.get() < crate::app::OUTLINE_DOCK_W {
            crate::ui::OUTLINE_W
        } else {
            0.0
        };
        let top = if self.reader.find.open { crate::ui::reader::find_slot_h_for_width(w) } else { 0.0 };
        let bottom = if self.foot_shows() { self.reader.reveal.band } else { 0.0 };
        [left.min(w / 2.0), top.min(h / 2.0), (w - right).max(w / 2.0), (h - bottom).max(h / 2.0)]
    }

    /// When an overlay sidebar or outline drawer opens over the map at narrow widths, nudge the
    /// selected node clear of the drawer so it stays readable beside it.
    pub(crate) fn map_drawer_opened(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Map || self.active_locked() {
            return;
        }
        let sel = self.map_selected();
        let l = self.map_laid();
        let Some(p) = l.at[sel] else { return };
        let ([px, py], z) = self.map.glide.as_ref().map_or_else(|| self.map_camera(), |g| g.to);
        let [fx0, _, fx1, _] = self.map_free();
        let pad = 16.0;
        let dx = if fx0 > 0.0 && px + p.x * z < fx0 + pad {
            fx0 + pad - (px + p.x * z)
        } else if fx1 < self.map_size()[0] && px + p.right() * z > fx1 - pad {
            let room = (fx1 - fx0 - 2.0 * pad).max(0.0);
            if p.w * z <= room {
                fx1 - pad - (px + p.right() * z)
            } else {
                fx0 + pad - (px + p.x * z)
            }
        } else {
            0.0
        };
        if dx != 0.0 {
            self.map.drawer_return_pan.get_or_insert([px, py]);
            self.map_glide(([px + dx, py], z), tx);
        }
    }

    /// When an overlay drawer closes, restore the camera pan if we nudged it clear of the drawer.
    pub(crate) fn map_drawer_closed(&mut self, tx: &mut Tx) {
        let Some(pan) = self.map.drawer_return_pan.take() else { return };
        if self.mode == Mode::Map && !self.active_locked() {
            let z = self.map.glide.as_ref().map_or_else(|| self.map_camera().1, |g| g.to.1);
            self.map_glide((pan, z), tx);
        }
    }

    /// The camera that fits everything the map draws (MAP-035).
    fn fit_camera(&self) -> ([f32; 2], f32) {
        let [x0, y0, x1, y1] = self.map_extent();
        let [fx0, fy0, fx1, fy1] = self.map_free();
        let (w, h) = (fx1 - fx0, fy1 - fy0);
        let (bw, bh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let z = (w / bw * 0.95).min(h / bh * 0.95).min(2.0).clamp(ZOOM_MIN, ZOOM_MAX);
        ([fx0 + w / 2.0 - (x0 + bw / 2.0) * z, fy0 + h / 2.0 - (y0 + bh / 2.0) * z], z)
    }

    pub fn map_fit(&mut self, tx: &mut Tx) {
        let cam = self.fit_camera();
        self.map_glide(cam, tx);
    }

    /// The camera at zoom `z` with node `i`'s centre at the centre of the canvas's free part.
    fn centred_on(&self, i: usize, z: f32) -> ([f32; 2], f32) {
        let l = self.map_laid();
        let [x0, y0, x1, y1] = self.map_free();
        let Some(p) = l.at[i] else { return self.map_camera() };
        ([(x0 + x1) / 2.0 - (p.x + p.w / 2.0) * z, (y0 + y1) / 2.0 - (p.y + p.h / 2.0) * z], z)
    }

    /// The camera at zoom `z` with node `i` anchored near the left edge of the free canvas and
    /// vertically centred, so left-to-right branches fill the viewport (MAP-065).
    fn left_anchored_on(&self, i: usize, z: f32) -> ([f32; 2], f32) {
        let l = self.map_laid();
        let [fx0, fy0, fx1, fy1] = self.map_free();
        let Some(p) = l.at[i] else { return self.map_camera() };
        let (fw, fh) = (fx1 - fx0, fy1 - fy0);
        let inset = (fw * 0.07).clamp(32.0, 56.0);
        ([fx0 + inset - p.x * z, fy0 + fh / 2.0 - (p.y + p.h / 2.0) * z], z)
    }

    /// `c`: the selected node to the middle at the current zoom (MAP-036).
    pub fn map_center(&mut self, tx: &mut Tx) {
        let cam = self.centred_on(self.map_selected(), self.map_camera().1);
        self.map_glide(cam, tx);
    }

    /// `Ctrl+0`: zoom 1.0 on the selected node (MAP-034).
    pub fn map_zoom_reset(&mut self, tx: &mut Tx) {
        let cam = self.centred_on(self.map_selected(), 1.0);
        self.map_glide(cam, tx);
    }

    /// Pans by the least that keeps node `i` 40 points inside the canvas's free part (MAP-036).
    fn ensure_visible(&mut self, i: usize, tx: &mut Tx) {
        let l = self.map_laid();
        let Some(p) = l.at[i] else { return };
        let ([px, py], z) = self.map_camera();
        let [x0, y0, x1, y1] = self.map_free();
        let least = |lo: f32, hi: f32, a: f32, b: f32| {
            if hi - lo > b - a - 2.0 * MARGIN || lo < a + MARGIN {
                a + MARGIN - lo
            } else if hi > b - MARGIN {
                b - MARGIN - hi
            } else {
                0.0
            }
        };
        let dx = least(px + p.x * z, px + p.right() * z, x0, x1);
        let dy = least(py + p.y * z, py + p.bottom() * z, y0, y1);
        if dx != 0.0 || dy != 0.0 {
            self.map_glide(([px + dx, py + dy], z), tx);
        }
    }

    fn map_select(&mut self, i: usize, tx: &mut Tx) {
        let id = self.map_tree().nodes[i].id;
        if let Some(v) = self.map_view_mut() {
            v.selected = id;
        }
        self.ensure_visible(i, tx);
    }

    /// Arrow and vi keys (MAP-056).
    pub fn map_parent(&mut self, tx: &mut Tx) {
        let i = self.map_selected();
        if let Some(p) = self.map_tree().nodes[i].parent {
            self.map_select(p, tx);
        }
    }

    pub fn map_child(&mut self, tx: &mut Tx) {
        let t = self.map_tree();
        let i = self.map_selected();
        let Some(&c) = t.nodes[i].children.first() else { return };
        let id = t.nodes[i].id;
        if let Some(v) = self.map_view_mut()
            && v.folded.remove(&id)
        {
            v.fold_ver += 1;
        }
        self.map_select(c, tx);
        self.map_relayout(None, tx);
    }

    /// The previous or next sibling, else the nearest shown node at the same depth that way.
    pub fn map_sibling(&mut self, down: bool, tx: &mut Tx) {
        let t = self.map_tree();
        let l = self.map_laid();
        let i = self.map_selected();
        let d = t.nodes[i].depth;
        let Some(at) = l.shown.iter().position(|&n| n == i) else { return };
        let found = if down { l.shown[at + 1..].iter().find(|&&n| t.nodes[n].depth == d) } else { l.shown[..at].iter().rev().find(|&&n| t.nodes[n].depth == d) };
        if let Some(&n) = found {
            self.map_select(n, tx);
        }
    }

    pub fn map_root(&mut self, tx: &mut Tx) {
        self.map_select(ROOT, tx);
    }

    /// Shifts the camera pan so node `anchor` stays at its pre-change screen position `(old_p)`,
    /// runs a shifted relayout glide around it, and only calls `ensure_visible` if the anchored
    /// node was completely outside the free viewport (MAP-043).
    fn finish_anchored_fold(&mut self, anchor: usize, old_p: Option<map_layout::Place>, tx: &mut Tx) {
        let new_p = self.map_laid().at[anchor];
        let shift = match (old_p, new_p) {
            (Some(a), Some(b)) => [b.right() - a.right(), b.line_y() - a.line_y()],
            _ => [0.0, 0.0],
        };
        if shift[0] != 0.0 || shift[1] != 0.0 {
            let ([px, py], z) = self.map_camera();
            self.set_camera([px - shift[0] * z, py - shift[1] * z], z);
            if let Some(g) = &mut self.map.glide {
                g.from.0[0] -= shift[0] * g.from.1;
                g.from.0[1] -= shift[1] * g.from.1;
                g.to.0[0] -= shift[0] * g.to.1;
                g.to.0[1] -= shift[1] * g.to.1;
            }
        }
        self.map_relayout_shifted(None, shift, tx);
        if let Some(p) = new_p {
            let ([px, py], z) = self.map_camera();
            let [fx0, fy0, fx1, fy1] = self.map_free();
            let clipped = px + p.x * z < fx0 || px + p.right() * z > fx1 || py + p.y * z < fy0 || py + p.bottom() * z > fy1;
            if clipped {
                self.ensure_visible(anchor, tx);
            }
        }
    }

    /// Folds or unfolds node `i`, or its whole branch with `all` (MAP-041, MAP-058).
    pub fn map_toggle(&mut self, i: usize, all: bool, tx: &mut Tx) {
        let Some(i) = self.map_press_node(i, tx) else { return };
        let t = self.map_tree();
        let n = &t.nodes[i];
        if n.children.is_empty() {
            return;
        }
        let old_p = self.map_drawn().laid.at[i];
        let Some(v) = self.map_view_mut() else { return };
        let fold = !v.folded.contains(&n.id);
        let ids: Vec<NodeId> = if all { t.nodes[i..n.end].iter().filter(|d| !d.children.is_empty()).map(|d| d.id).collect() } else { vec![n.id] };
        for id in ids {
            if fold {
                v.folded.insert(id);
            } else {
                v.folded.remove(&id);
            }
        }
        v.fold_ver += 1;
        self.keep_selection_shown();
        self.finish_anchored_fold(i, old_p, tx);
    }

    pub fn map_toggle_selected(&mut self, all: bool, tx: &mut Tx) {
        let i = self.map_selected();
        self.map_toggle(i, all, tx);
    }

    /// `]`: every fold opens (MAP-058).
    pub fn map_expand_all(&mut self, tx: &mut Tx) {
        let anchor = self.map_selected();
        let old_p = self.map_drawn().laid.at[anchor];
        if let Some(v) = self.map_view_mut()
            && !v.folded.is_empty()
        {
            v.folded.clear();
            v.fold_ver += 1;
        }
        self.finish_anchored_fold(anchor, old_p, tx);
    }

    /// `[`: folds every branch at depth 2 and below, or from depth 1 when those are folded
    /// already (MAP-058; depths are 1-based, the root at 1).
    pub fn map_collapse_to_level(&mut self, tx: &mut Tx) {
        let t = self.map_tree();
        let old_drawn = self.map_drawn();
        let branches = |from: usize| t.nodes.iter().filter(move |n| n.depth + 1 >= from && !n.children.is_empty()).map(|n| n.id);
        let Some(v) = self.map_view_mut() else { return };
        let from = if branches(2).all(|id| v.folded.contains(&id)) { 1 } else { 2 };
        v.folded.extend(branches(from));
        v.fold_ver += 1;
        self.keep_selection_shown();
        let anchor = self.map_selected();
        let old_p = old_drawn.laid.at[anchor];
        self.finish_anchored_fold(anchor, old_p, tx);
    }

    /// A fold that hides the selected node selects the fold instead (MAP-055).
    fn keep_selection_shown(&mut self) {
        let t = self.map_tree();
        let i = self.map_selected();
        let Some(v) = self.map_view_mut() else { return };
        let mut a = i;
        let mut shown = i;
        while let Some(p) = t.nodes[a].parent {
            if v.folded.contains(&t.nodes[p].id) {
                shown = p;
            }
            a = p;
        }
        v.selected = t.nodes[shown].id;
    }

    /// A node clicked on the canvas: an open field commits, then the node is selected, where
    /// it is. A double click edits it (MAP-016).
    pub fn map_click(&mut self, i: usize, double: bool, tx: &mut Tx) {
        if std::mem::take(&mut self.map.dropped) || self.map_editing() == Some(i) {
            return;
        }
        let Some(i) = self.map_press_node(i, tx) else { return };
        let id = self.map_tree().nodes[i].id;
        if let Some(v) = self.map_view_mut() {
            v.selected = id;
        }
        if double {
            self.map_edit(tx);
        } else {
            self.keys_to_note(tx);
        }
    }


    // ---------- The camera and the mode ----------

    /// The canvas laid out: its size, origin compensation when a docked panel or Zen bar shifts
    /// its left/top edge on an already-placed map, and the camera's first place once known.
    pub fn map_laid_out(&mut self, [x, y, w, h]: [f32; 4]) {
        let old_origin = self.map.origin.get();
        let old_size = self.map.size.get();
        let size = [w, h];
        self.map.origin.set([x, y]);
        self.map.size.set(size);
        if old_size[0] > 0.0 && (old_origin[0] != x || old_origin[1] != y) {
            let dx = old_origin[0] - x;
            let dy = old_origin[1] - y;
            if let Some(v) = self.map_view_mut()
                && v.placed
            {
                let pan = [v.pan[0] + dx, v.pan[1] + dy];
                let z = v.zoom;
                self.set_camera(pan, z);
                if let Some(g) = &mut self.map.glide {
                    g.from.0[0] += dx;
                    g.from.0[1] += dy;
                    g.to.0[0] += dx;
                    g.to.0[1] += dy;
                }
            }
        }
        self.place_due();
    }

    /// A tab shown on the map for the first time gets its first camera once the canvas has a
    /// size. The canvas reports its size only when it changes, so a note opened while the map
    /// shows is placed from `poll_map`.
    fn place_due(&mut self) -> bool {
        let due = self.mode == Mode::Map && self.map.size.get()[0] > 0.0 && !self.active_locked() && self.map_view_mut().is_some_and(|v| !v.placed);
        if due {
            self.place_camera();
        }
        due
    }

    /// A tab's first camera: the whole map when it fits at 0.7 or more, else 1.0 left-anchored
    /// on the selected node when it has children so left-to-right branches stay in view, or
    /// centred for a lone root or leaf target (MAP-065, MAP-071).
    fn place_camera(&mut self) {
        let fit = self.fit_camera();
        // An empty note's lone root opens at 100%, centred, not fitted up to 200% (MAP-071).
        let t = self.map_tree();
        let sel = self.map_selected();
        let alone = t.nodes.len() == 1;
        let leaf = t.nodes[sel].children.is_empty();
        let cam = if fit.1 >= 0.7 && !alone {
            fit
        } else if alone || leaf {
            self.centred_on(sel, 1.0)
        } else {
            self.left_anchored_on(sel, 1.0)
        };
        self.set_camera(cam.0, cam.1);
        if let Some(v) = self.map_view_mut() {
            v.placed = true;
        }
    }

    /// The selected node becomes the one holding source line `line`, its folds open, and
    /// the camera finds it (MAP-065).
    fn map_land(&mut self, line: usize, tx: &mut Tx) {
        let i = self.map_tree().at_line(line);
        let Some(placed) = self.map_open_to(i) else { return };
        self.map.glide = None;
        if !placed {
            if self.map.size.get()[0] > 0.0 {
                self.place_camera();
            }
        } else {
            self.ensure_visible(i, tx);
        }
    }

    /// The source line the mode being left is at: the caret's, or the preview's top block's.
    fn place_line_now(&self) -> usize {
        match self.mode {
            Mode::Source => {
                let text = self.active_text();
                let caret = self.active.as_deref().and_then(|a| self.edits.get(a)).map_or(0, |e| e.focus.min(text.len()));
                crate::md::line_col(text, caret).0
            }
            Mode::Preview => self.doc().blocks.get(self.top()).map_or(0, |b| b.lines.start),
            Mode::Map => {
                let t = self.map_tree();
                t.nodes[self.map_selected()].own.start
            }
        }
    }

    /// `Ctrl+M`: into the map, or back to the linear mode it came from (KEY-102).
    pub fn toggle_map(&mut self, tx: &mut Tx) {
        let to = if self.mode == Mode::Map { self.map.back } else { Mode::Map };
        self.show_mode(to, tx);
    }

    /// Shows the map, keeping the place (MAP-065).
    pub fn enter_map(&mut self, tx: &mut Tx) {
        if self.mode == Mode::Map {
            return;
        }
        tx.clear_text_selection();
        let line = self.place_line_now();
        self.map.back = self.mode;
        self.mode = Mode::Map;
        self.close_find(tx);
        self.keys_to_note(tx);
        if self.active_locked() {
            return;
        }
        self.map_land(line, tx);
    }

    /// Leaves the map for `to`, keeping the place (MAP-065).
    pub fn leave_map(&mut self, to: Mode, tx: &mut Tx) {
        if self.mode != Mode::Map || to == Mode::Map {
            return;
        }
        let line = if self.active_locked() { 0 } else { self.place_line_now() };
        self.mode = to;
        self.map.glide = None;
        if to == Mode::Source {
            self.source_jump(line, tx);
        } else {
            let doc = self.doc();
            let block = doc.blocks.iter().rposition(|b| b.lines.start <= line).unwrap_or(0);
            self.keys_to_note(tx);
            if block == 0 {
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, 0.0);
            } else {
                self.land(block, false, tx);
            }
        }
    }

    /// Esc on the map, after the layers (MAP-061): the notice, find, then `App::escape`'s
    /// list, then back to the linear mode.
    pub fn map_escape(&mut self, tx: &mut Tx) -> bool {
        if self.notice.take().is_some() {
            return true;
        }
        if self.reader.find.open {
            self.close_find(tx);
            return true;
        }
        if self.escape(tx) {
            return true;
        }
        self.leave_map(self.map.back, tx);
        self.persist_later();
        true
    }

    /// A tab closed: its map goes with it (MAP-044).
    pub fn drop_map(&mut self, path: &str) {
        self.map.views.remove(path);
    }
}

/// Zoom in powers of two, so the fold circles' 24-point screen target is sized per bucket
/// (MAP-041) and a zoom inside one bucket mounts the same elements.
pub fn hit_bucket(zoom: f32) -> i32 {
    zoom.log2().floor().clamp(-4.0, 2.0) as i32
}

// ---------- Edits: gestures that change the note (MAP-013 to MAP-025, MAP-053, MAP-059) ----------

/// The bytes `old` and `new` share at the start and at the end, on char boundaries.
fn shared(old: &str, new: &str) -> (usize, usize) {
    let mut pre = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(pre) || !new.is_char_boundary(pre) {
        pre -= 1;
    }
    let room = old.len().min(new.len()) - pre;
    let mut post = old.bytes().rev().zip(new.bytes().rev()).take(room).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(old.len() - post) || !new.is_char_boundary(new.len() - post) {
        post -= 1;
    }
    (pre, post)
}

impl App {
    /// The map's gestures change only an open, unlocked note (MAP-015).
    pub fn map_editable(&self) -> bool {
        self.mode == Mode::Map && self.active.is_some() && !self.active_locked()
    }

    /// Splice `s` of the note shown, whose map was `old`, goes in as one undo step (MAP-013);
    /// the folds follow their nodes and the node at the splice's focus line is selected.
    fn map_apply(&mut self, old: &Tree, s: &map::Splice, tx: &mut Tx) {
        self.map_apply_inner(old, s, false, tx);
    }

    fn map_apply_inner(&mut self, old: &Tree, s: &map::Splice, only_if_clipped: bool, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        let base = self.active_text().to_string();
        self.splice_into(&rel, &base, s);
        let t = self.map_follow(old, s);
        let i = s.focus.map_or(ROOT, |l| t.at_line(l));
        if self.map_open_to(i).is_some() {
            if !only_if_clipped {
                self.ensure_visible(i, tx);
            } else if let Some(p) = self.map_laid().at[i] {
                let ([px, py], z) = self.map_camera();
                let [fx0, fy0, fx1, fy1] = self.map_free();
                if px + p.x * z < fx0 || px + p.right() * z > fx1 || py + p.y * z < fy0 || py + p.bottom() * z > fy1 {
                    self.ensure_visible(i, tx);
                }
            }
        }
        self.map_relayout(Some((old, s)), tx);
    }

    /// A right-click on node `i`, or the Menu key or Shift+F10 on the selected node with `at`
    /// None (MAP-062): the node's menu, under it when opened from the keyboard.
    pub fn map_node_menu(&mut self, i: usize, at: Option<(f32, f32)>, tx: &mut Tx) {
        let Some(i) = self.map_press_node(i, tx) else { return };
        self.map_select(i, tx);
        let Some(p) = self.map_laid().at[i] else { return };
        let ([px, py], z) = self.map_camera();
        let [ox, oy] = self.map.origin.get();
        let under = (ox + px + p.x * z, oy + py + (p.bottom() + FOLD_R + 4.0) * z);
        self.open_menu(crate::layer::MenuOf::Node(self.map_tree().nodes[i].id), at.unwrap_or(under), tx);
        if at.is_none() {
            tx.focus("node-menu", true);
        }
    }

    /// A node menu's item picked: the node it was opened on is selected again, if it is still
    /// there, for the item to act on.
    pub fn map_pick(&mut self, id: NodeId, tx: &mut Tx) -> Option<usize> {
        let i = self.map_tree().find(id)?;
        self.map_select(i, tx);
        Some(i)
    }

    /// The Edit Sheet's save in Map mode: `lines` of the note shown become `with`, and the map
    /// keeps its place on the node the sheet began at (MAP-020).
    pub fn map_replace(&mut self, lines: std::ops::Range<usize>, with: &str, tx: &mut Tx) {
        let old = self.map_tree();
        let s = map::Splice { focus: Some(lines.start), lines, with: with.split('\n').map(str::to_string).collect(), moved: None };
        self.map_apply(&old, &s, tx);
    }

    /// After change `s` to the note shown, whose map was `old`: the folds and the selection
    /// follow their nodes into the new tree (MAP-010).
    fn map_follow(&mut self, old: &Tree, s: &map::Splice) -> Rc<Tree> {
        let t = self.map_tree();
        if let Some(v) = self.map_view_mut() {
            v.follow(old, &t, s);
        }
        t
    }

    /// Note `rel` changed outside the app from `old` (MAP-068): its map's folds and selection
    /// follow their nodes into the new text, and its camera stays. A field naming a new node
    /// keeps that node selected, and Esc goes back to where its anchor went.
    pub(crate) fn map_reloaded(&mut self, rel: &str, old: &str) {
        let Some(new) = self.note_text(rel).map(str::to_string) else { return };
        if !self.map.views.contains_key(rel) {
            return;
        }
        let tree = |text: &str| map::build(&crate::blocks::build(text), text, rel);
        let (was, now, s) = (tree(old), tree(&new), map::diff(old, &new));
        let Some(v) = self.map.views.get_mut(rel) else { return };
        let picked = v.selected;
        // A selected node the change removed gives way to the nearest node at or above where
        // it stood, rather than to the section around it.
        let gone = map::follow(&was, &now, &s, picked).is_none().then(|| was.find(picked).map(|i| &was.nodes[i].own)).flatten();
        v.follow(&was, &now, &s);
        if let Some(own) = gone {
            let line = s.lines.start + own.start.saturating_sub(s.lines.start).min(s.with.len());
            let above = now.nodes.iter().filter(|n| !n.own.is_empty() && n.own.start <= line).max_by_key(|n| n.own.start);
            v.selected = above.map_or(now.root().id, |n| n.id);
        }
        if let Some(e) = self.map.inline.as_mut().filter(|e| e.rel == rel && matches!(e.what, Editing::New(_))) {
            e.anchor = map::follow(&was, &now, &s, e.anchor).unwrap_or(v.selected);
            v.selected = picked;
        }
    }

    /// Node `i` is selected, the folds above it open, and the camera keeps it in view.
    fn map_reveal(&mut self, i: usize, tx: &mut Tx) {
        if self.map_open_to(i).is_some() {
            self.ensure_visible(i, tx);
        }
    }

    /// Node `i` is selected and the folds above it open. Whether the tab's camera has been
    /// placed, or None with no note shown.
    fn map_open_to(&mut self, i: usize) -> Option<bool> {
        let t = self.map_tree();
        let v = self.map_view_mut()?;
        v.selected = t.nodes[i].id;
        let mut a = i;
        while let Some(p) = t.nodes[a].parent {
            if v.folded.remove(&t.nodes[p].id) {
                v.fold_ver += 1;
            }
            a = p;
        }
        Some(v.placed)
    }

    /// A link's target on the map (MAP-054): the node holding source line `line` selected,
    /// its folds open and centred. A tab shown for the first time opens on it at 100%.
    pub fn map_go_line(&mut self, line: usize, tx: &mut Tx) {
        let i = self.map_tree().at_line(line);
        self.map_go_node(i, tx);
    }

    /// Node `i` selected, its folds open and centred, as `map_go_line`.
    pub(crate) fn map_go_node(&mut self, i: usize, tx: &mut Tx) {
        let Some(placed) = self.map_open_to(i) else { return };
        if placed {
            self.map_relayout(None, tx);
            let cam = self.centred_on(i, self.map_camera().1);
            self.map_glide(cam, tx);
        } else {
            let (pan, zoom) = self.centred_on(i, 1.0);
            self.set_camera(pan, zoom);
            if let Some(v) = self.map_view_mut() {
                v.placed = true;
            }
        }
    }

    /// `Delete` and `Backspace` (MAP-025).
    pub fn map_delete(&mut self, tx: &mut Tx) {
        if !self.map_editable() {
            return;
        }
        let t = self.map_tree();
        let i = self.map_selected();
        match map::delete(self.active_text(), &t, i) {
            Ok((s, n)) => {
                self.map_apply(&t, &s, tx);
                if n > 1 || t.nodes[i].card() {
                    let nodes = if n == 1 { "1 node".to_string() } else { format!("{n} nodes") };
                    let undo = crate::keymap::label(self.preset(), "map.undo");
                    self.toast(&format!("Deleted {nodes} · {undo} to undo"), tx);
                }
            }
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `x`, or a click on a task's box (MAP-053): one character, through the undo path.
    pub fn map_toggle_task(&mut self, i: usize, tx: &mut Tx) {
        self.map_toggle_task_inner(i, false, tx);
    }

    fn map_toggle_task_inner(&mut self, i: usize, from_pointer: bool, tx: &mut Tx) {
        if !self.map_editable() {
            return;
        }
        let t = self.map_tree();
        if let Some(s) = map::toggle_task(self.active_text(), &t, i) {
            self.map_apply_inner(&t, &s, from_pointer, tx);
        }
    }

    /// Node `s` moves to `at`, by a drop or a key (MAP-026 to MAP-031).
    pub fn map_move_node(&mut self, s: usize, at: map::Add, tx: &mut Tx) {
        if self.map_editable() {
            let t = self.map_tree();
            let r = map::move_to(self.active_text(), &t, s, at);
            self.map_try(&t, r, tx);
        }
    }

    /// Alt+arrows, Shift+Tab (MAP-031).
    pub fn map_step(&mut self, dir: map::Step, tx: &mut Tx) {
        if self.map_editable() {
            let t = self.map_tree();
            let s = self.map_selected();
            match map::step(&t, s, dir) {
                Ok(at) => self.map_move_node(s, at, tx),
                Err(why) => self.map_try(&t, Err(why), tx),
            }
        }
    }

    /// Ctrl+Alt+1 to 6 (MAP-031).
    pub fn map_level(&mut self, to: usize, tx: &mut Tx) {
        if self.map_editable() {
            let t = self.map_tree();
            let r = map::set_level(self.active_text(), &t, self.map_selected(), to);
            self.map_try(&t, r, tx);
        }
    }

    fn map_try(&mut self, t: &Tree, r: Result<map::Splice, map::Refused>, tx: &mut Tx) {
        match r {
            Ok(s) => self.map_apply(t, &s, tx),
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// `Ctrl+Z` and `Ctrl+Shift+Z` on the map: the note's own history, shared with Source
    /// (MAP-014). The selection goes to the node the step changed, else stays on its node.
    pub fn map_undo(&mut self, redo: bool, tx: &mut Tx) {
        if !self.map_editable() {
            return;
        }
        let old = self.map_tree();
        let before = self.active_text().to_string();
        let e = self.cur_edit();
        if redo {
            e.redo();
        } else {
            e.undo();
        }
        self.after_edit();
        if self.active_text() == before {
            return;
        }
        let s = map::diff(&before, self.active_text());
        let t = self.map_follow(&old, &s);
        let changed = t.nodes.iter().position(|n| !n.own.is_empty() && s.written().contains(&n.own.start));
        let i = changed.unwrap_or_else(|| self.map_selected());
        self.map_reveal(i, tx);
        self.map_relayout(Some((&*old, &s)), tx);
    }

    /// `Ctrl+C` and `Ctrl+Shift+C` (MAP-059).
    pub fn map_copy(&mut self, branch: bool, tx: &mut Tx) {
        if self.mode != Mode::Map || self.active_locked() {
            return;
        }
        let t = self.map_tree();
        tx.copy(map::markdown(self.active_text(), &t, self.map_selected(), branch));
        self.toast(if branch { "Copied the branch's Markdown" } else { "Copied" }, tx);
    }
}

const MULTI_LINE: &str = "Paste multi-line Markdown in Source or the Edit Sheet";

/// The inline field's key: a text field, so the note's caret keys stay out of it.
pub const FIELD: &str = "map-inline-edit";

/// What a new node is placed by: Tab, Enter, Shift+Enter (MAP-021, MAP-023).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Child,
    After,
    Before,
}

impl App {
    /// The node being edited in place, by index in the tree shown.
    pub fn map_editing(&self) -> Option<usize> {
        let e = self.map.inline.as_ref().filter(|e| Some(e.rel.as_str()) == self.active.as_deref())?;
        Some(match e.what {
            Editing::Node(i) if self.active_text() == e.base => i,
            // The note changed outside the app under the field (MAP-068): it stays on its node.
            Editing::Node(i) => {
                let t = self.map_tree();
                let s = map::diff(&e.base, self.active_text());
                map::follow(&e.tree, &t, &s, e.tree.nodes[i].id).and_then(|id| t.find(id)).unwrap_or_else(|| self.map_selected())
            }
            Editing::New(_) => self.map_selected(),
        })
    }

    /// `F2`, `e` or a double click (MAP-016, MAP-019, MAP-020): the inline field on a text
    /// node, the Edit Sheet on a card, the title bar on a root named by the file.
    pub fn map_edit(&mut self, tx: &mut Tx) {
        if !self.map_editable() || self.map.inline.is_some() {
            return;
        }
        let i = self.map_selected();
        if self.open_lightbox(i, 0, tx) {
            return;
        }
        let t = self.map_tree();
        match map::in_place(self.active_text(), &t, i) {
            map::InPlace::Field { text, multi } => {
                let mut edit = TextEdit::new(text);
                let n = edit.text.len();
                edit.set_selection(0, n);
                self.map_open_field(Editing::Node(i), edit, multi, None, tx);
            }
            map::InPlace::Sheet(lines) => self.open_sheet_lines(lines, crate::sheet::Scope::Block, tx),
            map::InPlace::FileName => self.start_rename(tx),
            map::InPlace::Nothing => {}
        }
    }

    /// A press on node `i`'s Expand chip, or on image `k` of its gallery (MAP-047): the node is
    /// selected and its image opens in the lightbox.
    pub fn map_lightbox(&mut self, i: usize, k: usize, tx: &mut Tx) {
        let Some(i) = self.map_press_node(i, tx) else { return };
        let id = self.map_tree().nodes[i].id;
        if let Some(v) = self.map_view_mut() {
            v.selected = id;
        }
        self.open_lightbox(i, k, tx);
    }

    /// The lightbox on node `i`'s image, or on image `k` of its gallery; false when it has none.
    fn open_lightbox(&mut self, i: usize, k: usize, tx: &mut Tx) -> bool {
        let t = self.map_tree();
        let Some(n) = t.nodes.get(i) else { return false };
        let Body::Card(bl) = &n.body else { return false };
        let img = match &bl.kind {
            crate::blocks::Kind::Image { img, .. } => img,
            crate::blocks::Kind::Gallery { items } => match items.get(k) {
                Some(it) => &it.img,
                None => return false,
            },
            _ => return false,
        };
        let lb = crate::layer::Lightbox { img: img.clone(), lines: n.own.clone() };
        self.open_layer(crate::layer::Layer::Lightbox(lb), tx);
        true
    }

    /// Ctrl+Enter on the selected node (MAP-054): the lightbox on an image, else its label's
    /// first link or tag, as a click on it.
    pub fn map_follow_link(&mut self, tx: &mut Tx) {
        if self.mode != Mode::Map || self.map.inline.is_some() {
            return;
        }
        let i = self.map_selected();
        if self.open_lightbox(i, 0, tx) {
            return;
        }
        let t = self.map_tree();
        let Body::Runs(runs) = &t.nodes[i].body else { return };
        match runs.iter().filter(|r| r.img.is_none()).find_map(|r| r.link.clone()) {
            Some(Link::Wiki(w)) => self.open_link(w.trim(), tx),
            Some(Link::Anchor(a)) => self.go_anchor(&a, tx),
            Some(Link::Tag(tag)) => self.search_tag(&tag, tx),
            Some(Link::Url(u)) => self.follow(&u, tx),
            None => {}
        }
    }

    /// The lightbox's Open externally: the image in the system's viewer or the browser.
    pub fn lightbox_open(&mut self, tx: &mut Tx) {
        let Some(crate::layer::Layer::Lightbox(lb)) = self.layer() else { return };
        let url = match self.image_source(&lb.img) {
            crate::images::Source::File(p) => format!("file://{}", p.display()),
            crate::images::Source::Url(u) => u,
            crate::images::Source::Missing(src) => {
                self.notice = Some(format!("No image at {src}"));
                return;
            }
        };
        if let Err(e) = tx.open_url(&url) {
            self.notice = Some(format!("Could not open {url}: {e}"));
        }
    }

    /// The lightbox's Edit Markdown: the Edit Sheet on the image's lines, in its place.
    pub fn lightbox_edit(&mut self, tx: &mut Tx) {
        let Some(crate::layer::Layer::Lightbox(lb)) = self.layer() else { return };
        let lines = lb.lines.clone();
        self.open_sheet_lines(lines, crate::sheet::Scope::Block, tx);
    }

    /// `E` (MAP-020): the Edit Sheet on the selected node's whole branch.
    pub fn map_edit_branch(&mut self, tx: &mut Tx) {
        if !self.map_editable() {
            return;
        }
        let t = self.map_tree();
        let n = &t.nodes[self.map_selected()];
        match n.kind {
            NodeKind::Footnotes => self.toast(map::FOOTNOTES, tx),
            NodeKind::Root if n.own.is_empty() => {}
            _ => self.open_sheet_lines(n.span.clone(), crate::sheet::Scope::Section, tx),
        }
    }

    /// Tab, Enter and Shift+Enter (MAP-021 to MAP-023): a new node, named in the inline field
    /// before anything is written.
    pub fn map_add(&mut self, place: Place, tx: &mut Tx) {
        if !self.map_editable() || self.map.inline.is_some() {
            return;
        }
        let t = self.map_tree();
        let i = self.map_selected();
        let at = match place {
            Place::Child => map::Add::Child(i),
            Place::After => map::Add::After(i),
            Place::Before => map::Add::Before(i),
        };
        // A zero-width space names the new node until the first commit, so it parses as one.
        match map::add(self.active_text(), &t, at, "\u{200b}") {
            Ok(s) => {
                let text = s.apply(self.active_text());
                let mut h = std::collections::hash_map::DefaultHasher::new();
                ("preview", &text).hash(&mut h);
                self.map_open_field(Editing::New(at), TextEdit::new(""), true, Some((h.finish(), text)), tx);
                let shown = self.map_tree();
                let n = s.focus.map_or(ROOT, |l| shown.at_line(l));
                if let Some(e) = &mut self.map.inline {
                    e.multi = !matches!(shown.nodes[n].kind, NodeKind::Heading { .. } | NodeKind::Root);
                }
                self.map_open_to(n);
                self.map_relayout(Some((&*t, &s)), tx);
                self.frame_inline_edit(n, tx);
            }
            Err(Some(why)) => self.toast(why, tx),
            Err(None) => {}
        }
    }

    /// Ctrl+V with no field focused (MAP-048): one line of text goes in as a new node, a child
    /// of a container or a sibling after a leaf, with one trailing line break dropped.
    pub fn map_paste(&mut self, text: &str, tx: &mut Tx) {
        if !self.map_editable() || self.map.inline.is_some() {
            return;
        }
        let line = text.strip_suffix('\n').map_or(text, |l| l.strip_suffix('\r').unwrap_or(l));
        if line.contains(['\n', '\r']) {
            self.toast(MULTI_LINE, tx);
            return;
        }
        if line.trim().is_empty() {
            return;
        }
        let t = self.map_tree();
        let i = self.map_selected();
        let at = if map::takes_children(&t, i) { map::Add::Child(i) } else { map::Add::After(i) };
        let footnote = matches!(t.nodes[i].kind, NodeKind::Footnotes | NodeKind::Footnote { .. });
        match map::add(self.active_text(), &t, at, line.trim()) {
            Ok(s) => self.map_apply(&t, &s, tx),
            Err(why) => {
                if let Some(why) = why.or(footnote.then_some(map::FOOTNOTES)) {
                    self.toast(why, tx);
                }
            }
        }
    }

    /// Where an image added on the map goes (MAP-048): a child of the selected container, else
    /// a sibling after it. None, with a toast, on the footnotes, which take no images (MAP-008).
    pub(crate) fn map_image_place(&mut self, tx: &mut Tx) -> Option<map::Add> {
        let t = self.map_tree();
        let i = self.map_selected();
        if matches!(t.nodes[i].kind, NodeKind::Footnotes | NodeKind::Footnote { .. }) {
            self.toast(map::FOOTNOTES, tx);
            return None;
        }
        Some(if map::takes_children(&t, i) { map::Add::Child(i) } else { map::Add::After(i) })
    }

    /// Image links `lines` go in as one node, one undo step, at the place for images. Several
    /// make one gallery, as they do at the editor's caret.
    pub(crate) fn map_insert_images(&mut self, lines: &[String], tx: &mut Tx) -> bool {
        self.map_commit(tx);
        let Some(at) = self.map_image_place(tx) else { return false };
        let t = self.map_tree();
        match map::add_block(self.active_text(), &t, at, &lines.join("\n")) {
            Ok(s) => {
                self.map_apply(&t, &s, tx);
                true
            }
            Err(why) => {
                if let Some(why) = why {
                    self.toast(why, tx);
                }
                false
            }
        }
    }

    /// Ensures the active inline-edit field is framed at a legible scale (>= `EDIT_ZOOM_MIN`)
    /// and fully visible inside the free canvas.
    fn frame_inline_edit(&mut self, i: usize, tx: &mut Tx) {
        let cam = self.map.glide.as_ref().map_or_else(|| self.map_camera(), |g| g.to);
        if cam.1 < EDIT_ZOOM_MIN {
            let to = self.centred_on(i, 1.0);
            self.map_glide(to, tx);
        } else {
            self.ensure_inline_visible(tx);
        }
    }

    /// Adjusts `cam` by the least pan needed if node `i` would be clipped outside `map_free`.
    fn keep_in_view_cam(&self, i: usize, cam: ([f32; 2], f32)) -> ([f32; 2], f32) {
        let l = self.map_laid();
        let Some(p) = l.at[i] else { return cam };
        let ([px, py], z) = cam;
        let [x0, y0, x1, y1] = self.map_free();
        let nudge = |lo: f32, hi: f32, a: f32, b: f32| {
            if lo >= a && hi <= b {
                0.0
            } else if hi - lo > b - a - 2.0 * MARGIN || lo < a {
                (a + MARGIN).min(b - (hi - lo)).max(a) - lo
            } else {
                (b - MARGIN).max(a + (hi - lo)).min(b) - hi
            }
        };
        let dx = nudge(px + p.x * z, px + p.right() * z, x0, x1);
        let dy = nudge(py + p.y * z, py + p.bottom() * z, y0, y1);
        ([px + dx, py + dy], z)
    }

    /// Pans by the least needed to keep the live inline-edit field rect (growing as text is
    /// typed) `MARGIN` points inside the canvas's free part; when a tall multi-line draft
    /// exceeds the free height, tracks the caret's line so newly typed text stays in view.
    fn ensure_inline_visible(&mut self, tx: &mut Tx) {
        let Some(i) = self.map_editing() else { return };
        let Some(e) = &self.map.inline else { return };
        let t = self.map_tree();
        let l = self.map_laid();
        let Some(p) = l.at[i] else { return };
        let b = self.settings().size as f32;
        let (prose, mono, _, _) = self.fonts();
        let room = task_room(&t.nodes[i], b);
        let st = style(tier(&t, i), b, prose, mono);
        let [fw, fh] = field_size(e, p, room, st, l.metrics);
        let w = p.w.max(fw + room + 2.0 * l.metrics.pad_x);
        let h = p.h.max(fh + l.metrics.pad_b) + map_layout::line_width(p.depth);
        let ([px, py], z) = self.map.glide.as_ref().map_or_else(|| self.map_camera(), |g| g.to);
        let [x0, y0, x1, y1] = self.map_free();
        let least = |lo: f32, hi: f32, a: f32, b: f32| {
            if hi - lo > b - a - 2.0 * MARGIN || lo < a + MARGIN {
                a + MARGIN - lo
            } else if hi > b - MARGIN {
                b - MARGIN - hi
            } else {
                0.0
            }
        };
        let dx = least(px + p.x * z, px + (p.x + w) * z, x0, x1);
        let (lo_y, hi_y) = (py + p.y * z, py + (p.y + h) * z);
        let dy = if hi_y - lo_y > (y1 - y0) - 2.0 * MARGIN {
            let anywhere = rune::text::RichLayoutOptions::new().wrap_anywhere();
            let mut end = e.edit.focus.min(e.edit.text.len());
            while end > 0 && !e.edit.text.is_char_boundary(end) {
                end -= 1;
            }
            let caret_h = if !e.multi {
                st.line_height
            } else {
                let pre = &e.edit.text[..end];
                let extra = if pre.ends_with('\n') { st.line_height } else { 0.0 };
                let body = pre.strip_suffix('\n').unwrap_or(pre);
                let base = if body.is_empty() {
                    st.line_height
                } else {
                    rune::text::measure_with(body, &[], st, Some(fw), anywhere).1.ceil()
                };
                (base + extra).max(st.line_height).min(fh)
            };
            let tail = if end == e.edit.text.len() { l.metrics.pad_b + map_layout::line_width(p.depth) } else { 0.0 };
            let caret_bot = py + (p.y + caret_h + tail) * z;
            let caret_top = py + (p.y + (caret_h - st.line_height).max(0.0)) * z;
            let pad = 16.0f32.min(((y1 - y0) - st.line_height * z).max(0.0) / 2.0);
            if caret_bot > y1 - pad {
                (y1 - pad) - caret_bot
            } else if caret_top < y0 + pad {
                (y0 + pad) - caret_top
            } else {
                0.0
            }
        } else {
            least(lo_y, hi_y, y0, y1)
        };
        if dx != 0.0 || dy != 0.0 {
            self.map_glide(([px + dx, py + dy], z), tx);
        }
    }

    fn map_open_field(&mut self, what: Editing, edit: TextEdit, multi: bool, preview: Option<(u64, String)>, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        let tree = self.map_tree();
        let base = self.active_text().to_string();
        let anchor = tree.nodes[self.map_selected()].id;
        let cam = self.map.glide.as_ref().map_or_else(|| self.map_camera(), |g| g.to);
        let edit_return_cam = Some(cam).filter(|c| c.1 < EDIT_ZOOM_MIN);
        let last_hash = edit_hash(&edit);
        self.map.inline = Some(Inline { rel, base, tree, what, edit, multi, preview, anchor, held: false, edit_return_cam, last_hash });
        tx.focus(FIELD, false);
        if let Editing::Node(i) = what {
            self.frame_inline_edit(i, tx);
        }
    }

    /// Enter, Tab, a click away (MAP-017): the field's text goes into the note, unless it is
    /// empty or unchanged (MAP-018). True when a field was open and has closed.
    pub fn map_commit(&mut self, tx: &mut Tx) -> bool {
        self.map_commit_inner(false, tx)
    }

    fn map_commit_inner(&mut self, restore_cam: bool, tx: &mut Tx) -> bool {
        let Some(e) = self.map.inline.take() else { return false };
        let return_cam = restore_cam.then_some(e.edit_return_cam).flatten();
        let Some(s) = e.splice() else {
            self.map_back_to(&e, tx);
            if let Some(cam) = return_cam {
                let cam = self.keep_in_view_cam(self.map_selected(), cam);
                self.map_glide(cam, tx);
            }
            return true;
        };
        if self.note_text(&e.rel) != Some(e.base.as_str()) {
            self.notice = Some(crate::sheet::CHANGED.into());
            // The field stays with its text to copy while its note is on the map (SHEET-044).
            if self.mode == Mode::Map && Some(e.rel.as_str()) == self.active.as_deref() {
                self.map.inline = Some(Inline { held: false, ..e });
                return false;
            }
            self.map_back_to(&e, tx);
            return true;
        }
        if Some(e.rel.as_str()) == self.active.as_deref() {
            self.map_apply(&e.tree, &s, tx);
            if let Some(cam) = return_cam {
                let cam = self.keep_in_view_cam(self.map_selected(), cam);
                self.map_glide(cam, tx);
            }
        } else {
            self.splice_into(&e.rel, &e.base, &s);
        }
        true
    }

    /// Before the notes shown change under the map (another folder, another note in Note
    /// mode): an open field's text goes into its note, as a click away would.
    pub(crate) fn map_settle(&mut self) {
        let Some(e) = self.map.inline.take() else { return };
        if let Some(s) = e.splice()
            && self.note_text(&e.rel) == Some(e.base.as_str())
        {
            self.splice_into(&e.rel, &e.base, &s);
        }
    }

    /// Splice `s` into note `rel`, whose text is `base`, as one change.
    fn splice_into(&mut self, rel: &str, base: &str, s: &map::Splice) {
        let new = s.apply(base);
        let (pre, post) = shared(base, &new);
        self.splice_note(rel, pre..base.len() - post, &new[pre..new.len() - post]);
    }

    /// A new folder of notes is shown (MAP-044): the last one's maps go, but for a Note mode
    /// note's, which `park` has kept.
    pub(crate) fn map_new_notes(&mut self) {
        self.map_settle();
        self.map.views.clear();
        self.map.glide = None;
        self.map.relayout = None;
        self.map.node_drag = None;
        self.map.drawer_return_pan = None;
    }

    /// Esc in the field (MAP-017): nothing is written, and a new node goes.
    pub fn map_cancel(&mut self, tx: &mut Tx) {
        if let Some(e) = self.map.inline.take() {
            self.map_back_to(&e, tx);
            if let Some(cam) = e.edit_return_cam {
                let cam = self.keep_in_view_cam(self.map_selected(), cam);
                self.map_glide(cam, tx);
            }
        }
        self.keys_to_note(tx);
    }

    /// After an edit that wrote nothing: the selection is where it was before a new node, and
    /// the new node goes.
    fn map_back_to(&mut self, e: &Inline, tx: &mut Tx) {
        if let (Editing::New(_), Some(v)) = (e.what, self.map.views.get_mut(&e.rel)) {
            v.selected = e.anchor;
            self.map_relayout(None, tx);
        }
    }

    /// A key in the inline field before the field's own (KEY-145 to KEY-149, MAP-016).
    pub fn map_field_key(&mut self, ev: &rune::KeyEvent, tx: &mut Tx) -> bool {
        let multi = self.map.inline.as_ref().is_some_and(|e| e.multi);
        for a in crate::keymap::lookup(self.preset(), crate::keymap::Ctx::MapEdit, ev) {
            match a.id {
                "mapedit.newline" if multi => return false,
                "mapedit.commit" | "mapedit.newline" => {
                    if self.map_commit_inner(true, tx) {
                        self.keys_to_note(tx);
                    }
                }
                "mapedit.commit-outdent" => {
                    let unnamed = self.map.inline.as_ref().is_some_and(|e| matches!(e.what, Editing::New(_)) && e.edit.text.trim().is_empty());
                    if self.map_commit(tx) {
                        self.keys_to_note(tx);
                        if !unnamed {
                            self.map_step(map::Step::Out, tx);
                        }
                    }
                }
                "mapedit.cancel" => {
                    if !self.esc_takes_notice() {
                        self.map_cancel(tx);
                    }
                }
                "mapedit.commit-add-child" => {
                    if self.map_commit(tx) {
                        self.keys_to_note(tx);
                        self.map_add(Place::Child, tx);
                    }
                }
                id => match crate::keys::handler(id) {
                    Some(run) => run(self, tx),
                    None => continue,
                },
            }
            return true;
        }
        false
    }

    /// A formatting chord while the field holds focus acts on its text (MAP-016).
    pub fn map_format(&mut self, f: crate::format::Fmt) -> bool {
        let Some(e) = &mut self.map.inline else { return false };
        let ed = crate::format::apply(&e.edit.text, e.edit.anchor, e.edit.focus, f);
        e.edit.undo_group(|e| {
            e.set_selection(ed.start, ed.end);
            e.insert(&ed.insert);
            e.set_selection(ed.sel.0, ed.sel.1);
        });
        true
    }

    /// Before each frame: a field that lost focus, or whose note or mode went, commits.
    /// While focused, re-checks visibility whenever the typed text or caret changes.
    fn poll_field(&mut self, tx: &mut Tx) -> bool {
        let Some(e) = &mut self.map.inline else { return false };
        if tx.focused() == Some(FIELD) && self.mode == Mode::Map {
            e.held = true;
            let h = edit_hash(&e.edit);
            if h != e.last_hash {
                e.last_hash = h;
                self.ensure_inline_visible(tx);
            }
            return false;
        }
        if !e.held && self.mode == Mode::Map && Some(e.rel.as_str()) == self.active.as_deref() {
            return false;
        }
        let restore = matches!(e.what, Editing::Node(_)) && self.mode == Mode::Map && Some(e.rel.as_str()) == self.active.as_deref();
        self.map_commit_inner(restore, tx)
    }

    /// A click on node `i`'s task box (MAP-053): toggles in place without panning a task
    /// already wholly inside the canvas out from under the pointer.
    pub fn map_task_click(&mut self, i: usize, tx: &mut Tx) {
        let Some(i) = self.map_press_node(i, tx) else { return };
        self.map_toggle_task_inner(i, true, tx);
        self.keys_to_note(tx);
    }

    /// A click on the Properties badge (MAP-006): the Edit Sheet on the front matter.
    pub fn map_props(&mut self, tx: &mut Tx) {
        self.map_commit(tx);
        if self.map_editable() && self.map.inline.is_none() {
            let lines = map::front_lines(self.active_text());
            self.open_sheet_lines(lines, crate::sheet::Scope::Block, tx);
        }
    }

    /// A press on node `i` of the tree shown: an open field commits first, and the press then
    /// lands on the same node in the new tree.
    pub(crate) fn map_press_node(&mut self, i: usize, tx: &mut Tx) -> Option<usize> {
        let shown = self.map_tree();
        let text = self.map.inline.as_ref().and_then(|e| e.preview.as_ref()).map_or_else(|| self.active_text().to_string(), |p| p.1.clone());
        let restore = self.map.inline.as_ref().is_some_and(|e| matches!(e.what, Editing::Node(_)));
        if !self.map_commit_inner(restore, tx) {
            return Some(i);
        }
        let t = self.map_tree();
        let s = map::diff(&text, self.active_text());
        map::follow(&shown, &t, &s, shown.nodes.get(i)?.id).and_then(|id| t.find(id))
    }
}
