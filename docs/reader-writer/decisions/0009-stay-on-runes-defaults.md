# 0009 Stay on Rune's defaults where a patch buys little

Status: accepted, 2026-11-15.

Quire carried eleven local Rune commits. Upstream Rune moved 277 commits on, and five of the
local ones conflict with it. Each local commit is a cost at every Rune update, so only the ones
that change what you see in an important way stay.

Decision:

- **Caret.** The writer and the edit sheet use Rune's caret in the accent colour: a 1.5 point bar
  that doesn't blink. omarky's is max(2, round(E / 9)) wide with round ends and a blink (WR-175,
  SHEET-029). Dropped `El::caret_style`.
- **Footnote references.** Reversed on 2026-11-24: they are raised as superscript, as omarky
  raises them (PAGE-78), by `Span::sup`, which hands Parley's own `vertical-align: super` through
  and gives the span no leading of its own. It was a few lines, not the patch this decision feared.
- **Where the reader's blocks are.** Quire keeps this itself, so it can later mark the block you
  are in. Each row of the page reports its height and place (`El::on_layout`), and the page map
  (`reader::PageMap`) adds the rows up, with the list's estimate for a row not laid out, then
  shifts the sum by how far the last row laid out really is from it. Blocks near the view are
  exact; far ones are estimates, and a trip to one lands again by its row once the row lays out.
  Dropped `El::on_row_tops` and the patch that let `scroll_key_to` reach rows not built.
- **Jumps and find.** A long jump (RD-24) still cuts to 0.8 of a view short and glides the rest
  in 240 ms. When its target is not laid out yet, the cut is placed again by the row a frame
  later. A find match (RD-60) whose block shows between 20% and 75% of the view stays put; any
  other match is centred, where omarky glides a visible one to 35%. A match in a code line past
  the visible part no longer scrolls the code block sideways. Dropped `Tx::scroll_key_to_with`
  and `Tx::scroll_band`.

Kept for now: the paragraph-span fix (typing speed in long notes and a screen-reader fix), and
the paragraph styles that hang marks in the gutter (decision 0004, under review). The right-click
caret fix stays too: Rune has no public way to find the text position under a point, so Quire
cannot place the caret itself before its own menu opens.

The local Rune branch was first `quire/slim`: upstream `bf3f9d4a` plus five commits. It is now
`quire/dev`: the same five commits rebased onto upstream `1dba3423`, plus a fix that finds a keyed
highlight in a by-paragraph field, so the writer's jumps (WR-102), typewriter scrolling (WR-86) and
find (WR-118) scroll to their line. On the rebase Quire dropped its own touchpad scale (5.3):
upstream's winit now scales by 12, as Chromium does and as omarky did (RD-26).

The user kept the right-click caret patch and chose to carry the paragraph-styles patch locally
rather than send it upstream (2026-11-15).

`quire/dev` is pushed to the fork `overp0werj0int/rune` (2026-11-15), and `desktop/Cargo.toml`
pins its revision for every Rune crate, so Quire builds without a local Rune checkout.
