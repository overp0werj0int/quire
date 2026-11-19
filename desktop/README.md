# Quire for the desktop

Quire is a plain editor for folders of Markdown files. Each note is a `.md` file on your disk, so any other app can open it. This is the desktop app, built with [Rune](https://github.com/anomalyco/rune), a GPU-rendered Rust UI framework.

## Build and run

You need the Rust toolchain. `rust-toolchain.toml` pins the version that Rune uses, and `rustup` installs it on the first build. On Linux you also need the `libxkbcommon` and `wayland` development packages.

```sh
cargo run --release              # opens the last folder, or the welcome screen
cargo run --release -- ~/notes   # opens a folder
cargo run --release -- note.md   # opens a note in its folder
```

To install Quire for your user, with a launcher entry and an icon, run:

```sh
./install.sh
```

The script puts the binary in `~/.local/bin/quire`, the icon in `~/.local/share/icons` and `quire.desktop` in `~/.local/share/applications`.

## What it does

### Writing

- Edits the `.md` files of a folder. Saves each note half a second after you stop typing, and on quit.
- Shows source with live Markdown highlighting, or a rendered preview (`Ctrl+P`).
- Has a floating formatting toolbar at the top of the note in source mode. It stays in view, and the note scrolls under it. The buttons are bold, italic, strikethrough, heading, link, note link, lists, task, quote, code and image. `Ctrl+B`, `Ctrl+I` and `Ctrl+K` do the same as the first buttons. Turn off Settings, Editor, **Formatting toolbar** to hide it.
- Hides everything but the text in zen mode. Use `Ctrl+.` or the zen mode button in the title bar.
- Fits a narrow window. Below 680 points the sidebar button opens the sidebar over the page, and below 1100 points the outline button does the same for the outline. A press outside, Esc or the button closes it. So does switching between source and preview, or another note. A shortcut such as `Ctrl+F` closes it and then acts.
- Locks a note with its own password: use the lock button at the end of the toolbar, or "Lock note with a password…" in the command palette. Quire encrypts the note with AES-256-GCM and a key from the password. The file on disk, search, the link index and sync see only the encrypted text. To read the note, enter the password. Each save encrypts the note again. The note locks again when you close its tab or select the lock button. "Remove password" in the command palette saves it as plain text again. If you forget the password, nobody can open the note. `../SYNC.md` describes the format.

### Images

- Shows images in the preview: `![alt](path)` relative to the note, `![[name.png]]` anywhere in the folder, and `https://` images. PNG, JPEG, GIF, WebP, BMP, SVG and AVIF work.
- Adds an image when you drop it on a note, paste it, or pick it with the image button in the toolbar (`Ctrl+Shift+I`). Images from outside the folder are copied into `attachments/`. A pasted screenshot is saved as `attachments/Pasted image <date> <time>.png`.
- Takes file drops on Wayland too. Rune's winit gets no file drops from Wayland, so `vendor/winit` adds them. See `vendor/winit/src/platform_impl/linux/wayland/dnd.rs`.

### Links and search

- Links notes with `[[Note name]]`. `Ctrl`-click a link to follow it. A link to a missing note creates the note.
- Rewrites links in other notes when you rename a note.
- Finds tags (`#tag`), searches every note (`Ctrl+Shift+G`), and lists headings and backlinks.
- Opens any note with the quick switcher (`Ctrl+O`) and runs any command from the palette (`Ctrl+Shift+K`).
- Has two keymaps: omarky's, the default, and the keys Quire used before. Settings, Shortcuts picks one and lists every key.

### Folders

- Shows the folder in the sidebar: the search field and "+" at the top, the open notes on the darker Open shelf, the folder's notes under its label, and the tags and settings buttons at the foot. A click on the Open or folder label folds that list to one row, and dragging the bar between them moves the split. Quire remembers both, the same for every folder.
- Makes folders: right-click the folder's label in the sidebar or a folder, or use "New folder" in the command palette.
- Creates a note inside a folder: use the `+` on the folder's row, right-click the folder, or type `Folder/Name` in the quick switcher. A path creates the folders it names.
- Moves a note into a folder when you drag it onto the folder. Drop it on the folder's label or on the tree's empty space to move it to the root. Links that spell out the old folder follow the note.
- Moves deleted notes to `.trash` inside the folder.
- Deletes a folder from its right-click menu. A folder with notes asks first. The folder moves to `.trash` with every file in it, so you can get it back.
- After a delete, a toast names the note or folder and offers Undo for six seconds, on its button or with `Ctrl+Shift+R`. Undo moves it back from `.trash` and reopens its tab. Opening another folder ends the offer, so a note only ever goes back to its own folder.
- Picks up changes that other apps make to the folder while Quire is open.

### Look

- Sets notes in iA Writer Quattro S, with code in iA Writer Mono S, at 14 px. Settings, Editor picks the source-mode font (Quattro S, Duospace, Mono S, IBM Plex Mono, Courier Prime or Spline Sans Mono), the code font and the size, from 9 to 32 px. Settings, Appearance picks the preview's font: Quattro S, Newsreader or Instrument Sans.
- Sets the sidebar, the menus, key names and tooltips in iA Writer Quattro S. For the system's monospace font instead, set `"panel_font": "mono"` under `settings` in `state.json` (`~/.config/quire/state.json` on Linux); `"quattro"` is the default. Settings has no row for it.
- Has 14 themes in Settings, Appearance: 6 for light mode and 8 for dark mode. You pick one for each mode, and "Match system" switches between them.
- Works Settings from the keyboard. Tab and Shift+Tab move through the controls in reading order, with a ring in the accent on the one in focus. Space or Return presses it. The arrow keys move through a set of choices, such as the themes or a segmented control, and pick one. Esc closes Settings.
- Works every menu from the keyboard. Up and Down move through it, a letter jumps to an item, Enter or Space picks one and Esc closes it. Right-click the folder's label in the sidebar, or press `Shift+F10` on it, for its menu. `Shift+F10` or the Menu key opens the editor's menu below the caret.
- Works the sidebar's lists from the keyboard: the file tree, the Open list, the search results, the tags, and the outline's headings and backlinks. Tab enters a list at its current row and the next Tab leaves it. Up, Down, Home and End move. In the tree, Right opens a folder and Left closes it or goes to its folder. Enter opens the note, folder, heading, match or tag, and `Shift+F10` or the Menu key opens a tree row's menu. Esc gives the keyboard back to the note. A click on a row leaves the keyboard where it was.
- Reaches every control from the keyboard. Tab reaches the search field, "+" (New note), the Open and folder labels (Enter or Space folds the list under one), the tags and settings buttons, and the formatting toolbar (one stop, Left and Right inside it). `F6` and `Shift+F6` move between the sidebar, the note and the outline. Down in the search field moves into the results. While a control in a side panel or the toolbar has focus, no key reaches the note behind it, and when that control goes away the keyboard goes back to the note.
- Opens one thing over the page at a time: a menu, a drawer, a dialog or the palette. It holds the keyboard while it is open, and when it closes the keyboard goes back where it was.
- Lets you pick the accent colour from 9 swatches, or type a `#rrggbb` value. An accent that is too dark to read on a dark theme gets lighter for text.
- On Omarchy, offers the current Omarchy theme in both lists and follows theme switches while Quire runs. Without a pick, "Match system" uses the Omarchy theme.

### Window

- On Linux, the title bar has no window buttons. The window manager closes and places the window. To quit, press `Ctrl+Q` or use **Quit** in the command palette.
- On Wayland, a touchpad swipe scrolls a note as far as Chromium scrolls a web page. Rune's winit scales the touchpad value by 12, as Chromium does.
- On Linux, lifting the fingers mid-swipe throws the page on Chrome's fling curve. Resting the fingers on the touchpad stops it, and so does a new swipe. A quick second swipe the same way adds to the throw, as in Chrome.
- On macOS the shortcuts use `⌘` in place of `Ctrl`.

On first launch Quire writes a sample folder, Field Notes, to `~/Documents/Field Notes`. If that folder already exists, the sample goes to Quire's data folder instead. Settings and open tabs live in `state.json` in Quire's config folder, `~/.config/quire/` on Linux, never in a notes folder.

## MCP server

Quire can serve a folder to AI apps that speak the Model Context Protocol, such as Claude Desktop or Cursor. Turn it on in Settings, MCP. Settings, MCP also shows the client configuration to copy.

With the stdio transport, the AI app starts Quire's helper itself:

```json
{ "mcpServers": { "quire": { "command": "quire", "args": ["mcp", "--folder", "/path/to/notes"] } } }
```

With the HTTP transport, Quire serves `http://127.0.0.1:3917/mcp` while the app is open. Each request needs the access token from Settings as a `Bearer` header.

Access is read-only by default. Each tool has its own switch, and hidden folders stay out of listings and search. When "Ask before changes" is on, tools that write return an error, because this version cannot show the approval prompt yet. Turn the option off to allow edits.

## Sync

Settings, Sync & backup connects a notes folder to a bucket. **Test connection** reads the remote folder and reports what it holds. After it passes, Quire syncs when the folder opens, after you save, every 5 minutes, or only when you select **Sync now**, as you choose. The status bar shows the last sync. Select it to open the sync settings.

- Each notes folder has its own **Remote folder**. Without a value, it is `quire/<folder name>/`.
- Notes, images up to 50 MB and folders sync. A file that you delete on one device moves to `.trash` on the others.
- A request that the service does not answer in time is tried again up to 4 times. Transfers run 8 at a time.

Quire keeps a record of the last sync for each folder and bucket in `sync/` in its config folder. The record lets Quire tell an edit from a delete. See [`../SYNC.md`](../SYNC.md) for the format.

## Not in this version

- Scrolling the reader takes about 5 ms a frame on a 5,000-line note, because the whole page is painted each frame. omarky's budget is under 1 ms on average.
- The passphrase and keys you enter are saved in `state.json`, which only your user can read. They are not in the system keychain.
- The macOS and Windows builds come from CI and are not tested on those systems yet.

## Develop

Use `./dev` while you work. It checks and tests an edit in seconds, and it leaves the app build alone.

```sh
./dev check            # types and borrows only: 1 to 3 seconds after an edit. Repeat this while you edit.
./dev clippy           # the same, with lints
./dev test wr_118      # the tests whose names match, built in seconds
./dev test             # every test: about 10 seconds to build, 3.5 minutes to run
./dev build            # an unoptimized binary at target/fast/quire, to try a change at once
./dev app              # the app itself: plain `cargo build`, target/debug/quire. Once, at the end.
```

- To see what the app does with a click or a key, without opening a window, run `tests/probe.py` (`tests/probe.py --help`). It replays input headless and prints focus, an element's place, the page's scroll position or the saved notes, once or sampled over time.
- Check first. Run `./dev check` after each edit, `./dev test` with a filter for what you changed, and the full suite and `./dev app` only for the last pass.
- `./dev` compiles this crate unoptimized with the Cranelift backend on a nightly toolchain, in its own profile (`fast`) and its own folder (`target/fast`). Dependencies stay on LLVM. Install the toolchain once:

  ```sh
  rustup toolchain install nightly-2026-11-17 --profile minimal -c rustc-codegen-cranelift-preview -c clippy
  ```

- Plain `cargo build`, `cargo test` and release builds still use the pinned stable toolchain. They work as before and need no nightly.
- The fast binary runs this crate's code unoptimized, so its timings are not the app's: on a 1,753-line note a mode switch took 81 to 92 ms in it when the app build took 55 to 73 ms (7 to 16 ms in the app build since Rune 98955867). Use it for behaviour. Measure timing on `./dev app` or a release build.
- Every `./dev` command runs at the lowest CPU and disk priority, under a 2.4 GB memory cap, with two build jobs. `QUIRE_DEV_MEM` and `CARGO_BUILD_JOBS` change the limits.
- After a change that touches most of the crate, measured on a 4-core laptop: `./dev check` 1.4 s, `./dev test` builds in 9 s, `./dev build` 7 s, `./dev app` 40 s.

```sh
cargo test                                               # unit tests, optimized: slower to build, 90 seconds to run
cargo run -- --snap shot.png --size 1280x800 --scale 2   # render a frame headless
cargo run -- --snap shot.png --scene dark                # scenes: welcome, preview, dark, focus, settings
cargo run -- --script events.json --snap shot.png --at mark:end   # play input events, then render
```

- A headless run opens the folder in `QUIRE_FOLDER`, and the note `QUIRE_NOTE` in it if set. `--scene parity` shows the reader as omarky's screenshots do (VER-30): preview mode, body size 14, the Omarchy theme under `HOME`, no side panels. Set `HOME`, `XDG_CONFIG_HOME` and `XDG_DATA_HOME` to an empty folder, so the run does not touch your own settings.
- The sync tests run two devices against an S3 test server in the test process. Three more tests check a real store, and they only run when you ask for them. Each one reads the settings file in `QUIRE_LIVE_STATE`, and none of them prints a key:

  ```sh
  QUIRE_LIVE_STATE=~/.config/quire/state.json cargo test live_ -- --ignored --nocapture
  ```

  `live_store_check` also writes and deletes one small object, `.quire/probe.txt`.
- `QUIRE_SMOOTHING` picks another text rasterizer, for comparing renders. `ref/` holds renders of the design in Chromium, used as the reference for pixel comparisons.
- `vendor/winit` is Rune's winit with Wayland file drops and the touchpad scroll scale added. `Cargo.toml` points the `winit` patch at it.
