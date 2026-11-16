# 0010 The chrome stays while typing

Status: accepted, 2026-11-16.

omarky fades its header, outline and word count while you type, and brings them back after a
1400 ms pause (WR-184 to WR-187). Quire took this in phase 7 and widened it to the title bar,
the floating toolbar, the status bar and the file tree.

In use, every keystroke faded the sidebar, the outline and the bars out, and every pause brought
them back, so the window flickered around each new paragraph. The user called it a bug and
chose that nothing fades.

Decision: the chrome stays fully drawn while typing. WR-185 is adapted to that; WR-184, WR-186,
WR-187 and WR-103 are dropped, and WR-190 and WR-222 lose their fading. Quire's focus mode
(WR-223) remains the way to a bare page.

Test: `writer_chrome_tests::typing_leaves_the_chrome_in_place` presses Enter in source mode and
checks that the title bar, file tree, toolbar and status bar stay at full opacity.
