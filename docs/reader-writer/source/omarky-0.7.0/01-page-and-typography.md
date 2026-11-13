# OmarkyReader page and typography spec (extracted from omarky)

Source repo is `/home/p0werj0int/Projects/omarky` (version 0.7.0). Paths are relative to it and cited as `path:line`.

## Units and conventions

- `B` is the reading body size in px. All `em` values below multiply `B`. At zoom 1.0 and the default UI base of 12, `B` = 14.
- `round()` is round half up (JavaScript `Math.round`), applied exactly where omarky applies it.
- Worked pixel values in brackets are for `B` = 14 and are computed, not measured.
- `S(n)` is a chrome length, `n` × (UI base / 12). At the default base `S(n)` = `n`.
- Colour role names (text, strong, dim, faint, ghost, accent, link, hairline and so on) are defined in section 4.
- `(inferred)` marks anything derived from reading code rather than stated in code, docs or tests.

---

## 1. Fonts

- **PAGE-01** Prose (paragraphs, lists, tables, quotes, captions) uses iA Writer Quattro S, Regular style (`services/Reading.qml:27`).
- **PAGE-02** Prose text always asks for the face by its style name "Regular", never by weight. The installed Bold file declares the same weight as Regular, so a weight-based request renders every paragraph bold. Inline bold, italic and bold italic resolve to the real Bold, Italic and Bold Italic faces (`docs/decisions/0003-quattro-s-regular-style-name.md:1-8`).
- **PAGE-03** Headings h1 to h4 use iA Writer Quattro S, Bold style (`services/Reading.qml:28`, `components/blocks/Heading.qml:30`). h5 and h6 use the Regular style of the heading family (`components/blocks/Heading.qml:30`).
- **PAGE-04** Code, diffs, inline code, raw HTML, alert titles, table headers, code badges and front-matter keys use iA Writer Mono S (`services/Reading.qml:29`, `docs/decisions/0005-heading-font.md:5-6`). Rationale is that the mono is lighter than a programmer's font and has no ligatures, so `->` and `===` show as typed (`docs/decisions/0007-code-font.md:1-9`).
- **PAGE-05** Icons use JetBrainsMono Nerd Font glyphs (octicons) only. That font is never used for text on the page (`docs/decisions/0007-code-font.md:9`, `components/blocks/ImageBlock.qml:70`).
- **PAGE-06** Each family can be overridden by environment, prose by `OMARKY_PROSE_FONT`, headings by `OMARKY_HEADING_FONT` as `Family/Style`, mono by `OMARKY_MONO_FONT` (or the older `OMARKY_CODE_FONT`) (`services/Reading.qml:20-22`, `services/Reading.qml:36-41`).
- **PAGE-07** An override naming a family that is not installed falls back to the default family. Matching of family names is case-insensitive (`services/Reading.qml:36`, `lib/fonts.js`, `test/tst_fonts.qml`).
- **PAGE-08** The installed-family list is read once at start and can be refreshed after a font is installed while the app runs (`services/Reading.qml:31-34`).
- **PAGE-09** Chrome outside the page (header, outline, find bar, toasts, help sheet, empty state labels) uses the shell UI family, default the `monospace` alias, at fixed UI sizes caption 10, bodySmall 11, body 12, subtitle 13, title 14, heading 16, display 24 px (`Commons/Style.qml:269`, `Commons/Style.qml:322-332`).

## 2. Sizes and the type scale

- **PAGE-10** Body size `B` = round(UI body 12 × 1.2 × zoom) [14 at zoom 1] (`services/Reading.qml:47`).
- **PAGE-11** Body leading is 1.6. Every text line sits in a fixed line box of round(size × leading) px [22 for body] (`services/Reading.qml:48`, `components/RichText.qml:23`).
- **PAGE-12** Zoom changes only reading text. Chrome inside the page (code badges, copy buttons) stays at UI sizes, `chrome` = UI body 12 and `chromeSmall` = UI caption 10 (`services/Reading.qml:49-52`).
- **PAGE-13** The modular scale ratio is 1.25. `size(step)` = round(B × 1.25^step) (`services/Reading.qml:67`, `services/Reading.qml:125`).
- **PAGE-14** Heading steps are h1 3, h2 2, h3 1, h4 0.5, h5 0, h6 0, giving h1 27, h2 22, h3 18, h4 16, h5 14, h6 14 px at `B` 14 (`components/blocks/Heading.qml:14`, `components/blocks/Heading.qml:27`).
- **PAGE-15** Heading leading is 1.25 for h1 and h2 and 1.35 for h3 to h6 [line boxes 34, 28, 24, 22, 19, 19 px] (`components/blocks/Heading.qml:28`).
- **PAGE-16** Code block size is round(B × 0.84) [12] with leading 1.5 [line 18 px] (`services/Reading.qml:58-59`, `components/blocks/CodeBlock.qml:17`).
- **PAGE-17** Inline code size is round(B × 0.86) [12] (`services/Reading.qml:80`).
- **PAGE-18** Other text sizes on the page, all multiples of `B`: quote text 0.94 [13], table cells 0.94 [13], table header 0.72 [10], footnote text 0.88 [12], image caption 0.84 [12], raw HTML 0.8 [11], alert title 0.76 [11], pill 0.76 [11], front-matter keys 0.74 [10], front-matter label 0.66 [9], front-matter values 0.9 [13], front-matter title size(1.5) [20] (`components/blocks/Quote.qml:32`, `components/blocks/TableBlock.qml:18-19`, `components/blocks/Footnotes.qml:14`, `components/blocks/ImageBlock.qml:104`, `components/blocks/HtmlBlock.qml:29`, `components/blocks/Alert.qml:48`, `components/Pill.qml:13`, `components/blocks/FrontMatter.qml:16`, `components/blocks/FrontMatter.qml:50`, `components/blocks/FrontMatter.qml:153`, `components/blocks/FrontMatter.qml:66`).
- **PAGE-19** Zoom is clamped to 0.6 to 2.2 and rounded to 0.01. Ctrl+Plus (or Ctrl+=) adds 0.1, Ctrl+Minus subtracts 0.1, Ctrl+0 resets to 1.0 (`services/Prefs.qml:23`, `reader/ui/app/ReaderWindow.qml:418-423`).
- **PAGE-20** Zoom persists across launches in the reader's state file. A stored value outside 0.6 to 2.2 is clamped on load, and an unreadable file is ignored with one warning (`services/Prefs.qml:56-64`).

## 3. The column (measure, margins, lean)

- **PAGE-21** The measure is round(average character width of the prose font at `B` × 68) in reading width and × 100 in wide width (`components/DocView.qml:19`). With Quattro S (average advance 608/1000 em) that is about 579 px and 851 px at `B` 14 (inferred, computed from the font's OS/2 table).
- **PAGE-22** The measure follows the prose font and the zoom, so it is always 68 (or 100) average characters regardless of size (`components/DocView.qml:19`, `services/Reading.qml:20`).
- **PAGE-23** Side room = max(S(24), min(S(56), view width × 0.06)) (`components/DocView.qml:20`).
- **PAGE-24** Column width = max(S(200), min(measure, view width − 2 × side room)) (`components/DocView.qml:21`).
- **PAGE-25** Left gutter (where the column starts) is computed from free = view width − column. shift = min(max(0, lean) / 2, free/2 − free/2.618). gutter = round(max(side room, free/2 − shift)) (`components/DocView.qml:27-31`).
- **PAGE-26** `lean` is the width the outline takes on the left (the page view's x offset). With the outline shown the column leans back toward the window's centre by up to half the outline width, but never further left than the golden-section point of the free space (`components/DocView.qml:23-26`, `reader/ui/app/ReaderWindow.qml:584`).
- **PAGE-27** With no outline the column is exactly centred in the window (lean 0, shift 0) (`components/DocView.qml:29`).
- **PAGE-28** The page has S(44) of empty room above the first block (`components/DocView.qml:32`, `components/DocView.qml:72`).
- **PAGE-29** The page has max(S(120), view height × 0.3) of empty room after the last block, so the last line can be read well above the bottom edge (`components/DocView.qml:73`).
- **PAGE-30** Markers hang left of the column so text keeps one straight left edge. This covers top-level bullets, numbers and task boxes, the heading `#` on hover, change tags and change washes (`README.md:13`, `components/blocks/ListBlock.qml:25-28`, `components/blocks/Heading.qml:41`).
- **PAGE-31** Content never shifts horizontally between blocks. Every top-level block starts at the gutter with the column width (`components/DocView.qml:353-355`).

## 4. Colour roles

All roles derive from the current Omarchy theme's foreground (fg), background (bg) and accent. `mix(a, b, t)` = a + (b − a) × t per channel. `alpha(c, a)` is c at opacity a over whatever is below. `flat(c, a)` = mix(bg, c, a), an opaque colour.

- **PAGE-32** Light mode is the theme's declared `mode`, else background luminance (0.2126 R + 0.7152 G + 0.0722 B) > 0.5 (`services/Theme.qml:24`, `services/Theme.qml:69-71`).
- **PAGE-33** text = fg. strong = the theme's `bright_foreground` if set, else fg (`services/Theme.qml:44-45`).
- **PAGE-34** dim = mix(fg, bg, 0.34). faint = mix(fg, bg, 0.58). ghost = mix(fg, bg, 0.8) (`services/Theme.qml:46-48`).
- **PAGE-35** raised = alpha(fg, light 0.045 / dark 0.04). sunken = alpha(light ? fg : black, light 0.035 / dark 0.22) (`services/Theme.qml:52-53`).
- **PAGE-36** panel (code and diff fill) = the theme's `dark_background` if set, else bg darkened by factor light 1.04 / dark 1.2 (`services/Theme.qml:56`).
- **PAGE-37** card = alpha(fg, 0.035). hover = alpha(fg, 0.06) (`services/Theme.qml:57`, `services/Theme.qml:63`).
- **PAGE-38** hairline = alpha(fg, light 0.14 / dark 0.12). hairlineSoft = alpha(fg, light 0.09 / dark 0.075) (`services/Theme.qml:58-59`).
- **PAGE-39** selection = alpha(accent, light 0.22 / dark 0.3), and selected text keeps the text colour (`services/Theme.qml:60`, `components/RichText.qml:59-60`).
- **PAGE-40** chipFill = alpha(fg, light 0.07 / dark 0.085). chipEdge = alpha(fg, light 0.07 / dark 0.06) (`services/Theme.qml:61-62`).
- **PAGE-41** Named colours green, red, yellow, blue, magenta, cyan come from the theme's named keys, else color2, color1, color3, color4, color5, color6, else accent (red falls back to the urgent colour instead) (`services/Theme.qml:32-37`).
- **PAGE-42** orange = theme orange, else yellow, else color3, else urgent (`services/Theme.qml:38`).
- **PAGE-43** link = accent, unless the accent is within RGB Euclidean distance 24 of fg, then blue. This keeps links visible in themes whose accent equals their text colour (`lib/palette.js:5-27`, `services/Theme.qml:40`).
- **PAGE-44** Inline-code text = mix(text, accent, 0.12). Inline-code background = flat(fg, light 0.07 / dark 0.085) (`services/Reading.qml:78-79`).
- **PAGE-45** Find match background = flat(accent, light 0.22 / dark 0.3). The current match is solid accent with bg-coloured text (`services/Reading.qml:87-89`, `docs/decisions/0010-window-behaviour-choices.md:7`).
- **PAGE-46** Window background is the theme bg. Popups (find bar, help card, toasts) use the shell's popup background, which equals the theme bg at full opacity (`reader/ui/app/Header.qml:58`, `components/FindBar.qml:21`).
- **PAGE-47** The theme is read live. Switching the Omarchy theme recolours the open page, code colours included, without a reload (`services/Theme.qml:16-23`, `README.md:15`, `reader/tools/check-theme`). `OMARKY_THEME_DIR` points at a theme directory instead (`services/Theme.qml:16`).
- **PAGE-48** Corner radius on page surfaces is max(4, shell rounding) for code, diff, alert, table, image and find bar, max(6, shell rounding) for front matter, washes, drop overlay and change flash. The shell rounding is the compositor's window rounding, default 0, so the default radius is 4 or 6 px (`Commons/Style.qml:31`, `components/blocks/CodeBlock.qml:48`, `components/blocks/FrontMatter.qml:34`).

## 5. Syntax colours

- **PAGE-49** Code colours are generated from the current theme's own palette so a code block matches its page (`docs/decisions/0006-ksh-themes-from-colors-toml.md:1-15`).
- **PAGE-50** Mapping (`lib/kshtheme.js:58-90`). Normal and Variable fg. Keyword, ControlFlow, Import magenta. Function, Extension, Information blue. String, VerbatimString green. SpecialString, BuiltIn, Others cyan. DecVal, BaseN, Float, Constant, Char, SpecialChar, Preprocessor orange. DataType, Attribute, Warning yellow. Operator and Annotation in a code-dim = mix(fg, bg, 0.3). Comment and Documentation in code-faint = mix(fg, bg, 0.52), italic. CommentVar code-dim italic. RegionMarker code-faint. Alert red bold. Error red underlined (`lib/kshtheme.js:38-39`).
- **PAGE-51** Code-dim (0.3) and code-faint (0.52) are slightly stronger than the page's dim (0.34) and faint (0.58) (`lib/kshtheme.js:38-39`, `services/Theme.qml:46-47`).
- **PAGE-52** Fence language names are mapped by alias, for example sh, shell, console to Bash, js, mjs to JavaScript, ts to TypeScript, py to Python, rs to Rust, yml to YAML, jsonc to JSON, c and h to C, cpp, cc, hpp to C++, patch to Diff, math and tex to LaTeX. Unknown names are passed through unchanged (`services/Highlight.qml:34-53`).
- **PAGE-53** When highlighting is unavailable, code renders as plain mono in the text colour with nothing logged. `OMARKY_NO_KSH=1` forces this path for checks (`services/Highlight.qml:57-58`, `components/blocks/CodeBlock.qml:96`). A theme added after launch uses the highlighter's stock light or dark theme until restart (`services/Highlight.qml:14-16`).

## 6. Vertical rhythm

The gap above each block depends on the block before it. Headings bind to what follows them. All values are round(B × factor) (`services/Reading.qml:92-113`).

- **PAGE-54** The first block has gap 0 (its room is the S(44) top room) (`services/Reading.qml:95`).
- **PAGE-55** A heading after a thematic break gets 1.5 em [21] (`services/Reading.qml:101`).
- **PAGE-56** A heading directly after a heading of a higher rank (for example h3 after h2) gets 1.25 em [18] when the earlier heading is h1 or h2, else 0.7 em [10] (`services/Reading.qml:102-103`).
- **PAGE-57** Any other heading gets h1 2.6 em [36], h2 2.2 em [31], h3 1.7 em [24], h4 1.4 em [20], h5 1.3 em [18], h6 1.3 em [18] (`services/Reading.qml:104`).
- **PAGE-58** "Framed" blocks are code, diff, table, alert, image, gallery, quote, front matter (`services/Reading.qml:106`).
- **PAGE-59** A block after a heading gets 0.75 em [11] if the heading is h1 or h2 or the block is framed, else 0.5 em [7] (`services/Reading.qml:107`).
- **PAGE-60** A block after front matter gets 2.2 em [31] (`services/Reading.qml:108`).
- **PAGE-61** A thematic break, or a block after one, gets 1.9 em [27] (`services/Reading.qml:109`).
- **PAGE-62** The footnotes block gets 2.6 em [36] (`services/Reading.qml:110`).
- **PAGE-63** A framed block, or a block after one, gets 1.25 em [18] (`services/Reading.qml:111`).
- **PAGE-64** Every other pair (paragraph to paragraph, paragraph to list and so on) gets 0.9 em [13] (`services/Reading.qml:112`).
- **PAGE-65** Rules apply in the order PAGE-55 to PAGE-64. The first match wins (`services/Reading.qml:94-113`).
- **PAGE-66** Inside a container (list item, quote, alert, footnote) the same rules apply, capped at 0.9 em [13] (`components/BlockStack.qml:27`).
- **PAGE-67** Inside a tight list item a list next to any block gets 0.3 em [4] (`components/BlockStack.qml:26`).
- **PAGE-68** A folded section in changes mode spaces like the heading it stands for (`services/Reading.qml:96-98`).
- **PAGE-69** There is no extra margin after a block. Vertical space is the gap above the next block only (`components/DocView.qml:297-298`).

## 7. Inline text

- **PAGE-70** Bold is the Bold face in the strong colour. Bold inside a link keeps the link colour (`lib/inline.js:173`).
- **PAGE-71** Italic is the true Italic face (`lib/inline.js`, `docs/decisions/0003-quattro-s-regular-style-name.md:7-8`).
- **PAGE-72** Strikethrough is a line-through in dim (`lib/inline.js:169`).
- **PAGE-73** Links are in the link colour with no underline. The underline appears only while the pointer is over that link (`lib/inline.js:174-176`).
- **PAGE-74** The pointer is a pointing hand over a link and an I-beam elsewhere on text (`components/RichText.qml:159`).
- **PAGE-75** Inline code is mono at round(B × 0.86) in the inline-code colour, padded by a four-per-em space (U+2005) on each side, on a chip (`lib/inline.js:164`).
- **PAGE-76** The inline-code chip is round(inline-code size × 1.5) [18] tall, corner radius max(3, round(height × 0.22)) [4], filled chipFill with a 1 px chipEdge border, and centred on the x-height of the line (`components/RichText.qml:76`, `components/RichText.qml:123-127`, `components/RichText.qml:154`).
- **PAGE-77** A chip that wraps across lines is drawn as one piece per line (`components/RichText.qml:90-100`). A chip piece is at least 4 px wide (`components/RichText.qml:96`, `components/RichText.qml:100`).
- **PAGE-78** Footnote references are superscript (`lib/inline.js:168`).
- **PAGE-79** Raw inline HTML is shown literally in mono at the inline-code size in faint, never interpreted (`lib/inline.js:166`, `docs/decisions/0002-raw-html-recognised-never-rendered.md:8-11`).
- **PAGE-80** `<br>` (any case, with or without slash) is a line break. Inline HTML comments are dropped (`lib/inline.js:47`, `docs/decisions/0002-raw-html-recognised-never-rendered.md:8-13`).
- **PAGE-81** Parsing has typographer on, so straight quotes become curly, `--` and `---` become dashes, `...` an ellipsis, `(c)` a copyright sign (inferred from markdown-it `typographer: true` at `lib/markdown.js:14`).
- **PAGE-82** Autolinks cover http, https and ftp URLs, `www.` hosts and e-mail addresses. Trailing punctuation is trimmed and balanced parentheses kept, as on GitHub. Bare domains like `file.md` are not linked. A match glued to letters before it is not linked (`lib/linkify.js:3-32`, `docs/decisions/0004-own-linkify-and-footnotes.md:3-8`).
- **PAGE-83** Text on the page is selectable with the mouse but never takes keyboard focus, so keys always reach the window. Only one block holds a selection at a time (`components/RichText.qml:52`, `docs/decisions/0010-window-behaviour-choices.md:22-24`, `components/MarkdownView.qml:123`).

## 8. Motion

- **PAGE-84** Every animation duration becomes 0 under reduced motion, which is `OMARKY_REDUCED_MOTION=1` or the compositor's animations switched off (`services/Reading.qml:61-66`, `services/Reading.qml:115-123`). Timers (toast hold, chord timeout, debounces) are not affected (inferred from their literal intervals).
- **PAGE-85** The page fades in over 150 ms with an ease-out-quad curve when a file opens (`components/DocView.qml:284-289`).
