//! Quire: a plain editor for folders of Markdown files.
// On Windows the app opens no console window. `quire mcp` still talks over the pipes its
// client gives it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
#[cfg(test)]
mod bench;
mod blocks;
mod cli;
mod commands;
mod folders;
mod format;
mod images;
mod keymap;
mod keys;
mod lock;
mod lock_app;
mod mcp;
mod highlight;
mod motion;
mod page;
mod reader;
mod writer;
mod writer_app;
mod md;
mod roles;
mod sample;
mod sheet;
mod store;
mod sync;
mod theme;
mod ui;
mod vault;
mod watch;

use rune::AppSpec;

use app::App;

fn scene_welcome(s: &mut App) {
    s.screen = app::Screen::Welcome;
    s.refresh_counts();
}
fn scene_preview(s: &mut App) {
    s.mode = app::Mode::Preview;
}
fn scene_dark(s: &mut App) {
    s.persist.settings.theme = "dark".into();
}
fn scene_focus(s: &mut App) {
    s.focus = true;
}
fn scene_settings(s: &mut App) {
    s.settings_open = true;
}
/// The reader as omarky's screenshots show it, for the parity pass (VER-30): preview mode, body
/// size 14, the Omarchy theme under `HOME`, no side panels. With `QUIRE_FOLDER` and `QUIRE_NOTE`.
fn scene_parity(s: &mut App) {
    s.mode = app::Mode::Preview;
    let set = &mut s.persist.settings;
    set.theme = "system".into();
    set.light_theme = None;
    set.dark_theme = None;
    set.size = 14;
    s.persist.left_open = false;
    s.persist.right_open = false;
}
fn scene_keys(s: &mut App) {
    s.settings_open = true;
    s.set_sec = app::Sec::Keys;
}

/// Text rasterization. `QUIRE_SMOOTHING` picks another mode, for comparing renders.
fn smoothing() -> rune::FontSmoothing {
    use rune::FontSmoothing as F;
    match std::env::var("QUIRE_SMOOTHING").as_deref() {
        Ok("windfoil-chromium") => F::WindfoilChromium,
        Ok("hinted") => F::Hinted,
        Ok("hinted-gpu") => F::HintedGpu,
        Ok("slug") => F::Slug,
        Ok("windfoil") => F::Windfoil,
        Ok("off") => F::Off,
        Ok("on") => F::On,
        _ => default_smoothing(),
    }
}

fn default_smoothing() -> rune::FontSmoothing {
    // Each platform's browser text, since the design is drawn in a browser.
    if cfg!(target_os = "macos") {
        rune::FontSmoothing::On
    } else if cfg!(target_os = "windows") {
        rune::FontSmoothing::DirectWrite
    } else {
        rune::FontSmoothing::HintedGpu
    }
}

fn main() {
    if cli::run_subcommand() {
        return;
    }
    rune::prewarm_gpu();
    rune::set_app_id("quire");
    // The Dock icon on macOS, and the title bar and taskbar icon on Windows. Linux takes the
    // icon from the launcher entry.
    if !cfg!(target_os = "linux") {
        let _ = rune::set_app_icon(&include_bytes!("../packaging/quire-512.png")[..]);
    }
    let spec = AppSpec {
        min_size: Some((520.0, 400.0)),
        fonts: theme::fonts(),
        families: ("Instrument Sans", "IBM Plex Mono"),
        on_key: Some(keys::on_key),
        poll: Some(|s: &mut App, t, tx| s.poll(t, tx)),
        setup: Some(|s: &mut App, waker| s.set_waker(waker)),
        scenes: &[("welcome", scene_welcome), ("preview", scene_preview), ("dark", scene_dark), ("focus", scene_focus), ("settings", scene_settings), ("keys", scene_keys), ("parity", scene_parity)],
        ..AppSpec::new("Quire", (1280.0, 800.0), app::init, ui::view)
    };
    rune::App::new(spec)
        .font_smoothing(smoothing())
        .chrome(rune::Chrome { full_size_content: true, traffic_lights: Some((14.0, 13.0)), drag_regions: true })
        .titlebar(rune::Titlebar::new(ui::titlebar::BAR_H))
        .on_close(|s: &mut App, _tx| s.flush_all())
        .run();
}

#[cfg(test)]
pub(crate) mod scroll_tests {
    use rune::{Env, Runtime};

    /// Tests that set HOME and the XDG folders hold this lock, because the variables are global.
    pub(crate) static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn settings_body_scrolls() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-scroll-{}", std::process::id()));
        // SAFETY: tests in this module run before any thread reads these variables.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.settings_open = true;
        s.set_sec = crate::app::Sec::Mcp;
        s.persist.settings.mcp.enabled = true;
        let mut rt = Runtime::new(s, crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        for _ in 0..3 {
            rt.frame(1.0 / 60.0);
        }
        let before = rt.presented("client").map(|p| p.0[1]);
        let (x, y) = rt.find("set-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -300.0);
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
        let after = rt.presented("client").map(|p| p.0[1]);
        eprintln!("client y before {before:?} after {after:?}");
        assert!(after < before, "the settings body scrolls");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A notes folder in `dir` holding `files`, its first file open in preview, and the
    /// watcher looking every `every`.
    fn vault(dir: &std::path::Path, files: &[(&str, &str)], every: std::time::Duration) -> Runtime<crate::app::App> {
        use crate::bench::DT;
        let _ = std::fs::remove_dir_all(dir);
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        for (name, text) in files {
            std::fs::write(root.join(name), text).unwrap();
        }
        // SAFETY: the caller holds the ENV lock, which keeps other tests that read these out.
        unsafe {
            std::env::set_var("HOME", dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut s = crate::app::init();
        s.watch_every = every;
        // The watcher starts with a waker, as in the app; the frames below stand in for wakes.
        s.waker = Some(std::sync::Arc::new(|| {}));
        let mut rt = Runtime::new(s, crate::ui::view, (1400.0, 1000.0), Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        rt.frame(DT);
        let first = files[0].0.to_string();
        rt.update(move |s, tx| {
            s.open_vault(root, tx);
            s.open_file(&first, None, tx);
            s.mode = crate::app::Mode::Preview;
        });
        for _ in 0..4 {
            rt.frame(DT);
        }
        rt
    }

    /// RD-19, VER-34: omarky's check-live sequence against the open note. After each outside
    /// change the same block stays at the top, at the same place.
    #[test]
    fn rd_19_live_reload_keeps_the_top_block() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-live-{}", std::process::id()));
        let blocks: Vec<String> = (0..80).map(|i| format!("Paragraph {i}, which says something of its own and runs on to fill the line.")).collect();
        let text = blocks.join("\n\n") + "\n";
        let mut rt = vault(&dir, &[("note.md", &text)], std::time::Duration::from_millis(50));
        let path = dir.join("notes/note.md");
        let line = text.lines().position(|l| l.starts_with("Paragraph 40,")).unwrap();
        rt.update(move |s, tx| s.jump_block(line, tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let key = crate::reader::block_key(&crate::blocks::build(&text).blocks[40]);
        let place = |rt: &mut Runtime<crate::app::App>| {
            let mut top = String::new();
            rt.update_if(|s, _| {
                top = crate::reader::block_key(&s.doc().blocks[s.top()]);
                false
            });
            (top, rt.presented(&key).map(|p| p.0[1].round()))
        };
        let start = place(&mut rt);
        assert_eq!(start.0, key, "paragraph 40 at the top");
        assert!(start.1.is_some(), "its row is drawn");
        // Waits until the app holds `want`, or 3 s.
        let settle = |rt: &mut Runtime<crate::app::App>, want: &str| {
            let t = std::time::Instant::now();
            loop {
                rt.frame(DT);
                let mut now = String::new();
                rt.update_if(|s, _| {
                    now = s.files().get("note.md").cloned().unwrap_or_default();
                    false
                });
                if now == want || t.elapsed().as_secs_f32() > 3.0 {
                    assert_eq!(now, want, "the app read the change");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            for _ in 0..30 {
                rt.frame(DT);
            }
        };
        let mut text = text;
        // Append.
        text.push_str("\nAppended at the end.\n");
        std::fs::write(&path, &text).unwrap();
        settle(&mut rt, &text);
        assert_eq!(place(&mut rt), start, "after an append");
        // Three blocks above the reading position.
        text = format!("# Inserted\n\nAbove, one.\n\nAbove, two.\n\n{text}");
        std::fs::write(&path, &text).unwrap();
        settle(&mut rt, &text);
        assert_eq!(place(&mut rt), start, "after three blocks above");
        // An editor's atomic save: a temporary file renamed over the note.
        text = text.replace("Paragraph 70,", "Paragraph seventy,");
        let tmp = dir.join("notes/.note.md.tmp");
        std::fs::write(&tmp, &text).unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        settle(&mut rt, &text);
        assert_eq!(place(&mut rt), start, "after a rename over the note");
        // A burst of ten writes.
        for i in 0..10 {
            text.push_str(&format!("\nBurst {i}.\n"));
            std::fs::write(&path, &text).unwrap();
        }
        settle(&mut rt, &text);
        assert_eq!(place(&mut rt), start, "after a burst of writes");
        // Deleted and written back within one look.
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, &text).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        settle(&mut rt, &text);
        let mut open = None;
        rt.update_if(|s, _| {
            open = s.active.clone();
            false
        });
        assert_eq!(open.as_deref(), Some("note.md"), "the note stays open");
        assert_eq!(place(&mut rt), start, "after a delete and restore");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-34, RD-35: preview mode's title bar buttons, and Find showing as on while the bar
    /// is open.
    #[test]
    fn rd_34_35_preview_buttons() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-buttons-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nText.\n")], crate::watch::EVERY);
        for key in ["find", "copy-all", "wide", "toggle-right", "help"] {
            assert!(rt.find(key).is_some(), "{key} in preview");
        }
        let (x, y) = rt.find("find").unwrap();
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut open = false;
        rt.update_if(|s, _| {
            open = s.reader.find.open;
            false
        });
        assert!(open, "the Find button opens the bar");
        let find = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("find")).expect("the Find button");
        assert!(find.fill.a > 0.0, "and shows as on, with the hover fill");
        assert_eq!(find.help.as_deref(), Some("Find  Ctrl+F"), "its tooltip names the preset's key");
        rt.update(|s, tx| s.toggle_mode(tx));
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert!(rt.find("copy-all").is_none(), "source mode keeps its own bar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-102: `[[other#Far Heading]]` opens the other note and travels to that heading.
    #[test]
    fn rd_102_wiki_link_to_a_heading() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-wiki-{}", std::process::id()));
        let para = "A paragraph long enough to take a line or two of the page, said again. ".repeat(3);
        let other = format!("# Other\n\n{}## Far Heading\n\nThe end.\n\n{}", format!("{para}\n\n").repeat(60), format!("{para}\n\n").repeat(30));
        let mut rt = vault(&dir, &[("note.md", "See [[other#Far Heading]].\n"), ("other.md", &other)], crate::watch::EVERY);
        rt.update(|s, tx| s.open_link("other#Far Heading", tx));
        for _ in 0..90 {
            rt.frame(DT);
        }
        let doc = crate::blocks::build(&other);
        let i = doc.blocks.iter().position(|b| b.lines.start == other.lines().position(|l| l == "## Far Heading").unwrap()).unwrap();
        let mut got = (String::new(), 0);
        rt.update_if(|s, _| {
            got = (s.active.clone().unwrap_or_default(), s.top());
            false
        });
        assert_eq!(got, ("other.md".to_string(), i), "the other note, its heading at the top");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The page is a virtual list (RD-25, RD-57): a jump and a find reach blocks far below
    /// the rows built so far, land where they would on a page built whole, and the block at
    /// the top follows.
    #[test]
    fn the_page_reaches_rows_not_built() {
        use crate::bench::{DT, LONG, open};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-virtual-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Preview);
        let doc = crate::blocks::build(LONG);
        let line = LONG.lines().position(|l| l == "# Chapter 42: Code").unwrap();
        let i = doc.blocks.iter().position(|b| b.lines.start == line).unwrap();
        rt.update(|s, tx| s.jump_block(line, tx));
        rt.frame(DT);
        // RD-24: a long jump cuts to 0.8 of a view short, then glides the rest. Measured on
        // screen: the offset alone also moves as rows above measure, with nothing moving. The
        // cut is placed by the heading's row once it lays out, a frame after the jump.
        rt.frame(DT);
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let key = crate::reader::block_key(&doc.blocks[i]);
        let gap = crate::page::gap(14.0, i.checked_sub(1).map(|p| crate::page::rk(&doc.blocks[p].kind)), crate::page::rk(&doc.blocks[i].kind));
        let short = rt.presented(&key).map(|r| r.0[1] + gap - page[1] - 20.0);
        assert!(short.is_some_and(|d| d > 0.0 && d <= 0.8 * page[3] + 1.0), "the first frame lands short of the heading: {short:?} of a {} view", page[3]);
        for _ in 0..90 {
            rt.frame(DT);
        }
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let row = rt.presented(&crate::reader::block_key(&doc.blocks[i])).expect("the row is built").0;
        let mut gap = 0.0;
        let mut top = 0;
        rt.update_if(|s, _| {
            gap = crate::page::gap(s.b(), i.checked_sub(1).map(|p| crate::page::rk(&doc.blocks[p].kind)), crate::page::rk(&doc.blocks[i].kind));
            top = s.top();
            false
        });
        assert!((row[1] + gap - page[1] - 20.0).abs() < 1.0, "the heading lands 20 px under the top: row {row:?}, gap {gap}, page {page:?}");
        assert_eq!(top, i, "the top block follows the jump");

        // From the top, four steps on reach a match in the middle of the note, far past the
        // rows built.
        rt.update(|s, tx| s.scroll_top(tx));
        for _ in 0..90 {
            rt.frame(DT);
        }
        rt.update(|s, tx| {
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("OmarkyReader from anywhere");
            s.find_changed(tx);
            assert_eq!(s.find_count(), 0, "the search waits for typing to pause (RD-56)");
        });
        for _ in 0..90 {
            rt.frame(DT);
        }
        let mut cur = 0;
        rt.update(|s, tx| {
            (0..4).for_each(|_| s.find_step(true, tx));
            cur = s.reader.find.cur;
        });
        assert_eq!(cur, 4, "find starts at the first match from the top (RD-57)");
        for _ in 0..90 {
            rt.frame(DT);
        }
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
        let mid = page[1] + page[3] / 2.0;
        assert!((y - mid).abs() < 2.0, "the match sits mid-page: at {y}, middle {mid}");

        // RD-60 (adapted): a match whose block shows between 20% and 75% of the view stays put;
        // shown above that, it is centred.
        let h = page[3];
        let nudge = |rt: &mut rune::Runtime<crate::app::App>, by: f32| {
            rt.update(|s, tx| {
                let at = s.reader.scroll.get()[0];
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, at + by);
            });
            rt.frame(DT);
            rt.update(|s, tx| s.center_match(tx));
            for _ in 0..60 {
                rt.frame(DT);
            }
            (rt.find(crate::reader::FIND_CUR).unwrap().1 - page[1]) / h
        };
        let at = nudge(&mut rt, 0.2 * h);
        assert!((at - 0.3).abs() < 0.01, "a match at 30% stays: {at}");
        let at = nudge(&mut rt, 0.25 * h);
        assert!((at - 0.5).abs() < 0.02, "a match at 5% is centred: {at}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod theme_choice_tests {
    /// Each mode uses its own pick, Paper and Ink are the defaults, and the accent recolours
    /// the palette.
    #[test]
    fn each_mode_uses_its_own_theme() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-themes-{}", std::process::id()));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        assert_eq!((s.theme_id(false), s.theme_id(true)), ("paper".to_string(), "ink".to_string()));
        s.persist.settings.light_theme = Some("gruvbox-light".into());
        s.persist.settings.dark_theme = Some("nord".into());
        s.persist.settings.theme = "system".into();
        assert!(std::ptr::eq(s.palette(true), crate::theme::theme_pal("nord", true)));
        assert!(std::ptr::eq(s.palette(false), crate::theme::theme_pal("gruvbox-light", false)));
        s.persist.settings.theme = "light".into();
        assert!(!s.palette(true).dark, "Light mode ignores the system");
        s.persist.settings.accent = "#1f8a80".into();
        assert_eq!(format!("{:?}", s.palette(false).button), format!("{:?}", crate::theme::hex("#1f8a80")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod toolbar_tests {
    use rune::{Env, Runtime};

    #[test]
    fn toolbar_actions_edit_the_selection() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toolbar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        let mut out = String::new();
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            let text = "Intro — with a dash\nhello world".to_string();
            let a = s.active.clone().unwrap();
            s.set_text(&a, text.clone());
            let start = text.find("hello").unwrap();
            s.cur_edit().set_selection(start, text.len());
            s.format(crate::format::Fmt::Bold, tx);
            s.format(crate::format::Fmt::Bullet, tx);
            out = s.active_text().to_string();
        });
        assert_eq!(out, "Intro — with a dash\n- **hello world**");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod lock_tests {
    use rune::{Env, Runtime};

    /// Lock a note, lock it again, unlock it, then remove the password. The folder copy and
    /// the file stay sealed until the password goes.
    #[test]
    fn a_note_locks_and_unlocks() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        let set = |s: &mut crate::app::App, key: &str, v: &str| s.field(key, String::new).replace_text(v.to_string());
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "# Secret\n\nThe code is 4711.".into());
            s.ask_lock(tx);
            set(s, "lock-pass", "short");
            set(s, "lock-repeat", "short");
            s.lock_note(tx);
            assert!(s.lock_dialog.as_deref().is_some_and(|e| e.contains("at least 8")), "a short password is refused");
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
            assert!(s.lock_dialog.is_none());
            assert!(crate::lock::is_locked(&s.files()[&a]), "the folder copy is sealed");
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 4711.", "the note stays open");
            // An edit while open is sealed too.
            s.set_text(&a, "# Secret\n\nThe code is 1234.".into());
            assert!(!s.files()[&a].contains("1234"));
            s.relock();
            assert!(s.note_locked(&a) && s.active_text().is_empty());
            set(s, "unlock-pass", "wrong password");
            s.unlock(tx);
            assert_eq!(s.unlock_error.as_deref(), Some("The password does not open this note."));
            set(s, "unlock-pass", "tiger lily");
            s.unlock(tx);
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 1234.");
            s.remove_password(tx);
            assert_eq!(s.files()[&a], "# Secret\n\nThe code is 1234.");
            s.writer.wait(std::time::Duration::from_secs(5));
            let disk = std::fs::read_to_string(s.vault.as_ref().unwrap().path.join(&a)).unwrap();
            assert_eq!(disk, "# Secret\n\nThe code is 1234.");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod a11y_tests {
    use rune::accesskit::Role;
    use rune::{Env, Runtime};

    /// The source editor lays out by paragraph; a screen reader still gets each line once, its
    /// newline last.
    #[test]
    fn the_editor_exposes_each_line_once() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-a11y-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "# Title\n\n#draft\n\nThe text".into());
            s.focus_editor(tx);
        });
        for _ in 0..3 {
            rt.frame(1.0 / 60.0);
        }
        let tree = rt.accessibility_tree();
        let (id, _) = tree.nodes.iter().find(|(_, n)| n.author_id() == Some("editor")).expect("the editor");
        let runs: Vec<String> = tree
            .nodes
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.children().to_vec())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| tree.nodes.iter().find(|(i, n)| i == c && n.role() == Role::TextRun))
            .map(|(_, n)| n.value().unwrap_or("").to_string())
            .collect();
        assert_eq!(runs.concat(), "# Title\n\n#draft\n\nThe text", "runs: {runs:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod chip_tests {
    use rune::{Env, Runtime};

    const DT: f64 = 1.0 / 60.0;

    /// BLK-51: a click on a code block's chip copies the code and the chip says Copied for
    /// 1400 ms, shown with the pointer gone, then gives way to the badge again.
    #[test]
    fn blk_51_the_chip_says_copied_for_1400_ms() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-chip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(DT);
        rt.update(|s, _| {
            s.mode = crate::app::Mode::Preview;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "Text\n\n```rust\nfn main() {}\n```\n".into());
        });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let (x, y) = rt.find("copy").expect("the chip");
        rt.pointer_move(x, y);
        for _ in 0..20 {
            rt.frame(DT);
        }
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(DT);
        let mut at = 0.0;
        rt.update_if(|s, _| {
            at = s.reader.copied.expect("the chip says Copied").1;
            assert_eq!(s.reader.copied.map(|c| c.0), Some(2), "the block's first source line");
            false
        });
        rt.pointer_leave();
        // The app's clock is `poll`'s, which a windowless runtime does not call: set it.
        let mut copied = None;
        rt.update(move |s, tx| {
            s.poll(at + 1.399, tx);
        });
        for _ in 0..20 {
            rt.frame(DT);
        }
        rt.update_if(|s, _| {
            copied = s.reader.copied;
            false
        });
        assert!(copied.is_some(), "still Copied at 1399 ms");
        let shown = rt.presented("copy").map(|p| p.1).unwrap_or(0.0);
        assert!(shown > 0.99, "the chip stays shown with the pointer gone: opacity {shown}");
        rt.update(move |s, tx| {
            s.poll(at + 1.401, tx);
        });
        rt.update_if(|s, _| {
            copied = s.reader.copied;
            false
        });
        assert!(copied.is_none(), "gone at 1401 ms");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// BLK-15: the heading's `#` fades in and out over 120 ms. BLK-30: an open task box takes
    /// its accent border at once under the pointer.
    #[test]
    fn blk_15_30_hover_timing() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-hover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(DT);
        rt.update(|s, _| {
            s.mode = crate::app::Mode::Preview;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "# Title\n\nText\n\n- [ ] task\n".into());
        });
        // Past the page's own fade in.
        for _ in 0..90 {
            rt.frame(DT);
        }
        let shown = |rt: &Runtime<_>, key: &str| rt.presented(key).map_or(-1.0, |p| p.1);
        let (ax, ay) = rt.find("anchor").expect("the heading's #");
        assert!(shown(&rt, "anchor") < 0.01, "hidden at rest");
        rt.pointer_move(ax + 80.0, ay);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mid = shown(&rt, "anchor");
        assert!((0.15..0.85).contains(&mid), "half way in at 50 ms: {mid}");
        for _ in 0..8 {
            rt.frame(DT);
        }
        assert!(shown(&rt, "anchor") > 0.99, "in by 120 ms");
        let (hx, hy) = rt.find("hot").expect("the open box's accent edge");
        assert!(shown(&rt, "hot") < 0.01, "faint at rest");
        rt.pointer_move(hx, hy);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mid = shown(&rt, "anchor");
        assert!((0.15..0.85).contains(&mid), "half way out at 50 ms: {mid}");
        assert!(shown(&rt, "hot") > 0.99, "accent at once: {}", shown(&rt, "hot"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod writer_chrome_tests {
    use crate::bench::{DT, LONG, open};

    fn info(rt: &rune::Runtime<crate::app::App>, key: &str) -> Option<rune::ElementInfo> {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key))
    }

    /// WR-102: a jump in the writer puts its line `min(max(12% of the view, 56), view / 3)`
    /// under the view's top; with typewriter scrolling (WR-86) it centres the caret instead.
    #[test]
    fn wr_102_86_a_jump_places_its_line() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        for typewriter in [false, true] {
            let dir = std::env::temp_dir().join(format!("quire-place-{typewriter}-{}", std::process::id()));
            let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
            rt.update(|s, _| s.persist.settings.typewriter_scroll = typewriter);
            for _ in 0..10 {
                rt.frame(DT);
            }
            let line = LONG.lines().count() * 2 / 3;
            rt.update(move |s, tx| s.source_jump(line, tx));
            for _ in 0..90 {
                rt.frame(DT);
            }
            let page = rt.presented(crate::reader::SCROLL).unwrap().0;
            let caret = info(&rt, "editor").and_then(|e| e.caret).expect("the editor shows a caret");
            let vh = page[3];
            let (at, want) = if typewriter {
                (caret[1] + caret[3] / 2.0 - page[1], vh / 2.0)
            } else {
                (caret[1] - page[1], (vh * 0.12).max(56.0).min(vh / 3.0))
            };
            assert!((at - want).abs() < caret[3], "typewriter {typewriter}: the line sits {at} under the top, want {want}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// WR-118: find selects the first match after the caret and brings it to the view's middle.
    #[test]
    fn wr_118_find_centres_its_match() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-find-centre-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let from = LONG[LONG.len() * 2 / 3..].find(" the ").unwrap() + LONG.len() * 2 / 3;
        rt.update(move |s, tx| {
            if let Some(e) = s.cur_edit_opt() {
                e.set_selection(from, from);
            }
            s.reader.find.q = rune::TextEdit::new("the".to_string());
            s.writer_find(tx);
        });
        for _ in 0..90 {
            rt.frame(DT);
        }
        // Closing the bar gives the editor back its focus, and its caret ends the match.
        rt.update(|s, tx| {
            s.close_find(tx);
            s.focus_editor(tx);
        });
        for _ in 0..5 {
            rt.frame(DT);
        }
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let caret = info(&rt, "editor").and_then(|e| e.caret).expect("the editor shows a caret");
        let at = caret[1] + caret[3] / 2.0 - page[1];
        assert!((at - page[3] / 2.0).abs() < caret[3], "the match sits {at} under the top of a {} view", page[3]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-93: the thumb is hidden at rest, shows while the note scrolls, and is as tall as the
    /// view is of the note; WR-177: the bands sit at the top and foot of the sheet and let
    /// presses through.
    #[test]
    fn wr_93_177_the_thumb_and_the_fade_bands() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-thumb-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.settings.toolbar = false);
        for _ in 0..60 {
            rt.frame(DT);
        }
        let shown = |rt: &rune::Runtime<crate::app::App>| rt.presented("thumb").map_or(0.0, |p| p.1);
        assert!(shown(&rt) < 0.01, "no thumb at rest: {}", shown(&rt));
        let area = rt.presented("doc-area").unwrap().0;
        let (top, foot) = (info(&rt, "fade-top").expect("the top band"), info(&rt, "fade-foot").expect("the foot band"));
        assert!(top.inert && foot.inert, "the bands let presses through");
        assert_eq!((top.rect[1], top.rect[3]), (area[1], 28.0), "top band {:?} in {area:?}", top.rect);
        assert_eq!((foot.rect[1] + foot.rect[3], foot.rect[3]), (area[1] + area[3], 64.0), "foot band {:?}", foot.rect);
        let (x, y) = rt.find("doc-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -400.0);
        for _ in 0..20 {
            rt.frame(DT);
        }
        assert!(shown(&rt) > 0.4, "the thumb shows while scrolling: {}", shown(&rt));
        let mut v = [0.0; 4];
        rt.update_if(|s, _| {
            v = s.reader.scroll.get();
            false
        });
        let thumb = rt.presented("thumb").unwrap().0;
        let h = 24f32.max(v[2] * v[2] / (v[1] + v[2]));
        assert!((thumb[3] - h).abs() < 0.5 && thumb[2] == 3.0, "thumb {thumb:?}, view {v:?}");
        for _ in 0..90 {
            rt.frame(DT);
        }
        assert!(shown(&rt) < 0.05, "the thumb fades 700 ms after the last move: {}", shown(&rt));
        // With the toolbar on, its own fade does the top band's job.
        rt.update(|s, _| s.persist.settings.toolbar = true);
        rt.frame(DT);
        for _ in 0..30 {
            rt.frame(DT);
        }
        let band = info(&rt, "fade-top").map(|e| (e.exiting, e.opacity));
        assert!(band.is_none(), "no top band under the toolbar: {band:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The caret's rect in the writer with the caret at byte `at`.
    fn caret_at(rt: &mut rune::Runtime<crate::app::App>, at: usize) -> [f32; 4] {
        rt.update(move |s, _| s.cur_edit().set_selection(at, at));
        rt.frame(DT);
        rt.frame(DT);
        info(rt, "editor").and_then(|e| e.caret).expect("a caret")
    }

    /// WR-16, WR-19, WR-22, WR-25, WR-26, WR-225: marks hang in the gutter, so each line's
    /// text starts on the column edge; a nested item's spaces stay in the column; wrapped lines
    /// align with the item's text; the marks are text the caret steps through and a click in
    /// the gutter reaches.
    #[test]
    fn wr_16_225_marks_hang_in_the_gutter() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-hang-{}", std::process::id()));
        let long = "word ".repeat(60);
        let text = format!("# Title\nPlain line\n- item\n  - nested\n> quote\n[^1]: note\n- [ ] task\n- {long}\n");
        let (mut rt, _) = open(&dir, &text, crate::app::Mode::Source);
        let at = |line: usize| text.split('\n').take(line).map(|l| l.len() + 1).sum::<usize>();
        let edge = caret_at(&mut rt, at(1))[0];
        let field = info(&rt, "editor").unwrap().rect;
        let mut size = 0.0;
        rt.update_if(|s, _| {
            size = s.settings().size as f32;
            false
        });
        let gutter = crate::ui::editor::hang_gutter(size);
        assert!((field[0] + gutter - edge).abs() < 1.0, "plain text starts a gutter into the field: {field:?}, edge {edge}");
        for (line, mark, what) in [(0, 2, "heading"), (2, 2, "item"), (4, 2, "quote"), (6, 2, "task item")] {
            let x = caret_at(&mut rt, at(line) + mark)[0];
            assert!((x - edge).abs() < 1.0, "the {what}'s text starts on the column edge: {x} vs {edge}");
            let start = caret_at(&mut rt, at(line))[0];
            assert!(start < edge - 4.0 && start >= field[0] - 0.5, "the {what}'s mark hangs in the gutter: {start}");
        }
        // "[^1]: " is wider than the gutter at 14: it starts at the gutter's outer edge and its
        // text a little past the column edge (Rune does not clip a mark as omarky does).
        let (label, note) = (caret_at(&mut rt, at(5))[0], caret_at(&mut rt, at(5) + 6)[0]);
        assert!((label - field[0]).abs() < 0.5 && note >= edge - 0.5 && note < edge + 8.0, "footnote {label}..{note}, edge {edge}");
        // The task box is the item's text: it starts on the edge, not in the gutter.
        let nested = caret_at(&mut rt, at(3) + 4)[0];
        let spaces = caret_at(&mut rt, at(3) + 2)[0];
        assert!(nested > edge + 4.0, "a nested item's spaces indent it: {nested} vs {edge}");
        assert!(spaces < nested && caret_at(&mut rt, at(3))[0] < spaces, "the caret steps through the spaces and the marker");
        // Right and left step through each mark character.
        let xs: Vec<f32> = (0..=2).map(|i| caret_at(&mut rt, at(0) + i)[0]).collect();
        assert!(xs[0] < xs[1] && xs[1] < xs[2], "one step per mark character: {xs:?}");
        // The long item wraps; its next line starts under the item's text.
        let first = caret_at(&mut rt, at(7) + 2);
        let wrapped = (at(7) + 2..at(7) + text.split('\n').nth(7).unwrap().len())
            .map(|i| caret_at(&mut rt, i))
            .find(|c| c[1] > first[1] + 1.0)
            .expect("the item wraps");
        assert!((wrapped[0] - edge).abs() < 1.0, "a wrapped line aligns with the item's text: {wrapped:?} vs {edge}");
        // A click on the heading's mark in the gutter puts the caret in the mark.
        let mark = caret_at(&mut rt, at(4));
        rt.update(|s, _| s.cur_edit().set_selection(at(1), at(1)));
        rt.frame(DT);
        rt.pointer_down(mark[0] + 1.0, mark[1] + mark[3] / 2.0);
        rt.pointer_up(mark[0] + 1.0, mark[1] + mark[3] / 2.0);
        rt.frame(DT);
        let mut focus = 0;
        rt.update_if(|s, _| {
            focus = s.cur_edit().focus;
            false
        });
        assert!((at(4)..at(4) + 2).contains(&focus), "the click lands in the quote's mark: byte {focus}, line at {}", at(4));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-99: the outline slides in from the right over about 160 ms rather than appearing.
    #[test]
    fn wr_99_the_outline_slides() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-slide-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.right_open = false);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(info(&rt, "outline").is_none());
        let (x, y) = rt.find("toggle-right").expect("the outline button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(DT);
        rt.frame(DT);
        let sheet = rt.presented("sheet-row").unwrap().0;
        let end = sheet[0] + sheet[2];
        let early = rt.presented("outline").expect("the outline mounts").0;
        assert!(early[0] > end - 236.0 + 20.0, "two frames in, the outline is still sliding: {early:?}, row ends at {end}");
        for _ in 0..30 {
            rt.frame(DT);
        }
        let done = rt.presented("outline").unwrap().0;
        assert!((done[0] - (end - 236.0)).abs() < 0.5, "it rests at the right: {done:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod edit_menu_tests {
    use rune::{Env, Key, KeyEvent, Mods, Runtime};

    const DT: f64 = 1.0 / 60.0;

    /// WR-135: a right-click with no selection moves the caret to the click and opens Quire's
    /// menu with Cut and Copy off; one in the selection keeps it, Cut takes the text out, and
    /// focus is back in the editor after.
    #[test]
    fn wr_135_the_editor_menu_cuts_the_selection() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-edit-menu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(DT);
        let text = "alpha beta gamma\n";
        rt.update(move |s, tx| {
            s.mode = crate::app::Mode::Source;
            let a = s.active.clone().unwrap();
            s.set_text(&a, text.into());
            s.focus_editor(tx);
            s.cur_edit().set_selection(text.len(), text.len());
        });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let ([x, y, _, _], _) = rt.presented("editor").expect("the editor");
        let (px, py) = (x + 2.0, y + 8.0);
        let right_click = |rt: &mut Runtime<crate::app::App>| {
            rt.secondary_down(px, py);
            rt.secondary_up(px, py);
            for _ in 0..3 {
                rt.frame(DT);
            }
        };
        right_click(&mut rt);
        let mut seen = (false, false, 99);
        rt.update_if(|s, _| {
            let caret = s.cur_edit().focus;
            seen = (s.edit_menu.is_some(), s.edit_menu_selected(), caret);
            false
        });
        assert_eq!(seen, (true, false, 0), "menu open, nothing selected, caret moved to the click");
        rt.update(|s, tx| s.close_edit_menu(tx));
        rt.frame(DT);
        // "alpha" selected; a right-click at its start keeps it.
        rt.update(|s, _| s.cur_edit().set_selection(0, 5));
        rt.frame(DT);
        right_click(&mut rt);
        rt.update_if(|s, _| {
            seen = (s.edit_menu.is_some(), s.edit_menu_selected(), 0);
            false
        });
        assert_eq!(seen, (true, true, 0), "menu open over the selection");
        let (cx, cy) = rt.find("edit-menu/tm-0").expect("Cut");
        rt.pointer_move(cx, cy);
        rt.frame(DT);
        rt.pointer_down(cx, cy);
        rt.pointer_up(cx, cy);
        for _ in 0..3 {
            rt.frame(DT);
        }
        rt.key(KeyEvent { key: Key::Text("Z".into()), mods: Mods::default() });
        rt.frame(DT);
        let mut now = String::new();
        let mut open = true;
        rt.update_if(|s, _| {
            now = s.active_text().to_string();
            open = s.edit_menu.is_some();
            false
        });
        assert!(!open, "the menu closed");
        assert_eq!(now, "Z beta gamma\n", "Cut took \"alpha\" out, and the key after it went to the editor");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
