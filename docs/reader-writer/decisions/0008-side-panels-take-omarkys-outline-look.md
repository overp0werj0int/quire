# 0008 Side panels take omarky's outline look

Status: accepted, 2026-11-14. Asked for by the user: "Sidebar fonts and spacing should be the same
as omarky".

omarky has one side panel, the outline (`components/Outline.qml`). Its type comes from the shell's
`Style.font` tokens on a 12 px base, in fontconfig's `monospace` family (JetBrains Mono on
Omarchy): captions 10 px, small body 11 px, body 12 px.

Decision: both of Quire's side panels, the file tree on the left and the outline on the right,
take the outline panel's type, spacing and text colours. This overrides 0005's "chrome keeps its
current colours" for the panels' text and row shades only.

- Font: the family `fc-match monospace` names, read once at start. Body 12 px, small body
  11 px, captions 10 px.
- Captions ("Contents", "Linked from", and the Files, Search and Tags switch): bold capitals,
  1 px apart, faint, 20 px from the left and 18 px from the top of their section, 10 px above the
  first row. The open panel's caption is in the text colour.
- Rows: the text's line plus 12 px tall. The shade is inset 10 px from each side with a 3 px
  radius: the hover colour under the pointer, a tenth of the accent on the current row. Text
  starts 22 px in, plus 14 px a level, and ends 18 px from the right edge.
- Outline rows: h1 to h4 only. The top level is 12 px bold dim, deeper levels 11 px between dim
  and faint, the current row in the text colour.
- Tree rows: folders 12 px bold, notes 12 px regular, both dim; the open note in the text colour.
  Folder chevrons sit in the 12 px left of the name. The folder icons are gone; omarky has none.
- The outline panel is filled with a wash of the text colour (2.5 % light, 2 % dark) and edged
  with the soft hairline. The sidebar keeps Quire's window background, because the sheet is a
  card that sits on it.

Not changed: panel widths (248 and 236 px), positions (decision 0007), menus (WR-193).

Update, 2026-11-17: the user asked for Quire's accent dot on the outline's current section
back. The current row keeps omarky's shading and also has a 5 px accent dot 13 px left of its
text, in the row's left padding, so the text stays where omarky puts it (RD-47, WR-100).

Update, 2026-11-17: the captions start 22 px from the left, on the rows' text edge, not at
omarky's 20 px. At 20 px the Files, Search and Tags switch started 2 px left of the rows it
labels.

Update, 2026-11-17: the panels' small text must read in every theme. Their dim and faint move
toward the text colour until faint is at least 4.5:1 against the window, the sidebar and the
outline's wash, and dim a step more (`Roles::panel`). In Paper the inactive tabs and the
footer's keys were 2.25:1 on the sidebar.

Update, 2026-11-17: the panel family now has a role across the chrome (PAGE-09). It is the
family of everything inside the panels and of the menus opened from them, which were in
Instrument Sans inside the monospace sidebar. Key names and literal values take it too, in
every view, so a key looks the same in the sidebar's footer, the palette and the shortcut list.
Dialogs, bars and toasts are Instrument Sans, and IBM Plex Mono is left only as a font the
user can pick for the editor. The panels' own type is unchanged.
