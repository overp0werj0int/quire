# 06 Changes mode

Reference behaviour from omarky's reader (changes mode, folded in from OmaChanges in OmaRead 0.4.0). Paths are relative to `/home/p0werj0int/Projects/omarky`. Items marked (inferred) were derived from the code's logic rather than stated in it.

Units and colour roles are as in `05-edit-sheet-and-rewrite.md`: `caption` 10 px, `bodySmall` 11 px, `body` 12 px chrome; reading body 14 px at zoom 1; corner radius 0 by default so `max(6, radius)` is 6; `dim`, `faint`, `ghost`, `hairline`, `hairlineSoft`, `hover` and `flat(c, a)` as defined there (`services/Theme.qml:44-77`, `Commons/Style.qml:327-329`, `services/Reading.qml:47-48`). Theme hues (`green`, `red`, `blue`, `yellow`, `magenta`, `cyan`) come from the active theme's named colours, falling back to the accent or urgent colour (`services/Theme.qml:32-38`). No mark uses a literal red or green. `docs/superpowers/plans/2026-10-25-omachanges.md:20`

## Concepts

CHG-001. A version is the file's text at one moment: a git check-in that touched the file, or "Now", the file on disk whether committed or not. A comparison has two sides, from (older) and to (newer). The page always reads as "to compared with from", and to defaults to Now. `docs/superpowers/specs/2026-10-25-omachanges-design.md:30-38`

CHG-002. Versions are shown as a person and a time, never as a commit hash, ref or branch. `docs/superpowers/plans/2026-10-25-omachanges.md:19`

CHG-003. Changes mode is a mode of the reading window, not a separate window. The column shows the to text with changes marked, and `Esc` returns to plain reading at the same place. `docs/decisions/0012-changes-as-a-mode-of-reading.md:7`

CHG-004. The window never runs git. A launcher answers two questions: the file's history and shortcuts (`--versions`), and one resolved side with its text (`--resolve SPEC`, optionally bounded by `--to`). `docs/decisions/0013-git-behind-the-launcher.md:7`, `reader/ui/app/ReaderWindow.qml:202-238`

## Knowing the history

CHG-005. When a file opens, the window asks for its versions: whether it is in a git work tree, whether it is dirty, the history newest first, the shortcut list, and the git user name. Outside a work tree the history is empty. `reader/ui/app/ReaderWindow.qml:215-220,268-272`, `launcher/omarky/versions.rb:19-26`

CHG-006. Dirty means the file on disk differs from its newest check-in. `launcher/omarky/versions.rb:22`

CHG-007. The versions are asked for again 400 ms after any reload of the page (debounced), and after the compared file changes on disk. `reader/ui/app/ReaderWindow.qml:199,221,306`

CHG-008. History comes from `git log --follow` on the file, with author name, author email, author date in local time, subject and body. A version's text is the file's blob at that commit, following renames. `launcher/omarky/history.rb:27-47`

CHG-009. A version's short name ("who") is the author's first name. A one-word git handle reads as the capitalised first segment of the email's local part when that segment is at least 3 characters, a strict prefix of the handle (so "dannymalonerace" with "danny.malone@" reads "Danny"). `launcher/omarky/resolver.rb:181-193`

CHG-010. A version's time reads "Fri 23 Oct, 4:12 pm" (abbreviated weekday, day without padding, abbreviated month, 12-hour time, lowercase am/pm). Its long form reads "Friday 23 October 2026, 4:12 pm". `launcher/omarky/resolver.rb:171-177`

## Entering changes mode

CHG-011. A branching icon button appears in the header only when the file has at least one check-in. Its tooltip is "Compare with an earlier version  V", or "Changed since the last check-in. Compare  V" when dirty, or "Choose the versions  V" while comparing. It shows as active while the list is open or while comparing. `reader/ui/app/Header.qml:185-198`

CHG-012. While dirty and not comparing, a 6 px accent dot sits on the button's top-right corner, inset 3 px. `reader/ui/app/Header.qml:199-210`

CHG-013. `V` (either case) toggles the version list when the file has history, and does nothing otherwise. Clicking the button or the pill also toggles it. `reader/ui/app/ReaderWindow.qml:449,285-288`, `reader/ui/app/Header.qml:164,196`

CHG-014. From the command line, `--from SPEC` opens comparing that version with Now; `--to SPEC` sets the newer side; `--to` without `--from` compares to with the check-in just before it; `--changes` (and the `changes` subcommand) compares Now with the newest version whose text differs from Now. `reader/launcher/omarky_reader/cli.rb:108-120`, `bin/omarky:6-11`

CHG-015. "The newest version whose text differs" means opening with `--changes` shows uncommitted work against the last check-in, or, when the file is clean, the last check-in against the one before it. `launcher/omarky/resolver.rb:136-141`, `docs/superpowers/specs/2026-10-25-omachanges-design.md:83`

CHG-016. When opening with both ends, the newer end is resolved first and the older end is then searched only among versions before it. `reader/ui/app/ReaderWindow.qml:261-267,250-253`

CHG-017. A file outside any git work tree, asked for anything other than "now", fails on the command line with "FILE is not in a git repository" and opens nothing. `reader/launcher/omarky_reader/cli.rb:111-117`

CHG-018. `--list` prints the versions one per line as who, time and subject separated by tabs, and opens nothing. `reader/launcher/omarky_reader/cli.rb:123-129`

## Version specs

CHG-019. Specs are case-insensitive. Empty or "now" is the file on disk. `launcher/omarky/resolver.rb:107-108`

CHG-020. A person ("danny", "keith", "me") resolves to that person's most recent check-in of the file. "me" is the git user name. A person matches on the full author name, the first name, or (for 3 or more letters) a prefix of the first name or of the email's local part. `launcher/omarky/resolver.rb:123-128,230-237`

CHG-021. A day resolves to the last check-in at or before 23:59:59 that day, local time. Accepted days: "today", "yesterday", "a week ago", "last week", "1 week ago", "N days ago", "N weeks ago", "YYYY-MM-DD", a weekday name or its first three letters (the most recent such day strictly before today), "23 oct", "oct 23", optionally with a year (a date without a year that falls after today means last year). Months match on 3 or more leading letters. `launcher/omarky/resolver.rb:129-133,195,271-298`

CHG-022. A person then a day ("danny fri", "keith 23 oct") resolves to that person's last check-in on or before that day. `launcher/omarky/resolver.rb:112-134`

CHG-023. "sent" resolves to the last check-in at or before the end of the first date found in a `SENT.md` beside the file, written as `YYYY-MM-DD` or "21 October 2026". `launcher/omarky/resolver.rb:143-158,322-325`

CHG-024. "before X" resolves to the check-in just before whatever X resolves to. "before now" is the newest version whose text differs from the file on disk. `launcher/omarky/resolver.rb:197-219`

CHG-025. Anything git accepts as a commit (a hash, `HEAD~2`, a tag, a branch) resolves to that commit, shown as its person and time even when it is outside the file's history. `launcher/omarky/resolver.rb:300-313`

CHG-026. When to is a past version, from specs are searched only among versions before to, so "keith" against to "danny" means Keith's last check-in before Danny's. `reader/launcher/omarky_reader/cli.rb:131-137`, `reader/ui/app/ReaderWindow.qml:235`

CHG-027. Failures produce one sentence each: "Danny has no check-ins of this file." (or "You have no check-ins of this file." for me); "The file did not exist yet on 3 October."; "There is no sent date in this directory."; "The file did not exist yet when it was sent."; "There is nothing before now."; "There is nothing before Danny, Fri 23 Oct, 4:12 pm."; "\"xyz\" is not a person, a day or a version." `launcher/omarky/resolver.rb:115-131,197-219,322-325`

CHG-028. In the window, a failed resolution shows its sentence as a toast and leaves the current state as it was. When the window was opening straight into a comparison, it falls back to plain reading. A malformed answer shows "Could not read that version". `reader/ui/app/ReaderWindow.qml:227,240-243`. Note: the design spec puts these sentences in a banner with the list open; the code uses a toast. `docs/superpowers/specs/2026-10-25-omachanges-design.md:172-182`

## The version list

CHG-029. The list drops below the header, 6 px under it, right-aligned to the pill while comparing or to the branching button otherwise, kept at least 8 px inside the window. `reader/ui/app/ReaderWindow.qml:658-670`, `reader/ui/app/Header.qml:43-46`

CHG-030. The list is 600 px wide, the popup background, a 1 px `hairline` border, corner radius `max(6, corner radius)`, with 8 px inner padding. `components/VersionList.qml:23-38`

CHG-031. Its first label is "COMPARE NOW WITH" (or "COMPARE WHICH VERSION" when picking the newer end), in `ghost` at `caption`, all capitals, letter spacing 1.2, padded 10 left, 8 top, 6 bottom. `components/VersionList.qml:40-49`

CHG-032. Shortcut chips follow, wrapping, 6 px apart, padded 10 px each side and 8 px below, shown only when picking the older end. Each chip is 24 px tall, its label plus 20 px wide, fully rounded, with a 1 px `hairline` border, its label at `bodySmall` in `dim`. Hovering gives a `hover` fill and `text` label. Clicking picks that chip's spec. `components/VersionList.qml:51-79`

CHG-033. The shortcuts, in order: "<Who>'s last edit" for each author other than the user (in history order of first appearance), "my last edit", "yesterday", "a week ago", and "when it was sent" only when `SENT.md` has a date. `launcher/omarky/resolver.rb:160-169`

CHG-034. A "HISTORY" label follows in the same style, padded 10 left, 6 top, 4 bottom. `components/VersionList.qml:81-90`

CHG-035. History rows, newest first, are 46 px tall. At most 7 rows show at once and the list scrolls beyond that, stopping at its ends. `components/VersionList.qml:20,92-98`

CHG-036. Each row: an author disc at 10 px from the left and 8 px from the top; the short name at `bodySmall` in `text`, 10 px right of the disc and 6 px from the top, bold on the current row; the time 10 px after the name on the same baseline at `bodySmall` in `faint`; the commit subject 1 px below the name, from the name's left edge to 12 px short of the right edge, elided, at `bodySmall`, in `dim` (in `text` on the current row). `components/VersionList.qml:113-154`

CHG-037. Row backgrounds: `hover` when hovered; the foreground at alpha 0.08 light / 0.07 dark on the row that is the current older side; transparent otherwise. Colour changes animate over 100 ms. Corner radius `max(4, corner radius - 2)`. `components/VersionList.qml:107-112`

CHG-038. Resting the pointer on a row for 400 ms opens a detail card directly below that row, 2 px down, inset 10 px each side of the list body, above other rows. It holds the full subject in bold `bodySmall` `text`, wrapped; the commit body in `dim` `bodySmall` with line height 1.3, wrapped, hidden when empty; and the author's full name, a comma and the long time, in `faint` `caption`. Card padding is 14 px horizontal and 12 px vertical, 6 px between lines, page background, 1 px `hairline` border, radius `max(6, corner radius)`. Leaving the row or the list hides it. `components/VersionList.qml:155-163,194-246,32`

CHG-039. A 1 px `hairlineSoft` rule separates the rows from a 34 px footer. The footer's left text, 10 px in, `caption` `faint`, reads "N versions" (or "1 version"), or "Pick the later version, then the earlier one." when picking the newer end. `components/VersionList.qml:168-181`

CHG-040. The footer's right holds a "Compare two past versions…" chip at `caption` size, 4 px from the edge, shown only when picking the older end and the history has more than one version. `components/VersionList.qml:182-190`

CHG-041. Clicking a row picks that version as the older side, with Now as the newer side unless a newer side is already set. The list closes after the pick resolves. `reader/ui/app/ReaderWindow.qml:240-257,275`

CHG-042. "Compare two past versions…" switches the list to picking the newer end. The next click resolves the newer side; the list then switches back to picking the older end and stays open for the second click, which is searched among versions before the newer one. `reader/ui/app/ReaderWindow.qml:255,668`, `components/VersionList.qml:13-15`

CHG-043. `Esc`, or a click anywhere outside the list, closes it and resets picking to the older end. `reader/ui/app/ReaderWindow.qml:401,650-656`

## Author discs

CHG-044. An author disc is a filled circle, 16 px by default and 15 px in the pill, with the uppercase first letter of the short name, bold, in the page background colour, at `round(0.6 * size)`. `components/AuthorDisc.qml:12-37`, `reader/ui/app/Header.qml:145,149`

CHG-045. The disc's colour is stable per person: a hash over the full author name (or the short name when absent), `h = (h * 31 + code unit) mod 2^32` from 0, indexes the theme palette in the order blue, green, yellow, magenta, cyan. The same person has the same colour in every document. `components/AuthorDisc.qml:14-19`. Note: the spec lists the order green, blue, yellow, magenta, cyan; the code order decides who gets which colour. `docs/superpowers/specs/2026-10-25-omachanges-design.md:160`

CHG-046. Now is an outlined, empty disc: transparent fill, border `max(1, round(size / 10))` in `text`, no letter. `components/AuthorDisc.qml:13,24-26`

## The pill and header while comparing

CHG-047. While comparing, a pill sits in the header's right group: 26 px tall, its contents plus 22 px wide, fully rounded, with a 1 px `hairlineSoft` border. On hover or while the list is open it takes a `hover` fill and `hairline` border, animated 120 ms. Clicking it toggles the list. `reader/ui/app/Header.qml:127-165`

CHG-048. The pill reads, 6 px apart, all at `bodySmall`: the newer side's disc (omitted for Now); the newer side's label in bold `text` ("Now" or the short name); the newer side's day in `dim` (omitted for Now); "compared with" in `dim`; the older side's disc; the older side's label in bold `text`; the older side's day in `dim`. Example: "Now compared with (D) Danny Fri 23 Oct". `reader/ui/app/Header.qml:141-151`

CHG-049. The day shown is the time text up to its first comma ("Fri 23 Oct"). When the header is narrower than 1100 px, days are dropped and "compared with" becomes "vs". `reader/ui/app/Header.qml:48-55,148`

CHG-050. The pill ends with a chevron in `faint` at `caption` (down when the list is closed, up when open) and a close mark in a 16 by 20 px target, `faint` (`text` on hover) at `bodySmall`, tooltip "Back to reading  Esc". Clicking the close mark leaves changes mode. `reader/ui/app/Header.qml:152-161`

CHG-051. After the pill, the change total in `dim` at `bodySmall` with tabular figures, padded 6 px each side: "no changes", "1 change" or "N changes". `reader/ui/app/Header.qml:167-178`

CHG-052. A fold toggle button appears among the header buttons only while comparing with at least one change, shown active while the fold is on, tooltip "Only what changed  F" or "Show everything  F". `reader/ui/app/Header.qml:182`

CHG-053. While comparing, the "reloaded" pulse does not fire for ordinary reloads of the page (a fold or a pick also reloads it). It fires when the compared file changes on disk. `reader/ui/app/ReaderWindow.qml:197-199,306`

CHG-054. The pulse is a 6 px accent dot and "reloaded" in accent at `caption`, fading in over 120 ms, holding 1100 ms, fading out over 500 ms. `reader/ui/app/Header.qml:98-117`

## When there is nothing to compare

CHG-055. When the two texts are identical, a banner under the header reads "No changes since <who>, <time>." (for example "No changes since Danny, Fri 23 Oct, 4:12 pm.") in the blue tone, and the page renders with no marks. `components/Changes.qml:57`, `reader/ui/app/ReaderWindow.qml:536-544`

CHG-056. The banner is 34 px tall, a tone wash at alpha 0.1 light / 0.09 dark, a 1 px bottom line in the tone at 0.25, and a centred info icon in the tone plus the message in `text` at `bodySmall`, 8 px apart. It floats over the page's top margin rather than pushing the text down. `components/Banner.qml:6-27`, `reader/ui/app/ReaderWindow.qml:535-544`

## The diff engine (behaviour)

CHG-057. Both texts are parsed with the reader's own block model. The output is the to document's top-level blocks in order, with removed blocks inserted where they stood. Each top-level block and list item carries one of `same`, `added`, `removed`, `reworded`. `lib/diff/changes.js:8-21`

CHG-058. Each side is cut into sections at headings (plus a leading section before the first heading). Sections align first exactly, by heading level plus whitespace-normalised heading text, in order (longest common subsequence). `lib/diff/changes.js:323-332,336-351`

CHG-059. Unmatched sections of the same heading level then pair when their title word similarity is at least 0.6, or when their body word similarity is at least 0.6, so a retitled section stays one section. `lib/diff/changes.js:23,341-351`

CHG-060. Similarity is the Dice coefficient over lowercased word tokens, punctuation ignored: `2 * shared / (n + m)`. Two empty texts score 1; empty against words scores 0. `lib/diff/words.js:179-210`

CHG-061. Similarity pairing is greedy: highest score first, ties to the pair at the same position in their lists, and no pair may cross an exact match or another chosen pair. `lib/diff/align.js:31-77`

CHG-062. Within aligned sections, blocks align first exactly (same type and same whitespace-normalised plain text), then same-type leftovers pair at similarity 0.5 or more. Images and rules never pair by similarity. `lib/diff/changes.js:23,289-296`

CHG-063. A paired paragraph or heading gets a word-level diff. Tokens are words (letters, digits, underscore, apostrophes including the curly one), single punctuation marks, and whitespace runs. All whitespace compares equal, so re-wrapped or re-spaced text with no word changes reports zero changes. `lib/diff/words.js:1-35`, `lib/diff/runs.js:43-54`, `docs/superpowers/specs/2026-10-25-omachanges-design.md:242`

CHG-064. In a reworded paragraph the shown text is one merged run sequence in diff order: removed text from the older side, inserted text from the newer side, shared text from the newer side (so inline styling on shared text is the newer side's). Matched whitespace between two changes does not split them into separate marks. `lib/diff/runs.js:43-54`, `lib/diff/words.js:123-131`

CHG-065. Lists align item by item (exact, then similarity 0.5) and each paired item's blocks diff recursively. Removed items are placed before the next kept item. `lib/diff/changes.js:186-249`, `docs/superpowers/plans/2026-10-25-omachanges.md:2683`

CHG-066. Tables: header cells diff by position. Rows align by their first cell's text, then by whole-row similarity 0.5; paired cells take the word diff; added rows have every cell inserted; removed rows stay where they were with every cell struck. `lib/diff/changes.js:251-285`

CHG-067. Changed code blocks diff by line and display as a diff block with context, removed and added lines. A removed code block displays as a diff block of all removed lines. `lib/diff/changes.js:84-94,172-184`

CHG-068. Front matter compares field by field by key. Footnotes align by label, then similarity. Quotes and callouts diff their inner blocks. `lib/diff/changes.js:132-157`

CHG-069. Blocks with no inner pairing (raw HTML, image galleries) that differ show as removed then added. An image whose source changed is removed plus added. `lib/diff/changes.js:159-160,309-311`, `docs/superpowers/specs/2026-10-25-omachanges-design.md:120`

CHG-070. A moved paragraph is not detected. It shows as removed in one place and added in another. A split or joined paragraph pairs the larger overlap and leaves the rest added or removed. `docs/superpowers/specs/2026-10-25-omachanges-design.md:22,127`

CHG-071. An empty older side marks every block added. Two identical texts mark every block same with a total of 0. `docs/superpowers/specs/2026-10-25-omachanges-design.md:125-126`

CHG-072. Counting: each added, removed or reworded block counts 1. A reworded list counts its changed items; a reworded quote or footnotes block counts its inner changes. The header total is the sum. `lib/diff/changes.js:14-21,164-167,186-190,234-249,374-381`

CHG-073. Each heading also carries its section's count: its own change plus every block's count until the next heading at its level or higher, so subsections are included. It also carries the words inserted and removed over that span (a block added or removed whole counts all its words). `lib/diff/changes.js:387-439`

CHG-074. Performance target: a 20,000-word document diffs within 200 ms. omarky mechanism: the exact pass runs first and word bags are computed lazily only for leftovers, and token diffs run per pair, so cost is near-linear in document length. `docs/superpowers/specs/2026-10-25-omachanges-design.md:128`, `lib/diff/changes.js:207-215`

## Marks on the page

CHG-075. Inserted words sit on a wash under the text: `flat(green, 0.28)` in light themes and `flat(green, 0.38)` in dark, `round(font size * 1.3)` tall, centred on the line's x-height, extending 2 px beyond the words on each side, radius 3 px. A wrapped span gets one wash per line. `services/Reading.qml:84`, `components/RichText.qml:77,83-104,135-148,155`. Note: the design spec says 0.22 light / 0.26 dark; the code's values are higher. `docs/superpowers/specs/2026-10-25-omachanges-design.md:139`

CHG-076. Removed words stay in place, drawn in `faint`, with a strike line over them in `flat(red, 0.85)`, `max(1, round(font size / 11))` px thick, at the line's ascent minus half its x-height. The strike is drawn above the text. `services/Reading.qml:85-86`, `components/RichText.qml:136-147,156`, `lib/inline.js:171`

CHG-077. A block added or removed whole sits on a full-width wash instead of word marks: added `flat(green, 0.10)` light / `flat(green, 0.15)` dark; removed `flat(red, 0.08)` light / `flat(red, 0.12)` dark. Radius `max(6, corner radius)`. `components/DocView.qml:303-330`

CHG-078. The wash reaches `round(reading body * 1.25)` left of the column (room for a hanging bullet) and `round(reading body * 0.8)` right of it, and 8 px above and below the block. `services/Reading.qml:56-57`, `components/DocView.qml:320-323`

CHG-079. Consecutive blocks of the same kind share one continuous wash: inner corners are square and the wash spans the gap between them. An added block next to a removed block splits the gap between them with a 2 px seam. `components/DocView.qml:306-328`

CHG-080. A removed block's text is `faint` on the red wash with no strike line. `components/DocView.qml:358`, `components/blocks/Paragraph.qml:13`, `components/blocks/Heading.qml:34`. Note: the design spec says struck through whole. `docs/superpowers/specs/2026-10-25-omachanges-design.md:140`

CHG-081. A reworded list is marked by its items, not as a whole. An item added or removed whole sits on its own wash with the same horizontal reach as a block wash and `0.3 * text size` above and below, radius `max(4, corner radius - 2)`. A list added or removed whole is washed and tagged once, as a block. `components/DocView.qml:299-302`, `components/blocks/ListBlock.qml:49-65`

CHG-082. A diff block (changed or removed code) has a header with "+N" in green and "−M" in red, mono, bold, at `caption`; each line on a full-width band, added green at alpha 0.14, removed red at 0.14, hunk blue at 0.07; a sign column with "+" or "−" in the line's tone; added and removed line text tinted 30% toward green or red. `components/blocks/DiffBlock.qml:62-65,84-104,141-156`, `lib/diff/changes.js:88-94`

## Margin tags

CHG-083. Every added, removed or reworded block carries a tag in the left margin: "added", "removed" or "reworded" in the UI family at `caption`, letter spacing 0.3, as a fully rounded pill `round(caption * 1.75)` tall (18 px) and its label plus `round(caption * 1.3)` (13 px) wide. Same blocks have no tag. `components/ChangeTag.qml:10-39`

CHG-084. Tag tones: added uses green, removed uses red, each filled at alpha 0.13 light / 0.16 dark with the label mixed from the tone toward `text` by 0.25 light / 0.2 dark. Reworded is quiet: `dim` fill at alpha 0.08 light / 0.1 dark and a `dim` label. "unchanged" (folded sections) has a `faint` label. `components/ChangeTag.qml:14-15,24,35`

CHG-085. The tag's right edge sits `round(reading body * 1.9)` left of the column, and it is vertically centred on the block's first line. `components/DocView.qml:331-337`

CHG-086. When the margin is narrower than the tag needs (room is the gutter minus `round(reading body * 1.9)` minus 6 px), a dot of `round(caption * 0.7)` px (7 px) in the tone stands in, at alpha 0.85 (0.55 for reworded), centred in the margin. Hovering the dot shows the word as a tooltip. `components/ChangeTag.qml:16-27`, `components/DocView.qml:333-335`

CHG-087. List items carry their own tags left of the bullet and centred on the item's first line, never closer than half the text size to the bullet. `components/blocks/ListBlock.qml:155-165`

## The fold

CHG-088. `F` (either case), or the header's fold button, toggles the fold, only while comparing and only when the total is above 0. `reader/ui/app/ReaderWindow.qml:450,521`

CHG-089. With the fold on, every run of consecutive unchanged non-heading blocks collapses to one row. A heading whose section (to the next heading at its level or higher) has no changes collapses together with its whole section to the heading alone. Headings with changes stay and are never part of a run. `lib/diff/fold.js:22-50`

CHG-090. A run row is `round(reading body * 1.6)` tall with a centred label in `faint` at `caption`: "N paragraphs unchanged" ("1 paragraph unchanged") when every block in the run is a paragraph, else "N blocks unchanged" ("1 block unchanged"). A 1 px `hairlineSoft` line runs from each column edge to `round(reading body * 0.8)` short of the label. `components/blocks/Fold.qml:18-24,64-82`

CHG-091. A folded section shows its heading text in `dim` at the heading's own size and face, and an "unchanged" tag in the margin like other blocks. It is spaced like a heading. `components/blocks/Fold.qml:40-62`, `services/Reading.qml:96-98`

CHG-092. Hovering a fold row shades it with `hover` on the same edges as a change wash (plus 6 px above and below), animated 120 ms, with a pointing-hand cursor. Clicking opens that run or section in place. `components/blocks/Fold.qml:26-38,84-85`, `components/Changes.qml:70-78`

CHG-093. An opened run or section stays open while the fold remains on. Toggling the fold off or on forgets which ones were opened. `components/Changes.qml:30-34`, `lib/diff/fold.js:9-12`

CHG-094. Fold state belongs to the window and resets to off when changes mode ends. `components/Changes.qml:63-68`. Note: the spec mentions a preference for the default fold state; the code has none, and the fold always starts off. `docs/superpowers/specs/2026-10-25-omachanges-design.md:166`

CHG-095. Toggling the fold keeps the reader's place on the page. omarky mechanism: the folded document is installed through the same reload path that captures and restores the reading anchor. `components/MarkdownView.qml:41,211`, `services/Doc.qml:54-69`

CHG-096. Folded sections keep their headings in the outline. `components/Changes.qml:41-48`

## Outline counts

CHG-097. While comparing, each outline entry with a section count above 0 shows the count at its right in the mono face at `caption`, tabular figures, `dim` (`text` for the current section), 8 px before a mix bar. `components/Outline.qml:126-138`

CHG-098. The mix bar is 22 by 4 px, 20 px from the outline's right edge, split by the share of words inserted against words inserted plus removed (half and half when both are 0): green `flat(green, 0.8)` on the left, red `flat(red, 0.75)` on the right, each radius 2, with a 1 px gap when both show. `components/Outline.qml:111-125`

CHG-099. Entries with a count of 0 show no count or bar. A folded section's entry counts 0. `components/Outline.qml:19-30,115,131`

CHG-100. With the fold on, outline entries whose count is 0 dim to opacity 0.5. `components/Outline.qml:107`, `reader/ui/app/ReaderWindow.qml:560`

CHG-101. The outline lists headings of levels 1 to 4 only, so deeper sections contribute to their parent's count without an entry. `components/Outline.qml:33`

## Stepping between changes

CHG-102. `n` moves to the next changed block below the top of the view, `p` to the previous one above it. Both wrap around at the ends. They work only while comparing. `reader/ui/app/ReaderWindow.qml:291-301,453-454`

CHG-103. When a find query is active, `n` steps through find matches instead, and `N` (Shift+n) steps back through them. Without a find query, uppercase `N` and `P` do nothing; the help sheet and README write the keys as "N  P". `reader/ui/app/ReaderWindow.qml:451-454`, `reader/ui/app/ShortcutSheet.qml:16`

CHG-104. A step glides the target block to 20 px below the top of the view over 240 ms. A jump farther than 1.5 view heights first cuts to 0.8 of a view short of the target, then glides. Under reduced motion it lands at once. `components/DocView.qml:88-125`

CHG-105. The landed block flashes: 300 ms after the step, a tint of the accent at alpha 0.1 light / 0.13 dark covers it (the column plus 12 px each side, 6 px above and below, radius `max(6, corner radius)`), fading in over 500 ms with ease-out; 350 ms after it starts, it fades out over 500 ms. `reader/ui/app/ReaderWindow.qml:592-618`

## Live re-run

CHG-106. When either side is Now, the window watches the file. A change on disk is debounced 150 ms, re-read, and the comparison re-runs in place, keeping the reader's place. `components/Changes.qml:82-97`, `services/Doc.qml:54-69`

CHG-107. Each re-run after the first load pulses "reloaded" in the header and refreshes the version list 400 ms later. `components/Changes.qml:91-95`, `reader/ui/app/ReaderWindow.qml:306`

CHG-108. Saving from the edit sheet in changes mode therefore shows the edit as a change immediately. `docs/decisions/0018-a-sheet-edits-one-block.md:21`

## Leaving and other interactions

CHG-109. `Esc` leaves changes mode only after closing, in order, the help sheet, the version list, the find bar, and any text selection. Leaving clears both sides, the fold, and the list, and the page reads the file plainly at the same place. `reader/ui/app/ReaderWindow.qml:396-401,456-460,277-283`

CHG-110. Opening another file in the window clears the comparison and asks for the new file's versions. `reader/ui/app/ReaderWindow.qml:220`

CHG-111. Task boxes can be ticked while comparing only when the newer side is Now, and never on a removed item. `reader/ui/app/ReaderWindow.qml:367-377`, `components/blocks/ListBlock.qml:115`

CHG-112. The edit sheet works in changes mode only when the newer side is Now, refuses fold rows and removed blocks, and shows a reworded block's older wording with a "Use this" chip (see SHEET-004 to SHEET-006 and SHEET-040 to SHEET-042). `reader/ui/app/ReaderWindow.qml:347-361`

CHG-113. Find searches the visible text, struck removed runs included. `docs/superpowers/specs/2026-10-25-omachanges-design.md:141`. (Not verified in code during this extraction.)

## Help sheet entries

CHG-114. When the file has history, the help sheet opens with a "Changes" group: "V" Choose the versions; "N  P" Next / previous change; "F" Only what changed / everything; "Esc" Back to reading. Without history the group is absent. `reader/ui/app/ShortcutSheet.qml:11-17`, `reader/ui/app/ReaderWindow.qml:676`
