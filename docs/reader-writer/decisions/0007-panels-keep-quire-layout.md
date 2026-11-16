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

Update, 2026-11-16: the tab strip left the title bar. The open notes are listed under an Open
heading at the top of the left sidebar, above the folder's name, set in the folder name's type
and in the file tree's row style (`ui/sidebar.rs`, `open_notes`). The note's folder and name, which
sat above the text, moved into the title bar where the tabs were, and still rename the note on
a click. The text now starts PAGE-28's 44 points under the title bar, or 24 under the writer's
toolbar.

Update, 2026-11-17: the sidebar runs the window's full height, with a line along its right edge
from top to bottom, and the title bar spans only the rest. The window's corner (the window dots
on macOS and the sidebar toggle) heads the sidebar when it is open and the title bar when it is
closed, at the same place, so the toggle never moves (`titlebar::corner`). The sheet keeps only
its top border.
