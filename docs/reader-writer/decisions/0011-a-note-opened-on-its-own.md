# 0011 A note opened on its own

Status: accepted, 2026-11-17.

The user wants Quire as the default app for `.md` files, including files outside the folders
in its list. Before this change, `quire path/to/note.md` opened the note's whole parent folder
as a notes folder. That had four side effects. The folder joined the list. It became the
folder opened at the next start. The app read and watched all of it. If sync was connected,
the app synced it.

Decision: the app opens a note on its own when the note is outside every folder in the list.
`cli::start_target` decides what the argument means:

- A folder opens as a notes folder, as before.
- A `.md` file inside a known folder, at any depth, opens that folder with the note active.
  The deepest known folder wins.
- Any other `.md` file opens on its own.
- Any other file is ignored, and the app starts as usual.

A vault opened this way has `Scope::Lone` (`vault.rs`). Its root is the note's folder, and it
holds that one note. While a note is open on its own:

- Only that note is read, and the watcher looks only at its file.
- The folder is not added to the list, does not become the folder opened next, and keeps no
  session.
- Sync and the MCP server's in-app endpoint leave it alone.
- Edits to the note save as usual. Making, renaming, moving or deleting notes and folders is
  refused with a notice, because those would act on a folder the user never added. A pasted
  or dropped image still goes to `attachments/` beside the note, because the user asked for
  that file in that place.
- The sidebar shows a line saying so and an "Add this folder" button in place of the file
  tree. The button opens the folder as a notes folder and keeps the note active.

A `Scope` value on `Vault`, rather than a flag on `App`, lets every place that cares ask the
open vault one question (`App::folder_open`).

Open limits:

- Only `.md` opens on its own. `.markdown` and other extensions do not.
- Each launch opens a new window. There is no single-instance reuse.
- Ctrl+click on a link to a missing note is refused, because it would make a note.

Test: `lone_tests::a_note_outside_the_list_opens_on_its_own` covers the start, the refusals,
saving, sync, the stored state and the "Add this folder" step.
`cli::tests::a_note_opens_in_its_known_folder_or_alone` covers how the argument is read, and
`watch::tests::a_lone_note_is_watched_alone` the watcher.
