# Keymap

Every key Quire's desktop app answers goes through one keymap (decision 0003). The keymap is a
table from a context and a chord to a named action. Quire ships two presets of that table,
**omarky** and **quire**, and Settings, Shortcuts picks one. This file lists every action with its
chord in each preset. The behaviour each action runs is in `reader.md`, `writer.md`,
`edit-sheet.md` and `page-and-typography.md`. Those files write keys as in the omarky preset.

Notes on reading this file:

- Ctrl means Cmd on macOS, in both presets. Labels follow Quire's `keys::k`, so `Ctrl+Shift+F`
  shows as `⌘⇧F` on macOS.
- Action IDs are kebab-case with a context prefix. `app.` actions are global.
- An item's context in parentheses says where the chord acts. `(writer, sheet)` means the same
  action and chord in both contexts.
- Status follows `README.md`, read for keys. `in` means omarky's chord in both presets.
  `adapted: 0003` means a preset moves the chord away from omarky's or Quire's. `quire` means a
  Quire action that keeps Quire's chord in both presets. `deferred` items keep their chords
  reserved, so no other action takes them before changes mode, rewrite and spelling land.
- A mouse-only action has a pointer gesture instead of a key. It is in the table so the help
  sheet can list it.
- Palette commands with no key (new folder, today's journal, theme, rename, delete, sync now,
  lock, switch folder, restore sample) are not keymap actions. They stay in the palette.

## Rules

- **KEY-01** [adapted: 0003] [verify: unit, state] The keymap has these contexts. Global. Reader, which is preview mode with no control focused. Writer, which is the source editor field while it has focus. Sheet, the edit sheet. Find, the find bar's field. Palette, the command palette and quick switcher field. Field, the single-line fields (rename in the breadcrumb, new folder in the tree, lock password, welcome, sync settings). Help, the shortcut sheet in either mode. Rewrite, the rewrite line, which is deferred.
- **KEY-02** [adapted: 0003] [verify: unit, script] A key goes first to the focused control, which keeps its own text keys (KEY-06). Then it goes to the topmost context. Help and sheet are modal, so a key they do not bind stops there (RD-80, RD-81, WR-159, SHEET-014, SHEET-036). The toast's Undo key (KEY-94) is the one exception: the toast is not a layer, so its key acts whatever is open. Find falls through to the mode's context (reader or writer) and then to global, but the writer's edits at the caret do nothing from it (KEY-98). Palette and field fall through to global only, so an editing key never changes a note hidden behind them. Reader and writer fall through to global. A focused stop owns the plain keys, and a shortcut with Ctrl, Alt or the logo key goes on from it as from the note (KEY-98).
- **KEY-03** [adapted: 0003] [verify: unit] A chord with Ctrl on a letter or digit matches that key with exactly those modifiers, so `Ctrl+K` never fires on `Ctrl+Shift+K`. A chord on a symbol matches the character the key produces, with Shift ignored. So `Ctrl+Shift+=` gives `+` and fires the `Ctrl++` binding, and `Ctrl+Shift+/` gives `?` and fires `Ctrl+?` (WR-72). omarky's paired symbols (`=` and `+`, `/` and `?`) are both bound for that reason. A plain reader key matches the character typed, so `G` is Shift+g. Reader keys with Alt or Super are ignored (RD-83). Ctrl+Alt chords are never commands (WR-78). omarky's reader also took its Ctrl letters with Shift held, so `Ctrl+Shift+F` found text there. That accident is not ported.
- **KEY-04** [in] [verify: unit, script] A sequence such as `g g` waits up to 900 ms for its second key. The first `g` does nothing on its own. Any other key cancels the wait and then acts as itself (RD-23).
- **KEY-05** [adapted: 0003] [verify: unit, state] Every key label is generated from the active preset. That covers the reader's help sheet (RD-90), the writer's help sheet (WR-166), the sheet's key row and help rows (SHEET-030, SHEET-111), button tooltips (RD-34), the shortcut column of the command palette (which `commands.rs` hard-codes today), and the list on Settings, Shortcuts. Switching the preset changes all of them at once, without a restart.
- **KEY-06** [in] [verify: script] Text fields keep Rune's caret, selection, clipboard and undo keys (arrows, Home, End, PageUp, PageDown, Backspace, Delete, Ctrl+A, Ctrl+C, Ctrl+X, Ctrl+V and Rune's undo and redo). The keymap binds none of them in the writer, sheet, find, palette or field contexts, except the palette's Up, Down and Backspace (KEY-81, KEY-82, KEY-84). Enter, Tab and Esc are not on this list, so the writer and the sheet may take them. The editor's right-click menu shows Cut, Copy and Paste with these keys (WR-135, SHEET-032).
- **KEY-07** [adapted: 0003] [verify: state] Settings, Shortcuts picks the preset and lists every action with its chord in the active preset. The choice is remembered with Quire's settings. The default is omarky (decision 0003, open). A user file that overrides single bindings is a later step.
- **KEY-95** [quire] [verify: -] The side panels' lists take their keys outside the keymap, as Rune's focus groups do. They are the file tree, the Open list, the search results, the tags, and the outline's headings and backlinks. Each list is one Tab stop: Tab enters it at its current row (the open note, the heading being read), else at its first, and the next Tab leaves it. Up and Down move between rows and stop at the ends. Home and End go to the first and last row. In the tree, Right opens a closed folder or moves to its first row, and Left closes an open folder or moves to the row's folder. Enter or Space does what a click does: it opens the note, opens or closes the folder, jumps to the heading, opens the note at the match, or shows the tag's notes with the keyboard on them. A row that shows a note then gives the keyboard to the note. The Menu key or Shift+F10 opens the row's menu below the row, where it has one, with its first item marked. Esc gives the keyboard back to the note. The focused row wears the focus ring, apart from the open row's fill, and the list scrolls to show the whole ring, never cut by the list's edge. A click on a row leaves the keyboard where it was.
- **KEY-98** [quire] [verify: -] One rule holds for every focus stop: a side list's row, the folder's name, the panel tabs, the new folder button, the search field and its clear button, New note, Settings, and the formatting toolbar's buttons. Tab reaches each of them. The title bar's buttons are pointer targets, each with its own shortcut, and are not Tab stops. Each panel tab is a Tab stop, Left and Right move between them, and Enter or Space opens one and puts the keyboard in it: the search field for SEARCH, the list's current row for FILES and TAGS. The toolbar is one Tab stop, with Left and Right inside it. A focused stop owns the plain keys: its own keys act (Space, Enter, the arrows, a letter that jumps to a row in a list), then Esc, which gives the keyboard back to the note, then F6. The key that shows or hides the stop's pane hides it. Every other plain key is dropped, so none reaches the reader or the writer behind it. A shortcut with Ctrl, Alt or the logo key acts exactly as it would with the keyboard on the note, in both modes and at every width: Ctrl+F opens find, Ctrl+B bolds in the writer. The editor's own chords (undo, redo, select all, cut, copy and paste) act there too: in the writer, from a stop that is not a text field, the drawer closes, the editor takes the keyboard and the chord acts in it. A text field is the exception: in the search field, find's field, the note's name and a new folder's name, the field's own editing keys work, and the writer's edits at the caret (a format, a heading, a link, indent, following a link) do nothing. The window's shortcuts, such as find, the palette, Settings, the mode and zen, still act. When the element that holds focus goes away, whatever the reason, the keyboard goes to the note: the editor in source, the reader in preview. Down in the search field moves to the first result, and Up from the first result goes back. Esc in the search field gives the keyboard to the note; in the sidebar's drawer that Esc closes the drawer, as it does from a row. A name field that opens behind the drawer (the note's name, from Rename or New note, by a menu, a button or a key) closes the drawer and takes the keyboard. A new folder's name shows inside the drawer, so the drawer stays, and when the name is entered or dropped a row of the tree has the keyboard with its ring: the new folder's row after Enter, and after Esc the row the field was opened from, the folder's row for a New folder here, else the tree's current row. After a layer closes, a stop it hands focus back to shows its ring. Once the note has the keyboard, however it got it, Tab goes on from the note: into its links in preview. Tab or F6 into the writer leaves the caret where it was, in view. Enter on a row that shows a note gives the note the keyboard and closes the drawer the row is in, even when that note was open already. A search match opens in the writer with the caret at its start and nothing selected.

## Global

- **KEY-08** [adapted: 0003] [verify: unit, script] app.command-palette (global): omarky preset `Ctrl+Shift+K`; quire preset `Ctrl+P`. Opens Quire's command palette on its commands list. In the omarky preset `Ctrl+P` and `Ctrl+Shift+P` both switch modes (WR-69), so the palette takes a chord that neither app uses.
- **KEY-09** [quire] [verify: -] app.quick-switcher (global): omarky preset `Ctrl+O`; quire preset `Ctrl+O`. Opens Quire's quick switcher of notes, recent first. omarky's file chooser is not ported (RD-82, RD-95, WR-67).
- **KEY-10** [adapted: 0003] [verify: unit, script] app.search-notes (global): omarky preset `Ctrl+Shift+G`; quire preset `Ctrl+Shift+F`. Opens search of all notes in the sidebar. In the omarky preset `Ctrl+Shift+F` is paragraph focus (WR-75), so search moves (RD-52).
- **KEY-11** [adapted: 0003] [verify: unit, script] app.toggle-sidebar (global): omarky preset `Ctrl+Shift+B`; quire preset `Ctrl+\`. Shows or hides the file tree. In the omarky preset `Ctrl+\` is the outline (RD-41, WR-74).
- **KEY-12** [adapted: 0003] [verify: unit, script] app.toggle-outline (global): omarky preset `Ctrl+\`; quire preset `Ctrl+Shift+O`. Shows or hides Quire's outline panel on the right, in both modes (RD-41, RD-82, WR-74, WR-95). It gives the palette's keyless "Show outline" command a key. The quire preset avoids `Ctrl+Shift+\`, because a symbol chord matches its character (KEY-03). Shift turns `\` into `|` on a US layout and into other characters elsewhere.
- **KEY-13** [adapted: 0003] [verify: unit, script] app.toggle-mode (global): omarky preset `Ctrl+P` or `Ctrl+Shift+P`; quire preset `Ctrl+E`. Switches the note between source and preview, keeping the place both ways (WR-69, WR-107, WR-108, WR-111, WR-112). In the omarky preset this is omarky's preview round trip. In the quire preset `Ctrl+Shift+P` stays unbound.
- **KEY-14** [quire] [verify: -] app.settings (global): omarky preset `Ctrl+,`; quire preset `Ctrl+,`. Opens or closes Settings.
- **KEY-15** [quire] [verify: -] app.focus-mode (global): omarky preset `Ctrl+.`; quire preset `Ctrl+.`. Enters or leaves Quire's focus mode, which hides the panels (WR-223).
- **KEY-16** [quire] [verify: -] app.new-note (global): omarky preset `Ctrl+N`; quire preset `Ctrl+N`. Makes a note named "Untitled" and opens it in a tab. omarky's new window is not ported (WR-66).
- **KEY-17** [quire] [verify: -] app.quit (global): omarky preset `Ctrl+Q`; quire preset `Ctrl+Q`. Quits on Linux and Windows. On macOS the app menu's Cmd+Q quits instead.
- **KEY-18** [quire] [verify: -] app.insert-image (global): omarky preset `Ctrl+Shift+I`; quire preset `Ctrl+Shift+I`. Picks an image and links it into the active note.
- **KEY-19** [in] [verify: unit, script] app.save (global): omarky preset `Ctrl+S`; quire preset `Ctrl+S`. Writes the active note now and toasts "Saved", in either mode (WR-64, WR-111).
- **KEY-20** [in] [verify: unit, script] app.close-tab (global): omarky preset `Ctrl+W`; quire preset `Ctrl+W`. Writes any unsaved text and closes the active tab. The window stays (RD-82, WR-68).
- **KEY-21** [in] [verify: unit, script] app.zoom-in (global): omarky preset `Ctrl+=` or `Ctrl++`; quire preset `Ctrl+=` or `Ctrl++`. Steps the text size up and remembers it, in both modes (RD-82, WR-72, WR-126, PAGE-19).
- **KEY-22** [in] [verify: unit, script] app.zoom-out (global): omarky preset `Ctrl+-`; quire preset `Ctrl+-`. Steps the text size down (RD-82, WR-72, WR-126, PAGE-19).
- **KEY-23** [quire] [verify: -] app.escape (global): omarky preset `Esc`; quire preset `Esc`. Runs Quire's `App::escape`. An open layer (the palette, Settings, the lock dialog, help, the edit sheet, a drawer or a menu) takes Esc first and closes, and focus goes back where it was. With none open, Esc closes new folder, rename and the create prompt, then leaves focus mode. Each context's own Esc runs first and falls through to it (RD-85, WR-77).
- **KEY-94** [quire] [verify: -] app.undo-delete (global): omarky preset `Ctrl+Shift+R`; quire preset `Ctrl+Shift+R`. While a delete's toast offers Undo, puts the note or folder back (`folders::Deleted`). The toast names the key. Text fields keep `Ctrl+Z`, so the delete takes a chord of its own. R is for restore.
- **KEY-96** [quire] [verify: -] app.next-pane (global): omarky preset `F6`; quire preset `F6`. Moves the keyboard to the next pane that shows, left to right, and round: the sidebar's current list, the note, the outline's current heading (KEY-98). The note's links and the toolbar are not panes, so F6 skips them.
- **KEY-97** [quire] [verify: -] app.previous-pane (global): omarky preset `Shift+F6`; quire preset `Shift+F6`. Moves the keyboard to the pane before, as KEY-96 does the other way.

## Reader

- **KEY-24** [in] [verify: unit, script] reader.scroll-down (reader): omarky preset `j` or `Down`; quire preset `j` or `Down`. Glides down one line step (RD-20, RD-22).
- **KEY-25** [in] [verify: unit, script] reader.scroll-up (reader): omarky preset `k` or `Up`; quire preset `k` or `Up`. Glides up one line step (RD-20, RD-22).
- **KEY-26** [in] [verify: unit, script] reader.page-down (reader): omarky preset `Space` or `PageDown`; quire preset `Space` or `PageDown`. Glides down one page step (RD-21).
- **KEY-27** [in] [verify: unit, script] reader.page-up (reader): omarky preset `Shift+Space` or `PageUp`; quire preset `Shift+Space` or `PageUp`. Glides up one page step (RD-21).
- **KEY-28** [in] [verify: unit, script] reader.top (reader): omarky preset `g g` or `Home`; quire preset `g g` or `Home`. Travels to the top of the note (RD-23, RD-24, KEY-04).
- **KEY-29** [in] [verify: unit, script] reader.bottom (reader): omarky preset `G` or `End`; quire preset `G` or `End`. Travels to the end of the note (RD-23, RD-24).
- **KEY-30** [in] [verify: unit, script] reader.toggle-outline (reader): omarky preset `o` or `O`; quire preset `o` or `O`. Runs app.toggle-outline (RD-41).
- **KEY-31** [in] [verify: unit, script] reader.toggle-wide (reader): omarky preset `w` or `W`; quire preset `w` or `W`. Switches between the reading measure and the wide measure (RD-51).
- **KEY-32** [in] [verify: unit, script] reader.find (reader): omarky preset `Ctrl+F` or `/`; quire preset `Ctrl+F` or `/`. Opens the find bar with focus in its field (RD-52, WR-124).
- **KEY-33** [in] [verify: unit, script] reader.find-next (reader): omarky preset `n`; quire preset `n`. Goes to the next match. It acts only while a query exists (RD-58).
- **KEY-34** [in] [verify: unit, script] reader.find-previous (reader): omarky preset `N`; quire preset `N`. Goes to the previous match. It acts only while a query exists (RD-58).
- **KEY-35** [in] [verify: unit, script] reader.copy-selection (reader): omarky preset `Ctrl+C`; quire preset `Ctrl+C`. Copies the selected page text and toasts "Copied" (RD-67).
- **KEY-36** [in] [verify: unit, script] reader.copy-block-under-pointer (reader): omarky preset `Ctrl+Shift+C`; quire preset `Ctrl+Shift+C`. Copies the Markdown source of the block under the pointer (RD-69, RD-100).
- **KEY-37** [in] [verify: unit, script] reader.copy-all (reader): omarky preset `Ctrl+Shift+A`; quire preset `Ctrl+Shift+A`. Copies the whole note and toasts "Copied the whole note" (RD-70).
- **KEY-38** [in] [verify: unit, script] reader.block-menu (reader): omarky preset `Right-click`; quire preset `Right-click`. Opens the clicked block's menu: copy its Markdown or text, or edit it or its section (RD-68, RD-90). From the keyboard the Menu key or Shift+F10 opens it while no control has focus, outside the keymap. With a control focused they open that control's menu, such as a tree row's (KEY-95).
- **KEY-39** [in] [verify: unit, script] reader.read-from-disk (reader): omarky preset `Ctrl+R`; quire preset `Ctrl+R`. Reads the note from disk now and keeps the place (RD-18).
- **KEY-40** [in] [verify: unit, script] reader.zoom-reset (reader): omarky preset `Ctrl+0`; quire preset `Ctrl+0`. Resets the text size to 14 px (RD-82, PAGE-19). In the writer `Ctrl+0` clears a heading instead (KEY-56).
- **KEY-41** [in] [verify: unit, script] reader.edit-block (reader): omarky preset `e`; quire preset `e`. Opens the edit sheet on one block (RD-78, SHEET-001, SHEET-002).
- **KEY-42** [in] [verify: unit, script] reader.edit-section (reader): omarky preset `E`; quire preset `E`. Opens the edit sheet on the block's section (RD-78, SHEET-001, SHEET-009).
- **KEY-43** [in] [verify: unit, script] reader.help (reader): omarky preset `?`; quire preset `?`. Opens the help sheet (RD-86).
- **KEY-44** [in] [verify: unit, script] reader.close-tab (reader): omarky preset `q`; quire preset `q`. Runs app.close-tab (RD-84).
- **KEY-45** [in] [verify: unit, script] reader.escape (reader): omarky preset `Esc`; quire preset `Esc`. Closes find, else clears the selection, else falls through to app.escape, else returns to source mode (RD-61, RD-72, RD-85, WR-111).
- **KEY-46** [deferred] [verify: -] reader.versions (reader): omarky preset `v` or `V`; quire preset `v` or `V`. Opens the version list in changes mode (RD-84).
- **KEY-47** [deferred] [verify: -] reader.fold (reader): omarky preset `f` or `F`; quire preset `f` or `F`. Folds unchanged text in changes mode (RD-84).
- **KEY-48** [deferred] [verify: -] reader.next-change (reader): omarky preset `n`; quire preset `n`. Steps to the next change. It acts only in changes mode with no find query, so it never meets reader.find-next (RD-84).
- **KEY-49** [deferred] [verify: -] reader.previous-change (reader): omarky preset `p`; quire preset `p`. Steps to the previous change in changes mode (RD-84).

## Writer

The editing actions also act in the sheet, because the sheet's editor is the writer's
(SHEET-035).

- **KEY-50** [in] [verify: unit, script] writer.bold (writer, sheet): omarky preset `Ctrl+B`; quire preset `Ctrl+B`. Toggles `**` by `format.rs`, as one undo step (WR-51, WR-52, WR-55, WR-56, WR-63).
- **KEY-51** [in] [verify: unit, script] writer.italic (writer, sheet): omarky preset `Ctrl+I`; quire preset `Ctrl+I`. Toggles `*` (WR-51, WR-56, WR-63).
- **KEY-52** [adapted: 0003] [verify: unit, script] writer.inline-code (writer, sheet): omarky preset `Ctrl+E`; quire preset `Ctrl+Shift+E`. Toggles a backtick pair, or a fence across lines (WR-51, WR-56). In the quire preset `Ctrl+E` is the mode switch.
- **KEY-53** [in] [verify: unit, script] writer.strikethrough (writer, sheet): omarky preset `Ctrl+Shift+X`; quire preset `Ctrl+Shift+X`. Toggles `~~` (WR-51).
- **KEY-54** [in] [verify: unit, script] writer.link (writer, sheet): omarky preset `Ctrl+K`; quire preset `Ctrl+K`. Makes a link from the selection, ready for the URL or the text (WR-57, WR-58).
- **KEY-55** [in] [verify: unit, script] writer.heading-1 to writer.heading-6 (writer, sheet): omarky preset `Ctrl+1` to `Ctrl+6`; quire preset `Ctrl+1` to `Ctrl+6`. Six actions that set the line's heading level, or clear it when it is already that level (WR-59, WR-60, WR-61).
- **KEY-56** [in] [verify: unit, script] writer.plain-paragraph (writer, sheet): omarky preset `Ctrl+0`; quire preset `Ctrl+0`. Clears the line's heading (WR-60, WR-62).
- **KEY-57** [in] [verify: unit, script] writer.indent (writer, sheet): omarky preset `Tab`; quire preset `Tab`. Indents a list item by its marker's step, else inserts 4 spaces (WR-47, WR-48, WR-49).
- **KEY-58** [in] [verify: unit, script] writer.outdent (writer, sheet): omarky preset `Shift+Tab`; quire preset `Shift+Tab`. Outdents a list item, else does nothing and is consumed (WR-50).
- **KEY-59** [in] [verify: unit, script] writer.continue-block (writer, sheet): omarky preset `Enter`; quire preset `Enter`. Continues or ends a list item or quote, else inserts a newline. Shift+Enter, Alt+Enter and Enter over a selection stay the field's plain newline (WR-39 to WR-45).
- **KEY-60** [quire] [verify: -] writer.follow-link (writer): omarky preset `Ctrl+Enter`; quire preset `Ctrl+Enter`. Follows the note link at the caret (WR-46, WR-220). In the sheet `Ctrl+Enter` saves instead (KEY-75).
- **KEY-61** [quire] [verify: -] writer.follow-link-click (writer): omarky preset `Ctrl+click`; quire preset `Ctrl+click`. Mouse-only. Follows the clicked note link (WR-220).
- **KEY-62** [in] [verify: unit, script] writer.find (writer): omarky preset `Ctrl+F`; quire preset `Ctrl+F`. Opens the find bar, seeded with the selection (WR-70, WR-115).
- **KEY-63** [in] [verify: unit, script] writer.find-next (writer): omarky preset `Ctrl+G` or `F3`; quire preset `Ctrl+G` or `F3`. Goes to the next match, even with the bar hidden (WR-70, WR-120).
- **KEY-64** [in] [verify: unit, script] writer.find-previous (writer): omarky preset `Shift+F3`; quire preset `Shift+F3`. Goes to the previous match (WR-70, WR-120).
- **KEY-65** [in] [verify: unit, script] writer.recent-notes (writer): omarky preset `Ctrl+R`; quire preset `Ctrl+R`. Opens the quick switcher, recent notes first (WR-71).
- **KEY-66** [in] [verify: unit, script] writer.help (writer): omarky preset `Ctrl+/` or `Ctrl+?`; quire preset `Ctrl+/` or `Ctrl+?`. Toggles the help sheet (WR-72, WR-73, WR-158, KEY-03).
- **KEY-67** [adapted: 0003] [verify: unit, script] writer.paragraph-focus (writer): omarky preset `Ctrl+Shift+F`; quire preset `Ctrl+Shift+D`. Toggles the fade of every paragraph but the current one (WR-75, WR-79). In the quire preset `Ctrl+Shift+F` searches all notes. D stands for dim.
- **KEY-68** [in] [verify: unit, script] writer.typewriter (writer): omarky preset `Ctrl+Shift+T`; quire preset `Ctrl+Shift+T`. Toggles typewriter scrolling (WR-75, WR-85).
- **KEY-69** [in] [verify: unit, script] writer.next-font (writer): omarky preset `Ctrl+Shift+M`; quire preset `Ctrl+Shift+M`. Steps the source font to the next iA Writer family (WR-75, WR-125).
- **KEY-70** [in] [verify: unit, script] writer.escape (writer): omarky preset `Esc`; quire preset `Esc`. Closes the find bar, else falls through to app.escape, else shows the preview. An open menu or the help sheet is a layer and takes Esc before it (WR-77).
- **KEY-71** [deferred] [verify: -] writer.spelling (writer): omarky preset `Ctrl+Shift+L`; quire preset `Ctrl+Shift+L`. Toggles spell checking (WR-75, WR-128).
- **KEY-72** [deferred] [verify: -] writer.rewrite (writer, sheet): omarky preset `Ctrl+J`; quire preset `Ctrl+J`. Opens the rewrite line on the block or selection (WR-76, WR-167, SHEET-034).
- **KEY-73** [deferred] [verify: -] writer.rewrite-section (writer, sheet): omarky preset `Ctrl+Shift+J`; quire preset `Ctrl+Shift+J`. Opens the rewrite line on the section (WR-76, WR-167, SHEET-034).
- **KEY-74** [dropped] [verify: -] writer.save-as (writer): omarky preset unbound; quire preset unbound. Quire has no Save As, so omarky's `Ctrl+Shift+S` is free in both presets (WR-65).

## Sheet

- **KEY-75** [in] [verify: unit, script] sheet.save (sheet): omarky preset `Ctrl+Enter` or `Ctrl+S`; quire preset `Ctrl+Enter` or `Ctrl+S`. Saves the sheet's text into the note and closes it (SHEET-033, SHEET-044, SHEET-045, SHEET-048).
- **KEY-76** [in] [verify: unit, script] sheet.cancel (sheet): omarky preset `Esc`; quire preset `Esc`. Closes the menu, else closes an unchanged sheet, else arms a discard that a second Esc within 2000 ms carries out (SHEET-037, SHEET-038, SHEET-039).

## Find

- **KEY-77** [in] [verify: unit, script] find.next (find): omarky preset `Enter`; quire preset `Enter`. Goes to the next match, wrapping (RD-58, WR-119).
- **KEY-78** [in] [verify: unit, script] find.previous (find): omarky preset `Shift+Enter`; quire preset `Shift+Enter`. Goes to the previous match, wrapping (RD-58, WR-119).
- **KEY-79** [in] [verify: unit, script] find.close (find): omarky preset `Esc`; quire preset `Esc`. Closes the bar and returns focus. In preview it also clears the query and highlights (RD-61). In source the query stays for `Ctrl+G` (WR-119, WR-120).
- **KEY-80** [in] [verify: unit, script] find.select-all (find): omarky preset `Ctrl+F`; quire preset `Ctrl+F`. Selects all of the field's text (RD-52, WR-119).

## Palette

- **KEY-81** [quire] [verify: -] palette.next (palette): omarky preset `Down`; quire preset `Down`. Moves to the next row and scrolls it into view.
- **KEY-82** [quire] [verify: -] palette.previous (palette): omarky preset `Up`; quire preset `Up`. Moves to the previous row.
- **KEY-83** [quire] [verify: -] palette.run (palette): omarky preset `Enter`; quire preset `Enter`. Opens the note or runs the command on the current row.
- **KEY-84** [quire] [verify: -] palette.back-to-notes (palette): omarky preset `Backspace`; quire preset `Backspace`. On an empty commands query, goes back to the note list. Typing a leading `>` in the note list switches to commands, which is text and not a chord.
- **KEY-85** [quire] [verify: -] palette.close (palette): omarky preset `Esc`; quire preset `Esc`. Closes the palette.

## Field

- **KEY-86** [quire] [verify: -] field.submit (field): omarky preset `Enter`; quire preset `Enter`. Commits a rename or a new folder, and submits the lock, welcome and sync forms.
- **KEY-87** [quire] [verify: -] field.cancel (field): omarky preset `Esc`; quire preset `Esc`. Cancels a rename or a new folder.

## Help

- **KEY-88** [in] [verify: unit, script] help.close (help): omarky preset `Esc`, `?`, `q`, `Ctrl+/` or `Ctrl+?`; quire preset `Esc`, `?`, `q`, `Ctrl+/` or `Ctrl+?`. Closes the help sheet in either mode. Every other key is swallowed (RD-80, WR-158, WR-159).

## Rewrite

- **KEY-89** [deferred] [verify: -] rewrite.send (rewrite): omarky preset `Enter`; quire preset `Enter`. Sends the instruction, or takes the ready rewrite on an empty line (SHEET-080).
- **KEY-90** [deferred] [verify: -] rewrite.close (rewrite): omarky preset `Esc`; quire preset `Esc`. Stops Claude while it works, else closes the line (SHEET-079).
- **KEY-91** [deferred] [verify: -] rewrite.earlier (rewrite): omarky preset `Alt+Up`; quire preset `Alt+Up`. Shows the earlier rewrite (SHEET-081).
- **KEY-92** [deferred] [verify: -] rewrite.later (rewrite): omarky preset `Alt+Down`; quire preset `Alt+Down`. Shows the later rewrite (SHEET-081).
- **KEY-93** [deferred] [verify: -] rewrite.recall (rewrite): omarky preset `Up` or `Down`; quire preset `Up` or `Down`. Steps through remembered instructions (SHEET-082).

## Conflict table

Each row is a chord that omarky and Quire bind to different things today.

| Chord | omarky | Quire today | omarky preset | quire preset |
| --- | --- | --- | --- | --- |
| `Ctrl+E` | Inline code (writer, sheet) | Switch source and preview | Inline code. The mode switch moves to `Ctrl+P` | Mode switch. Inline code moves to `Ctrl+Shift+E` |
| `Ctrl+P` | Preview and back (writer) | Command palette | Mode switch. The palette moves to `Ctrl+Shift+K` | Palette. The mode switch stays on `Ctrl+E` |
| `Ctrl+Shift+F` | Paragraph focus (writer) | Search all notes | Paragraph focus. Search moves to `Ctrl+Shift+G` | Search. Paragraph focus moves to `Ctrl+Shift+D` |
| `Ctrl+\` | Outline | Sidebar | Outline. The sidebar moves to `Ctrl+Shift+B` | Sidebar. The outline moves to `Ctrl+Shift+O` |
| `Ctrl+Enter` | Save the edit sheet | Follow the note link at the caret | Both. The sheet and the writer are separate contexts | Both, as in the omarky preset |
| `Ctrl+N` | New window on a draft | New note | New note | New note |
| `Ctrl+O` | System file chooser | Quick switcher | Quick switcher | Quick switcher |
| `Tab` | Indent a list item by its marker, else 4 spaces | Two spaces at the caret | omarky's rule (WR-47) | omarky's rule |
| `Shift+Tab` | Outdent a list item | Swallowed | omarky's rule (WR-50) | omarky's rule |
| `Enter` | Continue lists and quotes | Continue lists | omarky's rule (WR-39 to WR-45) | omarky's rule |
| `Esc` | Writer closes menu, help, find, then preview. Reader closes find, then the selection | Closes palette, Settings, menus, rename and new folder, then leaves focus mode | One chain. The context's steps, then Quire's, then the other mode: preview from source, source from preview | Same chain |

`Ctrl+B`, `Ctrl+I` and `Ctrl+K` mean the same in both apps. Both presets run Quire's `format.rs`
edge cases with omarky's link rule (WR-56, WR-57). Inside omarky `Ctrl+0` resets zoom in the
reader and clears a heading in the writer. The keymap keeps both, in their own contexts.

The moved chords were chosen so that none is used by either app or by the other preset for
something else. `Ctrl+Shift+K`, `Ctrl+Shift+G` and `Ctrl+Shift+B` are unbound in the quire preset.
`Ctrl+Shift+E`, `Ctrl+Shift+D` and `Ctrl+Shift+O` are unbound in the omarky preset.

## Checks

One Rust unit test module over the keymap table, run for each preset:

- No context binds one chord to two actions. Deferred actions count, so their chords stay
  reserved. Two actions may share a chord only when both declare guards and the guards exclude
  each other, as reader.find-next (a query exists) and reader.next-change (changes mode, no
  query) do.
- Every action has at least one chord in each preset. Mouse-only actions (KEY-38, KEY-61) and the
  dropped writer.save-as (KEY-74) are exempt.
- No chord in a context that falls through to global (reader, writer, find, palette, field)
  shadows a global chord in the same preset. Esc is exempt, because each context's Esc action
  falls through to app.escape.
- No chord in the writer, sheet, find, palette or field contexts is one of the text-field keys
  that KEY-06 leaves to Rune, apart from the palette's three.
- The table agrees with this file. The test reads the KEY items here and compares each action's
  chords in each preset with the table, so the spec and the code cannot drift apart.
- Generated labels follow the preset. Switching the preset changes the help sheet rows, the
  palette's shortcut column and the tooltips, and each label names a chord the table binds
  (KEY-05).

Scripted runs (`--script`) press a sample of chords in each preset and assert that the action
ran, as the `script` tags above say.

## Open questions

- Which preset is the default (decision 0003). Recommended default is omarky, as the decision's
  working default says, with a release note that points existing users to Settings, Shortcuts.
- The palette chord in the omarky preset (KEY-08). Many editors open their palette with
  `Ctrl+Shift+P`, but omarky uses it as a second preview key. Recommended default is
  `Ctrl+Shift+K`, because decision 0003 says omarky's keys win, revisited if users miss
  `Ctrl+Shift+P`.
- `Ctrl+Shift+G` for search all notes in the omarky preset (KEY-10). Some apps, and macOS text
  views, use it for the previous match. Quire's find uses `Shift+F3` and `Shift+Enter` for that.
  Recommended default is to keep `Ctrl+Shift+G`.
- `Ctrl+Shift+P` in the quire preset (KEY-13). Decision 0003 would give it omarky's preview, next
  to `Ctrl+P` for the palette, which invites mistakes. Recommended default is to leave it unbound.
- The outline chord in the quire preset (KEY-12). Recommended default is `Ctrl+Shift+O` over
  `Ctrl+Shift+\`, because a shifted symbol chord depends on the keyboard layout (KEY-03).
- Keys while the palette is open (KEY-02). Today they reach the window, so `Ctrl+B` can format
  the note behind the palette. Recommended default is that the palette falls through to global
  only.
- `Ctrl+S` in the sheet in the quire preset (edit-sheet open question). Recommended default is to
  bind it in both presets, as KEY-75 does.
- Naming Quire's `Ctrl+.` mode against omarky's paragraph focus (writer open question).
  Recommended default is to rename `Ctrl+.` to "Zen mode" in the help sheets and Settings,
  Shortcuts, so both presets list two distinct names.
