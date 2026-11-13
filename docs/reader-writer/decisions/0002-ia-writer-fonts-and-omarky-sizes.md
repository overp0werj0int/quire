# 0002 iA Writer fonts and omarky's sizes are the defaults

Status: accepted, 2026-11-13.

Decision: Quire bundles iA Writer Quattro S, iA Writer Duospace and iA Writer Mono S (Regular,
Bold, Italic and Bold Italic of each), under the SIL Open Font License. Quattro S is the default
prose font and Mono S the default code font, in both modes. The writer's font cycle steps through
Quattro S, Duospace and Mono S, as in omarky, and the preview keeps the prose font. Headings use Quattro S Bold. Quire's current fonts (Newsreader, Instrument Sans, IBM Plex
Mono, Courier Prime, Spline Sans Mono) stay selectable in Settings.

The default body size is omarky's: 14 px at zoom 1. The size stays configurable. Settings,
Editor keeps a size control, and the zoom keys from omarky (`Ctrl+=`, `Ctrl+-`, `Ctrl+0` in the
omarky keymap) change it and are remembered. Every other size, space and line height is a ratio
of the body size, as in omarky: leading 1.6, the 1.25 heading scale, code at 0.84 of the body,
and the vertical rhythm table in `behavior/page-and-typography.md`.

Faces are loaded by file and mapped to runs by file, never matched by weight. The installed
Quattro S Bold declares the regular weight, and omarky rendered every paragraph bold when it
asked by weight (omarky decision 0003). A test checks that a plain paragraph uses the Regular
face.

The measure is 68 average characters of the prose font at the body size, measured from the font,
not assumed. Wide mode is 100.

When a user picks one of Quire's other fonts, the same ratios apply to it. The measure is
recomputed from that font's own average character width.
