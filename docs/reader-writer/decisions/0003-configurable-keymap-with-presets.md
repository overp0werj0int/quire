# 0003 A single unified keymap with user overrides

Status: accepted, 2026-11-24 (supersedes the two-preset omarky/quire split from 2026-11-13).

Shipping two separate presets (`omarky` and `quire`) split muscle memory across modes and hid
discoverability. Instead, Quire uses one unified keymap with a keybinding editor in Settings,
Shortcuts, backed by `~/.config/quire/keybindings.json`.

Design rules:

- One table (`desktop/src/keymap.rs`, `docs/reader-writer/behavior/keymap.md`) defines every
  context, action ID and default chord.
- Non-edit modes (`Reader` and `Map`) expose bare single-key shortcuts wherever letters are free
  (`b`, `o`, `p`, `m`, `r`, `z`/`.`, `,`, `s`, `i`, plus navigation, edit and fold keys).
- Mode switches are explicit and idempotent: `Ctrl+E` (Source), `Ctrl+P` / `p` (Preview),
  `Ctrl+M` / `m` (Map). Pressing a mode's own key while already in that mode does nothing.
- `Esc` is one-way out of Edit mode: in Source it dismisses transient overlays then enters
  Preview; in Preview and Map it dismisses transient overlays and stays in the current mode.
- User overrides are stored as `action_id -> [chords]` in `~/.config/quire/keybindings.json`.
  Settings, Shortcuts edits any action interactively with conflict checks, per-action reset and
  reset-all. Every label, tooltip, palette shortcut and help sheet updates live from the active
  bindings.
