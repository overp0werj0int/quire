//! The layers over the window: everything that opens over the page and takes the keyboard
//! while it is open (ARCHITECTURE.md, "Layers"). One model for all of them:
//!
//! - At most one layer is open. A menu can open over the layer it was opened in (a field's
//!   menu over the edit sheet, a row's menu over the sidebar's drawer); anything else replaces
//!   whatever was open.
//! - Each layer remembers where focus was when it opened, and gives it back as it closes, by
//!   whatever way it closes.
//! - Each remembers what the page showed when it opened. A light layer (a menu or a drawer)
//!   closes when that changes; every layer closes when the window leaves the folder.
//! - `keys::on_key` decides what a key does while one is open, and `ui::layer` gives each its
//!   plane, its scrim and its outside-press behaviour.

use rune::Tx;

use crate::app::{App, Drawer, Mode, Palette, Screen};
use crate::folders::MenuOn;
use crate::sheet::Sheet;

pub enum Layer {
    /// The quick switcher or the command palette.
    Palette(Palette),
    Settings,
    /// The dialog that locks the note, with why the last try was refused.
    Lock(Option<String>),
    /// The keyboard help, with its search.
    Help(crate::help::Search),
    /// The edit sheet over the reader.
    Sheet(Box<Sheet>),
    /// The sidebar or the outline over the page, in a window too narrow to dock it.
    Drawer(Drawer),
    Menu(Menu),
    /// An image on the map, large (MAP-047).
    Lightbox(Lightbox),
    /// The map's layout picker, under its button in the title bar (MAP-089).
    Layouts,
}

/// The lightbox's image, and the source lines its Edit Markdown button opens.
pub struct Lightbox {
    pub img: crate::md::Img,
    pub lines: std::ops::Range<usize>,
}

/// A menu, at the point it opened at, on what it opened on.
pub struct Menu {
    pub at: (f32, f32),
    pub of: MenuOf,
}

/// What a menu was opened on.
pub enum MenuOf {
    /// The vault's label in the sidebar: switch folder, new folder.
    Folder,
    /// A note, a folder or the tree's empty space (`folders::MenuOn`).
    Tree(MenuOn),
    /// The tree menu's second step for a folder with notes: delete it or not.
    DeleteFolder(String),
    /// A text field, by key: the source editor or the edit sheet's (WR-135, SHEET-032).
    Edit(&'static str),
    /// A block of the page, by its first source line (RD-68).
    Block(usize),
    /// A map node (MAP-062).
    Node(crate::map::NodeId),
    /// A recent folder row on the welcome screen.
    RecentVault(std::path::PathBuf),
}

/// Which layer, without its state: what a key toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Palette,
    Settings,
    Lock,
    Help,
    Sheet,
    Drawer(Drawer),
    Menu,
    Lightbox,
    Layouts,
}

impl Layer {
    pub fn kind(&self) -> Kind {
        match self {
            Layer::Palette(_) => Kind::Palette,
            Layer::Settings => Kind::Settings,
            Layer::Lock(_) => Kind::Lock,
            Layer::Help(_) => Kind::Help,
            Layer::Sheet(_) => Kind::Sheet,
            Layer::Drawer(d) => Kind::Drawer(*d),
            Layer::Menu(_) => Kind::Menu,
            Layer::Lightbox(_) => Kind::Lightbox,
            Layer::Layouts => Kind::Layouts,
        }
    }
}

impl Kind {
    /// A menu, a drawer or the layout picker: the page around it stays in view, so a window
    /// shortcut closes it and acts as if it had not been there, and a change to the page closes it.
    pub fn light(self) -> bool {
        matches!(self, Kind::Drawer(_) | Kind::Menu | Kind::Layouts)
    }
}

/// The key of the layout picker's tile for `l`.
pub fn tile_key(l: crate::map::Layout) -> String {
    format!("layout-{}", l.word())
}

/// A field the page opens for a name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    /// The note's name, in the title bar.
    NoteName,
    /// A new folder's name, in the tree.
    FolderName,
}

impl Field {
    /// The drawer the field shows in while that drawer is open; None when it shows under
    /// every layer.
    fn drawer(self) -> Option<Drawer> {
        match self {
            Field::NoteName => None,
            Field::FolderName => Some(Drawer::Sidebar),
        }
    }
}

/// An open layer.
pub struct Open {
    pub layer: Layer,
    /// The element focus goes back to as it closes; `None` leaves it to Rune's modal, which
    /// gives it back to what had it.
    back: Option<String>,
    /// What the page showed as it opened.
    over: Context,
    /// The window's size as it opened: a menu closes when it changes, since the page under
    /// it reflows.
    size: (f32, f32),
}

/// What the page shows: the screen, the folder, the note and how it shows. A light layer
/// closes when it changes.
#[derive(Clone, PartialEq, Debug)]
struct Context {
    screen: Screen,
    folder: Option<std::path::PathBuf>,
    note: Option<String>,
    mode: Mode,
    locked: bool,
    zen: bool,
    docked: (bool, bool),
}

impl App {
    fn context(&self) -> Context {
        Context {
            screen: self.screen,
            folder: self.vault.as_ref().map(|v| v.path.clone()),
            note: self.active.clone(),
            mode: self.mode,
            locked: self.active.as_deref().is_some_and(|a| self.note_locked(a)),
            zen: self.focus,
            docked: (self.persist.left_open, self.persist.right_open),
        }
    }

    fn window_size(&self) -> (f32, f32) {
        (self.view_w.get(), self.view_h.get())
    }

    /// The top layer, if one is open.
    pub fn layer(&self) -> Option<&Layer> {
        self.layers.last().map(|o| &o.layer)
    }

    /// Every open layer, bottom first.
    pub fn open_layers(&self) -> impl DoubleEndedIterator<Item = &Layer> {
        self.layers.iter().map(|o| &o.layer)
    }

    pub fn layer_is(&self, kind: Kind) -> bool {
        self.layer().is_some_and(|l| l.kind() == kind)
    }

    /// The open palette or quick switcher.
    pub fn pal(&self) -> Option<&Palette> {
        self.open_layers().find_map(|l| match l {
            Layer::Palette(p) => Some(p),
            _ => None,
        })
    }

    pub fn pal_mut(&mut self) -> Option<&mut Palette> {
        self.layers.iter_mut().find_map(|o| match &mut o.layer {
            Layer::Palette(p) => Some(p),
            _ => None,
        })
    }

    /// The open drawer.
    pub fn drawer(&self) -> Option<Drawer> {
        self.open_layers().find_map(|l| match l {
            Layer::Drawer(d) => Some(*d),
            _ => None,
        })
    }

    /// The open menu.
    pub fn menu(&self) -> Option<&Menu> {
        self.open_layers().find_map(|l| match l {
            Layer::Menu(m) => Some(m),
            _ => None,
        })
    }

    /// The open edit sheet.
    pub fn sheet(&self) -> Option<&Sheet> {
        self.open_layers().find_map(|l| match l {
            Layer::Sheet(sh) => Some(&**sh),
            _ => None,
        })
    }

    pub fn sheet_mut(&mut self) -> Option<&mut Sheet> {
        self.layers.iter_mut().find_map(|o| match &mut o.layer {
            Layer::Sheet(sh) => Some(&mut **sh),
            _ => None,
        })
    }

    /// Why the lock dialog refused the last password, while it is open.
    pub fn lock_error(&self) -> Option<&str> {
        self.open_layers().find_map(|l| match l {
            Layer::Lock(e) => e.as_deref(),
            _ => None,
        })
    }

    /// The lock dialog refuses a password, and says why.
    pub fn refuse_lock(&mut self, why: String) {
        if let Some(Layer::Lock(e)) = self.layers.last_mut().map(|o| &mut o.layer) {
            *e = Some(why);
        }
    }

    /// Opens `layer` over the page, closing whatever was open. It takes over where focus goes
    /// back to from the layer it replaces, so a palette that gives way to Settings still hands
    /// the keyboard back to the note. A menu opens over the layer it was opened in, replacing
    /// only a menu, and a field's menu gives focus back to its field. Find closes too, keeping
    /// its query, and the layer gives the keyboard back to the note, not to find's field.
    pub fn open_layer(&mut self, layer: Layer, tx: &mut Tx) {
        let found = self.hide_find(tx);
        if found && !self.writing_note() {
            // Find's field had the keyboard; the reader's page takes it back as the layer closes.
            tx.blur();
        }
        let replaced = match &layer {
            Layer::Menu(_) => self.layers.pop_if(|o| matches!(o.layer, Layer::Menu(_))).into_iter().collect(),
            _ => std::mem::take(&mut self.layers),
        };
        // One that gives way to this one, now or earlier in this change, leaves at once rather
        // than fading out under it.
        self.layer_swap = !replaced.is_empty() || self.handed.is_some();
        // A drag in the tree under it ends: its ghost would outlive the press.
        self.dragging = None;
        let back = match (&layer, replaced.into_iter().next()) {
            (Layer::Menu(Menu { of: MenuOf::Edit(field), .. }), _) => Some(field.to_string()),
            (_, Some(first)) => first.back,
            (_, None) if found => self.writing_note().then(|| "editor".to_string()),
            (_, None) => self.handed.take().unwrap_or_else(|| self.focus_now(tx)),
        };
        self.handed = None;
        // Opened by keys, a drawer's first stop takes the keyboard with its ring, so the next
        // key visibly acts on it. Rune's modal would focus it without one.
        if let Layer::Drawer(d) = &layer {
            if tx.by_keys() {
                tx.focus(if *d == Drawer::Sidebar { "sidebar-over" } else { "outline-over" }, true);
            }
            // When the sidebar drawer opens, scroll an expanded active note into view so deep
            // notes are visible even inside short drawers (520×400, 640×480).
            if *d == Drawer::Sidebar
                && !self.note_mode()
                && self.query.text.trim().is_empty()
                && self.persist.sidebar.folded != Some(crate::sidebar::Section::Vault)
                && let Some(a) = self.active.as_deref()
                && self.files().contains_key(a)
                && self.ancestors_expanded(a)
            {
                self.tree_reveal = None;
                tx.scroll_into_view_in("side-scroll", crate::folders::row_key("file", a));
            }
        }
        self.layers.push(Open { layer, back, over: self.context(), size: self.window_size() });
        if self.drawer().is_some() {
            self.map_drawer_opened(tx);
        }
    }

    /// Opens the map's layout picker and puts the keyboard on the tile of the layout the note
    /// has, ringed when it came by keys (MAP-089).
    pub fn open_layouts(&mut self, tx: &mut Tx) {
        if !self.map_editable() {
            return;
        }
        self.map_commit(tx);
        let now = self.map_tree().opts.layout;
        self.open_layer(Layer::Layouts, tx);
        tx.focus(tile_key(now), tx.by_keys());
    }

    /// Opens a menu of `of` at `at`. Single-action pointer menus that arm Enter immediately
    /// (`RecentVault`) show visible focus on their item right away (`REVEALWEL`).
    pub fn open_menu(&mut self, of: MenuOf, at: (f32, f32), tx: &mut Tx) {
        let arm = matches!(of, MenuOf::RecentVault(_));
        self.open_layer(Layer::Menu(Menu { at, of }), tx);
        if arm {
            tx.focus("recent-menu", true);
        }
    }

    /// Opens a layer outside any event (a scene, a test): focus goes back where Rune's modal
    /// found it.
    pub fn show_layer(&mut self, layer: Layer) {
        self.layers.clear();
        self.layers.push(Open { layer, back: None, over: self.context(), size: self.window_size() });
    }

    /// Where focus is as a layer opens: what had it, and in the writer, the note. A press on a
    /// button in the window's chrome leaves the note's caret where it was, so a layer opened by
    /// the pointer hands the keyboard back to the note.
    fn focus_now(&self, tx: &Tx) -> Option<String> {
        let note = self.writing_note().then(|| "editor".to_string());
        if tx.by_keys() { tx.focused().map(str::to_string).or(note) } else { note.or_else(|| tx.focused().map(str::to_string)) }
    }

    /// The note is open in the writer.
    pub(crate) fn writing_note(&self) -> bool {
        self.screen == Screen::Vault && self.mode == Mode::Source && self.active.as_deref().is_some_and(|a| !self.note_locked(a))
    }

    /// The keyboard goes back to the note: to the editor in the writer, else to the reader's
    /// anchor (`focus::NOTE`), which hands the reader its keys. Tab then goes on from the note,
    /// into its links, however the note got the keyboard.
    pub fn keys_to_note(&mut self, tx: &mut Tx) {
        if self.writing_note() {
            self.focus_editor(tx);
        } else if self.screen == Screen::Vault && self.mode == Mode::Map && self.active_locked() {
            tx.focus(crate::ui::lock::PASS, false);
        } else if self.screen == Screen::Vault {
            tx.focus(crate::focus::NOTE, false);
        } else {
            tx.blur();
        }
    }

    /// Closes the top layer and gives focus back to where it was before it opened.
    pub fn close_layer(&mut self, tx: &mut Tx) {
        if let Some(open) = self.layers.pop() {
            if matches!(open.layer, Layer::Drawer(_)) {
                self.map_drawer_closed(tx);
            }
            self.give_back(open.back, tx);
        }
    }

    /// A menu's item is picked: the menu closes, giving focus back, and the item runs. An item
    /// that opens a name field gives it the keyboard (`field_opened`).
    pub fn pick(&mut self, run: &dyn Fn(&mut App, &mut Tx), tx: &mut Tx) {
        self.close_layer(tx);
        run(self, tx);
    }

    /// A name field opens and asks for the keyboard, from a menu's item, a button or a key.
    /// When it shows behind the layers still open (a note's name in the title bar, behind the
    /// sidebar's drawer), they close, with no hand-back, so the field takes the keys rather
    /// than a stop in the drawer.
    pub(crate) fn field_opened(&mut self, field: Field) {
        if !self.layers.is_empty() && field.drawer() != self.drawer() {
            self.layers.clear();
            self.handed = None;
        }
    }

    /// A row's menu deleted what the row showed. The row cannot take focus back, so the drawer
    /// it was in closes and the note gets the keys, as when the open note is deleted.
    pub fn row_deleted(&mut self, tx: &mut Tx) {
        self.close_drawer(tx);
        self.keys_to_note(tx);
    }

    /// Closes a drawer that is open, with no hand-back: the caller gives the keyboard on.
    pub(crate) fn close_drawer(&mut self, tx: &mut Tx) {
        let had_drawer = self.drawer().is_some();
        self.layers.retain(|o| !matches!(o.layer, Layer::Drawer(_)));
        if had_drawer {
            self.map_drawer_closed(tx);
        }
    }

    /// Closes every open layer.
    pub fn close_layers(&mut self, tx: &mut Tx) {
        if !self.layers.is_empty() {
            let had_drawer = self.drawer().is_some();
            let back = self.layers.swap_remove(0).back;
            self.layers.clear();
            if had_drawer {
                self.map_drawer_closed(tx);
            }
            self.give_back(back, tx);
        }
    }

    /// Esc, or a press outside the top layer. The edit sheet asks before it drops a change
    /// (SHEET-037 to SHEET-039).
    pub fn dismiss(&mut self, tx: &mut Tx) {
        match self.layer() {
            Some(Layer::Sheet(_)) => self.sheet_escape(tx),
            Some(_) => self.close_layer(tx),
            None => {}
        }
    }

    /// Focus goes back where it was, unless this change already put it somewhere (a picked
    /// item that renames a note focuses the name). A layer that opens in the same change
    /// takes it over (`open_layer`).
    fn give_back(&mut self, back: Option<String>, tx: &mut Tx) {
        if let Some(key) = &back {
            // A stop shows its ring, so the next key visibly acts on it (FOCUS).
            tx.return_focus(key.clone(), tx.focus_visible() || crate::focus::stop(key).is_some());
        }
        self.handed = Some(back);
    }

    /// Before each frame: a layer whose page changed under it closes, and with it everything
    /// over it; a menu closes when the window's size changes too. A layer opened in the same
    /// change keeps its hand-over only until then.
    pub(crate) fn drop_stale_layers(&mut self, tx: &mut Tx) -> bool {
        self.handed = None;
        self.layer_swap = false;
        if self.layers.is_empty() {
            return false;
        }
        let now = self.context();
        let size = self.window_size();
        let gone = |o: &Open| match o.layer {
            Layer::Drawer(d) => !self.drawer_fits(d),
            Layer::Menu(_) => o.size != size,
            _ => false,
        };
        let stale = |o: &Open| o.over.screen != now.screen || o.over.folder != now.folder || (o.layer.kind().light() && o.over != now) || gone(o);
        let Some(i) = self.layers.iter().position(stale) else { return false };
        let had_drawer = self.layers[i..].iter().any(|o| matches!(o.layer, Layer::Drawer(_)));
        let back = self.layers[i].back.clone();
        self.layers.truncate(i);
        if had_drawer {
            self.map_drawer_closed(tx);
        }
        if i == 0 {
            self.give_back(back, tx);
            self.handed = None;
        }
        true
    }
}

/// The layer an action's key opens and closes again: pressed while that layer is open, it
/// closes it, rather than closing it and acting.
pub fn toggles(id: &str) -> Option<Kind> {
    match id {
        "app.settings" => Some(Kind::Settings),
        "reader.help" | "writer.help" => Some(Kind::Help),
        "app.toggle-sidebar" => Some(Kind::Drawer(Drawer::Sidebar)),
        "app.toggle-outline" | "reader.toggle-outline" => Some(Kind::Drawer(Drawer::Outline)),
        _ => None,
    }
}
