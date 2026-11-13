# Rewrite with Claude (deferred)

Decision 0006 defers rewrite with Claude (`Ctrl+J` in the edit sheet and the writer) to a later
pass. This file is omarky's behaviour as extracted (`05-edit-sheet-and-rewrite.md`, the sections
that start with "Rewrite:"), kept as the starting point for that pass. It is not adapted to
Quire yet, and it has no status or verify tags. The sheet's entry keys (SHEET-034) and help row
(SHEET-111) stay in `../behavior/edit-sheet.md`, marked deferred.

Paths are relative to `/home/p0werj0int/Projects/omarky` (version 0.7.0). "The line" is the
instruction line, and "the range" is the text it rewrites.

Points the later pass has to settle for Quire:

- A locked note's plain text would go to Anthropic. The pass must decide whether rewrite works
  on locked notes at all, and say so where the user sees it.
- omarky runs Claude Code through a separate launcher (DEC-0019). In Quire this is a subprocess
  started from Rust, off the UI thread, with the same no-tools boundary.
- The style guide and checker live in omarky's config folder. Quire needs its own place for
  them, in its config folder and never in the notes folder.
- In the reader's sheet Claude sees only the sheet's text (SHEET-062). In the writer it sees
  the whole note. omarky's spec wanted the whole document in both.

## Rewrite: availability

- **SHEET-051** At startup the rewrite asks the launcher whether Claude Code is installed (`rewrite --available`, exit 0 when found). The answer is cached for the window's life (`components/Rewriter.qml:255-259`, `bin/omarky:51-53`).
- **SHEET-052** Claude Code is looked for in this order. An override path from the environment if set, else the first executable `claude` on PATH, else `~/.local/bin/claude`, `~/.claude/local/claude` and `~/.local/share/mise/installs/claude/latest/claude` (`launcher/omarky/rewrite.rb:44-52`).
- **SHEET-053** Without Claude Code, `Ctrl+J` and `Ctrl+Shift+J` show the toast "Rewrite needs Claude Code (claude on your PATH)" and open nothing. The spec says the key does nothing and the help sheet hides it. The code shows the toast, and the help sheet always lists `Ctrl+J` (`components/Rewriter.qml:60`, `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:71`, `reader/ui/app/ShortcutSheet.qml:24-25`).

## Rewrite: choosing the range

- **SHEET-054** A range is a span of character offsets in the editor's text. A range never includes the newline that ends its last line, nor trailing whitespace, so replacing it keeps the blank lines between blocks (`lib/ranges.js:15-16`, `lib/ranges.js:31-35`).
- **SHEET-055** `Ctrl+J` with a selection takes the selection, widened at both ends to whole words. A word character is a letter (Latin, Greek, Cyrillic), a digit, an apostrophe (straight or curly), an underscore, `*`, a backtick, `~`, `]` or `)` (`components/Rewriter.qml:68`, `lib/ranges.js:91-98`).
- **SHEET-056** `Ctrl+J` with no selection takes the top-level block holding the caret's line (paragraph, list, quote, table, code block and so on). On a blank line it is the next block. Past the last block it is the last block. With no blocks it is the whole text (`components/Rewriter.qml:69`, `lib/ranges.js:51-66`).
- **SHEET-057** `Ctrl+Shift+J` takes the section holding the caret, from the heading at or above it to the line before the next heading of the same or a higher level. Before the first heading it is the text up to that heading. A selection is ignored (`components/Rewriter.qml:67`, `lib/ranges.js:68-89`).
- **SHEET-058** `Ctrl+J` pressed again while the line is open and Claude is not working widens the range one step. Selection becomes block, block becomes section, section becomes each enclosing shallower section in turn, and then the whole text. A step that does not strictly contain the current range is skipped. Widening uses the caret position from when the line first opened (`components/Rewriter.qml:62-64`, `components/Rewriter.qml:206`, `lib/ranges.js:103-116`).
- **SHEET-059** `Ctrl+Shift+J` while the line is open and idle resets the range to the caret's section (`components/Rewriter.qml:63`).
- **SHEET-060** Changing the range (opening, widening, or the section key) clears the conversation. That means earlier instructions, rewrites, notes, the problem and the status (`components/Rewriter.qml:77-94`).
- **SHEET-061** While Claude is working, `Ctrl+J` only refocuses the instruction line (`components/Rewriter.qml:61-74`).
- **SHEET-062** In the reader's sheet, "the whole text" and "the document" mean the sheet's text only, not the whole file. The job sent to Claude carries the sheet editor's text as the document. So Claude sees only the opened block or section in the reader, while in the writer it sees the whole file (inferred) (`components/Rewriter.qml:144`, `reader/ui/app/EditSheet.qml:318-326`).
- **SHEET-063** The range is shaded in the editor with a hover fill that reaches 10 px left and right and 6 px above and below the range's lines. It spans the text width from the gutter, with radius max(6, corner radius), behind the text. There is no accent bar (`components/Editor.qml:81-89`, `components/Editor.qml:163-172`, `components/Rewriter.qml:96-99`).
- **SHEET-064** The range follows edits made elsewhere in the text while the line is open. The original range text is searched for at its old offset, then at the occurrence nearest the old offset. If it is found, the range and shade move with it. If not, the range is lost. The shade disappears and the status line reads "That text changed while the line was open" in red (`components/Rewriter.qml:116-126`, `lib/ranges.js:118-127`).

## Rewrite: the card

- **SHEET-065** The instruction line and the rewrite share one card that floats over the sheet. Its width is min(window width - 24, range width + 32). It is placed 16 px left of the range's left edge, clamped 12 px inside the window (`components/Rewriter.qml:267-277`).
- **SHEET-066** Vertically the card sits 14 px below the range's last line when it fits above the window's bottom margin of 12 px. Else it sits 14 px above the range's first line when that stays 12 px clear of the top. Else it sits as low as it fits (`components/Rewriter.qml:278-285`).
- **SHEET-067** The card has the popup background, a 1 px hairline border, corner radius max(6, corner radius), 16 px padding and 10 px between its rows. Clicking anywhere on it focuses the instruction line (`components/Rewriter.qml:267`, `components/Rewriter.qml:286-298`).
- **SHEET-068** The instruction row is at least 26 px tall. A prompt sits at its left in faint at caption, reading "Rewrite the selection", "Rewrite the block", "Rewrite the section" or "Rewrite the whole document". The input field follows 12 px after it (`components/Rewriter.qml:55`, `components/Rewriter.qml:301-311`).
- **SHEET-069** The input is in the reading prose face, Regular, at round(reading body × 0.92), 13 px at zoom 1, in text colour with an accent selection. Its placeholder, in ghost, is "Say what to change" when idle, empty while Claude works, and "Enter to use it, or say what else to change" when a rewrite is ready (`components/Rewriter.qml:312-329`).
- **SHEET-070** Below the instruction row, once there is rewrite text to show, come a 1 px hairlineSoft rule and then the rewrite (`components/Rewriter.qml:335`).
- **SHEET-071** The rewrite is shown as rendered Markdown blocks marked against the original range the way changes mode marks a document. Inserted words sit on the green wash, and removed words are faint under a red strike (see the CHG marks). A lone block rewritten as a lone block of the same type is always paired word by word, however different (`components/Rewriter.qml:263-264`, `components/Rewriter.qml:345-366`, `lib/diff/changes.js:471-479`).
- **SHEET-072** A block added or removed whole in the rewrite sits on a wash that reaches 8 px left and right and 4 px above and below, radius max(4, corner radius - 2). Added blocks use flat(green, 0.1 light / 0.15 dark) and removed blocks flat(red, 0.08 light / 0.12 dark). Blocks are spaced round(reading body × 0.7) (`components/Rewriter.qml:349-362`).
- **SHEET-073** The rewrite area is at most max(80, 0.45 × window height) tall and scrolls inside when longer, stopping at its bounds (`components/Rewriter.qml:336-344`).
- **SHEET-074** While text streams in, the marked rendering refreshes at most every 90 ms, and once more at once when the rewrite completes (`components/Rewriter.qml:155`, `components/Rewriter.qml:264-265`).
- **SHEET-075** When the checker still reports problems after its retry, a wrapped line "Your checker still flags: " plus the notes joined by commas shows in yellow at caption. It is hidden while Claude works (`components/Rewriter.qml:369-377`).
- **SHEET-076** The status row is 22 px tall, with status text on the left at caption, elided, leaving 12 px before the key hints. The text is, in priority order, the problem message, then "That text changed while the line was open", then the status word with "…" appended while working, then "Rewrite N of M" when the conversation holds more than one rewrite, else nothing. Problem and lost states are red, the rest faint (`components/Rewriter.qml:380-391`).
- **SHEET-077** While Claude works the status text pulses, to opacity 0.45 over 700 ms and back to 1 over 700 ms, repeating. It returns to full opacity when work stops. The durations collapse to 0 under reduced motion (`components/Rewriter.qml:392-398`, `services/Reading.qml:64-66`).
- **SHEET-078** Key hints sit at the right, 6 px apart, at caption in faint. "Enter use" shows when a rewrite is ready. "Ctrl+J wider" shows when idle with no ready rewrite. "Esc" always shows, followed by "stop" while working, "drop" when a rewrite is ready and "close" otherwise (`components/Rewriter.qml:400-413`).

## Rewrite: keys on the line

- **SHEET-079** `Esc` while Claude works stops it and sets the status to "Stopped", keeping the line open. Otherwise `Esc` closes the line, drops any rewrite, clears the shade, and returns focus to the editor with its text unchanged (`components/Rewriter.qml:102-114`, `components/Rewriter.qml:205`).
- **SHEET-080** `Enter` with text in the input sends it as an instruction. `Enter` on an empty input takes the shown rewrite when one is ready (SHEET-088) (`components/Rewriter.qml:201-204`).
- **SHEET-081** `Alt+Up` and `Alt+Down` step to the earlier or later rewrite of this conversation, clamped at the ends. They do nothing while working or with fewer than two rewrites (`components/Rewriter.qml:191-196`, `components/Rewriter.qml:207`).
- **SHEET-082** `Up` and `Down` (no modifiers) on an empty input, or while already recalling, step through remembered instructions. `Up` goes to older ones and `Down` to newer ones, and `Down` past the newest empties the input. The caret goes to the end of the recalled text (`components/Rewriter.qml:208-228`).
- **SHEET-083** Each sent instruction moves to the front of the remembered list, with duplicates removed and at most 40 kept. The list persists across sessions in the app's state directory, and is not saved in an ephemeral session (`components/Rewriter.qml:215-236`, `reader/ui/app/ReaderWindow.qml:689`).

## Rewrite: sending

- **SHEET-084** Sending trims the instruction and ignores it when empty or while working. When the range is lost, nothing is sent and the problem reads "That text changed. Press Esc and select it again." (`components/Rewriter.qml:130-133`).
- **SHEET-085** A second instruction refines rather than restarts. The previous instruction and the rewrite shown now (including one recalled with `Alt+Up`) are appended to the conversation's turns, and the new request carries every turn (`components/Rewriter.qml:134`, `components/Rewriter.qml:144`).
- **SHEET-086** On send the input clears, the shown rewrite and notes clear, the status becomes "Writing", and work begins (`components/Rewriter.qml:135-147`).
- **SHEET-087** Messages from the agent are handled in order. Text pieces append to the rewrite as they arrive. "checking" sets the status to "Checking against your writing guide". "retry" clears the rewrite and sets the status to "Fixing: " plus the checker's notes joined by commas. "done" sets the final text, adds it to the conversation's rewrites and shows it, keeps the notes, and sets the status to "No change" when the text equals the original range, else clears it. "error" stops work and shows the message as the problem (`components/Rewriter.qml:154-171`).

## Rewrite: taking it

- **SHEET-088** Taking a ready rewrite finds the original range text again, at its offset or else at the nearest occurrence. If it is found, the rewrite replaces that span as one undo step, the caret goes to the end of the inserted text, the line closes, the shade clears and the editor takes focus. `Ctrl+Z` then restores the original (`components/Rewriter.qml:175-189`, `components/Editor.qml:66-71`).
- **SHEET-089** Without omarky's native highlighter module the replacement is two undo steps, a remove and then an insert (`components/Editor.qml:64-68`, `docs/decisions/0019-claude-behind-the-launcher.md:24`).
- **SHEET-090** If the original range text is no longer in the editor, nothing is replaced, the rewrite is copied to the clipboard, and the problem reads "The text changed while Claude worked, so nothing was replaced. The rewrite is on the clipboard." (`components/Rewriter.qml:177-182`).
- **SHEET-091** In the sheet, taking a rewrite changes only the sheet's text. `Ctrl+Enter` still writes the file, and the stale-file refusal (SHEET-044) still applies (`reader/ui/app/EditSheet.qml:315-317`, `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:51`).
- **SHEET-092** The user can keep typing elsewhere in the text while Claude works (`docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:49`, `components/Rewriter.qml:116-126`).

## Rewrite: the agent

- **SHEET-093** The window never runs Claude itself. It writes a job file to the user runtime directory (or `/tmp`) named `omarky-rewrite-<epoch ms>-<random 0..999999>.json`, then runs the launcher's `rewrite --job FILE`. The launcher deletes the job file after reading it (`components/Rewriter.qml:144-146`, `components/Rewriter.qml:238-243`, `bin/omarky:54-61`, `docs/decisions/0019-claude-behind-the-launcher.md:7`).
- **SHEET-094** The job holds the file path, the full text being edited ("document"), the range's start and end offsets, the instruction, and the earlier turns as instruction and rewrite pairs (`components/Rewriter.qml:144`, `launcher/omarky/rewrite.rb:18-19`).
- **SHEET-095** A job file that cannot be written stops work with the problem "Could not start the rewrite: " plus the error (`components/Rewriter.qml:242`).
- **SHEET-096** Claude Code runs headless with model `claude-opus-5-5` (overridable by an environment variable), low effort, no tools, no setting sources, a strict empty MCP config, no session persistence, a fixed system prompt, and streaming JSON output with partial messages. Its working directory is the system temp directory, and four nested-session environment variables are removed (`launcher/omarky/rewrite.rb:27-29`, `launcher/omarky/rewrite.rb:104-109`, `launcher/omarky/rewrite.rb:138`, `launcher/omarky/rewrite.rb:216-218`).
- **SHEET-097** The system prompt tells Claude to rewrite only the selected part, following the instruction. It replies with the replacement Markdown and nothing else (no preamble, explanation, quotes or code fence). It keeps the selection's Markdown structure and the document's voice, tense, spelling conventions and facts. It adds no unsupported claim, changes nothing it was not asked to, and replies with the selection unchanged when the instruction cannot be applied (`launcher/omarky/rewrite.rb:31-37`).
- **SHEET-098** If `writing.md` exists in the config directory (`$XDG_CONFIG_HOME/omarky`, by default `~/.config/omarky`), it alone is the style guide. Else every `writing/*.md` file is used, in name order. Contents are trimmed, empty files dropped, and the rest joined with blank lines and appended to the system prompt after "The writer's style guide follows. Apply it to everything you write." (`launcher/omarky/rewrite.rb:39-41`, `launcher/omarky/rewrite.rb:67-81`).
- **SHEET-099** The user prompt starts with the document wrapped in a `document` tag carrying the file's base name, with the range wrapped in a `selection` tag in place. When there are earlier turns, it adds "Earlier rewrites of the selection, oldest first:" with each turn as an attempt holding its instruction and rewrite, and then "Start from the last rewrite and apply the new instruction to it." Then comes the instruction in an `instruction` tag, and last "Reply with only the new text for the selection." (`launcher/omarky/rewrite.rb:85-102`).
- **SHEET-100** If an executable `check` exists in the config directory and the reply is not blank, the launcher reports "checking", writes the reply to a temporary `.md` file (newline-terminated) and runs the checker on it. Exit 0 means it passes (`launcher/omarky/rewrite.rb:73-76`, `launcher/omarky/rewrite.rb:116-118`, `launcher/omarky/rewrite.rb:190-193`).
- **SHEET-101** A failing checker's output is reduced to notes. The notes are the non-blank trimmed lines, only those containing `[fix]` when any do, with a trailing ` | L<n>: ...` and a leading `[fix] ` removed, de-duplicated, and at most 12 (`launcher/omarky/rewrite.rb:195-199`).
- **SHEET-102** With notes, the launcher reports "retry" with them and asks Claude once more, appending the first reply and the notes and asking for corrected text. The checker then runs on the second reply, and whatever it still reports goes out with "done" as the notes. There is only one retry (`launcher/omarky/rewrite.rb:119-126`).
- **SHEET-103** The reply is cleaned. A reply wrapped whole in a code fence is unwrapped, unless the selection itself starts with a fence. The result is trimmed and given the selection's own leading and trailing whitespace. The final message is used, or the streamed text when the final message is empty (`launcher/omarky/rewrite.rb:172`, `launcher/omarky/rewrite.rb:177-187`).
- **SHEET-104** The card shows these errors. "Claude Code is not installed (no claude on PATH)." Claude's own error result text, or "Claude stopped with an error." when it is empty. The last line of Claude's stderr, or "Claude ended without a reply (exit N)." when nothing came back (`launcher/omarky/rewrite.rb:112`, `launcher/omarky/rewrite.rb:153-154`, `launcher/omarky/rewrite.rb:168-170`).
- **SHEET-105** If the agent process exits while work is still marked in progress, without having sent "done" or "error", the problem reads "The rewrite stopped (exit N)." unless the user stopped it (`components/Rewriter.qml:250-252`).
- **SHEET-106** Stopping sends the agent a terminate signal. The launcher terminates its Claude child and exits with code 143 (`components/Rewriter.qml:149-152`, `launcher/omarky/rewrite.rb:113`, `launcher/omarky/rewrite.rb:202-206`).
- **SHEET-107** A rewrite has no timeout. It runs until Claude finishes, errors, or the user presses `Esc` (inferred from the absence of any timer or deadline in `components/Rewriter.qml` and `launcher/omarky/rewrite.rb`).
- **SHEET-108** The launcher records the call's cost in USD, rounded to 4 places, in its "done" message. The window does not show it (`launcher/omarky/rewrite.rb:127`, `launcher/omarky/rewrite.rb:152`, `components/Rewriter.qml:158-165`).
- **SHEET-109** An empty reply is not called out. When the cleaned reply is empty, the card shows nothing to take and `Enter` does nothing. The spec intends "No change" for empty replies too (inferred from `ready` requiring non-empty text at `components/Rewriter.qml:54` and the status rule at `components/Rewriter.qml:164`, `docs/superpowers/specs/2026-10-26-rewrite-with-claude.md:67`).
- **SHEET-110** Each rewrite sends the job's document text to Anthropic through Claude Code on the user's subscription. There is no API key (`docs/decisions/0019-claude-behind-the-launcher.md:16-20`, `README.md:101`).
