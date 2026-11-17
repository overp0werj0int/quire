# Page and typography

This file covers the reader page in preview mode: fonts, sizes, the column, colour roles, syntax
colours, vertical rhythm, inline text and motion. Blocks are in `blocks.md`. The format of the
items is in `README.md`.

Units and conventions:

- `B` is the body size in px. It is the size setting (PAGE-86), 14 by default. Every `em` value
  multiplies `B`.
- A px value is a logical pixel, a Rune point at scale 1.
- Worked values in brackets are for `B` = 14. They are computed, not measured, and they scale
  with `B`. A value marked fixed does not scale.
- `S(n)` is a chrome length of `n` px, fixed. Quire has no UI base size, so it is omarky's
  default.
- `round()` is round half up, as JavaScript's `Math.round`, applied exactly where omarky
  applies it.
- Colour role names (text, strong, dim, faint, ghost, accent, link, hairline and so on) are
  defined in section 4.
- Citations in parentheses point into omarky 0.7.0 (`/home/p0werj0int/Projects/omarky`),
  unless they start with `quire:`.

## 1. Fonts

- **PAGE-01** [adapted: 0002] [verify: state] Prose (paragraphs, lists, tables, quotes, captions) uses iA Writer Quattro S Regular by default, loaded from the file Quire bundles. Settings can choose another prose font (PAGE-87). omarky: the installed iA Writer Quattro S, Regular style. (`services/Reading.qml:27`)
- **PAGE-02** [adapted: 0002] [verify: unit, golden] Each face is loaded from its own file, and runs are mapped to files: plain text to Regular, bold to Bold, italic to Italic, bold italic to Bold Italic. A face is never matched by weight, because the Quattro S Bold file declares the regular weight. A test renders a plain paragraph and checks that it used the Regular file. omarky: asks for the face by its style name "Regular", never by weight, and inline styles resolve to the real faces. (`docs/decisions/0003-quattro-s-regular-style-name.md:1-8`)
- **PAGE-03** [in] [verify: unit] Headings h1 to h4 use iA Writer Quattro S Bold. h5 and h6 use the Regular face of the heading family. (`services/Reading.qml:28`, `components/blocks/Heading.qml:30`)
- **PAGE-04** [adapted: 0002] [verify: state] Code, diffs, inline code, raw HTML, alert titles, table headers, code badges and front-matter keys use iA Writer Mono S by default, loaded from the file Quire bundles. Settings can choose another code font (PAGE-88). The reason is that Mono S is lighter than a programmer's font and has no ligatures, so `->` and `===` show as typed. omarky: the installed iA Writer Mono S. (`services/Reading.qml:29`, `docs/decisions/0005-heading-font.md:5-6`, `docs/decisions/0007-code-font.md:1-9`)
- **PAGE-05** [in] [verify: golden] Icons on the page are GitHub octicons. They are never set as text, and whatever supplies them is never used for page text. omarky draws them from JetBrainsMono Nerd Font glyphs. (`docs/decisions/0007-code-font.md:9`, `components/blocks/ImageBlock.qml:70`)
- **PAGE-06** [adapted: 0002] [verify: state] Font choices are Settings choices (PAGE-87, PAGE-88), not environment variables. omarky: `OMARKY_PROSE_FONT` for prose, `OMARKY_HEADING_FONT` as `Family/Style` for headings, `OMARKY_MONO_FONT` (or the older `OMARKY_CODE_FONT`) for mono. (`services/Reading.qml:20-22`, `services/Reading.qml:36-41`)
- **PAGE-07** [adapted: 0002] [verify: unit] A stored font id that Quire does not know, from an old or hand-edited `state.json`, falls back to the default font of that slot. omarky: an override naming a family that is not installed falls back to the default family, and family names match case-insensitively. (`services/Reading.qml:36`, `lib/fonts.js`, `test/tst_fonts.qml`)
- **PAGE-08** [dropped] [verify: -] The fonts are bundled, so there is no installed-family list to read at start or refresh later. omarky: reads the installed families once and refreshes them after a font is installed. (`services/Reading.qml:31-34`)
- **PAGE-09** [quire] [verify: -] Chrome outside the page sets its text in two families, through the named styles in `quire: desktop/src/ui/styles.rs`. The panel family (fontconfig's `monospace`, decision 0008) is for navigation: the side panels, their captions and footers, and the menus. Key names and literal values (paths, addresses, tokens, configuration) take it wherever they appear. Tooltips take it too, so a name and its key are one line in one family (`styles::tooltip`). Instrument Sans is for the rest: the palette, the quick switcher, Settings, the lock dialog, the help sheet, the edit sheet, toasts, the title bar, the find bar, the welcome screen and empty states. Every section label (the panels' captions, Open, the Settings menu's title, the shortcut list's groups, the help sheet's groups, Recent on the welcome screen) is one caption style: bold capitals 1 px apart at 10 pt in both families, in its role's family, in the panels' faint ink, which reads at 4.5:1 or more on the window and the sidebar in every theme (`styles::caption`). Where an item in these files says "UI family", it means the family of that text's role here. A test fails on any other family in `ui/` outside the page's own builders. omarky: the shell UI family, default `monospace`, at fixed sizes caption 10, bodySmall 11, body 12, subtitle 13, title 14, heading 16, display 24 px. (`Commons/Style.qml:269`, `Commons/Style.qml:322-332`)
- **PAGE-87** [adapted: 0002] [verify: state, geometry] Settings, Editor offers the prose font: iA Writer Quattro S (the default), Newsreader and Instrument Sans. Every ratio in this file applies unchanged to the chosen font, and the measure is recomputed from that font's own average character width (PAGE-22). Headings use the chosen font's Bold. A variable font file gives Regular at weight 400 and Bold at weight 700 from the same file. omarky has no font setting, only the environment overrides of PAGE-06. (`services/Reading.qml:20-22`)
- **PAGE-88** [adapted: 0002] [verify: state, geometry] Settings, Editor offers the code font: iA Writer Mono S (the default), IBM Plex Mono, Courier Prime and Spline Sans Mono. The code ratios (0.84 and 0.86 of the body, leading 1.5, tab stops of 4 average characters) apply to the chosen font, measured from its own metrics. Ligatures are off for every code font. omarky has no font setting, only `OMARKY_MONO_FONT`. (`services/Reading.qml:22`)

## 2. Sizes and the type scale

- **PAGE-10** [adapted: 0002] [verify: state] The body size `B` is the size setting (PAGE-86), 14 by default. omarky: `B` = round(UI body 12 × 1.2 × zoom), 14 at zoom 1. (`services/Reading.qml:47`)
- **PAGE-11** [in] [verify: geometry] Body leading is 1.6. Every text line sits in a fixed line box of round(size × leading) px [22 for body]. (`services/Reading.qml:48`, `components/RichText.qml:23`)
- **PAGE-12** [in] [verify: geometry] The size setting changes only reading text. Chrome inside the page (code badges, copy chips, diff counts) stays at fixed sizes, `chrome` 12 px and `chromeSmall` 10 px. (`services/Reading.qml:49-52`)
- **PAGE-13** [in] [verify: unit] The modular scale ratio is 1.25. `size(step)` = round(B × 1.25^step). (`services/Reading.qml:67`, `services/Reading.qml:125`)
- **PAGE-14** [in] [verify: unit, geometry] Heading steps are h1 3, h2 2, h3 1, h4 0.5, h5 0, h6 0. That gives h1 27, h2 22, h3 18, h4 16, h5 14 and h6 14 px at `B` 14. (`components/blocks/Heading.qml:14`, `components/blocks/Heading.qml:27`)
- **PAGE-15** [in] [verify: unit, geometry] Heading leading is 1.25 for h1 and h2, and 1.35 for h3 to h6 [line boxes 34, 28, 24, 22, 19, 19 px]. (`components/blocks/Heading.qml:28`)
- **PAGE-16** [in] [verify: unit, geometry] Code block size is round(B × 0.84) [12] with leading 1.5 [line 18 px]. (`services/Reading.qml:58-59`, `components/blocks/CodeBlock.qml:17`)
- **PAGE-17** [in] [verify: unit] Inline code size is round(B × 0.86) [12]. (`services/Reading.qml:80`)
- **PAGE-18** [in] [verify: unit] Other text sizes on the page, all multiples of `B`, are quote text 0.94 [13], table cells 0.94 [13], table header 0.72 [10], footnote text 0.88 [12], image caption 0.84 [12], raw HTML 0.8 [11], alert title 0.76 [11], pill 0.76 [11], front-matter keys 0.74 [10], front-matter label 0.66 [9], front-matter values 0.9 [13] and front-matter title size(1.5) [20]. (`components/blocks/Quote.qml:32`, `components/blocks/TableBlock.qml:18-19`, `components/blocks/Footnotes.qml:14`, `components/blocks/ImageBlock.qml:104`, `components/blocks/HtmlBlock.qml:29`, `components/blocks/Alert.qml:48`, `components/Pill.qml:13`, `components/blocks/FrontMatter.qml:16`, `components/blocks/FrontMatter.qml:50`, `components/blocks/FrontMatter.qml:153`, `components/blocks/FrontMatter.qml:66`)
- **PAGE-19** [adapted: 0002] [verify: unit, script] The zoom keys change the size setting. In the omarky preset `Ctrl+=` (and `Ctrl+Plus`) steps up, `Ctrl+-` steps down and `Ctrl+0` resets to 14. The steps follow omarky's zoom ladder, round(14.4 × z) for z from 0.6 to 2.2 in steps of 0.1, which is 9, 10, 12, 13, 14, 16, 17, 19, 20, 22, 23, 24, 26, 27, 29, 30 and 32. A step moves to the next ladder value above or below the current size, so a size set in Settings off the ladder joins it on the next key. The keys of both presets are in `keymap.md`. omarky: zoom is clamped to 0.6 to 2.2 and rounded to 0.01, `Ctrl+Plus` or `Ctrl+=` adds 0.1, `Ctrl+Minus` subtracts 0.1, `Ctrl+0` resets to 1.0. (`services/Prefs.qml:23`, `reader/ui/app/ReaderWindow.qml:418-423`)
- **PAGE-20** [adapted: 0002] [verify: state] The size is saved in Quire's `state.json` with the other settings. A stored size outside 9 to 32 is clamped on load. Test runs use a temporary home folder, so they never write the user's settings. omarky: zoom persists in the reader's state file, a stored value outside 0.6 to 2.2 is clamped on load, and an unreadable file is ignored with one warning. (`services/Prefs.qml:56-64`)
- **PAGE-86** [adapted: 0002] [verify: state, script] Settings, Editor keeps a size control for the body size, in whole px from 9 to 32, 14 by default. The control and the zoom keys change the same value, and the page reflows at once. omarky has no size control, only zoom. (`services/Prefs.qml:23`)

## 3. The column (measure, margins, lean)

- **PAGE-21** [in] [verify: unit, geometry] The measure is round(average character width of the prose font at `B` × 68) in reading width, and × 100 in wide width. The average character width is the font's OS/2 `xAvgCharWidth`, read from the bundled file. With Quattro S (608/1000 em) that is 579 px and 851 px at `B` 14. (`components/DocView.qml:19`)
- **PAGE-22** [adapted: 0002] [verify: geometry] The measure follows the prose font chosen in Settings and the size setting, so it is always 68 (or 100) of that font's average characters at any size. omarky: it follows the prose font override and the zoom. (`components/DocView.qml:19`, `services/Reading.qml:20`)
- **PAGE-23** [in] [verify: geometry] Side room = max(S(24), min(S(56), view width × 0.06)). The view is the page area between Quire's panels. (`components/DocView.qml:20`)
- **PAGE-24** [in] [verify: geometry] Column width = max(S(200), min(measure, view width - 2 × side room)). (`components/DocView.qml:21`)
- **PAGE-25** [adapted: 0007] [verify: unit, geometry] The left gutter (where the column starts in the view) is computed from free = view width - column. limit = free/2 - free/2.618. shift = sign(lean) × min(|lean| / 2, limit). gutter = round(max(side room, min(free - side room, free/2 - shift))). A positive shift moves the column left and a negative one moves it right. omarky: shift = min(max(0, lean) / 2, limit) and gutter = round(max(side room, free/2 - shift)), the same result when only a left panel is open. (`components/DocView.qml:27-31`)
- **PAGE-26** [adapted: 0007] [verify: geometry] `lean` is the width of the left panel (the file tree) minus the width of the right panel (the outline), each 0 while hidden. The column leans back toward the window's centre by up to half of `lean`, but never past the golden-section point of the free space on the side it moves toward. omarky's outline key toggles Quire's outline wherever it is. omarky: `lean` is the width of the outline on the left, the only side panel. (`components/DocView.qml:23-26`, `reader/ui/app/ReaderWindow.qml:584`)
- **PAGE-27** [adapted: 0007] [verify: geometry] With no panels, or panels of equal width, the column is exactly centred in the window (lean 0, shift 0). omarky: centred with no outline. (`components/DocView.qml:29`)
- **PAGE-28** [in] [verify: geometry] The page has S(44) of empty room above the first block. (`components/DocView.qml:32`, `components/DocView.qml:72`)
- **PAGE-29** [in] [verify: geometry] The page has max(S(120), view height × 0.3) of empty room after the last block, so the last line can be read well above the bottom edge. (`components/DocView.qml:73`)
- **PAGE-30** [in] [verify: geometry, golden] Markers hang left of the column, so text keeps one straight left edge. This covers top-level bullets, numbers and task boxes, and the heading `#` on hover. Change tags and change washes hang too, and belong to changes mode, which is deferred. (`README.md:13`, `components/blocks/ListBlock.qml:25-28`, `components/blocks/Heading.qml:41`)
- **PAGE-31** [in] [verify: geometry] Content never shifts sideways between blocks. Every top-level block starts at the gutter and has the column width. (`components/DocView.qml:353-355`)

## 4. Colour roles

Every role derives from the active Quire palette (decision 0005), not from Omarchy's
`colors.toml`. `fg` is the palette's ink, `bg` its background, and `accent` its accent after
the accent picker (PAGE-89). Under the Omarchy theme option the palette comes from the current
Omarchy theme, as it does in Quire today. Quire's own chrome keeps its current colours.

`mix(a, b, t)` = a + (b - a) × t per channel. `alpha(c, a)` is `c` at opacity `a` over
whatever is below. `flat(c, a)` = mix(bg, c, a), an opaque colour.

- **PAGE-32** [adapted: 0005] [verify: unit] Light or dark is the active palette's flag. A built-in theme declares it. The Omarchy theme option uses the theme's `mode`, else Quire's own luminance test (`quire: desktop/src/theme.rs`, `omarchy_pal`). Every light and dark pair below follows this flag. omarky: the theme's declared `mode`, else light when background luminance (0.2126 R + 0.7152 G + 0.0722 B) > 0.5. (`services/Theme.qml:24`, `services/Theme.qml:69-71`)
- **PAGE-33** [adapted: 0005] [verify: unit] text = fg. strong = the Omarchy theme's `bright_foreground` when the Omarchy option is active and the theme sets it, else fg. omarky: `bright_foreground` from the Omarchy theme, else its fg. (`services/Theme.qml:44-45`)
- **PAGE-34** [adapted: 0005] [verify: unit] dim = mix(fg, bg, 0.34). faint = mix(fg, bg, 0.58). ghost = mix(fg, bg, 0.8). These replace Quire's current second ink, muted and mark tones on the page. omarky: the same mixes of the Omarchy theme's fg and bg. (`services/Theme.qml:46-48`)
- **PAGE-35** [adapted: 0005] [verify: unit] raised = alpha(fg, light 0.045 / dark 0.04). sunken = alpha(light ? fg : black, light 0.035 / dark 0.22). omarky: the same factors over the Omarchy theme's fg. (`services/Theme.qml:52-53`)
- **PAGE-36** [adapted: 0005] [verify: unit] panel (the code and diff fill) = the Omarchy theme's `dark_background` when the Omarchy option is active and the theme sets it, else bg darkened as Qt's `darker` does, with the HSV value divided by 1.04 (light) or 1.2 (dark). omarky: the Omarchy theme's `dark_background`, else its bg darkened by the same factors. (`services/Theme.qml:56`)
- **PAGE-37** [adapted: 0005] [verify: unit] card = alpha(fg, 0.035). hover = alpha(fg, 0.06). omarky: the same over the Omarchy theme's fg. (`services/Theme.qml:57`, `services/Theme.qml:63`)
- **PAGE-38** [adapted: 0005] [verify: unit] hairline = alpha(fg, light 0.14 / dark 0.12). hairlineSoft = alpha(fg, light 0.09 / dark 0.075). omarky: the same over the Omarchy theme's fg. (`services/Theme.qml:58-59`)
- **PAGE-39** [adapted: 0005] [verify: unit, golden] selection = alpha(accent, light 0.22 / dark 0.3), and selected text keeps the text colour. omarky: the same over the Omarchy theme's accent. (`services/Theme.qml:60`, `components/RichText.qml:59-60`)
- **PAGE-40** [adapted: 0005] [verify: unit] chipFill = alpha(fg, light 0.07 / dark 0.085). chipEdge = alpha(fg, light 0.07 / dark 0.06). omarky: the same over the Omarchy theme's fg. (`services/Theme.qml:61-62`)
- **PAGE-41** [adapted: 0005] [verify: unit] The named colours green, red, yellow, blue, magenta and cyan resolve in this order. Under the Omarchy option, the theme's named key, else its numbered colour (`color2` for green, `color1` for red, `color3` for yellow, `color4` for blue, `color5` for magenta, `color6` for cyan). Under a built-in theme, the theme's own named colour where it defines one. Otherwise the accent, except red, which falls back to the palette's error colour. omarky: the Omarchy theme's named key, else its numbered colour, else the accent, with red falling back to the urgent colour. (`services/Theme.qml:32-37`)
- **PAGE-42** [adapted: 0005] [verify: unit] orange = the Omarchy theme's `orange`, else its `yellow`, else its `color3`. Under a built-in theme, its own orange, else its own yellow. Otherwise the palette's error colour. omarky: theme orange, else yellow, else `color3`, else urgent. (`services/Theme.qml:38`)
- **PAGE-43** [adapted: 0005] [verify: unit] link = accent, unless the accent is within RGB Euclidean distance 24 of fg, then blue (PAGE-41). This keeps links visible in themes whose accent equals their text colour. A picked accent is tested the same way. omarky: the same test on the Omarchy theme's accent. (`lib/palette.js:5-27`, `services/Theme.qml:40`)
- **PAGE-44** [adapted: 0005] [verify: unit] Inline-code text = mix(text, accent, 0.12). Inline-code background = flat(fg, light 0.07 / dark 0.085). omarky: the same over the Omarchy theme. (`services/Reading.qml:78-79`)
- **PAGE-45** [adapted: 0005] [verify: unit, golden] Find match background = flat(accent, light 0.22 / dark 0.3). The current match is solid accent with bg-coloured text. omarky: the same over the Omarchy theme. (`services/Reading.qml:87-89`, `docs/decisions/0010-window-behaviour-choices.md:7`)
- **PAGE-46** [adapted: 0005] [verify: golden] The page background is the palette's background. The find bar, help sheet and toasts are Quire chrome and use Quire's popup surfaces. omarky: the window is the theme bg, and popups use the shell's popup background, equal to the theme bg. (`reader/ui/app/Header.qml:58`, `components/FindBar.qml:21`)
- **PAGE-47** [adapted: 0005] [verify: state] Colours are live. Switching Quire's theme, light or dark mode, or accent, or the Omarchy theme while the Omarchy option is active, recolours the open page, code colours included, without a reload and without moving the scroll position. omarky: reads the Omarchy theme live, and `OMARKY_THEME_DIR` points at another theme directory. (`services/Theme.qml:16-23`, `README.md:15`, `reader/tools/check-theme`)
- **PAGE-48** [in] [verify: geometry] Corner radius on page surfaces is 4 px, fixed, for code, diff, alert, table, image and the find bar, and 6 px, fixed, for front matter, washes, the drop overlay and the change flash. omarky takes the larger of these and the compositor's window rounding, which is 0 by default, and Quire has no such rounding. (`Commons/Style.qml:31`, `components/blocks/CodeBlock.qml:48`, `components/blocks/FrontMatter.qml:34`)
- **PAGE-89** [adapted: 0005] [verify: unit] The accent picker in Settings replaces the theme's accent in every accent role on the page (link, selection, find, inline-code tint, quote bar, done task box, alert and pill fallbacks). Quire first adjusts a picked accent until it reads on the background (`quire: desktop/src/theme.rs`, `with_accent`), and the page uses the adjusted colour. omarky has no accent picker. (`services/Theme.qml:30`)

## 5. Syntax colours

- **PAGE-49** [adapted: 0005] [verify: unit, state] Code colours are generated in memory from the active palette's roles and named colours, so a code block matches its page. The table is rebuilt whenever the palette changes. omarky: generates highlighter theme files from the Omarchy theme's palette at start. (`docs/decisions/0006-ksh-themes-from-colors-toml.md:1-15`)
- **PAGE-50** [in] [verify: unit] The mapping is Normal and Variable in fg. Keyword, ControlFlow and Import in magenta. Function, Extension and Information in blue. String and VerbatimString in green. SpecialString, BuiltIn and Others in cyan. DecVal, BaseN, Float, Constant, Char, SpecialChar and Preprocessor in orange. DataType, Attribute and Warning in yellow. Operator and Annotation in code-dim = mix(fg, bg, 0.3). Comment and Documentation in code-faint = mix(fg, bg, 0.52), italic. CommentVar in code-dim italic. RegionMarker in code-faint. Alert in red bold. Error in red, underlined. The highlighter's own token kinds map onto these style names. (`lib/kshtheme.js:58-90`, `lib/kshtheme.js:38-39`)
- **PAGE-51** [in] [verify: unit] Code-dim (0.3) and code-faint (0.52) are slightly stronger than the page's dim (0.34) and faint (0.58). (`lib/kshtheme.js:38-39`, `services/Theme.qml:46-47`)
- **PAGE-52** [in] [verify: unit] Fence language names are mapped by alias. sh, shell and console map to Bash. js and mjs to JavaScript. ts to TypeScript. py to Python. rs to Rust. yml to YAML. jsonc to JSON. c and h to C. cpp, cc and hpp to C++. patch to Diff. math and tex to LaTeX. Unknown names pass through unchanged. (`services/Highlight.qml:34-53`)
- **PAGE-53** [adapted: 0005] [verify: unit] Code in a language the highlighter does not know renders as plain mono in the text colour, and nothing is logged. Because the syntax table is rebuilt on every palette change, no theme ever falls back to stock colours. omarky: when highlighting is unavailable code is plain mono, `OMARKY_NO_KSH=1` forces that path, and a theme added after launch uses the highlighter's stock theme until restart. (`services/Highlight.qml:57-58`, `components/blocks/CodeBlock.qml:96`, `services/Highlight.qml:14-16`)

## 6. Vertical rhythm

The gap above each block depends on the block before it. Headings bind to what follows them.
All values are round(B × factor). (`services/Reading.qml:92-113`)

- **PAGE-54** [in] [verify: unit, geometry] The first block has gap 0. Its room is the S(44) top room. (`services/Reading.qml:95`)
- **PAGE-55** [in] [verify: unit, geometry] A heading after a thematic break gets 1.5 em [21]. (`services/Reading.qml:101`)
- **PAGE-56** [in] [verify: unit, geometry] A heading directly after a heading of a higher rank (h3 after h2, for example) gets 1.25 em [18] when the earlier heading is h1 or h2, else 0.7 em [10]. (`services/Reading.qml:102-103`)
- **PAGE-57** [in] [verify: unit, geometry] Any other heading gets h1 2.6 em [36], h2 2.2 em [31], h3 1.7 em [24], h4 1.4 em [20], h5 1.3 em [18], h6 1.3 em [18]. (`services/Reading.qml:104`)
- **PAGE-58** [in] [verify: unit] Framed blocks are code, diff, table, alert, image, gallery, quote and front matter. (`services/Reading.qml:106`)
- **PAGE-59** [in] [verify: unit, geometry] A block after a heading gets 0.75 em [11] if the heading is h1 or h2 or the block is framed, else 0.5 em [7]. (`services/Reading.qml:107`)
- **PAGE-60** [in] [verify: unit, geometry] A block after front matter gets 2.2 em [31]. (`services/Reading.qml:108`)
- **PAGE-61** [in] [verify: unit, geometry] A thematic break, or a block after one, gets 1.9 em [27]. (`services/Reading.qml:109`)
- **PAGE-62** [in] [verify: unit, geometry] The footnotes block gets 2.6 em [36]. (`services/Reading.qml:110`)
- **PAGE-63** [in] [verify: unit, geometry] A framed block, or a block after one, gets 1.25 em [18]. (`services/Reading.qml:111`)
- **PAGE-64** [in] [verify: unit, geometry] Every other pair (paragraph to paragraph, paragraph to list and so on) gets 0.9 em [13]. (`services/Reading.qml:112`)
- **PAGE-65** [in] [verify: unit] The rules apply in the order PAGE-55 to PAGE-64, and the first match wins. (`services/Reading.qml:94-113`)
- **PAGE-66** [in] [verify: unit, geometry] Inside a container (list item, quote, alert, footnote) the same rules apply, capped at 0.9 em [13]. (`components/BlockStack.qml:27`)
- **PAGE-67** [in] [verify: unit, geometry] Inside a tight list item, a list next to any block gets 0.3 em [4]. (`components/BlockStack.qml:26`)
- **PAGE-68** [deferred] [verify: -] A folded section in changes mode spaces like the heading it stands for. Changes mode is deferred (decision 0006). (`services/Reading.qml:96-98`)
- **PAGE-69** [in] [verify: geometry] There is no extra margin after a block. Vertical space is only the gap above the next block. (`components/DocView.qml:297-298`)

## 7. Inline text

- **PAGE-70** [in] [verify: golden] Bold is the Bold face in the strong colour. Bold inside a link keeps the link colour. (`lib/inline.js:173`)
- **PAGE-71** [in] [verify: golden] Italic is the true Italic face. (`lib/inline.js`, `docs/decisions/0003-quattro-s-regular-style-name.md:7-8`)
- **PAGE-72** [in] [verify: golden] Strikethrough is a line through the text, in dim. (`lib/inline.js:169`)
- **PAGE-73** [in] [verify: script] Links are in the link colour with no underline. The underline appears only while the pointer is over that link. (`lib/inline.js:174-176`)
- **PAGE-74** [in] [verify: script] The pointer is a pointing hand over a link and an I-beam elsewhere on text. (`components/RichText.qml:159`)
- **PAGE-75** [in] [verify: geometry] Inline code is mono at round(B × 0.86) in the inline-code colour, padded by a four-per-em space (U+2005) on each side, on a chip. (`lib/inline.js:164`)
- **PAGE-76** [in] [verify: geometry] The inline-code chip is round(inline-code size × 1.5) [18] tall, with corner radius max(3, round(height × 0.22)) [4], filled chipFill with a 1 px chipEdge border, and centred on the x-height of the line. The room each side is a no-break space, so a line never breaks between the chip's edge and its code. (`components/RichText.qml:76`, `components/RichText.qml:123-127`, `components/RichText.qml:154`)
- **PAGE-77** [in] [verify: geometry] A chip that wraps across lines is drawn as one piece per line. A chip piece is at least 4 px wide. (`components/RichText.qml:90-100`)
- **PAGE-78** [adapted: 0009] [verify: golden] Footnote references are superscript. Quire draws them small, 0.7 of the body size, on the baseline (decision 0009). (`lib/inline.js:168`)
- **PAGE-79** [in] [verify: unit, golden] Raw inline HTML is shown literally in mono at the inline-code size in faint, never interpreted. (`lib/inline.js:166`, `docs/decisions/0002-raw-html-recognised-never-rendered.md:8-11`)
- **PAGE-80** [in] [verify: unit] `<br>`, in any case and with or without a slash, is a line break. Inline HTML comments are dropped. (`lib/inline.js:47`, `docs/decisions/0002-raw-html-recognised-never-rendered.md:8-13`)
- **PAGE-81** [in] [verify: unit] Typographer replacements are on, as markdown-it's typographer does them. Straight quotes become curly, `--` and `---` become dashes, `...` an ellipsis and `(c)` a copyright sign. (`lib/markdown.js:14`)
- **PAGE-82** [in] [verify: unit] Autolinks cover http, https and ftp URLs, `www.` hosts and e-mail addresses. Trailing punctuation is trimmed and balanced parentheses kept, as on GitHub. Bare domains like `file.md` are not linked. A match glued to letters before it is not linked. (`lib/linkify.js:3-32`, `docs/decisions/0004-own-linkify-and-footnotes.md:3-8`)
- **PAGE-83** [in] [verify: script] Text on the page can be selected with the mouse but never takes keyboard focus, so keys always reach the reader. Only one block holds a selection at a time. (`components/RichText.qml:52`, `docs/decisions/0010-window-behaviour-choices.md:22-24`, `components/MarkdownView.qml:123`)

## 8. Motion

- **PAGE-84** [in] [verify: script] Every animation duration becomes 0 under reduced motion. Reduced motion is on when `QUIRE_REDUCED_MOTION=1` is set, when Hyprland's `animations:enabled` is off, or when the system setting Rune reads asks for it. Timers (toast hold, chord timeout, debounces) are not affected. (`services/Reading.qml:61-66`, `services/Reading.qml:115-123`)
- **PAGE-85** [in] [verify: script] The page fades in over 150 ms on an ease-out-quad curve when a note opens in preview mode. (`components/DocView.qml:284-289`)

## Open questions

- Resolved in phase 5: built-in themes carry named colours. The upstream palettes keep their own
  (Solarized, Gruvbox, Catppuccin, Rosé Pine, Nord, Tokyo Night), and Paper, White, Ink and Black
  have a muted set picked to sit beside their accent (`theme.rs`, `decisions.tsv`).
- Light and dark thresholds differ. For an Omarchy theme without `mode`, omarky calls it light
  above luminance 0.5 and Quire calls it dark below 0.25, so a mid-grey background can disagree.
  The recommended default is to follow Quire's flag (PAGE-32), so the page and Quire's chrome always
  agree.
- Headings under another prose font. Decision 0002 sets Quattro S Bold for headings, but says
  nothing about headings when the user picks Newsreader or Instrument Sans. The recommended
  default is that headings use the chosen prose font's Bold (PAGE-87), because Quattro S Bold over a serif body
  mixes two families the user did not pick.
- iA Writer Duospace as a prose font. It is in the writer's font cycle but not named as a
  preview choice. The recommended default is to leave it out of the preview list, since decision 0002 says
  the preview keeps the prose font.
- Quire's "Line width" setting. Quire offers Readable and Full width today. omarky has reading
  (68) and wide (100) on a key. The recommended default is that the setting becomes Reading or Wide, the
  wide key toggles the same setting, and a stored "full" maps to Wide.
- The default size for existing users. Quire's old default size of 16 is written into
  `state.json`, so it cannot be told apart from a chosen 16. The recommended default is to leave stored
  values alone and say in the release notes that the new default is 14.
- The highlighter. PAGE-50 is written in the KSyntaxHighlighting style names omarky uses.
  The recommended default is to pick the Rust highlighter in phase 3 by checking its token kinds against
  that list, and keep the mapping as one table that a unit test reads.
