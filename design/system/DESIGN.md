# Quire design system

Quire is a Markdown notes app: a Rust desktop app drawn with Rune, and an Android app in
Compose. This describes the desktop design system implemented in `desktop/src/ui/tokens.rs`,
`desktop/src/ui/styles.rs`, and `desktop/src/ui/components/`. `quire-design-system.html` shows
it; `tokens.css` holds the values; `ISSUES.md` tracks settled and remaining findings.

## Character

Warm paper and ink, one violet accent, hairlines instead of boxes, very little shadow. The
chrome is quiet and small (10 to 14 pt) so the note is the loudest thing in the window. Every
action has a key, and the key is shown wherever the action is.

## Colour

A theme is eight colours: page, sidebar, ink, second ink, muted text, faint marks, accent,
error. Every built-in theme, including Paper and Ink, derives its remaining tones through
`theme::build()` and `theme::with_accent()`: the hairline is the page 11% toward ink, hover is
ink at 6%, active is ink at 9%, the soft fill is the page 12% toward the accent, the code fill
is the page 4.5% toward ink. Dark themes use 10%, 5%, 8% and 20%. There are 14 built-in themes,
a fifteenth that follows the Omarchy desktop theme, and ten accents.

Colour is used by role, never by hex. `tokens::Zone` (`Chrome`, `Panel`, `Page`) supplies
zone-matched hover fills, selected fills, and contrast-lifted icon inks (at least 3.2:1 on
every theme). The page and the side panels take roles derived from `roles.rs` and lifted until
they read: faint text at 4.5:1, rows 1.43 times that, current text 1.15 times that again. An
accent is moved toward white or black until it reaches 3:1.

The scrim behind a dialog is the page colour darkened 85%, at 40% over light and 60% over dark.

## Type

Two families in the chrome, each with one job:

- The panel family, iA Writer Quattro S or the system monospace: navigation, menus, tooltips,
  key names, paths and other literal values.
- Instrument Sans: dialogs, bars, the palette, Settings, toasts, the welcome screen.

Named sizes in `ui/styles.rs` (`styles::scale`): 10 caption (capitals, 1 px tracking), 11.5
small and tooltips, 12.5 controls and descriptions, 13 body and fields, 14 row titles, 15
palette input, 17 titles. Key caps use 10 (compact) and 11 (full); count pills use 11.

The note is not chrome. Its fonts are the user's choice and every size is a multiple of the
body size B: `size(step) = round(B × 1.25^step)`, H1 to H4 at steps 3, 2, 1 and 0.5, body line
1.6, headings 1.25 and 1.35, code 0.84 B, quotes 0.94 B.

## Shape, spacing and size

All tokens live in `desktop/src/ui/tokens.rs`:

- Spacing grid: 1, 2, 4, 6, 8, 10, 12, 16, 24, 32 with semantic gap and padding roles.
- Concentric radius scale: `INNER` (4) inside controls and key caps, `CONTROL` (6) for icon
  buttons, rows, bars, segments and panel search, `FIELD` (8) for text buttons, dialog inputs,
  segmented tracks, cards and menus, `COLUMN` (10) for board columns, `DIALOG` (12) for every
  modal dialog, `PILL` (999) for toasts, notices, counts and switches.
- Control heights (4 pt steps): `CONTROL_BAR` (24) in bars and pills, `CONTROL_PANEL` (28) in
  panels, segments, switches and row buttons, `CONTROL_DIALOG` (32) in dialog buttons and
  inputs. Menu rows are 30; title bar is 40; side-panel rows share 28 with 8 inset and text at
  16; both panel footers share `FOOTER_H` (41).
- Icon sizes couple to button size: 12 in 24 pt mini-bars and find bar, 14 in 24 pt title bar
  and 28 pt panel or dialog close buttons, 16 in the 28 pt formatting toolbar, 10 for fold
  chevrons.
- Disabled uses 38% opacity for icons and plain labels and 45% for bordered buttons, distinct
  from busy async buttons at 65% fill with a busy label.
- Pointer targets are at least 24 by 24. Borders are 1 in the hairline colour.

## Elevation

`tokens::Elevation` defines six theme-aware elevations (warm brown on light themes, neutral
black on dark themes): `Toolbar` (mini-bars, formatting toolbar, toasts), `Popup` (menus, find
bar, layout picker), `Dialog` (Settings, Lock, Trash, Palette, Help, Edit sheet, Lightbox),
`Segment` (raised segment), `Knob` (switch knob), and `SelectionGlass` (Map/Board/picker
sliding glass selection lift). Panels have no shadow, only a hairline.

## Motion

Named constants in `tokens::motion`: hover fades in 0.1 s, dialogs fade with spring 0.08,
board drag tilt uses spring 0.12, in-place swaps and drawers use spring 0.16, tabs slide and
board layouts glide over 0.18, zen bar reveals over 0.25.
Under reduced motion everything jumps. A toast stays 1.6 s, Undo 6 s, and armed destructive
buttons lapse after 2 s in both the edit sheet and Trash.

## Components

One module per component in `desktop/src/ui/components/`: `button.rs` (`Primary`, `Outline`,
`AccentOutline`, `Destructive`, `ArmedDestructive`; sizes `Dialog` 32, `Sm` 28, `Bar` 24),
`icon_button.rs` (`Mini`, `Bar`, `Panel`, `Toolbar` with `ToggleKind`), `input.rs` (`Dialog` 32 /
r=8, `Panel` 28 / r=6, password, literal, secret and area helpers), `choice.rs` (`choice_card`,
`choice_pill`, `checkbox_box`), `switch.rs`, `segmented.rs` (`segmented` 28 / r=6 inside 32 track,
`pill_group` 24 / r=6 inside hairline track), `selection_glass.rs` (`SelectionGlass`, `GlassBackdrop`,
`GLASS_R` — single sliding glass selection outline + zoom shared by Map, Board, Layout Picker,
Settings theme/font/provider grids, and Command Palette), `kbd.rs` (`kbd`, `key_hint`), `badge.rs`
(`count_pill`, `tag_badge`, `status_dot`, `progress_dot`, `error_line`), `mini_bar.rs`, `mini_card.rs`
(`MiniCard` — modal mini-cards with whole-header & whole-footer hover zones, top-right hover-revealed
icon-only `mini_bar`, and `×` always rightmost), `dialog.rs`
(`dialog_frame`, `dialog_plane`, `popup_frame`, `bar_edge`, `faded`, `faded_h`), and `chrome.rs`
(`Control`, `label`, `padded`, `bar_sep`, `dots`, `name_tip`).

## Mini-card

Modal mini-cards (Edit sheet, Kanban card preview/edit, Keyboard help, and Map lightbox) compose
through `ui/components/mini_card.rs` (`MiniCard`):

- Three borderless bands inside `dialog_frame`: **header** (hover area), **body** (not a hover area,
  so reading and typing keep chrome invisible), and **footer** (hover area).
- Header and footer share one `hover_group_shared` group; hovering anywhere in either band (or the
  bar itself) reveals a single top-right `mini_bar` on `motion::zen_spring()`. The bar stays pinned
  visible when discard is armed or an error/conflict line is shown.
- All actions in the mini-bar are icon-only `mini_button`s (`24 × 24`, `12 pt` icons, `"Action · Key"`
  tooltips) — never text `Button`s and never a permanently visible standalone close `×`.
- **`×` (Close / Cancel / Discard) is always the rightmost button** (`MiniCard::close` appends it
  last: `[✓ Save] [× Cancel/Discard]`, `[✎ Edit] [× Close]`, `[⚙ Customize] [× Close]`,
  `[←] [→] [↗] [✎] [×]`).
- Settings uses a fixed height (`min(vh - 48, 720)`) so switching sections never resizes the dialog.

## Interaction

- Layers. Menus, drawers and the layout picker are light: Esc or a click outside closes them
  and focus goes back where it was. The palette, Settings, lock, trash, help, edit sheet and
  lightbox are heavy: they sit on the scrim and keep Tab inside.
- Focus. `tokens::FocusRing` defines four named rings (`Standard`, `Tight`, `Input`, `Inset`).
  Every dialog names its initial focus target. A list is one Tab stop; arrows move inside it;
  F6 moves between panes.
- Esc closes the nearest thing: a notice, then find, then an inline field, then the layer. In
  the palette and help search, Esc clears a non-empty query before closing the layer.
- Keys are written `Ctrl+Shift+K`. A tooltip reads "Name (Key)".
- Destructive actions take two steps: the button arms (`ArmedDestructive`) and lapses after 2 s.
- Words: "note", never "document". Sentence case.
