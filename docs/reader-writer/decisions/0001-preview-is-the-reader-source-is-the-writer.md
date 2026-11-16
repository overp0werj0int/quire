# 0001 Preview is the reader, source is the writer

Status: accepted, 2026-11-13.

omarky has two apps: a reader that sets a file as a page and never edits it, and a writer that
shows live-formatted source. Quire has one window with two modes, preview and source, inside a
notes folder with a file tree, tabs and an outline.

Decision: Quire keeps one window and two modes. Preview mode gets omarky's reader behaviour:
the page, the blocks, the reader keys, find, the outline that follows, the help sheet and the
edit sheet. Source mode gets omarky's writer behaviour: live formatting, hanging marks, the
editing keys, focus and typewriter scrolling. (The chrome that stepped aside while typing went in
decision 0010.)

omarky's writer preview (`Ctrl+P`, `Esc` back, opened at the paragraph you were in) maps to
Quire's mode switch. Switching keeps the reading position in both directions.

omarky behaviour that exists only because it is a single-file app is not ported where Quire
already has its own answer: opening files, untitled drafts, recent files, new windows, and the
header with the file name. The behaviour files mark these items `quire`.
