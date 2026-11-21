# 0014 Map mode: live mindmap editor over Markdown

Status: proposed, 2026-11-21.

Quire edits a note's Markdown in two views: Source (the live-formatted writer) and Preview (the
reader page with the block edit sheet). Users who think spatially or outline long notes want a
third view: a live mindmap canvas where headings, lists, images and code blocks appear as an
interactive tree, and every edit writes straight back to the same `.md` file.

Decision: add **Map** as a third mode beside Source and Preview (`Mode::Map`), with
[Markmap](https://markmap.js.org/) (`github.com/markmap/markmap`, classic `v0.18.13`
`packages/markmap-lib`, `packages/markmap-html-parser`, `packages/markmap-view`, plus `v1.1.1`
`packages/parse`) as the visual and structural basis. `behavior/map.md` (`MAP-001` to `MAP-086`)
and `behavior/keymap.md` (`KEY-102` to `KEY-149`, plus `KEY-50` to `KEY-54`) define the behaviour.

## What comes from Markmap

- **Left-to-right tidy tree layout.** A `d3-flextree`-style horizontal tree with the root on the
  left and children growing to the right (`packages/markmap-view/src/view.ts`,
  `v1.1.1/packages/render/src/layouts/tree.ts`). Default spacing keeps Markmap's proportions at
  the 14 px body size: 80 px between columns, 5 px between siblings (10 px between subtrees of
  different parents, plus line width), 8 px horizontal label padding and 16 px minimum node height
  (`packages/markmap-view/src/constants.ts`).
- **Node underlines, S-curves and fold circles.** Each node sits on a coloured horizontal line
  whose width narrows by depth as `1 + 3 / 2^depth` (4.0, 2.5, 1.75, 1.375 px). Horizontal cubic
  Bézier S-curves connect a parent underline's right end to each child underline's left end. A
  6 px radius circle at the right end of a parent's underline toggles folding; Ctrl-click
  (Cmd-click on macOS) toggles recursively (`packages/markmap-view/src/view.ts`).
- **Heading and list hierarchy.** Top-level headings nest by level (`H1` to `H6`), list items
  nest under their owning heading or parent item, and a lone top-level `H1` acts as the root
  (`packages/markmap-lib/src/transform.ts`, `v1.1.1/packages/parse/src/to-tree.ts`).
- **Fit and directive conventions.** Fit-to-view keeps `fitRatio = 0.95` and
  `maxInitialScale = 2.0`. `<!-- markmap: fold -->`, `<!-- markmap: foldAll -->` and a safe
  subset of `frontmatter.markmap` options (`initialExpandLevel`, `maxWidth`, `colorFreezeLevel`,
  `spacingHorizontal`, `spacingVertical`) work as in Markmap
  (`https://markmap.js.org/docs/json-options`).

## What Quire changes

- **Live editing with Markdown as the single source of truth.** Markmap is a read-only renderer.
  Quire's Map mode edits nodes inline, adds children and siblings, deletes branches, toggles task
  checkboxes, drags nodes to reorder or reparent within valid CommonMark boundaries (refusing
  cross-kind heading/list conversions and `H7+` shifts in v1), and opens Quire's Edit Sheet on
  cards. Every gesture (including task toggles) calls `ensure_edit`, applies the minimal changed
  line range inside one `undo_group` on the note's `TextEdit`, and then calls `App::set_text` so
  undo history is preserved, locked notes are sealed, and writes go via the `Writer` thread.
  Lines the user did not touch stay byte-for-byte identical. No sidecar map file ever goes into a
  notes folder.
- **No dropped Markdown blocks, and rich cards.** Classic `markmap-html-parser` silently drops
  plain paragraphs, blockquotes, alerts and raw HTML. Quire builds the tree from `blocks::Doc`
  so every content block appears as a child node or card in source order: standalone images and
  galleries show visual previews (with a new `Layer::Lightbox` modal layer), code and diff blocks
  render as syntax-highlighted capped cards with Copy and Edit chips, and tables, quotes, alerts
  and raw HTML render as capped Quire cards (clipping with `"+N more lines/rows"`, no internal
  scroll view in v1). Footnote definitions gather into a collapsed `Footnotes` branch (with
  `blocks::Footnote` gaining `lines`, supporting Edit, Copy and fold only), and front matter
  stays out of the branch tree behind a root properties badge.
- **Quire's theme palette and typography.** Markmap's `d3.schemeCategory10` colours and CSS
  sans-serif are replaced with Quire's active palette (decision 0005) and Settings fonts
  (decision 0002). The root uses `Roles::accent`, and depth-1 branches cycle through the active
  theme's six named chromatic colours (`blue`, `green`, `orange`, `magenta`, `cyan`, `yellow`),
  each lifted to at least 3.0:1 contrast on `Roles::bg` and frozen per depth-1 branch by default.
  Switching built-in themes, accents or the Omarchy theme recolours the map live.
- **Default wrapping width and fold counts.** Markmap defaults to `maxWidth: 0` (unbounded
  lines) and shows only a filled dot when folded. Quire wraps text nodes at `round(B × 22)`
  (308 px at 14 px) so prose stays legible, caps cards at `round(B × 30)` (420 px), and shows a
  `+N` hidden-descendant pill beside every folded circle.
- **Infinite canvas and Quire motion.** Dragging empty space, middle-dragging, touchpad swipes
  and the wheel pan without bounds everywhere on the canvas; `Ctrl+wheel`, pinch, `+`/`=` and
  `-` scale the canvas (`0.1` to `4.0`) without changing Settings' text size (`Ctrl+=`/`Ctrl+-`
  stay global text size). Layout transitions take 180 ms and camera glides 200 ms (markmap:
  500 ms), dropping to 0 ms under Quire's reduced-motion gate (`PAGE-84`). Viewport
  virtualisation keeps pan and zoom frames live-only unless the mounted set changes, with worst
  frame under 16 ms and initial layout under 50 ms on a 2,000-node bench sample in `bench.rs`.
- **Full Quire integration.** `[[wiki links]]`, `#tags`, task checkboxes, `attachments/` images,
  Find (expanding folded branches to reveal matches, with Map's structure edits disabled from the
  Find field), the Outline panel, Zen mode, Note mode (decision 0011), locked notes and live
  outside-change reloading all work in Map mode.

## What this overrides or extends

- **Title bar mode control (`RD-34`, `WR-227`).** The single Source/Preview flip button becomes
  three compact `24 × 24 px` mode buttons (Source, Preview, Map) with the active mode marked on,
  so any mode is one click away. Map mode places Find, Fit to view and Help left of the hairline.
- **Keymap (`keymap.md`, decision 0003).** Adds `Ctx::Map` (canvas navigation and structure
  edits), `Ctx::MapEdit` (inline text input, sharing `KEY-50` to `KEY-54` inline formatting with
  Writer and Sheet), and global `app.toggle-map` (`KEY-102`) on `Ctrl+M` in both presets, plus
  `KEY-103` to `KEY-149`. `Ctrl+M` enters Map mode or returns to the previous linear mode
  (defaulting to Source when the session started in Map); `app.toggle-mode` (`Ctrl+P` in omarky,
  `Ctrl+E` in quire) switches from Map to the other linear mode.
- **Session persistence (`store.rs`).** `Persist.mode` accepts `"source"`, `"preview"` and
  `"map"` in Folder mode. Note mode still opens in Preview and Zen and keeps `"map"` out of
  `state.json` (decision 0011).

## Out of scope for the first release

Deferred to later passes (`TODO.md`, `MAP-076` to `MAP-086`): multi-select and batch branch
operations, exporting maps to SVG or PNG, a minimap navigator, alternative layouts (two-sided
and org-chart), LaTeX math rendering (waits for math support in Preview), free-position node
pinning off the tidy tree, collaborative cursors, cross-kind heading/list structure conversions
(including `H7+` fallback to bullets, the Convert menu, and `Ctrl+Alt+1`..`6` on non-headings),
multi-line Markdown paste onto the canvas, internal scrolling inside code/diff/table cards, and
`Space`+drag canvas pan. Markmap's `extraJs` and `extraCss` frontmatter hooks are dropped because
Quire is a native Rust app and never executes document scripts.
