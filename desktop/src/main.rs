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
#[cfg(test)]
mod throw_tests;
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
        assert_eq!(find.help.as_deref(), Some("Find (Ctrl+F)"), "its tooltip names the preset's key");
        // Every title bar button with a key names it the same way: "Name (Keys)".
        let tips = |rt: &Runtime<crate::app::App>| -> Vec<(String, String)> {
            rt.elements()
                .into_iter()
                .filter(|e| ["/titlebar/", "/left-zone/"].iter().any(|z| ("/".to_owned() + &e.path + "/").contains(z)) && matches!(e.role, "button" | "switch") && !e.exiting)
                .filter_map(|e| Some((e.key?, e.help?)))
                .collect()
        };
        let keyed = ["toggle-left", "mode", "focus", "find", "copy-all", "wide", "toggle-right", "help", "paragraph-focus", "typewriter", "next-font"];
        let check = |tips: Vec<(String, String)>, want: usize| {
            let named: Vec<_> = tips.iter().filter(|(k, _)| keyed.contains(&k.as_str())).collect();
            assert_eq!(named.len(), want, "{tips:?}");
            for (key, tip) in named {
                assert!(tip.ends_with(')') && tip.contains(" (") && !tip.contains("  "), "{key}: {tip:?}");
            }
        };
        check(tips(&rt), 8);
        rt.update(|s, tx| s.toggle_mode(tx));
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert!(rt.find("copy-all").is_none(), "source mode keeps its own bar");
        check(tips(&rt), 7);
        // The outline button shows as on while the outline is open, in source mode too.
        rt.update(|s, _| s.persist.right_open = true);
        rt.pointer_move(700.0, 600.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(rt.find("outline").is_some(), "the outline is open");
        let button = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toggle-right") && !e.exiting).expect("the outline button");
        assert!(button.fill.a > 0.0, "the outline button shows as on in source mode");
        // The panel switch's captions start on the text edge of the rows they label.
        let els = rt.elements();
        let files = els.iter().find(|e| e.key.as_deref() == Some("panel-files")).expect("the Files caption");
        let row = els.iter().find(|e| e.text.as_deref() == Some("note") && e.path.contains("file:note.md")).expect("the note's row");
        assert_eq!(files.rect[0], row.rect[0], "the Files caption lines up with the rows");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The command palette fits the smallest window: its frame and foot stay inside, and the
    /// list scrolls in what is left.
    #[test]
    fn palette_fits_a_small_window() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-palette-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nText.\n")], crate::watch::EVERY);
        for (w, h) in [(520.0, 400.0), (640.0, 480.0), (1400.0, 1000.0)] {
            rt.set_viewport((w, h), 1.0);
            rt.update(|s, tx| s.open_palette(crate::app::PalMode::Commands, tx));
            for _ in 0..30 {
                rt.frame(DT);
            }
            let rect = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect).expect(key);
            for key in ["palette-frame", "pal-foot"] {
                let r = rect(key);
                assert!(r[1] >= 0.0 && r[1] + r[3] <= h, "{key} {r:?} in a {w}x{h} window");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-86, RD-87: in a small window the help card fits it and its rows scroll inside, a
    /// wheel stops at the sheet and leaves the note where it was, and `?` and Esc still close it.
    #[test]
    fn rd_86_help_fits_and_holds_the_note() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-help-{}", std::process::id()));
        let long = format!("# A\n\n{}", "A paragraph of text that runs on.\n\n".repeat(200));
        let mut rt = vault(&dir, &[("note.md", &long)], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((640.0, 480.0), 1.0);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let rect = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect);
        let help = |rt: &mut Runtime<crate::app::App>| {
            let mut on = false;
            rt.update_if(|s, _| {
                on = s.reader.help;
                false
            });
            on
        };
        rt.update(|s, _| s.reader.help = true);
        settle(&mut rt);
        let card = rect(&rt, "help-card").expect("the help card");
        assert!(card[1] >= 0.0 && card[1] + card[3] <= 480.0, "the card fits the window: {card:?}");
        let (page, body) = (rect(&rt, "doc-col").unwrap(), rect(&rt, "help-body").unwrap());
        rt.pointer_move(320.0, 240.0);
        rt.wheel(320.0, 240.0, -400.0);
        settle(&mut rt);
        assert_eq!(rect(&rt, "doc-col").unwrap()[1], page[1], "the note behind stays put");
        assert!(rect(&rt, "help-body").unwrap()[1] < body[1], "the card's rows scroll");
        rt.key(KeyEvent { key: Key::Text("?".into()), mods: Mods { shift: true, ..Default::default() } });
        settle(&mut rt);
        assert!(!help(&mut rt), "? closes help");
        rt.update(|s, _| s.reader.help = true);
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        settle(&mut rt);
        assert!(!help(&mut rt), "Esc closes help");
        // WR-158: in the writer, closing help puts the caret back in the note.
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.focus_editor(tx);
        });
        settle(&mut rt);
        rt.update(|s, _| s.reader.help = true);
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        settle(&mut rt);
        assert!(!help(&mut rt), "Esc closes the writer's help");
        assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("editor") && e.focused), "and the editor has the keyboard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-122: in the source the find bar takes its own row under the formatting toolbar, so
    /// they never overlap, from the smallest window up.
    #[test]
    fn wr_122_find_bar_clears_the_toolbar() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findbar-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nText with the word the in it.\n")], crate::watch::EVERY);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.writer_find(tx);
        });
        for (w, h) in [(520.0, 400.0), (720.0, 600.0), (1000.0, 700.0), (1400.0, 1000.0)] {
            rt.set_viewport((w, h), 1.0);
            for _ in 0..20 {
                rt.frame(DT);
            }
            let rect = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect).expect(key);
            let (bar, find) = (rect("toolbar"), rect("find-bar"));
            assert!(find[1] >= bar[1] + bar[3], "at {w}x{h} the find bar {find:?} sits under the toolbar {bar:?}");
            assert!(find[0] >= 0.0 && find[0] + find[2] <= w, "and inside the window");
        }
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

    /// Typing leaves the chrome where it is: the title bar, the file tree, the toolbar and the
    /// status line stay fully drawn through Enter and the pause after it (decision 0010).
    #[test]
    fn typing_leaves_the_chrome_in_place() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-chrome-stays-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.right_open = true);
        for _ in 0..10 {
            rt.frame(DT);
        }
        rt.update(|s, tx| s.writer_enter(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        for key in ["titlebar", "status", "sidebar", "toolbar"] {
            let o = info(&rt, key).unwrap_or_else(|| panic!("{key} is drawn")).opacity;
            assert!(o > 0.99, "{key} at opacity {o} after Enter");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-47, WR-100: the outline's current section has the accent dot left of its text, one
    /// dot only, and in the reader it moves with the scroll.
    #[test]
    fn the_current_section_has_the_dot() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-outline-dot-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Preview);
        rt.update(|s, _| s.persist.right_open = true);
        for _ in 0..30 {
            rt.frame(DT);
        }
        // The lit dots, by their place, and the text of the row each sits in.
        let lit = |rt: &rune::Runtime<crate::app::App>| {
            let all = rt.elements();
            all.iter()
                .filter(|e| e.key.as_deref() == Some("dot") && e.fill.a > 0.5)
                .map(|e| {
                    let row = e.parent.map(|p| &all[p]).expect("a dot sits in its row");
                    assert!(row.key.as_deref().is_some_and(|k| k.starts_with("ol-")), "in an outline row");
                    let name = all.iter().find(|t| t.parent == e.parent && t.text.is_some()).expect("the row's text");
                    assert!(e.rect[0] + e.rect[2] < name.rect[0] - 10.0, "the dot {:?} sits left of the text {:?}", e.rect, name.rect);
                    (e.rect[1], name.text.clone().unwrap())
                })
                .collect::<Vec<_>>()
        };
        let first = lit(&rt);
        assert_eq!(first.len(), 1, "one dot: {first:?}");
        let last = crate::md::headings(LONG).iter().filter(|h| h.level <= 4).last().unwrap().line;
        rt.update(move |s, tx| s.jump(last, tx));
        for _ in 0..90 {
            rt.frame(DT);
        }
        let later = lit(&rt);
        assert_eq!(later.len(), 1, "still one dot: {later:?}");
        assert!(later[0].0 > first[0].0, "the dot moved down the outline with the page: {first:?} to {later:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-189: the word count and reading time sit centred at the bottom of the outline, in
    /// both modes. In the writer a footer of fixed height opens under them with the caret's
    /// line and column; in preview there is no footer and nothing says "preview".
    #[test]
    fn wr_189_counts_above_the_writers_footer() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-counts-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.right_open = true);
        for _ in 0..30 {
            rt.frame(DT);
        }
        let inside = |r: [f32; 4], o: [f32; 4]| r[0] >= o[0] - 0.5 && r[1] >= o[1] - 0.5 && r[0] + r[2] <= o[0] + o[2] + 0.5 && r[1] + r[3] <= o[1] + o[3] + 0.5;
        // The counts centre in the outline, on one line, and end where `below` begins.
        let counts_end_at = |rt: &rune::Runtime<crate::app::App>, below: f32| {
            let outline = rt.presented("outline").unwrap().0;
            let counts = rt.presented("counts").expect("the counts show").0;
            assert!(inside(counts, outline), "the counts {counts:?} sit in the outline {outline:?}");
            assert!((counts[1] + counts[3] - below).abs() < 1.0, "the counts {counts:?} end at {below}");
            let (wc, rd) = (info(rt, "word-count").unwrap().rect, info(rt, "read-time").unwrap().rect);
            assert!((wc[1] - rd[1]).abs() < 0.5, "the count and the reading time share a line");
            let mid = outline[0] + outline[2] / 2.0;
            assert!(((wc[0] + rd[0] + rd[2]) / 2.0 - mid).abs() < 1.0, "centred: {wc:?} {rd:?}, middle {mid}");
            let bar = rt.presented("titlebar").unwrap().0;
            assert!(!inside(wc, bar));
        };
        let outline = rt.presented("outline").unwrap().0;
        let foot = rt.presented("status").expect("the writer's footer shows").0;
        assert!(inside(foot, outline) && (foot[1] + foot[3] - (outline[1] + outline[3])).abs() < 1.0, "the footer {foot:?} sits at the outline's foot");
        assert!((foot[3] - 29.0).abs() < 0.5, "the footer keeps its height: {}", foot[3]);
        assert!(inside(info(&rt, "caret-pos").expect("the caret's position shows").rect, foot));
        counts_end_at(&rt, foot[1]);
        rt.update(|s, tx| s.toggle_mode(tx));
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(rt.presented("status").is_none(), "no footer in preview");
        assert!(info(&rt, "caret-pos").is_none());
        let outline = rt.presented("outline").unwrap().0;
        counts_end_at(&rt, outline[1] + outline[3]);
        assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("preview")), "nothing says preview");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-223: zen mode's word count and how to leave sit centred in the top bar, drawn without
    /// a hover, above the page rather than over its text.
    #[test]
    fn wr_223_the_zen_hint_sits_in_the_top_bar() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-zen-hint-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, tx| s.toggle_focus(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let bar = rt.presented("titlebar").unwrap().0;
        let hint = info(&rt, "focus-hint").expect("the zen hint shows");
        let inside = |r: [f32; 4], o: [f32; 4]| r[0] >= o[0] - 0.5 && r[1] >= o[1] - 0.5 && r[0] + r[2] <= o[0] + o[2] + 0.5 && r[1] + r[3] <= o[1] + o[3] + 0.5;
        assert!(inside(hint.rect, bar), "the hint {:?} sits in the top bar {bar:?}", hint.rect);
        assert!(hint.opacity > 0.99, "drawn without a hover: opacity {}", hint.opacity);
        let words = rt.elements().into_iter().find(|e| e.text.as_deref().is_some_and(|t| t.contains("esc to leave zen mode"))).expect("the hint's text");
        assert!(inside(words.rect, bar) && words.opacity > 0.99);
        let mid = bar[0] + bar[2] / 2.0;
        assert!((words.rect[0] + words.rect[2] / 2.0 - mid).abs() < 1.0, "centred: {:?} in {bar:?}", words.rect);
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        assert!(page[1] >= bar[1] + bar[3] - 0.5, "the page {page:?} starts under the bar {bar:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-177 in zen mode's preview: the page's text fades at the top and into the foot, as the
    /// writer's does. Outside zen mode the preview has no fades.
    #[test]
    fn zen_preview_fades_the_page_edges() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-zen-fade-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Preview);
        for _ in 0..60 {
            rt.frame(DT);
        }
        let mut reading = false;
        rt.update_if(|s, _| {
            reading = crate::ui::reading(s);
            false
        });
        assert!(reading, "the preview shows");
        assert!(info(&rt, "fade-top").is_none() && info(&rt, "fade-foot").is_none(), "no fades in the preview outside zen mode");
        rt.update(|s, tx| s.toggle_focus(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let top = info(&rt, "fade-top").expect("the top fade").rect;
        let foot = info(&rt, "fade-foot").expect("the foot fade").rect;
        assert!((top[1] - page[1]).abs() < 0.5 && top[3] == 28.0, "the top fade {top:?} at the page's top {page:?}");
        assert!((foot[1] + foot[3] - (page[1] + page[3])).abs() < 0.5 && foot[3] == 64.0, "the foot fade {foot:?} at the page's foot {page:?}");
        assert!(top[2] >= page[2] - 0.5 && foot[2] >= page[2] - 0.5, "both span the page");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Zen mode's floating bar: the title bar's buttons on a pill in the top-right corner,
    /// hidden until the pointer comes to the top band. The editor's set has the writer's view
    /// toggles, the preview's has the reader's chips, and neither has the outline toggle.
    #[test]
    fn zen_mode_floats_the_bar_buttons_on_hover() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-zen-bar-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, tx| s.toggle_focus(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let bar = rt.presented("titlebar").unwrap().0;
        let pill = info(&rt, "zen-actions").expect("the floating bar");
        let r = pill.rect;
        assert!(r[1] >= bar[1] - 0.5 && r[1] + r[3] <= bar[1] + bar[3] + 0.5, "in the top band: {r:?} in {bar:?}");
        assert!(r[0] > bar[0] + bar[2] / 2.0 && bar[0] + bar[2] - (r[0] + r[2]) < 24.0, "at the right: {r:?} in {bar:?}");
        assert!(pill.opacity < 0.05, "hidden without a hover: {}", pill.opacity);
        let inside = |key: &str| info(&rt, key).is_some_and(|e| e.rect[0] >= r[0] && e.rect[0] + e.rect[2] <= r[0] + r[2] + 0.5);
        for key in ["mode", "focus", "paragraph-focus", "typewriter", "next-font"] {
            assert!(inside(key), "{key} on the editor's pill");
        }
        for key in ["toggle-right", "find", "help"] {
            assert!(info(&rt, key).is_none(), "{key} is not on the editor's pill");
        }
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        rt.pointer_move(cx, cy);
        for _ in 0..40 {
            rt.frame(DT);
        }
        let shown = info(&rt, "zen-actions").unwrap().opacity;
        assert!(shown > 0.99, "shown under the pointer: {shown}");
        let (px, py) = rt.find("paragraph-focus").expect("the paragraph focus button");
        rt.pointer_move(px, py);
        rt.pointer_down(px, py);
        rt.pointer_up(px, py);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut on = (false, false);
        rt.update_if(|s, _| {
            on = (s.settings().paragraph_focus, s.focus);
            false
        });
        assert_eq!(on, (true, true), "the button turns paragraph focus on and zen mode stays");
        rt.update(|s, tx| s.toggle_mode(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let r = info(&rt, "zen-actions").expect("the floating bar in preview").rect;
        for key in ["mode", "focus", "find", "copy-all", "wide", "help"] {
            assert!(info(&rt, key).is_some_and(|e| e.rect[0] >= r[0] && e.rect[0] + e.rect[2] <= r[0] + r[2] + 0.5), "{key} on the preview's pill");
        }
        for key in ["paragraph-focus", "toggle-right"] {
            assert!(info(&rt, key).is_none(), "{key} is not on the preview's pill");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A press in the editor dragged below the page and held there scrolls the page, and the
    /// selection grows with it, as in a text view.
    #[test]
    fn a_drag_selection_held_below_the_page_scrolls_it() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-drag-scroll-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        let field = rt.presented("editor").unwrap().0;
        let (x, y) = (field[0] + 40.0, field[1] + 30.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.frame(DT);
        rt.pointer_move(x, y + 40.0);
        rt.frame(DT);
        let focus = |rt: &mut rune::Runtime<crate::app::App>| {
            let mut f = 0;
            rt.update_if(|s, _| {
                f = s.cur_edit().focus;
                false
            });
            f
        };
        let before = (rt.presented("editor").unwrap().0[1], focus(&mut rt));
        rt.pointer_move(x, page[1] + page[3] + 60.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        let after = (rt.presented("editor").unwrap().0[1], focus(&mut rt));
        rt.pointer_up(x, page[1] + page[3] + 60.0);
        assert!(after.0 < before.0 - 100.0, "the page scrolls under the held drag: editor top {} then {}", before.0, after.0);
        assert!(after.1 > before.1, "the selection grows with it: focus {} then {}", before.1, after.1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-178: the progress bar runs under the title bar, and along the window's top edge in
    /// zen mode, in both modes.
    #[test]
    fn wr_178_zen_progress_runs_along_the_window_top() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        for mode in [crate::app::Mode::Source, crate::app::Mode::Preview] {
            let dir = std::env::temp_dir().join(format!("quire-zen-bar-{mode:?}-{}", std::process::id()));
            let (mut rt, _) = open(&dir, LONG, mode);
            for _ in 0..10 {
                rt.frame(DT);
            }
            rt.update(|s, tx| s.scroll_by(600.0, tx));
            for _ in 0..60 {
                rt.frame(DT);
            }
            let bar = info(&rt, "progress").expect("the progress bar is drawn").rect;
            assert!(bar[1] >= crate::ui::titlebar::BAR_H && bar[2] > 1.0, "{mode:?}: under the title bar, got {bar:?}");
            rt.update(|s, tx| s.toggle_focus(tx));
            for _ in 0..60 {
                rt.frame(DT);
            }
            let bar = info(&rt, "progress").expect("the progress bar is drawn in zen mode").rect;
            assert!(bar[1] == 0.0 && bar[2] > 1.0, "{mode:?}: zen bar at the window top, got {bar:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// The sidebar runs the window's full height, and its toggle keeps its place in the window's
    /// corner whether the sidebar is open or closed.
    #[test]
    fn the_sidebar_toggle_stays_in_the_corner() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toggle-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.left_open = true);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let side = info(&rt, "sidebar").expect("the sidebar shows").rect;
        assert!(side[1] == 0.0 && side[3] == rt.presented("root").unwrap().0[3], "the sidebar runs top to bottom: {side:?}");
        let open = info(&rt, "toggle-left").expect("the toggle shows").rect;
        rt.update(|s, _| s.toggle_left());
        for _ in 0..120 {
            rt.frame(DT);
        }
        let closed = info(&rt, "toggle-left").expect("the toggle shows with the sidebar closed").rect;
        assert!(info(&rt, "sidebar").is_none(), "the sidebar closed");
        assert_eq!(open, closed, "the toggle stays put");
        let _ = std::fs::remove_dir_all(&dir);
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
mod escape_tests {
    use crate::app::Mode;
    use rune::{Env, Key, KeyEvent, Mods, Runtime};

    const DT: f64 = 1.0 / 60.0;

    /// WR-77: Esc in the editor closes what is open first, one thing per press, and with
    /// nothing open goes to the preview. Esc there comes back (WR-111).
    #[test]
    fn wr_77_esc_in_the_editor_goes_to_the_preview() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-escape-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.on_key = Some(crate::keys::on_key);
        rt.frame(DT);
        rt.update(|s, tx| {
            s.mode = Mode::Source;
            s.focus = true;
            s.focus_editor(tx);
        });
        let esc = |rt: &mut Runtime<crate::app::App>| {
            rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
            for _ in 0..3 {
                rt.frame(DT);
            }
            let mut seen = (Mode::Source, false, false);
            rt.update_if(|s, _| {
                seen = (s.mode, s.focus, s.edit_menu.is_some());
                false
            });
            seen
        };
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert_eq!(esc(&mut rt), (Mode::Source, false, false), "the first Esc leaves zen mode only");
        let ([x, y, _, _], _) = rt.presented("editor").expect("the editor");
        rt.secondary_down(x + 2.0, y + 8.0);
        rt.secondary_up(x + 2.0, y + 8.0);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut open = false;
        rt.update_if(|s, _| {
            open = s.edit_menu.is_some();
            false
        });
        assert!(open, "the right-click opened the editor's menu");
        assert_eq!(esc(&mut rt), (Mode::Source, false, false), "Esc closes the editor's menu and stays");
        assert_eq!(esc(&mut rt), (Mode::Preview, false, false), "with nothing open, Esc goes to the preview");
        assert_eq!(esc(&mut rt), (Mode::Source, false, false), "and Esc in the preview comes back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod bar_focus_tests {
    use crate::app::Mode;
    use rune::{Env, Key, KeyEvent, Mods, Runtime};

    const DT: f64 = 1.0 / 60.0;

    /// A press on a title bar button leaves the keyboard where it was: typing after it still
    /// goes into the note, in the bar and on zen mode's floating pill.
    #[test]
    fn bar_buttons_leave_the_editor_focused() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-bar-focus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.on_key = Some(crate::keys::on_key);
        rt.frame(DT);
        rt.update(|s, tx| {
            s.mode = Mode::Source;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "alpha\n".into());
            s.focus_editor(tx);
            s.cur_edit().set_selection(5, 5);
        });
        let mut typed = |rt: &mut Runtime<crate::app::App>, button: &str, ch: &str| {
            for _ in 0..30 {
                rt.frame(DT);
            }
            let (x, y) = rt.find(button).unwrap_or_else(|| panic!("the {button} button"));
            rt.pointer_move(x, y);
            for _ in 0..30 {
                rt.frame(DT);
            }
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            for _ in 0..3 {
                rt.frame(DT);
            }
            rt.key(KeyEvent { key: Key::Text(ch.into()), mods: Mods::default() });
            rt.frame(DT);
            let mut text = String::new();
            rt.update_if(|s, _| {
                text = s.active_text().to_string();
                false
            });
            text
        };
        assert_eq!(typed(&mut rt, "typewriter", "X"), "alphaX\n", "typing goes on after the typewriter button");
        assert_eq!(typed(&mut rt, "next-font", "Y"), "alphaXY\n", "and after the next font button");
        rt.update(|s, tx| s.toggle_focus(tx));
        assert_eq!(typed(&mut rt, "paragraph-focus", "Z"), "alphaXYZ\n", "and after a button on zen mode's pill");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// In preview Down and Space still scroll the page after a press on a title bar button,
    /// and Space does not press the button again.
    #[test]
    fn bar_buttons_leave_the_preview_scrolling() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-bar-focus-preview-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, crate::bench::LONG, Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        let (px, py) = rt.find(crate::reader::SCROLL).expect("the page");
        rt.pointer_down(px, py);
        rt.pointer_up(px, py);
        let down = |rt: &mut Runtime<crate::app::App>, key: Key| {
            let y = |rt: &mut Runtime<crate::app::App>| {
                let mut y = 0.0;
                rt.update_if(|s, _| {
                    y = s.reader.scroll.get()[0];
                    false
                });
                y
            };
            let before = y(rt);
            for _ in 0..4 {
                rt.key(KeyEvent { key: key.clone(), mods: Mods::default() });
                rt.frame(DT);
            }
            for _ in 0..40 {
                rt.frame(DT);
            }
            y(rt) - before
        };
        assert!(down(&mut rt, Key::Down) > 1.0, "Down scrolls the page");
        let (x, y) = rt.find("wide").expect("the width button");
        rt.pointer_move(x, y);
        rt.frame(DT);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let wide = |rt: &mut Runtime<crate::app::App>| {
            let mut w = false;
            rt.update_if(|s, _| {
                w = s.wide();
                false
            });
            w
        };
        assert!(wide(&mut rt), "the button made the page wide");
        assert!(down(&mut rt, Key::Down) > 1.0, "Down still scrolls after the width button");
        assert!(down(&mut rt, Key::Text(" ".into())) > 1.0, "and so does Space");
        assert!(wide(&mut rt), "Space did not press the width button again");
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

#[cfg(test)]
mod note_mode_tests {
    use crate::app::Mode;
    use crate::bench::DT;
    use rune::{Env, Runtime};
    use std::path::PathBuf;

    fn shown(rt: &Runtime<crate::app::App>, key: &str) -> bool {
        rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
    }

    /// Note mode (decision 0011): a note from outside the list opens as a file, in zen and in
    /// preview, as a vault of its own. Leaving zen shows the page with the sidebar collapsed;
    /// the sidebar holds only the Open list. Ctrl+O and links open more notes, each from its
    /// own folder, so two notes of one name stay apart. Folder mode's list, the folder that
    /// opens next, its sidebar, its mode and sync stay as they were.
    #[test]
    fn notes_open_as_files_in_note_mode() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-note-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (notes, loose, other) = (dir.join("notes"), dir.join("loose"), dir.join("other"));
        for d in [&notes, &loose.join("sub"), &other] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(notes.join("x.md"), "# x").unwrap();
        std::fs::write(loose.join("a.md"), "# loose a\n\nSee [b](b.md).\n").unwrap();
        std::fs::write(loose.join("b.md"), "# loose b").unwrap();
        std::fs::write(loose.join("sub/c.md"), "# c").unwrap();
        std::fs::write(other.join("a.md"), "# other a").unwrap();
        let (loose, other) = (loose.canonicalize().unwrap(), other.canonicalize().unwrap());
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        let (n, file) = (notes.clone(), loose.join("a.md"));
        rt.update(move |s, tx| {
            s.open_vault(n, tx);
            s.persist.left_open = true;
            s.mode = Mode::Source;
            s.persist.settings.sync.provider = "s3".into();
            s.persist.settings.sync.connected = true;
            assert!(s.start_note(&file));
        });
        for _ in 0..10 {
            rt.frame(DT);
        }
        let listed = |s: &crate::app::App| s.persist.vaults.iter().map(|v| v.path.clone()).collect::<Vec<_>>();
        let (notes2, loose1, a) = (notes.clone(), loose.clone(), loose.join("a.md"));
        rt.update_if(move |s, _| {
            assert!(s.note_mode() && !s.folder_open());
            assert_eq!(s.files().keys().collect::<Vec<_>>(), ["a.md"], "only the note is read");
            assert_eq!(s.notes, [a.clone()]);
            assert!(s.focus && s.mode == Mode::Preview, "it starts in zen, in preview");
            assert!(listed(s).contains(&notes2) && !listed(s).contains(&loose1), "its folder stays out of the list");
            assert_eq!(s.persist.last.as_ref(), Some(&notes2), "the folder that opens next is unchanged");
            assert!(!s.sync_ready() && s.sync_label().is_none(), "Note mode isn't synced");
            false
        });

        // Leaving zen shows the page, the sidebar collapsed; opened, it holds only Open.
        rt.update(|s, tx| s.toggle_focus(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        assert!(!shown(&rt, "sidebar"), "the sidebar is collapsed");
        rt.update(|s, _| s.toggle_left());
        for _ in 0..60 {
            rt.frame(DT);
        }
        let row = format!("note:{}", loose.join("a.md").display());
        assert!(shown(&rt, "open-notes") && shown(&rt, &row), "the Open list holds the note");
        for key in ["vault-head", "panel-tabs", "side-scroll", "foot-new"] {
            assert!(!shown(&rt, key), "no {key} in Note mode");
        }

        // Edits save to the file; nothing is added to, renamed in, or deleted from its folder.
        rt.update(|s, tx| {
            s.set_text("a.md", "edited, see [b](b.md)".into());
            s.new_note(None, tx);
            s.start_new_folder("", tx);
            s.delete_note();
            s.start_rename(tx);
            assert!(s.renaming.is_none(), "no rename");
            assert!(s.notice.is_some(), "says why");
        });

        // Ctrl+O asks the system for a file; a note of the same name from another folder
        // opens beside the first, in preview, from its own folder.
        rt.update(|s, tx| {
            s.mode = Mode::Source;
            s.quick_open(tx);
            assert!(s.palette.is_none(), "no quick switcher");
        });
        assert!(rt.file_dialog().is_some(), "the file picker is up");
        rt.resolve_file_dialog(Some(other.join("a.md"))).unwrap();
        rt.frame(DT);
        let (a1, a2) = (loose.join("a.md"), other.join("a.md"));
        rt.update_if(move |s, _| {
            assert_eq!(s.notes, [a1.clone(), a2.clone()]);
            assert_eq!(s.note_path().as_ref(), Some(&a2));
            assert_eq!(s.files().get("a.md").map(String::as_str), Some("# other a"), "its own text");
            assert_eq!(s.mode, Mode::Preview, "a newly opened note shows in preview");
            false
        });
        assert_eq!(std::fs::read_to_string(loose.join("a.md")).unwrap(), "edited, see [b](b.md)", "the first note saved on the switch");

        // Back to the first, and a link from it opens its neighbour as one more note.
        let a1 = loose.join("a.md");
        rt.update(move |s, tx| {
            s.open_note(&a1);
            assert_eq!(s.files().get("a.md").map(String::as_str), Some("edited, see [b](b.md)"));
            s.follow("b.md", tx);
        });
        let (b, a2) = (loose.join("b.md"), other.join("a.md"));
        rt.update_if(move |s, _| {
            assert_eq!(s.note_path().as_ref(), Some(&b));
            assert_eq!(s.notes.len(), 3);
            // Closing the note shown shows its neighbour; closing them all leaves none.
            s.close_note(&b);
            assert_eq!(s.note_path().as_ref(), Some(&a2));
            for n in s.notes.clone() {
                s.close_note(&n);
            }
            assert!(s.notes.is_empty() && s.active.is_none());
            s.flush_all();
            false
        });
        let mut on_disk: Vec<String> = std::fs::read_dir(&loose).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        on_disk.sort();
        assert_eq!(on_disk, ["a.md", "b.md", "sub"], "the folder keeps exactly its files");
        let saved = crate::store::load();
        assert_eq!(saved.last.as_ref(), Some(&notes), "quitting keeps the folder that opens next");
        assert!(saved.left_open && saved.mode == "source", "Folder mode keeps its sidebar and mode: {} {}", saved.left_open, saved.mode);
        assert!(!saved.vaults.iter().any(|v| v.path == loose || v.path == other), "no list entry for a note's folder");
        assert!(!saved.sessions.keys().any(|k| k.contains("loose") || k.contains("other")), "no session for one either");

        // Opening a folder leaves Note mode and brings Folder mode's sidebar and mode back.
        let n: PathBuf = notes.clone();
        rt.update(move |s, tx| {
            s.open_vault(n, tx);
            assert!(s.folder_open() && s.notes.is_empty());
            assert!(s.persist.left_open && s.mode == Mode::Source);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A note that leaves the screen in Note mode keeps its caret, undo history and unlocked
    /// text until it shows again. If the file changed meanwhile, it comes back from disk.
    #[test]
    fn a_note_keeps_its_undo_and_unlock_across_switches() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-note-park-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (one, two) = (dir.join("one"), dir.join("two"));
        std::fs::create_dir_all(&one).unwrap();
        std::fs::create_dir_all(&two).unwrap();
        std::fs::write(one.join("a.md"), "first").unwrap();
        std::fs::write(two.join("a.md"), "other").unwrap();
        let (a, b) = (one.canonicalize().unwrap().join("a.md"), two.canonicalize().unwrap().join("a.md"));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(DT);
        let set = |s: &mut crate::app::App, key: &str, v: &str| s.field(key, String::new).replace_text(v.to_string());
        let (a1, b1) = (a.clone(), b.clone());
        rt.update(move |s, tx| {
            assert!(s.start_note(&a1));
            s.mode = Mode::Source;
            let e = s.cur_edit();
            e.set_selection(5, 5);
            e.paste(" edit");
            s.after_edit();
            assert_eq!(s.active_text(), "first edit");
            s.ask_lock(tx);
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
            assert!(s.note_unlocked("a.md"));
            // Away to a note of the same name, and back.
            s.open_note(&b1);
            assert!(!s.note_unlocked("a.md"), "the other note isn't unlocked by the first one's key");
            assert_eq!(s.active_text(), "other");
            s.open_note(&a1);
            assert!(s.note_unlocked("a.md"), "still unlocked");
            assert_eq!(s.active_text(), "first edit");
            s.cur_edit().undo();
            assert_eq!(s.cur_edit().text, "first", "the undo history came back");
            s.after_edit();
        });

        // Changed outside while away: it comes back from disk, its history gone.
        let (a2, b2) = (a.clone(), b.clone());
        rt.update(move |s, _| {
            s.open_note(&b2);
            s.flush_all();
            std::fs::write(&a2, "changed outside").unwrap();
            s.open_note(&a2);
            assert!(!s.note_unlocked("a.md"));
            assert_eq!(s.active_text(), "changed outside");
            s.cur_edit().undo();
            assert_eq!(s.cur_edit().text, "changed outside", "no history from before");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Edits save on their own 0.5 s later, with no Ctrl+S, in both modes (WR-141).
    #[test]
    fn edits_autosave_in_both_modes() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-autosave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (folder, loose) = (dir.join("folder"), dir.join("loose"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(folder.join("f.md"), "folder").unwrap();
        std::fs::write(loose.join("n.md"), "note").unwrap();
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        let saved_by_itself = |rt: &mut Runtime<crate::app::App>, file: &std::path::Path, want: &str| {
            for _ in 0..60 {
                rt.frame(DT);
            }
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::fs::read_to_string(file).unwrap() != want && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(std::fs::read_to_string(file).unwrap(), want, "{} saved by itself", file.display());
        };
        let f = folder.clone();
        rt.update(move |s, tx| {
            s.open_vault(f, tx);
            s.open_file("f.md", None, tx);
            s.set_text("f.md", "folder edited".into());
        });
        saved_by_itself(&mut rt, &folder.join("f.md"), "folder edited");
        let n = loose.join("n.md");
        rt.update(move |s, _| {
            assert!(s.start_note(&n) && s.note_mode());
            s.set_text("n.md", "note edited".into());
        });
        saved_by_itself(&mut rt, &loose.join("n.md"), "note edited");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
