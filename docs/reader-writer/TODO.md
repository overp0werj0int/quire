# Later passes

These parts of omarky are out of scope for the current port (decision 0006). Each needs its
own pass that writes its behaviour file and decisions, the way `behavior/` was written.

- **Changes mode.** Pick an earlier git version of a note and see what changed on the page:
  word-level marks, margin tags, the fold, next and previous change. Starting point is
  `deferred/changes-mode.md` (CHG items). To settle first: notes folders that are not git
  repositories, how it fits Quire's sync, and where version resolution lives (omarky decision
  0013 keeps it behind a boundary).
- **Rewrite with Claude.** `Ctrl+J` in the writer and the edit sheet. Starting point is
  `deferred/rewrite-with-claude.md` (SHEET-051 to SHEET-110). To settle first: locked notes,
  whose plain text would be sent to Anthropic, and the boundary for the `claude` subprocess.
- **Spell checking.** Squiggles that appear only after you leave a word, right-click
  suggestions, a personal dictionary. The items are in `behavior/writer.md` with status
  `deferred`. To settle first: a spell backend on Linux (Hunspell or similar), a Rune change
  for app-drawn underlines, and in-app context menus, since Rune has no native menus on Linux.
- **A user keymap file.** Single bindings that override a preset (decision 0003).

## Map mode

Deferred items from `behavior/map.md` (decision 0014):

- **Map multi-select.** Select multiple nodes by Shift/Ctrl+click or marquee drag to move, delete, copy or fold them together (`MAP-076`).
- **Map SVG and PNG export.** Export a note's full mindmap to an SVG or high-DPI PNG file (`MAP-077`).
- **Map minimap.** Floating corner overview showing the viewport over large maps (`MAP-078`).
- **Map alternative layouts.** Two-sided balanced mindmap and top-down org-chart layouts (`MAP-079`).
- **Math rendering in maps.** Render `$...$` and `$$...$$` LaTeX formulas in map nodes once Preview supports math (`MAP-080`).
- **Map free-position pinning.** Pin nodes off the tidy tree layout once a metadata format is decided (`MAP-081`).
- **Map collaborative cursors.** Live multi-user presence and cursors on the map canvas (`MAP-082`).
- **Map cross-kind conversions.** Convert between headings and list items on drag or keyboard moves, fall back headings shifted past `H6` to nested bullets, offer Convert in the node menu, and run `Ctrl+Alt+1` to `Ctrl+Alt+6` on list items and paragraphs (`MAP-083`).
- **Map multi-line Markdown paste.** Paste multi-line Markdown trees directly onto the canvas while not inline-editing (`MAP-084`).
- **Map internal card scrolling.** Scroll inside overflowing code, diff and table cards with wheel hand-off to the canvas (`MAP-085`).
- **Map Space+drag pan.** Pan anywhere on the canvas with `Space` plus left-drag without triggering `Space` fold on key-down (`MAP-086`).
