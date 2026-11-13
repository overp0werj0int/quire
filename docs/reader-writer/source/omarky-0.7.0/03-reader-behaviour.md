# OmarkyReader behaviour spec (extracted from omarky)

Source repo is `/home/p0werj0int/Projects/omarky` (version 0.7.0). Paths are relative to it and cited as `path:line`. Units and colour roles follow `01-page-and-typography.md` (`B` = body size, `S(n)` = chrome length). The edit sheet is specified in `05-edit-sheet-and-rewrite.md` and changes mode in `06-changes-mode.md`. This file covers plain reading and the shared chrome.

---

## 1. Windows and files

- **RD-01** One window per opened file. Opening the same file twice gives two windows (`reader/ui/shell.qml:45-47`).
- **RD-02** A single app instance serves all windows. A second launch hands its files to the running instance and exits (`reader/ui/shell.qml:13`, `reader/ui/shell.qml:60-70`, `reader/ui/shell.qml:122`).
- **RD-03** A target is `path` or `path#anchor`. A `#` counts as an anchor only when it comes after the last `/`, so a folder name containing `#` is kept. A target may add TAB FROM TAB TO to open comparing (`reader/ui/shell.qml:33-42`).
- **RD-04** An anchor in the target scrolls to that heading (or footnote) once the file has loaded (`reader/ui/app/ReaderWindow.qml:190-195`).
- **RD-05** Closing the last window quits the app (`reader/ui/shell.qml:49-53`).
- **RD-06** The window title is "OmarkyReader — <file name>", or "OmarkyReader" with no file (`reader/ui/app/ReaderWindow.qml:75`).
- **RD-07** The default window size is S(1180) × S(900) (`reader/ui/app/ReaderWindow.qml:76-77`).
- **RD-08** Each successfully opened file goes to the top of the recent list (newest first, at most 20, a re-open moves it up). Recents persist across launches (`services/Prefs.qml:7-31`, `reader/ui/app/ReaderWindow.qml:183-186`).
- **RD-09** Screenshot and test runs never read or write zoom or recents (`services/Prefs.qml:9`, `services/Prefs.qml:14`).

## 2. Live reload

- **RD-10** The file is watched. A change on disk reloads the page after a 150 ms debounce, so a burst of writes reloads once (`services/Doc.qml:103-110`).
- **RD-11** After a reload the reader stays on the same block at the same offset. The block is found by its type and source text, the nearest such block by index when the text repeats (`components/DocView.qml:240-283`).
- **RD-12** If that block's text changed, the reader falls back to the block nearest by source line, keeping the offset into it (capped at the block's height) (`components/DocView.qml:271-282`).
- **RD-13** A reader at the very top of the page stays at the top after a reload (`components/DocView.qml:248`, `components/DocView.qml:261`).
- **RD-14** Each reload outside changes mode pulses a "reloaded" mark in the header (RD-31) (`reader/ui/app/ReaderWindow.qml:199`).
- **RD-15** An open find refreshes its matches on reload and keeps the current index, clamped to the new count (`components/MarkdownView.qml:199-204`).
- **RD-16** A deleted or moved file keeps its last content on screen with a yellow banner "<file> was deleted or moved. Showing the last version; it reappears if the file comes back." The path is re-checked every 1000 ms and the page returns to live when the file reappears. This covers editors that save by delete and recreate (`services/Doc.qml:99-101`, `services/Doc.qml:112-118`, `reader/ui/app/ReaderWindow.qml:536-544`, `docs/decisions/0010-window-behaviour-choices.md:10-14`).
- **RD-17** A file that could not be opened at all shows the empty state with "Cannot open <file name>" in dim, and is retried every 1000 ms (`reader/ui/app/ReaderWindow.qml:624`, `services/Doc.qml:112-118`).
- **RD-18** Ctrl+R reloads by hand (`reader/ui/app/ReaderWindow.qml:412-413`).
- **RD-19** The check the suite runs is append, insert three blocks above the reading position, editor-style atomic rename, a burst of 10 writes, delete, restore. After each, the top block must be the same block (`reader/tools/check-live:2-5`).

## 3. Scrolling

- **RD-20** j or Down scrolls down one line step, k or Up scrolls up. A line step is round(B × 1.6 × 3) [67] px, three body lines (`reader/ui/app/ReaderWindow.qml:437-438`, `components/DocView.qml:137`).
- **RD-21** Space or PageDown scrolls down a page step, Shift+Space or PageUp up. A page step is max(line step, view height − S(80)), so 80 px of context stays on screen (`reader/ui/app/ReaderWindow.qml:439-440`, `components/DocView.qml:138`).
- **RD-22** Key scrolls glide over 110 ms with an ease-out-cubic curve. A key pressed during a glide continues from where the glide is heading, so held or repeated keys never lose distance (`components/DocView.qml:132-136`, `components/DocView.qml:158-162`).
- **RD-23** `g g` (second g within 900 ms) or Home goes to the top. G or End goes to the bottom (`reader/ui/app/ReaderWindow.qml:428-442`, `reader/ui/app/ReaderWindow.qml:472`).
- **RD-24** Long jumps (top, bottom, outline, links, anchors) travel in two parts. If the target is more than 1.5 view heights away the view first cuts to 0.8 view height short of it, then glides the rest in 240 ms. The landing is exact even when block heights were estimates (`components/DocView.qml:97-116`).
- **RD-25** Jumping to a block lands its top S(20) below the top edge of the view (`components/DocView.qml:118-124`).
- **RD-26** Touchpad scrolling moves 12 px per compositor pixel, matching Chromium on Wayland (`components/DocView.qml:165-172`, `components/DocView.qml:188-195`, `docs/decisions/0011-touchpad-scrolling.md:1-17`).
- **RD-27** On finger lift, if the last movement was within 200 ms, the page flings at the velocity of the last 3 movement frames. Speed decays as dv/dt = −3.7 (v + 172), that is speed(t) = (|v0| + 172) × e^(−3.7 t) − 172, stopping when it reaches 0. Flings start only above 50 px/s and are capped at 20,946 px/s. No fling under reduced motion (`components/DocView.qml:200-237`).
- **RD-28** A mouse wheel notch (120 units) glides 120 px like a key scroll (`components/DocView.qml:197-198`, `components/DocView.qml:170-171`).
- **RD-29** Mouse drag on the page selects text and never scrolls. Scrolling at the bounds stops dead with no overshoot (`components/DocView.qml:66-69`, `docs/decisions/0011-touchpad-scrolling.md:21-23`).
- **RD-30** Scrolling a 5,400-line document with about 1,100 blocks must hold a mean step under 1 ms and a worst step of about 19 ms (one first-use highlighter compile). Code languages are warmed one per 40 ms right after open to achieve this (`docs/decisions/0009-scrolling-performance.md:1-15`).

## 4. Header

- **RD-31** The header is S(40) tall on the bg colour with a hairlineSoft line at its bottom. Left margin S(16), right margin S(10) (`reader/ui/app/Header.qml:56-65`, `reader/ui/app/Header.qml:123`).
- **RD-32** Left side, spaced S(8), is a faint file icon, the file name in bold UI body text (elided in the middle), and the folder in faint UI bodySmall with the home folder shown as `~` (elided on the left) (`reader/ui/app/Header.qml:67-95`).
- **RD-33** After a reload a "reloaded" mark appears beside the name, an S(6) accent dot and caption-size accent text spaced S(5). It fades in over 120 ms, holds 1100 ms, fades out over 500 ms (`reader/ui/app/Header.qml:99-116`).
- **RD-34** Right side, spaced S(2), are chip buttons with tooltips. Find "Find  Ctrl+F". Copy all "Copy all  Ctrl+Shift+A". Fold (changes mode only). Wide "Wide width  W" or "Reading width  W" when wide. Outline "Outline  O" (only with more than one heading). Versions (only with git history). Help "Shortcuts  ?" (`reader/ui/app/Header.qml:176-212`).
- **RD-35** A chip shows as active (hover fill and hairline) while its feature is on (find open, wide, outline shown) (`reader/ui/app/Header.qml:180-184`, `components/ChipButton.qml:21-23`).
- **RD-36** Below a window width of S(1100) the header goes compact (changes-mode pill drops dates and says "vs") (`reader/ui/app/Header.qml:47-50`).
- **RD-37** A 2 px accent progress line runs along the very top of the window, its width the reading progress, animated over 90 ms. Hidden with no file (`reader/ui/app/ReaderWindow.qml:525-533`).
- **RD-38** Progress is the source line of the top block over the file's line count. Over the last screen it eases the rest of the way to 1 by remaining distance. At the very top it is 0, at the end 1 (`components/DocView.qml:47-60`).

## 5. Outline

- **RD-39** The outline is a S(264) wide panel on the left, sliding in and out over 160 ms ease-out-cubic (`reader/ui/app/ReaderWindow.qml:552-571`).
- **RD-40** It is wanted by default when the window is at least S(1180) wide, and shown only when wanted, the document has more than one heading, and there is a file (`reader/ui/app/ReaderWindow.qml:43-44`).
- **RD-41** O, o or Ctrl+\ toggles it (`reader/ui/app/ReaderWindow.qml:416-417`, `reader/ui/app/ReaderWindow.qml:447`).
- **RD-42** Its background is alpha(fg, light 0.025 / dark 0.02) with a 1 px hairlineSoft right edge (`components/Outline.qml:43-45`).
- **RD-43** A "CONTENTS" title sits at (S(20), S(18)), UI caption, bold, letter spacing 1, faint (`components/Outline.qml:47-59`).
- **RD-44** It lists h1 to h4 only. Depth is the level minus the shallowest level present. Each row's text starts at S(22) + depth × S(14) (`components/Outline.qml:32-34`, `components/Outline.qml:97`).
- **RD-45** Row height is label height + S(12). Depth 0 is UI body, bold. Deeper rows are UI bodySmall. Long titles elide on the right. A heading with no text shows "Untitled" (`components/Outline.qml:84`, `components/Outline.qml:99-106`).
- **RD-46** Colours are text for the current row, dim for depth 0, mix(dim, faint, 0.5) deeper. Colour changes take 120 ms (`components/Outline.qml:103`, `components/Outline.qml:108`).
- **RD-47** The current section is the last listed heading at or above the top block. Its row gets alpha(accent, 0.1) shading, hovered rows get the hover fill. Shading is inset S(10) each side with radius max(3, shell rounding − 2). There is no accent bar (`components/Outline.qml:35-41`, `components/Outline.qml:86-92`, `reader/ui/app/ReaderWindow.qml:560-567`, `docs/decisions/0012-changes-as-a-mode-of-reading.md:18`).
- **RD-48** The current row is kept in view as the reader scrolls (`components/Outline.qml:71-73`).
- **RD-49** Clicking a row travels to that heading (RD-24, RD-25) (`components/Outline.qml:143-144`, `reader/ui/app/ReaderWindow.qml:568`).
- **RD-50** The list starts S(10) under the title and ends S(12) above the bottom (`components/Outline.qml:62-67`).

## 6. Reading width

- **RD-51** W or w toggles between reading width (68 characters) and wide width (100 characters). Wide is for tables and code (`reader/ui/app/ReaderWindow.qml:446`, `components/DocView.qml:19`, `README.md:35`). It is not persisted (inferred, no preference stores it).

## 7. Find

- **RD-52** Ctrl+F or `/` opens the find bar with focus in its field. Reopening selects the existing text (`reader/ui/app/ReaderWindow.qml:407-409`, `reader/ui/app/ReaderWindow.qml:455`, `components/FindBar.qml:16`).
- **RD-53** The bar is S(380) × S(40), popup background, 1 px hairline border, radius max(4, shell rounding), S(16) from the right edge and S(10) under the header (and under the banner when one shows). It floats over the page (`components/FindBar.qml:18-23`, `reader/ui/app/ReaderWindow.qml:634-636`).
- **RD-54** It holds a faint search icon, the field with ghost placeholder "Find in document", a count "i / n" in faint or "No matches" in red, and three chip buttons with tooltips "Previous  Shift+Enter", "Next  Enter", "Close  Esc" (`components/FindBar.qml:26-88`).
- **RD-55** Matching is case-insensitive plain substring, non-overlapping, across every paragraph, heading, list item, quote, alert, footnote, code block, diff and table cell, including blocks not yet drawn. Front matter, raw HTML and image captions are not searched (`lib/blocks.js:455-482`, `lib/inline.js:75-79`, `docs/decisions/0010-window-behaviour-choices.md:3-7`).
- **RD-56** Search runs 40 ms after the last keystroke, 120 ms for documents over 400 blocks (`reader/ui/app/ReaderWindow.qml:644-647`).
- **RD-57** The first match chosen is the first one at or below the top of the view, else the first in the document (`components/MarkdownView.qml:173-180`).
- **RD-58** Enter and n go to the next match, Shift+Enter and N to the previous, wrapping at both ends. n and N work after the bar loses focus while a query exists (`components/FindBar.qml:48-52`, `components/MarkdownView.qml:182-185`, `reader/ui/app/ReaderWindow.qml:450-451`).
- **RD-59** Every match is painted in the text with the find background. The current one is solid accent with bg-coloured text (`services/Reading.qml:87-89`, `lib/inline.js:139-140`).
- **RD-60** A current match off screen is brought to the centre of the view. A visible match does not move the page if it lies between 20% and 75% of the view height, else the page glides in 200 ms to put it 35% from the top (`components/MarkdownView.qml:186-191`, `components/DocView.qml:150-155`).
- **RD-61** Esc closes find and clears the query and highlights (`components/FindBar.qml:52`, `reader/ui/app/ReaderWindow.qml:464-470`).

## 8. Links

- **RD-62** `#anchor` links travel to that heading or footnote in the page (`components/MarkdownView.qml:136`).
- **RD-63** http, https, mailto and ftp links open in the default browser or mail client (`components/MarkdownView.qml:142-146`).
- **RD-64** A link to a Markdown file (.md, .markdown, .mdown, .mkd, .mdx, any case) opens it in a new reader window at its fragment. Relative paths resolve against the document's folder and `~` expands to home (`components/MarkdownView.qml:147-154`).
- **RD-65** A link to the same file (empty path or the same path) just scrolls to its fragment (`components/MarkdownView.qml:151`).
- **RD-66** Links to other local files open in the system default app (`components/MarkdownView.qml:155`).

## 9. Copy

- **RD-67** Ctrl+C copies the current text selection and toasts "Copied" (`reader/ui/app/ReaderWindow.qml:405-406`, `components/MarkdownView.qml:107-109`).
- **RD-68** Right-click on any block copies its exact Markdown source lines and toasts "Copied the block's Markdown" (`components/DocView.qml:345-349`, `components/MarkdownView.qml:116-119`).
- **RD-69** Ctrl+Shift+C copies the source of the block under the pointer, with the same toast (`reader/ui/app/ReaderWindow.qml:402`).
- **RD-70** Ctrl+Shift+A or the header's Copy all copies the whole file and toasts "Copied the whole file" (`reader/ui/app/ReaderWindow.qml:403`).
- **RD-71** The heading `#` copies `file.md#slug` (BLK-16). A code block's Copy chip copies the code (BLK-51).
- **RD-72** Esc with a selection (and find closed) clears the selection (`reader/ui/app/ReaderWindow.qml:456-460`).

## 10. Ticking tasks

- **RD-73** Clicking a task box flips that item's box in the file, `[ ]` to `[x]` and back, changing exactly one character on the item's marker line and keeping line endings (`docs/decisions/0020-task-boxes-tick-in-the-file.md:3-7`, `lib/blocks.js:114-130`).
- **RD-74** The whole file is written atomically. The page does not change the box itself, it shows the new state when the file reloads, so the page never shows a tick the file does not have (`reader/ui/app/ReaderWindow.qml:364-387`).
- **RD-75** Boxes are clickable only when the window has a path, the file is loaded and not missing, the edit sheet is closed, and (in changes mode) the newer side is Now. Removed items never take clicks (`reader/ui/app/ReaderWindow.qml:367-368`, `components/blocks/ListBlock.qml:111-114`).
- **RD-76** If the line is no longer that task item (the file changed underneath), nothing is written and the toast says "That task is not where it was. Try again" (`reader/ui/app/ReaderWindow.qml:373`).
- **RD-77** A failed write toasts "Could not save: <error>". A second click while a write is in flight is ignored (`reader/ui/app/ReaderWindow.qml:370`, `reader/ui/app/ReaderWindow.qml:386`).

## 11. Editing entry points

- **RD-78** e edits the block under the pointer, else the block holding the selection, else the top block. E (Shift+e) edits that block's section, from its heading to before the next heading of the same or higher rank (`reader/ui/app/ReaderWindow.qml:310-342`, `reader/ui/app/ReaderWindow.qml:448-449`).
- **RD-79** Editing is refused with a toast when comparing against an older newer side ("Compare with Now to edit the file"), on a fold ("Open the fold to edit what is in it") or on removed text ("That text is not in the file now"). Nothing happens without a path, before load, or while missing (`reader/ui/app/ReaderWindow.qml:345-352`). The sheet itself is in `05-edit-sheet-and-rewrite.md`.

## 12. Keys (complete map)

- **RD-80** While the help sheet is open, only Esc, `?` and q act (they close it) and every other key is swallowed (`reader/ui/app/ReaderWindow.qml:395-400`).
- **RD-81** While the edit sheet is open, all keys go to it (`reader/ui/app/ReaderWindow.qml:394`).
- **RD-82** Ctrl keys are C copy, F find, O open file, R reload, W close window, \ outline, + or = zoom in, − zoom out, 0 zoom reset. Other Ctrl keys pass through (`reader/ui/app/ReaderWindow.qml:404-426`).
- **RD-83** Keys with Alt or Super are ignored (`reader/ui/app/ReaderWindow.qml:427`).
- **RD-84** Plain keys are j k Down Up, Space Shift+Space PageDown PageUp, Home End, g g, G, ? help, q close, w W wide, o O outline, e E edit, v V versions (history only), f F fold (comparing only), n N find step (with a query), n p change step (comparing, no query), / find, Esc (`reader/ui/app/ReaderWindow.qml:428-461`).
- **RD-85** Esc acts in order, close the version list, else close find, else clear the selection, else stop comparing (`reader/ui/app/ReaderWindow.qml:401`, `reader/ui/app/ReaderWindow.qml:456-460`).

## 13. Help sheet

- **RD-86** `?` or the header's help chip opens the shortcut sheet over a scrim alpha(bg, 0.72). Clicking outside the card closes it (`reader/ui/app/ShortcutSheet.qml:13`, `reader/ui/app/ReaderWindow.qml:443`).
- **RD-87** The card is min(window width − S(48), S(520)) wide, popup background, 1 px hairline, padding S(24) sideways and S(22) vertically, groups S(18) apart, rows S(24) tall, a key cap then the description at x S(190) in UI bodySmall text colour (`reader/ui/app/ShortcutSheet.qml:41-92`).
- **RD-88** "OmarkyReader <version>" sits top-right in ghost caption (`reader/ui/app/ShortcutSheet.qml:49-55`).
- **RD-89** Group titles are UI caption bold capitals in faint (`reader/ui/app/ShortcutSheet.qml:72-76`).
- **RD-90** Groups and rows, in order (`reader/ui/app/ShortcutSheet.qml:14-27`). Changes (only with history) "V Choose the versions", "N  P Next / previous change", "F Only what changed / everything", "Esc Back to reading". Read "j  k  ↓  ↑ Scroll", "Space  Shift+Space Page down / up", "g g  G Top / bottom", "O  Ctrl+\ Outline", "W Reading / wide measure", "Ctrl+  Ctrl−  Ctrl+0 Zoom text". Find & copy "Ctrl+F Find", "Enter  Shift+Enter Next / previous match", "Right-click Copy a block's Markdown", "Ctrl+Shift+C Copy the block under the mouse", "Ctrl+Shift+A Copy the whole file". Edit "E Edit the block under the mouse", "Shift+E Edit its section", "Ctrl+Enter  Esc Save to the file / cancel", "Ctrl+J In the sheet, rewrite with Claude". File "Ctrl+O Open a file", "Ctrl+R Reload", "Ctrl+W  q Close window", "? This sheet".
- **RD-91** A key cap is UI caption text in dim on a raised fill with a 1 px hairline border, padded S(12) wide and S(6) tall in total, radius max(3, shell rounding − 2) (`components/KeyCap.qml:9-23`).

## 14. Empty state, opening and dropping

- **RD-92** A window with no file shows a centred column min(width − S(48), S(460)) wide, placed 38% down the free height (at least S(40) from the top), items spaced S(14) (`reader/ui/app/EmptyState.qml:17-20`).
- **RD-93** It shows the app icon at S(64) and 0.9 opacity, "Open a Markdown file" in prose at size(1) [18] text colour, then a Ctrl+O key cap with "or drop a file on this window" in faint UI bodySmall (`reader/ui/app/EmptyState.qml:22-47`).
- **RD-94** With recents, a "RECENT" caption in faint follows, then up to 10 rows S(32) tall, each a faint file icon, the name in UI bodySmall text and the `~`-abbreviated folder in faint. Hover gives the hover fill, click opens (`reader/ui/app/EmptyState.qml:50-100`).
- **RD-95** Ctrl+O opens the system file chooser titled "Open a Markdown file" with filters "Markdown (*.md *.markdown *.mdown *.mkd *.mdx)" and "All files (*)", starting in the current file's folder or home (`reader/ui/app/ReaderWindow.qml:474-480`).
- **RD-96** Dropping files on a window loads the first into this window if it is empty or failed, and opens every other in a new window (`reader/ui/app/ReaderWindow.qml:703-710`).
- **RD-97** While dragging over the window an overlay shows, inset S(10), fill alpha(accent, 0.06), 2 px border alpha(accent, 0.5), radius max(6, shell rounding) (`reader/ui/app/ReaderWindow.qml:712-720`).

## 15. Toasts and banners

- **RD-98** A toast is a pill at the bottom centre, S(28) above the bottom edge, popup background, 1 px hairline, padded S(28) wide and S(14) tall in total, UI text. It rises S(8) over 160 ms ease-out-cubic and fades over 140 ms, and hides after 1600 ms (`components/Toast.qml:18-40`).
- **RD-99** A banner is S(34) tall across the window under the header, fill alpha(tone, light 0.1 / dark 0.09), a bottom line alpha(tone, 0.25), an alert icon in the tone and UI text, floating over the page's top room rather than pushing the page down (`components/Banner.qml:10-23`, `reader/ui/app/ReaderWindow.qml:535-545`).

## 16. Hover and pointer

- **RD-100** The block under the pointer is tracked continuously for Ctrl+Shift+C and e (`components/DocView.qml:339-344`).
- **RD-101** Hover feedback on the page is limited to link underlines, the heading `#`, the code Copy chip, the task box border and table or image tooltips. Blocks themselves never highlight on hover (inferred, `components/DocView.qml:292-361` draws no hover fill).
