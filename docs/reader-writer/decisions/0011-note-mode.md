# 0011 Note mode: notes opened as files

Status: accepted, 2026-11-17. Revised the same day after the first version was tried.

The user wants Quire as the default app for `.md` files, including files outside the folders
in its list. Before this change, `quire path/to/note.md` opened the note's whole parent folder
as a notes folder. That had four side effects. The folder joined the list. It became the
folder opened at the next start. The app read and watched all of it. If sync was connected,
the app synced it.

Decision: Quire has two separate modes.

- **Folder mode** treats a folder as a vault: files, search, tags, sync, the MCP server,
  sessions. This is the app as it was.
- **Note mode** is a plain Markdown editor. You open one or more note files, they show in the
  sidebar's Open list, and no folder features are available.

`cli::start_target` decides which mode the launch argument opens:

- A folder opens in Folder mode.
- A `.md` or `.markdown` file inside a known folder, at any depth, opens that folder in Folder
  mode with the note active. The deepest known folder wins.
- Any other `.md` or `.markdown` file opens in Note mode.
- Any other file is ignored, and the app starts as usual.

In Note mode each note is a vault of its own (`Scope::Note` in `vault.rs`), rooted at the
note's folder and holding only that note. So links, images and `attachments/` resolve beside
the note, as they would in a folder, and nothing reads the rest of the folder. `App::notes`
holds the Open list as full paths. The note shown is the open vault, and showing another
note swaps it in, read again from disk. Two notes with one name from different folders stay
apart.

A note that leaves the screen keeps its editor, with the caret and undo history, and its
plain text if it was unlocked (`Parked`, by full path). It gets them back when it shows again,
if the file still holds the text it had. If the file changed outside meanwhile, the note
comes back from disk instead, and a locked one locks again. Closing a note drops what it kept.

While in Note mode:

- The app starts in zen, in preview. Leaving zen shows the page with the sidebar collapsed.
- Every note newly opened shows in preview first.
- The sidebar holds only the Open list and Settings: no folder menu, files, search, tags or
  New note.
- Ctrl+O shows the system's file picker instead of the quick switcher. A link to a note file
  that exists opens it as one more note.
- Edits save as usual. Making, renaming, moving or deleting notes and folders is refused with a
  notice, and the command palette leaves those commands out. A pasted or dropped image still
  goes to `attachments/` beside the note, because the user asked for that file there.
- The watcher looks only at the shown note's file.
- Nothing is added to the folder list, the folder that opens next or the stored sessions.
  Sync and the MCP server's in-app endpoint stay off.
- Note mode's collapsed sidebar and its preview never reach `state.json`: Folder mode opens
  with its own sidebar setting and mode (`App::saved`).

Opening a folder through the command palette's "Switch folder…" leaves Note mode.

An earlier version of this decision kept the parent folder in the sidebar with an "Add this
folder" button. The user found that confusing and asked for the two modes instead.

Open limits:

- Each launch opens a new window. There is no single-instance reuse.
- A note open in the list but not shown isn't watched. It is read again when shown.

Test: `note_mode_tests::notes_open_as_files_in_note_mode` covers the start in zen and preview,
the collapsed sidebar and its Open list, the refusals, saving, Ctrl+O's file picker, two notes
of one name, links, closing notes, what reaches `state.json`, and leaving for a folder.
`note_mode_tests::a_note_keeps_its_undo_and_unlock_across_switches` covers what a note keeps
while another shows, and a note changed outside meanwhile.
`cli::tests::a_note_opens_in_its_known_folder_or_alone` covers how the argument is read, and
`watch::tests::a_lone_note_is_watched_alone` the watcher.
