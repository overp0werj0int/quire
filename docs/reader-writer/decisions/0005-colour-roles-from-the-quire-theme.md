# 0005 Colour roles come from Quire's theme

Status: accepted, 2026-11-13.

omarky derives every page colour from the current Omarchy theme's foreground, background and
accent: dim, faint and ghost as mixes of foreground into background, washes and hairlines as
alphas, and code colours from the theme's named colours.

Decision: Quire keeps its 14 themes, the Omarchy theme option and the accent picker. The page
roles are derived from the active Quire palette with omarky's factors (`behavior/page-and-typography.md`,
colour roles). The syntax palette is generated in memory from the same palette and rebuilt when
the theme changes, so a theme switch recolours code without a reload. Where a Quire theme lacks
a named colour that omarky reads (green, red, yellow, blue, magenta, cyan, orange), the fallback
order in the behaviour file applies.

Quire's own chrome (title bar, sidebar, settings) keeps its current colours.

Update, 2026-11-17: the modal scrim is no longer the page colour at 72% (omarky's help sheet).
That faded the note while the page stayed as bright as before, so a dialog did not stand out
from it. Every modal layer, the drawers' too, now lays one dark neutral over the window: the
page's colour at 15% of its light, at 40% in a light theme and 60% in a dark one
(`theme::scrim`). The page under it loses at least a fifth of its luminance in every theme.
