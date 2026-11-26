# Quire design system

The unified desktop design system in `desktop/src/ui/tokens.rs`, `desktop/src/ui/styles.rs` and
`desktop/src/ui/components/`, together with the audit of 66 inconsistencies (42 settled in code;
24 remaining open across keymap, wording, spec drift and Android parity).

| File | What it is |
| --- | --- |
| `quire-design-system.html` | The whole system on one page: tokens, 18 components drawn on Paper and Ink, views, rules, and a card for each finding. One file, no scripts. This is the file to take into Claude Design. |
| `DESIGN.md` | The same system in words, for a model or a person to read first. |
| `tokens.css`, `tokens.json` | The `--q-*` variables: 14 themes and the radius, size, space, type and shadow values, each with the file and line it lives at. |
| `ISSUES.md`, `issues.json` | The 66 findings (42 settled, 24 open). `issues.json` is the source; each has a `decision` field. |
| `scales.json` | The hand-kept list of size tokens and where each lives in the code. |
| `build.py` | Rebuilds the HTML, the tokens and `ISSUES.md` from `theme.rs`, `scales.json` and `issues.json`. |
| `handoff.py` | Reads the design back and writes the implementer's brief. |

The ten maps the page was merged from (about 800 components with every property, both apps and
the prototype) are in `.audit/design-system/maps/`, which git does not track.

## Into Claude Design

Claude Design's help says a design system can come from "a GitHub repo, design files, raw
uploads, or your local codebase". It does not say which file types an upload takes, so this
has not been tried from here. In order of how likely each is to work:

1. Upload `quire-design-system.html`, `DESIGN.md` and `tokens.css` when you create the design
   system or a project.
2. Commit and push `design/system/`, then link the repository.
3. Run `/design-sync` in Claude Code from the repository. Reports describe it for React and
   Storybook projects; whether it reads a Rust app is unknown.

Then tell it how the page is built, so the way back keeps working:

> This page is Quire's design system as built. The tokens are the `--q-*` CSS variables in the
> `<style id="quire-tokens">` block; every component is drawn from them. Each component is an
> `<article data-component="…">` and each inconsistency an `<article data-issue="…">`. When we
> settle an inconsistency, change the tokens and components to match and write the decision
> into that card's `data-decision` attribute and its "Decision" line. Keep the variable names,
> the `data-component` and `data-issue` attributes and the file references. Give anything new
> a `data-component` name and a `data-new` attribute.

## Back to an implementer

Export the design as HTML (or a zip), then:

```sh
python3 design/system/handoff.py path/to/export.html -o HANDOFF.md
```

The brief lists each token that changed with the line it lives at in the app, each decision
with the places to change, and everything marked new. Give the implementer `HANDOFF.md`
and `ISSUES.md`. If the export keeps none of the markers, the script says so; then hand over
the Claude Design link itself, which Claude Code can read, together with `ISSUES.md`.

You can also skip Claude Design: fill the `decision` fields in `issues.json` and run
`handoff.py design/system/issues.json`.

## Keeping it true

After the app changes, correct `scales.json` and `issues.json` and run
`python3 design/system/build.py`. Theme colours are read from `theme.rs` on every build.

Two limits. The components are HTML stand-ins for what Rune draws, right in size and colour
and not pixel copies. Findings marked `reported` come from the Haiku maps and were not read
again against the code; those marked `checked` were.
