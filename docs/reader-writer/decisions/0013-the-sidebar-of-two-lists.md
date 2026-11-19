# 0013 The sidebar of two lists

Status: accepted, 2026-11-19. The user agreed the layout with a designer and answered the
spec's four open questions the same day. Not built yet.

The sidebar had grown parts one at a time: the Open list, the folder's name with its menu, a
Files, Search and Tags switch, a new folder button, New note and Settings rows with their
keys. The active note was marked twice, once in the Open list and once in the tree. Search
and Tags were tabs that hid the tree.

Decision: one sidebar of two lists, the open tabs above and the folder's notes below, each
with its own surface, a handle between them, a search field above both and two buttons
below. `behavior/sidebar.md` (SB-01 to SB-135) says what it does. It replaces
`ui/sidebar.rs` in full.

What this overrides:

- **0007, the 2026-11-16 update.** The Open list is no longer set in the tree's row style.
  Each open note is a tab of two rows, name and folder, on a darker surface, in the order the
  user drags it to (SB-15, SB-21). Closing the active tab makes the one above it active, where
  the one below was before (SB-20). The rest of 0007 stands: the lean, the full-height
  sidebar, the window corner, the width of 248.
- **0008, rows.** Sidebar rows keep the panel family and its sizes but take a 6 pt radius, a
  1 pt gap and 8 pt side padding in place of the 10 pt inset and 3 pt radius (SB-07). The
  outline panel on the right keeps 0008 as it is.
- **0008, the current row.** The tenth of the accent on the current row is gone from the
  sidebar. One neutral selected fill, about 10 % of the text colour, marks the active tab and
  the result Enter opens (SB-03). The active note in the tree is bold, with no fill.
- **0008, folder icons.** Folders have icons again, in the accent, with no chevrons (SB-49).
  Notes have none.
- **0008, the 2026-11-18 update.** The Files, Search and Tags switch and its pill are gone
  (SB-103).
- **0008 and PAGE-09, families.** Section labels, their counts and tab captions are in
  Instrument Sans, inside the panel. Rows, titles and key names stay in the panel family
  (SB-07). The caption style itself is unchanged: `styles::caption` already has a sans
  variant.
- **0008, colours.** The shelf's surface and the selected fill join the fills `Roles::panel`
  lifts faint against, so the 4.5:1 rule holds on them too. Accent used as text, a new use,
  gets the same lift (SB-06).
- **0011, Note mode's sidebar.** It holds the shelf, with tabs of two rows, and a footer with
  only the settings button (SB-119). Still no folder menu, files, search, tags or New note.
- **KEY-66, KEY-43 and KEY-88.** Ctrl+/ focuses the search field in both modes (SB-100). The
  writer's help sheet moves to F1 and stays in the palette (SB-101).
- **WR-193.** The folder's name no longer opens a menu on a click or Enter. Its menu opens
  from the vault label on a right-click or the Menu key, and Settings leaves it (SB-110).

Kept, though the spec is silent or draws otherwise:

- The width stays 248, not the mock's 264, so the page does not move (SB-124).
- Rows stay in the panel family. The spec asks for "Quattro S", which Quire does not use.
- The title bar's magnifier stays, because it finds in the note (SB-118).
- The drawer below 680 pt holds the same sidebar (SB-121).
- Every keyboard rule of KEY-95 and KEY-98 holds. The spec draws only the pointer, so
  SB-80 to SB-102 fill in the keys.

Items marked `[proposed]` in `behavior/sidebar.md` are choices nobody confirmed. Each names
the other choice.
