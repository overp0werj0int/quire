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
