# Keymap

Every key Quire's desktop app answers goes through one unified keymap (decision 0003). The
keymap is a table from a context and a chord to a named action, with user overrides persisted
in `~/.config/quire/keybindings.json` and edited in Settings, Shortcuts. This file lists every
action with its default chords. The behaviour each action runs is in `reader.md`, `writer.md`,
`edit-sheet.md`, `map.md` and `page-and-typography.md`.

Notes on reading this file:

- Ctrl means Cmd on macOS. Labels follow `keymap::label_of`, so `Ctrl+Shift+F` shows as `⌘⇧F`
  on macOS.
- Action IDs are kebab-case with a context prefix. `app.` actions are global.
- An item's context in parentheses says where the chord acts. `(writer, sheet)` means the same
  action and chord in both contexts.
- Status follows `README.md`, read for keys. `in` means omarky's chord. `adapted: 0003` (or
  `0013`, `0014`) means Quire moved or unified the chord. `quire` means a Quire-only action.
  `deferred` items keep their chords reserved. `dropped` items are unbound.
- A mouse-only action has a pointer gesture instead of a key. It is in the table so the help
  sheet and Settings can list it.
- Palette commands with no key (new folder, today's journal, theme, rename, delete, sync now,
  lock, switch folder, restore sample) are not keymap actions. They stay in the palette.

## Rules

- **KEY-01** [adapted: 0003] [verify: unit, state] The keymap has these twelve contexts. Global. Reader, which is preview mode with no control focused. Writer, which is the source editor field while it has focus. Map, which is map mode with no inline input or control focused (decision 0014). MapEdit, the map node inline text input (decision 0014). Board, which is board mode with no control focused (decision 0015). Sheet, the edit sheet. Find, the find bar's field. Palette, the command palette and quick switcher field. Field, the single-line fields (rename in the breadcrumb, new folder in the tree, lock password, welcome, sync settings). Help, the shortcut sheet in any mode. Rewrite, the rewrite line, which is deferred.
- **KEY-02** [adapted: 0003] [verify: unit, script] A key goes first to the focused control, which keeps its own text keys (KEY-06). Then it goes to the topmost context. Help and sheet are modal, so a key they do not bind stops there (RD-80, RD-81, WR-159, SHEET-014, SHEET-036). The toast's Undo key (KEY-94) is the one exception: the toast is not a layer, so its key acts whatever is open. Find falls through to the mode's context (reader, writer, map or board) and then to global, but the writer's edits at the caret and Reader/Map/Board single-key shortcuts do nothing from it (KEY-98, MAP-066, BRD-030). Palette, field and mapedit fall through to global only, so an editing key never changes a note hidden behind them. Reader, writer, map and board fall through to global. A focused stop owns the plain keys, and a shortcut with Ctrl, Alt or the logo key goes on from it as from the note (KEY-98).
- **KEY-03** [adapted: 0003] [verify: unit] A chord with Ctrl on a letter or digit matches `Key::Command` with exact Shift and Alt modifiers (never `Key::Text`, so AltGr-produced characters on Windows and European layouts type text instead of firing `Ctrl+Alt+*` chords). `Ctrl+Shift+0..9` matches both the digit and its US shifted symbol (`&`, `*`, `(`, etc.). A chord on a symbol matches the character the key produces, with Shift ignored (`Ctrl+Shift+=` gives `+` and fires `Ctrl++`, `Ctrl+Shift+/` gives `?` and fires `Ctrl+?`). A plain reader/map/board key matches the character typed, so `G` is Shift+g and `Shift+1` matches `1` or `!`. Reader/map/board keys with Alt or Super are ignored unless Alt is named (RD-83).
- **KEY-04** [in] [verify: unit, script] A sequence such as `g g` waits up to 900 ms for its second key. The first `g` does nothing on its own. Any other key cancels the wait and then acts as itself (RD-23).
- **KEY-05** [adapted: 0003] [verify: unit, state] Every key label is generated from the active keymap overrides. That covers the reader's, writer's, map's and board's help sheets (RD-90, WR-166, MAP-063, BRD-029), the sheet's key row (SHEET-030), button tooltips (RD-34), the shortcut column of the command palette, and Settings, Shortcuts. Rebinding an action updates all of them at once, without a restart.
- **KEY-06** [in] [verify: script] Text fields keep Rune's caret, selection, clipboard and undo keys (arrows, Home, End, PageUp, PageDown, Backspace, Delete, Ctrl+A, Ctrl+C, Ctrl+X, Ctrl+V and Rune's undo and redo). The keymap binds none of them in the writer, sheet, find, palette, field or mapedit contexts, except the palette's Up, Down and Backspace (KEY-81, KEY-82, KEY-84). Enter, Tab and Esc are not on this list, so the writer, the sheet and mapedit may take them. The editor's right-click menu shows Cut, Copy and Paste with these keys (WR-135, SHEET-032).
- **KEY-07** [adapted: 0003] [verify: state] Settings, Shortcuts lists every context's actions and lets the user rebind, clear or reset any action, persisted in `~/.config/quire/keybindings.json`. Recording checks for intra-context, global-shadowing and reserved text-field conflicts before saving.
- **KEY-95** [quire] [verify: -] The side panels' lists take their keys outside the keymap, as Rune's focus groups do. They are the file tree, the Open list, the search results, the tags, and the outline's headings and backlinks. Each list is one Tab stop: Tab enters it at its current row, and the next Tab leaves it. Up and Down move between rows and stop at the ends. Home and End go to the first and last row. In the tree, Right opens a closed folder or moves to its first row, Left closes an open folder or moves to the row's folder, and `F2` renames the focused note. In the Open shelf, Right and Left toggle the active tab's inline outline, Alt+Up and Alt+Down reorder tabs, and Delete or Backspace closes the focused tab. Enter or Space does what a click does. The Menu key or Shift+F10 opens the row's menu below the row. Esc gives the keyboard back to the note.
- **KEY-98** [quire] [verify: -] One rule holds for every focus stop: a side list's row, the folder's name, the panel tabs, the new folder button, the search field and its clear button, New note, Settings, and the formatting toolbar's buttons. A focused stop owns the plain keys: its own keys act, then Esc, then F6. The key that shows or hides the stop's pane hides it. Every other plain key is dropped. A shortcut with Ctrl, Alt or the logo key acts as it would with the keyboard on the note. A text field keeps its own editing keys while window shortcuts still act.

## Global

- **KEY-08** [adapted: 0003] [verify: unit, script] app.command-palette (global): `Ctrl+K`. Opens Quire's command palette on its commands list.
- **KEY-09** [adapted: 0003] [verify: unit, script] app.quick-switcher (global): `Ctrl+Shift+O`. Opens Quire's quick switcher of notes, recent first (or the file picker in Note mode).
- **KEY-10** [adapted: 0003] [verify: unit, script] app.search-notes (global): `Ctrl+Shift+F`. Puts the keyboard in the sidebar's search field (SB-11).
- **KEY-11** [adapted: 0003] [verify: unit, script] app.toggle-sidebar (global): `Ctrl+Shift+B`. Shows or hides the sidebar in every mode. Preview and Map also bind `b`, `B` and `Ctrl+B` (KEY-162, KEY-176).
- **KEY-12** [adapted: 0003] [verify: unit, script] app.toggle-outline (global): `Ctrl+O`. Shows or hides Quire's outline panel on the right in every mode. Preview and Map also bind `o` and `O` (KEY-30, KEY-140).
- **KEY-13** [adapted: 0003] [verify: unit, script] app.mode-source (global): `Ctrl+E`. Switches to Source (edit) mode idempotently, keeping the reading place.
- **KEY-160** [adapted: 0003] [verify: unit, script] app.mode-preview (global): `Ctrl+P`. Switches to Preview (reader) mode idempotently, keeping the place. Preview and Map also bind `p` (KEY-163, KEY-177).
- **KEY-102** [adapted: 0014] [verify: unit, script] app.mode-map (global): `Ctrl+M`. Switches to Map mode idempotently, keeping the place (overrides macOS Minimize). Preview and Map also bind `m` (KEY-164, KEY-178).
- **KEY-195** [adapted: 0015] [verify: unit, script] app.mode-board (global): `Ctrl+Shift+K`. Switches to Board mode from any mode, and from Board back to the last linear mode (BRD-029).
- **KEY-14** [quire] [verify: -] app.settings (global): `Ctrl+,`. Opens or closes Settings.
- **KEY-15** [quire] [verify: -] app.focus-mode (global): `Ctrl+.`. Enters or leaves Zen mode, which hides the panels (WR-223).
- **KEY-16** [quire] [verify: -] app.new-note (global): `Ctrl+N`. Makes a note named "Untitled" and opens it in a tab.
- **KEY-17** [quire] [verify: -] app.quit (global): `Ctrl+Q`. Quits on Linux and Windows. On macOS the app menu's Cmd+Q quits instead.
- **KEY-18** [quire] [verify: -] app.insert-image (global): `Ctrl+Shift+I`. Picks an image and links it into the active note.
- **KEY-19** [in] [verify: unit, script] app.save (global): `Ctrl+S`. Writes the active note now and toasts "Saved", in any mode (WR-64, WR-111).
- **KEY-20** [in] [verify: unit, script] app.close-tab (global): `Ctrl+W`. Writes any unsaved text and closes the active tab. The window stays (RD-82, WR-68).
- **KEY-161** [quire] [verify: -] app.reopen-tab (global): `Ctrl+Shift+T`. Reopens the most recently closed tab in the active folder.
- **KEY-39** [adapted: 0003] [verify: unit, script] app.read-from-disk (global): `Ctrl+R`. Reads the note from disk now and keeps the place, in every mode (RD-18, MAP-068).
- **KEY-21** [adapted: 0003] [verify: unit, script] app.zoom-in (global): `Ctrl+=` or `Ctrl++`. Steps text size up in Source and Preview, and zooms the canvas in on the Map (RD-82, WR-72, MAP-034).
- **KEY-22** [adapted: 0003] [verify: unit, script] app.zoom-out (global): `Ctrl+-`. Steps text size down in Source and Preview, and zooms the canvas out on the Map (RD-82, WR-72, MAP-034).
- **KEY-40** [adapted: 0003] [verify: unit, script] app.zoom-reset (global): `Ctrl+0`. Resets text size to 14 px in Source and Preview, and resets canvas zoom to 100% centred on the selected node in Map (PAGE-19, MAP-034).
- **KEY-66** [adapted: 0003] [verify: unit, script] app.help (global): `Ctrl+/`, `Ctrl+?` or `F1`. Toggles the keyboard help sheet in any mode. Preview and Map also bind `?` (KEY-43, KEY-142).
- **KEY-23** [quire] [verify: -] app.escape (global): `Esc`. Runs Quire's `App::escape`. An open layer takes Esc first and closes; otherwise dismisses notices, closes inline name fields, and leaves Zen mode.
- **KEY-94** [quire] [verify: -] app.undo-delete (global): `Ctrl+Shift+R`. While a delete's toast offers Undo, puts the note or folder back (`folders::Deleted`).
- **KEY-96** [quire] [verify: -] app.next-pane (global): `F6`. Moves the keyboard to the next visible pane, left to right (KEY-98).
- **KEY-97** [quire] [verify: -] app.previous-pane (global): `Shift+F6`. Moves the keyboard to the previous visible pane.
- **KEY-99** [quire] [verify: -] app.tab-1 to app.tab-9 (global): `Ctrl+1` to `Ctrl+9`. Switches to the first through ninth tab in the Open shelf.
- **KEY-100** [quire] [verify: -] app.next-tab (global): `Ctrl+Tab`. Switches to the next tab in the Open shelf, wrapping around.
- **KEY-101** [quire] [verify: -] app.previous-tab (global): `Ctrl+Shift+Tab`. Switches to the previous tab in the Open shelf, wrapping around.
- **KEY-186** [quire] [verify: -] app.nav-back (reader, writer): `Alt+Left`. Navigates back to the previously visited note in history.
- **KEY-187** [quire] [verify: -] app.nav-forward (reader, writer): `Alt+Right`. Navigates forward to the next note in history.
- **KEY-188** [quire] [verify: -] note.lock (global): `Ctrl+Alt+L`. Locks a plain note with a password, or immediately re-locks an unlocked note.
- **KEY-189** [quire] [verify: -] note.lock-all (global): `Ctrl+Alt+Shift+L`. Immediately re-locks every unlocked note in memory.

## Reader

- **KEY-24** [in] [verify: unit, script] reader.scroll-down (reader): `j` or `Down`. Glides down one line step (RD-20, RD-22).
- **KEY-25** [in] [verify: unit, script] reader.scroll-up (reader): `k` or `Up`. Glides up one line step (RD-20, RD-22).
- **KEY-26** [in] [verify: unit, script] reader.page-down (reader): `Space` or `PageDown`. Glides down one page step (RD-21).
- **KEY-27** [in] [verify: unit, script] reader.page-up (reader): `Shift+Space` or `PageUp`. Glides up one page step (RD-21).
- **KEY-28** [in] [verify: unit, script] reader.top (reader): `g g` or `Home`. Travels to the top of the note (RD-23, RD-24, KEY-04).
- **KEY-29** [in] [verify: unit, script] reader.bottom (reader): `G` or `End`. Travels to the end of the note (RD-23, RD-24).
- **KEY-162** [adapted: 0003] [verify: unit, script] reader.toggle-sidebar (reader): `b`, `B` or `Ctrl+B`. Runs `app.toggle-sidebar`.
- **KEY-30** [in] [verify: unit, script] reader.toggle-outline (reader): `o` or `O`. Runs `app.toggle-outline` (RD-41).
- **KEY-31** [in] [verify: unit, script] reader.toggle-wide (reader): `w` or `W`. Switches between the reading measure and the wide measure (RD-51).
- **KEY-163** [adapted: 0003] [verify: unit, script] reader.mode-preview (reader): `p`. Idempotent Preview mode key.
- **KEY-164** [adapted: 0003] [verify: unit, script] reader.mode-map (reader): `m`. Switches to Map mode.
- **KEY-165** [adapted: 0003] [verify: unit, script] reader.read-from-disk (reader): `r`. Runs `app.read-from-disk`.
- **KEY-166** [adapted: 0003] [verify: unit, script] reader.focus-mode (reader): `z` or `.`. Runs `app.focus-mode`.
- **KEY-167** [adapted: 0003] [verify: unit, script] reader.settings (reader): `,`. Runs `app.settings`.
- **KEY-168** [adapted: 0003] [verify: unit, script] reader.save (reader): `s`. Runs `app.save`.
- **KEY-169** [adapted: 0003] [verify: unit, script] reader.insert-image (reader): `i`. Runs `app.insert-image`.
- **KEY-32** [in] [verify: unit, script] reader.find (reader): `Ctrl+F` or `/`. Opens the find bar with focus in its field (RD-52, WR-124).
- **KEY-33** [adapted: 0003] [verify: unit, script] reader.find-next (reader): `n`, `F3` or `Ctrl+G`. Goes to the next match; `n` acts only while a query exists, while `F3` and `Ctrl+G` open Find when no query exists (RD-58).
- **KEY-34** [adapted: 0003] [verify: unit, script] reader.find-previous (reader): `N`, `Shift+F3` or `Ctrl+Shift+G`. Goes to the previous match; `N` acts only while a query exists (RD-58).
- **KEY-35** [in] [verify: unit, script] reader.copy-selection (reader): `Ctrl+C`. Copies the selected page text and toasts "Copied" (RD-67).
- **KEY-36** [in] [verify: unit, script] reader.copy-block-under-pointer (reader): `Ctrl+Shift+C`. Copies the Markdown source of the block under the pointer (RD-69, RD-100).
- **KEY-37** [in] [verify: unit, script] reader.copy-all (reader): `Ctrl+Shift+A`. Copies the whole note and toasts "Copied the whole note" (RD-70).
- **KEY-38** [in] [verify: unit, script] reader.block-menu (reader): `Right-click`. Opens the clicked block's menu; from the keyboard the Menu key or Shift+F10 opens it while the note holds focus (RD-68, KEY-95).
- **KEY-41** [in] [verify: unit, script] reader.edit-block (reader): `e`. Opens the edit sheet on one block (RD-78, SHEET-001, SHEET-002).
- **KEY-42** [in] [verify: unit, script] reader.edit-section (reader): `E`. Opens the edit sheet on the block's section (RD-78, SHEET-001, SHEET-009).
- **KEY-184** [quire] [verify: -] reader.undo (reader): `Ctrl+Z`. Undoes the last edit to the active note from Preview.
- **KEY-185** [quire] [verify: -] reader.redo (reader): `Ctrl+Shift+Z` or `Ctrl+Y`. Redoes the last undone edit to the active note from Preview.
- **KEY-194** [quire] [verify: -] reader.toggle-fold (reader): `Ctrl+Shift+[`. Folds or unfolds the hovered or current heading section in Preview.
- **KEY-43** [adapted: 0003] [verify: unit, script] reader.help (reader): `?`. Opens the help sheet (`Ctrl+/` and `F1` act globally via KEY-66).
- **KEY-44** [dropped] [verify: -] reader.close-tab (reader): unbound. Removed to prevent accidental tab loss; use `Ctrl+W` (KEY-20).
- **KEY-45** [adapted: 0003] [verify: unit, script] reader.escape (reader): `Esc`. Dismisses Quire's notice, else closes find, else clears the selection, else runs `app.escape`, staying in Preview (decision 0003).
- **KEY-46** [deferred] [verify: -] reader.versions (reader): `v` or `V`. Opens the version list in changes mode (RD-84).
- **KEY-47** [deferred] [verify: -] reader.fold (reader): `f` or `F`. Folds unchanged text in changes mode (RD-84).
- **KEY-48** [deferred] [verify: -] reader.next-change (reader): `n`. Steps to the next change in changes mode with no find query (RD-84).
- **KEY-49** [deferred] [verify: -] reader.previous-change (reader): `P`. Steps to the previous change in changes mode (moved from `p` for KEY-163).

## Writer

The editing actions also act in the sheet, because the sheet's editor is the writer's
(SHEET-035). Inline formatting actions (`KEY-50` to `KEY-54`, `KEY-170`) also act in `mapedit`
(`MAP-016`).

- **KEY-50** [in] [verify: unit, script] writer.bold (writer, sheet, mapedit): `Ctrl+B`. Toggles `**` by `format.rs`, as one undo step (WR-51, WR-52, WR-55, WR-56, WR-63, MAP-016).
- **KEY-51** [in] [verify: unit, script] writer.italic (writer, sheet, mapedit): `Ctrl+I`. Toggles `*` (WR-51, WR-56, WR-63, MAP-016).
- **KEY-52** [adapted: 0003] [verify: unit, script] writer.inline-code (writer, sheet, mapedit): `Ctrl+Shift+C`. Toggles a backtick pair, or a fence across lines (WR-51, WR-56, MAP-016).
- **KEY-53** [in] [verify: unit, script] writer.strikethrough (writer, sheet, mapedit): `Ctrl+Shift+X`. Toggles `~~` (WR-51, MAP-016).
- **KEY-54** [adapted: 0003] [verify: unit, script] writer.link (writer, sheet, mapedit): `Ctrl+L`. Makes a link from the selection, ready for the URL or the text (WR-57, WR-58, MAP-016).
- **KEY-170** [quire] [verify: -] writer.wiki-link (writer, sheet, mapedit): `Ctrl+Shift+L`. Toggles `[[...]]` around the selection or inserts a note link at the caret.
- **KEY-171** [quire] [verify: -] writer.blockquote (writer, sheet): `Ctrl+Shift+Q`. Toggles `> ` blockquote prefixes on the selected lines.
- **KEY-172** [quire] [verify: -] writer.toggle-checkbox (writer, sheet): `Ctrl+Shift+Enter`. Toggles `[ ]` / `[x]` on task lines in the selection, or converts a plain line to a task.
- **KEY-173** [quire] [verify: -] writer.move-up (writer, sheet): `Alt+Up`. Moves the current line or selected lines up one line.
- **KEY-174** [quire] [verify: -] writer.move-down (writer, sheet): `Alt+Down`. Moves the current line or selected lines down one line.
- **KEY-55** [adapted: 0003] [verify: unit, script] writer.heading-1 to writer.heading-6 (writer, sheet): `Ctrl+Alt+1` to `Ctrl+Alt+6`. Six actions that set the line's heading level, or clear it when already at that level (WR-59, WR-60, WR-61).
- **KEY-56** [adapted: 0003] [verify: unit, script] writer.plain-paragraph (writer, sheet): `Ctrl+Alt+0`. Clears the line's heading (WR-60, WR-62).
- **KEY-152** [quire] [verify: -] writer.numbered-list (writer, sheet): `Ctrl+Shift+7`. Toggles numbered list prefixes on the selected lines (WR-222).
- **KEY-153** [quire] [verify: -] writer.bulleted-list (writer, sheet): `Ctrl+Shift+8`. Toggles bulleted list prefixes on the selected lines (WR-222).
- **KEY-154** [quire] [verify: -] writer.task-list (writer, sheet): `Ctrl+Shift+9`. Toggles task list prefixes on the selected lines (WR-222).
- **KEY-57** [in] [verify: unit, script] writer.indent (writer, sheet): `Tab`. Indents a list item by its marker's step, else inserts 4 spaces (WR-47, WR-48, WR-49).
- **KEY-58** [adapted: 0001] [verify: unit, script] writer.outdent (writer, sheet): `Shift+Tab`. Outdents a list item or indented prose/code line, else does nothing and is consumed (WR-50).
- **KEY-59** [in] [verify: unit, script] writer.continue-block (writer, sheet): `Enter`. Continues or ends a list item or quote, else inserts a newline (WR-39 to WR-45).
- **KEY-60** [quire] [verify: -] writer.follow-link (writer): `Ctrl+Enter`. Follows the note link at the caret (WR-46, WR-220).
- **KEY-61** [quire] [verify: -] writer.follow-link-click (writer): `Ctrl+click`. Mouse-only. Follows the clicked note link (WR-220).
- **KEY-62** [in] [verify: unit, script] writer.find (writer): `Ctrl+F`. Opens the find bar, seeded with the selection (WR-70, WR-115).
- **KEY-183** [quire] [verify: -] writer.replace (writer): `Ctrl+H`. Opens the find bar with the replace row expanded in Source mode.
- **KEY-63** [in] [verify: unit, script] writer.find-next (writer): `Ctrl+G` or `F3`. Goes to the next match, even with the bar hidden (WR-70, WR-120).
- **KEY-64** [adapted: 0003] [verify: unit, script] writer.find-previous (writer): `Ctrl+Shift+G` or `Shift+F3`. Goes to the previous match (WR-70, WR-120).
- **KEY-65** [dropped] [verify: -] writer.recent-notes (writer): unbound. Replaced by `app.quick-switcher` (`Ctrl+Shift+O`); `Ctrl+R` reloads from disk globally (KEY-39).
- **KEY-67** [adapted: 0003] [verify: unit, script] writer.paragraph-focus (writer): `Ctrl+Shift+D`. Toggles the fade of every paragraph but the current one (WR-75, WR-79).
- **KEY-68** [adapted: 0003] [verify: unit, script] writer.typewriter (writer): `F9`. Toggles typewriter scrolling (WR-75, WR-85).
- **KEY-69** [in] [verify: unit, script] writer.next-font (writer): `Ctrl+Shift+M`. Steps the source font to the next iA Writer family (WR-75, WR-125).
- **KEY-70** [in] [verify: unit, script] writer.escape (writer): `Esc`. Dismisses Quire's notice, else closes the find bar, else closes inline prompts, else exits Source mode to Preview while keeping zen mode active (WR-77, decision 0003).
- **KEY-71** [deferred] [verify: -] writer.spelling (writer): `F7`. Toggles spell checking (moved from `Ctrl+Shift+L` for KEY-170).
- **KEY-72** [deferred] [verify: -] writer.rewrite (writer, sheet): `Ctrl+J`. Opens the rewrite line on the block or selection (WR-76, WR-167, SHEET-034).
- **KEY-73** [deferred] [verify: -] writer.rewrite-section (writer, sheet): `Ctrl+Shift+J`. Opens the rewrite line on the section (WR-76, WR-167, SHEET-034).
- **KEY-74** [dropped] [verify: -] writer.save-as (writer): unbound. Quire has no Save As (WR-65).

## Sheet

- **KEY-75** [in] [verify: unit, script] sheet.save (sheet): `Ctrl+Enter` or `Ctrl+S`. Saves the sheet's text into the note and closes it (SHEET-033, SHEET-044, SHEET-045, SHEET-048).
- **KEY-76** [in] [verify: unit, script] sheet.cancel (sheet): `Esc`. Closes the menu, else closes an unchanged sheet, else arms a discard that a second Esc within 2000 ms carries out (SHEET-037, SHEET-038, SHEET-039).

## Find

- **KEY-77** [in] [verify: unit, script] find.next (find): `Enter`. Goes to the next match, wrapping (RD-58, WR-119).
- **KEY-78** [in] [verify: unit, script] find.previous (find): `Shift+Enter`. Goes to the previous match, wrapping (RD-58, WR-119).
- **KEY-79** [in] [verify: unit, script] find.close (find): `Esc`. Closes the bar and returns focus (RD-61, WR-119).
- **KEY-80** [in] [verify: unit, script] find.select-all (find): `Ctrl+F`. Selects all of the field's text (RD-52, WR-119).

## Palette

- **KEY-81** [quire] [verify: -] palette.next (palette): `Down`. Moves to the next row and scrolls it into view.
- **KEY-82** [quire] [verify: -] palette.previous (palette): `Up`. Moves to the previous row.
- **KEY-83** [quire] [verify: -] palette.run (palette): `Enter`. Opens the note or runs the command on the current row.
- **KEY-84** [quire] [verify: -] palette.back-to-notes (palette): `Backspace`. On an empty commands query, goes back to the note list.
- **KEY-85** [quire] [verify: -] palette.close (palette): `Esc`. Clears a non-empty filter query first, then closes the palette.

## Field

- **KEY-86** [quire] [verify: -] field.submit (field): `Enter`. Commits a rename or a new folder, and submits the lock, welcome and sync forms.
- **KEY-87** [quire] [verify: -] field.cancel (field): `Esc`. Cancels a rename or a new folder.

## Help

- **KEY-88** [adapted: 0003] [verify: unit, script] help.close (help): `Esc`, `?`, `F1`, `Ctrl+/` or `Ctrl+?`. Closes the help sheet in any mode. With text in its search, `Esc` clears the search instead and `?` types (RD-105).

## Rewrite

- **KEY-89** [deferred] [verify: -] rewrite.send (rewrite): `Enter`. Sends the instruction, or takes the ready rewrite on an empty line (SHEET-080).
- **KEY-90** [deferred] [verify: -] rewrite.close (rewrite): `Esc`. Stops Claude while it works, else closes the line (SHEET-079).
- **KEY-91** [deferred] [verify: -] rewrite.earlier (rewrite): `Alt+Up`. Shows the earlier rewrite (SHEET-081).
- **KEY-92** [deferred] [verify: -] rewrite.later (rewrite): `Alt+Down`. Shows the later rewrite (SHEET-081).
- **KEY-93** [deferred] [verify: -] rewrite.recall (rewrite): `Up` or `Down`. Steps through remembered instructions (SHEET-082).

## Map

- **KEY-103** [adapted: 0014] [verify: unit, script] map.left (map): `Left` or `h`. Selects the neighbour to the left (MAP-056).
- **KEY-104** [adapted: 0014] [verify: unit, script] map.right (map): `Right` or `l`. Selects the neighbour to the right (MAP-056).
- **KEY-105** [adapted: 0014] [verify: unit, script] map.up (map): `Up` or `k`. Selects the neighbour above (MAP-056).
- **KEY-106** [adapted: 0014] [verify: unit, script] map.down (map): `Down` or `j`. Selects the neighbour below (MAP-056).
- **KEY-107** [adapted: 0014] [verify: unit, script] map.root (map): `Home` or `g g`. Selects the root node (MAP-056, KEY-04).
- **KEY-108** [adapted: 0014] [verify: unit, script] map.edit (map): `F2` or `e`. Starts inline edit on a text node, opens the Edit Sheet on a non-image card or multi-line setext heading, or opens the lightbox on an image card (MAP-016, MAP-020, MAP-047).
- **KEY-109** [adapted: 0014] [verify: unit, script] map.edit-subtree (map): `E`. Opens the Edit Sheet scoped to the selected node's `subtree_lines` (MAP-020).
- **KEY-110** [adapted: 0014] [verify: unit, script] map.add-child (map): `Tab`. Unfolds the selected container node if folded and starts inline edit on a new child at the end of its children (MAP-021, MAP-022).
- **KEY-111** [adapted: 0014] [verify: unit, script] map.add-sibling (map): `Enter`. Starts inline edit on a new sibling after the selected node, or adds a child when the root is selected (MAP-023).
- **KEY-112** [adapted: 0014] [verify: unit, script] map.add-sibling-before (map): `Shift+Enter`. Starts inline edit on a new sibling before the selected node (MAP-023).
- **KEY-113** [adapted: 0014] [verify: unit, script] map.delete (map): `Delete` or `Backspace`. Deletes the selected non-root node and its subtree as one undo step (MAP-025).
- **KEY-114** [adapted: 0014] [verify: unit, script] map.toggle-fold (map): `Space` or `f`. Toggles fold on the selected node (MAP-058).
- **KEY-115** [adapted: 0014] [verify: unit, script] map.toggle-fold-recursive (map): `Shift+Space` or `F`. Toggles fold recursively on the selected node and its descendants (MAP-058).
- **KEY-116** [adapted: 0014] [verify: unit, script] map.toggle-task (map): `x`. Toggles the task checkbox on the selected task list item as one undo step (MAP-053).
- **KEY-117** [adapted: 0014] [verify: unit, script] map.expand-all (map): `]`. Expands every folded node in the map (MAP-058).
- **KEY-118** [adapted: 0014] [verify: unit, script] map.collapse-to-level (map): `[`. Folds all nodes at 1-based depth >= 2, or depth >= 1 when depth >= 2 is already folded (MAP-058).
- **KEY-119** [adapted: 0014] [verify: unit, script] map.fit (map): `0`, `1` or `Shift+1`. Fits all unfolded nodes inside the free viewport (MAP-035).
- **KEY-120** [dropped] [verify: -] map.zoom-reset (map): unbound. Unified into `app.zoom-reset` (`Ctrl+0`, KEY-40).
- **KEY-121** [adapted: 0014] [verify: unit, script] map.center (map): `c`. Pans at the current zoom to centre the selected node in the free viewport (MAP-036).
- **KEY-122** [adapted: 0014] [verify: unit, script] map.zoom-in (map): `+` or `=`. Zooms the canvas in by × 1.25 around the selected node (`Ctrl+=`/`Ctrl++` also zoom via KEY-21) (MAP-034).
- **KEY-123** [adapted: 0014] [verify: unit, script] map.zoom-out (map): `-`. Zooms the canvas out by × 0.8 around the selected node (`Ctrl+-` also zooms via KEY-22) (MAP-034).
- **KEY-124** [adapted: 0014] [verify: unit, script] map.move-up (map): `Alt+Up`. Moves the selected node one sibling earlier (MAP-031).
- **KEY-125** [adapted: 0014] [verify: unit, script] map.move-down (map): `Alt+Down`. Moves the selected node one sibling later (MAP-031).
- **KEY-126** [adapted: 0014] [verify: unit, script] map.outdent (map): `Shift+Tab`. Promotes or outdents the selected node to After its parent (MAP-031).
- **KEY-127** [adapted: 0014] [verify: unit, script] map.move-right (map): `Alt+Right`. Moves the selected node right (indent or outdent according to side) (MAP-031).
- **KEY-159** [quire] [verify: -] map.layout (map): `L`. Opens the map layout picker (MAP-089).
- **KEY-158** [adapted: 0014] [verify: unit, script] map.move-left (map): `Alt+Left`. Moves the selected node left (outdent or indent according to side) (MAP-031).
- **KEY-128** [adapted: 0014] [verify: unit, script] map.heading-1 to map.heading-6 (map): `Ctrl+Alt+1` to `Ctrl+Alt+6`. Sets a selected heading to level 1 to 6 and shifts its subheadings by the delta, or converts a text node to that heading (MAP-031).
- **KEY-190** [quire] [verify: -] map.paragraph (map): `Ctrl+Alt+0`. Converts the selected text node to a paragraph.
- **KEY-191** [quire] [verify: -] map.task (map): `Ctrl+Shift+7`. Converts the selected text node to a task item.
- **KEY-192** [quire] [verify: -] map.bulleted (map): `Ctrl+Shift+8`. Converts the selected text node to a bulleted list item.
- **KEY-193** [quire] [verify: -] map.numbered (map): `Ctrl+Shift+9`. Converts the selected text node to a numbered list item.
- **KEY-129** [adapted: 0014] [verify: unit, script] map.follow-link (map): `Ctrl+Enter`. Activates the selected node's first link or tag, or opens the lightbox on an image card (MAP-054).
- **KEY-130** [adapted: 0014] [verify: unit, script] map.find (map): `Ctrl+F` or `/`. Opens the find bar with focus in its field (MAP-066).
- **KEY-131** [adapted: 0014] [verify: unit, script] map.find-next (map): `n`, `F3` or `Ctrl+G`. Steps to the next match; `n` acts only while a query exists (MAP-066).
- **KEY-132** [adapted: 0014] [verify: unit, script] map.find-previous (map): `N`, `Shift+F3` or `Ctrl+Shift+G`. Steps to the previous match; `N` acts only while a query exists (MAP-066).
- **KEY-133** [adapted: 0014] [verify: unit, script] map.undo (map): `Ctrl+Z`. Undoes one map gesture in the note's shared `TextEdit` history (MAP-014).
- **KEY-134** [adapted: 0014] [verify: unit, script] map.redo (map): `Ctrl+Shift+Z` or `Ctrl+Y`. Redoes one map gesture in the note's shared `TextEdit` history (MAP-014).
- **KEY-175** [quire] [verify: -] map.cut (map): `Ctrl+X`. Copies the selected branch's Markdown and deletes it as one undo step.
- **KEY-135** [adapted: 0014] [verify: unit, script] map.copy-node (map): `Ctrl+C`. Copies the selected node's Markdown (`own_lines`) and toasts "Copied" (MAP-059).
- **KEY-136** [adapted: 0014] [verify: unit, script] map.copy-subtree (map): `Ctrl+Shift+C`. Copies the selected node's whole subtree Markdown (`subtree_lines`) (MAP-059).
- **KEY-137** [adapted: 0014] [verify: unit, script] map.copy-all (map): `Ctrl+Shift+A`. Copies the whole note and toasts "Copied the whole note" (MAP-059, RD-70).
- **KEY-138** [adapted: 0014] [verify: unit, script] map.paste (map): `Ctrl+V`. Pastes a clipboard image into `attachments/` or single-line plain text as a new child or sibling (MAP-048).
- **KEY-176** [adapted: 0003] [verify: unit, script] map.toggle-sidebar (map): `b`, `B` or `Ctrl+B`. Runs `app.toggle-sidebar`.
- **KEY-140** [adapted: 0014] [verify: unit, script] map.toggle-outline (map): `o` or `O`. Runs `app.toggle-outline` (MAP-067, RD-41).
- **KEY-177** [adapted: 0003] [verify: unit, script] map.mode-preview (map): `p`. Switches to Preview mode.
- **KEY-178** [adapted: 0003] [verify: unit, script] map.mode-map (map): `m`. Idempotent Map mode key.
- **KEY-139** [adapted: 0003] [verify: unit, script] map.read-from-disk (map): `r`. Runs `app.read-from-disk`.
- **KEY-179** [adapted: 0003] [verify: unit, script] map.focus-mode (map): `z` or `.`. Runs `app.focus-mode`.
- **KEY-180** [adapted: 0003] [verify: unit, script] map.settings (map): `,`. Runs `app.settings`.
- **KEY-181** [adapted: 0003] [verify: unit, script] map.save (map): `s`. Runs `app.save`.
- **KEY-182** [adapted: 0003] [verify: unit, script] map.insert-image (map): `i`. Runs `app.insert-image`.
- **KEY-141** [dropped] [verify: -] map.close-tab (map): unbound. Removed to prevent accidental tab loss; use `Ctrl+W` (KEY-20).
- **KEY-142** [adapted: 0003] [verify: unit, script] map.help (map): `?`. Opens the Map help sheet (`Ctrl+/` and `F1` act globally via KEY-66).
- **KEY-143** [adapted: 0003] [verify: unit, script] map.escape (map): `Esc`. Cancels an active drag, else dismisses Quire's notice, else closes Find, else runs `app.escape`, staying in Map (decision 0003).
- **KEY-144** [adapted: 0014] [verify: unit, script] map.node-menu (map): `Right-click`. Mouse-only. Opens the clicked node's context menu; from the keyboard the Menu key or Shift+F10 opens it (MAP-062).

## Map edit

Inline formatting actions (`KEY-50` to `KEY-54`, `KEY-170`) also act in `mapedit` (`MAP-016`).

- **KEY-145** [adapted: 0014] [verify: unit, script] mapedit.commit (mapedit): `Enter`. Commits the inline text edit and keeps the node selected (MAP-017).
- **KEY-146** [adapted: 0014] [verify: unit, script] mapedit.newline (mapedit): `Shift+Enter`. Inserts a newline in paragraph and list-item nodes, and commits in headings (MAP-017).
- **KEY-147** [adapted: 0014] [verify: unit, script] mapedit.cancel (mapedit): `Esc`. Cancels the inline edit and restores the original text, or removes an uncommitted placeholder node (MAP-017).
- **KEY-148** [adapted: 0014] [verify: unit, script] mapedit.commit-add-child (mapedit): `Tab`. Commits the inline edit and immediately starts inline edit on a new child node (MAP-017).
- **KEY-149** [adapted: 0014] [verify: unit, script] mapedit.commit-outdent (mapedit): `Shift+Tab`. Commits the inline edit and promotes or outdents the node via `MAP-031` (MAP-017).
- **KEY-150** [adapted: 0014] [verify: unit, script] mapedit.paragraph (mapedit): `Ctrl+Alt+0`. Makes the new node being named a plain paragraph (MAP-087).
- **KEY-151** [adapted: 0014] [verify: unit, script] mapedit.heading-1 to mapedit.heading-6 (mapedit): `Ctrl+Alt+1` to `Ctrl+Alt+6`. Makes the new node being named a heading of that level (MAP-087).
- **KEY-155** [quire] [verify: -] mapedit.numbered (mapedit): `Ctrl+Shift+7`. Makes the new node being named a numbered item (MAP-087).
- **KEY-156** [quire] [verify: -] mapedit.bulleted (mapedit): `Ctrl+Shift+8`. Makes the new node being named a bulleted item (MAP-087).
- **KEY-157** [quire] [verify: -] mapedit.task (mapedit): `Ctrl+Shift+9`. Makes the new node being named a task (MAP-087).

## Board

- **KEY-196** [adapted: 0015] [verify: unit, script] board.left (board): `Left` or `h`. Selects the card at the same height in the previous column, else its last card, else its header (BRD-025).
- **KEY-197** [adapted: 0015] [verify: unit, script] board.right (board): `Right` or `l`. Selects the card at the same height in the next column, else its last card, else its header (BRD-025).
- **KEY-198** [adapted: 0015] [verify: unit, script] board.up (board): `Up` or `k`. Selects the previous card in the column, or from the first card its header (BRD-025).
- **KEY-199** [adapted: 0015] [verify: unit, script] board.down (board): `Down` or `j`. Selects the next card in the column, or from the header its first card (BRD-025).
- **KEY-200** [adapted: 0015] [verify: unit, script] board.home (board): `Home`. Selects the first column's header (BRD-025).
- **KEY-201** [adapted: 0015] [verify: unit, script] board.move-up (board): `Alt+Up`. Moves the selected card one place earlier in its column (BRD-026).
- **KEY-202** [adapted: 0015] [verify: unit, script] board.move-down (board): `Alt+Down`. Moves the selected card one place later in its column (BRD-026).
- **KEY-203** [adapted: 0015] [verify: unit, script] board.move-left (board): `Alt+Left`. Moves the selected card to the end of the previous column, or moves the selected column left (BRD-026).
- **KEY-204** [adapted: 0015] [verify: unit, script] board.move-right (board): `Alt+Right`. Moves the selected card to the end of the next column, or moves the selected column right (BRD-026).
- **KEY-205** [adapted: 0015] [verify: unit, script] board.toggle-task (board): `x`. Toggles the task checkbox on the selected task card (BRD-027).
- **KEY-206** [adapted: 0015] [verify: unit, script] board.edit (board): `Enter`, `e` or `F2`. Opens the Edit Sheet on the card's subtree or column header's own line (BRD-027).
- **KEY-207** [adapted: 0015] [verify: unit, script] board.delete (board): `Delete` or `Backspace`. Deletes the selected card as the map does, and refuses on a column header (BRD-027).
- **KEY-208** [adapted: 0015] [verify: unit, script] board.copy (board): `Ctrl+C`. Copies the selected card's Markdown (BRD-027).
- **KEY-209** [adapted: 0015] [verify: unit, script] board.undo (board): `Ctrl+Z`. Undoes one edit in the note's shared history (BRD-027).
- **KEY-210** [adapted: 0015] [verify: unit, script] board.redo (board): `Ctrl+Shift+Z` or `Ctrl+Y`. Redoes one edit in the note's shared history (BRD-027).
- **KEY-211** [adapted: 0015] [verify: unit, script] board.follow-link (board): `Ctrl+Enter`. Follows the selected card's first link or opens an image card's lightbox (BRD-027).
- **KEY-212** [adapted: 0015] [verify: unit, script] board.card-menu (board): `Right-click`. Mouse-only. Opens the clicked card's context menu; from the keyboard the Menu key or Shift+F10 opens it (BRD-028).
- **KEY-213** [adapted: 0015] [verify: unit, script] board.find (board): `Ctrl+F` or `/`. Opens the find bar with focus in its field (BRD-030).
- **KEY-214** [adapted: 0015] [verify: unit, script] board.find-next (board): `n`, `F3` or `Ctrl+G`. Steps to the next board match; `n` acts only while a query exists (BRD-030).
- **KEY-215** [adapted: 0015] [verify: unit, script] board.find-previous (board): `N`, `Shift+F3` or `Ctrl+Shift+G`. Steps to the previous board match; `N` acts only while a query exists (BRD-030).
- **KEY-216** [adapted: 0015] [verify: unit, script] board.help (board): `?`. Opens the Board help sheet (BRD-029).
- **KEY-217** [adapted: 0015] [verify: unit, script] board.escape (board): `Esc`. Cancels an active hold, else dismisses a notice, else closes Find, else runs `app.escape` (BRD-019, BRD-030).

## Checks

One Rust unit test module over the keymap table (`desktop/src/keymap.rs`):

- No context binds one chord to two actions (deferred actions count). Two actions may share a chord only when both declare guards that exclude each other.
- Every live action has at least one chord (mouse-only and dropped actions are exempt).
- No chord in a context that falls through to global shadows a global chord (`Esc` is exempt).
- No chord in text-editing contexts takes a Rune text-field key (KEY-06), apart from the palette's three.
- The table agrees with this file: every KEY item's chords, contexts and status match.
- User overrides reject conflicts and reserved text-field keys, persist cleanly to `keybindings.json`, and update labels and lookup immediately.
