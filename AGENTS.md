# Notes for coding agents

Read [`ARCHITECTURE.md`](ARCHITECTURE.md) before you change code. It explains the parts of
both apps, how a keystroke becomes a saved and synced file, and where the desktop and Android
apps differ.

- [`SYNC.md`](SYNC.md) is the contract between the two apps: the sync format, the locked-note
  format and the test values.
- [`desktop/README.md`](desktop/README.md) and [`android/README.md`](android/README.md) have
  the build, test and run commands.
- Desktop tests: `cd desktop && cargo test`. Android tests: `cd android && ./gradlew testDebugUnitTest`.
- The "Rules for changes" section at the end of `ARCHITECTURE.md` applies to every change.
- When a change makes `ARCHITECTURE.md` wrong, update it in the same change.
