# Sidebar

The left sidebar of the desktop app, as the user agreed it with a designer on 2026-11-19. It
replaces the sidebar of `ui/sidebar.rs` in full: the Open list, the folder's name, the Files,
Search and Tags tabs and the New note and Settings rows. Decision 0013 says what this overrides
in decisions 0007, 0008 and 0011.

The designer's spec draws five screens for a pointer, in one dark theme. This file turns it
into Quire's terms: Quire's colour roles and text styles, both themes, the keyboard, and what
today's sidebar does that the spec does not mention.

Each item ends with its source:

- `spec`: the designer's spec, by screen (S1 to S5), the interactions table, the sizes and
  colours, or the "removed" list.
- `user`: the user's answers to the spec's open questions, 2026-11-19. They outrank the spec.
- `kept`: what Quire does today, kept as it is, with the item or code that does it.
- `[proposed]`, after the verify tag: a choice made here that nobody confirmed. The item says
  what the other choice was.

All items are `adapted: 0013`, or `dropped` for what goes. Sizes are in points at scale 1.

## Regions and design rules

- **SB-01** [adapted: 0013] [verify: geometry] The sidebar is one column 248 pt wide, docked or in its drawer. It runs the window's full height with a 1 pt line (`Pal::line`) on its right edge. Top to bottom it holds the window corner's room (`titlebar::corner_room`), the top bar, the Open shelf, the handle, the vault section and the footer (spec: What it is; kept: width and edge, decision 0007).
- **SB-02** [adapted: 0013] [verify: golden, unit] Two surfaces. The shelf sits on the shelf surface, which is darker than the panel surface under the top bar, the vault and the footer, in both themes. The two lists cannot be mistaken for each other (spec: design rules).
- **SB-03** [adapted: 0013] [verify: state, golden] One filled row. Only the active tab has the selected fill, and in search only the row Enter would open (SB-61). The active note in the vault is marked by bold, strong text alone: no fill and no accent. A hover fill under the pointer is not a mark (spec: design rules, S1).
- **SB-04** [adapted: 0013] [verify: geometry] Four controls show at rest: the search field, the "+" button, the tags button and the settings button. Every other control shows on hover or is a row of a list (spec: design rules).
- **SB-05** [adapted: 0013] [verify: geometry] Both section labels, Open and the vault's, have one style: small capitals, a chevron on the left, a count on the right. Search's section labels (Titles, Text, Tags) take the same style with no chevron (spec: design rules, S3).

## Colours, type and sizes

The spec gives values for its mock. Each role maps to a Quire token where one exists. The mock
is 264 pt wide; Quire keeps 248, so names cut 16 pt sooner.

| Role (spec) | Mock, dark | Quire, dark theme | Quire, light theme |
| --- | --- | --- | --- |
| Shelf surface, search field fill | `#14171d` | `Roles::panel(p).shelf` (black at 32 % over `Pal::side`) | `Roles::panel(p).shelf` (the text colour at 4.5 % over `Pal::side`) |
| Panel surface: top bar, vault, footer | `#1b1f27` | `Pal::side`, as today | `Pal::side`, as today |
| Hover | `#222731` | `Roles::panel(p).hover` (text at 4.5 %) | `Roles::panel(p).hover` (text at 4 %) |
| Selected: active tab, filled result | `#2b313c` | `Roles::panel(p).selected` (text at 13 %) | `Roles::panel(p).selected` (text at 11 %) |
| Line: dividers, guides, field border | `#2d333e` | `Pal::line` | `Pal::line` |
| Text, strong | `#e4e6eb` | `Roles::panel(p).text` | the same |
| Text | `#abb0bc` | `Roles::panel(p).dim` | the same |
| Text, dim | `#7d8493` | `Roles::panel(p).faint` | the same |
| Accent: folder icons | `#e0a36a` | `Pal::acc` | `Pal::acc` |
| Accent as text: current entry, matched letters | `#e0a36a` | new `Roles::panel(p).accent_text`: the accent moved toward the text colour until it reads at 4.5:1 | the same rule |

| Element | Mock | Quire |
| --- | --- | --- |
| Row: vault note, folder | 12.5 px mono, about 27 tall, padding 5 and 8, 1 px gap, radius 6 | `styles::nav(12.5, false)` (panel family: iA Writer Quattro S by default, SB-07); 27 pt tall; 8 pt outer inset, 8 pt side padding, 1 pt gap, radius 6 |
| Folder name | weight 500, text strong | `styles::medium(12.5)` (weight 600, resolving to Bold on Quattro S) |
| Active note in the vault | weight 600, text strong | `styles::nav(12.5, true)` |
| Tab | title 12.5 px mono, caption 10.5 px sans, line 1.15, padding 4, about 35 tall | title `styles::nav(12.5, false).lh(14.0)`; caption `styles::sans(10.5, 400).lh(12.0)`; 1 pt gap, 4 pt top and bottom padding, 8 pt side padding, 8 pt outer inset; 35 pt tall |
| Outline entry | 11.5 px, padding 3, about 22 tall, indent 16 with a guide | `styles::nav(11.5, false)`; 24 tall, the least pointer target (SB-08), 1 pt gap; the guide 16 pt in from the tab's edge (x = 24), the text 12 pt past the guide (x = 37) |
| Section label | 10 px sans, 600, capitals, 0.1 em apart; 10 px chevron; count 10 px, not capitals | Instrument Sans 10/1, 600, capitals, 1 pt tracking, faint; `icons::CHEVRON_DOWN` at 10 in faint with 6 pt gap; count Instrument Sans 10/1, 500 in faint; 8 pt inset and side padding; 24 pt tall for clickable labels (SB-08), 25 pt (10 top, 5 bottom) in search |
| Icons | 14 px, 1.5 px stroke, round caps | 14 pt; `icons::{FOLDER, FOLDER_OPEN, TAG, GEAR, PLUS, SEARCH, CLOSE}` from Phosphor Icons (`ui/icons.rs`); `icons::CHEVRON_DOWN` at 10 pt |
| Tree indent | 21 px a level; guide 1 px at the parent icon's centre | 21 pt a level from x = 16; 1 pt guide 14 pt in from the parent row's left edge (x = 22 at level 1); `panel::INDENT` (14) stays for the outline panel |
| Top bar, footer | padding 8; buttons 28 × 28, radius 6; field radius 6; 1 px divider | the mock's values with unified 6 pt outer / 4 pt inner corner tokens; dividers in `Pal::line` |
| Handle | 9 px tall, a line above and below, grip 22 × 2 | the mock's values; lines `Pal::line`, grip in faint, radius 1 |
| Key hint (`Ctrl /`, `Enter`) | 10 px sans, 500, 1 px border, radius 4 | `styles::keys(styles::CAPTION)` (panel family, PAGE-09), 1 pt `Pal::line` border, radius 4 |

- **SB-06** [adapted: 0013] [verify: unit, golden] The sidebar's colours are the roles in the first table, in every theme. `Roles::panel` lifts faint, dim and text against the shelf surface and the selected fill as well as today's fills, so faint keeps 4.5:1 on all of them. The accent as text gets the same lift to 4.5:1 (spec: sizes and colours; kept: decision 0008's contrast rule).
- **SB-07** [adapted: 0013] [verify: geometry, unit] The sidebar's type and sizes are those of the second table. Rows and titles are in the panel family. Tab captions, section labels and their counts are in Instrument Sans. The panel family is a setting kept in the settings file, `panel_font`, with no row in Settings: `quattro`, iA Writer Quattro S, the face the note uses, is the default; `mono` is the system's monospace, as decision 0008 reads it. It sets both side panels, their menus, key names and tooltips. Every row, label and key hint fits its room in both, at 1x and 1.5x (user; spec: sizes; decision 0013 amends PAGE-09 and 0008 for these).
- **SB-08** [adapted: 0013] [verify: geometry] A row's pointer target is at least 24 pt each way. A tab's close button and a folder's "+" are 14 pt icons in 24 pt targets (kept: `sidebar_targets_are_large_enough`).

## Top bar

- **SB-09** [adapted: 0013] [verify: geometry] The top bar holds the search field, which takes the row's width, and the "+" button at its right, 6 pt apart, with 8 pt padding and a 1 pt divider under it (spec: S1, sizes).
- **SB-10** [adapted: 0013] [verify: geometry, state] The field shows a magnifier, then "Search notes" in faint, then the key hint of search from the active preset (`Ctrl+/`) while it is empty. With text, the text is in strong and a "×" clear button takes the hint's place. Its border turns faint while it has focus (spec: S1, S3).
- **SB-11** [adapted: 0013] [verify: script] A click in the field, or Ctrl+/, puts the keyboard in it, in both modes. Ctrl+Shift+G (omarky preset) and Ctrl+Shift+F (quire preset) do the same. A key that focuses it selects the text already there. A hidden sidebar shows first, docked or as the drawer. Ctrl+P and Ctrl+F keep their meanings (user; spec: S1; kept: `App::open_search`).
- **SB-12** [adapted: 0013] [verify: script] The "+" button and Ctrl+N make a note in the active tab's folder, or at the vault root with no tab. It opens in a new tab at the end of the shelf, and the note's name field opens in the title bar, as New note does today. Its tooltip is "New note" with its key (spec: S1, interactions; kept: `folders::new_note_in`).
- **SB-13** [dropped] [verify: -] Settings' new notes folder. "+" and Ctrl+N follow the open note (SB-12), so the setting goes, with its row in Settings. A settings file that still names a folder loads, and the folder is ignored (user).

## Open shelf

- **SB-14** [adapted: 0013] [verify: state] The shelf starts with the label "Open", a chevron pointing down and the number of tabs (spec: S1).
- **SB-15** [adapted: 0013] [verify: state, geometry] Each open note has one tab of two rows: the note's name without ".md", then a caption with its folder's path from the vault root. A note at the root shows the vault's name. The caption is in faint, the title in Text, the active tab's title in strong (spec: S1).
- **SB-16** [adapted: 0013] [verify: geometry] The active tab has the selected fill and always shows its close button at its right edge. Another tab shows its close button only while the pointer is over it (spec: S1, and the drawings, which give no other tab a close button).
- **SB-17** [adapted: 0013] [verify: state] The active tab's caption adds " · N sections", N being its note's H1 headings, with "1 section" for one. A note with no H1 adds nothing. No other tab has the count (spec: S1; user).
- **SB-18** [adapted: 0013] [verify: state] Tabs keep the order the user gives them. A newly opened note joins at the end. Opening a note that has a tab switches to that tab (spec: S1; kept: `App::open_file`).
- **SB-19** [adapted: 0013] [verify: script] A click on another tab switches to it. A click on the active tab folds its outline out or back (SB-26). A middle click on any tab closes it. A click leaves the keyboard where it was (spec: interactions; kept: middle click, KEY-95).
- **SB-20** [adapted: 0013] [verify: unit, script] Closing a tab, by its "×", a middle click, Ctrl+W or `q` in the reader, takes it off the shelf. When it was the active tab, the tab above it becomes active, or the one below when it was first. With no tab left, no note shows. Today the tab below became active (spec: interactions).
- **SB-21** [adapted: 0013] [verify: script] A tab dragged up or down moves within the shelf. Past a few points it lifts and follows the pointer, and the tabs it passes step aside on springs. The drop puts it there. Esc, or the window losing focus, puts it back. A press that does not move is a click (spec: interactions; designer's proposal, built as written).
- **SB-22** [adapted: 0013] [verify: script] [proposed] A drag that lifts a tab folds the active tab's outline back, so every tab is one height while it moves. The other choice was to carry the outline with its tab.
- **SB-23** [adapted: 0013] [verify: geometry] The shelf scrolls when its tabs and outline are taller than it (spec: interactions).
- **SB-24** [adapted: 0013] [verify: geometry] [proposed] When the active tab changes, by a click, a key or a note opened elsewhere, the shelf scrolls just far enough to show it. The other choice was to leave the shelf's scroll alone.

## Outline under the active tab

- **SB-25** [adapted: 0013] [verify: state] Every tab starts with its outline folded. Whether an outline is out is not remembered (spec: S2).
- **SB-26** [adapted: 0013] [verify: script] A click on the active tab folds its outline out, and the next folds it back. A click on another tab only switches to it. Only the active tab can show its outline: switching tabs folds the one that was out (spec: S2).
- **SB-27** [adapted: 0013] [verify: unit, state] The outline lists the note's H1 headings, one level, in order, as the heading's text. A note with one H1 has a one-line outline. A note with none has no outline, and a click on its active tab does nothing (user; spec: S2).
- **SB-28** [adapted: 0013] [verify: script] A click on an entry scrolls the note to its heading, as the outline panel's rows do (`App::jump`), in both modes (spec: interactions; kept: RD-47, WR-100's jump).
- **SB-29** [adapted: 0013] [verify: state] The entry for the section in view is in the accent as text. In preview it is the last H1 at or above the top block and follows scrolling with no view run, as the outline panel does (VER-32). In the writer it is the last H1 at or before the caret. Above the first H1 no entry is marked (spec: S2; kept: RD-47, WR-100).
- **SB-30** [adapted: 0013] [verify: geometry] Entries are smaller than tab titles, in faint, indented 16 pt under the tab with a 1 pt guide line, and never filled, under the pointer either (spec: S2, sizes).
- **SB-31** [dropped] [verify: -] An entry has no hover mark. A text that turned from faint to Text under the pointer was proposed, but the hover tint paints over the live accent of the current entry in preview, so that entry lost its mark while the pointer was on it (stage 5, probe).
- **SB-32** [adapted: 0013] [verify: state] The outline panel on the right is unchanged and still lists H1 to H4 (spec: S2).

## Handle and split

- **SB-33** [adapted: 0013] [verify: geometry] The handle sits between the shelf and the vault: 9 pt tall, a 1 pt line above and below, a 22 × 2 pt grip at its centre. Over it the pointer is the row-resize cursor (spec: S1, sizes).
- **SB-34** [adapted: 0013] [verify: script] Dragging the handle moves the split. The shelf takes the height above it and the vault the rest (spec: interactions).
- **SB-35** [adapted: 0013] [verify: geometry] [proposed] Until the handle is first dragged, the shelf is as tall as its tabs and outline, up to half the room between the top bar and the footer. After a drag the shelf keeps the dragged height. The other choice was a fixed share of the room.
- **SB-36** [adapted: 0013] [verify: geometry] [proposed] The shelf keeps at least its label and one tab, and the vault at least its label and two rows. A drag stops at those limits. The other choice was to fold a section when the drag passes its limit.
- **SB-37** [adapted: 0013] [verify: state] [proposed] The handle is for the pointer only and is not a Tab stop. From the keyboard a section folds instead (SB-41), and each list scrolls its focused row into view. The other choice was a focusable splitter that Up and Down move one row at a time.
- **SB-38** [adapted: 0013] [verify: geometry] The handle is hidden while a section is folded and while search or tags show (spec: S4).

## Folding the sections

- **SB-39** [adapted: 0013] [verify: script] A click anywhere on a section's label row, chevron or name, folds that section to one row, and a click on the folded row unfolds it. The split is the default; folding happens only on a click (user; spec: S4, interactions).
- **SB-40** [adapted: 0013] [verify: state] Both sections are never folded at once: folding one unfolds the other (spec: S4).
- **SB-41** [adapted: 0013] [verify: script] [proposed] Each section label is a Tab stop with the role of a button that is expanded or not. Enter or Space folds or unfolds its section, and the keyboard stays on it. The other choice was a fold key inside the lists.
- **SB-42** [adapted: 0013] [verify: geometry] The folded vault is one row directly above the footer, with a 1 pt line above it: a chevron pointing right, the vault's name and "N notes". The shelf takes the full height, and the handle is hidden (spec: S4).
- **SB-43** [adapted: 0013] [verify: state] Unfolding the vault restores the split at the handle's previous place (spec: S4).
- **SB-44** [adapted: 0013] [verify: geometry] The folded shelf shows its label with a chevron pointing right and "N tabs", then only the active tab, still filled, closable and able to fold its outline out. A 1 pt line runs under it. The vault takes the rest (spec: S5).
- **SB-45** [adapted: 0013] [verify: state] Opening a note from the vault while the shelf is folded adds its tab and makes it the one shown. The shelf stays folded (spec: S5).
- **SB-46** [adapted: 0013] [verify: state] The fold is remembered between sessions (spec: S4).
- **SB-47** [adapted: 0013] [verify: state] One fold and one handle place serve every folder, kept in `state.json`, not in each folder's session (user).

## Vault

- **SB-48** [adapted: 0013] [verify: state] The vault's label is its name, in the label style, with its total note count on the right (spec: S1).
- **SB-49** [adapted: 0013] [verify: script] A folder row has a folder icon in the accent, closed or open, and no chevron. A click on the row opens or closes the folder, and the icon changes (spec: S1, interactions).
- **SB-50** [adapted: 0013] [verify: geometry] Each level is indented 21 pt. Root rows start at the list's left padding edge. A 1 pt guide line drops from the centre of an open folder's icon beside its rows (spec: S1, sizes).
- **SB-51** [adapted: 0013] [verify: state] A folder row shows its note count on the right in faint (spec: S1).
- **SB-52** [adapted: 0013] [verify: unit] [proposed] A folder's count includes the notes in its subfolders. The other choice was the folder's own notes only (spec's suggestion, taken by the orchestrator).
- **SB-53** [adapted: 0013] [verify: script] While the pointer is over a folder row, a "+" replaces its count. A click on it makes a note in that folder, opened in a new tab at the end, with its name field as SB-12 (spec: S1, interactions).
- **SB-54** [adapted: 0013] [verify: state] In a folder, subfolders come first, then notes, each in today's order: by name, ignoring case. Notes have no icon. The root follows the same rule (spec: S1; kept: `natural`).
- **SB-55** [adapted: 0013] [verify: state] Folder names are in strong at weight 600 (`styles::medium`, resolving cleanly on 400/700 families like Quattro S so folders stand out above regular note rows). Note names are in Text. The active tab's note is bold and in strong, with no fill and no accent (spec: S1, design rules).
- **SB-56** [adapted: 0013] [verify: script] A click on a note switches to its tab if it has one, or opens it in a new tab at the end. Opening a note opens the folders above it (spec: interactions; kept: `App::open_file`).

## Search

- **SB-57** [adapted: 0013] [verify: script] As soon as the field holds text other than spaces, one result list replaces the shelf, the handle and the vault. The top bar and the footer stay. Clearing the field, by "×", Esc or deleting its text, brings both lists back exactly as they were: folds, split, scroll and open folders (spec: S3).
- **SB-58** [adapted: 0013] [verify: state] The list has three sections, Titles, Text and Tags, in that order, each under a label with its count. A section with no match is left out. There are no filter chips or tabs (spec: S3).
- **SB-59** [adapted: 0013] [verify: state] Titles lists the notes whose name holds the query, ignoring case, in the tree's order. Text lists the notes whose text holds it, one row each, most matches first, then by path. A locked note's text is not searched; its name is (spec: S3; kept: `md::search`'s matching and order).
- **SB-60** [adapted: 0013] [verify: state] [proposed] A note listed under Titles is not listed again under Text. The other choice was to list it in both.
- **SB-61** [adapted: 0013] [verify: geometry, state] Title and Text rows use the tab's two rows: the name on top, and below it the folder (Titles) or the first matching line cut at words, as `md::snippet` cuts it today (Text). The matched letters are in the accent as text (spec: S3; kept: `md::snippet`).
- **SB-62** [adapted: 0013] [verify: state] A Tags row shows a 12 pt tag icon, the tag, and "N notes" on the right. Tags are listed by how many notes carry them, most first (spec: S3; kept: `md::tags`' order).
- **SB-63** [adapted: 0013] [verify: state] A note that has a tab says " · open" in its caption. Choosing it switches to its tab. Search never opens a second tab for a note (spec: S3).
- **SB-64** [adapted: 0013] [verify: script] Choosing a Titles row opens or switches to the note, in the current mode. Choosing a Text row does the same, at its first match, and stays in the current mode. In the writer the caret goes to the start of the match, with nothing selected. In preview the note scrolls to the match and marks it as find marks it. Reading stays reading (user; spec: S3; kept: KEY-98's caret at the match). Today a match always opened in the writer.
- **SB-65** [adapted: 0013] [verify: state] The row Enter would open has the selected fill and an "Enter" key hint at its right. While the field has the keyboard it is the first row. While a row has the keyboard it is that row (spec: S3).
- **SB-66** [adapted: 0013] [verify: script] Enter in the field opens the filled row. Down in the field moves the keyboard to the first row. Up and Down move between rows. Up from the first row moves the keyboard back to the field (spec: S3; kept: KEY-98's Down and Up).
- **SB-67** [adapted: 0013] [verify: state] Search never creates a note. There is no "create note" row (spec: S3).
- **SB-68** [adapted: 0013] [verify: script] The "×" clears the field and leaves the keyboard in it (spec: S3; kept).
- **SB-69** [adapted: 0013] [verify: state] With no match, the list shows "No matches" in faint and nothing else, and Enter does nothing (kept: today's text).
- **SB-70** [adapted: 0013] [verify: script] [proposed] The results stay after a row is chosen, so several can be opened in turn. The lists come back only when the field is cleared. The other choice was to clear the field on a choice.
- **SB-71** [adapted: 0013] [verify: script] The results follow the notes: a note saved, renamed, moved, deleted or synced in changes them on the next frame, with no new keystroke (kept: today's search reads the notes each frame).
- **SB-72** [adapted: 0013] [verify: script] [proposed] A row that has the keyboard and leaves the results hands the keyboard to the field. The other choice was the note.
- **SB-73** [adapted: 0013] [verify: state] Searching while a section is folded shows the results all the same, and clearing brings the fold back (spec: S3, "exactly as they were").

## Tags

- **SB-74** [adapted: 0013] [verify: script] A click on the footer's tags button shows the list of all tags in place of the shelf and the vault, in the search layout: one Tags section with every tag and its note count (spec: interactions).
- **SB-75** [adapted: 0013] [verify: script] [proposed] The tags button shows the list by putting "#" in the search field, with the keyboard in it and the caret after it, so typing narrows the tags, and Esc or "×" brings the lists back. Clicking the tags button while the tags show clears the field too. The other choice was a tags view of its own with the field empty.
- **SB-76** [adapted: 0013] [verify: unit, script] [proposed] A query that starts with "#" searches tags only. Tags lists the tags that hold the rest of the query. When the rest is a whole tag, a section "Tagged" follows, listing the notes that carry it as two-row rows with their folder. Titles and Text are left out. The other choice was today's: a text search for "#tag", which also finds the tag in code and in longer tags.
- **SB-77** [adapted: 0013] [verify: script] Choosing a tag lists its notes: the field becomes "#" and the tag, and the keyboard, when it chose by keys, goes to the first note (spec: S3; kept: today's tag row puts the keyboard on the results).

## Footer

- **SB-78** [adapted: 0013] [verify: geometry] The footer holds the tags button on the left and the settings button on the right, and nothing else, under a 1 pt divider. The buttons are 28 × 28 with radius 6, their icons in Text, the hover fill under the pointer. Their tooltips are "Tags" and "Settings" with its key (spec: S1, sizes).
- **SB-79** [adapted: 0013] [verify: script] The settings button and Ctrl+, open Settings (spec: interactions; kept: KEY-14).

## Keyboard and focus

The spec is drawn for a pointer. These items follow Quire's focus rules (KEY-95, KEY-98, the
focus model in `ARCHITECTURE.md`) wherever the spec says nothing.

- **SB-80** [adapted: 0013] [verify: script] Tab goes through the docked sidebar in this order: the search field, its "×" while it shows, "+", the Open label, the shelf's list, the vault's label, the tree, the tags button, the settings button, then the note. Shift+Tab goes back. A folded vault leaves its tree out. While search or tags show: the field, "×", "+", the results, tags, settings (kept: one stop each, KEY-98).
- **SB-81** [adapted: 0013] [verify: script] Each list is one Tab stop and enters at its current row: the shelf at the active tab, the tree at the active note's row while it shows, else its first row, the results at the filled row (kept: KEY-95).
- **SB-82** [adapted: 0013] [verify: script] In the shelf, Up and Down move through the tabs and, while it is out, the active tab's outline entries in their place, stopping at the ends. Home and End go to the first and last row. A letter moves to the next row that starts with it (kept: KEY-95).
- **SB-83** [adapted: 0013] [verify: script] Enter or Space on another tab switches to it and gives the note the keyboard. On the active tab it folds the outline out or back, and the keyboard stays on the tab. On an entry it jumps to the heading and gives the note the keyboard (kept: KEY-95's "Enter does what a click does").
- **SB-84** [adapted: 0013] [verify: script] [proposed] Right on the active tab folds its outline out, and Left folds it back. Left on an entry moves to its tab. Right on another tab does nothing. The other choice was Enter alone.
- **SB-85** [adapted: 0013] [verify: script] [proposed] Alt+Up and Alt+Down on a focused tab move it one place, and the keyboard stays on it. The other choice was Ctrl+Shift+PageUp and PageDown on the active tab from anywhere, as browsers do.
- **SB-86** [adapted: 0013] [verify: script] [proposed] Delete or Backspace on a focused tab closes that tab. The keyboard moves to the tab that takes its place by SB-20's rule, or to the note when none is left. The other choice was Ctrl+W alone, which closes the active tab from anywhere (kept: KEY-20).
- **SB-87** [adapted: 0013] [verify: script] The tree keeps today's keys: Up, Down, Home, End and letters move; Right opens a closed folder or moves into an open one; Left closes an open folder or moves to the row's folder; Enter or Space does what a click does; the Menu key or Shift+F10 opens the row's menu below it (kept: KEY-95).
- **SB-88** [adapted: 0013] [verify: script] In the results, Up, Down, Home, End and letters move. Enter or Space on a note's row chooses it and gives the note the keyboard. On a tag's row it lists the tag's notes, with the keyboard on the first (kept: KEY-95).
- **SB-89** [adapted: 0013] [verify: script] In the search field, Down and Up act as SB-66 says and Enter as SB-66 says. Esc with text clears it and leaves the keyboard in it. Esc with the field empty gives the note the keyboard. The field keeps its own editing keys, and the writer's edits at the caret do nothing from it (brief; spec: S3, interactions; kept: KEY-98).
- **SB-90** [adapted: 0013] [verify: script] [proposed] Esc on a result row clears the search and puts the keyboard in the empty field, with its ring. So in search the first Esc clears and the second leaves, from the field or a row. The other choice was to leave at once and keep the query.
- **SB-91** [adapted: 0013] [verify: script] Esc on any other stop of the docked sidebar gives the note the keyboard (kept: KEY-98).
- **SB-92** [adapted: 0013] [verify: script] In the drawer, every Esc that would give the note the keyboard closes the drawer instead, and focus goes back where it was before the drawer opened. Esc that clears the search clears it and the drawer stays (kept: KEY-23, KEY-98).
- **SB-93** [adapted: 0013] [verify: script] While a notice shows, the first Esc from any stop dismisses it and the keyboard stays where it was, docked or in the drawer (kept: KEY-23, KEY-98).
- **SB-94** [adapted: 0013] [verify: script] Esc with Shift, Ctrl or Alt acts as Esc on every stop of the sidebar (kept: `keys::esc_again`).
- **SB-95** [adapted: 0013] [verify: geometry] A focused stop wears the focus ring, apart from the selected fill: a glass card in the text colour, as the map's selection is (MAP-055), reaching 5 pt past the stop (a 2 pt ring 3 pt out), with a top-lit rim, a faint sheen and a soft shadow; every keyboard focus ring in the app draws this way. When the keyboard moves to another stop under the same clip (the next row, the next button in a bar), the card slides there over `180 ms`, easing out, as the map's glass slides between nodes; under reduced motion, or into another clip, it jumps. Each list leaves room for the ring at its ends and scrolls a row in far enough to show it whole (kept: `panel::RING_ROOM`).
- **SB-96** [adapted: 0013] [verify: script] A click on a tab, a row, a label or a button leaves the keyboard where it was. A click in the search field takes it (kept: KEY-95, KEY-98).
- **SB-97** [adapted: 0013] [verify: script] [proposed] F6 into the sidebar lands on the filled result while search or tags show, else on the active tab, else on the tree's current row, else on the search field. The other choice was the tree's row before the tab (kept: KEY-96's pane order).
- **SB-98** [adapted: 0013] [verify: script] From any stop, the sidebar's key (KEY-11) hides the sidebar, and a shortcut with Ctrl, Alt or the logo key acts as it would from the note (kept: KEY-98).
- **SB-99** [adapted: 0013] [verify: script] When the element that has the keyboard goes away, a tab closed, a row gone, its section folded, the note gets the keyboard, except where SB-72, SB-86 and SB-90 say otherwise. Folding from a label leaves the keyboard on the label (kept: `focus_return(NOTE)`).
- **SB-100** [adapted: 0013] [verify: unit, script] Ctrl+/ runs search (KEY-10) in both presets, beside Ctrl+Shift+G or Ctrl+Shift+F. Ctrl+? is bound with it, as KEY-03 pairs shifted symbols (user).
- **SB-101** [adapted: 0013] [verify: unit, script] F1 toggles the writer's help sheet, which Ctrl+/ toggled before. The command palette still lists it (user).
- **SB-102** [adapted: 0013] [verify: unit, script] [proposed] F1 also opens the reader's help sheet, beside `?`, and help closes on Esc, `?`, `q` or F1. Ctrl+/ and Ctrl+? no longer close it. The other choice was F1 for the writer only.

## What today's sidebar does

Each part of today's sidebar that the spec does not mention is kept, with its place in the new
layout, or dropped, with why that is safe.

- **SB-103** [dropped] [verify: -] The Files, Search and Tags tabs. Files is the vault section, Search the top field, Tags the footer's button (spec: removed).
- **SB-104** [dropped] [verify: -] The New note and Settings rows at the foot, with their keys always shown. They are the "+" and the settings button, whose tooltips name the keys (spec: removed).
- **SB-105** [dropped] [verify: -] The folder's name in bold with its chevron, and the menu a click on it opened. The vault's name is the vault's label, and the menu opens from it on a right-click (SB-110) (spec: removed).
- **SB-106** [dropped] [verify: -] The new folder button. New folder stays in four places: a folder's menu ("New folder here"), the tree's empty space, the vault label's menu and the palette (spec: removed; user).
- **SB-107** [dropped] [verify: -] The accent wash on the active note's row in the tree. The tab is the one filled row (spec: removed).
- **SB-108** [dropped] [verify: -] The Settings item of the folder's menu. The settings button and Ctrl+, open Settings.
- **SB-109** [dropped] [verify: -] The search panel's summary line ("3 matches in 2 notes", "Searches every note in this folder") and its list of up to four hits a note with "+ N more". Each section has its count, and a note is one row (spec: S3).
- **SB-110** [adapted: 0013] [verify: script] A right-click on the vault's label, or the Menu key or Shift+F10 on it, opens its menu below the folder's path: "Switch folder…" and "New folder", which names a folder at the root. Both are in the command palette too (user; kept: the path head).
- **SB-111** [adapted: 0013] [verify: script] [proposed] New folder from the vault label's menu, while the vault is folded, unfolds it first. The other choice was to refuse.
- **SB-112** [adapted: 0013] [verify: script] The tree's menus keep their items. A folder: New note here, New folder here, Delete folder, which asks first when the folder holds notes. A note: New note here, Rename, Move to folder root (away from the root), Delete note. The tree's empty space: New note, New folder (kept: `ui/menu.rs`, `tree`).
- **SB-113** [adapted: 0013] [verify: script] A new folder's name field opens in the tree, at its place among the folder's rows. Enter makes the folder and Esc drops it, with today's keyboard rules (kept: KEY-98, `App::end_new_folder`).
- **SB-114** [adapted: 0013] [verify: script] Rename stays where it is: a click on the note's name in the title bar, the tree menu's Rename and the palette. A renamed note's tab keeps its place (kept: WR-180).
- **SB-115** [adapted: 0013] [verify: script] Delete stays where it is: the tree menu and the palette, with the toast's Undo and Ctrl+Shift+R. Undo puts the note's tab back at its place, active again if it was (kept: KEY-94, `folders::Deleted`).
- **SB-116** [adapted: 0013] [verify: script] A note dragged in the tree onto a folder row, or a note row of that folder, moves there. Onto the vault's label or the tree's empty space it moves to the root. The shelf, the top bar, the footer and the results take no drop. A tab dragged on the shelf reorders and moves no file (kept: `folders::drag_note`).
- **SB-117** [adapted: 0013] [verify: geometry] A name or folder cut short offers its whole path in a tooltip, and one shown whole offers none (kept: C7, `name_tip`).
- **SB-118** [adapted: 0013] [verify: state] The title bar's magnifier stays. It is find in the note, shown in preview, not the old Search tab (spec: removed, "if it only opened the old Search tab").

## Note mode and the drawer

- **SB-119** [adapted: 0013] [verify: state] [proposed] In Note mode (decision 0011) the sidebar holds the window corner, the shelf and a footer with the settings button alone. There is no search field, "+", handle, vault or tags button. The Open label has no chevron and does not fold. A tab's caption is its file's folder, with `~` for the home folder, cut at its start. The other choice was a top bar with the field greyed out.
- **SB-120** [adapted: 0013] [verify: script] [proposed] In Note mode the active tab's outline folds out as in Folder mode. The other choice was no outline in Note mode.
- **SB-121** [adapted: 0013] [verify: geometry] Below 680 pt the same sidebar opens as the drawer over the page's left edge, 248 wide, over the scrim, sliding in from the edge. The title bar and the window corner stay live, and the drawer closes as today (kept: `ui::drawer`, `App::drop_stale_layers`).
- **SB-122** [adapted: 0013] [verify: script] In the drawer, a row, tab or result that shows a note closes the drawer and gives the note the keyboard, even when that note showed already. Folding an outline, folding a section and dragging a tab or the handle keep the drawer open (kept: KEY-98, `lists::opener`).
- **SB-123** [adapted: 0013] [verify: geometry] The toast, its Undo and the notice centre in the part of the note area the drawer leaves, which takes the sidebar's 248 pt (kept: `reader::foot_room`).
- **SB-124** [adapted: 0013] [verify: geometry] The page leans by the docked sidebar's 249 pt, its width and its line, as PAGE-25 and PAGE-26 say. The width is unchanged, so the column does not move (kept: PAGE-25, PAGE-26).

## Edge cases

- **SB-125** [adapted: 0013] [verify: geometry] With no note open the shelf, its label and the handle are gone, and the vault takes the full height. "+" makes a note at the root (kept: today's Open list goes with no note; spec: S1).
- **SB-126** [adapted: 0013] [verify: state] A folder with no notes shows its label with 0 and "No notes yet." in faint. The tags list with no tags shows "No tags yet. Write #something in a note." (kept).
- **SB-127** [adapted: 0013] [verify: state] Two notes of one name in different folders have two tabs, told apart by their captions, as their results are (spec: S1).
- **SB-128** [adapted: 0013] [verify: geometry] A long title is cut at its end and keeps its start. A long caption cuts the folder's path at its start and keeps its last part, at most 72 pt, and " · N sections" or " · open" always shows whole. A long tree name is cut at its end (kept: RD-32, `last_part`).
- **SB-129** [adapted: 0013] [verify: geometry] A deep tree indents 21 pt a level up to depth 5 (121 pt left edge), keeping readable label room in the 248 pt sidebar so deep rows never turn blank. Guide lines stop at the cap so they stay clear of text, names are cut to fit, and the tree never scrolls sideways.
- **SB-130** [adapted: 0013] [verify: script] The active note deleted, from a menu, the palette, a sync or another app, closes its tab by SB-20's rule (kept: `App::close_tab`).
- **SB-131** [adapted: 0013] [verify: script] The active note renamed or moved keeps its tab and its place, with the new name and folder. In the tree it shows bold at its new place, with the folders above it open (kept: `App::move_note`, rename).
- **SB-132** [adapted: 0013] [verify: geometry] [proposed] A window too short for the handle's place moves the handle up as far as SB-36's limits need, and keeps the stored place, so a taller window puts it back. When the window cannot hold both limits, the vault's label stays and its tree scrolls in what is left. The other choice was to fold the vault.
- **SB-133** [adapted: 0013] [verify: state] A locked note shows in the tree and on the shelf as any note. While it is locked its outline has no entries, and search finds its name but not its text (kept: `md::search`, sealed text).

## Remembered

- **SB-134** [adapted: 0013] [verify: state] `state.json` keeps the fold and the handle's place (SB-46, SB-47). Each folder's session keeps its tab order, active tab and open folders, as today. The outline's fold, the query and the lists' scroll are not kept; opening a folder clears the query, as today (spec: S4; kept: `store::Session`).
- **SB-135** [adapted: 0013] [verify: unit] A `state.json` written before this sidebar opens it split, with the shelf at its fitted height (SB-35) (kept: `serde(default)`).
