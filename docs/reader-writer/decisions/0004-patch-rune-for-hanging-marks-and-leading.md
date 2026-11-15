# 0004 Patch Rune for hanging marks and per-line leading

Status: accepted, 2026-11-13. Built 2026-11-15 (phase 10 E): `El::paragraph_styles` takes a
`ParagraphStyle` per paragraph with `indent`, `first_line_indent`, `hang(bytes)`, `lead(bytes)`,
`line_height` and `space_above`, carried on the fork branch `quire/by-paragraph-fixes` (now `quire/dev` on the fork `overp0werj0int/rune`, decision
0009), pinned by revision. Quire uses `indent` and `hang` with `lead` so far.

omarky's writer hangs heading marks, list markers and quote marks in the left margin, so every
line's text starts at the same edge, while the marks stay real characters you can edit. Heading
lines also get their own size, leading and space above. omarky does this with a paragraph
format: a left margin plus a negative first-line indent, and a line height per block, painted
outside the undo history (omarky decisions 0014 and 0015).

Rune's text field at the pinned revision has neither. A span can change size, weight, colour
and background, but a sized span always takes the body's line-height ratio, and there is no
per-paragraph indent or negative first-line indent. The only indent belongs to Rune's own rich
text blocks, is fixed per kind and is private. (Research report, `crates/rune/src/text.rs` and
`edit/model.rs` at revision `bf3f9d4`.)

A workaround exists: squeeze the mark characters to nearly zero width and redraw them in the
margin. It makes the caret jump over marks, a click on a hanging mark misses the text, and
headings keep the body leading. The user chose the patch after comparing omarky's screenshots.

Decision: Quire carries a small patch to Rune that adds, for a multi-line text field:

1. A per-paragraph left indent and first-line indent, which may be negative, set from the app's
   spans or a per-paragraph style. The caret, selection, hit testing and painting all use it.
2. A per-span or per-paragraph line height, so a heading line can have its own leading and a
   paragraph can have space above it.

Both are layout, not text. They never enter the undo history, and undo only moves text.

The patch stays small and is written so it can go upstream. How it is carried (a fork branch of
Rune pinned by revision, or a vendored crate patched through `[patch]` the way `vendor/winit` is)
is decided in phase 2 by measuring both. The Rune crate is about 200,000 lines, so vendoring the
whole crate is not a given.

The patch is proven before any writer work depends on it: caret movement across marks,
selection, clicks in the margin, IME, undo after typing `- `, and typing latency at 400, 1,000
and 3,000 lines with `dir_by_paragraph` on.
