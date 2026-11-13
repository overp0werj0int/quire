# 05 Edit sheet and rewrite with Claude

Reference behaviour from omarky (OmarkyReader 0.5.0 sheet, Omarky 0.7.0 rewrite). Paths are relative to `/home/p0werj0int/Projects/omarky`. Items marked (inferred) were derived from reading the code's logic rather than stated in it.

Units. Sizes are logical pixels at the default type base (12 px) and spacing scale 1. Chrome type tokens are `caption` 10 px, `bodySmall` 11 px, `body` 12 px (`Commons/Style.qml:327-329`). Reading body text is `round(12 * 1.2 * zoom)`, 14 px at zoom 1 (`services/Reading.qml:47`), with line height 1.6 (`services/Reading.qml:48`). "Corner radius" is the desktop's window rounding, 0 by default, so every `max(6, radius)` below resolves to 6 (`Commons/Style.qml:31`). Spacing values scale with the type base (`Commons/Style.qml:208-223`).

Colour roles. `text` is the theme foreground. `dim`, `faint` and `ghost` are the foreground mixed toward the background by 0.34, 0.58 and 0.8 (`services/Theme.qml:44-48`). `hairline` is the foreground at alpha 0.14 light / 0.12 dark, `hairlineSoft` at 0.09 / 0.075, `hover` at 0.06, `raised` at 0.045 / 0.04 (`services/Theme.qml:51-63`). `flat(c, a)` means colour `c` mixed onto the page background at `a`, giving an opaque colour (`services/Theme.qml:75-77`). "Popup background" is the desktop's popup surface colour.

## Opening the sheet

SHEET-001. `E` (lowercase `e`, no modifiers) opens the sheet on one block. `Shift+E` (uppercase `E`) opens it on the block's whole section. `reader/ui/app/ReaderWindow.qml:447-448`

SHEET-002. The block chosen is, in order of preference: the block under the mouse pointer; else the block containing the current text selection; else the block at the top of the visible page. `reader/ui/app/ReaderWindow.qml:311-317`

SHEET-003. Nothing opens when the window has no file, the file has not loaded, or the file has been deleted from disk. `reader/ui/app/ReaderWindow.qml:346`

SHEET-004. In changes mode, when the newer side of the comparison is a past version rather than the file on disk, the sheet refuses with the toast "Compare with Now to edit the file". `reader/ui/app/ReaderWindow.qml:347`

SHEET-005. In changes mode, a fold row cannot be edited. The toast reads "Open the fold to edit what is in it". `reader/ui/app/ReaderWindow.qml:351`

SHEET-006. In changes mode, a removed block cannot be edited. The toast reads "That text is not in the file now". `reader/ui/app/ReaderWindow.qml:352`

SHEET-007. A block is edited as its exact source lines, from its first line up to (not including) its end line, taken from the document's source as it is when the sheet opens. Front matter, a horizontal rule and the footnotes block edit the same way. `reader/ui/app/EditSheet.qml:49-59`, `services/Doc.qml:88-91`, `docs/superpowers/specs/2026-10-26-block-editing-sheet.md:18`

SHEET-008. Block mode context: the nearest block before and the nearest block after are shown faint. Neighbour search skips fold rows and removed blocks, so the context is always text that is in the file. A missing neighbour (first or last block) shows no context box on that side. `reader/ui/app/ReaderWindow.qml:320-326,361`, `reader/ui/app/EditSheet.qml:52-53,187,261`

SHEET-009. Section mode range: from the nearest heading at or above the chosen block's first line through to the line before the next heading of the same or a higher level (a smaller or equal level number). With no such later heading, the range runs to the end line of the last block in the file. Removed headings and removed or folded blocks are ignored when finding headings and the last block. `reader/ui/app/ReaderWindow.qml:331-343`

SHEET-010. Section mode when the chosen block sits before the first heading: the range is line 0 up to the first heading's line. There is no "before" context, and the "after" context is the first heading. With no headings at all the range is the whole file. `reader/ui/app/ReaderWindow.qml:338`

SHEET-011. Section mode context: "before" is the heading immediately preceding the section's heading in the heading list, of any level. "After" is the heading that ends the section (same or higher level). Either is absent when there is none. `reader/ui/app/ReaderWindow.qml:341-342`

SHEET-012. When a block is opened in changes mode and it is reworded and the engine knows where its older wording was, the sheet also shows the older wording (see SHEET-040). Section mode never shows older wording. `reader/ui/app/ReaderWindow.qml:353-361`

SHEET-013. On open, the editor holds the block's source text, the caret is at position 0, the editor is scrolled to its top, and keyboard focus goes to the editor. Any earlier problem message and any armed discard state are cleared. `reader/ui/app/EditSheet.qml:55-63`

SHEET-014. While the sheet is shown, the window's own reading keys are inert. Every key goes to the sheet's editor. `reader/ui/app/ReaderWindow.qml:395`

## Layout

SHEET-015. The sheet is modal. A scrim covers the whole window in the page background colour at alpha 0.72. Clicks of any mouse button and wheel scrolling on the scrim do nothing, and clicking the scrim does not close the sheet. `reader/ui/app/EditSheet.qml:128-130`, `docs/superpowers/specs/2026-10-26-block-editing-sheet.md:22`

SHEET-016. The card is centred in the window. Its width is `min(window width - 48, reading column width + 2 * editor gutter + 2 * 22)`. The editor gutter is `round(editor font size * 3.2)`, 45 px at zoom 1 with a 14 px editor font. `reader/ui/app/EditSheet.qml:141-142`, `components/Editor.qml:24,28`

SHEET-017. The card has corner radius `max(6, corner radius)`, the popup background colour, and a 1 px `hairline` border. Inner padding is 22 px on all sides. `reader/ui/app/EditSheet.qml:132,144-147,152-154`

SHEET-018. Content stacks top to bottom with 10 px between items: the title row, the "before" context, the older-wording row (changes mode only), the editor, the "after" context, and the key row. `reader/ui/app/EditSheet.qml:150-311`

SHEET-019. The card's height is its chrome plus the editor's height. The editor's height is `max(90, min(editor text height, round(0.7 * window height) - chrome))`, so the card grows with its text up to about 70% of the window and the editor scrolls inside after that. Chrome is `2 * 22 + title height + 10 + key row height + 14`, plus each visible context box or older row and 10 px for each. `reader/ui/app/EditSheet.qml:133-137,143`. Note: the spec says "about two thirds"; the code uses 0.7. `docs/superpowers/specs/2026-10-26-block-editing-sheet.md:22`

SHEET-020. Title row, 8 px spacing: the file name in the UI family at `bodySmall`, bold, `text` colour; then the line span in `faint` at `bodySmall`, reading "line N" for a single line or "lines A to B" otherwise, where A is the 1-based first line and B is the 1-based last line. `reader/ui/app/EditSheet.qml:157-174`

SHEET-021. When the native syntax highlighter is unavailable the title row ends with "plain editor" in `ghost` at `caption`. The sheet still edits, unhighlighted. `reader/ui/app/EditSheet.qml:175-181`, `docs/decisions/0018-a-sheet-edits-one-block.md:22`

SHEET-022. Context blocks are drawn by the reader's own block renderer in the dim text tier, at the reading column width, indented by the editor's gutter so they align with the editor's text. `reader/ui/app/EditSheet.qml:191-200,265-273`

SHEET-023. Each context box is clipped to a maximum height of three reading lines, `round(body * 1.6 * 3)`, 67 px at zoom 1. `reader/ui/app/EditSheet.qml:133,189,263`

SHEET-024. The "before" box is bottom-aligned, so it shows the last lines of the preceding block. When the block is taller than the box, a 28 px gradient at the top fades from the popup background to transparent. `reader/ui/app/EditSheet.qml:196,202-209`

SHEET-025. The "after" box is top-aligned, so it shows the first lines of the following block. When the block is taller than the box, a 28 px gradient at the bottom fades from transparent to the popup background. `reader/ui/app/EditSheet.qml:265-283`

SHEET-026. Both context boxes are drawn at opacity 0.55 on top of the dim text tier. `reader/ui/app/EditSheet.qml:210,284`

SHEET-027. The editor in the sheet uses the editor's reading face (iA Writer Quattro S Regular) at `round(12 * 1.2 * zoom)`, the reader's current zoom, spelling check on, focus mode and typewriter scrolling off, and 6 px top and bottom padding instead of a page's padding. `reader/ui/app/EditSheet.qml:246-256`, `components/Editor.qml:19-24,40-41`

SHEET-028. The editor's text measure is `min(width - 2 * gutter, round(average char width * 66))`, about 66 characters. Heading marks, list markers and quote marks hang in the gutter. `components/Editor.qml:25-28`

SHEET-029. The editor caret is the theme accent, `max(2, round(font size / 9))` px wide, fully rounded. It blinks: visible 520 ms, fade out 180 ms, hidden 340 ms, fade in 180 ms. It stays solid while typing (any keystroke within the last 1400 ms). `components/Editor.qml:139,191-207`

SHEET-030. Key row, 24 px tall, 8 px spacing: a key cap "Esc" with "cancels", a 10 px gap, then a key cap "Ctrl+Enter" with "saves". Labels are `caption` in `faint`. `reader/ui/app/EditSheet.qml:287-300`

SHEET-031. A key cap is the key name in the UI family at `caption`, `dim`, on a `raised` fill with a 1 px `hairline` border, 12 px wider and 6 px taller than its label, corner radius `max(3, corner radius - 2)`. `components/KeyCap.qml:8-23`

SHEET-032. Right-clicking in the editor opens a menu: for a misspelt word with the highlighter present, up to the speller's suggestions (or "No suggestions"), then "Add "word" to dictionary", a separator, then Cut, Copy and Paste with their shortcuts. Cut and Copy are disabled with no selection. Closing the menu returns focus to the editor. `components/Editor.qml:218-241`, `reader/ui/app/EditSheet.qml:255,328`

## Keys inside the sheet

SHEET-033. `Ctrl+Enter` and `Ctrl+S` both save. `components/Editor.qml:357,377`, `reader/ui/app/EditSheet.qml:87`

SHEET-034. `Ctrl+J` opens the rewrite line on the block (or selection) and `Ctrl+Shift+J` opens it on the section, inside the sheet's text. `components/Editor.qml:373`, `reader/ui/app/EditSheet.qml:88-89`

SHEET-035. The editor's text-editing keys work as in the writer: Enter continues lists and quotes, Tab and Shift+Tab indent list items, Ctrl+B, Ctrl+I, Ctrl+E (inline code), Ctrl+Shift+X (strike) and Ctrl+K (link) wrap, Ctrl+0 to Ctrl+6 set heading levels. `components/Editor.qml:352-374`

SHEET-036. Every other window command the editor raises (find, preview, new, open, close, zoom, help, outline, font, focus, typewriter, spelling, save as, recent) is ignored in the sheet. `reader/ui/app/EditSheet.qml:86-96`, `docs/decisions/0018-a-sheet-edits-one-block.md:24`

SHEET-037. `Esc` resolves in this order: close the context menu if open; else dismiss the rewrite line if open (SHEET-079); else close the sheet at once if the text is unchanged from what it opened with; else arm a discard. `reader/ui/app/EditSheet.qml:90-95`

SHEET-038. Armed discard: the key row's "cancels" changes to "again to discard your edit" in the `text` colour. A second `Esc` within 2000 ms closes the sheet without writing. After 2000 ms the arm lapses and the row returns to "cancels". `reader/ui/app/EditSheet.qml:94,117,296`

SHEET-039. Closing without saving writes nothing, closes any open rewrite line, and returns keyboard focus to the page. `reader/ui/app/EditSheet.qml:66-70`, `reader/ui/app/ReaderWindow.qml:692`

## Older wording in changes mode

SHEET-040. For a reworded block in changes mode, a row above the editor shows the block's wording on the older side of the comparison: the older text's source lines from the paired block's older start line to its older end line. `reader/ui/app/ReaderWindow.qml:358-360`, `lib/diff/changes.js:110-113`

SHEET-041. The older row is a red wash, `flat(red, 0.06)` in light themes and `flat(red, 0.1)` in dark, corner radius `max(4, corner radius - 2)`. Its text sits 12 px from the left and 8 px from the top, the row is the text height plus 16 px, and the text is plain, wrapped, at most 4 lines with an ellipsis, in `dim`, in the reading prose face at `bodySmall`. `reader/ui/app/EditSheet.qml:214-234`

SHEET-042. A "Use this" chip sits at the row's right, 8 px in, vertically centred, at `bodySmall`. Activating it replaces the editor's whole text with the older wording and refocuses the editor. Saving then restores the old wording. `reader/ui/app/EditSheet.qml:81-84,235-243`

SHEET-043. A chip is its label in the reading mono face, `dim` (`text` on hover), height `round(size * 2)`, width label plus `round(size * 1.3)`, radius `max(3, min(corner radius, height / 3))`, with a `hover` fill and `hairline` border on hover, colour change animated 120 ms. `components/ChipButton.qml:16-38`

## Saving

SHEET-044. Before writing, the sheet compares the document's current source with the source captured when the sheet opened. If they differ (the file changed on disk while the sheet was open), nothing is written, the sheet stays open with the text intact, and the problem line reads "The file changed while you were editing. Copy your text and open it again." `reader/ui/app/EditSheet.qml:72-76`

SHEET-045. The write replaces the opened source lines with the editor's lines (split on newline) and writes the whole file atomically. All lines outside the range are untouched, so the file's final newline is kept. `reader/ui/app/EditSheet.qml:77-78,119-123`. Final newline preservation (inferred from the split and join on newline).

SHEET-046. A failed write keeps the sheet open and shows "Could not save: " followed by the error. `reader/ui/app/EditSheet.qml:125`

SHEET-047. The problem line replaces the key row: one line in `red` at `caption`, elided at the right. It stays until the sheet is next opened. `reader/ui/app/EditSheet.qml:292,301-310,55`

SHEET-048. After a successful write the sheet closes, a toast says "Saved", and focus returns to the page. The page reloads from disk through the reader's normal file watching and keeps its place. In changes mode the comparison re-runs, so the edit shows as a change at once. `reader/ui/app/EditSheet.qml:124`, `reader/ui/app/ReaderWindow.qml:691-692`, `docs/decisions/0018-a-sheet-edits-one-block.md:21`

SHEET-049. The toast is a pill centred at the bottom of the window, 28 px above the bottom edge, text at `bodySmall` in `text` on the popup background with a `hairline` border, 28 px wider and 14 px taller than its text. It fades in over 140 ms while rising 8 px over 160 ms, and hides after 1600 ms. `components/Toast.qml:10-40`

SHEET-050. The reader never holds the file open for writing. The sheet is the only path by which reading changes text, apart from ticking task boxes. `docs/decisions/0018-a-sheet-edits-one-block.md:7`, `reader/ui/app/ReaderWindow.qml:364-377`

## Rewrite: availability

SHEET-051. At startup the rewrite asks the launcher whether Claude Code is installed (`rewrite --available`, exit 0 when found). The answer is cached for the window's life. `components/Rewriter.qml:255-259`, `bin/omarky:51-53`

SHEET-052. Claude Code is looked for as: an override path from the environment if set; else the first executable `claude` on PATH; else `~/.local/bin/claude`, `~/.claude/local/claude`, `~/.local/share/mise/installs/claude/latest/claude`. `launcher/omarky/rewrite.rb:44-52`

SHEET-053. Without Claude Code, `Ctrl+J` and `Ctrl+Shift+J` show the toast "Rewrite needs Claude Code (claude on your PATH)" and open nothing. `components/Rewriter.qml:60`. Note: the spec says the key does nothing and the help sheet hides it; the code shows the toast and the help sheet always lists `Ctrl+J`. `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:71`, `reader/ui/app/ShortcutSheet.qml:24-25`

## Rewrite: choosing the range

SHEET-054. A range is a span of character offsets in the editor's text. A range never includes the newline that ends its last line, nor trailing whitespace, so replacing it keeps the blank lines between blocks. `lib/ranges.js:15-16,31-35`

SHEET-055. `Ctrl+J` with a selection: the selection, widened at both ends to whole words. A word character is a letter (Latin, Greek, Cyrillic), digit, apostrophe (straight or curly), underscore, `*`, backtick, `~`, `]` or `)`. `components/Rewriter.qml:68`, `lib/ranges.js:91-98`

SHEET-056. `Ctrl+J` with no selection: the top-level block holding the caret's line (paragraph, list, quote, table, code block and so on). On a blank line it is the next block; past the last block it is the last block; with no blocks it is the whole text. `components/Rewriter.qml:69`, `lib/ranges.js:51-66`

SHEET-057. `Ctrl+Shift+J`: the section holding the caret, from the heading at or above it to the line before the next heading of the same or a higher level. Before the first heading it is the text up to that heading. A selection is ignored. `components/Rewriter.qml:67`, `lib/ranges.js:68-89`

SHEET-058. `Ctrl+J` pressed again while the line is open and Claude is not working widens the range one step: selection to block, block to section, section to each enclosing shallower section in turn, then the whole text. A step that does not strictly contain the current range is skipped. Widening uses the caret position from when the line first opened. `components/Rewriter.qml:62-64,206`, `lib/ranges.js:103-116`

SHEET-059. `Ctrl+Shift+J` while the line is open and idle resets the range to the caret's section. `components/Rewriter.qml:63`

SHEET-060. Changing the range (opening, widening, or the section key) clears the conversation: earlier instructions, rewrites, notes, problem and status. `components/Rewriter.qml:77-94`

SHEET-061. While Claude is working, `Ctrl+J` only refocuses the instruction line. `components/Rewriter.qml:61-74`

SHEET-062. In the reader's sheet, "the whole text" and "the document" mean the sheet's text only, not the whole file. The job sent to Claude carries the sheet editor's text as the document. `components/Rewriter.qml:144`, `reader/ui/app/EditSheet.qml:318-326`. (inferred consequence: Claude sees only the opened block or section in the reader, while in the writer it sees the whole file.)

SHEET-063. The range is shaded in the editor: a `hover` fill extending 10 px left and right and 6 px above and below the range's lines, spanning the text width from the gutter, radius `max(6, corner radius)`, behind the text. There is no accent bar. `components/Editor.qml:81-89,163-172`, `components/Rewriter.qml:96-99`

SHEET-064. The range follows edits made elsewhere in the text while the line is open: the original range text is searched for at its old offset, then at the occurrence nearest the old offset. If it is found, the range and shade move with it. If it is not found, the range is "lost": the shade disappears and the status line reads "That text changed while the line was open" in red. `components/Rewriter.qml:116-126`, `lib/ranges.js:118-127`

## Rewrite: the card

SHEET-065. The instruction line and the rewrite share one card that floats over the sheet. Its width is `min(window width - 24, range width + 32)`. It is placed 16 px left of the range's left edge, clamped 12 px inside the window. `components/Rewriter.qml:267-277`

SHEET-066. Vertically the card sits 14 px below the range's last line when it fits above the window's bottom margin of 12 px; else 14 px above the range's first line when that stays 12 px clear of the top; else as low as it fits. `components/Rewriter.qml:278-285`

SHEET-067. The card has the popup background, a 1 px `hairline` border, corner radius `max(6, corner radius)`, 16 px padding, and 10 px between its rows. Clicking anywhere on it focuses the instruction line. `components/Rewriter.qml:267,286-298`

SHEET-068. The instruction row is at least 26 px tall. A prompt "Rewrite the selection", "Rewrite the block", "Rewrite the section" or "Rewrite the whole document" in `faint` at `caption` sits at the left, and the input field follows 12 px after it. `components/Rewriter.qml:55,301-311`

SHEET-069. The input is in the reading prose face, Regular, at `round(reading body * 0.92)` (13 px at zoom 1), `text` colour, accent selection. Its placeholder, in `ghost`, is "Say what to change" when idle, empty while Claude works, and "Enter to use it, or say what else to change" when a rewrite is ready. `components/Rewriter.qml:312-329`

SHEET-070. Below the instruction row, once there is rewrite text to show, a 1 px `hairlineSoft` rule, then the rewrite. `components/Rewriter.qml:335`

SHEET-071. The rewrite is shown as rendered Markdown blocks marked against the original range the way changes mode marks a document: inserted words on the green wash, removed words faint under a red strike (see CHG marks). A lone block rewritten as a lone block of the same type is always paired word by word, however different. `components/Rewriter.qml:263-264,345-366`, `lib/diff/changes.js:471-479`

SHEET-072. A block added or removed whole in the rewrite sits on a wash extending 8 px left and right and 4 px above and below, radius `max(4, corner radius - 2)`: added `flat(green, 0.1 light / 0.15 dark)`, removed `flat(red, 0.08 light / 0.12 dark)`. Blocks are spaced `round(reading body * 0.7)`. `components/Rewriter.qml:349-362`

SHEET-073. The rewrite area is at most `max(80, 0.45 * window height)` tall and scrolls inside when longer, stopping at its bounds. `components/Rewriter.qml:336-344`

SHEET-074. While text streams in, the marked rendering refreshes at most every 90 ms, and once more immediately when the rewrite completes. `components/Rewriter.qml:155,264-265`

SHEET-075. When the checker still reports problems after its retry, a wrapped line "Your checker still flags: " plus the notes joined by commas shows in `yellow` at `caption`, hidden while Claude works. `components/Rewriter.qml:369-377`

SHEET-076. Status row, 22 px: status text on the left at `caption`, elided, leaving 12 px before the key hints. The text is, in priority order: the problem message; "That text changed while the line was open"; the status word, with "…" appended while working; "Rewrite N of M" when the conversation holds more than one rewrite; else empty. Problem and lost states are `red`, the rest `faint`. `components/Rewriter.qml:380-391`

SHEET-077. While Claude works the status text pulses: opacity to 0.45 over 700 ms, back to 1 over 700 ms, repeating. It returns to full opacity when work stops. Durations collapse to 0 under reduced motion. `components/Rewriter.qml:392-398`, `services/Reading.qml:64-66`

SHEET-078. Key hints at the right, 6 px spacing, `caption` `faint`: "Enter use" when a rewrite is ready; "Ctrl+J wider" when idle with no ready rewrite; always "Esc" followed by "stop" while working, "drop" when a rewrite is ready, "close" otherwise. `components/Rewriter.qml:400-413`

## Rewrite: keys on the line

SHEET-079. `Esc` while Claude works stops it and sets the status to "Stopped", keeping the line open. `Esc` otherwise closes the line, drops any rewrite, clears the shade, and returns focus to the editor with its text unchanged. `components/Rewriter.qml:102-114,205`

SHEET-080. `Enter` with text in the input sends it as an instruction. `Enter` on an empty input takes the shown rewrite when one is ready (SHEET-088). `components/Rewriter.qml:201-204`

SHEET-081. `Alt+Up` and `Alt+Down` step to the earlier or later rewrite of this conversation, clamped at the ends. They do nothing while working or with fewer than two rewrites. `components/Rewriter.qml:191-196,207`

SHEET-082. `Up` and `Down` (no modifiers) on an empty input, or while already recalling, step through remembered instructions: `Up` to older, `Down` to newer, and `Down` past the newest empties the input. The caret goes to the end of the recalled text. `components/Rewriter.qml:208-228`

SHEET-083. Remembered instructions: each sent instruction moves to the front of the list, duplicates removed, at most 40 kept. The list persists across sessions in the app's state directory, and is not saved in an ephemeral session. `components/Rewriter.qml:215-236`, `reader/ui/app/ReaderWindow.qml:689`

## Rewrite: sending

SHEET-084. Sending trims the instruction and ignores it when empty or while working. When the range is lost, the problem reads "That text changed. Press Esc and select it again." and nothing is sent. `components/Rewriter.qml:130-133`

SHEET-085. A second instruction refines rather than restarts: the previous instruction and the rewrite currently shown (including one recalled with `Alt+Up`) are appended to the conversation's turns, and the new request carries every turn. `components/Rewriter.qml:134,144`

SHEET-086. On send the input clears, the shown rewrite and notes clear, the status becomes "Writing", and work begins. `components/Rewriter.qml:135-147`

SHEET-087. Messages from the agent, in order: text pieces append to the rewrite as they arrive; "checking" sets the status to "Checking against your writing guide"; "retry" clears the rewrite and sets the status to "Fixing: " plus the checker's notes joined by commas; "done" sets the final text, adds it to the conversation's rewrites and shows it, keeps the notes, and sets the status to "No change" when the text equals the original range, else clears it; "error" stops work and shows the message as the problem. `components/Rewriter.qml:154-171`

## Rewrite: taking it

SHEET-088. Taking a ready rewrite finds the original range text again (at its offset, else nearest occurrence). If found, it replaces that span with the rewrite as one undo step, puts the caret at the end of the inserted text, closes the line, clears the shade, and focuses the editor. `Ctrl+Z` then restores the original. `components/Rewriter.qml:175-189`, `components/Editor.qml:66-71`

SHEET-089. Without the native highlighter module, the replacement is two undo steps (remove, then insert). `components/Editor.qml:64-68`, `docs/decisions/0019-claude-behind-the-launcher.md:24`

SHEET-090. If the original range text is no longer in the editor, nothing is replaced, the rewrite is copied to the clipboard, and the problem reads "The text changed while Claude worked, so nothing was replaced. The rewrite is on the clipboard." `components/Rewriter.qml:177-182`

SHEET-091. In the sheet, taking a rewrite changes only the sheet's text. `Ctrl+Enter` still writes the file, and the stale-file refusal (SHEET-044) still applies. `reader/ui/app/EditSheet.qml:315-317`, `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:51`

SHEET-092. The user can keep typing elsewhere in the text while Claude works. `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:49`, `components/Rewriter.qml:116-126`

## Rewrite: the agent

SHEET-093. The window never runs Claude itself. It writes a job file to the user runtime directory (or `/tmp`) named `omarky-rewrite-<epoch ms>-<random 0..999999>.json`, then runs the launcher's `rewrite --job FILE`. The launcher deletes the job file after reading it. `components/Rewriter.qml:144-146,238-243`, `bin/omarky:54-61`, `docs/decisions/0019-claude-behind-the-launcher.md:7`

SHEET-094. The job holds the file path, the full text being edited ("document"), the range's start and end offsets, the instruction, and the earlier turns as instruction and rewrite pairs. `components/Rewriter.qml:144`, `launcher/omarky/rewrite.rb:18-19`

SHEET-095. A job file that cannot be written stops work with the problem "Could not start the rewrite: " plus the error. `components/Rewriter.qml:242`

SHEET-096. Claude Code runs headless with: model `claude-opus-5-5` (overridable by an environment variable), effort low, no tools, no setting sources, strict empty MCP config, no session persistence, a fixed system prompt, streaming JSON output with partial messages. Its working directory is the system temp directory, and four nested-session environment variables are removed. `launcher/omarky/rewrite.rb:27-29,104-109,138,216-218`

SHEET-097. The system prompt tells Claude to rewrite only the selected part following the instruction; reply with the replacement Markdown and nothing else (no preamble, explanation, quotes or code fence); keep the selection's Markdown structure, the document's voice, tense, spelling conventions and facts; add no unsupported claim; change nothing not asked for; and reply with the selection unchanged when the instruction cannot be applied. `launcher/omarky/rewrite.rb:31-37`

SHEET-098. Style guide: if `writing.md` exists in the config directory (`$XDG_CONFIG_HOME/omarky`, default `~/.config/omarky`) it alone is used; else every `writing/*.md` file in name order. Contents are trimmed, empty files dropped, joined with blank lines, and appended to the system prompt after "The writer's style guide follows. Apply it to everything you write." `launcher/omarky/rewrite.rb:39-41,67-81`

SHEET-099. The user prompt is: the document wrapped in a `document` tag carrying the file's base name, with the range wrapped in a `selection` tag in place; then, when there are earlier turns, "Earlier rewrites of the selection, oldest first:" with each turn as an attempt holding its instruction and rewrite, and "Start from the last rewrite and apply the new instruction to it."; then the instruction in an `instruction` tag; then "Reply with only the new text for the selection." `launcher/omarky/rewrite.rb:85-102`

SHEET-100. Checker: if an executable `check` exists in the config directory and the reply is not blank, the launcher reports "checking", writes the reply to a temporary `.md` file (newline-terminated) and runs the checker on it. Exit 0 means it passes. `launcher/omarky/rewrite.rb:73-76,116-118,190-193`

SHEET-101. A failing checker's output is reduced to notes: non-blank trimmed lines, preferring only lines containing `[fix]` when any do, with a trailing ` | L<n>: ...` and a leading `[fix] ` removed, de-duplicated, at most 12. `launcher/omarky/rewrite.rb:195-199`

SHEET-102. On notes, the launcher reports "retry" with them and asks Claude once more, appending the first reply and the notes and asking for corrected text. The checker then runs on the second reply and whatever it still reports goes out with "done" as the notes. There is only one retry. `launcher/omarky/rewrite.rb:119-126`

SHEET-103. The reply is cleaned: a reply wrapped whole in a code fence is unwrapped unless the selection itself starts with a fence; the result is trimmed and given the selection's own leading and trailing whitespace. The final message is used, or the streamed text when the final message is empty. `launcher/omarky/rewrite.rb:172,177-187`

SHEET-104. Error messages shown in the card: "Claude Code is not installed (no claude on PATH)."; Claude's own error result text, or "Claude stopped with an error." when it is empty; the last line of Claude's stderr, or "Claude ended without a reply (exit N)." when nothing came back. `launcher/omarky/rewrite.rb:112,153-154,168-170`

SHEET-105. If the agent process exits while work is still marked in progress without having sent "done" or "error", the problem reads "The rewrite stopped (exit N)." unless the user had stopped it. `components/Rewriter.qml:250-252`

SHEET-106. Stopping sends the agent a terminate signal. The launcher terminates its Claude child and exits with code 143. `components/Rewriter.qml:149-152`, `launcher/omarky/rewrite.rb:113,202-206`

SHEET-107. There are no timeouts on a rewrite. It runs until Claude finishes, errors, or the user presses `Esc`. (inferred from the absence of any timer or deadline in `components/Rewriter.qml` and `launcher/omarky/rewrite.rb`)

SHEET-108. The launcher records the call's cost in USD (rounded to 4 places) in its "done" message. The window does not display it. `launcher/omarky/rewrite.rb:127,152`, `components/Rewriter.qml:158-165`

SHEET-109. An empty reply is not called out: when the cleaned reply is empty, the card shows nothing to take and `Enter` does nothing. (inferred from `ready` requiring non-empty text at `components/Rewriter.qml:54` and the status rule at `components/Rewriter.qml:164`.) The spec intends "No change" for empty replies too. `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:67`

SHEET-110. Privacy: each rewrite sends the job's document text to Anthropic through Claude Code on the user's subscription. There is no API key. `docs/decisions/0019-claude-behind-the-launcher.md:16-20`, `README.md:101`

## Help sheet entries

SHEET-111. The help sheet's "Edit" group lists: "E" Edit the block under the mouse; "Shift+E" Edit its section; "Ctrl+Enter  Esc" Save to the file / cancel; "Ctrl+J" In the sheet, rewrite with Claude. `reader/ui/app/ShortcutSheet.qml:24-25`

## Spec and code divergences worth deciding

- Spec says the rewrite line sits in room left under the sheet's editor; the code floats a card over the sheet anchored to the range (SHEET-065, SHEET-066). `docs/superpowers/specs/2026-10-26-block-editing-sheet.md:56`
- Spec says "Esc again to discard" for two seconds; code shows "again to discard your edit" (SHEET-038).
- Spec says Claude sees the whole document; in the reader's sheet it sees only the sheet's text (SHEET-062).
