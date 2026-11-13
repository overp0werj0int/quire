# 0003 A configurable keymap with two presets

Status: accepted, 2026-11-13. The default preset is open, see below.

omarky and Quire bind some keys to different actions. Examples: `Ctrl+E` is inline code in
omarky and the mode switch in Quire. `Ctrl+P` is preview in omarky and the command palette in
Quire. `Ctrl+Shift+F` is focus mode in omarky and search all notes in Quire. `Ctrl+\` is the
outline in omarky and the sidebar in Quire.

Decision: key handling goes through one keymap: a table from key chords to named actions, per
context (reader, writer, sheet, find bar, palette, global). Quire ships two presets:

- **omarky**: omarky's bindings as they are. Quire actions that omarky has no key for, or whose
  key omarky uses for something else, get free keys.
- **quire**: Quire's current bindings as they are. omarky actions get omarky's keys where those
  are free, and new keys where they clash.

Settings, Shortcuts picks the preset. The help sheets (`?` in the reader, `Ctrl+/` in the writer)
and the Shortcuts page are generated from the active keymap, so they never disagree with it.
A user keymap file that overrides single bindings is a later step, not part of this port.

The full table is `behavior/keymap.md`. Each action has one ID, and both presets are listed side
by side.

Open: which preset is the default. The working default is **omarky**, because the point of this
port is omarky's experience. Existing users see the change in the release notes and can switch
back in Settings.
