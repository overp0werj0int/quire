# 0006 Scope of the port

Status: accepted, 2026-11-13.

In scope, for the desktop app only:

- The reader page: column, measure, rhythm, typography, every block type, hanging marks, colour
  roles, syntax colours.
- Reader behaviour: keys, scrolling and its feel, the outline that follows, find, the wide
  measure, zoom, live reload that keeps the position, links, copy source, task ticking, the help
  sheet.
- The writer: live formatting with hanging marks, the editing keys, focus, typewriter scrolling,
  the chrome that steps aside while typing (dropped by decision 0010), word count, the round trip between modes, the help
  sheet.
- The edit sheet (`E` and `Shift+E` in the reader).
- The keymap with two presets (decision 0003).

Out of scope for now, kept in `TODO.md` with omarky's extracted behaviour as a starting point,
to be specified properly in later passes:

- Changes mode (git versions shown on the page).
- Rewrite with Claude (`Ctrl+J`).
- Spell checking.

Not ported: omarky behaviour that exists only because it is a single-file app (decision 0001),
and anything that is only about omarky's toolkit.
