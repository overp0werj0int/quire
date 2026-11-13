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
