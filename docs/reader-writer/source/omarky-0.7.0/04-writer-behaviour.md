# OmarkyWriter: writer behaviour spec (extracted from omarky)

Source repo: `/home/p0werj0int/Projects/omarky` (version 0.7.0). All paths below are relative to it. Line citations are `path:line`.

`writer/ui/Commons`, `components`, `lib`, `services`, `Ui` and `vendor` are symlinks to the repo's shared directories (`writer/ui/` listing), so `components/Editor.qml` is both the writer's editor and the reader's edit-sheet editor. Writer-only code is `writer/ui/app/WriterWindow.qml`, `writer/ui/app/HelpSheet.qml`, `writer/ui/app/Settings.qml`, `writer/ui/shell.qml` and `writer/launcher/`.

## Units and conventions

- `S(n)` means a chrome length of `n` logical pixels scaled by the shell's spacing scale: `max(1, round(n * spacingScale * base/12))`, where `base` is the theme font base size, default 12 (`Commons/Style.qml:213-223`, `Commons/Style.qml:279-284`). At the default base, `S(n) = n`.
- Chrome text sizes at default base 12: `caption` 10, `bodySmall` 11, `body` 12, `heading` 16 (`Commons/Style.qml:327-332`). Chrome text uses the shell UI family (default the `monospace` alias) (`Commons/Style.qml:269`, `Commons/Style.qml:322`).
- `E` (the editor text size) = `round(body * 1.2 * zoom)`, so 14 px at base 12 and zoom 1.0 (`components/Editor.qml:24`).
- Durations go through a reduced-motion gate: every animated duration becomes 0 when `OMARKY_REDUCED_MOTION=1` or Hyprland animations are off (`services/Reading.qml:64-66`). Timers (autosave, typing pause and so on) are not gated.
- Colour roles are defined in WR-170 onward and referred to by name (text, strong, dim, faint, ghost, editor-ghost, accent, link, mark, code, codeBg, error, hover, hairline, hairlineSoft, selection, raised, popup background).
- Every `(inferred)` item was derived from reading code paths, not from a statement or a test in omarky.

---

## 1. The live formatting model

### Principles

- **WR-01** The text in the editor is always the file's Markdown, character for character. Formatting is paint only: nothing in the display changes a character. Only the editing keys in section 2 (and accepting a rewrite) change text beyond what the user types (`README.md:75`, `components/Editor.qml:6-10`, `native/markdownhighlighter.h:18-25`).
- **WR-02** Saving writes exactly the editor text, with no normalisation (`components/Buffer.qml:23-28`, `writer/ui/app/WriterWindow.qml:81-85`).
- **WR-03** Line layout (line height, hanging indents) is presentation and is never an undo step. Undo and redo only ever move text. Typing `- ` (which turns a line into a hanging list item), then undo, must remove the text in one step and never get stuck re-applying the indent (`docs/decisions/0015-layout-outside-undo.md:7-17`, `native/markdownhighlighter.cpp:15-24`, `native/markdownhighlighter.cpp:341-382`). omarky mechanism: block formats are set in a queued pass after painting with the document's undo recording switched off.
- **WR-04** Formatting is computed per line, live on every keystroke, and carries multi-line state (fenced code, front matter, HTML comments) from line to line (`native/markdownhighlighter.cpp:26-27`, `native/markdownhighlighter.cpp:384-390`).
- **WR-05** Every line starts from a base look: prose family, size `E`, normal weight, upright, colour text (`native/markdownhighlighter.cpp:392-397`).
- **WR-06** Bold and italic faces are selected by the font's style name (Bold, Semibold; Italic, Oblique; Bold Italic), never by numeric weight, because iA Writer Quattro S gives every face the same weight value (`native/markdownhighlighter.cpp:65-92`, `native/markdownhighlighter.cpp:766-772`).
- **WR-07** Line height is 155 percent of the font's natural line height for every line type: paragraphs, headings, list items, quotes, code and blank lines alike. There is no extra paragraph spacing; blank lines in the source are the only vertical gaps (`components/Editor.qml:263`, `native/markdownhighlighter.cpp:324-327`). The highlighter's own default when not told is 160 percent (`native/markdownhighlighter.cpp:144`).

### Line-level rules (first match wins, in this order)

- **WR-08** Line kind precedence, evaluated top to bottom: inside a fence; inside front matter; inside an HTML comment; front-matter opener; fence opener; HTML comment opener; ATX heading; thematic break; setext underline; block quote; list item; table separator row; reference definition; footnote definition; plain (`native/markdownhighlighter.cpp:438-576`). Consequence: a list inside a quote (`> - item`) is styled as a quote and keeps the quote's hang (`native/markdownhighlighter.cpp:537`).
- **WR-09** Fence opener: up to 3 spaces, then 3 or more backticks or tildes, then anything. A backtick fence whose info string contains a backtick is not a fence. The whole line is mono at `round(E * 0.88)` and faint (`native/markdownhighlighter.cpp:44`, `native/markdownhighlighter.cpp:479-486`, `native/markdownhighlighter.cpp:421-425`).
- **WR-10** Inside a fence every line is mono at `round(E * 0.88)` in the code colour, with no background and no inline parsing or spelling. The fence closes on a line of the same character, at least as long as the opener, with only whitespace after; the closing line is faint (`native/markdownhighlighter.cpp:439-453`).
- **WR-11** Code lines (fenced) do not hang: they start at the text column (`native/markdownhighlighter.cpp:430-435`, `native/markdownhighlighter.cpp:330-331`). Indented (4-space) code blocks have no rule and are painted as plain text (inferred, no pattern exists).
- **WR-12** Front matter: only when line 1 of the document is exactly `---`. The opener is mono and faint. Each following line is mono and dim, with the key up to and including the first colon in the mark colour. A line of exactly `---` or `...` closes it, faint (`native/markdownhighlighter.cpp:454-478`).
- **WR-13** HTML comments: a line whose trimmed text starts with `<!--` is faint with no inline parsing, and the faint state continues to following lines until a line containing `-->` (`native/markdownhighlighter.cpp:467-472`, `native/markdownhighlighter.cpp:487-492`).
- **WR-14** ATX heading: up to 3 spaces, 1 to 6 `#`, then a space, tab or end of line. The whole line is bold in the strong colour at a size of `round(E * scale)` with scale H1 1.5, H2 1.28, H3 1.12, H4 1.0, H5 1.0, H6 1.0 (`native/markdownhighlighter.cpp:40`, `native/markdownhighlighter.cpp:493-501`).
- **WR-15** The heading mark (leading spaces, the `#` run and the spaces after it) is faint and normal weight at the heading's size, and is not spell checked. A closing run of `#` (whitespace then `#`s at line end) is styled the same (`native/markdownhighlighter.cpp:502-513`).
- **WR-16** The heading mark hangs: its width (measured at the heading's size) sits left of the text column, so the heading text starts on the column edge (`native/markdownhighlighter.cpp:514-516`, `docs/screenshots/writer/editor.png`).
- **WR-17** Thematic break (`---`, `***`, `___`, three or more, spaces allowed): the line is faint (`native/markdownhighlighter.cpp:41`, `native/markdownhighlighter.cpp:517-520`).
- **WR-18** Setext underline (`===` or `---` runs) under a non-blank line is faint. The text line above it is not styled as a heading (`native/markdownhighlighter.cpp:48`, `native/markdownhighlighter.cpp:521-525`). Because the thematic-break rule runs first, `---` under text is caught as a break, with the same faint result (inferred).
- **WR-19** Block quote: one or more `>` (each with up to 3 leading spaces and one optional following space). The marks are in the mark colour; the rest of the line is dim and italic. The marks hang, measured at body size `E` (`native/markdownhighlighter.cpp:43`, `native/markdownhighlighter.cpp:526-536`).
- **WR-20** List item: leading whitespace, a marker (`-`, `*`, `+`, or 1 to 9 digits followed by `.` or `)`), one or more spaces or tabs, and an optional task box `[ ]`, `[x]` or `[X]`. The marker and the spaces after it are in the mark colour (`native/markdownhighlighter.cpp:42`, `native/markdownhighlighter.cpp:538-543`).
- **WR-21** Task box: in the mark colour and in the mono family. A done box (`x` or `X`) makes the rest of the line faint with a strikethrough (`native/markdownhighlighter.cpp:544-555`, `docs/screenshots/writer/editor.png`).
- **WR-22** List hang: the item's leading whitespace stays inside the text (it indents the item by its own width); only the marker and its trailing spaces hang into the margin. The task box belongs to the item's text and does not hang. Wrapped lines of an item align with the item's text, not with the marker (`native/markdownhighlighter.cpp:556-562`, `native/markdownhighlighter.cpp:328-332`).
- **WR-23** Table separator rows (`|---|:--:|` shapes) are faint (`native/markdownhighlighter.cpp:45`, `native/markdownhighlighter.cpp:563-566`).
- **WR-24** Reference definitions (`[id]: url ...`, not footnotes) are faint (`native/markdownhighlighter.cpp:47`, `native/markdownhighlighter.cpp:567-570`).
- **WR-25** Footnote definitions (`[^x]:`): the label and colon are in the mark colour and hang by their width plus one space; the rest is parsed inline (`native/markdownhighlighter.cpp:46`, `native/markdownhighlighter.cpp:571-576`).
- **WR-26** Hang geometry: every line's left margin is `gutter + leading indent` and its first line is pulled left by the hang, capped at that left margin, so a mark wider than the gutter is clipped at the gutter's outer edge rather than overflowing. `gutter = round(E * 3.2)` (`native/markdownhighlighter.cpp:324-333`, `components/Editor.qml:27-28`).

### Inline rules (applied after the line rule, from where the line's mark ends)

Inline spans claim characters in this order. A character claimed as literal by an earlier rule cannot start a later rule's span (`native/markdownhighlighter.cpp:400-419`, `native/markdownhighlighter.cpp:578-708`).

- **WR-27** Escapes (`\` before one of ``\ ` * _ { } [ ] ( ) # + - . ! | ~ > =``): the backslash is faint and the escaped character is literal (`native/markdownhighlighter.cpp:62`, `native/markdownhighlighter.cpp:581-587`).
- **WR-28** Code span (the code chip): a run of N backticks, content, and a matching run of N backticks. The whole span, backticks included, is mono at `round(E * 0.88)` in the code colour on the codeBg background. The backticks are faint (still on the chip). Nothing inside is parsed or spell checked (`native/markdownhighlighter.cpp:50`, `native/markdownhighlighter.cpp:588-600`, `docs/screenshots/writer/editor.png`). The chip is a background behind the glyphs with no padding or rounded corners (inferred from a character background).
- **WR-29** Inline HTML tags (`<tag ...>`, `</tag>`, `<tag/>`) are faint (`native/markdownhighlighter.cpp:61`, `native/markdownhighlighter.cpp:601-609`).
- **WR-30** Inline link `[text](url "title")`: the text is in the link colour. `[` is faint. `](url "title")` is faint and at `round(E * 0.86)`. Link text stays spell checked; the URL part does not. No underline (`native/markdownhighlighter.cpp:51`, `native/markdownhighlighter.cpp:610-630`).
- **WR-31** Image `![alt](src)`: the alt text is dim and italic; `![` faint; `](src)` faint at `round(E * 0.86)` (`native/markdownhighlighter.cpp:616-629`).
- **WR-32** Reference link `[text][id]`: text in the link colour, the brackets and label faint (`native/markdownhighlighter.cpp:52`, `native/markdownhighlighter.cpp:631-640`).
- **WR-33** Autolink `<https://...>` or `<mailto:...>`: link colour, with the angle brackets faint (`native/markdownhighlighter.cpp:53`, `native/markdownhighlighter.cpp:641-650`).
- **WR-34** Bare URL (`http://`, `https://` or `www.` not preceded by `(`, `<`, a word character or `/`): link colour; trailing `.,;:!?'"` and brackets are excluded from the link (`native/markdownhighlighter.cpp:54`, `native/markdownhighlighter.cpp:651-658`).
- **WR-35** Footnote reference `[^x]`: mark colour (`native/markdownhighlighter.cpp:60`, `native/markdownhighlighter.cpp:659-666`).
- **WR-36** Emphasis, applied in this order so a longer mark is never reused by a shorter pattern: bold (`**x**` or `__x__`, no space just inside), italic star (`*x*`, not touching another `*` or a backslash), italic underscore (`_x_`, only at word boundaries, never intraword), strikethrough (`~~x~~`), highlight (`==x==`). In each, both marks are faint and literal (`native/markdownhighlighter.cpp:55-59`, `native/markdownhighlighter.cpp:668-700`).
- **WR-37** Emphasis looks: bold is the bold face in the strong colour; italic is the italic face, colour unchanged ("emphasis leans"); strikethrough is struck and dim; highlight is a background of the accent at alpha 0.22 (`native/markdownhighlighter.cpp:684-695`).
- **WR-38** Table rows (trimmed line starts with `|`): every unescaped, non-literal `|` is faint (`native/markdownhighlighter.cpp:702-707`).

---

## 2. Editing keys

The editor has keyboard focus nearly always and handles every key itself; window commands are raised by the editor rather than listened for by the window (`components/Editor.qml:47-50`, `components/Editor.qml:344-384`).

### Enter

- **WR-39** Plain Enter (no Ctrl, Alt or Shift, no selection) on a list item, with the cursor at or after the end of the item's prefix (indent, marker, spaces, task box): inserts a newline plus the same indent, marker and spacing. The cursor lands after the new prefix (`lib/editing.js:7`, `lib/editing.js:27-40`).
- **WR-40** Numbered continuation counts up: `3.` continues as `4.`, `9)` as `10)`, keeping the delimiter. Example: `1. a` then Enter gives `1. a\n2. ` (`lib/editing.js:35-36`, `writer/tools/check:45-46`).
- **WR-41** Task continuation: an item with a task box (`[ ]`, `[x]` or `[X]`) continues with an empty box `[ ] `, whatever the original's state (`lib/editing.js:37-38`).
- **WR-42** Empty-item exit: Enter on an item whose text after the prefix is empty or whitespace, with the cursor at the very end of the line, deletes the whole line's content (marker included) and leaves the cursor at the start of that now-empty line. No newline is added. Example: typing `- one`, Enter, `two`, Enter, Enter, `x` gives `- one\n- two\nx` (`lib/editing.js:31-34`, `writer/tools/check:43-44`).
- **WR-43** Quote continuation: Enter on a quote line, cursor after the quote marks, inserts a newline plus the same `>` prefix (nesting preserved, for example `> > `). If the text after the marks is empty or whitespace, the line's content is removed instead and the cursor stays at the line start (`lib/editing.js:8`, `lib/editing.js:41-47`).
- **WR-44** Enter with the cursor inside a list or quote prefix (before the end of the marker), or on any other line, is a plain newline (`lib/editing.js:31`, `lib/editing.js:42`, `lib/editing.js:48`, `components/Editor.qml:352-355`).
- **WR-45** Enter with a selection, or with Shift or Alt held, gets no list handling and behaves as a plain text-field Enter (replaces the selection with a newline; Shift+Enter inserts a newline) (`components/Editor.qml:352`) (inferred for the default behaviour).
- **WR-46** Ctrl+Enter raises `submit`, which the writer window does not handle, so it does nothing in the writer (`components/Editor.qml:356-357`, `writer/ui/app/WriterWindow.qml:145-185`).

### Tab and Shift+Tab

- **WR-47** Tab (no Ctrl, no selection) on a list item inserts spaces at the start of the line: 2 for a bullet (`-`, `*`, `+`), and for a numbered item the width of its marker plus its following spaces (3 for `2. `, 4 for `10. `). The cursor moves right by the same amount. Example: `2. ` becomes `   2. ` (`lib/editing.js:53-66`, `writer/tools/check:47-48`).
- **WR-48** Tab on a line that is not a list item inserts 4 spaces at the cursor (`components/Editor.qml:358-362`).
- **WR-49** Tab with a selection is not handled by the editor and falls to the plain text-field Tab (inferred: replaces the selection with a tab character) (`components/Editor.qml:358`). Tab stops are 4 average character widths (`components/Editor.qml:190`).
- **WR-50** Shift+Tab (no Ctrl) on a list item with leading whitespace removes up to the same step (2 for bullets, marker-plus-spaces width for numbers) from the start of the line, never more than the whitespace present. The cursor moves left by the removed amount, never before the line start. On a list item with no indent, or a non-list line, Shift+Tab does nothing (and is consumed) (`lib/editing.js:59-64`, `components/Editor.qml:363-367`).

### Inline formatting toggles (Ctrl+B, Ctrl+I, Ctrl+E, Ctrl+Shift+X)

- **WR-51** Marks: Ctrl+B `**`, Ctrl+I `*`, Ctrl+E `` ` ``, Ctrl+Shift+X `~~` (`components/Editor.qml:369-371`).
- **WR-52** With no selection: inserts the mark twice at the cursor and places the cursor between them (`**|**`). Example: `## word` then Ctrl+B at the end gives `## word****` (`lib/editing.js:69-82`, `writer/tools/check:52-53`).
- **WR-53** Toggle-off rule 1: a non-empty selection whose text is immediately surrounded by the mark on both sides (outside the selection) has those outer marks removed; the selection then covers the same inner text (`lib/editing.js:75-77`).
- **WR-54** Toggle-off rule 2: a selection that itself starts and ends with the mark, and is longer than two marks, has those marks stripped; the selection covers the bare text (`lib/editing.js:78-81`).
- **WR-55** Otherwise the selection is wrapped in the mark and stays selected on the inner text (shifted by the mark length) (`lib/editing.js:82`).
- **WR-56** Edge results of these rules: pressing Ctrl+B twice with no selection gives eight asterisks, not a toggle-off (rule 1 needs a selection). Ctrl+I on a selected `**word**` strips one `*` from each end (rule 2), leaving `*word*`; Ctrl+I on `word` selected inside `**word**` does the same via rule 1 (inferred from `lib/editing.js:72-83`).

### Links and headings

- **WR-57** Ctrl+K with a selection (or none) that is not a URL: replaces it with `[selection]()` and places the cursor inside the parentheses, ready for the URL (`lib/editing.js:85-95`).
- **WR-58** Ctrl+K with a selection that is a URL (`http://`, `https://` or `mailto:` followed by non-space text, nothing else): replaces it with `[](url)` and places the cursor inside the empty brackets, ready for the link text (`lib/editing.js:88-92`). There is no toggle-off for links.
- **WR-59** Ctrl+1 to Ctrl+6 (no Shift) set the cursor line's heading level: any existing prefix (up to 3 spaces, 1 to 6 `#`, any spaces or tabs) is replaced by N `#` and one space. The cursor keeps its offset within the heading text. Example: `word` then Ctrl+2 gives `## word` (`lib/editing.js:98-109`, `writer/tools/check:50-51`).
- **WR-60** Pressing the line's current level again clears the heading (the prefix is removed); Ctrl+0 always clears it (`lib/editing.js:100-106`).
- **WR-61** The heading key's prefix match does not require a space after `#`, so a line starting `#tag` counts as level 1 (Ctrl+1 strips it to `tag`, Ctrl+2 makes `## tag`). On a list line, the `#` prefix is put in front of the marker (`## - item`) (inferred from `lib/editing.js:102`).
- **WR-62** Ctrl+0 in the writer is heading-clear, not zoom reset; the writer has no zoom-reset key (`components/Editor.qml:374`, `writer/ui/app/HelpSheet.qml:19`).

### Undo granularity

- **WR-63** Key-driven edits are applied as "remove range, then insert text" (`components/Editor.qml:54-62`). Whether that is one undo step is not guaranteed in omarky (inferred: a wrap with a selection can be two undo steps). Only accepting a rewrite uses a guaranteed single-step replace (`components/Editor.qml:64-71`, `native/markdownhighlighter.cpp:797-809`). Quire should decide this explicitly; one step per key is the obvious intent (inferred).

### Window command keys

All raised by the editor (`components/Editor.qml:368-383`). Without Shift: S save, N new, O open, W close, P preview, F find, R recent, `=` or `+` zoom in, `-` zoom out, `/` or `?` help, G find next, `\` outline. With Shift: F focus, T typewriter, L spelling, S save as, M font, P preview.

- **WR-64** Ctrl+S: a named file is saved immediately and the toast "Saved" shows. A draft opens the Save As dialog instead (`writer/ui/app/WriterWindow.qml:81-85`).
- **WR-65** Ctrl+Shift+S: Save As dialog (section 9) (`writer/ui/app/WriterWindow.qml:148`).
- **WR-66** Ctrl+N: opens a new window on a new untitled draft (`writer/ui/app/WriterWindow.qml:149`, `writer/ui/shell.qml:110`).
- **WR-67** Ctrl+O: open-file dialog (section 9) (`writer/ui/app/WriterWindow.qml:150`).
- **WR-68** Ctrl+W: saves (or deletes an emptied draft) and closes the window; closing the last window quits the app (`writer/ui/app/WriterWindow.qml:69-73`, `writer/ui/shell.qml:54-59`, `writer/ui/shell.qml:112`).
- **WR-69** Ctrl+P and Ctrl+Shift+P toggle the preview (section 6) (`components/Editor.qml:376-377`).
- **WR-70** Ctrl+F opens find; Ctrl+G and F3 go to the next match; Shift+F3 to the previous (section 7) (`components/Editor.qml:378-383`).
- **WR-71** Ctrl+R (or a click on the file name in the header) opens the recent menu (section 9) (`writer/ui/app/WriterWindow.qml:176`, `writer/ui/app/WriterWindow.qml:450`).
- **WR-72** Ctrl+= and Ctrl++ zoom in by 0.1; Ctrl+- zooms out by 0.1 (section 8). The shifted variants (Ctrl+Shift+= producing `+`, Ctrl+Shift+/ producing `?`) are looked up in the Shift map, which has neither, so on a US layout only unshifted Ctrl+= and Ctrl+/ work (inferred from `components/Editor.qml:375-380`).
- **WR-73** Ctrl+/ toggles the help sheet (section 10) (`writer/ui/app/WriterWindow.qml:173`).
- **WR-74** Ctrl+\ toggles the outline (section 5) (`writer/ui/app/WriterWindow.qml:153-158`).
- **WR-75** Ctrl+Shift+F focus, Ctrl+Shift+T typewriter, Ctrl+Shift+L spelling, Ctrl+Shift+M next font (sections 3, 4, 8) (`writer/ui/app/WriterWindow.qml:159-162`).
- **WR-76** Ctrl+J rewrites the block or selection; Ctrl+Shift+J the section (section 11) (`components/Editor.qml:373`).
- **WR-77** Esc closes one thing, in this priority: the popup menu, then the rewrite line, then the help sheet, then the find bar (focus back to the editor), then the preview. With none open it does nothing (`writer/ui/app/WriterWindow.qml:177-183`).
- **WR-78** Ctrl+Alt combinations are not handled as commands (`components/Editor.qml:368`).

---

## 3. Focus mode

- **WR-79** Focus mode (Ctrl+Shift+F or the target chip) is a persisted on/off setting, default off (`writer/ui/app/Settings.qml:22`, `writer/ui/app/WriterWindow.qml:159`).
- **WR-80** Toggling shows the toast "Focus on" or "Focus off" (`writer/ui/app/WriterWindow.qml:159`).
- **WR-81** The current paragraph is the run of consecutive non-blank lines containing the cursor's line. When the cursor is on a blank line, the current paragraph is that one blank line, so everything else fades (`native/markdownhighlighter.cpp:216-238`). A heading or list directly against a paragraph with no blank line between counts as the same paragraph (inferred).
- **WR-82** Every line outside the current paragraph is repainted in the editor-ghost colour (foreground mixed 72 percent toward the background). The fade is a colour substitution, not opacity: backgrounds (code chips, highlights) are cleared, spelling underlines are removed; sizes, weights, italics and strikethrough remain (`native/markdownhighlighter.cpp:756-764`, `components/Editor.qml:253`, `docs/screenshots/writer/focus.png`).
- **WR-83** The current paragraph keeps full formatting. When the cursor moves into another paragraph the old and new paragraph lines are repainted immediately, with no transition (`native/markdownhighlighter.cpp:191-214`).
- **WR-84** Focus mode does not dim the chrome, the header or the outline (inferred: only the editor paint changes).

## 4. Typewriter scrolling and scrolling in general

- **WR-85** Typewriter mode (Ctrl+Shift+T or the I-beam chip) is a persisted setting, default off; toggling shows "Typewriter on" or "Typewriter off" and then re-centres the cursor line with a 220 ms glide (`writer/ui/app/Settings.qml:23`, `writer/ui/app/WriterWindow.qml:160`).
- **WR-86** In typewriter mode the top padding is half the editor's height, so the first line can sit at the middle; the bottom padding is always half the editor's height (`components/Editor.qml:38-41`).
- **WR-87** Outside typewriter mode the top padding is `max(S(56), 12 percent of the editor height)` (`components/Editor.qml:40`).
- **WR-88** In typewriter mode every cursor move scrolls so the centre of the cursor line is at the vertical centre of the editor: a 120 ms glide for ordinary moves, 220 ms for forced moves (open at end, find, outline jump, mode toggle) (`components/Editor.qml:99-106`).
- **WR-89** Outside typewriter mode the cursor is kept in view with a margin of `min(editor height / 4, 3 cursor-line heights)` above and below. Ordinary moves (typing, arrows) jump instantly; forced moves glide 220 ms (`components/Editor.qml:107-110`, `components/Editor.qml:212`).
- **WR-90** All glides ease out cubic and are clamped to the scrollable range; under reduced motion they jump (`components/Editor.qml:121-130`, `components/Editor.qml:149`).
- **WR-91** Mouse wheel (notched): each event glides by the wheel delta in pixels (120 px per notch) over 160 ms, accumulating from the current glide target (`components/Editor.qml:132`, `components/Editor.qml:295-297`).
- **WR-92** Touchpad: content follows the finger at 12 times the reported pixel delta; on gesture end, if the last sample is no older than 200 ms, a fling starts from the average velocity of the last 3 samples (`components/Editor.qml:286-307`). The fling ignores velocities under 50 px/s, caps at 20946 px/s, and decays each frame as `speed = (|v| + 172) * exp(-3.7 * dt) - 172`, stopping at 0 or an edge; no fling under reduced motion (`components/Editor.qml:310-326`). This is the reader's Chromium-matched scrolling (`docs/decisions/0011-touchpad-scrolling.md`).
- **WR-93** Scroll thumb: a pill at the right edge, inset `S(4)`, `S(3)` wide, faint, shown only when content is taller than 1.05 times the view. Height `max(S(24), view * view / content)`. Opacity 0.5 while a glide or fling runs or within 700 ms of any scroll movement, 0 otherwise, fading over 300 ms (`components/Editor.qml:328-342`).
- **WR-94** A press anywhere in the editor area, including the margins beside the column, places the cursor at the nearest text position (clamped into the text) and focuses the editor. The pointer is an I-beam everywhere over the editor (`components/Editor.qml:151-161`).

## 5. The outline in the writer

- **WR-95** The outline is off by default and persisted; Ctrl+\ or its header chip toggles it (`writer/ui/app/Settings.qml:25-26`, `writer/ui/app/WriterWindow.qml:153-158`, `docs/writer/CHANGELOG.md:5`).
- **WR-96** The outline shows only when all hold: the setting is on, the window is at least `S(960)` wide, the document has two or more headings, and the preview is not shown (`writer/ui/app/WriterWindow.qml:46-47`).
- **WR-97** Turning it on with fewer than two headings toasts "No outline: fewer than two headings"; with too narrow a window, "Widen the window for the outline". The setting stays on either way, so the outline appears once conditions are met (`writer/ui/app/WriterWindow.qml:153-158`).
- **WR-98** Headings come from the reader's parser, so `#` lines inside fenced code are not headings. They are reparsed 400 ms after typing stops, also while the outline is hidden (the header chip depends on them) (`writer/ui/app/WriterWindow.qml:56`, `writer/ui/app/WriterWindow.qml:227-229`, `lib/ranges.js:132-137`, `writer/tools/check:95-108`).
- **WR-99** The outline panel is `S(264)` wide, from below the header to the window bottom, at the window's left edge. It slides in and out horizontally over 160 ms, ease-out cubic (`writer/ui/app/WriterWindow.qml:328-340`).
- **WR-100** The current section is the last heading whose start is at or before the cursor; that entry is shaded and scrolled into view in the list (`writer/ui/app/WriterWindow.qml:48-55`, `components/Outline.qml:35-41`, `components/Outline.qml:73`).
- **WR-101** Only H1 to H4 are listed, indented by `S(14)` per level below the shallowest level present (`components/Outline.qml:33-34`, `components/Outline.qml:97`).
- **WR-102** Clicking an entry places the cursor at the start of the heading line, glides that line to `min(top padding, view height / 3)` below the top over 220 ms (centre in typewriter mode), and focuses the editor (`writer/ui/app/WriterWindow.qml:342`, `components/Editor.qml:112-119`).
- **WR-103** The outline steps aside with the chrome while typing (section 12): opacity 0 while typing unless hovered, fading out over 600 ms and back over 160 ms (`writer/ui/app/WriterWindow.qml:337-339`).
- **WR-104** Outline look: background foreground at alpha 0.025 (light) or 0.02 (dark); a 1 px hairlineSoft line at its right edge; title "CONTENTS" (uppercase) at `(S(20), S(18))`, caption size, bold, letter spacing 1, faint (`components/Outline.qml:43-59`).
- **WR-105** Outline entries: the list starts `S(10)` below the title and ends `S(12)` above the bottom; each row is label height plus `S(12)`; the shade is inset `S(10)` each side with radius `max(3, cornerRadius - 2)`, hover colour on hover, accent at alpha 0.1 when current, colour change 120 ms (`components/Outline.qml:61-93`).
- **WR-106** Entry text starts at `S(22) + depth * S(14)`, elided at the right with `S(18)` to spare. Top-level entries are body size, bold, dim; deeper entries are bodySmall in a colour halfway between dim and faint; the current entry is in the text colour; colour change 120 ms. Empty heading text shows "Untitled" (`components/Outline.qml:95-109`).

## 6. Preview (Ctrl+P)

- **WR-107** Ctrl+P, Ctrl+Shift+P or the eye chip shows the preview: the document set by the reader's renderer (the same page the reader shows), filling the window below the header, with the editor, edge fades and word count hidden (`writer/ui/app/WriterWindow.qml:95-106`, `writer/ui/app/WriterWindow.qml:368-386`, `README.md:93`).
- **WR-108** On open, the preview is scrolled (without animation) so the block containing the cursor's source line is at the top: the last block whose source start line is at or before the cursor line. If that is the first block, the preview is not scrolled (`writer/ui/app/WriterWindow.qml:108-118`).
- **WR-109** The preview fades in over 160 ms (`writer/ui/app/WriterWindow.qml:373-374`).
- **WR-110** The preview keeps its place when its text is updated (`keepPlace`), and refreshes from the editor text 200 ms after a change while shown (`writer/ui/app/WriterWindow.qml:230-234`, `writer/ui/app/WriterWindow.qml:376`, `components/MarkdownView.qml:211-216`).
- **WR-111** In the preview, Esc or Ctrl+P returns to the editor and focuses it; Ctrl+S saves. Other keys are the reader's (`writer/ui/app/WriterWindow.qml:381-385`).
- **WR-112** Returning puts the user at the same place in the source: the editor's scroll and cursor are untouched while the preview is up (`README.md:93`; mechanism inferred from `writer/ui/app/WriterWindow.qml:102-105`).
- **WR-113** While the preview shows, the outline is hidden, the header's bottom hairline shows, the top progress bar tracks the preview's progress, and the chrome stays visible (`writer/ui/app/WriterWindow.qml:47`, `writer/ui/app/WriterWindow.qml:415`, `writer/ui/app/WriterWindow.qml:429`, `writer/ui/app/WriterWindow.qml:604`).
- **WR-114** The preview's text size follows the reader's saved zoom (omarky-reader state), not the writer's zoom (inferred from `services/Reading.qml:13`, `services/Prefs.qml:16-17`). Quire should decide whether that coupling is wanted.

## 7. Find

- **WR-115** Ctrl+F (or the search chip) shows the find bar and focuses its input, selecting all of its text. If the editor has a selection, the selection's text becomes the query first (`writer/ui/app/WriterWindow.qml:165-170`).
- **WR-116** Matching is literal and case-insensitive, non-overlapping, at most 10000 matches (`lib/editing.js:140-148`).
- **WR-117** Typing in the input searches from the editor's selection start: the current match is the first at or after it, wrapping to the first match (`writer/ui/app/WriterWindow.qml:124-132`, `writer/ui/app/WriterWindow.qml:559`).
- **WR-118** The current match is shown by selecting it in the editor and scrolling it into view (forced glide). Other matches are not marked (`writer/ui/app/WriterWindow.qml:140-143`, `components/Editor.qml:94-97`).
- **WR-119** In the input: Enter next match, Shift+Enter previous, both wrapping; Esc runs the global Esc (WR-77); Ctrl+F selects all of the input (`writer/ui/app/WriterWindow.qml:560-564`).
- **WR-120** From the editor: Ctrl+G and F3 go to the next match (running a fresh search when there are no matches yet), Shift+F3 to the previous; these work while the bar is hidden if a query remains (`writer/ui/app/WriterWindow.qml:171-172`).
- **WR-121** Matches are recomputed on every text change while the bar is shown (`writer/ui/app/WriterWindow.qml:364`).
- **WR-122** Find bar look: right-aligned `S(16)` from the window edge, `S(10)` below the header, `S(340)` by `S(40)`, radius `max(6, cornerRadius)`, popup background, 1 px hairline border. A search glyph (Nerd Font U+F002) at x `S(12)`, bodySmall, faint; the input `S(8)` after it, body size, text colour, selection colour; the count `S(12)` from the right, caption size (`writer/ui/app/WriterWindow.qml:526-576`).
- **WR-123** The count reads empty with no query, "none" in the error colour with no matches, otherwise "i of n" in faint (`writer/ui/app/WriterWindow.qml:571-572`).
- **WR-124** The shared `components/FindBar.qml` is the reader's; the writer has its own bar as above (`reader/ui/app/ReaderWindow.qml:630`; no writer use).

## 8. Fonts, size, spelling

- **WR-125** Font cycle (Ctrl+Shift+M or the font chip): iA Writer Quattro S, then iA Writer Duospace, then iA Writer Mono S, then back. The toast shows the new family name. Persisted; default Quattro S. The preview keeps the reader's family (`writer/ui/app/Settings.qml:19-21`, `writer/ui/app/Settings.qml:38-41`, `writer/ui/app/WriterWindow.qml:162`).
- **WR-126** Text size: Ctrl+= / Ctrl++ add 0.1 to zoom, Ctrl+- subtract 0.1, clamped to 0.6 to 2.2 and rounded to two decimals. No toast. Persisted; default 1.0. Zoom changes only the editor text, never the chrome (`writer/ui/app/Settings.qml:36`, `writer/ui/app/WriterWindow.qml:163-164`, `components/Editor.qml:24`).
- **WR-127** Code spans and fenced code use the mono family (iA Writer Mono S by default, overridable by `OMARKY_MONO_FONT`) whatever the prose family (`components/Editor.qml:261`, `services/Reading.qml:37-38`).
- **WR-128** Spelling (Ctrl+Shift+L) is a persisted setting, default on; toggling toasts "Spelling on" or "Spelling off" (`writer/ui/app/Settings.qml:24`, `writer/ui/app/WriterWindow.qml:161`).
- **WR-129** Dictionary language is Australian English (en_AU), fixed (`native/markdownhighlighter.h:105`, `README.md:87`).
- **WR-130** A misspelt word gets a straight single underline in the error colour; glyph colour unchanged. (The header comment says "wavy", the code draws a single straight line, and the README calls it "a quiet line".) (`native/markdownhighlighter.cpp:748-751`, `native/markdownhighlighter.h:19-20`, `docs/screenshots/writer/editor.png`).
- **WR-131** Words are runs of letters (with combining marks and apostrophes, straight or curly). Trailing apostrophes are trimmed; words shorter than 2 letters are skipped (`native/markdownhighlighter.cpp:63`, `native/markdownhighlighter.cpp:714-724`).
- **WR-132** Not checked: anything in code spans, fenced code, front matter, HTML, comments, link URLs and syntax marks; words touching a digit, `_`, `/` or `@` on either side; words followed by `.` and a letter (file names); words with more than one capital letter (acronyms, camelCase) (`native/markdownhighlighter.cpp:725-742`).
- **WR-133** Deferred squiggle: a word is never marked while it is being typed. The word ending exactly at the cursor, or containing the cursor on the cursor's line, is skipped. When the cursor leaves a line, that line is re-checked, so a misspelling appears once you move past it (`native/markdownhighlighter.cpp:184-190`, `native/markdownhighlighter.cpp:743-745`, `README.md:87`). Moving within the same line does not re-check until the line is next repainted (inferred).
- **WR-134** Right-click on a misspelt word (spelling on and a dictionary present): the menu lists up to 6 suggestions in bold strong colour (each replaces the word and puts the cursor after it), or "No suggestions" (disabled), then `Add “word” to dictionary` (adds to the personal dictionary and repaints), then a separator (`components/Editor.qml:216-234`, `native/markdownhighlighter.cpp:301-322`).
- **WR-135** Every right-click menu then lists Cut (Ctrl+X) and Copy (Ctrl+C), disabled without a selection, and Paste (Ctrl+V). A right-click with no selection first moves the cursor to the click (`components/Editor.qml:235-240`).
- **WR-136** With no dictionary available every word counts as correct: no underlines and no spelling items (`native/markdownhighlighter.cpp:289-299`).

## 9. Files: drafts, untitled, autosave, reload, recent

- **WR-137** State lives in `$OMARKY_WRITER_STATE_DIR`, else `$XDG_STATE_HOME/omarky-writer`, else `~/.local/state/omarky-writer`; drafts in its `drafts/` (`writer/ui/app/Settings.qml:14-16`).
- **WR-138** With no file, a window opens on a new draft path `drafts/untitled-YYYYMMDD-HHMMSS-R.md` (R a random 0 to 999). The file does not exist until something is saved (`writer/ui/app/Settings.qml:52-58`, `writer/ui/app/WriterWindow.qml:189`).
- **WR-139** A draft's header shows "Untitled" in dim and "draft" as its folder; the window title uses "Untitled" (`writer/ui/app/WriterWindow.qml:23-30`, `writer/ui/app/WriterWindow.qml:446`).
- **WR-140** A path that does not exist opens as an empty document; the file appears on the first save (`components/Buffer.qml:54-60`).
- **WR-141** Autosave: 900 ms after the last text change, if the text differs from what was last read or written. A draft is not written while it is empty or only whitespace (`writer/ui/app/WriterWindow.qml:214-224`, `writer/tools/check:54-55`).
- **WR-142** Writes are atomic (`components/Buffer.qml:42`, `writer/ui/app/WriterWindow.qml:269`).
- **WR-143** Closing a window (Ctrl+W, the window's close, or the app ending) saves unsaved text; a draft that is empty or whitespace is deleted instead, leaving no file (`writer/ui/app/WriterWindow.qml:67-73`).
- **WR-144** Unsaved state shows as a `S(6)` dot after the folder label in faint; a save error turns it to the error colour. It shows when the text differs from the last read or written text, or after an error (`writer/ui/app/WriterWindow.qml:463-468`). A save error's message is otherwise not shown (inferred).
- **WR-145** The file is watched. A change on disk is read 150 ms after the last change event and ignored when it matches what was last read or written (`components/Buffer.qml:43-53`, `components/Buffer.qml:61`, `components/Buffer.qml:66`).
- **WR-146** Reload when clean: with no unsaved edits, the new text replaces the editor text, the cursor keeps its offset (clamped to the new length), and the toast says "Reloaded from disk" (`writer/ui/app/WriterWindow.qml:199-206`, `writer/tools/check:56-58`).
- **WR-147** Conflict when dirty: a banner `S(40)` tall under the header, across the window, washed with yellow at alpha 0.1 (light) or 0.09 (dark), a 1 px yellow line at alpha 0.25 along its bottom, centred content `S(10)` apart: "The file changed on disk." (bodySmall, text colour), a "Load theirs" chip and a "Keep mine" chip (bodySmall) (`writer/ui/app/WriterWindow.qml:487-523`).
- **WR-148** "Load theirs" replaces the editor text with the disk text and clears the banner (cursor not preserved). "Keep mine" writes the editor text over the file and clears the banner (`writer/ui/app/WriterWindow.qml:506-521`).
- **WR-149** While the banner is up, autosave is not suspended: further typing writes the editor text over the disk version 900 ms later, with the banner still showing (inferred from `writer/ui/app/WriterWindow.qml:216-224`). Quire should decide whether to pause autosave during a conflict.
- **WR-150** Open (Ctrl+O): a dialog titled "Open a Markdown file", filters "Markdown (*.md *.markdown *.mdown *.mkd *.txt)" and "All files (*)", starting in the file's folder, or home for a draft (`writer/ui/app/WriterWindow.qml:236-245`).
- **WR-151** Opening a file from an empty draft window (empty or whitespace) loads it into that window and deletes the draft file; otherwise it opens in a new window, or brings forward the window that already has it (`writer/ui/app/WriterWindow.qml:87-93`, `writer/ui/app/WriterWindow.qml:316-320`, `writer/ui/shell.qml:33-50`).
- **WR-152** Save As (Ctrl+Shift+S, or Ctrl+S on a draft): a save dialog titled "Save as", default suffix `md`, filters "Markdown (*.md)" and "All files (*)", starting in the file's folder, or `~/Documents` for a draft. On success the window switches to the new path, a former draft file is deleted, the path joins the recent list, and the toast says "Saved as NAME". On failure the toast says "Could not save: ERROR" (`writer/ui/app/WriterWindow.qml:247-282`).
- **WR-153** Recent list: `recent.json`, newest first, unique, at most 20. A file is added when opened (not drafts) and after Save As (`writer/ui/app/Settings.qml:43-50`, `writer/ui/app/WriterWindow.qml:196`, `writer/ui/app/WriterWindow.qml:277`).
- **WR-154** Ctrl+R menu, anchored at `(S(12), header height - S(4))`: first up to 10 recent files other than the current one (label: file name; right column: folder with `~` for home), then a separator (only if both parts have rows), then the drafts other than the current one from the 12 most recently modified (label: the first non-blank line with leading `#` and spaces removed, cut to 48 characters, or "Untitled"; right column: `draft · D Mon HH:MM`, for example `draft · 3 Nov 14:05`). With nothing: one disabled row "Nothing recent yet". Picking a row opens it as in WR-151 (`writer/ui/app/WriterWindow.qml:284-314`).
- **WR-155** Recent files that no longer exist are still listed (inferred: no existence check in `writer/ui/app/WriterWindow.qml:295-300`).
- **WR-156** A file opens with the cursor at the start and the view at the top, unless opened with `--end`, which puts the cursor after the last character and scrolls it into view (for capture notes) (`writer/ui/app/WriterWindow.qml:190-198`, `bin/omarky:13`).
- **WR-157** One app instance: launching with files while it runs opens each in a new window there; a file already open brings its window forward (and with `--end` moves its cursor to the end) instead of opening twice. No files opens a new draft window (`writer/ui/shell.qml:13-50`, `writer/ui/shell.qml:80`, `launcher/omarky/instance.rb:39-47`).

## 10. Help sheet (Ctrl+/)

- **WR-158** Ctrl+/ or the `?` chip toggles the sheet; Esc or a click anywhere outside the card closes it; clicks on the card do nothing. Closing focuses the editor (`writer/ui/app/WriterWindow.qml:173`, `writer/ui/app/WriterWindow.qml:621-626`, `writer/ui/app/HelpSheet.qml:37`, `writer/ui/app/HelpSheet.qml:82`).
- **WR-159** The editor keeps keyboard focus under the sheet, so typed characters still edit the hidden text (inferred: the sheet takes no focus). Quire should probably block text input while it shows.
- **WR-160** Scrim: the page background at alpha 0.72 over the whole window (`writer/ui/app/HelpSheet.qml:12`).
- **WR-161** Card: centred, width `min(window - S(48), content + S(48))`, height content plus `S(44)`, radius `max(6, cornerRadius)`, popup background, 1 px hairline border. Content starts at `(S(24), S(22))` (`writer/ui/app/HelpSheet.qml:74-97`).
- **WR-162** Version label: "OmarkyWriter VERSION" at the card's top right, `S(24)` from the right edge, `S(22)` from the top, caption size, ghost colour (`writer/ui/app/HelpSheet.qml:84-92`).
- **WR-163** Layout: two columns when the window is wider than `S(1040)`, else one. Columns are `S(470)` wide and `S(40)` apart; the first column takes the first `ceil(groups / 2)` groups. Groups are `S(18)` apart (`writer/ui/app/HelpSheet.qml:39-42`, `writer/ui/app/HelpSheet.qml:94-108`).
- **WR-164** Group: an uppercase title in the mono family, caption size, letter spacing 1, faint; then rows `S(26)` tall with `S(7)` between items. Each row is a keycap at x 0 and the description at x `S(170)`, bodySmall, text colour (`writer/ui/app/HelpSheet.qml:44-72`).
- **WR-165** Keycap: label plus `S(12)` wide and `S(6)` tall, radius `max(3, cornerRadius - 2)`, raised fill, 1 px hairline border, caption size, dim text (`components/KeyCap.qml:9-23`).
- **WR-166** Content, in order (the second group only when Claude Code is installed) (`writer/ui/app/HelpSheet.qml:14-28`):
  - Write: "Ctrl+B  Ctrl+I" Bold, italic; "Ctrl+E" Inline code; "Ctrl+Shift+X" Strikethrough; "Ctrl+K" Link; "Ctrl+1 … Ctrl+6" Heading level; "Ctrl+0" Plain paragraph; "Tab  Shift+Tab" Indent, outdent a list item; "Enter" Continue a list or quote.
  - Rewrite with Claude: "Ctrl+J" Rewrite the block or selection; "Ctrl+Shift+J" Rewrite the section; "Ctrl+J again" Take in more; "Enter  Esc" Use the rewrite, drop it; "Alt+↑  Alt+↓" Earlier and later rewrites.
  - View: "Ctrl+P" Preview; "Ctrl+\" Outline; "Ctrl+Shift+F" Focus on the paragraph; "Ctrl+Shift+T" Typewriter scrolling; "Ctrl+Shift+M" Next font; "Ctrl+Shift+L" Spelling; "Ctrl+=  Ctrl+−" Text size (the minus is U+2212).
  - File: "Ctrl+S" Save (saving is automatic); "Ctrl+Shift+S" Save as; "Ctrl+N" New window; "Ctrl+O" Open; "Ctrl+R" Recent files and drafts; "Ctrl+F  F3" Find, next match; "Ctrl+W" Close.

## 11. Rewrite with Claude (writer entry points only)

The rewrite line and card are a shared component also used by the reader's edit sheet; its full behaviour belongs to that spec.

- **WR-167** Ctrl+J opens the rewrite line for the selection, or the block at the cursor; Ctrl+Shift+J for the section. Without Claude Code installed the toast says "Rewrite needs Claude Code (claude on your PATH)" (`components/Editor.qml:373`, `components/Rewriter.qml:58-59`).
- **WR-168** In the line: Enter takes the rewrite as one undo step, Esc drops it, Ctrl+J again widens block to section to whole document, Alt+Up and Alt+Down step through rewrites, Up and Down recall earlier instructions (`components/Rewriter.qml:201-209`, `writer/tools/check:60-92`, `CHANGELOG.md:9-13`).
- **WR-169** The range being rewritten gets a soft shade in the hover colour, extending `S(10)` beyond the text column on each side and `S(6)` above and below, radius `max(6, cornerRadius)`. The rewrite UI is hidden while the preview shows (`components/Editor.qml:90-92`, `components/Editor.qml:163-172`, `writer/ui/app/WriterWindow.qml:617`).

---

## 12. Window, column and chrome

### Window and column

- **WR-170** Window: default `S(1100)` by `S(900)`, page-background colour, title "OmarkyWriter" + space + U+2014 + space + the file name ("Untitled" for drafts), app id `omarky-writer` (`writer/ui/app/WriterWindow.qml:62-65`, `writer/ui/shell.qml:1`).
- **WR-171** Measure: `min(editor width - 2 * gutter, round(66 * average character width of the editor font at size E))`, about 66 characters, "the measure Byword used" (`components/Editor.qml:25-26`).
- **WR-172** Column placement: with `free = editor width - measure`, the column's left edge is at `round(max(gutter, free / 2 - shift))`, where `shift = min(lean / 2, free / 2 - free / 2.618)` and `lean` is how far the window extends left of the editor (the outline's width, `S(264)`, when shown; 0 otherwise). So the column is centred in the area beside the outline, then leans back toward the window's centre by up to half the outline width, stopping at a golden split of the free space (`components/Editor.qml:29-37`, `writer/ui/app/WriterWindow.qml:345-350`).
- **WR-173** The editor occupies the window to the right of the outline and slides with it during the 160 ms outline animation (`writer/ui/app/WriterWindow.qml:347-348`).
- **WR-174** Text geometry: the text box starts one gutter left of the column edge and is `measure + 2 * gutter` wide; each line's left margin is one gutter, so text runs from the column edge to one gutter past the measure on the right. The effective wrap width is therefore `measure + gutter` (inferred from `components/Editor.qml:174-178` and `native/markdownhighlighter.cpp:330`; may be unintended). Without the highlighter, text starts one gutter left of the column edge (`components/Editor.qml:85-87`).
- **WR-175** Caret: width `max(2, round(E / 9))`, fully rounded, accent colour, shown only while the editor has focus. It blinks as visible 520 ms, fade out 180 ms, hidden 340 ms, fade in 180 ms, repeating; while typing it stays solid and resets to visible (`components/Editor.qml:191-207`).
- **WR-176** Selection: selection colour behind, selected text in the strong colour (`components/Editor.qml:185-186`).
- **WR-177** Edge fades: a `S(28)` band at the top of the editor area fading from page colour to transparent, placed just under the header while the header is more than half visible, else at the window top; a `S(64)` band at the bottom fading from transparent to page colour by 60 percent of its height. Both span the editor's width and hide in preview (`writer/ui/app/WriterWindow.qml:388-410`).
- **WR-178** Progress: a 2 px bar along the top edge, accent at opacity 0.55, its width the window width times the scroll progress (editor, or preview when shown), drawn only when wider than 1 px. It does not step aside while typing (`writer/ui/app/WriterWindow.qml:601-608`, `components/Editor.qml:45`).

### Header

- **WR-179** Header: `S(44)` tall, page colour, full width. A 1 px hairlineSoft line along its bottom shows when the preview is up or the editor has scrolled past 0.2 percent progress, fading over 200 ms (`writer/ui/app/WriterWindow.qml:417-431`).
- **WR-180** Left group, `S(18)` from the left, items `S(8)` apart: the file name (body size, bold, text colour or dim for drafts, middle-elided at 40 percent of header width, pointing-hand cursor, click opens the recent menu); the folder (bodySmall, faint, start-elided to fit, `~` for home, "draft" for drafts); the unsaved dot (WR-144) (`writer/ui/app/WriterWindow.qml:433-469`).
- **WR-181** Right group, `S(10)` from the right, chips `S(2)` apart, in order: outline (only with two or more headings), find, font, focus, typewriter, preview, help. Glyphs (Nerd Font): U+F451, U+F002, U+F031, U+F140, U+F246, U+F06E, U+F128. Tooltips: "Outline  Ctrl+\", "Find  Ctrl+F", "Font  Ctrl+Shift+M", "Focus  Ctrl+Shift+F", "Typewriter  Ctrl+Shift+T", "Preview  Ctrl+P", "Shortcuts  Ctrl+/" (`writer/ui/app/WriterWindow.qml:471-484`).
- **WR-182** A chip shows as active (hover fill and hairline border, text-colour glyph) while its mode is on: outline shown, find shown, focus on, typewriter on, preview shown, help shown (`writer/ui/app/WriterWindow.qml:477-483`).
- **WR-183** Chip: glyph at body size; height twice the glyph size; width content plus 1.1 times the size; radius `max(3, min(cornerRadius, height / 3))`; transparent when idle, hover colour fill and hairline border on hover or active, colour change 120 ms; glyph dim, text colour on hover or active (`components/ChipButton.qml:13-30`).

### Chrome stepping aside while typing

- **WR-184** "Typing" means the text changed within the last 1400 ms (`components/Editor.qml:44`, `components/Editor.qml:139`, `components/Editor.qml:209`).
- **WR-185** The chrome (header, outline, word count) is visible when not typing, or while the pointer is over the header or the outline, or while the preview, the find bar or the conflict banner is shown (`writer/ui/app/WriterWindow.qml:415`).
- **WR-186** Stepping aside fades to opacity 0 over 600 ms; returning fades to 1 over 160 ms (`writer/ui/app/WriterWindow.qml:423`, `writer/ui/app/WriterWindow.qml:339`, `writer/ui/app/WriterWindow.qml:597`).
- **WR-187** The README says the chrome comes back "when you move the mouse", but the code only brings it back after the 1400 ms pause or when the pointer is over the (invisible) header or outline. A page-wide hover handler and a `lastMove` property exist and are unused (`README.md:16`, `README.md:87`, `writer/ui/app/WriterWindow.qml:413-415`). Quire must pick one; the README's intent is "any mouse move" (inferred).

### Word count

- **WR-188** Counted 250 ms after the last change: front matter removed; characters are all non-whitespace characters including Markdown marks; for words, fenced code, link and image URLs (keeping link text), HTML tags, bare URLs and the characters ``# > * _ ` ~ = | [ ] -`` are removed, then words are runs of letters or digits that may contain `'`, `’`, `.` or `-`. Hyphenated words therefore count as two. Reading time is `round(words / 230)` minutes, at least 1 when there is any word (`lib/editing.js:9-12`, `lib/editing.js:111-125`, `writer/ui/app/WriterWindow.qml:226`).
- **WR-189** Display: "N words" ("1 word" for one); on hover "N words   ·   C characters   ·   M min read" (three spaces either side of each middle dot); with a selection "S of N words selected", where S counts the selection. Numbers use en_AU grouping (1,493) (`writer/ui/app/WriterWindow.qml:585-592`, `docs/screenshots/writer/focus.png`).
- **WR-190** Position and look: `S(14)` above the window bottom, centred on the editor area (not the leaned column), caption size, faint, dim on hover. Visible with the chrome or whenever there is a selection; hidden in preview (`writer/ui/app/WriterWindow.qml:579-599`).

### Toasts and menus

- **WR-191** Toast: a pill centred at the bottom, `S(28)` above the window bottom, sliding up `S(8)` as it appears; padding `S(14)` across and `S(7)` down (width label plus `S(28)`, height label plus `S(14)`), radius half its height, popup background, hairline border, bodySmall text colour. Fades over 140 ms, moves over 160 ms ease-out cubic, hides 1600 ms after the last message (`components/Toast.qml:10-40`).
- **WR-192** Toast messages in the writer: "Saved", "Saved as NAME", "Could not save: ERROR", "Reloaded from disk", "Focus on/off", "Typewriter on/off", "Spelling on/off", the font name, "No outline: fewer than two headings", "Widen the window for the outline" (`writer/ui/app/WriterWindow.qml:84`, `writer/ui/app/WriterWindow.qml:156-162`, `writer/ui/app/WriterWindow.qml:206`, `writer/ui/app/WriterWindow.qml:278-280`).
- **WR-193** Popup menu (right-click and recent): card at the requested point, kept `S(8)` inside the window; width at least `S(200)`; `S(6)` padding; radius `max(6, cornerRadius)`; popup background; hairline border. Rows `S(30)` tall, separators `S(9)` with a hairlineSoft line inset `S(8)`; label at x `S(10)`, bodySmall, text colour (strong and bold for suggestions, faint when disabled); key hint right-aligned `S(10)` in, caption, faint; hover row fill in the hover colour with radius `max(3, cornerRadius - 2)`. A press outside closes it; closing refocuses the editor (`components/PopupMenu.qml:25-91`, `writer/ui/app/WriterWindow.qml:628-631`).

---

## 13. Settings (there is no settings sheet)

The writer has no settings screen. `writer/ui/app/Settings.qml` is the persisted store, shared by every window of the app, and every setting is changed by a key or a header chip (`writer/ui/app/Settings.qml:1-27`).

- **WR-194** `settings.json` in the state directory holds `{ zoom, font, focus, typewriter, spell, outline }`, written as pretty JSON after every change, and only after the file has first been read (or found missing) (`writer/ui/app/Settings.qml:31-34`, `writer/ui/app/Settings.qml:62-66`).
- **WR-195** Defaults and validation on load: zoom 1.0 (numbers clamped to 0.6 to 2.2), font "iA Writer Quattro S" (only one of the three families is accepted), focus false, typewriter false, spell true, outline false (booleans only). An unreadable file is ignored with a log warning (`writer/ui/app/Settings.qml:18-26`, `writer/ui/app/Settings.qml:75-88`).
- **WR-196** Settings are app-wide: toggling focus, typewriter, spelling, font, zoom or outline in one window changes every open window (inferred: one shared store bound by every window, `writer/ui/app/WriterWindow.qml:352-356`).

## 14. Colour roles and opacities

All derive from the live Omarchy theme and follow theme switches (`services/Theme.qml:8-12`).

- **WR-197** text: the theme foreground (`services/Theme.qml:44`).
- **WR-198** strong: the theme's `bright_foreground`, else the foreground. Used for bold, headings and selected text (`services/Theme.qml:45`).
- **WR-199** dim: foreground mixed 34 percent toward the background (`services/Theme.qml:46`).
- **WR-200** faint: foreground mixed 58 percent toward the background. Used for every faded mark (`services/Theme.qml:47`).
- **WR-201** editor-ghost (focus-mode fade): foreground mixed 72 percent toward the background (`components/Editor.qml:253`).
- **WR-202** ghost (chrome, the help sheet's version label): foreground mixed 80 percent toward the background (`services/Theme.qml:48`).
- **WR-203** accent: the theme accent. Used for the caret and the progress bar (`services/Theme.qml:30`).
- **WR-204** link: the accent, unless the accent is within RGB distance 24 of the foreground, then the theme's blue (`services/Theme.qml:40`, `lib/palette.js:11`, `lib/palette.js:25-27`).
- **WR-205** mark (list markers, quote marks, task boxes, footnotes, front-matter keys): the accent mixed 15 percent toward the background (`components/Editor.qml:256`).
- **WR-206** code: the text colour mixed 12 percent toward the accent (`components/Editor.qml:257`).
- **WR-207** codeBg: the foreground flattened onto the background at 0.07 (light themes) or 0.085 (dark) (`components/Editor.qml:258`, `services/Theme.qml:75-77`).
- **WR-208** highlight wash (`==x==`): the accent at alpha 0.22 (`native/markdownhighlighter.cpp:692-695`).
- **WR-209** error (misspelling, "none" in find, save-error dot): the theme red, else color1, else the urgent colour (`services/Theme.qml:33`, `components/Editor.qml:259`).
- **WR-210** selection: the accent at alpha 0.22 (light) or 0.3 (dark) (`services/Theme.qml:60`).
- **WR-211** hover: the foreground at alpha 0.06 (`services/Theme.qml:63`).
- **WR-212** hairline: the foreground at alpha 0.14 (light) or 0.12 (dark); hairlineSoft: 0.09 (light) or 0.075 (dark) (`services/Theme.qml:58-59`).
- **WR-213** raised (keycaps): the foreground at alpha 0.045 (light) or 0.04 (dark) (`services/Theme.qml:52`).
- **WR-214** popup background (find bar, help card, menus, toasts): the shell's popup background, defaulting to the theme background (`Commons/ShellColor.qml:80`).
- **WR-215** Light or dark is the theme's declared mode, else background luminance above 0.5 means light (`services/Theme.qml:24`).
- **WR-216** Opacities in one place: help scrim 0.72 of page colour; progress bar 0.55; scroll thumb 0.5 while scrolling; outline current-row accent 0.1; outline panel fill 0.025 or 0.02; conflict wash 0.1 or 0.09 and its line 0.25; highlight wash 0.22 (WR-104, WR-105, WR-147, WR-160, WR-178, WR-93, WR-208).

## 15. Durations (all in one place)

- **WR-217** Timers (not affected by reduced motion): typing pause 1400 ms; autosave 900 ms; word count 250 ms; outline reparse 400 ms; preview refresh 200 ms; disk change debounce 150 ms; scroll-thumb linger 700 ms; toast hold 1600 ms; fling sample window 200 ms (`components/Editor.qml:139`, `writer/ui/app/WriterWindow.qml:218`, `writer/ui/app/WriterWindow.qml:226-234`, `components/Buffer.qml:66`, `components/Editor.qml:341`, `components/Toast.qml:40`, `components/Editor.qml:300`).
- **WR-218** Animations (zero under reduced motion): chrome fade out 600 ms, in 160 ms; outline slide 160 ms ease-out cubic; header hairline 200 ms; preview fade-in 160 ms; caret 520 / 180 / 340 / 180 ms cycle; forced glide 220 ms; typewriter follow 120 ms; wheel glide 160 ms; scroll-thumb fade 300 ms; toast fade 140 ms and slide 160 ms; chip and outline-row colour 120 ms (WR-186, WR-99, WR-179, WR-109, WR-175, WR-88, WR-89, WR-91, WR-93, WR-191, WR-183, WR-105).

## 16. Plain editor fallback

- **WR-219** When the native formatter is unavailable, the editor still opens and edits in the right font, size and measure, with the same keys and autosave, but with no painted Markdown, no hanging marks, no line-height change and no spelling items; the log says it runs without the native highlighter (`docs/decisions/0016-the-editor-in-the-library.md:7`, `components/Editor.qml:267-276`, `writer/tools/check:114-116`). The writer's launcher requires the native build and refuses to start (with a desktop notification) if it fails (`writer/launcher/omarky_writer/cli.rb:24-32`). For Quire this item only matters if it ships an unpainted mode.
