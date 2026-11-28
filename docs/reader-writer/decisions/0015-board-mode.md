# 0015 Board mode: a `kanban-plugin` note as a kanban board

Status: revised, 2026-11-27.

A kanban note tracks tasks across heading columns (`## Todo`, `## Doing`, `## Done`) with task items
(`- [ ]` / `- [x]`), indented detail blocks, and nested subtasks. Moving a task between columns is
the edit such a note gets most, while non-kanban prose notes are not structured as boards.

Decision: add **Board** as a fourth mode beside Source, Preview and Map (`Mode::Board`), gated to
notes whose front matter declares `kanban-plugin`. `behavior/board.md` (`BRD-001` onwards) defines
the behaviour.

## What the board is

- **Gated to `kanban-plugin` notes.** `board::is_kanban` reads `blocks::Kind::FrontMatter` from the
  note's parsed `Doc`. Regular notes hide Board mode; kanban notes without a saved per-note mode
  override enter Board mode automatically.
- **Task cards with recursive subtasks.** Root heading children become columns; only task items
  (`- [ ]` / `- [x]`) become cards. Nested task items stay folded behind a folding handle with
  colored progress dots (solid for done, outlined for incomplete) and unfold recursively as
  sub-cards obeying the same rules.
- **Card mini window on click.** Clicking a card opens a modal mini window (`Layer::Sheet` with
  `CardView`) in Preview mode rendering everything under the task node through `ui::preview`'s own
  block renderer, with a header `Edit` button that switches in-place into the shared Edit Sheet
  editor and returns to Preview mode on save.
- **The map's tree and moves.** The board reads `map::Tree` and adds no parser. Every drop or move
  maps a board `Slot` (`SlotParent::Column` or `SlotParent::Card`) to a `map::Add`, validates via
  `map::zone`, and applies `map::move_to` through `App::map_apply` as one undo step.
- **One selection.** The tab's selected node is the map's (`MapView::selected`), synced across
  modes.

## What is the board's own

- **The held card.** The board drags the card itself, scaled gently, elevated, and tilted with
  speed-sensitive spring physics, opening an animated drop slot while sibling cards glide aside.
- **Flow layout.** Columns and cards use Rune flow layout and design-system tokens/components
  (`ui/tokens.rs`, `ui/styles.rs`, `ui/components/`), reporting element rects via `El::on_layout`.
