# 0007 Panels keep Quire's layout, the column leans from both sides

Status: accepted as the working default, 2026-11-13. Open to change.

omarky has one side panel, the outline, on the left. With the outline open the column leans back
toward the window's centre by up to half the outline's width, but never past the golden-section
point of the free space.

Quire has the file tree on the left and the outline on the right.

Decision: keep Quire's layout. Generalise the lean: `lean` is the left panel's width minus the
right panel's width, and the column shifts by half of it toward the window's centre, with the
same golden-section limit on the side it moves toward. With no panels, or equal panels, the
column is centred in the window. omarky's outline key toggles Quire's outline wherever it is.

Open: moving the outline to the left to match omarky exactly. That would put two panels on the
left when the file tree is open.
