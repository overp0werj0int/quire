# Blocks

This file covers every block type on the reader page in preview mode: how the note is split into
blocks, and how each block is set. The page itself (fonts, column, colour roles, rhythm, inline
text) is in `page-and-typography.md`. The format of the items is in `README.md`.

Units and conventions are those of `page-and-typography.md`. `B` is the body size, 14 by
default. Worked values in brackets are for `B` = 14 and scale with it. A value marked fixed does
not scale. Colour role names are defined in `page-and-typography.md`, section 4. Citations in
parentheses point into omarky 0.7.0 (`/home/p0werj0int/Projects/omarky`), unless they start with
`quire:`.

## 1. Block model and parsing

- **BLK-01** [in] [verify: unit] A note is a flat list of top-level blocks, and each block knows its first and last source line. Lists, quotes, alerts and footnotes hold their child blocks inside themselves, and children are drawn by the same block builder, recursively. (`docs/decisions/0001-flat-blocks-in-a-listview.md:3-6`, `lib/blocks.js:15-29`)
- **BLK-02** [in] [verify: unit] The block types are frontmatter, heading, paragraph, list, quote, alert, code, diff, table, image, gallery, html, hr and footnotes. omarky's fold type belongs to changes mode, which is deferred (decision 0006). (`lib/blocks.js:15-29`)
- **BLK-03** [in] [verify: unit] Parsing is CommonMark plus GFM tables and strikethrough, with raw HTML recognised, autolinks on (PAGE-82) and typographer replacements on (PAGE-81). (`lib/markdown.js:7-15`)
- **BLK-04** [in] [verify: unit] Front matter is removed from the body before parsing. Its lines are blanked rather than deleted, so every later block keeps its true source line numbers. (`lib/frontmatter.js:12-29`)
- **BLK-05** [in] [verify: unit] A paragraph whose only content is one image, ignoring soft breaks and whitespace, becomes an image block. (`lib/blocks.js:70-89`)
- **BLK-06** [in] [verify: unit] A heading's slug follows GitHub. It is lowercased, punctuation and symbol ranges are removed, and each space becomes `-`. Repeats get `-1`, `-2` and so on in document order. The first heading with a slug wins the anchor. (`lib/blocks.js:354-358`, `lib/blocks.js:426-430`, `lib/blocks.js:452`)
- **BLK-07** [in] [verify: unit] Anchors resolve heading slugs, footnotes as `fn-N` and citation points as `fnref-N-M`. (`lib/blocks.js:435-452`, `lib/blocks.js:378`)
- **BLK-08** [in] [verify: geometry, golden] Malformed input never blanks the page or squeezes the column away. The cases checked include 24-deep lists, 15-deep quotes, a 3,000-character line and binary junk. (`REPORT.md`, `samples/malformed.md`, `components/blocks/ListBlock.qml:25-28`, `components/blocks/Quote.qml:15-16`)
- **BLK-101** [adapted: 0001] [verify: unit, golden] Quire's own note syntax keeps working on the page. A `[[wiki link]]` (with an optional `|alias`) is a page link in the link role (PAGE-43, PAGE-73), and a link to a note that does not exist is faint. A `#tag` is a chip set like inline code (PAGE-75, PAGE-76) with its text in the accent. A line holding only `![[image]]` embeds becomes an image block, like BLK-05. Clicking a wiki link or a tag does what it does in Quire today. omarky has no notes folder, so it has none of this. (`quire: desktop/src/ui/preview.rs`, `Rich::inline`)

## 2. Paragraph

- **BLK-09** [in] [verify: geometry, golden] A paragraph is set at body size `B`, leading 1.6, in the text colour, left-aligned and ragged right. There is no hyphenation and no justification. (`components/blocks/Paragraph.qml:11-12`, `services/Reading.qml:48`)
- **BLK-10** [in] [verify: geometry, golden] Inside a quote, alert body or footnote, a paragraph takes that container's size and colour tier. (`components/blocks/Paragraph.qml:11-12`, `components/blocks/Quote.qml:32`)

## 3. Headings

- **BLK-11** [in] [verify: unit, geometry] Size, face and leading follow PAGE-14 and PAGE-15. All six levels are kept. (`components/blocks/Heading.qml:14`, `components/blocks/Heading.qml:27-30`)
- **BLK-12** [in] [verify: golden] h1 to h4 are Bold in the strong colour. h5 and h6 are Regular in dim. (`components/blocks/Heading.qml:30-31`)
- **BLK-13** [in] [verify: golden] h6 is set in capitals with letter tracking 0.6 px, fixed. (`components/blocks/Heading.qml:32-33`)
- **BLK-14** [in] [verify: golden] No heading has a rule, underline or background. Hierarchy comes from size, weight, colour and space alone. (`components/blocks/Heading.qml:21-34`)
- **BLK-15** [in] [verify: geometry, script] While the pointer is over a heading, a `#` appears in the left margin. It is mono at round(B × 0.95) [13], its right edge round(B × 0.6) [8] left of the text, vertically centred on the first line, in faint, and in accent while the pointer is on the `#` itself. It fades in and out over 120 ms and changes colour over 120 ms. (`components/blocks/Heading.qml:38-51`)
- **BLK-16** [in] [verify: script] Clicking that `#` copies `<file name>#<slug>`, with the note's file name, and shows the toast "Copied <file name>#<slug>". (`components/blocks/Heading.qml:53`, `components/MarkdownView.qml:112-113`)

## 4. Thematic break

- **BLK-17** [in] [verify: geometry, golden] A break is three dots, never a line. Each dot is max(3, round(B × 0.22)) [3] px across, in faint, spaced round(B × 0.9) [13] px apart, centred in a box round(B × 1.2) [17] px tall across the column. (`components/blocks/Rule.qml:4-24`)

## 5. Lists

Sizes here use `T`, the list's text size: `B` at top level, the container's size inside a quote
or footnote. (`components/blocks/ListBlock.qml:16`)

- **BLK-18** [in] [verify: geometry] The marker gap, from the marker's right edge to the text, is round(T × 0.55) [8]. (`components/blocks/ListBlock.qml:18`)
- **BLK-19** [in] [verify: geometry] The marker width is ceil(T × 0.9) [13] for bullets, the advance of the widest number plus "." for ordered lists, and at least the task box for task lists. (`components/blocks/ListBlock.qml:19-24`)
- **BLK-20** [in] [verify: geometry] A top-level list has no indent. Its markers hang in the left margin and its text sits on the column edge. (`components/blocks/ListBlock.qml:25-28`)
- **BLK-21** [in] [verify: geometry] A nested list (depth 1 to 10), or a top-level list inside a quote, alert or footnote, is indented by max(marker width + marker gap, round(T × 1.5)) [21]. Past depth 10 nesting stops stepping in. (`components/blocks/ListBlock.qml:25-28`)
- **BLK-22** [in] [verify: unit, geometry] Item spacing is round(T × 0.3) [4] for tight lists and round(T × 0.75) [11] for loose lists. A list is loose when any item's paragraphs are not hidden by the parser, which happens when there is a blank line between items. (`components/blocks/ListBlock.qml:31`, `lib/blocks.js:136-145`)
- **BLK-23** [in] [verify: geometry, golden] Bullets cycle by depth mod 3. Depth 0 is a filled dot round(T × 0.3) [4] across, in dim. Depth 1 is a ring round(T × 0.32) [4] across with a border of max(1, round(T / 14)) [1], in faint. Depth 2 is a dash round(T × 0.36) [5] wide and max(1, round(T / 12)) [1] tall, fully rounded, in faint. Each step is lighter. (`components/blocks/ListBlock.qml:72-93`)
- **BLK-24** [in] [verify: geometry] Dots and rings sit round(T × 0.12) [2] in from the marker box's right edge, and the dash round(T × 0.1) [1] in. (`components/blocks/ListBlock.qml:80`, `components/blocks/ListBlock.qml:91`)
- **BLK-25** [in] [verify: geometry] Every marker is vertically centred on the measured x-height centre of the item's first line, not on the line box. (`components/blocks/ListBlock.qml:33-37`, `components/blocks/ListBlock.qml:81`, `components/blocks/ListBlock.qml:92`)
- **BLK-26** [in] [verify: geometry, golden] Ordered markers show the item number and ".", in faint, right-aligned in the marker box, with tabular figures, in the same line box as the item text so the baselines match. (`components/blocks/ListBlock.qml:95-105`)
- **BLK-27** [in] [verify: unit] An ordered list honours its start number. Each item shows its own source number if it has one, else start + position. (`lib/blocks.js:134`, `lib/blocks.js:147`)
- **BLK-28** [in] [verify: unit] A task item is `[ ]`, `[x]` or `[X]` followed by a space at the start of the item. Its marker is a box in place of the bullet. (`lib/blocks.js:105`, `lib/blocks.js:114`)
- **BLK-29** [in] [verify: geometry] The task box is round(T × 0.9) [13] square, with corner radius max(2, round(box × 0.24)) [3], centred on the x-height minus T × 0.04. (`components/blocks/ListBlock.qml:29`, `components/blocks/ListBlock.qml:119-120`)
- **BLK-30** [in] [verify: golden] An open box has a border of max(1, round(T / 12)) [1] px in faint, and in accent while hovered when it can be clicked. A done box is filled accent with no border and a bg-coloured tick through (0.26, 0.53), (0.43, 0.70) and (0.75, 0.32) of the box, with stroke max(1.5, box / 7) [1.86]. (`components/blocks/ListBlock.qml:121-150`)
- **BLK-31** [in] [verify: golden] A done item's text is dim. (`components/blocks/ListBlock.qml:174`)
- **BLK-32** [in] [verify: geometry] The box's hit area extends round(T × 0.5) [7] beyond the box in total. Ticking is specified in `reader.md` from RD-73. (`components/blocks/ListBlock.qml:130-131`)

## 6. Quotes

- **BLK-33** [in] [verify: geometry, golden] A quote has a vertical bar max(2, round(B / 6)) [2] px wide with round ends, full height, in mix(accent, bg, 0.35). (`components/blocks/Quote.qml:13`, `components/blocks/Quote.qml:19-24`)
- **BLK-34** [in] [verify: geometry] Quote content is inset round(B × 1.2) [17] from the bar's left edge, and round(B × 0.5) [7] past nesting depth 8. (`components/blocks/Quote.qml:15-16`)
- **BLK-35** [in] [verify: geometry, golden] Quote text is round(B × 0.94) [13] in dim. Nested content (lists, code) inherits the size and tier. (`components/blocks/Quote.qml:32`)
- **BLK-36** [in] [verify: geometry] A nested quote draws its own bar inside the parent's inset. (`components/blocks/Quote.qml:26-33`)

## 7. Alerts

- **BLK-37** [in] [verify: unit] A quote whose first line is exactly `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]` or `[!CAUTION]`, in any case and with nothing else on that line, becomes an alert titled Note, Tip, Important, Warning or Caution. (`lib/blocks.js:31`, `lib/blocks.js:160-186`)
- **BLK-38** [in] [verify: unit] The tints are blue for note, green for tip, magenta for important, yellow for warning and red for caution, resolved as in PAGE-41. (`components/blocks/Alert.qml:14-15`)
- **BLK-39** [in] [verify: golden] The icons are GitHub's octicons for each kind, info, light-bulb, report, alert and stop (Nerd Font codepoints U+F449, U+F400, U+F50A, U+F421 and U+F52F in omarky). (`components/blocks/Alert.qml:17-18`)
- **BLK-40** [in] [verify: geometry, golden] The alert is a card with padding round(B × 1.05) [15], corner radius 4 px (PAGE-48), fill alpha(tint, light 0.075 / dark 0.09) and a 1 px border alpha(tint, light 0.3 / dark 0.24). (`components/blocks/Alert.qml:19-25`)
- **BLK-41** [in] [verify: geometry] The title row is the icon at round(B × 0.95) [13], then the title in mono Bold at round(B × 0.76) [11], in capitals, with letter spacing round(B × 0.08) [1], in the tint. Icon to title and title row to body are both round(B × 0.5) [7]. (`components/blocks/Alert.qml:32-52`)
- **BLK-42** [in] [verify: geometry, golden] Alert body text is body size in the text tier, not dim. Lists inside an alert are indented rather than hung, since the card has no margin to hang in. (`components/blocks/Alert.qml:55-59`, `components/blocks/ListBlock.qml:25-28`)

## 8. Code blocks

- **BLK-43** [in] [verify: geometry] Code is mono at round(B × 0.84) [12] with line height round(size × 1.5) [18]. (`components/blocks/CodeBlock.qml:17`, `services/Reading.qml:58-59`)
- **BLK-44** [in] [verify: geometry] Padding is round(B × 1.1) [15] left and right, round(B × 1.25) [18] on top (room for the badge) and round(B × 0.85) [12] at the bottom. (`components/blocks/CodeBlock.qml:18-22`, `components/blocks/CodeBlock.qml:47`)
- **BLK-45** [in] [verify: golden] The fill is panel, with a 1 px hairlineSoft border and corner radius 4 px (PAGE-48). (`components/blocks/CodeBlock.qml:48-51`)
- **BLK-46** [in] [verify: geometry] Line numbers appear only when the block has more than 8 lines. They are ghost, tabular and right-aligned, in a gutter as wide as the largest number plus round(size × 1.4) [17]. (`components/blocks/CodeBlock.qml:23-24`, `components/blocks/CodeBlock.qml:64-73`)
- **BLK-47** [in] [verify: script] Lines never wrap. A long line scrolls sideways with Shift+wheel or a sideways swipe, by half the wheel's angle delta per event, which is 60 px per wheel notch. (`components/blocks/CodeBlock.qml:107-116`)
- **BLK-48** [in] [verify: golden] While there is more to the right, the right edge fades over round(B × 2.4) [34] px from transparent to the panel colour. (`components/blocks/CodeBlock.qml:118-129`)
- **BLK-49** [in] [verify: geometry] Tab stops are 4 average mono characters. (`components/blocks/CodeBlock.qml:101`)
- **BLK-50** [in] [verify: geometry, golden] The language badge shows the fence language in lower case, mono at 10 px (fixed), with letter spacing 0.4, in faint, top right inside the padding. There is no badge when there is no language. (`components/blocks/CodeBlock.qml:137-159`)
- **BLK-51** [in] [verify: script] While the pointer is over the block, the badge fades out over 120 ms and a "Copy" chip fades in over 140 ms in its place. Clicking it copies the code text exactly, shows "Copied" with a check icon in green for 1400 ms, and shows the toast "Copied". (`components/blocks/CodeBlock.qml:158-179`, `components/MarkdownView.qml:107-109`)
- **BLK-52** [in] [verify: script] When a find match is in a long line past the visible part, the block scrolls sideways to put the match a third of the way in. (`components/blocks/CodeBlock.qml:34-43`)

## 9. Diff blocks

- **BLK-53** [in] [verify: unit] A fence with language diff, patch or udiff, in any case, renders as a diff block. (`lib/blocks.js:220-222`)
- **BLK-54** [in] [verify: unit] The line kinds are meta (`diff `, `index `, `--- `, `+++ `, `new file`, `deleted file`, `similarity`, `rename `, `old mode`, `new mode`, `Binary files`), hunk (`@@`), add (`+`), del (`-`) and context. Meta and hunk lines show no sign. Add shows "+", and del shows the minus sign U+2212. (`lib/blocks.js:190-208`)
- **BLK-55** [in] [verify: geometry] A header band round(B × 2.1) [29] tall shows "+N" in green and "-M" (with U+2212) in red, in mono Bold at 10 px (fixed), spaced round(B × 0.6) [8], with the language badge and copy chip on the right. A hairlineSoft line separates the header from the body. (`components/blocks/DiffBlock.qml:20`, `components/blocks/DiffBlock.qml:84-138`)
- **BLK-56** [in] [verify: golden] Full-width line bands are alpha(green, 0.14) for adds, alpha(red, 0.14) for dels and alpha(blue, 0.07) for hunks. (`components/blocks/DiffBlock.qml:150-152`)
- **BLK-57** [in] [verify: golden] Line text is mix(text, green, 0.3) for adds, mix(text, red, 0.3) for dels, blue for hunks, faint for meta and text for context. (`components/blocks/DiffBlock.qml:58-67`)
- **BLK-58** [in] [verify: geometry, script] The sign column is round(size × 1.6) [19] wide. Padding is round(B × 1.1) [15] sideways and round(B × 0.7) [10] vertically. Lines do not wrap and scroll sideways like code. (`components/blocks/DiffBlock.qml:18-21`, `components/blocks/DiffBlock.qml:205`)

## 10. Tables

- **BLK-59** [in] [verify: geometry, golden] Cell text is round(B × 0.94) [13] with leading 1.45. Header text is mono Bold round(B × 0.72) [10], in capitals, with tracking 0.8, in dim, leading 1.35, on a raised band with a hairline below. (`components/blocks/TableBlock.qml:18-19`, `components/blocks/TableBlock.qml:116-145`, `components/blocks/TableBlock.qml:176`)
- **BLK-60** [in] [verify: geometry] Cell padding is round(B × 0.85) [12] sideways and round(B × 0.55) [8] vertically. (`components/blocks/TableBlock.qml:20-21`)
- **BLK-61** [in] [verify: geometry] The table is as wide as its columns need, up to the column width. It is not stretched. (`components/blocks/TableBlock.qml:81`, `components/blocks/TableBlock.qml:90`)
- **BLK-62** [in] [verify: golden] The frame is a 1 px hairline border with corner radius 4 px (PAGE-48). Rows are separated by hairlineSoft lines. Odd body rows get a zebra wash alpha(fg, light 0.022 / dark 0.018). (`components/blocks/TableBlock.qml:92-95`, `components/blocks/TableBlock.qml:157`, `components/blocks/TableBlock.qml:188`)
- **BLK-63** [in] [verify: unit] A column's natural width is its widest cell (or its uppercased header), capped at 34 average characters, plus padding. (`components/blocks/TableBlock.qml:22`, `components/blocks/TableBlock.qml:36-45`)
- **BLK-64** [in] [verify: unit] A column's minimum is the larger of its longest header word and 10 average characters, plus padding, and never more than its natural width. A word wider than its column, such as a URL, breaks inside the word, as Qt's default wrap does, rather than running into the next column. (`components/blocks/TableBlock.qml:46-53`)
- **BLK-65** [in] [verify: unit, geometry] If the natural widths fit, they are used. Else, if the minimums fit, each column gives up width in proportion to its slack above its minimum, so prose wraps in place. Else the table scrolls sideways, with each column capped at 24 average characters and at least its minimum. (`components/blocks/TableBlock.qml:54-66`)
- **BLK-66** [in] [verify: unit] Alignment follows the separator row. A column with no alignment whose non-empty cells are all numeric is right-aligned with tabular figures. (`components/blocks/TableBlock.qml:24-28`, `components/blocks/TableBlock.qml:178`)
- **BLK-67** [in] [verify: unit] Numeric means an optional sign (+, -, U+2212), an optional currency sign ($, €, £, ¥), digits with commas, optional decimals, and an optional suffix of %, a time like :30, k, K, m, M, b, B, x, ms, s or h. (`lib/blocks.js:227`, `lib/blocks.js:256-265`)
- **BLK-68** [in] [verify: unit] Rows with fewer cells than the header are padded with empty cells. Extra cells are dropped. (`lib/blocks.js:250-255`)
- **BLK-69** [in] [verify: golden] When the table scrolls sideways, the left edge fades over round(B × 2.2) [31] and the right over round(B × 2.8) [39], each only while there is more in that direction. (`components/blocks/TableBlock.qml:200-222`)

## 11. Images

- **BLK-70** [in] [verify: geometry] An image shows at its natural size, capped at the column width, never upscaled, centred, with its aspect kept and its corners masked to radius 4 px (PAGE-48). (`components/blocks/ImageBlock.qml:28-29`, `components/blocks/ImageBlock.qml:36-50`)
- **BLK-71** [quire] [verify: -] Quire's image loading covers this (`quire: desktop/src/images.rs`). Web images download, `![[name]]` embeds are found in the notes folder, and a relative path resolves against the note's folder, then the notes folder. omarky: relative paths resolve against the document's folder, and a local file is checked for readability before loading. (`components/MarkdownView.qml:97-105`, `components/blocks/ImageBlock.qml:86-90`)
- **BLK-72** [in] [verify: geometry, golden] Until an image loads, or when it is missing, a placeholder card shows. It is round(B × 3.4) [48] tall and as wide as its content plus 2.4 em (at most the column), with card fill, a hairline border, a faint image icon at round(B × 1.05) [15] and mono round(B × 0.76) [11] faint text "Loading image…", "Image not found · <src>" or "No image". (`components/blocks/ImageBlock.qml:56-82`)
- **BLK-73** [in] [verify: geometry] The alt text is the caption, centred under the image, in the prose Italic at round(B × 0.84) [12] in dim, round(B × 0.6) [8] below the image. There is no caption when the alt text is empty. (`components/blocks/ImageBlock.qml:24`, `components/blocks/ImageBlock.qml:93-104`)

## 12. Raw HTML and galleries

- **BLK-74** [in] [verify: unit] An HTML block made only of comments is dropped. An unclosed `<!--` is shown, so it cannot swallow the rest of the file. (`lib/blocks.js:313-317`, `docs/decisions/0002-raw-html-recognised-never-rendered.md:8-10`)
- **BLK-75** [in] [verify: unit] HTML made only of `<img>` tags, optionally wrapped in `p`, `div`, `center` and `a`, and spaced by `br` or `&nbsp;`, becomes a gallery. Any other tag or text makes it literal HTML. (`lib/blocks.js:286-311`)
- **BLK-76** [in] [verify: geometry, golden] Other HTML blocks show literally, with a 1 px hairline rule on the left, text inset round(B × 0.9) [13], in mono round(B × 0.8) [11], faint, wrapping anywhere. (`components/blocks/HtmlBlock.qml:13-29`)
- **BLK-77** [in] [verify: unit, geometry] Gallery image sizes come from the tag's CSS or attribute width or height, in CSS px scaled by B / 16. With neither, the height is the natural height capped at round(B × 16) [224], or round(B × 10) [140] while unknown. An unknown aspect is 0.75. Any image wider than the column scales down to it. (`components/blocks/GalleryBlock.qml:18-34`)
- **BLK-78** [in] [verify: geometry] Gallery rows fill greedily with gaps of round(B × 0.5) [7]. Each row is centred, and the images in a row are bottom-aligned. Rows are separated by the same gap. (`components/blocks/GalleryBlock.qml:37-66`)
- **BLK-79** [in] [verify: script] A gallery image shows its alt or title as a tooltip. A missing one shows a card with "Image not found · <src>". (`components/blocks/GalleryBlock.qml:114-132`)

## 13. Footnotes

- **BLK-80** [in] [verify: unit] All footnote definitions gather into one footnotes block at the end of the note, numbered by order of first citation. Unreferenced notes follow in definition order. (`lib/blocks.js:362-382`, `docs/decisions/0004-own-linkify-and-footnotes.md:10-13`)
- **BLK-81** [in] [verify: geometry] The block starts with a box round(B × 0.6) [8] tall holding a hairline rule 30% of the column wide. (`components/blocks/Footnotes.qml:35-39`)
- **BLK-82** [in] [verify: geometry] Each note is a number column round(B × 1.9) [27] wide, holding "N." in mono at round(text × 0.9) [11] in the link colour with tabular figures, then the note body at round(B × 0.88) [12] in dim. Notes are round(B × 0.55) [8] apart. (`components/blocks/Footnotes.qml:14-16`, `components/blocks/Footnotes.qml:50-71`)
- **BLK-83** [in] [verify: script] Clicking a note's number jumps to its first citation (`fnref-N-1`). (`components/blocks/Footnotes.qml:61-62`)
- **BLK-84** [in] [verify: unit, script] Back-links " ↩" are appended to the note's last paragraph, one per citation, numbered " ↩1", " ↩2" only when the note is cited more than once. Each jumps to its citation. (`components/blocks/Footnotes.qml:18-33`)

## 14. Front matter

- **BLK-85** [in] [verify: unit] Front matter is either fenced (`---` on line 1, closed by `---` or `...`) or bare (two or more `key: value` lines from line 1, ended by the first blank line). Keys are letters, digits, `_` and `-`. A one-line `Note: …` stays prose. (`lib/frontmatter.js:3-17`)
- **BLK-86** [in] [verify: unit] Values support scalars, quoted strings, flow lists, `- item` lists and `>` or `|` block scalars. Anything else is kept as raw text. (`lib/frontmatter.js:13-15`)
- **BLK-87** [in] [verify: unit] A `title` field becomes the card's title and is not listed as a row. (`lib/blocks.js:395-407`)
- **BLK-88** [in] [verify: unit] Each other field's kind is list (if a list), empty (if blank), link (if http, https or mailto), pill (if 32 characters or fewer and 4 words or fewer), else text. (`lib/blocks.js:399-404`)
- **BLK-89** [in] [verify: geometry] The card has padding round(B × 1.2) [17], card fill, a 1 px hairline border and corner radius 6 px (PAGE-48). (`components/blocks/FrontMatter.qml:15`, `components/blocks/FrontMatter.qml:34-37`)
- **BLK-90** [in] [verify: geometry, golden] A "FRONT MATTER" label sits top right in mono round(B × 0.66) [9], in capitals, with tracking 0.8, in ghost. (`components/blocks/FrontMatter.qml:43-52`)
- **BLK-91** [in] [verify: geometry] The title is in the heading face at size(1.5) [20], in strong, leading 1.3, with round(B × 0.6) [8] below it. (`components/blocks/FrontMatter.qml:60-71`)
- **BLK-92** [in] [verify: geometry] Keys are mono round(B × 0.74) [10], in capitals, with tracking 0.8, in faint, in a column as wide as the widest key (capped at 9 em) plus round(B × 1.2) [17]. (`components/blocks/FrontMatter.qml:16-21`, `components/blocks/FrontMatter.qml:88-96`)
- **BLK-93** [in] [verify: geometry] Each row is round(B × 1.55) [22] tall. Rows are round(B × 0.55) [8] apart. (`components/blocks/FrontMatter.qml:17`, `components/blocks/FrontMatter.qml:85`)
- **BLK-94** [in] [verify: unit] Pill values use the pill style (BLK-99). A `status` or `state` value is toned green when it matches accept, approv, done, complete, shipped, merged, final, publish, active, ready, resolved or live. It is yellow for draft, propos, wip, progress, pending, review, open, todo, planned or exploring. It is red for reject, deprecat, supersed, block, abandon, closed, cancel, obsolete or failed. (`components/blocks/FrontMatter.qml:24-29`, `components/blocks/FrontMatter.qml:121`)
- **BLK-95** [in] [verify: golden] A value that is a date (YYYY-MM-DD, optionally with a time) gets a calendar icon in its pill. (`components/blocks/FrontMatter.qml:31`)
- **BLK-96** [in] [verify: geometry] A list value shows as a row of neutral pills spaced round(B × 0.35) [5], wrapping. (`components/blocks/FrontMatter.qml:128-135`)
- **BLK-97** [in] [verify: script] A link value is prose round(B × 0.9) [13] in the link colour, underlined on hover, and opens like a page link. (`components/blocks/FrontMatter.qml:141-160`)
- **BLK-98** [in] [verify: golden] An empty value shows an em dash (U+2014) in ghost. A text value is prose round(B × 0.9) [13] at leading 1.5. (`components/blocks/FrontMatter.qml:163-184`)

## 15. Shared small parts

- **BLK-99** [in] [verify: geometry, golden] A pill is mono at round(B × 0.76) [11], 2 × size [22] tall, fully rounded, and as wide as its content plus round(size × 1.3). Neutral pills have raised fill, a hairline border, dim text and a faint icon. Toned pills have fill alpha(tone, light 0.12 / dark 0.14), border alpha(tone, light 0.4 / dark 0.35), text mix(tone, text, light 0.25 / dark 0.1) and the icon in the tone. (`components/Pill.qml:5-39`)
- **BLK-100** [in] [verify: geometry, script] A chip button is 2 × its size tall, and as wide as its content plus round(size × 1.3) with a label or round(size × 1.1) without. Its corner radius is 3 px, fixed (omarky's max(3, min(shell rounding, height / 3)) with no shell rounding). It is transparent at rest. On hover or when active it gets the hover fill and a 1 px hairline border, and its icon and label turn from their tint to text. Colour changes take 120 ms. (`components/ChipButton.qml:13-37`)

## Open questions

- Styling of wiki links and tags. omarky has neither, so BLK-101 places them in omarky's roles.
  Today Quire underlines every wiki link and draws missing ones in its muted tone. The recommended
  default is to follow BLK-101 (no underline until hover, faint for a missing note), so wiki links
  read like every other page link.
- Sideways scroll calibration. BLK-47 copies omarky's "half the angle delta". That number depends
  on what unit the windowing layer delivers, and Rune's winit already scales touchpad
  deltas by 12, as Chromium does. The recommended default is to calibrate sideways swipes with the same measurement as the
  reader's vertical scrolling (`reader.md`, RD-20 to RD-30), and keep 60 px per wheel notch.
- Tables need a renderer. Quire's parser has no tables today, and core Rune has no table layout.
  The recommended default is to lay out cells from Rune's `text::content_widths` with BLK-63 to BLK-65,
  rather than adopting `rune-kit`'s table, whose column rules differ from omarky's.
