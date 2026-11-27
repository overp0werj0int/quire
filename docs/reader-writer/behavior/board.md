# Board mode

Board mode shows a note as a kanban board: its headings as columns and what is under each as
cards (decision 0015). It is the fourth mode, beside Source, Preview and Map. It reads the tree
Map mode reads (`map.md`, section 1) and moves a card with Map mode's own move, so the note's
Markdown stays the one source of truth.

Notes on reading this file:

- The format and status tags follow `README.md`. Every in-scope item uses `[adapted: 0015]`.
  Items are checked by behaviour: a `script` item is a case in `desktop/tests/cases/board/` that
  replays input on a note and checks the note on disk or a probe reading.
- `B` is the body size setting (`PAGE-10`), 14 px by default. Values in brackets are for
  `B` = 14 and scale with `B / 14` unless marked fixed.
- Colour names are the page roles of `map.md`. "The tree", "the root", "a node" and
  `subtree_lines` are Map mode's (`MAP-001` to `MAP-010`).

## 1. The note as a board

- **BRD-001** [adapted: 0015] [verify: script] The board is built from Map mode's tree of the note (`quire: desktop/src/map.rs`, `map::build`) and from nothing else. It needs no front matter and no special format. A locked note shows the lock panel, as on the map (`MAP-015`).
- **BRD-002** [adapted: 0015] [verify: script, state] A column is a heading that is a child of the root (`MAP-002`), in note order. With a lone `H1` as the root these are its `H2`s, or whatever headings sit directly under it (`MAP-003`). With a virtual root they are the note's top-level headings.
- **BRD-003** [adapted: 0015] [verify: script, state] A card is a child of a column that is not a heading: a list item, a paragraph, or a block (code, diff, image, gallery, table, quote, alert, raw HTML), in note order. A list item's children (nested items and indented blocks) stay inside its card and make no cards of their own.
- **BRD-004** [adapted: 0015] [verify: script, state] A heading under a column is a card too, a section card, after the column's other cards, as it is in the note. It shows its label in bold and how many nodes it holds. `map::zone` keeps it among headings (`MAP-027`), so it moves only before or after another section card or to the end of a column.
- **BRD-005** [adapted: 0015] [verify: script, state] The root's children that are not headings (text before the first column heading) are the cards of a first column with no heading of its own, the loose column. Its title is the root's label in `dim`. It shows only when it has cards, or while a card is held and a drop there is allowed. It can't be moved. The synthetic `Footnotes` group (`MAP-008`) is not on the board.
- **BRD-006** [adapted: 0015] [verify: script, geometry] A note with no column headings shows the loose column, if it has cards, and beside it a quiet line in the caption style: `Headings become columns. Add one in Source to start a board.`

## 2. Layout and look

- **BRD-007** [adapted: 0015] [verify: geometry] The board fills the note area on `bg`. Columns sit in a row from the left with `round(B × 12 / 14)` [12 px] between them and `round(B × 16 / 14)` [16 px] round the row. When the row is wider than the area it scrolls sideways, by `Shift+wheel`, a touchpad swipe or a drag on empty board. There is no reading column and no progress line.
- **BRD-008** [adapted: 0015] [verify: geometry] A column is `round(B × 20)` [280 px] wide. It is a `panel` surface with a `10 px` radius and `8 px` padding, as tall as its cards and never taller than the area: past that its cards scroll inside it, under its header, with the scroll-edge fades the Edit Sheet uses (`ui::components::faded`).
- **BRD-009** [adapted: 0015] [verify: geometry] A column's header is one row: a `3 × 14 px` bar in the column's branch colour (`MAP-040`), the heading's inline runs in bold `B` in `strong`, cut to one line with an ellipsis, and at the right end the number of its cards in `ui::components::count_pill`.
- **BRD-010** [adapted: 0015] [verify: geometry, golden] A card is a `card` surface with a `1 px` `hairline` border, an `8 px` radius, `8 px` top and bottom and `10 px` side padding, and `6 px` between cards. Its body is what the map's node draws (`map::Node::body`): a label through the reader's inline runs, wrapping at the card's inner width, or the map's block card (`ui/map/card.rs`) at that width with the map's caps on lines and rows.
- **BRD-011** [adapted: 0015] [verify: geometry, script] A task card has its box left of the label (`ui::components::checkbox_box`) and a done task's label is `dim` (`BLK-31`). A list item with children has a caption line under its label in mono `round(B × 0.72)` [10 px] `readable_dim`: `N items`, and for child tasks `done/all`, such as `2/5`.
- **BRD-012** [adapted: 0015] [verify: geometry] Hovering a card lays `hover` over it. The selected card has the focus glass the rest of the chrome uses (`Cx::focus_glass`). Exactly one card or column header is selected, or none on a board with neither.
- **BRD-013** [adapted: 0015] [verify: script] Theme, accent and text size changes restyle the board live. Switching to Board plays the mode fade (`WR-109`), and a tab switch slides as in the other modes.

## 3. Holding and dropping a card

- **BRD-014** [adapted: 0015] [verify: script, geometry] A press on a card that travels past `6 px` lifts it. The held card is the card itself, at its own size, drawn over everything on the board with the dialog elevation (`tokens::Elevation`) and an `accent`-tinted border. It follows the pointer, holding the point it was pressed at. A press on a link, a task box or a mini-bar button acts as that control and lifts nothing.
- **BRD-015** [adapted: 0015] [verify: script, geometry] The held card is tilted. On the lift it turns `3°` clockwise about the point it is held at. While it travels sideways it leans the way it goes: the tilt's sign is the sign of its horizontal travel (clockwise to the right, counter-clockwise to the left) and its size goes from `3°` at rest to `6°` at `1200 px/s` or more. Travel under `4 px` between frames keeps the last sign, so a still hand holds a still card. The tilt changes on a `120 ms` spring (`El::rotate`, `El::rotate_motion`). On release the card turns upright as it settles (`BRD-019`).
- **BRD-016** [adapted: 0015] [verify: script, geometry] The held card's old place closes: the cards under it move up. A slot opens where a drop would put it: a gap as tall as the held card, with a `1.5 px` dashed `accent` outline at 55% and an `accent` wash at 10%, `8 px` radius. The cards at and after the slot move down to make it. Cards move on the `180 ms` layout glide (`MAP-043`).
- **BRD-017** [adapted: 0015] [verify: script] The slot follows the pointer. The column is the one under the pointer's `x`, or the nearest when the pointer is between or past columns. In it the slot goes before the first card whose middle is below the held card's middle, else at the end. A slot that `map::zone` refuses (`MAP-027` to `MAP-030`) moves to the nearest slot in that column it allows, so a text card held over the section cards lands above them. When the column allows no slot there is none, the held card shows the refusal cue (a `dim` wash and the reason in the caption style under it), and a release shows the reason as a toast and changes nothing.
- **BRD-018** [adapted: 0015] [verify: script] Holding the pointer within `36 px` of the board's left or right edge scrolls the row at up to `480 px/s`, and within `36 px` of a column's top or bottom scrolls that column's cards.
- **BRD-019** [adapted: 0015] [verify: script] A release over a slot moves the card: `map::move_to` to the slot's place (before the card after it, else after the card before it, else as the column's child), applied by `App::map_apply` as one undo step (`MAP-013`). The card glides from the pointer into the slot and turns upright over `180 ms`, stays selected, and its column scrolls by the least that shows it whole. A release in the slot the card came from writes nothing. `Esc`, or a release with no slot, puts the card back with the same glide and writes nothing.
- **BRD-020** [adapted: 0015] [verify: script] A moved list item takes the target list's marker and indent and both lists renumber, and a moved block keeps its lines (`MAP-028`, `MAP-030`). A drop into an empty column makes the card that column's first child. The lines outside the two places stay byte for byte (`MAP-012`).
- **BRD-021** [adapted: 0015] [verify: script, geometry] A press on a column's header that travels past `6 px` lifts the whole column, tilted by the rule of `BRD-015` at half the angles. A slot as wide as a column opens between columns where a drop would put it, and a release moves the heading with everything under it before or after the other heading (`MAP-027`).
- **BRD-022** [adapted: 0015] [verify: script] Under reduced motion (`PAGE-84`) the held card has a fixed `3°` tilt with no lean, and nothing glides: cards, the slot and the settle land at once.
- **BRD-023** [adapted: 0015] [verify: script] While a card or column is held, no hover shows, wheel and swipe still scroll, and a change to the note from outside (`RD-01`) ends the hold with nothing written.

## 4. Selection, keys and the rest of the window

- **BRD-024** [adapted: 0015] [verify: script, state] The board's selection is the tab's map selection (`MAP-044`): a click selects a card or a column header, and a switch between Map and Board keeps the node. When the selected node is inside a card, the card is selected on the board. Switching to Source or Preview goes to the selected node's line, as from the map.
- **BRD-025** [adapted: 0015] [verify: script] `Up`/`k` and `Down`/`j` select the previous and next card in the column, and from the first card the column's header. `Left`/`h` and `Right`/`l` select the card at the same height in the next column that way, else its last card, else its header. From a column's header they select the next column's header that way, so `Home`, `Right` and `Alt+Right` walk and move columns. `Home` selects the first column's header. The board scrolls by the least that shows the selection.
- **BRD-026** [adapted: 0015] [verify: script] `Alt+Up` and `Alt+Down` move the selected card one place in its column. `Alt+Left` and `Alt+Right` move it to the end of the next column that way, and on a column header move the column. Each is the move a drop there would be (`BRD-019`), and a move the rules refuse shows its toast and changes nothing.
- **BRD-027** [adapted: 0015] [verify: script] `x` or a click on the box toggles the selected task (`MAP-053`). `Enter`, `e`, `F2` or a double-click opens the Edit Sheet on the card's `subtree_lines`, and on a column header on the heading's own line (`MAP-020`). `Delete` or `Backspace` deletes the selected card as the map does, with its toast (`MAP-025`), and is refused on a column header with the toast `Delete a column in Source or on the map`. `Ctrl+C` copies the card's Markdown. `Ctrl+Z` and `Ctrl+Shift+Z` or `Ctrl+Y` undo and redo (`MAP-014`). `Ctrl+Enter` follows the card's first link (`MAP-054`).
- **BRD-028** [adapted: 0015] [verify: script] A right-click on a card opens a menu layer with Edit, Copy, Move to (one item per other column) and Delete.
- **BRD-029** [adapted: 0015] [verify: script, state] `Ctrl+Shift+K` (`app.mode-board`) switches to Board from any mode, and from Board back to the last linear mode, as `Ctrl+M` does for Map (`KEY-102`). The title bar's mode control gains a Board segment after Map. `state.json` names the mode `board`. The command palette has `Switch to Board`. The board's keys are a context of their own (`Ctx::Board`, falling through to `Ctx::Global`), listed in `keymap.md` and on the help sheet.
- **BRD-030** [adapted: 0015] [verify: script] Find (`Ctrl+F`, `/`) matches card labels and column titles in note order and selects and shows the match. The outline's current entry is the heading the selection is under, and a click on an entry selects that heading's column or card.
- **BRD-031** [adapted: 0015] [verify: bench] A drag frame runs no parse of the note and builds again only the held card, the slot and the columns whose cards moved. A board of 20 columns and 2,000 cards keeps drag frames under `16 ms`.

## 5. Later

- **BRD-032** [deferred] [verify: -] Adding a card or a column on the board, and renaming one in place, with an inline field as on the map (`MAP-016`, `MAP-021`).
- **BRD-033** [deferred] [verify: -] Folded columns: a column folded on the map shown as a narrow strip.
- **BRD-034** [deferred] [verify: -] A board rooted at a chosen heading, so a subheading's children can be the columns.
- **BRD-035** [deferred] [verify: -] Holding several cards at once.
