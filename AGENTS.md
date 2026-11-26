# Notes for coding agents

Read [`ARCHITECTURE.md`](ARCHITECTURE.md) before you change code. It explains the parts of
both apps, how a keystroke becomes a saved and synced file, and where the desktop and Android
apps differ.

- [`SYNC.md`](SYNC.md) is the contract between the two apps: the sync format, the locked-note
  format and the test values.
- [`desktop/README.md`](desktop/README.md) and [`android/README.md`](android/README.md) have
  the build, test and run commands.
- Desktop: work with `cd desktop && ./dev check` after each edit, and `./dev test <filter>` for what you changed. Run the full `./dev test` and `./dev app` only for the last pass. Do not use `cargo build` or `cargo test` between edits: they take minutes where `./dev check` takes seconds. See "Develop" in `desktop/README.md`.
- Desktop: to see what the app does, use `desktop/tests/probe.py`. It replays clicks and keys
  in a headless Quire and prints where focus is, where an element sits, how far the page has
  scrolled, or what was saved. Use it before a change, to see the behaviour you are about to
  change, and after, to see the new one, before you write or trust a test. From `desktop/`:

  ```sh
  tests/probe.py "tap:editor@0.1 key:End+cmd@0.4 type:hello@0.8 key:f+cmd@1.6" --print focus,rect:find-bar
  tests/probe.py "tap:editor@0.1 key:End+cmd@0.5 key:s+cmd@1.0" --series 1.0:1.6:0.05
  ```

  The first prints one reading. The second samples the same replay over time and prints how
  far the page moved between samples, which is how to see a jump, a glide or a bounce.
  `--print keys` lists the element keys you can name. `--fast` uses the `./dev build` binary.
  `tests/probe.py --help` has the rest. Run one probe at a time: each is a whole copy of the
  app. It needs a built binary and nothing else, and never touches your own notes or settings.
- Desktop behaviour cases: `desktop/tests/cases.py` runs `tests/cases/**/*.case`. Each case is
  a note, a probe replay and the note expected on disk after it (plus optional probe readings).
  Prefer a case to a unit test for anything a user can do: it checks what the user gets.
- Android tests: `cd android && ./gradlew testDebugUnitTest`.
- [`docs/reader-writer/`](docs/reader-writer/README.md) is the plan for bringing omarky's
  reader and writer to the desktop app. Its `behavior/` files say what the preview and the
  editor must do. Read the matching file before you change either, and run
  `python3 docs/reader-writer/tools/check-specs.py` after you edit one.
- Desktop UI design system: read [`design/system/DESIGN.md`](design/system/DESIGN.md),
  [`desktop/src/ui/tokens.rs`](desktop/src/ui/tokens.rs),
  [`desktop/src/ui/styles.rs`](desktop/src/ui/styles.rs), and
  [`desktop/src/ui/components/mod.rs`](desktop/src/ui/components/mod.rs) before adding or
  changing desktop UI.
  - Always reuse existing components from `desktop/src/ui/components/` (`Button`, `IconButton`,
    `input_frame` / `password_field` / `literal_field`, `choice_card` / `choice_pill` /
    `checkbox_box`, `switch`, `segmented` / `pill_group`, `kbd` / `key_hint`, `count_pill` /
    `tag_badge` / `status_dot` / `error_line`, `mini_bar` / `mini_button`, `dialog_frame` /
    `popup_frame` / `bar_edge` / `faded`, `Control` / `label` / `bar_sep`). Never hand-roll
    buttons, icon buttons, inputs, choice cards/pills, checkboxes, badges, error lines, popups,
    or modal dialogs inside a layout file.
  - Create a new component type only when a screen genuinely needs a distinct pattern that
    existing components cannot express cleanly. Place it in its own module under
    `desktop/src/ui/components/`, compose it from `ui/tokens.rs` (`space`, `radius`, `size`,
    `state`, `motion`, `Elevation`, `FocusRing`, `Zone`), `ui/styles.rs` type constructors, and
    more basic components, and export it from `desktop/src/ui/components/mod.rs`.
  - Keep the structural guardrails in `ui/tokens.rs` and `ui/styles.rs` passing, and when a
    change alters shared tokens or components, update `design/system/` and run
    `python3 design/system/build.py`.
- The "Rules for changes" section at the end of `ARCHITECTURE.md` applies to every change.
- When a change makes `ARCHITECTURE.md` wrong, update it in the same change.
