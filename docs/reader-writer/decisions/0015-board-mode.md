# 0015 Board mode: a note as a kanban board

Status: proposed, 2026-11-27.

A note that tracks work is often a few headings with a list under each: Todo, Doing, Done.
Moving an item from one list to the next is the edit such a note gets most, and in Source it is
a cut and a paste. Map mode can do it with a drag, but a map spends its room on the tree's
shape, not on the lists.

Decision: add **Board** as a fourth mode beside Source, Preview and Map (`Mode::Board`). It
shows the note as columns of cards and moves a card by a drag. `behavior/board.md` (`BRD-001`
onwards) defines the behaviour.

## What the board is

- **A view of the map's tree.** The board reads `map::Tree` and adds no parser. A column is a
  heading child of the tree's root. A card is a child of a column that is not a heading. So any
  note is a board, with no front matter and no special format, and a note written for Obsidian's
  Kanban plugin (`## Column`, `- [ ] card`) reads as the same board.
- **The map's moves.** A drop is `map::move_to` to a `map::Add`, applied by `App::map_apply`. The
  rules for where a node may go are `map::zone`'s. The board adds no rewrite of its own, so undo,
  the save path, locked notes, sync and outside changes work as they do on the map.
- **One selection.** The tab's selected node is the map's (`MapView::selected`). A card selected
  on the board is selected on the map, and a switch of modes keeps the place.

## What is the board's own

- **The held card.** The map drags a small ghost and marks drop zones on its targets. The board
  drags the card itself, lifted and tilted, and opens a slot where it would land while the other
  cards make room. A board is a set of lists, and a list shows a drop best by making the room.
- **Flow layout.** The map lays its world out itself. The board is rows and columns, so Rune
  lays it out, and the board asks Rune where its cards are.

## What was not chosen

- **A board format in front matter** (`kanban-plugin: board`). It would make a board a kind of
  note. Here it is a way to look at any note, as the map is.
- **A board of the map's folds, or of deeper levels.** The columns are one level. A board of a
  subheading's children can come later, as a board rooted at the selected heading.
- **Adding and renaming on the board.** The first pass shows and moves. A card is edited in the
  Edit Sheet, which exists. An inline field for a new card comes with the next pass.
