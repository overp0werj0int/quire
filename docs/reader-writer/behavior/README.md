# Behaviour files

Each file here lists the behaviour Quire's desktop reader and writer must have, one numbered item
per behaviour. The items come from omarky's code and docs, adapted to Quire by the decisions in
`../decisions/`. They are the basis for verification: every item that is in scope names how it
is checked.

| File | Prefix | Covers |
| --- | --- | --- |
| `page-and-typography.md` | `PAGE` | Fonts, sizes, the column, colour roles, syntax colours, vertical rhythm, inline text, motion |
| `blocks.md` | `BLK` | Every block type on the reader page |
| `reader.md` | `RD` | Preview mode: reload, scrolling, outline, find, links, copy, tasks, help sheet |
| `writer.md` | `WR` | Source mode: live formatting, editing keys, focus, typewriter, chrome, help sheet |
| `edit-sheet.md` | `SHEET` | Editing one block or section from the reader |
| `keymap.md` | `KEY` | Every key binding, in the omarky and quire presets |
| `verification.md` | `VER` | How the items are checked |

## Item format

```
- **PAGE-21** [in] [verify: geometry] The measure is 68 average characters ... (`components/DocView.qml:19`)
```

- The ID never changes and is never reused. A removed behaviour keeps its ID with status `dropped`.
- The first tag is the status:
  - `[in]`: Quire does what omarky does, as written. Sizes in px assume the default 14 px body and
    scale with it.
  - `[adapted: NNNN]`: Quire does it with a change that decision `NNNN` explains. The text states
    Quire's behaviour. What omarky did is kept after "omarky:".
  - `[quire]`: Quire's existing behaviour covers this need, and omarky's version is not ported.
    The text says which Quire behaviour.
  - `[deferred]`: a later pass, listed in `../TODO.md`.
  - `[dropped]`: does not apply to Quire (toolkit detail, or a single-file-app detail).
- The second tag says how the item is checked. Items with status `quire`, `deferred` or
  `dropped` use `[verify: -]`. The methods are:
  - `unit`: a Rust unit test of pure logic (parser, colour maths, keymap).
  - `state`: an assertion on the app state in a headless run.
  - `geometry`: an assertion on element boxes in a headless run (`Runtime::presented`, `--tree`).
  - `golden`: a headless PNG compared with a stored reference.
  - `script`: a scripted input sequence (`--script`) followed by state or geometry assertions.
  - `bench`: a timing measured against a budget.
  - `manual`: a person checks it, usually side by side with omarky.
- Source citations in parentheses point into omarky (`/home/p0werj0int/Projects/omarky`,
  version 0.7.0) unless they start with `quire:`.

`../tools/check-specs.py` checks the format of every file except this one and `verification.md`, which is a plan rather than a list of behaviour. Run it after every change to these files.
