# The reader and writer port

Quire's desktop app gets the reading and writing experience of omarky
(`/home/p0werj0int/Projects/omarky`, version 0.7.0). omarky is a Markdown reader that sets a
file like a printed page, and a writer whose live-formatted source keeps the file's exact text.
This folder holds what Quire must do, why, and the path to get there. It covers the desktop app
only.

| Path | Holds |
| --- | --- |
| `decisions/` | The decisions that adapt omarky to Quire, one record each |
| `behavior/` | What Quire must do, one numbered item per behaviour, each with how it is checked |
| `deferred/` | omarky behaviour kept for later passes: changes mode, rewrite with Claude |
| `source/omarky-0.7.0/` | The behaviour extracted from omarky's code and docs, with citations. The behaviour files are adapted from it |
| `tools/check-specs.py` | Checks the behaviour files' format, and that no extracted item is missing |
| `tools/parity.py` | The parity pass (VER-30): renders Quire beside omarky's shots under the review themes, measures each line's drift, and writes a page for sign-off |
| `reference/omarky-0.7.0/shots` | omarky's own screenshots of the samples under the review themes: the parity target |
| `TODO.md` | Work left for later passes |
| `decisions.tsv` | The decision trail of the planning run |

## The decisions in one place

- Preview mode becomes omarky's reader. Source mode becomes omarky's writer. Quire keeps its
  notes folder, file tree, tabs and outline (decision 0001).
- iA Writer Quattro S, Duospace and Mono S are bundled. Quattro S and Mono S are the defaults.
  The default body is omarky's 14 px, and the size stays configurable. Every other size is a
  ratio of the body (decision 0002).
- Keys go through one keymap with an omarky preset and a quire preset (decision 0003).
- Quire patches Rune so a text field can hang marks in the margin and give a line its own
  leading (decision 0004).
- Page colours derive from Quire's active theme with omarky's factors (decision 0005).
- In scope: the page, every block, reader behaviour, the writer, the edit sheet, the keymap.
  Changes mode, rewrite with Claude and spell checking come later (decision 0006).
- Quire's panels stay where they are, and the column leans from both sides (decision 0007).
- Both side panels take omarky's outline type, spacing and text colours (decision 0008).

## How Quire knows it is done

The port is done when all of these hold:

1. `python3 docs/reader-writer/tools/check-specs.py` passes.
2. Every item with status `in` or `adapted` has a passing check of the kind its `verify` tag
   names. The coverage script from phase 1 lists no untested item.
3. Every `manual` item is signed off by the user in a side-by-side review with omarky.
4. Typing and scrolling stay inside the budgets in `behavior/writer.md` (WR-224) and
   `behavior/verification.md` (VER-32).
5. `cargo test` passes, including Quire's existing sync, lock and folder tests.

## Rigor

High. Three choices are hard to undo: how the Rune patch is carried, which Markdown parser the
block model uses, and the default keymap preset. The writer change touches every keystroke,
the save path and locked notes. So each phase below ends in a check against the real app,
reviewed before the next phase builds on it. Each unit lands as its own commit, and each phase
as its own branch.

## Phases

Phases run in this order, riskiest first after the harness. Phases 1 and 2 can run at the same
time, because they touch different code.

### Phase 1. Verification harness and baseline

Status: paused at the user's request, 2026-11-14. The unbuilt work is on branch
`rw/phase-1-harness`. Until it returns, units are checked by unit tests and headless
`--snap` renders.

Build the checks before the work, and record Quire as it is today, so every later check reads
"old value, new value".

- Copy omarky's samples into Quire's test fixtures (`behavior/verification.md` lists them).
- A state dump and a geometry dump of the preview and the editor, readable from
  `rune::Runtime` tests and from `--tree`.
- Golden PNGs of today's preview and source mode for each sample, at 1400 × 1000 and
  800 × 1000.
- Reference PNGs from omarky's own screenshot tool, on the same samples and themes. These are
  the parity target.
- A clean-log gate for headless runs.
- A scroll bench and a typing bench (400, 1,000 and 3,000 lines), with today's numbers recorded.
- A coverage script that lists in-scope items with no check yet.

Done when the harness runs headless and the baseline numbers and images are committed.

### Phase 2. Rune patch spike

Status: paused at the user's request, 2026-11-14. The work in progress is on branch
`quire/paragraph-layout` in `~/Projects/rune-quire`. Decision 0004 stays open.

The riskiest unknown. It answers whether hanging marks and per-line leading work in a Rune text
field with honest caret behaviour and acceptable typing speed.

- Design the API with the `architect` skill: a per-paragraph indent and first-line indent, and a
  per-span or per-paragraph line height, all outside undo.
- Measure the two ways to carry the patch: a fork branch of Rune pinned by revision, or a
  vendored crate through `[patch]`. Pick one.
- Implement it in Rune with Rune-side tests.
- Try it in Quire's editor on one sample note: hanging `#`, `-` and `>`, and heading leading.
- Check WR-03 (undo after typing `- `), WR-225 (caret, selection, clicks and IME on hanging
  marks), and typing latency with `dir_by_paragraph` on, against the phase 1 numbers.

Done when those checks pass. If typing latency fails its budget, stop and plan again before
phase 7. The fallback is one text field per block, which brings its own caret and undo work.

### Phase 3. Typography and keymap foundation

Status: done on branch `rw/phase-3-typography`, except the highlighter crate, which moves to
phase 5 with the code blocks that use it. See `decisions.tsv`.

- Bundle the fonts with their licence, and load faces by file (PAGE-02 has the test).
- The size setting, 14 px by default, the zoom keys, and how a stored size from an older
  version is treated.
- Colour roles derived from the palette, with unit tests on the colour maths.
- The syntax palette generated from the theme, and the choice of highlighter crate.
- Reduced motion as one gate for every duration.
- The keymap: an action registry, the two presets, the conflict test (`behavior/keymap.md`,
  Checks), and the Shortcuts page generated from it.

### Phase 4. Block model and parser

Status: done on branch `rw/phase-3-typography`, with pulldown-cmark as the parser.

- Choose the parser: extend `md.rs`, or use a crate such as pulldown-cmark or comrak, with the
  `architect` skill. It must give source line maps for every block.
- The flat block list with source maps and stable keys (a content hash plus an ordinal), so
  scroll anchoring survives edits and reloads.
- Every block type in `behavior/blocks.md`: tables, alerts, footnotes, front matter, raw HTML,
  autolinks, typographer.
- omarky's parser test tables, ported (VER-38).

### Phase 5. The reader page

Status: done on branch `rw/phase-3-typography`, except the edge fades and the hover-only details
listed in `decisions.tsv`. Virtualization landed in phase 9. Code colours come from `rune-highlights`.

- The measure, gutter and lean (PAGE-21 to PAGE-31).
- The vertical rhythm table (PAGE-54 to PAGE-69).
- Each block's setting, hanging marks on the page, inline chips, colours.
- Virtualization for long notes.

Checked with geometry and golden tests, then a side-by-side look against omarky's reference
PNGs.

### Phase 6. Reader behaviour

Status: done on branch `rw/phase-3-typography`, except touchpad fling, the find debounce and
band, two-part anchor glides, hover fades and the preview toolbar buttons listed in
`decisions.tsv`.

Key scrolling and its glide, touchpad scrolling (measured, not copied), the outline that follows
the reading position, find, the wide measure, zoom, live reload that keeps the position, links,
copy source, task ticking, the heading anchor, the help sheet, toasts and banners
(`behavior/reader.md`).

### Phase 7. The writer

Status: done on branch `rw/phase-3-typography`, except hanging marks (phase 2 was skipped),
the caret's width and blink, the scroll thumb, the fade bands and glides on jumps, listed in
`decisions.tsv`. The right-click menu and the typing budget landed in phase 9.

Live formatting per line, hanging marks and leading through the phase 2 patch, the editing keys,
the paragraph fade, typewriter scrolling, the chrome that steps aside while typing, word count,
the round trip between modes keeping the position, the font cycle, the help sheet
(`behavior/writer.md`). The typing bench must stay inside budget.

### Phase 8. The edit sheet

Status: done on branch `rw/phase-3-typography`, except hanging marks in the sheet's editor
(SHEET-028, as in phase 7) and the caret's width and blink (SHEET-029), listed in
`decisions.tsv`. The right-click menu (SHEET-032) landed in phase 9.

`E` and `Shift+E` in the reader, the sheet over the page, saving through Quire's save path,
locked notes, and refusing to write over a change that landed while the sheet was open
(`behavior/edit-sheet.md`).

### Phase 9. Parity pass

Status: done on branch `rw/phase-3-typography`, signed off by the user on 2026-11-15. The zen
mode rename, the budgets as benches (`desktop/src/bench.rs`), typing within budget (through a
local Rune branch), the editor's right-click menu, the code chip's Copied state, the raised
footnote mark (`Span::raise`), the page as a virtual list, and the measured parity pass
(`tools/parity.py`): over the 7 samples and 6 review themes every matched line sits within
2 px of omarky's, except the first block under Quire's file-name line (3 px) and threshold
artifacts, listed in `decisions.tsv`. The scroll bench holds its budget (mean 0.7 ms, worst
6.9 ms).

Every sample under the review themes, side by side with omarky. The user signs off the `manual`
items. Budgets are checked again. `README.md`, `desktop/README.md` and `ARCHITECTURE.md` are
updated.

### Phase 10. The deferred details

Status: in progress on branch `rw/phase-3-typography`. The items phases 5 to 8 deferred, in
passes the user checks one at a time:

- A. Reader motion and find: long jumps travel in two parts (RD-24), find waits for typing
  to pause (RD-56) and keeps the match in a band (RD-60), and a match in a long code line
  scrolls into view sideways (BLK-52). Done. Touchpad fling tuning (RD-27) waits for the
  user's feel.
- B. Hover details: the heading `#` fades over 120 ms and turns accent under the pointer
  (BLK-15), an open task box takes an accent edge (BLK-30), chip buttons share omarky's look
  and 120 ms fades (BLK-100, BLK-51), and inline code and tag chips get their hairline edge
  (PAGE-76). The code and table edge fades were already in (BLK-48, BLK-69). Done.
- C. Reader chrome: in preview the title bar gains Find, Copy all, Wide and Shortcuts
  buttons with tooltips from the keymap, and Find, Wide and the outline show as on while on
  (RD-34, RD-35). `[[note#Heading]]` travels to the heading (RD-102, RD-04). omarky's
  check-live sequence runs as a test with the watcher's period injected (RD-19, VER-34).
  Done.
- D. Writer chrome: the caret's width and blink (WR-175, SHEET-029), the scroll thumb (WR-93),
  the top fade band (WR-177), selected text in the strong colour, the outline's slide (WR-99).
- E. Hanging marks (WR-16 and the rest, SHEET-028): resumes phase 2, paused by the user.

## Open questions

Each behaviour file ends with its open questions and a recommended default. The work follows
the recommended default unless the user decides otherwise. These change what users see the
most:

- The default keymap preset. Recommended: omarky (`decisions/0003`, `behavior/keymap.md`).
- Quire's `Ctrl+.` focus mode and omarky's paragraph focus share a name. Recommended: call
  Quire's mode "Zen mode", and make omarky's paragraph fade and typewriter scrolling separate
  toggles, off by default as in omarky (`behavior/writer.md`).
- The outline's side. Recommended: keep it on the right (`decisions/0007`).
- A note changed on disk while it has unsaved edits. Recommended: keep Quire's rule, where your
  edits win (`behavior/writer.md`).
- The floating formatting toolbar. Recommended: keep it, and let it step aside while typing
  (`behavior/writer.md`).
- Code colours on Quire's built-in themes. Resolved in phase 5: each theme carries its upstream
  palette's colours, and Paper, White, Ink and Black have their own set (`decisions.tsv`).
- `format.rs` rules are shared with Android. Recommended: a `Ctrl+K` change lands in both apps,
  so the toolbar rules stay the same (`behavior/writer.md`).
