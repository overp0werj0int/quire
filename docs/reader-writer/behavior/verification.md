# Verification plan

This file says how the behaviour items are checked. Every in-scope item in the other behaviour
files names a method (`unit`, `state`, `geometry`, `golden`, `script`, `bench`, `manual`). This
plan says what to build for each method, which items each part guards, and which roadmap phase
builds it. It is a plan, not a list of behaviour, so `../tools/check-specs.py` does not check
its format.

The roadmap phases are:

1. Verification harness and baseline.
2. Rune patch spike.
3. Typography foundation.
4. Block model and parser.
5. Reader page.
6. Reader behaviour.
7. Writer.
8. Edit sheet.
9. Parity pass.

Citations in parentheses point into omarky 0.7.0 (`/home/p0werj0int/Projects/omarky`), unless
they start with `quire:`. "Inferred" marks a claim read from code but not yet run.

## How omarky checks itself

- **VER-01, VER-02** Unit suites run headless under omarky's own JS engine, one file per library
  module (blocks, inline, palette, fonts, ranges and others), followed by Ruby suites for the
  launcher. One command runs them all. (`tools/test:2-7`)
- **VER-03, VER-06 to VER-09** End-to-end checks drive a private, offscreen app instance over
  IPC. Commands open files, press keys, hover, follow links, tick tasks and work the edit sheet.
  Each check asserts on a JSON state dump that holds the scroll position, top block, column
  width, zoom, find state, clipboard and more. (`reader/tools/dev:2-9`, `reader/ui/app/ReaderWindow.qml:107-121`)
- **VER-04, VER-05, VER-10, VER-11** A screenshot tool renders every sample under six review
  themes at 1400 × 1000 and 800 × 1000, three pages each. Shot options put the window in a state
  first (outline, wide, help, find, reloaded, missing, hover, anchor). The shots are for people
  to review, and README shots are framed at 2×. (`reader/tools/screenshot:8-15`, `components/Shooter.qml:8`)
- **VER-12, VER-13** Any warning or error in the app log fails a screenshot run or a check,
  after a short, commented filter for external noise. (`reader/tools/screenshot:89-95`, `docs/decisions/0008-log-rules.md:1-16`)
- **VER-14, VER-15** A bench scrolls a 5,422-line sample in 120 px steps with synchronous layout
  and reports the worst and mean step. The recorded figures are a mean under 1 ms and a worst of
  18 to 19 ms once per run. (`reader/ui/app/ReaderWindow.qml:138-154`, `docs/decisions/0009-scrolling-performance.md:1-15`)
- **VER-16 to VER-22** Fixtures cover a kitchen sink, prose, code, tables, alerts, front matter,
  a long file, malformed input and an empty file. Scripted checks cover live reload through real
  file operations, theme switches on a fake theme tree, task ticks as one-character file diffs,
  and rewrite against a fake `claude`. Every check that uses the optional native painter also
  runs without it. (`samples/`, `reader/tools/check-live:2-18`, `reader/tools/check-theme:2-7`, `reader/tools/check-tasks:2-5`)
- **VER-23, VER-24** Look decisions were made from side-by-side screenshots and zoomed crops.
  Scroll feel was matched to Chromium by reading both stacks' source, not by tuning by eye.
  (`docs/decisions/0005-heading-font.md:1-6`, `docs/decisions/0011-touchpad-scrolling.md:1-17`)

## Fixtures

omarky is MIT, as Quire is. Copy these files from omarky's `samples/` into
`desktop/tests/fixtures/omarky/`, keeping their names and folders, and put omarky's license text
beside them as `LICENSE-omarky`, since the MIT license asks that the notice travels with the
files.

| File | What it exercises |
| --- | --- |
| `kitchen-sink.md` | Every block type on one page. It uses `assets/dusk.png`. |
| `prose.md` | Long paragraphs, headings and rhythm. |
| `code.md` | Code blocks in many languages, line numbers, long lines, diffs. |
| `tables.md` | Natural, squeezed and scrolling tables, numeric columns. |
| `alerts.md` | All five alert kinds. |
| `frontmatter.md` | Fenced front matter with every value kind. |
| `bare-frontmatter.md` | Bare `key: value` front matter. |
| `long.md` | 5,470 lines for the bench and virtualisation. Copy `tools/gen-long` too, to regenerate it. It uses `assets/dusk.png`. |
| `malformed.md` | 24-deep lists, 15-deep quotes, a 3,000-character line, binary junk. |
| `empty.md` | An empty file. |
| `sample.md` | A seven-byte file. |
| `real/reader-spec.md` | A real document of about 20 KB. |
| `assets/dusk.png` | The image the kitchen sink and the long file show. |

`samples/changes/` (`make-repo`, `proposal-before.md`, `proposal-after.md`) belongs to changes
mode. It is copied when changes mode is specified, not now.

Two more fixture sets are needed.

- Omarchy themes for the parity pass and theme checks. Copy `colors.toml` of the six review themes
  (gruvbox, tokyo-night, catppuccin-latte, rose-pine, flexoki-light, matte-black) and of kanagawa,
  whose accent is close to its text colour, from an Omarchy install (`/usr/share/omarchy/themes/`)
  into `desktop/tests/fixtures/omarchy-themes/`. Tests install one under the temporary home
  folder at `~/.config/omarchy/current/theme/`, where Quire looks for it (`quire: desktop/src/theme.rs`, `omarchy_dir`).
- A Quire notes fixture for wiki links, tags and `![[image]]` embeds (BLK-101), written in
  phase 4 from Quire's "Field Notes" sample (`quire: desktop/src/sample.rs`).

## Methods and where they are built

| Method | Built by | Phase |
| --- | --- | --- |
| `unit` | Rust tests of pure modules: parser (VER-38), colour roles (VER-35), rhythm and column formulas (VER-28), keymap (VER-42), font faces (VER-39) | 3, 4, 6 |
| `state` | The state dump (VER-25) | 1 |
| `geometry` | The geometry dump (VER-28) | 1, then each phase adds its keys |
| `golden` | Golden PNGs (VER-29) | 1 runner, goldens from 5 |
| `script` | Key grammar and input scripts (VER-26) on the virtual clock (VER-27) | 1 |
| `bench` | Scroll bench (VER-32), typing latency (VER-41) | 1, 2 |
| `manual` | Parity contact sheets (VER-30) | 9 |

## The plan

- **VER-25** State dump. Build one serialisable snapshot of the app, read in tests through
  `Runtime::update` and written as JSON by a headless run for scripts. It holds the mode, note
  path, loaded and missing flags, block count, parse time, reload count, top block index and its
  source line, scroll offset, progress (3 decimals), outline and file tree shown, wide, theme id,
  light or dark, background and accent hex, size, prose and code font, keymap preset, column
  width, gutter, help shown, find (shown, query, count, current), toast text, banner text,
  clipboard (first 200 characters), selection and edit-sheet state. Phase 1 builds it with the
  fields Quire has today, and each later phase adds the fields its items need. Guards PAGE-10,
  PAGE-19, PAGE-20, PAGE-47, PAGE-86 to PAGE-88, RD-10 to RD-101, and the state items in
  `writer.md` and `edit-sheet.md`.
- **VER-26** Scripted input with omarky's key grammar. Build a translator from omarky's key specs
  (`j`, `G`, `g g`, `ctrl+f`, `shift+space`, `escape`, `type:foo`) to Rune key events, used from
  Rust tests and compiled into Rune `--script` files. Add `wheel:DY`, `swipe:DY,DT,...` with lift
  timing, `click:X,Y`, `rclick:X,Y`, `hover:X,Y` and `drop:PATH`. With the same grammar, omarky's
  check scripts port almost line by line. Phase 1 builds the grammar, and phase 6 adds what the
  reader keys need. Guards RD-80 to RD-85, the `keymap.md` items, WR-64 to WR-78 and the sheet's
  keys.
- **VER-27** A virtual clock. Rune's `Runtime::frame(dt)` advances time by a given step, and
  Quire's `poll` gets its time from the runtime (inferred, confirmed in phase 1). Build an
  `advance(ms)` helper that steps frames, and replace any wall-clock read that drives a UI timer
  with the runtime's time. Then every duration in the specs (110, 120, 140, 150, 160, 900, 1100,
  1400, 1600 ms) is an exact assertion, such as "advance 1399 ms, the chip says Copied; advance
  2 ms more, it is gone". Phase 1. Guards PAGE-85, BLK-15, BLK-51, BLK-100, RD-22, RD-24, RD-33,
  RD-98, WR-217 and WR-218.
- **VER-28** Geometry assertions from a layout dump. Give every block a stable key and key its
  parts (marker, quote bar, badge, chip). Dump each block's type, source range, x, y, width,
  height and gap above, from `Runtime::presented` and `elements()` in tests, or `--tree` from the
  command line. Assert the rhythm table over every pair of block types, and the measure, side room
  and gutter at window widths 800, 1100, 1400 and 1920, sizes 9, 14, 20 and 32, and the panel
  states none, file tree, outline, both and equal widths. Assert list indents and marker
  positions, table column widths, code padding and front-matter rows. The rhythm table and the
  column formulas are also pure functions with `unit` tests. Phase 1 builds the dump, phase 3 the
  column and rhythm checks, phases 4 and 5 the block checks. Guards PAGE-11 to PAGE-31, PAGE-54
  to PAGE-69, PAGE-75 to PAGE-77 and every `geometry` item in `blocks.md`.
- **VER-29** Golden PNGs on the omarky matrix. A runner snaps each fixture with `--snap` and a
  scene per state, under the six review themes (through the Omarchy option and the copied theme
  files) plus Quire's Paper and Ink, at 1400 × 1000 and 800 × 1000, three pages each, at scale 1
  and 2. omarky's shot options become scene parameters (outline, file tree, wide, help, find
  query, reloaded, missing, hovered block, anchor placed S(26) below the top). Goldens live in
  `desktop/tests/golden/` and are compared with a small per-pixel tolerance and a cap on the
  share of differing pixels, because rasterisers differ between machines. Goldens change only
  through an explicit update flag, and a person looks at the difference images before they are
  accepted. Phase 1 builds the runner on today's preview, and goldens are accepted from phase 5.
  Guards every `golden` item, such as PAGE-30, PAGE-39, PAGE-45, PAGE-46, PAGE-70 to PAGE-72 and
  the block looks.
- **VER-30** Parity pass against omarky. Render the fixtures with omarky's own
  `reader/tools/screenshot` and render Quire at the same page size, body size 14, the same Omarchy
  theme and no panels. Compare ink positions measured from the PNGs rather than pixels, relative
  to the page's top-left corner, since Quire adds a title bar. The positions are each block's
  first baseline, the left text edge, marker x, heading cap heights, code and table boxes, and
  table column x. Build the ink measurement as a script, and make contact sheets of crops (a
  heading, a list, a code block, a table, an alert, front matter) side by side for a person to
  judge. Phase 1 renders and stores omarky's references (VER-45). Phase 9 runs the full pass.
  Guards the column, rhythm and block geometry as a second check, and every `manual` item.
- **VER-31** Clean-log gate. Every headless run (UI tests, `--snap`, `--script`, the bench)
  captures the log and fails on any warning or error line. An allowlist file names each piece of
  external noise with a comment saying where it comes from and why it cannot be fixed. Phase 1.
  Guards omarky decision 0008 and, through it, every item.
- **VER-32** Scroll bench. A headless command opens `long.md` in preview mode and scrolls from the
  top in 120 px steps, forcing layout every step, until the end or 5,000 steps. It reports steps,
  worst ms, mean ms (one decimal), the count over 16 ms and over 8.3 ms, the first 12 slow steps
  with the block type and code language there, and total ms. It also reports parse time and open
  to first full frame. The budgets are a worst step under 16 ms and a mean under 1 ms. omarky's
  figures are parse 45 to 60 ms and open about 650 ms. Results are recorded with the commit and
  the machine. Phase 1 runs it on today's preview as the baseline, and phase 5 makes the budget a
  gate. Guards RD-30, BLK-01 and the highlighter cost behind PAGE-49 to PAGE-53.
- **VER-33** Fling curve as a pure function. Test Rune's fling (`runtime/scroll/fling.rs`)
  against a line-by-line port of Chromium's `PhysicsBasedFlingCurve`, and the reader against
  v² / 2a, the 200 ms lift window, the hold, the boost and frames that arrive together
  (`quire: desktop/src/throw_tests.rs`, decision 0012). Separately, record a real swipe's raw deltas on the target
  machine (after the vendored winit's touchpad scaling), replay them as a script, and compare the
  distance travelled with the same swipe in Chromium. Phase 6. Guards RD-26 to RD-28 and BLK-47.
- **VER-34** File-system scripts for reload. Port `check-live`'s six operations against a note
  in a temporary notes folder: append, insert three blocks above the reading position, an
  editor-style atomic rename, a burst of 10 writes, delete and restore. After each, assert that
  the top block is the same block at the same offset, plus the debounce and the re-check while
  the file is missing as `reader.md` sets them. Quire's watcher polls every 1.5 s today, so its
  interval is injected in tests. Phase 6. Guards RD-10 to RD-19.
- **VER-35** Theme switching. In a headless runtime, switch between Quire themes, light and dark,
  picked accents, and the Omarchy option with a fake theme folder that is swapped the way
  `omarchy theme set` swaps it. After each switch, assert every page role and syntax colour
  against values computed by the `unit` colour functions, assert the scroll position did not
  move, and assert no reload happened. Include kanagawa for the link fallback and a light theme
  for every light and dark pair. Phase 3. Guards PAGE-32 to PAGE-53 and PAGE-89.
- **VER-36** Task tick round trip. Assert one-character file diffs for a tick and a clear (after
  the writer thread's barrier), CR and CRLF line endings kept, a stale line refused with the file
  byte-identical, and no click while the file is missing or the sheet is open. Add a locked note,
  where the tick must write sealed text only. Phase 6. Guards RD-73 to RD-77 and BLK-28 to
  BLK-32.
- **VER-37** Malformed and edge fixtures. Render `malformed.md`, `empty.md`, `sample.md` and
  `long.md`. Assert the page renders, the column width equals its formula value, no element has a
  zero or negative width, and the log is clean. Phases 4 and 5. Guards BLK-08, BLK-21, BLK-34
  and PAGE-24.
- **VER-38** Parser parity tests. Port omarky's blocks, inline, url and palette suites as Rust
  table tests over Markdown strings: slugs with repeats, task detection, alert detection, diff
  line kinds, numeric columns, front-matter kinds, gallery detection, comment dropping and
  autolink edge cases. Where it works, produce the expected values by running omarky's own
  `lib/*.js` under Node with the `.pragma library` line stripped, so the corpus comes from omarky
  rather than from reading it (inferred, tried in phase 4). Phase 4. Guards BLK-01 to BLK-08,
  BLK-22, BLK-27, BLK-28, BLK-37, BLK-53, BLK-54, BLK-63 to BLK-68, BLK-74, BLK-75, BLK-80,
  BLK-84 to BLK-88, BLK-94, BLK-101 and PAGE-79 to PAGE-82.
- **VER-39** Font face check. A unit test maps each (font, bold, italic) to its bundled file for
  every prose and code font. A rendering test sets a plain paragraph and checks that its runs
  used the Regular file, and compares its stem width with references rendered from the Regular
  and Bold files. Phase 3. Guards PAGE-01 to PAGE-04, PAGE-07, PAGE-87 and PAGE-88, and omarky
  decision 0003.
- **VER-40** Reduced motion. Run the scripted suites again with `QUIRE_REDUCED_MOTION=1` and
  Rune's reduced-motion flag. Assert that every move lands in the same frame, a touchpad fling still runs (decision 0012),
  the fade-in is skipped, and timers keep their durations. Phase 6. Guards PAGE-84, PAGE-85,
  RD-27 and WR-217.
- **VER-41** Rune patch proof (decision 0004). Headless tests on the patched text field cover
  caret movement across hanging marks, selection, a click in the margin landing on the mark, IME
  preedit next to a mark, undo after typing `- ` as one step, and a heading line with its own
  leading and space above. A bench measures typing latency at 400, 1,000 and 3,000 lines with
  `dir_by_paragraph` on, for both ways of carrying the patch. Phase 2. Guards the hanging-mark
  and line rules in `writer.md` (WR-08 to WR-26), WR-63 and the typewriter items WR-85 to WR-94.
- **VER-42** Keymap tests (decision 0003). Unit tests load both presets, check that no chord is
  bound twice in one context, and check that every action in `keymap.md` exists in both. The
  help sheets and the Shortcuts page are asserted to list exactly the active keymap's entries.
  Phase 6, extended in phase 7. Guards the `keymap.md` items, RD-80 to RD-91 and WR-158 to
  WR-166.
- **VER-43** Settings round trip (decision 0002). Under a temporary home folder, change the size
  with the Settings control and with the zoom keys, pick each prose and code font, and switch the
  keymap preset. Let `state.json` flush, start a new runtime from the same folder, and assert the
  values came back. Also assert that a stored size of 40 loads as 32 and an unknown font id
  loads as the default. Phase 3. Guards PAGE-06, PAGE-07, PAGE-19, PAGE-20 and PAGE-86 to
  PAGE-88.
- **VER-44** Mode round trip (decision 0001). Switch between preview and source at many
  positions in `long.md` and assert the reading position is kept both ways, by top block and
  source line. Phase 7. Guards WR-107 to WR-114.
- **VER-45** Baseline. Phase 1 records where Quire starts. Snap today's preview and source modes
  for every fixture, render omarky's references for every fixture, theme and size, and run the
  scroll bench on today's preview. Store them with the commit and the machine. These are the
  references for VER-29, VER-30 and VER-32.
- **VER-46** Item coverage report. A script lists every in-scope item with no test that names
  it. Tests carry the item ID in their name (`page_54_first_block_has_no_gap`), and golden scenes
  and parity crops list the IDs they cover in their scene table. The script runs beside
  `check-specs.py`, so a phase is done only when its items are covered. Phase 1.

## Order of work

| Phase | Builds |
| --- | --- |
| 1 Verification harness and baseline | VER-25, VER-26, VER-27, VER-28 (dump), VER-29 (runner), VER-31, VER-32, VER-45, VER-46 |
| 2 Rune patch spike | VER-41 |
| 3 Typography foundation | VER-28 (column and rhythm), VER-35, VER-39, VER-43 |
| 4 Block model and parser | VER-37 (parsing), VER-38 |
| 5 Reader page | VER-28 (blocks), VER-29 (goldens accepted), VER-32 (budget as a gate), VER-37 (rendering) |
| 6 Reader behaviour | VER-26 (reader keys), VER-33, VER-34, VER-36, VER-40, VER-42 |
| 7 Writer | VER-42 (writer keys), VER-44 |
| 8 Edit sheet | VER-25 (sheet fields), VER-26 (sheet keys) |
| 9 Parity pass | VER-30 |
