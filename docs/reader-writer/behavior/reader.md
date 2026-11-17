# Reader

Quire's preview mode is the reader (decision 0001). This file lists how it behaves: reload,
scrolling, the outline, find, links, copy, task ticking, the edit-sheet entry points, the keys
and the help sheet. The edit sheet itself is in `edit-sheet.md`, the page and its blocks in
`page-and-typography.md` and `blocks.md`, and every key in `keymap.md`.

The items come from omarky's reader (`03-reader-behaviour.md` in the extraction). Notes on
reading them:

- "The note" is the open note in the active tab. "The page" is the preview of that note.
- Colour names (text, dim, faint, ghost, accent, hover, hairline, hairlineSoft, raised, red,
  find background) are the page roles. Decision 0005 derives them from Quire's palette, so an
  item that only names roles is `in`.
- Chrome lengths are px at scale 1. omarky's `S(n)` is written as `n px`. Page lengths use `B`,
  the body size, 14 px by default (decision 0002).
- Keys are written as they are in the omarky preset (decision 0003). `keymap.md` gives the
  quire preset.
- Changes mode is deferred (decision 0006). Items or parts of items that only apply to it say so.

## 1. Windows and files

- **RD-01** [quire] [verify: -] Quire opens a note in its one window and lists it under Open at the top of the sidebar; a note that is already open becomes current again. omarky opened one window per file, and the same file twice gave two windows (`reader/ui/shell.qml:45-47`).
- **RD-02** [quire] [verify: -] Quire is one window over one notes folder. omarky ran one app instance for all windows and handed a second launch's files to it (`reader/ui/shell.qml:13`, `reader/ui/shell.qml:60-70`, `reader/ui/shell.qml:122`).
- **RD-03** [dropped] [verify: -] Command-line targets of the form `path#anchor`, with the compare suffix, are a single-file-app detail. Quire opens a folder, not a file (`reader/ui/shell.qml:33-42`).
- **RD-04** [adapted: 0001] [verify: script] Opening a note at a fragment, from a link (RD-64, RD-102), travels to that heading or footnote once the note is shown. omarky: an anchor in the launch target scrolls to that heading or footnote once the file has loaded (`reader/ui/app/ReaderWindow.qml:190-195`).
- **RD-05** [quire] [verify: -] Closing Quire's window quits the app. omarky quit when the last of its windows closed (`reader/ui/shell.qml:49-53`).
- **RD-06** [quire] [verify: -] Quire names the note in the sidebar's Open list and in the title bar. omarky titled the window "OmarkyReader" followed by the file name (`reader/ui/app/ReaderWindow.qml:75`).
- **RD-07** [quire] [verify: -] Quire's window opens at 1280 by 800. omarky's default was 1180 by 900 (`reader/ui/app/ReaderWindow.qml:76-77`).
- **RD-08** [quire] [verify: -] Quire keeps its own recent notes list (newest first, 30 entries, in `state.json`). omarky kept 20 recent files (`services/Prefs.qml:7-31`, `reader/ui/app/ReaderWindow.qml:183-186`).
- **RD-09** [adapted: 0002] [verify: state] Headless runs and tests never write the user's `state.json`, so they never change the remembered zoom (decision 0002) or the recent list. omarky: screenshot and test runs never read or write zoom or recents (`services/Prefs.qml:9`, `services/Prefs.qml:14`).

## 2. Live reload

- **RD-10** [quire] [verify: -] Quire's watcher checks the notes folder every 1.5 s and takes in outside changes, so a burst of writes inside one check lands once. Notes with unsaved edits are skipped and their edits win (`quire: desktop/src/watch.rs`, `apply_outside_changes`). omarky watched the file and reloaded 150 ms after the last change (`services/Doc.qml:103-110`).
- **RD-11** [adapted: 0001] [verify: script] When the note's text changes from outside the page (the watcher, a sync download, or a locked note re-opened with its key after an outside change), the page stays on the same block at the same offset. The block is found by its type and source text, the nearest such block by index when the text repeats. A note with unsaved edits is not changed by the watcher (RD-10), so its page does not move. omarky: the same rule after each file reload (`components/DocView.qml:240-283`).
- **RD-12** [in] [verify: script] If that block's text changed, the page falls back to the block nearest by source line, keeping the offset into it, capped at the block's height (`components/DocView.qml:271-282`).
- **RD-13** [in] [verify: script] A page at the very top stays at the top after an outside change (`components/DocView.qml:248`, `components/DocView.qml:261`).
- **RD-14** [adapted: 0001] [verify: state] Each outside change to the note pulses the "reloaded" mark (RD-33). omarky: each reload outside changes mode pulses the mark in the header (`reader/ui/app/ReaderWindow.qml:199`).
- **RD-15** [in] [verify: script, state] An open find refreshes its matches when the note's text changes and keeps the current index, clamped to the new count (`components/MarkdownView.qml:199-204`).
- **RD-16** [quire] [verify: -] When a note is deleted outside Quire, the watcher removes it and closes it, unless it has unsaved edits. An editor that saves by atomic rename never leaves the file missing, so the note stays. omarky kept a deleted file on screen under a yellow banner and polled for its return every 1000 ms (`services/Doc.qml:99-101`, `services/Doc.qml:112-118`, `reader/ui/app/ReaderWindow.qml:536-544`, `docs/decisions/0010-window-behaviour-choices.md:10-14`).
- **RD-17** [quire] [verify: -] Quire reads the whole folder when it opens it, so a note in a tab always has text. omarky showed "Cannot open <file name>" for a file it could not read and retried every 1000 ms (`reader/ui/app/ReaderWindow.qml:624`, `services/Doc.qml:112-118`).
- **RD-18** [adapted: 0001] [verify: script] Ctrl+R (omarky preset) reads the note from disk at once, without waiting for the watcher, and applies it by the rules of RD-11. A note with unsaved edits is left as it is. omarky: Ctrl+R reloads the file by hand (`reader/ui/app/ReaderWindow.qml:412-413`).
- **RD-19** [adapted: 0001] [verify: script] A headless check runs this sequence against an open note in preview: append, insert three blocks above the reading position, editor-style atomic rename, a burst of 10 writes, delete and restore within one watcher period. After each step the top block must be the same block. omarky: the same sequence against a file, with delete and restore as separate steps (`reader/tools/check-live:2-5`).

## 3. Scrolling

- **RD-20** [adapted: 0003] [verify: script] j or Down scrolls down one line step, k or Up scrolls up (omarky preset). A line step is round(B × 1.6 × 3), 67 px at the default size, three body lines (`reader/ui/app/ReaderWindow.qml:437-438`, `components/DocView.qml:137`).
- **RD-21** [adapted: 0003] [verify: script] Space or PageDown scrolls down a page step, Shift+Space or PageUp up (omarky preset). A page step is max(line step, view height - 80 px), so 80 px of context stays on screen (`reader/ui/app/ReaderWindow.qml:439-440`, `components/DocView.qml:138`).
- **RD-22** [in] [verify: script] Key scrolls glide over 110 ms with an ease-out-cubic curve. A key pressed during a glide continues from where the glide is heading, so held or repeated keys never lose distance (`components/DocView.qml:132-136`, `components/DocView.qml:158-162`).
- **RD-23** [adapted: 0003] [verify: script] `g g` (the second g within 900 ms) or Home goes to the top. G or End goes to the bottom (omarky preset) (`reader/ui/app/ReaderWindow.qml:428-442`, `reader/ui/app/ReaderWindow.qml:472`).
- **RD-24** [in] [verify: script] Long jumps (top, bottom, outline, links, anchors) travel in two parts. If the target is more than 1.5 view heights away, the view first cuts to 0.8 view height short of it, then glides the rest in 240 ms. The landing is exact even when block heights were estimates (`components/DocView.qml:97-116`).
- **RD-25** [in] [verify: geometry] Jumping to a block lands its top 20 px below the top edge of the view (`components/DocView.qml:118-124`).
- **RD-26** [quire] [verify: -] Rune's winit (vendored in `quire: desktop/vendor/winit`) scales touchpad scrolling by 12 points per compositor unit, as Chromium does, which is what omarky moved per compositor pixel. Quire's own 5.3 scale was dropped with the move to upstream Rune (decision 0009). The goal is a swipe that travels as far as in the user's browser (`components/DocView.qml:165-172`, `components/DocView.qml:188-195`, `docs/decisions/0011-touchpad-scrolling.md:1-17`).
- **RD-27** [adapted: 0012] [verify: unit, script, manual] The touchpad throws the page as Chrome on Linux does. When the fingers lift within 200 ms of the last movement, the page flings at the velocity fitted by least squares to the last 3 scroll frames, timed by the platform's event times. It travels v² / 2a, with a = 2,645.7 points per s², at most three window heights, over |v| / 2300 seconds (at most 2 s) on Chrome's ease-out cubic Bézier, and stops dead at the page's ends. Fingers landing on the touchpad (the Wayland hold gesture) or a new swipe stop it where it is. A swipe that begins within 50 ms of that stop, heads the same way and lifts above 350 points a second adds the stopped fling's start speed to its own. It flings under reduced motion too, as Chrome does. omarky: speed decays as dv/dt = -3.7 (v + 172) from the last 3 frames, starting above 50 px/s and capped at 20,946 px/s, with no stop by a hold, no boost, and no fling under reduced motion (`components/DocView.qml:200-237`).
- **RD-28** [in] [verify: script] A mouse wheel notch (120 units) glides 120 px like a key scroll (`components/DocView.qml:197-198`, `components/DocView.qml:170-171`).
- **RD-29** [in] [verify: script] A mouse drag on the page selects text and never scrolls. Scrolling at the bounds stops dead with no overshoot (`components/DocView.qml:66-69`, `docs/decisions/0011-touchpad-scrolling.md:21-23`).
- **RD-30** [in] [verify: bench] Scrolling a 5,400-line note with about 1,100 blocks holds a mean step under 1 ms and a worst step under one 60 Hz frame. omarky measured a worst step of about 19 ms, one first-use highlighter compile, and warmed code languages one per 40 ms after open. Quire measures first and warms only if its highlighter needs it (`docs/decisions/0009-scrolling-performance.md:1-15`).

## 4. Header

- **RD-31** [quire] [verify: -] Quire's title bar, with the note's folder and name, takes the header's place. omarky's header was 40 px tall on the bg colour with a hairlineSoft line at its bottom (`reader/ui/app/Header.qml:56-65`, `reader/ui/app/Header.qml:123`).
- **RD-32** [quire] [verify: -] Quire shows the note's folder and name in the title bar, and its name in the sidebar's Open list. A name or folder too long for its room ends in an ellipsis inside it, before the title bar's buttons: the folder keeps its end and the name its start. The same holds in the tree, the outline and the quick switcher, where a note's folder takes at most 220 pt so the name keeps the rest of the row. omarky showed a file icon, the file name and the `~`-abbreviated folder at the header's left (`reader/ui/app/Header.qml:67-95`).
- **RD-33** [adapted: 0001] [verify: state, geometry] After an outside change a "reloaded" mark appears after the note's name in the title bar, a 6 px accent dot and caption-size accent text spaced 5 px. It fades in over 120 ms, holds 1100 ms and fades out over 500 ms. omarky: the mark sits beside the file name in the header (`reader/ui/app/Header.qml:99-116`).
- **RD-34** [adapted: 0001] [verify: geometry, state] In preview mode the right group of Quire's title bar, beside the mode, focus and outline buttons, gains buttons with tooltips: Find "Find (Ctrl+F)", Copy all "Copy all (Ctrl+Shift+A)", Wide "Full width (w)" or "Readable width (w)" when wide, the names Settings gives the two widths, and Help "Shortcuts (?)". Quire's outline button gets the tooltip "Outline (o)". Tooltip keys come from the active keymap. Every tooltip in Quire that names a key writes it as "Name (Keys)" (`keymap::tip`). omarky writes "Find  Ctrl+F", with two spaces. The Fold and Versions buttons are deferred with changes mode. omarky: a row of chip buttons at the header's right, spaced 2 px, with the outline chip shown only with more than one heading (`reader/ui/app/Header.qml:176-212`).
- **RD-35** [in] [verify: state] A button shows as active (hover fill and hairline) while its feature is on: find open, wide, outline shown (`reader/ui/app/Header.qml:180-184`, `components/ChipButton.qml:21-23`).
- **RD-36** [deferred] [verify: -] The compact header below a window width of 1100 px only changes the changes-mode pill, so it waits for changes mode (`reader/ui/app/Header.qml:47-50`).
- **RD-37** [adapted: 0001] [verify: geometry] In preview mode a 2 px accent progress line runs along the top edge of the note area, just under the title bar. Its width is the reading progress, animated over 90 ms. omarky: the line runs along the very top of the window, hidden with no file (`reader/ui/app/ReaderWindow.qml:525-533`).
- **RD-38** [in] [verify: unit, state] Progress is the source line of the top block over the note's line count. Over the last screen it eases the rest of the way to 1 by remaining distance. At the very top it is 0, at the end 1 (`components/DocView.qml:47-60`).

## 5. Outline

- **RD-39** [adapted: 0007] [verify: geometry, script] The outline is Quire's outline panel on the right, at Quire's width (236 px today), sliding in and out over 160 ms ease-out-cubic. omarky: a 264 px panel on the left (`reader/ui/app/ReaderWindow.qml:552-571`).
- **RD-40** [adapted: 0007] [verify: state] The outline shows when it is turned on, focus mode is off and a note is open. In a window at least 1100 px wide it docks beside the page. In a narrower window its button opens it as a drawer over the page's right edge instead, on the page colour, with a shadow on its edge and the same slide. The page keeps its width under it, under the modal scrim. A press on the scrim, Esc, the button or a picked heading closes it. A press on another title bar button closes it and does what that button does, as with the sidebar's drawer. While open it holds the keyboard, as both drawers do: focus moves into it, Tab stays in it, keys and the wheel never reach the page behind, and focus goes back where it was when it closes. Opening the palette, the quick switcher, Settings, a dialog, help or the edit sheet closes either drawer, so it never shows over or through them. The drawer and the find bar never show together: opening one closes the other. This narrow outline is not saved, so a narrow window never opens with the page covered. Quire's outline also lists backlinks, so it does not hide when the note has fewer than two headings. omarky: wanted by default at a window width of at least 1180 px, and shown only when the document has more than one heading (`reader/ui/app/ReaderWindow.qml:43-44`).
- **RD-41** [adapted: 0003] [verify: script] O, o or Ctrl+\ toggles the outline (omarky preset). In that preset Quire's sidebar toggle moves to a free key (`reader/ui/app/ReaderWindow.qml:416-417`, `reader/ui/app/ReaderWindow.qml:447`).
- **RD-42** [adapted: 0007] [verify: golden] The panel's background is alpha(fg, light 0.025 / dark 0.02), with a 1 px hairlineSoft edge on its left, the side that faces the page. omarky: the hairline is on the right edge of a left panel (`components/Outline.qml:43-45`).
- **RD-43** [adapted: 0008] [verify: geometry, golden] A "CONTENTS" title sits at (22 px, 18 px), on the rows' text edge as every panel caption is, UI caption, bold, letter spacing 1, faint. omarky puts it at 20 px (`components/Outline.qml:47-59`).
- **RD-44** [in] [verify: geometry] It lists h1 to h4 only. Depth is the level minus the shallowest level present. Each row's text starts at 22 px + depth × 14 px (`components/Outline.qml:32-34`, `components/Outline.qml:97`).
- **RD-45** [in] [verify: geometry] Row height is label height + 12 px. Depth 0 is UI body, bold. Deeper rows are UI bodySmall. Long titles elide on the right. A heading with no text shows "Untitled" (`components/Outline.qml:84`, `components/Outline.qml:99-106`).
- **RD-46** [in] [verify: state] Row colours are text for the current row, dim for depth 0 and mix(dim, faint, 0.5) deeper. Colour changes take 120 ms (`components/Outline.qml:103`, `components/Outline.qml:108`).
- **RD-47** [adapted: 0008] [verify: state, geometry] The current section is the last listed heading at or above the top block. Its row gets alpha(accent, 0.1) shading, and hovered rows get the hover fill. Shading is inset 10 px each side with radius max(3, rounding - 2). Quire keeps its accent dot as well: a 5 px accent dot 13 px left of the current row's text, in the row's left padding, so the text keeps its place. It moves with the scroll as the shading does. omarky has no accent bar and no dot (`components/Outline.qml:35-41`, `components/Outline.qml:86-92`, `reader/ui/app/ReaderWindow.qml:560-567`, `docs/decisions/0012-changes-as-a-mode-of-reading.md:18`).
- **RD-48** [in] [verify: script] The current row is kept in view as the reader scrolls (`components/Outline.qml:71-73`).
- **RD-49** [in] [verify: script] Clicking a row travels to that heading (RD-24, RD-25) (`components/Outline.qml:143-144`, `reader/ui/app/ReaderWindow.qml:568`).
- **RD-50** [in] [verify: geometry] The list starts 10 px under the title and ends 12 px above the bottom (`components/Outline.qml:62-67`).

## 6. Reading width

- **RD-51** [adapted: 0003] [verify: script] W or w toggles between reading width (68 characters) and wide width (100 characters) (omarky preset). Wide is for tables and code. The toggle is not saved (`reader/ui/app/ReaderWindow.qml:446`, `components/DocView.qml:19`, `README.md:35`).

## 7. Find

- **RD-52** [adapted: 0003] [verify: script] Ctrl+F or `/` opens the find bar with focus in its field (omarky preset). Reopening selects the existing text. Quire's search of all notes keeps its own key (`reader/ui/app/ReaderWindow.qml:407-409`, `reader/ui/app/ReaderWindow.qml:455`, `components/FindBar.qml:16`).
- **RD-53** [adapted: 0007] [verify: geometry] The bar is 380 by 40 px, popup background, 1 px hairline border, radius max(4, rounding). It floats over the page 16 px from the right edge of the note area, so it never covers the outline, and 10 px under the top of the note area. omarky: 16 px from the window's right edge and 10 px under the header, or under the banner when one shows (`components/FindBar.qml:18-23`, `reader/ui/app/ReaderWindow.qml:634-636`).
- **RD-54** [in] [verify: geometry, state] The bar holds a faint search icon, the field with ghost placeholder "Find in document", a count "i / n" in faint or "No matches" in red, and three buttons with tooltips "Previous (Shift+Enter)", "Next (Enter)" and "Close (Esc)", in Quire's tooltip format (RD-34) (`components/FindBar.qml:26-88`).
- **RD-55** [in] [verify: unit] Matching is case-insensitive plain substring, non-overlapping, across every paragraph, heading, list item, quote, alert, footnote, code block, diff and table cell, including blocks not yet drawn. Front matter, raw HTML and image captions are not searched (`lib/blocks.js:455-482`, `lib/inline.js:75-79`, `docs/decisions/0010-window-behaviour-choices.md:3-7`).
- **RD-56** [in] [verify: script] Search runs 40 ms after the last keystroke, or 120 ms for notes over 400 blocks (`reader/ui/app/ReaderWindow.qml:644-647`).
- **RD-57** [in] [verify: script] The first match chosen is the first one at or below the top of the view, else the first in the note (`components/MarkdownView.qml:173-180`).
- **RD-58** [adapted: 0003] [verify: script] Enter and n go to the next match, Shift+Enter and N to the previous, wrapping at both ends (omarky preset). n and N work after the bar loses focus while a query exists (`components/FindBar.qml:48-52`, `components/MarkdownView.qml:182-185`, `reader/ui/app/ReaderWindow.qml:450-451`).
- **RD-59** [in] [verify: state, golden] Every match is painted in the text with the find background. The current one is solid accent with bg-coloured text (`services/Reading.qml:87-89`, `lib/inline.js:139-140`).
- **RD-60** [adapted: 0009] [verify: script] A current match off screen is brought to the centre of the view. A visible match does not move the page if it lies between 20% and 75% of the view height. Otherwise the page glides in 200 ms to put it 35% from the top. Quire keeps a match still while its block lies in that band and centres any other (decision 0009) (`components/MarkdownView.qml:186-191`, `components/DocView.qml:150-155`).
- **RD-61** [in] [verify: script] Esc closes find and clears the query and the highlights (`components/FindBar.qml:52`, `reader/ui/app/ReaderWindow.qml:464-470`).

## 8. Links

- **RD-62** [in] [verify: script] `#anchor` links travel to that heading or footnote in the page (`components/MarkdownView.qml:136`).
- **RD-63** [in] [verify: unit] http, https, mailto and ftp links open in the default browser or mail client. Quire today hands any URL to the system, and this narrows it to these schemes (`components/MarkdownView.qml:142-146`).
- **RD-64** [adapted: 0001] [verify: script] A link to a Markdown file (.md, .markdown, .mdown, .mkd, .mdx, any case) that resolves to a note in the notes folder opens that note in a tab at its fragment (RD-04). Relative paths resolve against the note's folder and `~` expands to home. omarky: it opens the file in a new reader window (`components/MarkdownView.qml:147-154`).
- **RD-65** [in] [verify: script] A link to the same note (empty path or the same path) just travels to its fragment (`components/MarkdownView.qml:151`).
- **RD-66** [in] [verify: unit] Links to other local files open in the system default app (`components/MarkdownView.qml:155`).

## 9. Copy

- **RD-67** [adapted: 0003] [verify: script] Ctrl+C copies the current text selection and toasts "Copied" (omarky preset) (`reader/ui/app/ReaderWindow.qml:405-406`, `components/MarkdownView.qml:107-109`).
- **RD-68** [in] [verify: script] A right-click on any block copies its exact Markdown source lines and toasts "Copied the block's Markdown". For a locked note that is open, the source is the plain text (`components/DocView.qml:345-349`, `components/MarkdownView.qml:116-119`).
- **RD-69** [adapted: 0003] [verify: script] Ctrl+Shift+C copies the source of the block under the pointer, with the same toast (omarky preset) (`reader/ui/app/ReaderWindow.qml:402`).
- **RD-70** [adapted: 0003] [verify: script] Ctrl+Shift+A or the Copy all button copies the whole note and toasts "Copied the whole file" (omarky preset) (`reader/ui/app/ReaderWindow.qml:403`).
- **RD-71** [adapted: 0001] [verify: script] The heading `#` copies the note's path relative to the notes folder plus `#slug` (BLK-16). A code block's Copy chip copies the code (BLK-51). omarky: the heading `#` copies `file.md#slug` with the file's own name.
- **RD-72** [in] [verify: script] Esc with a selection, while find is closed, clears the selection (`reader/ui/app/ReaderWindow.qml:456-460`).

## 10. Ticking tasks

- **RD-73** [adapted: 0001] [verify: unit] Clicking a task box flips that item's box in the note's text, `[ ]` to `[x]` and back. Exactly one character changes, on the item's marker line, and line endings are kept. Quire's save path writes it (RD-74). omarky: the flip goes straight into the file (`docs/decisions/0020-task-boxes-tick-in-the-file.md:3-7`, `lib/blocks.js:114-130`).
- **RD-74** [adapted: 0001] [verify: script, state] The flip changes the note's text in memory the way an edit does (`quire: set_text`). An unlocked note's text is sealed before it reaches the folder copy, the note is marked dirty, and the Writer thread writes it with a temporary file and a rename. The page draws from that text, so it shows the tick at once and never shows a tick the note's text does not have. omarky: the whole file is written atomically and the page shows the new state only when the file reloads (`reader/ui/app/ReaderWindow.qml:364-387`).
- **RD-75** [adapted: 0006] [verify: state] Boxes take clicks only in preview mode on an open note while the edit sheet is closed. A locked note shows the lock panel, so its boxes take clicks only while it is unlocked. omarky also required the newer side to be Now in changes mode and refused removed items. That part is deferred with changes mode (`reader/ui/app/ReaderWindow.qml:367-368`, `components/blocks/ListBlock.qml:111-114`).
- **RD-76** [adapted: 0001] [verify: unit] If the clicked line is not a task item in the note's current text, nothing changes and the toast says "That task is not where it was. Try again". Quire draws the page from the current text, so this guards against a click that lands in the same frame as an outside change. omarky: the check runs against the file on disk (`reader/ui/app/ReaderWindow.qml:373`).
- **RD-77** [adapted: 0001] [verify: script] A failed write shows in Quire's error notice, as every failed note write does. Clicks never wait on a write, because each click changes the text in memory. omarky: a failed write toasts "Could not save: <error>", and a second click while a write is in flight is ignored (`reader/ui/app/ReaderWindow.qml:370`, `reader/ui/app/ReaderWindow.qml:386`).

## 11. Editing entry points

- **RD-78** [adapted: 0003] [verify: script] e edits the block under the pointer, else the block holding the selection, else the top block. E (Shift+e) edits that block's section, from its heading to before the next heading of the same or higher rank (omarky preset) (`reader/ui/app/ReaderWindow.qml:310-342`, `reader/ui/app/ReaderWindow.qml:448-449`).
- **RD-79** [adapted: 0006] [verify: state] The sheet opens only on an open note in preview mode, and on a locked note only while it is unlocked. The refusals that belong to changes mode ("Compare with Now to edit the file", "Open the fold to edit what is in it", "That text is not in the file now") are deferred with it. omarky: nothing happens without a path, before load or while missing (`reader/ui/app/ReaderWindow.qml:345-352`). The sheet itself is in `edit-sheet.md`.

## 12. Keys

- **RD-80** [in] [verify: script] While the help sheet is open, only Esc, `?` and q act, and they close it. Every other key is swallowed (`reader/ui/app/ReaderWindow.qml:395-400`).
- **RD-81** [in] [verify: script] While the edit sheet is open, all keys go to it (`reader/ui/app/ReaderWindow.qml:394`).
- **RD-82** [adapted: 0003] [verify: unit] In the omarky preset the reader's Ctrl keys are C copy, F find, O open (Quire's quick switcher), R read from disk (RD-18), W close the tab, \ outline, + or = zoom in, - zoom out and 0 zoom reset. Other Ctrl keys reach Quire's window shortcuts. omarky: O opened the file chooser and W closed the window (`reader/ui/app/ReaderWindow.qml:404-426`).
- **RD-83** [in] [verify: unit] Reader keys with Alt or Super are ignored (`reader/ui/app/ReaderWindow.qml:427`).
- **RD-84** [adapted: 0003] [verify: unit] In the omarky preset the reader's plain keys are j k Down Up, Space Shift+Space PageDown PageUp, Home End, g g, G, ? help, q close the tab, w W wide, o O outline, e E edit, n N find step (with a query), / find and Esc. The changes-mode keys (v V versions, f F fold, n p change step) are deferred with changes mode. omarky: q closed the window (`reader/ui/app/ReaderWindow.qml:428-461`).
- **RD-85** [adapted: 0006] [verify: script] Esc acts in this order. Close find, else clear the selection, else run Quire's own Esc (close menus and dialogs, then leave focus mode). omarky started by closing the version list and ended by leaving a comparison. Both steps are deferred with changes mode (`reader/ui/app/ReaderWindow.qml:401`, `reader/ui/app/ReaderWindow.qml:456-460`).

## 13. Help sheet

- **RD-86** [adapted: 0003] [verify: script] `?` (omarky preset) or the Help button opens the shortcut sheet over the modal scrim, a dark neutral of the page's hue at 40% in a light theme and 60% in a dark one (decision 0005; omarky: alpha(bg, 0.72)). The sheet is modal: the note behind takes no wheel, press or key while it is open. Clicking outside the card closes it (`reader/ui/app/ShortcutSheet.qml:13`, `reader/ui/app/ReaderWindow.qml:443`).
- **RD-87** [adapted: 0001] [verify: geometry] The card is min(window width - 48 px, 520 px) wide and at most the window height - 48 px tall, its rows scrolling inside it when they do not fit (omarky's card does not scroll), popup background, 1 px hairline, padding 24 px sideways and 22 px vertically, groups 18 px apart, rows 24 px tall, a key cap and then the description at x 190 px in UI bodySmall text colour (`reader/ui/app/ShortcutSheet.qml:41-92`).
- **RD-88** [adapted: 0001] [verify: state] "Quire <version>" sits top-right in ghost caption. omarky: "OmarkyReader <version>" (`reader/ui/app/ShortcutSheet.qml:49-55`).
- **RD-89** [in] [verify: golden] Group titles are UI caption bold capitals in faint: the caption style's faint, which reads at 4.5:1 on the card in every theme (`reader/ui/app/ShortcutSheet.qml:72-76`).
- **RD-90** [adapted: 0003] [verify: unit] The groups and rows are generated from the active keymap's reader context, in omarky's order and wording. The Read group has "Scroll", "Page down / up", "Top / bottom", "Outline", "Readable / full width" (Settings' names for the two widths) and "Zoom text". The Find & copy group has "Find", "Next / previous match", "Copy a block's Markdown" (Right-click), "Copy the block under the mouse" and "Copy the whole file". The Edit group has "Edit the block under the mouse", "Edit its section" and "Save to the file / cancel". The File group has "Open a note", "Read from disk", "Close the tab" and "This sheet". The Changes group and the "In the sheet, rewrite with Claude" row are deferred (decision 0006). omarky: a fixed list with its own keys and a File group of "Open a file", "Reload" and "Close window" (`reader/ui/app/ShortcutSheet.qml:14-27`).
- **RD-91** [in] [verify: geometry] A key cap is UI caption text in dim on a raised fill with a 1 px hairline border, padded 12 px wide and 6 px tall in total, radius max(3, rounding - 2) (`components/KeyCap.qml:9-23`).

## 14. Empty state, opening and dropping

- **RD-92** [quire] [verify: -] Quire shows its own screens when no note is open (no tab, empty folder, welcome). omarky showed a centred column placed 38% down the free height (`reader/ui/app/EmptyState.qml:17-20`).
- **RD-93** [quire] [verify: -] Quire's empty screens replace omarky's icon, "Open a Markdown file" line and Ctrl+O hint (`reader/ui/app/EmptyState.qml:22-47`).
- **RD-94** [quire] [verify: -] Quire's quick switcher and file tree give the recent and folder lists. A long name ends in an ellipsis in both (RD-32). omarky listed up to 10 recent files in its empty state (`reader/ui/app/EmptyState.qml:50-100`).
- **RD-95** [quire] [verify: -] Quire opens notes from its file tree and quick switcher. omarky's Ctrl+O opened the system file chooser with Markdown filters (`reader/ui/app/ReaderWindow.qml:474-480`).
- **RD-96** [quire] [verify: -] Quire takes dropped image files and links them into the note. Notes live in the folder, so a dropped Markdown file does not open a window. omarky loaded the first dropped file into an empty window and opened the rest in new windows (`reader/ui/app/ReaderWindow.qml:703-710`).
- **RD-97** [quire] [verify: -] Quire rings the note area with an inset accent line while files hover over it. omarky drew an inset overlay with an alpha(accent, 0.06) fill and a 2 px alpha(accent, 0.5) border (`reader/ui/app/ReaderWindow.qml:712-720`).

## 15. Toasts and banners

- **RD-98** [in] [verify: geometry, script] A toast is a pill at the bottom centre, 28 px above the bottom edge, popup background, 1 px hairline, padded 28 px wide and 14 px tall in total, UI text. It rises 8 px over 160 ms ease-out-cubic, fades over 140 ms, and hides after 1600 ms. Errors still go to Quire's notice panel (`components/Toast.qml:18-40`).
- **RD-99** [quire] [verify: -] omarky's banner only told of a missing file. Quire closes the tab of a deleted note (RD-16) and reports other problems in its notice panel. omarky's banner was 34 px tall under the header, tinted by its tone (`components/Banner.qml:10-23`, `reader/ui/app/ReaderWindow.qml:535-545`).

## 16. Hover and pointer

- **RD-100** [in] [verify: state] The block under the pointer is tracked all the time, for Ctrl+Shift+C and e (`components/DocView.qml:339-344`).
- **RD-101** [in] [verify: state] Hover feedback on the page is limited to link underlines, the heading `#`, the code Copy chip, the task box border and the table or image tooltips. Blocks never highlight on hover (`components/DocView.qml:292-361`, inferred).

## 17. Quire additions

- **RD-102** [adapted: 0001] [verify: script] A click on a `[[wiki]]` link in the page opens the note it names in a tab, using Quire's name index. A name with no note creates that note, as Quire does today. omarky has no wiki links (`quire: desktop/src/app.rs`, `open_link`).
- **RD-103** [adapted: 0001] [verify: script] Each tab keeps its own reading position in preview. Switching back to a tab shows the page where it was left. Find closes on a tab switch. omarky has one file per window, so the window kept the position.

## Open questions

- Where the preview-mode buttons go (RD-34). Recommended default. The right group of the title bar, shown only in preview mode, so source mode keeps its current bar.
- Outline width (RD-39). Recommended default. Keep Quire's 236 px, because decision 0007 keeps Quire's layout and the column lean depends on both panel widths.
- Whether the omarky outline styling (RD-42 to RD-47) also applies to the outline in source mode, and where backlinks go. Recommended default. One outline styled the omarky way in both modes, with backlinks kept below the headings.
- How `W` relates to Quire's Settings width ("readable" or full). Recommended default. The setting picks the measure a note opens with, and `W` toggles it for the session without saving. Resolved in phase 6 as recommended.
- Whether `q` should close the tab (RD-84), since a single letter now closes something in a multi-tab window. Recommended default. Keep it in the omarky preset. Closing a tab loses nothing, and the note stays in the folder. Resolved in phase 6 as recommended.
- A relative `.md` link that points outside the notes folder (RD-64). Recommended default. Open it in the system default app, as for other local files (RD-66). Resolved in phase 6 as recommended.
- `[[note#Heading]]` links (RD-102). Quire ignores the fragment today. Recommended default. Travel to the heading, by RD-04.
- What the heading `#` copies (RD-71). A `[[Note#Heading]]` wiki link would paste better inside Quire. Recommended default. The relative path with `#slug`, as in omarky, because it works in any Markdown tool. Resolved in phase 6 as recommended.
- Whether a note deleted outside Quire should stay on screen as in omarky (RD-16) instead of closing it. Recommended default. Keep Quire's behaviour, because the file tree already shows the note is gone.
