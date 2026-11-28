//! The keymap: every key the app answers, as a single unified table from a context and a chord
//! to a named action (`docs/reader-writer/behavior/keymap.md`, decision 0003), with user
//! overrides persisted in `~/.config/quire/keybindings.json`.
//!
//! Chords are written as the spec writes them ("Ctrl+Shift+B", "j", "G", "g g", "Space"), so
//! the table reads against the spec line by line, and a test checks that they agree. What each
//! action does is `keys::handler`; an action with no handler yet falls through.

use std::collections::BTreeMap;

use rune::{Key, KeyEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ctx {
    Global,
    Reader,
    Writer,
    Sheet,
    Find,
    Palette,
    Field,
    Help,
    Rewrite,
    Map,
    MapEdit,
    Board,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    In,
    Adapted,
    Quire,
    Deferred,
    Dropped,
}

pub struct Action {
    /// The KEY item in the spec, which the spec test reads.
    #[cfg_attr(not(test), allow(dead_code))]
    pub item: u16,
    pub id: &'static str,
    /// How Settings and the help sheets name it.
    pub name: &'static str,
    pub ctx: &'static [Ctx],
    pub status: Status,
    pub chords: &'static [&'static str],
    /// Two actions may share a chord in one context only when both have guards and the guards
    /// exclude each other. The conflict test reads it; the feature checks the condition.
    #[cfg_attr(not(test), allow(dead_code))]
    pub guard: Option<&'static str>,
}

const fn a(item: u16, id: &'static str, name: &'static str, ctx: &'static [Ctx], status: Status, chords: &'static [&'static str]) -> Action {
    Action { item, id, name, ctx, status, chords, guard: None }
}

use Ctx::*;
use Status::*;

const W: &[Ctx] = &[Writer, Sheet];
const WM: &[Ctx] = &[Writer, Sheet, MapEdit];
const M: &[Ctx] = &[Map];
const ME: &[Ctx] = &[MapEdit];
const B: &[Ctx] = &[Board];

pub const TABLE: &[Action] = &[
    // Global.
    a(8, "app.command-palette", "Command palette", &[Global], Adapted, &["Ctrl+K"]),
    a(9, "app.quick-switcher", "Quick switcher", &[Global], Adapted, &["Ctrl+Shift+O"]),
    a(10, "app.search-notes", "Search all notes", &[Global], Adapted, &["Ctrl+Shift+F"]),
    a(11, "app.toggle-sidebar", "Toggle sidebar", &[Global], Adapted, &["Ctrl+Shift+B"]),
    a(12, "app.toggle-outline", "Toggle outline", &[Global], Adapted, &["Ctrl+O"]),
    a(13, "app.mode-source", "Source mode", &[Global], Adapted, &["Ctrl+E"]),
    a(160, "app.mode-preview", "Preview mode", &[Global], Adapted, &["Ctrl+P"]),
    a(102, "app.mode-map", "Map mode", &[Global], Adapted, &["Ctrl+M"]),
    a(195, "app.mode-board", "Board mode", &[Global], Adapted, &["Ctrl+Shift+K"]),
    a(14, "app.settings", "Settings", &[Global], Quire, &["Ctrl+,"]),
    a(15, "app.focus-mode", "Zen mode", &[Global], Quire, &["Ctrl+."]),
    a(16, "app.new-note", "New note", &[Global], Quire, &["Ctrl+N"]),
    a(17, "app.quit", "Quit", &[Global], Quire, &["Ctrl+Q"]),
    a(18, "app.insert-image", "Insert an image", &[Global], Quire, &["Ctrl+Shift+I"]),
    a(19, "app.save", "Save now", &[Global], In, &["Ctrl+S"]),
    a(20, "app.close-tab", "Close tab", &[Global], In, &["Ctrl+W"]),
    a(161, "app.reopen-tab", "Reopen closed tab", &[Global], Quire, &["Ctrl+Shift+T"]),
    a(39, "app.read-from-disk", "Read from disk", &[Global], Adapted, &["Ctrl+R"]),
    a(21, "app.zoom-in", "Zoom in", &[Global], Adapted, &["Ctrl+=", "Ctrl++"]),
    a(22, "app.zoom-out", "Zoom out", &[Global], Adapted, &["Ctrl+-"]),
    a(40, "app.zoom-reset", "Reset zoom", &[Global], Adapted, &["Ctrl+0"]),
    a(66, "app.help", "Keyboard help", &[Global], Adapted, &["Ctrl+/", "Ctrl+?", "F1"]),
    a(23, "app.escape", "Close or leave", &[Global], Quire, &["Esc"]),
    a(94, "app.undo-delete", "Undo the delete", &[Global], Quire, &["Ctrl+Shift+R"]),
    a(96, "app.next-pane", "Next pane", &[Global], Quire, &["F6"]),
    a(97, "app.previous-pane", "Previous pane", &[Global], Quire, &["Shift+F6"]),
    a(99, "app.tab-1", "Tab 1", &[Global], Quire, &["Ctrl+1"]),
    a(99, "app.tab-2", "Tab 2", &[Global], Quire, &["Ctrl+2"]),
    a(99, "app.tab-3", "Tab 3", &[Global], Quire, &["Ctrl+3"]),
    a(99, "app.tab-4", "Tab 4", &[Global], Quire, &["Ctrl+4"]),
    a(99, "app.tab-5", "Tab 5", &[Global], Quire, &["Ctrl+5"]),
    a(99, "app.tab-6", "Tab 6", &[Global], Quire, &["Ctrl+6"]),
    a(99, "app.tab-7", "Tab 7", &[Global], Quire, &["Ctrl+7"]),
    a(99, "app.tab-8", "Tab 8", &[Global], Quire, &["Ctrl+8"]),
    a(99, "app.tab-9", "Tab 9", &[Global], Quire, &["Ctrl+9"]),
    a(100, "app.next-tab", "Next tab", &[Global], Quire, &["Ctrl+Tab"]),
    a(101, "app.previous-tab", "Previous tab", &[Global], Quire, &["Ctrl+Shift+Tab"]),
    a(186, "app.nav-back", "Go back", &[Reader, Writer], Quire, &["Alt+Left"]),
    a(187, "app.nav-forward", "Go forward", &[Reader, Writer], Quire, &["Alt+Right"]),
    a(188, "note.lock", "Lock or lock now", &[Global], Quire, &["Ctrl+Alt+L"]),
    a(189, "note.lock-all", "Lock all notes", &[Global], Quire, &["Ctrl+Alt+Shift+L"]),
    // Reader.
    a(24, "reader.scroll-down", "Scroll down", &[Reader], In, &["j", "Down"]),
    a(25, "reader.scroll-up", "Scroll up", &[Reader], In, &["k", "Up"]),
    a(26, "reader.page-down", "Page down", &[Reader], In, &["Space", "PageDown"]),
    a(27, "reader.page-up", "Page up", &[Reader], In, &["Shift+Space", "PageUp"]),
    a(28, "reader.top", "Top of the note", &[Reader], In, &["g g", "Home"]),
    a(29, "reader.bottom", "End of the note", &[Reader], In, &["G", "End"]),
    a(162, "reader.toggle-sidebar", "Toggle sidebar", &[Reader], Adapted, &["b", "B", "Ctrl+B"]),
    a(30, "reader.toggle-outline", "Toggle outline", &[Reader], In, &["o", "O"]),
    a(31, "reader.toggle-wide", "Readable or full width", &[Reader], In, &["w", "W"]),
    a(163, "reader.mode-preview", "Preview mode", &[Reader], Adapted, &["p"]),
    a(164, "reader.mode-map", "Map mode", &[Reader], Adapted, &["m"]),
    a(165, "reader.read-from-disk", "Read from disk", &[Reader], Adapted, &["r"]),
    a(166, "reader.focus-mode", "Zen mode", &[Reader], Adapted, &["z", "."]),
    a(167, "reader.settings", "Settings", &[Reader], Adapted, &[","]),
    a(168, "reader.save", "Save now", &[Reader], Adapted, &["s"]),
    a(169, "reader.insert-image", "Insert an image", &[Reader], Adapted, &["i"]),
    a(32, "reader.find", "Find", &[Reader], In, &["Ctrl+F", "/"]),
    Action { guard: Some("a find query exists"), ..a(33, "reader.find-next", "Next match", &[Reader], Adapted, &["n", "F3", "Ctrl+G"]) },
    a(34, "reader.find-previous", "Previous match", &[Reader], Adapted, &["N", "Shift+F3", "Ctrl+Shift+G"]),
    a(35, "reader.copy-selection", "Copy the selection", &[Reader], In, &["Ctrl+C"]),
    a(36, "reader.copy-block-under-pointer", "Copy the block's Markdown", &[Reader], In, &["Ctrl+Shift+C"]),
    a(37, "reader.copy-all", "Copy the whole note", &[Reader], In, &["Ctrl+Shift+A"]),
    a(38, "reader.block-menu", "Copy or edit a block", &[Reader], In, &["Right-click"]),
    a(41, "reader.edit-block", "Edit this block", &[Reader], In, &["e"]),
    a(42, "reader.edit-section", "Edit this section", &[Reader], In, &["E"]),
    a(184, "reader.undo", "Undo", &[Reader], Quire, &["Ctrl+Z"]),
    a(185, "reader.redo", "Redo", &[Reader], Quire, &["Ctrl+Shift+Z", "Ctrl+Y"]),
    a(194, "reader.toggle-fold", "Fold or unfold section", &[Reader], Quire, &["Ctrl+Shift+["]),
    a(43, "reader.help", "Keyboard help", &[Reader], Adapted, &["?"]),
    a(44, "reader.close-tab", "Close tab", &[Reader], Dropped, &[]),
    a(45, "reader.escape", "Close find or clear the selection", &[Reader], Adapted, &["Esc"]),
    a(46, "reader.versions", "Versions", &[Reader], Deferred, &["v", "V"]),
    a(47, "reader.fold", "Fold unchanged text", &[Reader], Deferred, &["f", "F"]),
    Action { guard: Some("changes mode with no find query"), ..a(48, "reader.next-change", "Next change", &[Reader], Deferred, &["n"]) },
    a(49, "reader.previous-change", "Previous change", &[Reader], Deferred, &["P"]),
    // Writer, and the sheet, whose editor is the writer's.
    a(50, "writer.bold", "Bold", WM, In, &["Ctrl+B"]),
    a(51, "writer.italic", "Italic", WM, In, &["Ctrl+I"]),
    a(52, "writer.inline-code", "Inline code", WM, Adapted, &["Ctrl+Shift+C"]),
    a(53, "writer.strikethrough", "Strikethrough", WM, In, &["Ctrl+Shift+X"]),
    a(54, "writer.link", "Link", WM, Adapted, &["Ctrl+L"]),
    a(170, "writer.wiki-link", "Note link", WM, Quire, &["Ctrl+Shift+L"]),
    a(171, "writer.blockquote", "Blockquote", W, Quire, &["Ctrl+Shift+Q"]),
    a(172, "writer.toggle-checkbox", "Toggle task", W, Quire, &["Ctrl+Shift+Enter"]),
    a(173, "writer.move-up", "Move line up", W, Quire, &["Alt+Up"]),
    a(174, "writer.move-down", "Move line down", W, Quire, &["Alt+Down"]),
    a(55, "writer.heading-1", "Heading 1", W, Adapted, &["Ctrl+Alt+1"]),
    a(55, "writer.heading-2", "Heading 2", W, Adapted, &["Ctrl+Alt+2"]),
    a(55, "writer.heading-3", "Heading 3", W, Adapted, &["Ctrl+Alt+3"]),
    a(55, "writer.heading-4", "Heading 4", W, Adapted, &["Ctrl+Alt+4"]),
    a(55, "writer.heading-5", "Heading 5", W, Adapted, &["Ctrl+Alt+5"]),
    a(55, "writer.heading-6", "Heading 6", W, Adapted, &["Ctrl+Alt+6"]),
    a(56, "writer.plain-paragraph", "Plain paragraph", W, Adapted, &["Ctrl+Alt+0"]),
    a(152, "writer.numbered-list", "Numbered list", W, Quire, &["Ctrl+Shift+7"]),
    a(153, "writer.bulleted-list", "Bulleted list", W, Quire, &["Ctrl+Shift+8"]),
    a(154, "writer.task-list", "Task list", W, Quire, &["Ctrl+Shift+9"]),
    a(57, "writer.indent", "Indent", W, In, &["Tab"]),
    a(58, "writer.outdent", "Outdent", W, Adapted, &["Shift+Tab"]),
    a(59, "writer.continue-block", "Continue a list or quote", W, In, &["Enter"]),
    a(60, "writer.follow-link", "Follow the link at the caret", &[Writer], Quire, &["Ctrl+Enter"]),
    a(61, "writer.follow-link-click", "Follow a [[link]]", &[Writer], Quire, &["Ctrl+click"]),
    a(62, "writer.find", "Find", &[Writer], In, &["Ctrl+F"]),
    a(183, "writer.replace", "Find and replace", &[Writer], Quire, &["Ctrl+H"]),
    a(63, "writer.find-next", "Next match", &[Writer], In, &["Ctrl+G", "F3"]),
    a(64, "writer.find-previous", "Previous match", &[Writer], Adapted, &["Ctrl+Shift+G", "Shift+F3"]),
    a(65, "writer.recent-notes", "Recent notes", &[Writer], Dropped, &[]),
    a(67, "writer.paragraph-focus", "Paragraph focus", &[Writer], Adapted, &["Ctrl+Shift+D"]),
    a(68, "writer.typewriter", "Typewriter scrolling", &[Writer], Adapted, &["F9"]),
    a(69, "writer.next-font", "Next font", &[Writer], In, &["Ctrl+Shift+M"]),
    a(70, "writer.escape", "Close a menu, help or find, else show the preview", &[Writer], In, &["Esc"]),
    a(71, "writer.spelling", "Spell checking", &[Writer], Deferred, &["F7"]),
    a(72, "writer.rewrite", "Rewrite", W, Deferred, &["Ctrl+J"]),
    a(73, "writer.rewrite-section", "Rewrite the section", W, Deferred, &["Ctrl+Shift+J"]),
    a(74, "writer.save-as", "Save as", &[Writer], Dropped, &[]),
    // Sheet.
    a(75, "sheet.save", "Save the sheet", &[Sheet], In, &["Ctrl+Enter", "Ctrl+S"]),
    a(76, "sheet.cancel", "Close the sheet", &[Sheet], In, &["Esc"]),
    // Find.
    a(77, "find.next", "Next match", &[Find], In, &["Enter"]),
    a(78, "find.previous", "Previous match", &[Find], In, &["Shift+Enter"]),
    a(79, "find.close", "Close find", &[Find], In, &["Esc"]),
    a(80, "find.select-all", "Select the query", &[Find], In, &["Ctrl+F"]),
    // Palette.
    a(81, "palette.next", "Next row", &[Palette], Quire, &["Down"]),
    a(82, "palette.previous", "Previous row", &[Palette], Quire, &["Up"]),
    a(83, "palette.run", "Open or run", &[Palette], Quire, &["Enter"]),
    a(84, "palette.back-to-notes", "Back to notes", &[Palette], Quire, &["Backspace"]),
    a(85, "palette.close", "Close the palette", &[Palette], Quire, &["Esc"]),
    // Field.
    a(86, "field.submit", "Submit", &[Field], Quire, &["Enter"]),
    a(87, "field.cancel", "Cancel", &[Field], Quire, &["Esc"]),
    // Help.
    a(88, "help.close", "Close help", &[Help], Adapted, &["Esc", "?", "F1", "Ctrl+/", "Ctrl+?"]),
    // Rewrite.
    a(89, "rewrite.send", "Send", &[Rewrite], Deferred, &["Enter"]),
    a(90, "rewrite.close", "Close", &[Rewrite], Deferred, &["Esc"]),
    a(91, "rewrite.earlier", "Earlier rewrite", &[Rewrite], Deferred, &["Alt+Up"]),
    a(92, "rewrite.later", "Later rewrite", &[Rewrite], Deferred, &["Alt+Down"]),
    a(93, "rewrite.recall", "Earlier instructions", &[Rewrite], Deferred, &["Up", "Down"]),
    // Map, and its inline text input.
    a(103, "map.left", "Select left", M, Adapted, &["Left", "h"]),
    a(104, "map.right", "Select right", M, Adapted, &["Right", "l"]),
    a(105, "map.up", "Select up", M, Adapted, &["Up", "k"]),
    a(106, "map.down", "Select down", M, Adapted, &["Down", "j"]),
    a(107, "map.root", "Root node", M, Adapted, &["Home", "g g"]),
    a(108, "map.edit", "Edit the node", M, Adapted, &["F2", "e"]),
    a(109, "map.edit-subtree", "Edit the branch", M, Adapted, &["E"]),
    a(110, "map.add-child", "Add a child", M, Adapted, &["Tab"]),
    a(111, "map.add-sibling", "Add a sibling", M, Adapted, &["Enter"]),
    a(112, "map.add-sibling-before", "Add a sibling before", M, Adapted, &["Shift+Enter"]),
    a(113, "map.delete", "Delete the branch", M, Adapted, &["Delete", "Backspace"]),
    a(114, "map.toggle-fold", "Fold or unfold", M, Adapted, &["Space", "f"]),
    a(115, "map.toggle-fold-recursive", "Fold or unfold the branch", M, Adapted, &["Shift+Space", "F"]),
    a(116, "map.toggle-task", "Tick the task", M, Adapted, &["x"]),
    a(117, "map.expand-all", "Unfold everything", M, Adapted, &["]"]),
    a(118, "map.collapse-to-level", "Fold to a level", M, Adapted, &["["]),
    a(119, "map.fit", "Fit the map", M, Adapted, &["0", "1", "Shift+1"]),
    a(120, "map.zoom-reset", "Zoom 100%", M, Dropped, &[]),
    a(121, "map.center", "Centre the node", M, Adapted, &["c"]),
    a(122, "map.zoom-in", "Zoom in", M, Adapted, &["+", "="]),
    a(123, "map.zoom-out", "Zoom out", M, Adapted, &["-"]),
    a(124, "map.move-up", "Move up", M, Adapted, &["Alt+Up"]),
    a(125, "map.move-down", "Move down", M, Adapted, &["Alt+Down"]),
    a(126, "map.outdent", "Outdent", M, Adapted, &["Shift+Tab"]),
    a(127, "map.move-right", "Move right", M, Adapted, &["Alt+Right"]),
    a(159, "map.layout", "Map layout", M, Quire, &["L"]),
    a(158, "map.move-left", "Move left", M, Adapted, &["Alt+Left"]),
    a(128, "map.heading-1", "Heading 1", M, Adapted, &["Ctrl+Alt+1"]),
    a(128, "map.heading-2", "Heading 2", M, Adapted, &["Ctrl+Alt+2"]),
    a(128, "map.heading-3", "Heading 3", M, Adapted, &["Ctrl+Alt+3"]),
    a(128, "map.heading-4", "Heading 4", M, Adapted, &["Ctrl+Alt+4"]),
    a(128, "map.heading-5", "Heading 5", M, Adapted, &["Ctrl+Alt+5"]),
    a(128, "map.heading-6", "Heading 6", M, Adapted, &["Ctrl+Alt+6"]),
    a(190, "map.paragraph", "Convert to paragraph", M, Quire, &["Ctrl+Alt+0"]),
    a(191, "map.task", "Convert to task", M, Quire, &["Ctrl+Shift+7"]),
    a(192, "map.bulleted", "Convert to bulleted list", M, Quire, &["Ctrl+Shift+8"]),
    a(193, "map.numbered", "Convert to numbered list", M, Quire, &["Ctrl+Shift+9"]),
    a(129, "map.follow-link", "Follow the node's link", M, Adapted, &["Ctrl+Enter"]),
    a(130, "map.find", "Find", M, Adapted, &["Ctrl+F", "/"]),
    a(131, "map.find-next", "Next match", M, Adapted, &["n", "F3", "Ctrl+G"]),
    a(132, "map.find-previous", "Previous match", M, Adapted, &["N", "Shift+F3", "Ctrl+Shift+G"]),
    a(133, "map.undo", "Undo", M, Adapted, &["Ctrl+Z"]),
    a(134, "map.redo", "Redo", M, Adapted, &["Ctrl+Shift+Z", "Ctrl+Y"]),
    a(175, "map.cut", "Cut the branch", M, Quire, &["Ctrl+X"]),
    a(135, "map.copy-node", "Copy the node's Markdown", M, Adapted, &["Ctrl+C"]),
    a(136, "map.copy-subtree", "Copy the branch's Markdown", M, Adapted, &["Ctrl+Shift+C"]),
    a(137, "map.copy-all", "Copy the whole note", M, Adapted, &["Ctrl+Shift+A"]),
    a(138, "map.paste", "Paste", M, Adapted, &["Ctrl+V"]),
    a(176, "map.toggle-sidebar", "Toggle sidebar", M, Adapted, &["b", "B", "Ctrl+B"]),
    a(140, "map.toggle-outline", "Toggle outline", M, Adapted, &["o", "O"]),
    a(177, "map.mode-preview", "Preview mode", M, Adapted, &["p"]),
    a(178, "map.mode-map", "Map mode", M, Adapted, &["m"]),
    a(139, "map.read-from-disk", "Read from disk", M, Adapted, &["r"]),
    a(179, "map.focus-mode", "Zen mode", M, Adapted, &["z", "."]),
    a(180, "map.settings", "Settings", M, Adapted, &[","]),
    a(181, "map.save", "Save now", M, Adapted, &["s"]),
    a(182, "map.insert-image", "Insert an image", M, Adapted, &["i"]),
    a(141, "map.close-tab", "Close tab", M, Dropped, &[]),
    a(142, "map.help", "Keyboard help", M, Adapted, &["?"]),
    a(143, "map.escape", "Close find or cancel drag", M, Adapted, &["Esc"]),
    a(144, "map.node-menu", "Node menu", M, Adapted, &["Right-click"]),
    a(145, "mapedit.commit", "Commit the edit", ME, Adapted, &["Enter"]),
    a(146, "mapedit.newline", "New line", ME, Adapted, &["Shift+Enter"]),
    a(147, "mapedit.cancel", "Cancel the edit", ME, Adapted, &["Esc"]),
    a(148, "mapedit.commit-add-child", "Commit and add a child", ME, Adapted, &["Tab"]),
    a(149, "mapedit.commit-outdent", "Commit and outdent", ME, Adapted, &["Shift+Tab"]),
    a(150, "mapedit.paragraph", "New node as a paragraph", ME, Adapted, &["Ctrl+Alt+0"]),
    a(151, "mapedit.heading-1", "New node as heading 1", ME, Adapted, &["Ctrl+Alt+1"]),
    a(151, "mapedit.heading-2", "New node as heading 2", ME, Adapted, &["Ctrl+Alt+2"]),
    a(151, "mapedit.heading-3", "New node as heading 3", ME, Adapted, &["Ctrl+Alt+3"]),
    a(151, "mapedit.heading-4", "New node as heading 4", ME, Adapted, &["Ctrl+Alt+4"]),
    a(151, "mapedit.heading-5", "New node as heading 5", ME, Adapted, &["Ctrl+Alt+5"]),
    a(151, "mapedit.heading-6", "New node as heading 6", ME, Adapted, &["Ctrl+Alt+6"]),
    a(155, "mapedit.numbered", "New node as a numbered item", ME, Quire, &["Ctrl+Shift+7"]),
    a(156, "mapedit.bulleted", "New node as a bulleted item", ME, Quire, &["Ctrl+Shift+8"]),
    a(157, "mapedit.task", "New node as a task", ME, Quire, &["Ctrl+Shift+9"]),
    // Board.
    a(196, "board.left", "Select left", B, Adapted, &["Left", "h"]),
    a(197, "board.right", "Select right", B, Adapted, &["Right", "l"]),
    a(198, "board.up", "Select up", B, Adapted, &["Up", "k"]),
    a(199, "board.down", "Select down", B, Adapted, &["Down", "j"]),
    a(200, "board.home", "First column", B, Adapted, &["Home"]),
    a(201, "board.move-up", "Move card up", B, Adapted, &["Alt+Up"]),
    a(202, "board.move-down", "Move card down", B, Adapted, &["Alt+Down"]),
    a(203, "board.move-left", "Move left", B, Adapted, &["Alt+Left"]),
    a(204, "board.move-right", "Move right", B, Adapted, &["Alt+Right"]),
    a(205, "board.toggle-task", "Tick the task", B, Adapted, &["x"]),
    a(206, "board.edit", "Edit card", B, Adapted, &["e", "F2"]),
    a(218, "board.open", "Open card", B, Adapted, &["Enter"]),
    a(219, "board.toggle-fold", "Fold or unfold subtasks", B, Adapted, &["Space"]),
    a(207, "board.delete", "Delete the card", B, Adapted, &["Delete", "Backspace"]),
    a(208, "board.copy", "Copy Markdown", B, Adapted, &["Ctrl+C"]),
    a(209, "board.undo", "Undo", B, Adapted, &["Ctrl+Z"]),
    a(210, "board.redo", "Redo", B, Adapted, &["Ctrl+Shift+Z", "Ctrl+Y"]),
    a(211, "board.follow-link", "Follow the card's link", B, Adapted, &["Ctrl+Enter"]),
    a(212, "board.card-menu", "Card menu", B, Adapted, &["Right-click"]),
    a(213, "board.find", "Find", B, Adapted, &["Ctrl+F", "/"]),
    a(214, "board.find-next", "Next match", B, Adapted, &["n", "F3", "Ctrl+G"]),
    a(215, "board.find-previous", "Previous match", B, Adapted, &["N", "Shift+F3", "Ctrl+Shift+G"]),
    a(216, "board.help", "Keyboard help", B, Adapted, &["?"]),
    a(217, "board.escape", "Close find or cancel drag", B, Adapted, &["Esc"]),
];

pub fn action(id: &str) -> Option<&'static Action> {
    TABLE.iter().find(|a| a.id == id)
}

/// User keybinding overrides loaded from `~/.config/quire/keybindings.json`.
/// Actions absent from `bindings` use their default chords in `TABLE`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub bindings: BTreeMap<String, Vec<String>>,
    /// Monotonic revision bumped on every change, for cache keys.
    pub rev: u64,
}

impl Overrides {
    pub fn from_raw(raw: BTreeMap<String, Vec<String>>) -> Self {
        let mut bindings = BTreeMap::new();
        for (id, chords) in raw {
            let Some(act) = action(&id) else { continue };
            if matches!(act.status, Deferred | Dropped) {
                continue;
            }
            let valid: Vec<String> = chords.into_iter().filter(|s| parse(s).is_some()).collect();
            let def: Vec<String> = act.chords.iter().map(|s| (*s).to_string()).collect();
            if valid != def {
                bindings.insert(id, valid);
            }
        }
        Overrides { bindings, rev: 0 }
    }

    pub fn load() -> Self {
        Self::from_raw(crate::store::load_keybindings())
    }

    pub fn save(&self) -> std::io::Result<()> {
        crate::store::save_keybindings(&self.bindings)
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub fn is_custom(&self, id: &str) -> bool {
        self.bindings.contains_key(id)
    }

    /// Effective chord specs for `a`: user override if set, else `a.chords`.
    pub fn action_chords<'a>(&'a self, a: &'static Action) -> Vec<&'a str> {
        match self.bindings.get(a.id) {
            Some(v) => v.iter().map(String::as_str).collect(),
            None => a.chords.to_vec(),
        }
    }

    /// Effective chord specs for action `id`.
    pub fn chords<'a>(&'a self, id: &str) -> Vec<&'a str> {
        action(id).map_or_else(Vec::new, |a| self.action_chords(a))
    }

    pub fn set(&mut self, id: &str, chords: Vec<String>) {
        let Some(act) = action(id) else { return };
        let valid: Vec<String> = chords.into_iter().filter(|s| parse(s).is_some()).collect();
        let def: Vec<String> = act.chords.iter().map(|s| (*s).to_string()).collect();
        if valid == def {
            self.bindings.remove(id);
        } else {
            self.bindings.insert(id.to_string(), valid);
        }
        self.rev += 1;
    }

    pub fn reset(&mut self, id: &str) {
        if self.bindings.remove(id).is_some() {
            self.rev += 1;
        }
    }

    pub fn reset_all(&mut self) {
        if !self.bindings.is_empty() {
            self.bindings.clear();
            self.rev += 1;
        }
    }

    /// Returns a conflicting action if binding `id` to `spec` would collide in any context
    /// `id` belongs to or shadow/be shadowed by Global.
    pub fn conflict(&self, id: &str, spec: &str) -> Option<&'static Action> {
        let target = action(id)?;
        let seq = parse(spec)?;
        let is_esc = seq.len() == 1 && seq[0].key == K::Esc;
        for other in TABLE.iter().filter(|o| o.id != id && !matches!(o.status, Deferred | Dropped)) {
            let shares_ctx = target.ctx.iter().any(|c| other.ctx.contains(c));
            let shadows = !is_esc
                && ((target.ctx.contains(&Global) && other.ctx.iter().any(|c| falls_to_global(*c)))
                    || (other.ctx.contains(&Global) && target.ctx.iter().any(|c| falls_to_global(*c))));
            if !shares_ctx && !shadows {
                continue;
            }
            if shares_ctx && target.guard.is_some() && other.guard.is_some() && target.guard != other.guard {
                continue;
            }
            if self.action_chords(other).iter().filter_map(|s| parse(s)).any(|o_seq| o_seq == seq) {
                return Some(other);
            }
        }
        None
    }

    /// The action `ev` runs in `ctx`, for chords of a single step.
    pub fn lookup(&self, ctx: Ctx, ev: &KeyEvent) -> Vec<&'static Action> {
        TABLE
            .iter()
            .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
            .filter(|a| self.action_chords(a).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 1 && matches(&seq[0], ev)))
            .collect()
    }

    /// The actions whose two-key chord is `first` then `ev` in `ctx` (`g g`, RD-23).
    pub fn lookup_seq(&self, ctx: Ctx, first: &KeyEvent, ev: &KeyEvent) -> Vec<&'static Action> {
        TABLE
            .iter()
            .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
            .filter(|a| self.action_chords(a).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 2 && matches(&seq[0], first) && matches(&seq[1], ev)))
            .collect()
    }

    /// Whether `ev` starts a two-key chord in `ctx`.
    pub fn starts_seq(&self, ctx: Ctx, ev: &KeyEvent) -> bool {
        TABLE
            .iter()
            .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
            .any(|a| self.action_chords(a).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 2 && matches(&seq[0], ev)))
    }

    /// The first user-facing chord of `id`, labelled, or "" when it has none.
    pub fn label(&self, id: &str) -> String {
        self.chords(id).into_iter().find(|s| !is_display_alias(s)).map(label_of).unwrap_or_default()
    }

    /// A button's tooltip: its name, then its first key in brackets when it has one.
    pub fn tip(&self, name: &str, id: &str) -> String {
        match self.label(id) {
            k if k.is_empty() => name.to_string(),
            k => format!("{name} ({k})"),
        }
    }

    /// Every user-facing chord of `id`, labelled individually (hiding internal shifted-key
    /// aliases like `Ctrl+?`).
    pub fn display_labels(&self, id: &str) -> Vec<String> {
        self.chords(id).into_iter().filter(|s| !is_display_alias(s)).map(label_of).collect()
    }
}

fn falls_to_global(ctx: Ctx) -> bool {
    matches!(ctx, Reader | Writer | Find | Palette | Field | Map | MapEdit | Board)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum K {
    /// A character, lowercase for letters.
    Char(char),
    Enter,
    Tab,
    Esc,
    Space,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

/// One key with its modifiers. Ctrl is Cmd on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: K,
}

/// A chord as the spec writes it, or None for a mouse gesture. "G" is Shift+g, and "Ctrl++"
/// ends in the "+" key. A sequence ("g g") gives one chord per step.
pub fn parse(spec: &str) -> Option<Vec<Chord>> {
    if spec.contains("click") {
        return None;
    }
    spec.split(' ').map(parse_one).collect()
}

fn parse_one(s: &str) -> Option<Chord> {
    let (mods, key) = match s.strip_suffix("++") {
        Some(m) => (m, "+"),
        None if s.len() > 1 && s.contains('+') => s.rsplit_once('+')?,
        None => ("", s),
    };
    let mut c = Chord { ctrl: false, shift: false, alt: false, key: K::Space };
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        match m {
            "Ctrl" => c.ctrl = true,
            "Shift" => c.shift = true,
            "Alt" => c.alt = true,
            _ => return None,
        }
    }
    c.key = match key {
        "Enter" => K::Enter,
        "Tab" => K::Tab,
        "Esc" => K::Esc,
        "Space" => K::Space,
        "Backspace" => K::Backspace,
        "Delete" => K::Delete,
        "Up" => K::Up,
        "Down" => K::Down,
        "Left" => K::Left,
        "Right" => K::Right,
        "Home" => K::Home,
        "End" => K::End,
        "PageUp" => K::PageUp,
        "PageDown" => K::PageDown,
        f if f.len() > 1 && f.starts_with('F') => K::F(f[1..].parse().ok()?),
        ch => {
            let mut it = ch.chars();
            let (ch, None) = (it.next()?, it.next()) else { return None };
            // A plain capital is the letter typed with Shift; with Ctrl the spec writes
            // letters as capitals and says Shift outright.
            if ch.is_ascii_uppercase() && !c.ctrl {
                c.shift = true;
            }
            K::Char(ch.to_ascii_lowercase())
        }
    };
    Some(c)
}

fn named(key: &Key) -> Option<K> {
    Some(match key {
        Key::Enter => K::Enter,
        Key::Tab => K::Tab,
        Key::Escape => K::Esc,
        Key::Backspace => K::Backspace,
        Key::Delete => K::Delete,
        Key::Up => K::Up,
        Key::Down => K::Down,
        Key::Left => K::Left,
        Key::Right => K::Right,
        Key::Home => K::Home,
        Key::End => K::End,
        Key::PageUp => K::PageUp,
        Key::PageDown => K::PageDown,
        Key::Function(n) => K::F(*n),
        Key::Text(t) if t == " " => K::Space,
        Key::Command(' ') => K::Space,
        _ => return None,
    })
}

/// US-layout shifted number-row symbol back to its digit, so `Ctrl+Shift+7` matches both
/// `Key::Command('7')` (scripts/some layouts) and `Key::Command('&')` (live US keyboard).
fn unshift_digit(ch: char) -> Option<char> {
    Some(match ch {
        ')' => '0',
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        _ => return None,
    })
}

/// Whether `ev` is `c`, by KEY-03's rules.
pub fn matches(c: &Chord, ev: &KeyEvent) -> bool {
    let m = ev.mods;
    // Alt never joins a chord that does not name it, so AltGr types text.
    if m.alt != c.alt || (m.logo && !cfg!(target_os = "macos")) {
        return false;
    }
    let ctrl_ok = m.cmd == c.ctrl || (cfg!(target_os = "macos") && c.key == K::Tab && c.ctrl && m.ctrl && !m.cmd);
    if !ctrl_ok {
        return false;
    }
    match c.key {
        K::Char(want) => {
            // Rune delivers AltGr-produced characters on Windows as `Key::Text` with `ctrl+alt`.
            // Never fire a `Ctrl+...` command on `Key::Text` so AltGr can type `# [ ] { } | ~ \`.
            let got = match &ev.key {
                Key::Command(ch) => *ch,
                Key::Text(t) if !c.ctrl && t.chars().count() == 1 => t.chars().next().unwrap_or(' '),
                _ => return false,
            };
            if want.is_ascii_alphanumeric() {
                if c.ctrl {
                    if m.shift != c.shift {
                        return false;
                    }
                    if want.is_ascii_digit() && c.shift {
                        got == want || unshift_digit(got) == Some(want)
                    } else {
                        got.to_ascii_lowercase() == want
                    }
                } else {
                    // The character typed: "G" is Shift+g; "Shift+1" matches '1' or '!'.
                    if want.is_ascii_digit() && c.shift {
                        m.shift && (got == want || unshift_digit(got) == Some(want))
                    } else {
                        got == if c.shift { want.to_ascii_uppercase() } else { want }
                    }
                }
            } else {
                // A symbol is the character the key types, whatever Shift did to get it.
                got == want
            }
        }
        k => named(&ev.key) == Some(k) && m.shift == c.shift,
    }
}

/// Formats a live `KeyEvent` as a canonical keymap spec string for the Settings keybinding
/// recorder, or None for bare modifier presses or unsupported keys.
pub fn format_event(ev: &KeyEvent) -> Option<String> {
    let m = ev.mods;
    if m.logo && !cfg!(target_os = "macos") {
        return None;
    }
    let ctrl = m.cmd || (cfg!(target_os = "macos") && m.ctrl && matches!(ev.key, Key::Tab));
    let mut parts: Vec<String> = Vec::new();
    if ctrl {
        parts.push("Ctrl".into());
    }
    if m.alt {
        parts.push("Alt".into());
    }
    if let Some(k) = named(&ev.key) {
        if m.shift {
            parts.push("Shift".into());
        }
        let name = match k {
            K::Enter => "Enter".into(),
            K::Tab => "Tab".into(),
            K::Esc => "Esc".into(),
            K::Space => "Space".into(),
            K::Backspace => "Backspace".into(),
            K::Delete => "Delete".into(),
            K::Up => "Up".into(),
            K::Down => "Down".into(),
            K::Left => "Left".into(),
            K::Right => "Right".into(),
            K::Home => "Home".into(),
            K::End => "End".into(),
            K::PageUp => "PageUp".into(),
            K::PageDown => "PageDown".into(),
            K::F(n) => format!("F{n}"),
            K::Char(_) => return None,
        };
        parts.push(name);
        return Some(parts.join("+"));
    }
    let raw = match &ev.key {
        Key::Command(ch) => *ch,
        Key::Text(t) if t.chars().count() == 1 => t.chars().next()?,
        _ => return None,
    };
    if raw.is_control() {
        return None;
    }
    if ctrl {
        if m.shift {
            if let Some(d) = unshift_digit(raw) {
                parts.push("Shift".into());
                parts.push(d.to_string());
                return Some(parts.join("+"));
            }
            if raw.is_ascii_alphanumeric() {
                parts.push("Shift".into());
                parts.push(raw.to_ascii_uppercase().to_string());
                return Some(parts.join("+"));
            }
        }
        let key_str = if raw.is_ascii_alphabetic() { raw.to_ascii_uppercase().to_string() } else { raw.to_string() };
        parts.push(key_str);
        return Some(parts.join("+"));
    }
    if m.alt {
        if m.shift {
            parts.push("Shift".into());
        }
        parts.push(raw.to_ascii_uppercase().to_string());
        return Some(parts.join("+"));
    }
    // Plain character: keep capitals ("G") and symbols ("?", "/"), and normalize Shift+digit ("Shift+1").
    if m.shift && let Some(d) = unshift_digit(raw) {
        return Some(format!("Shift+{d}"));
    }
    Some(raw.to_string())
}

/// Default-table label for places with no `App` (such as the sample vault note).
pub fn default_label(id: &str) -> String {
    action(id)
        .and_then(|a| a.chords.iter().find(|s| !is_display_alias(s)).copied())
        .map(label_of)
        .unwrap_or_default()
}

/// A chord's label on this platform: as the spec writes it, or with ⌘⇧⌥ run together on macOS.
pub fn label_of(spec: &str) -> String {
    if !cfg!(target_os = "macos") {
        return spec.to_string();
    }
    if spec == "Ctrl+click" {
        return "⌘-click".into();
    }
    spec.replace("Ctrl+", "⌘").replace("Shift+", "⇧").replace("Alt+", "⌥")
}

/// A platform-formatted primary-modifier chord label for a built-in editing key (e.g. `'x'` ->
/// `"Ctrl+X"` on Linux/Windows, `"⌘X"` on macOS) (`TYPOPOLISH`).
pub fn cmd_label(ch: char) -> String {
    label_of(&format!("Ctrl+{}", ch.to_ascii_uppercase()))
}

/// Internal shifted-key aliases kept for key matching (`Ctrl+?` for `Ctrl+Shift+/`) that
/// should not clutter displayed shortcut lists.
pub fn is_display_alias(spec: &str) -> bool {
    spec == "Ctrl+?"
}

/// Reserved text-field keys that KEY-06 leaves to Rune in text-editing contexts.
pub fn is_reserved_field_key(ctx: Ctx, spec: &str) -> bool {
    if !matches!(ctx, Writer | Sheet | MapEdit | Find | Palette | Field) {
        return false;
    }
    let Some(seq) = parse(spec) else { return false };
    const FIELD_KEYS: &[&str] = &[
        "Up", "Down", "Left", "Right", "Home", "End", "PageUp", "PageDown",
        "Backspace", "Delete", "Ctrl+A", "Ctrl+C", "Ctrl+X", "Ctrl+V",
        "Ctrl+Z", "Ctrl+Shift+Z", "Ctrl+Y",
    ];
    FIELD_KEYS.iter().filter_map(|s| parse(s)).any(|f| f == seq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;
    use std::collections::HashMap;

    fn ev(key: Key, ctrl: bool, shift: bool) -> KeyEvent {
        KeyEvent { key, mods: Mods { cmd: ctrl, ctrl: ctrl && !cfg!(target_os = "macos"), shift, ..Default::default() } }
    }

    fn ev_alt(key: Key, ctrl: bool, alt: bool, shift: bool) -> KeyEvent {
        KeyEvent { key, mods: Mods { cmd: ctrl, ctrl: ctrl && !cfg!(target_os = "macos"), alt, shift, ..Default::default() } }
    }

    fn ids(km: &Overrides, ctx: Ctx, e: &KeyEvent) -> Vec<&'static str> {
        km.lookup(ctx, e).iter().map(|a| a.id).collect()
    }

    fn live(a: &Action) -> bool {
        !matches!(a.status, Dropped)
    }

    fn by_chord() -> HashMap<(Ctx, Vec<Chord>), Vec<&'static Action>> {
        let mut m: HashMap<(Ctx, Vec<Chord>), Vec<&'static Action>> = HashMap::new();
        for a in TABLE.iter().filter(|a| live(a)) {
            for s in a.chords {
                let Some(seq) = parse(s) else { continue };
                for c in a.ctx {
                    m.entry((*c, seq.clone())).or_default().push(a);
                }
            }
        }
        m
    }

    #[test]
    fn every_chord_parses() {
        for a in TABLE {
            for s in a.chords {
                assert!(parse(s).is_some() || s.contains("click"), "{} {s}", a.id);
            }
        }
        assert_eq!(parse("G"), Some(vec![Chord { ctrl: false, shift: true, alt: false, key: K::Char('g') }]));
        assert_eq!(parse("Ctrl++").unwrap()[0].key, K::Char('+'));
        assert_eq!(parse("g g").unwrap().len(), 2);
        assert_eq!(parse("Shift+F3").unwrap()[0], Chord { ctrl: false, shift: true, alt: false, key: K::F(3) });
        assert_eq!(parse("Shift+1").unwrap()[0], Chord { ctrl: false, shift: true, alt: false, key: K::Char('1') });
    }

    /// Checks: no context binds one chord to two actions, unless both have guards that differ.
    #[test]
    fn no_chord_does_two_things() {
        for ((ctx, seq), acts) in by_chord() {
            if acts.len() > 1 {
                let guarded = acts.iter().all(|a| a.guard.is_some()) && acts[0].guard != acts[1].guard;
                assert!(guarded && acts.len() == 2, "{ctx:?} {seq:?}: {:?}", acts.iter().map(|a| a.id).collect::<Vec<_>>());
            }
        }
    }

    /// Checks: every live action has a chord, apart from mouse-only and dropped ones.
    #[test]
    fn every_action_has_a_chord() {
        for a in TABLE.iter().filter(|a| live(a)) {
            assert!(!a.chords.is_empty(), "{} has no chord", a.id);
        }
    }

    /// Checks: no chord in a context that falls through to global shadows a global chord.
    #[test]
    fn no_context_shadows_a_global_chord() {
        let m = by_chord();
        for ((ctx, seq), acts) in &m {
            if !falls_to_global(*ctx) || seq[0].key == K::Esc {
                continue;
            }
            assert!(!m.contains_key(&(Global, seq.clone())), "{ctx:?} {} shadows a global chord", acts[0].id);
        }
    }

    /// Checks: the writer, sheet, map edit, find, palette and field contexts leave the text-field
    /// keys to Rune (KEY-06), apart from the palette's Up, Down and Backspace.
    #[test]
    fn text_field_keys_stay_with_the_field() {
        for a in TABLE.iter().filter(|a| live(a)) {
            for &ctx in a.ctx {
                for &s in a.chords {
                    if is_reserved_field_key(ctx, s) {
                        assert!(matches!(a.id, "palette.next" | "palette.previous" | "palette.back-to-notes"), "{} takes a field key {s}", a.id);
                    }
                }
            }
        }
    }

    /// Checks: the table agrees with `docs/reader-writer/behavior/keymap.md`.
    #[test]
    fn the_table_agrees_with_the_spec() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/reader-writer/behavior/keymap.md");
        let spec = std::fs::read_to_string(path).unwrap();
        let mut seen = 0;
        for line in spec.lines().filter(|l| l.starts_with("- **KEY-")) {
            let item: u16 = line.strip_prefix("- **KEY-").unwrap().split_once("**").unwrap().0.parse().unwrap();
            let Some((_, after_ctx)) = line.split_once("): ") else { continue };
            let Some(before_ctx) = line.split_once("): ").map(|x| x.0) else { continue };
            if !before_ctx.contains('(') {
                continue;
            }
            seen += 1;
            let chords_part = after_ctx.split_once(". ").map_or(after_ctx, |x| x.0);
            let ticks = |s: &str| -> Vec<String> { s.split('`').skip(1).step_by(2).map(String::from).collect() };
            let ctxs: Vec<&str> = before_ctx.rsplit_once('(').unwrap().1.split(", ").collect();
            let status = line.split_once("** [").unwrap().1.split(&[']', ':'][..]).next().unwrap();
            let acts: Vec<&Action> = TABLE.iter().filter(|a| a.item == item).collect();
            assert!(!acts.is_empty(), "KEY-{item} is not in the table");
            if item == 55 || item == 128 || item == 151 {
                assert_eq!(acts.len(), 6);
                for (i, a) in acts.iter().enumerate() {
                    assert_eq!(a.chords, &[format!("Ctrl+Alt+{}", i + 1).as_str()][..]);
                }
                continue;
            }
            if item == 99 {
                assert_eq!(acts.len(), 9);
                for (i, a) in acts.iter().enumerate() {
                    assert_eq!(a.chords, &[format!("Ctrl+{}", i + 1).as_str()][..]);
                }
                continue;
            }
            let a = acts[0];
            assert_eq!(ticks(chords_part), a.chords.to_vec(), "KEY-{item} chords");
            let names: Vec<String> = a.ctx.iter().map(|c| format!("{c:?}").to_lowercase()).collect();
            assert_eq!(ctxs, names, "KEY-{item} contexts");
            let want = match a.status {
                In => "in",
                Adapted => "adapted",
                Quire => "quire",
                Deferred => "deferred",
                Dropped => "dropped",
            };
            assert_eq!(status, want, "KEY-{item} status");
        }
        let items: std::collections::HashSet<u16> = TABLE.iter().map(|a| a.item).collect();
        assert_eq!(items.len(), seen, "the table has no item the spec lacks (table={} spec={seen})", items.len());
    }

    #[test]
    fn key_03_chords_and_input_fixes() {
        let km = Overrides::default();
        assert_eq!(ids(&km, Global, &ev(Key::Command('k'), true, false)), ["app.command-palette"]);
        assert_eq!(ids(&km, Writer, &ev(Key::Command('l'), true, false)), ["writer.link"]);
        assert_eq!(ids(&km, Writer, &ev(Key::Command('l'), true, true)), ["writer.wiki-link"]);
        assert_eq!(ids(&km, Global, &ev(Key::Command('+'), true, true)), ["app.zoom-in"]);
        assert_eq!(ids(&km, Global, &ev(Key::Command('='), true, false)), ["app.zoom-in"]);
        assert_eq!(ids(&km, Global, &ev(Key::Command('/'), true, false)), ["app.help"]);
        assert_eq!(ids(&km, Global, &ev(Key::Command('?'), true, true)), ["app.help"]);
        assert_eq!(ids(&km, Reader, &ev(Key::Text("G".into()), false, true)), ["reader.bottom"]);
        assert_eq!(ids(&km, Reader, &ev(Key::Text("?".into()), false, true)), ["reader.help"]);
        assert_eq!(ids(&km, Reader, &ev(Key::Text(" ".into()), false, true)), ["reader.page-up"]);
        // AltGr on Windows arrives as Key::Text with ctrl+alt: must never fire Ctrl+Alt+digit.
        assert!(ids(&km, Writer, &ev_alt(Key::Text("1".into()), true, true, false)).is_empty(), "AltGr text never fires Ctrl+Alt+1");
        assert_eq!(ids(&km, Writer, &ev_alt(Key::Command('1'), true, true, false)), ["writer.heading-1"]);
        // Shifted digits match both '7'/'8'/'9' and '&'/'*'/'('.
        assert_eq!(ids(&km, Writer, &ev(Key::Command('7'), true, true)), ["writer.numbered-list"]);
        assert_eq!(ids(&km, Writer, &ev(Key::Command('&'), true, true)), ["writer.numbered-list"]);
        assert_eq!(ids(&km, Writer, &ev(Key::Command('*'), true, true)), ["writer.bulleted-list"]);
        assert_eq!(ids(&km, Writer, &ev(Key::Command('('), true, true)), ["writer.task-list"]);
        // Map Shift+1 Fit matches both '1' and '!'.
        assert_eq!(ids(&km, Map, &ev(Key::Text("!".into()), false, true)), ["map.fit"]);
        assert_eq!(ids(&km, Map, &ev(Key::Text("1".into()), false, false)), ["map.fit"]);
        assert_eq!(ids(&km, Map, &ev(Key::Text("0".into()), false, false)), ["map.fit"]);
    }

    #[test]
    fn overrides_set_conflict_and_reset() {
        let mut km = Overrides::default();
        assert_eq!(km.conflict("app.command-palette", "Ctrl+B").map(|a| a.id), Some("reader.toggle-sidebar"));
        assert!(km.conflict("app.command-palette", "Ctrl+Shift+P").is_none());
        km.set("app.command-palette", vec!["Ctrl+Shift+P".into()]);
        assert!(km.is_custom("app.command-palette"));
        assert_eq!(ids(&km, Global, &ev(Key::Command('p'), true, true)), ["app.command-palette"]);
        assert!(ids(&km, Global, &ev(Key::Command('k'), true, false)).is_empty());
        km.reset("app.command-palette");
        assert!(!km.is_custom("app.command-palette"));
        assert_eq!(ids(&km, Global, &ev(Key::Command('k'), true, false)), ["app.command-palette"]);
    }

    #[test]
    fn format_event_round_trips() {
        assert_eq!(format_event(&ev(Key::Command('k'), true, false)).as_deref(), Some("Ctrl+K"));
        assert_eq!(format_event(&ev(Key::Command('&'), true, true)).as_deref(), Some("Ctrl+Shift+7"));
        assert_eq!(format_event(&ev_alt(Key::Command('1'), true, true, false)).as_deref(), Some("Ctrl+Alt+1"));
        assert_eq!(format_event(&ev(Key::Function(9), false, false)).as_deref(), Some("F9"));
        assert_eq!(format_event(&ev(Key::Text("b".into()), false, false)).as_deref(), Some("b"));
        assert_eq!(format_event(&ev(Key::Text("G".into()), false, true)).as_deref(), Some("G"));
        assert_eq!(format_event(&ev(Key::Text("!".into()), false, true)).as_deref(), Some("Shift+1"));
    }
}
