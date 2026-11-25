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
mod focus;
mod folders;
mod format;
mod help;
mod images;
mod keymap;
mod keys;
mod layer;
mod lock;
mod lock_app;
mod map;
mod map_app;
mod map_drag;
mod map_find;
mod map_fold;
mod map_geom;
mod map_layout;
mod map_motion;
mod map_throw;
mod mcp;
mod highlight;
mod motion;
mod page;
mod reader;
mod reveal;
mod writer;
mod writer_app;
mod md;
mod roles;
mod sample;
mod search;
mod sheet;
mod sidebar;
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
    s.show_layer(layer::Layer::Settings);
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
/// Note mode (decision 0011): the open note, opened again as a file of its own, with the
/// sidebar Note mode starts without.
fn scene_note(s: &mut App) {
    let file = s.vault.as_ref().zip(s.active.as_ref()).map(|(v, a)| v.path.join(a));
    if let Some(f) = file {
        s.open_note(&f);
        s.persist.left_open = true;
    }
}
fn scene_keys(s: &mut App) {
    s.show_layer(layer::Layer::Settings);
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
        scenes: &[("welcome", scene_welcome), ("preview", scene_preview), ("dark", scene_dark), ("focus", scene_focus), ("settings", scene_settings), ("keys", scene_keys), ("parity", scene_parity), ("note", scene_note)],
        ..AppSpec::new("Quire", (1280.0, 800.0), app::init, ui::view)
    };
    rune::App::new(spec)
        .font_smoothing(smoothing())
        .chrome(rune::Chrome { full_size_content: true, traffic_lights: Some((14.0, 13.0)), drag_regions: true })
        .titlebar(rune::Titlebar::new(ui::titlebar::BAR_H))
        .on_close(|s: &mut App, _tx| s.flush_all())
        .on_touchpad_hold(App::touchpad_hold)
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
        s.show_layer(crate::layer::Layer::Settings);
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

    /// The settings box is as tall as its page, up to the window less a margin, and keeps its
    /// top when the section changes. A page taller than the box fades out at its bottom edge
    /// until it is scrolled to its end.
    #[test]
    fn the_settings_box_fits_its_page() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-setbox-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        let show = |rt: &mut Runtime<crate::app::App>, sec: crate::app::Sec| {
            rt.update(|s, tx| {
                s.open_settings(tx);
                s.set_sec = sec;
            });
            for _ in 0..30 {
                rt.frame(DT);
            }
            rt.elements().into_iter().find(|e| e.key.as_deref() == Some("settings-frame")).unwrap().rect
        };
        let mcp = show(&mut rt, crate::app::Sec::Mcp);
        let keys = show(&mut rt, crate::app::Sec::Keys);
        assert!(mcp[3] < 400.0, "a short page gets a short box: {mcp:?}");
        assert!((keys[3] - 720.0).abs() < 0.5, "a long page fills the box's room: {keys:?}");
        assert_eq!(mcp[1], keys[1], "the box keeps its top");
        let held = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap().stuck;
        let stuck = |rt: &Runtime<crate::app::App>| held(rt, "set-fade-hold");
        assert!(stuck(&rt).abs() > 12.0, "the fade shows while more page is below");
        assert_eq!(held(&rt, "set-fade-top-hold"), 0.0, "no fade under the title at the page's top");
        let (x, y) = rt.find("set-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -100_000.0);
        for _ in 0..60 {
            rt.frame(DT);
        }
        assert_eq!(stuck(&rt), 0.0, "and goes at the page's end");
        assert!(held(&rt, "set-fade-top-hold").abs() > 12.0, "while the scrolled page fades under the title");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// HS: help and the palette's list fade at their edges as Settings' page does, through the
    /// one helper: at the bottom while more is below, at the top once scrolled.
    #[test]
    fn every_scrolling_sheet_fades_alike() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-sheet-fades-{}", std::process::id()));
        let notes: Vec<(String, &str)> = (0..40).map(|i| (format!("note {i:02}.md"), "# A\n")).collect();
        let notes: Vec<(&str, &str)> = notes.iter().map(|(n, t)| (n.as_str(), *t)).collect();
        let mut rt = vault(&dir, &notes, crate::watch::EVERY);
        rt.set_viewport((640.0, 480.0), 1.0);
        let held = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("{key} is drawn")).stuck;
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let check = |rt: &mut Runtime<crate::app::App>, name: &str| {
            settle(rt);
            assert!(held(rt, &format!("{name}-fade-hold")).abs() > 12.0, "{name}: the bottom fade shows while more is below");
            assert_eq!(held(rt, &format!("{name}-fade-top-hold")), 0.0, "{name}: no top fade at the top");
            let (x, y) = rt.find(&format!("{name}-scroll")).unwrap();
            rt.pointer_move(x, y);
            rt.wheel(x, y, -100_000.0);
            settle(rt);
            assert_eq!(held(rt, &format!("{name}-fade-hold")), 0.0, "{name}: no bottom fade at the end");
            assert!(held(rt, &format!("{name}-fade-top-hold")).abs() > 12.0, "{name}: the top fade shows once scrolled");
        };
        rt.update(|s, tx| s.toggle_help(tx));
        check(&mut rt, "help");
        rt.update(|s, tx| s.close_layer(tx));
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Files, tx));
        check(&mut rt, "pal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-12: "+" and Ctrl+N make the note in the active tab's folder, or at the root with no
    /// tab, and a name with a slash goes from the root.
    #[test]
    fn new_notes_follow_the_open_note() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-new-follows-{}", std::process::id()));
        let mut rt = vault(&dir, &[("Box/a.md", "# A\n"), ("b.md", "# B\n")], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        let active = |rt: &mut Runtime<crate::app::App>| {
            let mut a = None;
            rt.update_if(|s, _| {
                a = s.active.clone();
                false
            });
            a.unwrap_or_default()
        };
        rt.update(|s, tx| s.open_file("Box/a.md", None, tx));
        rt.frame(DT);
        let (x, y) = rt.find("new-note").expect("the + button");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(DT);
        assert_eq!(active(&mut rt), "Box/Untitled.md", "+ makes the note in the open note's folder");
        rt.update(|s, tx| {
            s.cancel_rename(tx);
            s.open_file("b.md", None, tx);
            s.focus_editor(tx);
        });
        rt.frame(DT);
        rt.key(rune::KeyEvent { key: rune::Key::Command('n'), mods: rune::Mods { cmd: true, ..rune::Mods::default() } });
        rt.frame(DT);
        assert_eq!(active(&mut rt), "Untitled.md", "Ctrl+N from a note at the root makes it at the root");
        rt.update(|s, tx| {
            s.cancel_rename(tx);
            s.open_file("Box/a.md", None, tx);
            s.new_note(Some("Drafts/Idea"), tx);
        });
        assert_eq!(active(&mut rt), "Drafts/Idea.md", "a path goes from the root");
        rt.update(|s, tx| {
            for t in s.tabs.clone() {
                s.close_tab(&t);
            }
            s.new_note(None, tx);
        });
        assert_eq!(active(&mut rt), "Untitled 2.md", "with no tab the note goes at the root");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-05, SB-14, SB-48, SB-125: both section labels have one style, a chevron, the name in
    /// small capitals and a count: Open with its tabs, the vault's name with its notes. With no
    /// note open the Open label goes with its list.
    #[test]
    fn section_labels_share_one_style() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-labels-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n"), ("b.md", "# B\n")], crate::watch::EVERY);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.open_file("note.md", None, tx));
        for _ in 0..20 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let under = |key: &str| -> Vec<rune::ElementInfo> { els.iter().filter(|e| !e.exiting && e.path.split('/').any(|seg| seg == key)).cloned().collect() };
        let mut looks = Vec::new();
        for (key, title, count) in [("label-shelf", "OPEN", "1"), ("label-vault", "", "2")] {
            let parts = under(key);
            let texts: Vec<&rune::ElementInfo> = parts.iter().filter(|e| e.text.is_some()).collect();
            assert_eq!(texts.len(), 2, "{key}: a name and a count: {:?}", texts.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
            let (t, n) = (texts[0], texts[1]);
            if !title.is_empty() {
                assert_eq!(t.text.as_deref(), Some(title), "{key}'s name");
            }
            assert_eq!(t.text.clone().map(|x| x.to_uppercase()), t.text.clone(), "{key}: the name is in capitals");
            assert_eq!(n.text.as_deref(), Some(count), "{key}'s count");
            assert!(parts.iter().any(|e| e.role == "icon"), "{key} has a chevron");
            looks.push((t.rect[3], t.text_color, n.text_color, parts[0].rect[3]));
        }
        assert_eq!(looks[0], looks[1], "both labels look the same");
        rt.update(|s, _| s.close_tab("note.md"));
        for _ in 0..20 {
            rt.frame(DT);
        }
        assert!(!rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some("OPEN")), "no Open label over an empty shelf");
        assert!(rt.find("handle").is_none(), "nor a handle");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every icon in the title bar is drawn at one size, in preview and in source.
    #[test]
    fn title_bar_icons_share_one_size() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-baricons-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        for mode in [crate::app::Mode::Preview, crate::app::Mode::Source] {
            rt.update(|s, _| s.mode = mode);
            for _ in 0..10 {
                rt.frame(DT);
            }
            let sizes: std::collections::BTreeSet<u32> = rt.elements().iter().filter(|e| e.role == "icon" && e.path.contains("/titlebar/")).map(|e| e.rect[2].round() as u32).collect();
            assert_eq!(sizes.into_iter().collect::<Vec<_>>(), [15], "{mode:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// KT: a title bar tooltip names its button and its key on one line in the panel family, as
    /// the key is in the palette. With `panel_font` set to `mono` its characters share one
    /// width (SB-07).
    #[test]
    fn tooltips_name_the_key_in_the_panel_family() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tooltips-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        let line = crate::ui::styles::panel_line(11.5);
        let mut per_char = Vec::new();
        for button in ["toggle-right", "toggle-left", "mode-map"] {
            rt.pointer_move(640.0, 500.0);
            for _ in 0..30 {
                rt.frame(DT);
            }
            let at = rt.find(button).expect("the button");
            rt.pointer_move(at.0, at.1);
            for _ in 0..90 {
                rt.frame(DT);
            }
            let (text, r) = rt.help_tooltip().map(|(t, r)| (t.to_string(), r)).unwrap_or_else(|| panic!("{button} shows a tooltip"));
            assert!(text.contains(" (") && text.ends_with(')'), "{button}: '{text}' names its key");
            assert!((r[3] - (line + 8.0)).abs() < 0.6, "{button}: '{text}' is one line of the panel family, {} high", r[3]);
            per_char.push((text.clone(), (r[2] - 14.0) / text.chars().count() as f32));
        }
        let (lo, hi) = per_char.iter().fold((f32::MAX, 0.0f32), |(lo, hi), (_, w)| (lo.min(*w), hi.max(*w)));
        if crate::ui::styles::panel_family() == crate::ui::styles::panel_family_of("mono") {
            assert!(hi - lo < 0.25, "every character is one width, as in a monospace: {per_char:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ELGAP: a name or folder cut to the title bar's room ends where its ink does, so "/" sits
    /// as close after a cut folder as after one shown whole, at every width and scale; the cut
    /// name keeps its whole text for the tooltip and the screen reader. DOTS: ".md" sits a thin
    /// gap after a cut name's ellipsis, so the two never read as four dots, and right after a
    /// name shown whole.
    #[test]
    fn a_cut_name_keeps_a_thin_gap_before_its_extension() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-elgap-{}", std::process::id()));
        let name = "A very long name for a note that keeps going and going on and on past every edge";
        let folder = "Quarterly planning notes for the product and design teams across every region";
        let mut rt = vault(&dir, &[(&format!("{folder}/{name}.md"), "# A\n"), (&format!("{folder}/Short.md"), "# B\n")], crate::watch::EVERY);
        let style = crate::ui::styles::literal(12.0);
        let bar = |rt: &Runtime<crate::app::App>| -> Vec<rune::ElementInfo> {
            rt.elements().into_iter().filter(|e| e.path.contains("/crumb/") && !e.exiting && e.text.is_some()).collect()
        };
        for note in [name, "Short"] {
            rt.update(|s, tx| s.open_file(&format!("{folder}/{note}.md"), None, tx));
            for (w, scale) in [(1280.0, 1.0), (640.0, 1.0), (520.0, 1.0), (640.0, 1.5), (520.0, 2.0)] {
                rt.set_viewport((w, 600.0), scale);
                for _ in 0..30 {
                    rt.frame(DT);
                }
                let els = bar(&rt);
                let after = |sep: &str| els.iter().position(|e| e.text.as_deref() == Some(sep)).map(|i| (&els[i - 1], &els[i])).unwrap_or_else(|| panic!("{note} at {w}: no {sep}"));
                for sep in ["/", ".md"] {
                    let (cut, next) = after(sep);
                    let t = cut.text.clone().unwrap_or_default();
                    let ink = rune::text::measure(&t, &[], style, None).0;
                    assert!((cut.rect[2] - ink).abs() <= 1.0, "{note} at {w} x{scale}: '{t}' before '{sep}' is {} wide for {ink} of text", cut.rect[2]);
                    let gap = next.rect[0] - (cut.rect[0] + cut.rect[2]);
                    let want = match sep {
                        "/" => 6.0,
                        _ if t.ends_with('…') => crate::ui::editor::CUT_GAP,
                        _ => 0.0,
                    };
                    assert!((gap - want).abs() < 0.5, "{note} at {w} x{scale}: '{sep}' sits {gap} pt after '{t}', not {want}");
                }
                let (shown, _) = after(".md");
                if shown.text.as_deref() != Some(note) {
                    assert!(shown.text.as_deref().is_some_and(|t| t.ends_with('…')), "{note} at {w}: a cut name ends in an ellipsis: {:?}", shown.text);
                    assert_eq!(shown.label.as_deref(), Some(note), "{note} at {w}: a cut name keeps its whole text");
                    assert!(shown.help.is_some(), "{note} at {w}: a cut name has its tooltip");
                }
                if w == 520.0 {
                    assert_ne!(after("/").0.text.as_deref(), Some(folder), "{note}: the case cuts the folder");
                    assert!(note == "Short" || shown.text.as_deref() != Some(note), "the case cuts the long name");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// EL: a long name stays in its room and ends in an ellipsis. In the title bar it ends
    /// before the bar's buttons; in the quick switcher a long folder never squeezes the name;
    /// the search field's key hint ends inside the field, and a tab's long name is cut in its
    /// row (SB-10, SB-128).
    #[test]
    fn long_names_stay_in_their_room() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-longnames-{}", std::process::id()));
        let long = "A moderately long but perfectly normal note title about planning the next product release";
        let folder = "Quarterly planning notes for the product and design teams across every region";
        let mut rt = vault(&dir, &[(&format!("{long}.md"), "# A\n"), (&format!("{folder}/{long} two.md"), "# B\n")], crate::watch::EVERY);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..30 {
                rt.frame(DT);
            }
        };
        let rect = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect).unwrap_or_else(|| panic!("{key} shows"));
        let texts = |rt: &Runtime<crate::app::App>, under: &str| -> Vec<(String, [f32; 4])> {
            rt.elements().into_iter().filter(|e| e.path.contains(&format!("/{under}/")) && !e.exiting).filter_map(|e| e.text.map(|t| (t, e.rect))).collect()
        };
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        let buttons = rect(&rt, "right-zone")[0];
        for (t, r) in texts(&rt, "crumb") {
            assert!(r[0] + r[2] <= buttons + 0.5, "the title bar's '{t}' ends at {} past the buttons at {buttons}", r[0] + r[2]);
        }
        rt.update(|s, tx| s.open_search(tx));
        settle(&mut rt);
        let (field, new) = (rect(&rt, "search-box"), rect(&rt, "new-note"));
        if let Some(kbd) = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("kbd")).map(|e| e.rect) {
            assert!(kbd[0] > field[0] && kbd[0] + kbd[2] <= field[0] + field[2] - 2.0, "the key hint {kbd:?} ends inside the field {field:?}");
        }
        assert!(field[0] + field[2] + 6.0 <= new[0] + 0.5, "the field {field:?} ends 6 pt before \"+\" {new:?}");
        assert!(rect(&rt, "search")[2] >= 80.0, "the field keeps room to type: {:?}", rect(&rt, "search"));
        let side = rect(&rt, "sidebar-over");
        for (t, r) in texts(&rt, "open-notes") {
            assert!(r[0] + r[2] <= side[0] + side[2] - 12.0, "the tab's '{t}' ends at {} in the sidebar at {}", r[0] + r[2], side[0] + side[2]);
        }
        rt.update(|s, tx| s.close_layers(tx));
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Files, tx));
        settle(&mut rt);
        let row = (0..4).map(|i| format!("pal-{i}")).find(|k| texts(&rt, k).iter().any(|(t, _)| t == folder)).expect("the switcher lists the note in the folder");
        let parts = texts(&rt, &row);
        let name = parts.iter().find(|(t, _)| t.starts_with(long)).unwrap().1;
        let sub = parts.iter().find(|(t, _)| t == folder).unwrap().1;
        assert!(name[2] >= 1.4 * sub[2], "the name keeps most of the row: {} wide, its folder {}", name[2], sub[2]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sidebar's small buttons, its section labels and its top bar's and footer's buttons
    /// take a pointer over at least 24 by 24 points, and the rows they sit in keep their height
    /// (SB-08).
    #[test]
    fn sidebar_targets_are_large_enough() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-targets-{}", std::process::id()));
        let mut rt = vault(&dir, &[("a/note.md", "# A\n"), ("b.md", "# B\n")], crate::watch::EVERY);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let mut seen = std::collections::BTreeSet::new();
        for e in &els {
            let Some(k) = e.key.as_deref() else { continue };
            if matches!(k, "x" | "add" | "new-note" | "label-shelf" | "label-vault" | "foot-tags" | "foot-settings") {
                assert!(e.rect[2] >= 24.0 && e.rect[3] >= 24.0, "{k} is {:?}", e.rect);
                seen.insert(k.to_string());
            }
            if k.starts_with("dir:") {
                assert_eq!(e.rect[3], 27.0, "{k} keeps its height");
            }
            if k.starts_with("tab:") {
                assert_eq!(e.rect[3], 35.0, "{k}, a tab of two rows, keeps its height");
            }
        }
        assert_eq!(seen.len(), 7, "every kind was measured: {seen:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// In a small window each settings row stacks its control under its text, so descriptions
    /// keep a readable width and no text runs past the page's margin.
    #[test]
    fn settings_fit_a_small_window() {
        use crate::app::Sec;
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-setsmall-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        for (w, h) in [(520.0, 400.0), (640.0, 480.0)] {
            rt.set_viewport((w, h), 1.0);
            for sec in [Sec::Appearance, Sec::Editor, Sec::Files, Sec::Sync, Sec::Mcp, Sec::Keys] {
                rt.update(|s, tx| {
                    s.open_settings(tx);
                    s.set_sec = sec;
                });
                for _ in 0..30 {
                    rt.frame(DT);
                }
                let els = rt.elements();
                let body = els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("set-body-"))).unwrap().rect;
                let edge = body[0] + body[2] - 24.0;
                for e in els.iter().filter(|e| e.text.is_some() && e.path.contains("/set-body-") && !e.path.contains("/cfg/")) {
                    assert!(e.rect[0] + e.rect[2] <= edge + 0.5, "{w}x{h} {sec:?}: {:?} ends at {} past {edge}", e.text, e.rect[0] + e.rect[2]);
                }
                if sec == Sec::Appearance {
                    let desc = els.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("Match system follows"))).unwrap();
                    assert!(desc.rect[2] >= 180.0, "{w}x{h}: the Mode description is {} wide", desc.rect[2]);
                }
            }
        }
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
            std::fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
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

    /// C7: a name that shows in full has no tooltip; a name cut short offers it whole, without
    /// its extension, in the tree and in the Open list.
    #[test]
    fn a_name_tooltip_shows_only_when_the_name_is_cut() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-cut-tips-{}", std::process::id()));
        let long = "A note whose name is far too long for the sidebar to show.md";
        let mut rt = vault(&dir, &[("short.md", "# A\n"), (long, "# B\n")], crate::watch::EVERY);
        rt.update(move |s, tx| s.open_file(long, None, tx));
        rt.update(|s, tx| s.open_file("short.md", None, tx));
        for _ in 0..5 {
            rt.frame(crate::bench::DT);
        }
        // The tooltips under a row, excluding its action buttons (close, outline chevron).
        let tips = |rt: &Runtime<crate::app::App>, row: &str| -> Vec<String> {
            rt.elements()
                .into_iter()
                .filter(|e| e.path.split('/').any(|seg| seg == row) && !e.path.ends_with("/x") && !e.key.as_deref().is_some_and(|k| k.starts_with("tab-chev:")))
                .filter_map(|e| e.help)
                .collect()
        };
        for row in ["file:short.md", "tab:short.md"] {
            assert!(tips(&rt, row).is_empty(), "{row} shows its whole name, so no tooltip");
        }
        for row in [format!("file:{long}"), format!("tab:{long}")] {
            assert_eq!(tips(&rt, &row), [long.trim_end_matches(".md")], "{row} is cut, so it offers the whole name");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// TN: a button's tooltip names its command as the palette does, in the state the app is
    /// in: the sidebar and outline toggles, the mode and zen buttons, and the toolbar's image
    /// and lock buttons.
    #[test]
    fn tooltips_and_palette_names_agree() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tip-names-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nSome text.\n")], crate::watch::EVERY);
        // Each button, and the palette command it runs: by its key, or by name for the lock.
        let pairs = [("toggle-left", "app.toggle-sidebar"), ("toggle-right", "app.toggle-outline"), ("mode-source", "app.mode-source"), ("mode-preview", "app.mode-preview"), ("mode-map", "app.mode-map"), ("focus", "app.focus-mode"), ("fmt-image", "app.insert-image"), ("fmt-lock", "")];
        let name = |tip: &str| match tip.rsplit_once(" (") {
            Some((n, k)) if k.ends_with(')') => n.to_string(),
            _ => tip.to_string(),
        };
        let mut seen = std::collections::BTreeSet::new();
        let states: [(crate::app::Mode, bool, bool, bool); 5] = [
            (crate::app::Mode::Source, true, false, false),
            (crate::app::Mode::Source, false, true, false),
            (crate::app::Mode::Preview, true, true, false),
            (crate::app::Mode::Preview, false, false, false),
            (crate::app::Mode::Source, true, false, true),
        ];
        for (mode, left, right, zen) in states {
            rt.update(move |s, _| {
                s.mode = mode;
                s.persist.left_open = left;
                s.persist.right_open = right;
                s.focus = zen;
            });
            for _ in 0..30 {
                rt.frame(DT);
            }
            let mut palette = Vec::new();
            rt.update_if(|s, _| {
                palette = crate::commands::commands(s, false).into_iter().map(|c| (c.action, c.label)).collect();
                false
            });
            let els = rt.elements();
            // The shown mode's button names that mode; it runs nothing.
            let shown = match mode {
                crate::app::Mode::Source => "mode-source",
                crate::app::Mode::Preview => "mode-preview",
                crate::app::Mode::Map => "mode-map",
            };
            seen.insert(shown);
            for (key, action) in pairs {
                if key == shown {
                    continue;
                }
                let Some(tip) = els.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.help.clone()) else { continue };
                let tip = name(&tip);
                let want: Vec<&String> = palette.iter().filter(|(a, l)| if action.is_empty() { l.starts_with("Lock note") } else { *a == action }).map(|(_, l)| l).collect();
                assert_eq!(want, [&tip], "{mode:?} left {left} right {right} zen {zen}: the {key} button's tooltip and the palette");
                seen.insert(key);
            }
        }
        assert_eq!(seen.len(), pairs.len(), "every pair was compared: {seen:?}");
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
        assert_eq!(find.help.as_deref(), Some("Find in note (Ctrl+F)"), "its tooltip names the preset's key");
        // Every title bar button with a key names it the same way: "Name (Keys)".
        let tips = |rt: &Runtime<crate::app::App>| -> Vec<(String, String)> {
            rt.elements()
                .into_iter()
                .filter(|e| ["/titlebar/", "/left-zone/"].iter().any(|z| ("/".to_owned() + &e.path + "/").contains(z)) && matches!(e.role, "button" | "switch" | "radio") && !e.exiting)
                .filter_map(|e| Some((e.key?, e.help?)))
                .collect()
        };
        let keyed = ["toggle-left", "mode-source", "mode-preview", "mode-map", "focus", "find", "copy-all", "wide", "toggle-right", "help", "paragraph-focus", "typewriter", "next-font"];
        // The shown mode's button names that mode and has no key.
        let check = |tips: Vec<(String, String)>, want: usize, shown: &str| {
            let named: Vec<_> = tips.iter().filter(|(k, _)| keyed.contains(&k.as_str()) && k != shown).collect();
            assert_eq!(named.len(), want, "{tips:?}");
            for (key, tip) in named {
                assert!(tip.ends_with(')') && tip.contains(" (") && !tip.contains("  "), "{key}: {tip:?}");
            }
        };
        check(tips(&rt), 9, "mode-preview");
        rt.update(|s, tx| s.show_mode(crate::app::Mode::Source, tx));
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert!(rt.find("copy-all").is_none(), "source mode keeps its own bar");
        check(tips(&rt), 10, "mode-source");
        // The sidebar and outline buttons both show as active switches while their panels are open.
        rt.update(|s, _| s.persist.right_open = true);
        rt.pointer_move(700.0, 600.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(rt.find("outline").is_some(), "the outline is open");
        let left_btn = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toggle-left") && !e.exiting).expect("the sidebar button");
        assert!(left_btn.role == "switch" && left_btn.fill.a > 0.0, "the sidebar button shows as on in source mode");
        let button = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toggle-right") && !e.exiting).expect("the outline button");
        assert!(button.role == "switch" && button.fill.a > 0.0, "the outline button shows as on in source mode");
        // A collapsible section label's chevron starts on the text edge of the rows it labels,
        // with the label name 16 pt past it (10 pt chevron + 6 pt gap).
        let els = rt.elements();
        let chevron = els.iter().find(|e| e.text.is_none() && e.path.contains("/label-vault/") && (e.rect[2] - 10.0).abs() < 0.1).expect("the vault's chevron");
        let label = els.iter().find(|e| e.text.is_some() && e.path.contains("/label-vault/")).expect("the vault's name");
        let row = els.iter().find(|e| e.text.as_deref() == Some("note") && e.path.contains("file:note.md")).expect("the note's row");
        assert_eq!(chevron.rect[0], row.rect[0], "the vault's label chevron lines up with the rows");
        assert_eq!(label.rect[0], row.rect[0] + 16.0, "the vault's label name sits past its chevron");
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
                on = s.layer_is(crate::layer::Kind::Help);
                false
            });
            on
        };
        rt.update(|s, tx| s.toggle_help(tx));
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
        rt.update(|s, tx| s.toggle_help(tx));
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
        rt.update(|s, tx| s.toggle_help(tx));
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        settle(&mut rt);
        assert!(!help(&mut rt), "Esc closes the writer's help");
        assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("editor") && e.focused), "and the editor has the keyboard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn help_sheet_scrolls_by_keyboard_and_lists_mode_and_f1() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-helpkbd-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nBody.\n")], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((520.0, 400.0), 1.0);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let rect = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect);
        rt.update(|s, tx| s.keys_to_note(tx));
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Function(1), mods: Mods::default() });
        settle(&mut rt);
        let y0 = rect(&rt, "help-body").expect("help-body")[1];
        let close = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("help-close") && !e.exiting).expect("help-close");
        assert!(close.focusable, "help-close is keyboard-focusable");
        let all_text: String = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join(" ");
        assert!(all_text.contains(&crate::keymap::label_of("Ctrl+P")), "lists Ctrl+P: {all_text}");
        assert!(all_text.contains("F1"), "lists F1: {all_text}");
        for _ in 0..3 {
            rt.key(KeyEvent { key: Key::PageDown, mods: Mods::default() });
            settle(&mut rt);
        }
        let y_down = rect(&rt, "help-body").unwrap()[1];
        assert!(y_down < y0 - 80.0, "PageDown scrolls help-body: {y0} -> {y_down}");
        rt.key(KeyEvent { key: Key::Home, mods: Mods::default() });
        settle(&mut rt);
        let y_home = rect(&rt, "help-body").unwrap()[1];
        assert!((y_home - y0).abs() < 2.0, "Home returns to top: {y_home} vs {y0}");
        rt.key(KeyEvent { key: Key::End, mods: Mods::default() });
        settle(&mut rt);
        let y_end = rect(&rt, "help-body").unwrap()[1];
        assert!(y_end < y0 - 80.0, "End scrolls to bottom: {y0} -> {y_end}");
        // HELPSHADOW: close button focus ring clearance >= 3.5pt from top and header divider.
        let hhead = rect(&rt, "help-header").expect("help-header");
        let hclose = rect(&rt, "help-close").expect("help-close");
        let top_c = hclose[1] - hhead[1];
        let bot_c = (hhead[1] + hhead[3]) - (hclose[1] + hclose[3]);
        assert!(top_c.min(bot_c) - 5.0 >= 3.5, "help-close ring clearance >= 3.5pt: top={top_c}, bot={bot_c}");
        // HELPSHADOW: rapid End -> wheel up -> Up stays in sync and moves further up, never deeper.
        rt.set_viewport((520.0, 400.0), 1.5);
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::End, mods: Mods::default() });
        for _ in 0..9 {
            rt.frame(DT);
        }
        rt.wheel(260.0, 200.0, 120.0);
        settle(&mut rt);
        let y_wheel_only = rect(&rt, "help-body").unwrap()[1];
        rt.key(KeyEvent { key: Key::End, mods: Mods::default() });
        for _ in 0..9 {
            rt.frame(DT);
        }
        rt.wheel(260.0, 200.0, 120.0);
        for _ in 0..3 {
            rt.frame(DT);
        }
        rt.key(KeyEvent { key: Key::Up, mods: Mods::default() });
        settle(&mut rt);
        let y_wheel_up = rect(&rt, "help-body").unwrap()[1];
        assert!(y_wheel_up > y_wheel_only + 10.0, "rapid wheel up + Up moves further up: wheel_only={y_wheel_only:.1}, wheel+Up={y_wheel_up:.1}");
        let (cx, cy) = rt.find("help-close").unwrap();
        rt.pointer_move(cx, cy);
        rt.pointer_down(cx, cy);
        rt.pointer_up(cx, cy);
        settle(&mut rt);
        let mut open = true;
        rt.update_if(|s, _| {
            open = s.layer_is(crate::layer::Kind::Help);
            false
        });
        assert!(!open, "help-close dismisses help");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn welcome_keyboard_focus_recent_menu_and_narrow_lock() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-welkbd-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..20 {
                rt.frame(DT);
            }
        };
        // Narrow lock dialog fits a 340pt window.
        rt.set_viewport((340.0, 500.0), 1.0);
        rt.update(|s, tx| s.ask_lock(tx));
        settle(&mut rt);
        let dlg = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("lock-dialog") && !e.exiting).expect("lock-dialog");
        assert!(dlg.rect[0] >= 0.0 && dlg.rect[0] + dlg.rect[2] <= 340.0, "lock-dialog fits narrow window: {:?}", dlg.rect);
        rt.update(|s, tx| s.close_layers(tx));
        // vault-menu near the bottom accounts for header height and never clips below vh.
        rt.update(|s, tx| s.open_menu(crate::layer::MenuOf::Folder, (20.0, 490.0), tx));
        settle(&mut rt);
        let vm = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("vault-menu") && !e.exiting).expect("vault-menu");
        assert!(vm.rect[1] + vm.rect[3] <= 500.0, "vault-menu stays inside bottom edge: {:?}", vm.rect);
        // Welcome screen: Tab focuses `create` with a visible ring, and `recent-0` opens in-app menu.
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.go_welcome(tx));
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
        settle(&mut rt);
        let els = rt.elements();
        let create = els.iter().find(|e| e.key.as_deref() == Some("create")).expect("create");
        let open = els.iter().find(|e| e.key.as_deref() == Some("open")).expect("open");
        let recent0 = els.iter().find(|e| e.key.as_deref() == Some("recent-0")).expect("recent-0");
        assert!(create.focusable && open.focusable && recent0.focusable);
        assert!(create.focused && create.focus_visible, "Tab focuses create with visible ring");
        // In-app recent-menu removes a folder from recents.
        let mut path = std::path::PathBuf::new();
        let mut before = 0usize;
        rt.update_if(|s, _| {
            before = s.persist.vaults.len();
            path = s.persist.vaults[0].path.clone();
            false
        });
        rt.update(move |s, tx| s.open_menu(crate::layer::MenuOf::RecentVault(path), (500.0, 500.0), tx));
        settle(&mut rt);
        assert!(rt.find("recent-menu").is_some(), "recent-menu opens as in-app menu");
        let (fx, fy) = rt.find("menu-forget").expect("menu-forget");
        rt.pointer_move(fx, fy);
        rt.pointer_down(fx, fy);
        rt.pointer_up(fx, fy);
        settle(&mut rt);
        let mut after = 0usize;
        rt.update_if(|s, _| {
            after = s.persist.vaults.len();
            false
        });
        assert_eq!(after, before - 1, "menu-forget removes the recent folder");
        // WELFOCUS: Cancel on narrow Welcome returns focus to `create` (in view, not clipped recent-0).
        rt.set_viewport((520.0, 400.0), 1.0);
        rt.update(|s, tx| {
            s.go_welcome(tx);
            tx.blur();
        });
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Tab, mods: Mods::default() });
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        settle(&mut rt);
        let focused = rt.elements().into_iter().find(|e| e.focused && !e.exiting).expect("focused after Cancel");
        assert_eq!(focused.key.as_deref(), Some("create"), "Cancel returns focus to create: {:?}", focused.key);
        assert!(focused.rect[1] >= 0.0 && focused.rect[1] + focused.rect[3] <= 400.0, "create is in view: {:?}", focused.rect);
        // WELFOCUS: pointer-opened recent-menu activates cleanly on Enter.
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, _| {
            if s.persist.vaults.is_empty() {
                s.persist.vaults.push(crate::store::VaultRef { name: "Vault".into(), path: dir.clone() });
            }
        });
        settle(&mut rt);
        let (rx, ry) = rt.find("recent-0").expect("recent-0");
        rt.pointer_move(rx, ry);
        rt.secondary_down(rx, ry);
        rt.secondary_up(rx, ry);
        settle(&mut rt);
        assert!(rt.find("recent-menu").is_some(), "secondary click opens recent-menu");
        // REVEALWEL: pointer-opened recent-menu visibly highlights menu-forget immediately.
        let mf = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("menu-forget") && !e.exiting).expect("menu-forget");
        assert!(mf.focused && mf.focus_visible, "pointer-opened recent-menu visibly marks menu-forget: focused={} visible={}", mf.focused, mf.focus_visible);
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        settle(&mut rt);
        assert!(rt.find("recent-menu").is_none(), "Enter closes recent-menu");
        assert!(rt.find("recent-0").is_none(), "Enter removes recent-0");
        // REVEALWEL: removing the last recent vault returns focus to `create`.
        let foc = rt.elements().into_iter().find(|e| e.focused && !e.exiting).expect("focused after removing last recent");
        assert_eq!(foc.key.as_deref(), Some("create"), "removing last recent returns focus to create: {:?}", foc.key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-122: in the source the find bar sits on the formatting toolbar's row when it fits
    /// beside it, and otherwise takes its own row under it, with the text starting below that
    /// row. The two never overlap, from the smallest window up.
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
        for (w, h, beside) in [(520.0, 400.0, false), (720.0, 600.0, false), (1000.0, 700.0, false), (1280.0, 800.0, true), (1400.0, 1000.0, true)] {
            rt.set_viewport((w, h), 1.0);
            for _ in 0..20 {
                rt.frame(DT);
            }
            let rect = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect).expect(key);
            let (bar, find, text) = (rect("toolbar"), rect("find-bar"), rect("editor"));
            assert_eq!(bar[2], crate::ui::editor::TOOLBAR_W, "the toolbar's width");
            assert!(find[0] >= 0.0 && find[0] + find[2] <= w, "at {w}x{h} the find bar is inside the window");
            if beside {
                assert_eq!(find[1] + find[3] / 2.0, bar[1] + bar[3] / 2.0, "at {w}x{h} the find bar {find:?} is centred on the toolbar's row {bar:?}");
                assert!(find[0] >= bar[0] + bar[2] + 16.0, "with room between them");
            } else {
                assert!(find[1] >= bar[1] + bar[3], "at {w}x{h} the find bar {find:?} sits under the toolbar {bar:?}");
                assert!(text[1] >= find[1] + find[3], "and the text {text:?} starts below it");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// NARROWFIND: at narrow widths (<= 560 pt, e.g. 520×400), the find bar spans the usable
    /// width with crisp 12 pt margins (no preview text slicing left of it), keeps >= 24×24 pt
    /// action chips, and compacts the stacked Source toolbar + find bar to <= 92 pt below the
    /// title bar.
    #[test]
    fn narrowfind_spans_width_and_compacts_source_stack() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-narrowfind-{}", std::process::id()));
        let md = "# Rendering probes\n\nA long paragraph with the word the near the start.\n\n".repeat(12);
        let mut rt = vault(&dir, &[("note.md", md.as_str())], crate::watch::EVERY);
        rt.set_viewport((520.0, 400.0), 1.0);
        // 1. Preview mode at 520×400
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Preview;
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("the");
            s.find_changed(tx);
        });
        for _ in 0..20 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let get = |k: &str| els.iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("{k}")).rect;
        let fb = get("find-bar");
        assert!(fb[0] <= 16.0 && fb[2] >= 488.0, "preview narrow find-bar spans width: {fb:?}");
        assert!(els.iter().any(|e| e.key.as_deref() == Some("find-narrow-band") && !e.exiting), "narrow preview backing band renders");
        for k in ["find-prev", "find-next", "find-close"] {
            let b = get(k);
            assert!(b[2] >= 24.0 && b[3] >= 24.0, "{k} keeps >= 24×24 pt target: {b:?}");
        }
        for e in &els {
            if e.text.is_some() && e.path.contains("/preview/") {
                let r = e.rect;
                assert!(!(r[1] < fb[1] + fb[3] - 1.0 && r[1] + r[3] > fb[1] + 1.0 && r[0] < fb[0] - 2.0), "no preview text slices left of find-bar: {:?} {r:?} vs {fb:?}", e.text);
            }
        }
        // 2. Source mode at 520×400: toolbar + find-bar stack ends <= 92 pt below titlebar (41 pt).
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.writer_find(tx);
        });
        for _ in 0..20 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let get = |k: &str| els.iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("{k}")).rect;
        let (bar, fb) = (get("toolbar"), get("find-bar"));
        let stack_h = fb[1] + fb[3] - 41.0;
        assert!(fb[0] <= 16.0 && fb[2] >= 488.0, "source narrow find-bar spans width: {fb:?}");
        assert!(fb[1] >= bar[1] + bar[3], "find-bar clears toolbar: bar={bar:?}, fb={fb:?}");
        assert!(stack_h <= 92.0, "narrow source toolbar+find stack <= 92 pt below titlebar: {stack_h}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDPOLISH: reading progress stays painted above `find-narrow-band`, 561×400 and 640×480
    /// Preview Find span full width without peeking text, Find steppers sit on a 24×24 pt grid,
    /// and narrow Source toolbar aligns its left edge with `find-bar`.
    #[test]
    fn findpolish_progress_breakpoint_steppers_and_aligned_toolbar() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findpolish-{}", std::process::id()));
        let md = format!("# Title\n\n{}", "Paragraph of readable prose for Find polish checks.\n\n".repeat(45));
        let mut rt = vault(&dir, &[("note.md", md.as_str())], crate::watch::EVERY);
        // 1. 520×400 scrolled Preview: progress quad paints after find-narrow-band; steppers are 24×24.
        rt.set_viewport((520.0, 400.0), 1.0);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Preview;
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("prose");
            s.find_changed(tx);
        });
        for _ in 0..10 {
            rt.frame(DT);
        }
        rt.wheel(260.0, 200.0, -240.0);
        for _ in 0..20 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let get = |k: &str| els.iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("{k}")).rect;
        let prog = get("progress");
        let band = get("find-narrow-band");
        assert!(prog[2] > 10.0 && prog[3] >= 1.5, "scrolled progress bar has width: {prog:?}");
        let mut acc = rune::Color::BLACK;
        rt.update_if(|s, _| {
            acc = crate::roles::Roles::new(s.palette(false)).accent;
            false
        });
        let near = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 0.02);
        let prog_idx = rt.instances.iter().rposition(|q| near(q.fill, [acc.r, acc.g, acc.b, acc.a]) && (q.rect[1] - prog[1]).abs() < 1.0 && q.rect[3] <= 3.0);
        let band_idx = rt.instances.iter().rposition(|q| (q.rect[1] - band[1]).abs() < 1.0 && (q.rect[3] - band[3]).abs() < 1.0);
        assert!(prog_idx.is_some() && band_idx.is_some() && prog_idx.unwrap() > band_idx.unwrap(), "progress ({prog_idx:?}) must paint above find-narrow-band ({band_idx:?})");
        for k in ["find-prev", "find-next", "find-close"] {
            let r = get(k);
            assert!((r[2] - 24.0).abs() <= 0.5 && (r[3] - 24.0).abs() <= 0.5, "{k} must be 24×24 pt: {r:?}");
        }
        // 2. 561×400 and 640×480 scrolled Preview: full-width Find with zero peeking text left of bar.
        for (w, h) in [(561.0, 400.0), (640.0, 480.0)] {
            rt.set_viewport((w, h), 1.0);
            for _ in 0..15 {
                rt.frame(DT);
            }
            let els = rt.elements();
            let fb = els.iter().find(|e| e.key.as_deref() == Some("find-bar") && !e.exiting).unwrap().rect;
            assert!(fb[0] <= 14.0 && fb[2] >= w - 28.0, "at {w}x{h} narrow find spans width: {fb:?}");
            for e in &els {
                if !e.exiting && e.text.is_some() && e.path.contains("/preview/") {
                    let r = e.rect;
                    assert!(!(r[1] < fb[1] + fb[3] - 1.0 && r[1] + r[3] > fb[1] + 1.0 && r[0] < fb[0] - 2.0), "at {w}x{h} no peeking text left of find-bar: {:?} {r:?} vs {fb:?}", e.text);
                }
            }
        }
        // 3. 520×400 Source mode: toolbar left edge aligns with find-bar within <= 2 pt.
        rt.set_viewport((520.0, 400.0), 1.0);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.writer_find(tx);
        });
        for _ in 0..15 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let tb = els.iter().find(|e| e.key.as_deref() == Some("toolbar") && !e.exiting).unwrap().rect;
        let fb = els.iter().find(|e| e.key.as_deref() == Some("find-bar") && !e.exiting).unwrap().rect;
        assert!((tb[0] - fb[0]).abs() <= 2.0, "narrow Source toolbar ({tb:?}) aligns left edge with find-bar ({fb:?})");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDMODEL: full-width page-coloured Find backing covers Preview and toolbar-hidden Source
    /// across 520, 680, 760 and 1280 pt widths with smoothstep clearance below find-bar, progress
    /// stays painted above the backing, and 520×400 sidebar-over drawer never slices the crumb.
    #[test]
    fn findmodel_wide_narrow_backing_progress_and_drawer_crumb() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findmodel-{}", std::process::id()));
        let md = format!("# Rendering probes\n\n| Col A | Col B |\n| --- | --- |\n| one | two |\n\n{}", "A paragraph with the word the for Find backing tests.\n\n".repeat(40));
        let mut rt = vault(&dir, &[("d01/d02/d03/d04/d05/d06/Leaf.md", md.as_str())], crate::watch::EVERY);
        // 1. Preview across 520×400, 680×480, 760×480, 1280×800 (closed sidebar):
        // full-width backing band extends smoothly past find-bar foot and paints below progress.
        rt.update(|s, tx| {
            s.persist.left_open = false;
            s.mode = crate::app::Mode::Preview;
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("the");
            s.find_changed(tx);
        });
        for (w, h, want_key) in [(520.0, 400.0, "find-narrow-band"), (680.0, 480.0, "find-narrow-band"), (760.0, 480.0, "find-band"), (1280.0, 800.0, "find-band")] {
            rt.set_viewport((w, h), 1.0);
            for _ in 0..10 {
                rt.frame(DT);
            }
            rt.wheel(w / 2.0, h / 2.0, -200.0);
            for _ in 0..20 {
                rt.frame(DT);
            }
            let els = rt.elements();
            let get = |k: &str| els.iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("{w}x{h}: missing {k}")).rect;
            let fb = get("find-bar");
            let band = get(want_key);
            let scroll = get("doc-scroll");
            let prog = get("progress");
            assert!(band[2] >= w - 2.0, "{w}x{h} {want_key} must span note area width: {band:?}");
            assert!(band[1] + band[3] >= fb[1] + fb[3] + 6.0, "{w}x{h} {want_key} must clear find-bar foot: band={band:?}, fb={fb:?}");
            assert!(scroll[1] >= fb[1] + fb[3] + 6.0, "{w}x{h} doc-scroll must start strictly below find-bar: scroll={scroll:?}, fb={fb:?}");
            let mut acc = rune::Color::BLACK;
            rt.update_if(|s, _| {
                acc = crate::roles::Roles::new(s.palette(false)).accent;
                false
            });
            let near = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 0.02);
            let prog_idx = rt.instances.iter().rposition(|q| near(q.fill, [acc.r, acc.g, acc.b, acc.a]) && (q.rect[1] - prog[1]).abs() < 1.0 && q.rect[3] <= 3.0);
            let band_idx = rt.instances.iter().rposition(|q| (q.rect[1] - band[1]).abs() < 1.0 && (q.rect[3] - band[3]).abs() < 1.0);
            assert!(prog_idx.is_some() && band_idx.is_some() && prog_idx.unwrap() > band_idx.unwrap(), "{w}x{h}: progress ({prog_idx:?}) must paint above {want_key} ({band_idx:?})");
        }
        // 1b. 760×480 Preview with docked sidebar: doc-scroll starts below find-bar and zero preview text overlaps find-bar.
        rt.update(|s, _| s.persist.left_open = true);
        rt.set_viewport((760.0, 480.0), 1.0);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let fb = els.iter().find(|e| e.key.as_deref() == Some("find-bar") && !e.exiting).unwrap().rect;
        let scroll = els.iter().find(|e| e.key.as_deref() == Some("doc-scroll") && !e.exiting).unwrap().rect;
        assert!(scroll[1] >= fb[1] + fb[3] + 6.0, "760×480 docked-sidebar doc-scroll clears find-bar: scroll={scroll:?}, fb={fb:?}");
        rt.update(|s, _| s.persist.left_open = false);
        // 2. Toolbar-hidden Source at 760×480 also renders full-width find-band above doc-scroll.
        rt.set_viewport((760.0, 480.0), 1.0);
        rt.update(|s, tx| {
            s.settings_mut().toolbar = false;
            s.mode = crate::app::Mode::Source;
            s.writer_find(tx);
        });
        for _ in 0..15 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let fb = els.iter().find(|e| e.key.as_deref() == Some("find-bar") && !e.exiting).unwrap().rect;
        let band = els.iter().find(|e| e.key.as_deref() == Some("find-band") && !e.exiting).expect("toolbar-hidden Source renders find-band").rect;
        let scroll = els.iter().find(|e| e.key.as_deref() == Some("doc-scroll") && !e.exiting).unwrap().rect;
        assert!(band[2] >= 758.0 && band[1] + band[3] >= fb[1] + fb[3] + 6.0 && scroll[1] >= fb[1] + fb[3] + 6.0, "toolbar-hidden Source find-band clears find-bar: {band:?} vs {fb:?}, scroll={scroll:?}");
        // 3. 520×400 sidebar-over drawer hides titlebar crumb so drawer edge never slices text.
        rt.set_viewport((520.0, 400.0), 1.0);
        rt.frame(DT);
        rt.update(|s, tx| {
            s.close_find(tx);
            s.toggle_left(tx);
        });
        for _ in 0..15 {
            rt.frame(DT);
        }
        let els = rt.elements();
        assert!(els.iter().any(|e| e.key.as_deref() == Some("sidebar-over") && !e.exiting), "sidebar-over drawer opens at 520×400");
        assert!(els.iter().all(|e| e.key.as_deref() != Some("crumb") || e.exiting), "titlebar crumb hides while sidebar-over drawer is open");
        rt.update(|s, tx| s.toggle_left(tx));
        for _ in 0..15 {
            rt.frame(DT);
        }
        assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("crumb") && !e.exiting), "titlebar crumb returns once drawer closes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-122: with the find bar on its own row, the band over the text eases out across the
    /// bar's foot to 40 pt under it, and the caret moving up stays below the fade.
    #[test]
    fn wr_122_the_caret_stays_below_the_find_band() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findband-{}", std::process::id()));
        let long = format!("# A\n\n{}", "A line of text.\n\n".repeat(60));
        let mut rt = vault(&dir, &[("note.md", long.as_str())], crate::watch::EVERY);
        rt.set_viewport((640.0, 480.0), 1.0);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.writer_find(tx);
        });
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..30 {
                rt.frame(DT);
            }
        };
        settle(&mut rt);
        rt.update(|s, tx| {
            let n = s.cur_edit().text.len();
            s.cur_edit().set_selection(n, n);
            s.focus_editor(tx);
        });
        settle(&mut rt);
        for _ in 0..30 {
            rt.key(KeyEvent { key: Key::Up, mods: Mods::default() });
            rt.frame(DT);
        }
        settle(&mut rt);
        let els = rt.elements();
        let get = |key: &str| els.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("{key}"));
        let (find, band) = (get("find-bar").rect, get("find-band").rect);
        assert_eq!(band[1] + band[3], find[1] + find[3] + 40.0, "the band eases out to 40 pt under the bar");
        let caret = get("editor").caret.expect("the editor has a caret");
        assert!(caret[1] >= band[1] + band[3] - 0.5, "the caret {caret:?} stays below the band's fade {band:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-40, WR-96: below 1100 pt the outline button opens the outline as a drawer over the
    /// page's right edge, the page keeps its width, and Esc, the button, a press outside it or
    /// a picked heading closes it. It never shows with the find bar. From 1100 pt up it docks
    /// as before.
    #[test]
    fn the_outline_opens_over_a_narrow_page() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-overlay-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\n## B\n\nText.\n")], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        rt.update(|s, _| s.mode = crate::app::Mode::Source);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let rect = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect);
        let press = |rt: &mut Runtime<crate::app::App>| {
            let (x, y) = rt.find("toggle-right").expect("the outline button");
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
        };
        rt.set_viewport((960.0, 600.0), 1.0);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_none(), "no outline at first");
        let page = rect(&rt, "doc-area").unwrap();
        press(&mut rt);
        settle(&mut rt);
        let o = rect(&rt, "outline").expect("the button opens the outline");
        assert!(o[0] >= 0.0 && o[0] + o[2] <= 960.0 && o[2] >= 180.0, "inside the window: {o:?}");
        assert_eq!(rect(&rt, "doc-area").unwrap(), page, "the page does not reflow under it");
        assert!(rect(&rt, "outline-over").is_some(), "over the page, not docked");
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_none(), "Esc closes it");
        press(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_some(), "the button opens it again");
        press(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_none(), "and closes it");
        // It is a drawer: a press outside it closes it and reaches nothing under it.
        press(&mut rt);
        settle(&mut rt);
        let caret = |rt: &mut Runtime<crate::app::App>| {
            let mut at = 0;
            rt.update_if(|s, _| {
                at = s.cur_edit().focus;
                false
            });
            at
        };
        let before = caret(&mut rt);
        rt.pointer_move(200.0, 300.0);
        rt.pointer_down(200.0, 300.0);
        rt.pointer_up(200.0, 300.0);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_none(), "a press outside closes it");
        assert_eq!(caret(&mut rt), before, "and does not move the caret");
        // It never shares the page with the find bar, in either order.
        press(&mut rt);
        settle(&mut rt);
        rt.update(|s, tx| s.writer_find(tx));
        settle(&mut rt);
        assert!(rect(&rt, "find-bar").is_some() && rect(&rt, "outline").is_none(), "find closes the drawer");
        press(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_some() && rect(&rt, "find-bar").is_none(), "the drawer closes find");
        // A picked heading closes it.
        let (x, y) = rt.find("ol-1").expect("heading B");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_none(), "a picked heading closes it");
        // Wide, the button docks it and the page makes room.
        rt.set_viewport((1400.0, 1000.0), 1.0);
        settle(&mut rt);
        let page = rect(&rt, "doc-area").unwrap();
        press(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "outline").is_some() && rect(&rt, "outline-over").is_none(), "docked");
        assert!(rect(&rt, "doc-area").unwrap()[2] < page[2], "the page makes room");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-109: on a mode switch the old text goes at once and the new one fades in from 40%, so
    /// the two never show together, the page never falls below 40%, and the switch is over
    /// within 250 ms.
    #[test]
    fn wr_109_the_mode_switch_shows_one_text() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-swap-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nSome text to read.\n")], crate::watch::EVERY);
        rt.update(|s, _| s.mode = crate::app::Mode::Source);
        for _ in 0..40 {
            rt.frame(DT);
        }
        let shown = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().filter(|e| e.key.as_deref() == Some(key)).map(|e| e.opacity).fold(0.0f32, f32::max);
        let (x, y) = rt.find("mode-preview").expect("the Preview button");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        let mut faded = false;
        for i in 1..=15 {
            rt.frame(DT);
            let (src, pre) = (shown(&rt, "source"), shown(&rt, "preview"));
            assert!(src.min(pre) <= 0.05, "frame {i}: source at {src} and preview at {pre} show together");
            assert!(src.max(pre) >= crate::ui::SWAP_FROM - 0.01, "frame {i}: the page shows its text at {}", src.max(pre));
            faded |= pre > 0.0 && pre < 0.9;
            if i as f64 * DT >= 0.25 {
                assert!(pre >= 0.97, "the preview shows whole by 250 ms: {pre}");
            }
        }
        assert!(faded, "the preview fades in");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tab switching slides the incoming page cleanly from the side on a fast critically damped
    /// curve at full opacity, edge-to-edge with the outgoing tab (zero blank-screen gap, zero
    /// double-exposure), reversing direction on backward switches and landing instantly under
    /// reduced motion.
    #[test]
    fn tab_switch_slides_directionally_without_blank_flash() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-slide-{}", std::process::id()));
        let mut rt = vault(
            &dir,
            &[("a.md", "# Alpha\n\nFirst tab content.\n"), ("b.md", "# Beta\n\nSecond tab content.\n")],
            crate::watch::EVERY,
        );
        rt.on_key = Some(crate::keys::on_key);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Preview;
            s.open_file("a.md", None, tx);
            s.open_file("b.md", None, tx);
            s.open_file("a.md", None, tx);
        });
        for _ in 0..40 {
            rt.frame(DT);
        }
        let stage_x = rt.presented("tab-stage").expect("tab-stage").0[0];
        let stage_w = rt.presented("tab-stage").expect("tab-stage").0[2];

        // Forward switch (a.md -> b.md via Ctrl+Tab): incoming slides in from right, outgoing leaves left.
        let ctrl_tab = rune::KeyEvent { key: rune::Key::Tab, mods: rune::Mods { ctrl: true, cmd: !cfg!(target_os = "macos"), ..rune::Mods::default() } };
        let ctrl_shift_tab = rune::KeyEvent { key: rune::Key::Tab, mods: rune::Mods { ctrl: true, cmd: !cfg!(target_os = "macos"), shift: true, ..rune::Mods::default() } };
        let _ = rt.key(ctrl_tab.clone());
        let mut saw_fwd_slide = false;
        let mut prev_in_x = f32::INFINITY;
        for i in 1..=20 {
            rt.frame(DT);
            let els = rt.elements();
            let incoming = els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tab-page:")) && !e.exiting).expect("incoming tab-page");
            assert!(incoming.opacity >= 0.99, "frame {i}: incoming tab-page must stay full opacity, got {}", incoming.opacity);
            assert!(incoming.rect[0] <= prev_in_x + 0.1, "frame {i}: incoming x must move monotonically left toward target");
            prev_in_x = incoming.rect[0];
            if let Some(exiting) = els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tab-page:")) && e.exiting) {
                if incoming.rect[0] > stage_x + 20.0 {
                    saw_fwd_slide = true;
                    assert!(exiting.rect[0] < stage_x - 20.0, "frame {i}: outgoing tab should exit left ({})", exiting.rect[0]);
                    let seam = (exiting.rect[0] + stage_w - incoming.rect[0]).abs();
                    assert!(seam < 2.0, "frame {i}: outgoing/incoming seam must stay gapless, got {seam}");
                }
            }
        }
        assert!(saw_fwd_slide, "forward tab switch should slide in from the right");
        assert!((prev_in_x - stage_x).abs() < 2.0, "forward tab slide should settle cleanly by ~330 ms");

        // Backward switch (b.md -> a.md via Ctrl+Shift+Tab): incoming slides in from left, outgoing leaves right.
        let _ = rt.key(ctrl_shift_tab);
        let mut saw_bwd_slide = false;
        for i in 1..=15 {
            rt.frame(DT);
            let els = rt.elements();
            let incoming = els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tab-page:")) && !e.exiting).expect("incoming tab-page");
            assert!(incoming.opacity >= 0.99, "bwd frame {i}: incoming tab-page full opacity");
            if incoming.rect[0] < stage_x - 20.0 {
                saw_bwd_slide = true;
            }
        }
        assert!(saw_bwd_slide, "backward tab switch should slide in from the left");

        // Reduced motion lands at once with zero side travel.
        rt.set_system_reduced_motion(true);
        let _ = rt.key(ctrl_tab);
        rt.frame(DT);
        let els = rt.elements();
        let incoming = els.iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tab-page:")) && !e.exiting).expect("incoming tab-page");
        assert!((incoming.rect[0] - stage_x).abs() < 0.5, "reduced motion lands immediately");
        assert!(!els.iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tab-page:")) && e.exiting), "reduced motion has no exiting tab-page");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PAGE-84: under reduced motion the mode switch and the sidebar show and hide at once, with
    /// no frame mid-fade.
    #[test]
    fn page_84_reduced_motion_lands_at_once() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-reduced-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nSome text to read.\n")], crate::watch::EVERY);
        rt.set_system_reduced_motion(true);
        rt.update(|s, _| s.mode = crate::app::Mode::Source);
        for _ in 0..40 {
            rt.frame(DT);
        }
        let shown = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().filter(|e| e.key.as_deref() == Some(key)).map(|e| e.opacity).fold(0.0f32, f32::max);
        for (button, key) in [("mode-preview", "preview"), ("toggle-left", "sidebar")] {
            let (x, y) = rt.find(button).expect(button);
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            for i in 1..=15 {
                rt.frame(DT);
                let o = shown(&rt, key);
                assert!(o <= 0.03 || o >= 0.97, "{key} at {o} in frame {i} after {button}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One sidebar toggle: while the sidebar closes and opens again, one toggle shows, and it
    /// stays under the pointer.
    #[test]
    fn one_sidebar_toggle_stays_put() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toggle-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nSome text.\n")], crate::watch::EVERY);
        for _ in 0..40 {
            rt.frame(DT);
        }
        let toggles = |rt: &Runtime<crate::app::App>| rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("toggle-left") && e.opacity > 0.1).map(|e| e.rect).collect::<Vec<_>>();
        let at = toggles(&rt)[0];
        for _ in 0..2 {
            let (x, y) = rt.find("toggle-left").expect("the sidebar toggle");
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            for i in 1..=20 {
                rt.frame(DT);
                assert_eq!(toggles(&rt), vec![at], "frame {i}: one toggle, where it was");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A delete says what went and offers Undo for a while. Undo puts the note back on disk,
    /// with its tab where it was and shown again; a folder comes back with its notes.
    #[test]
    fn a_delete_can_be_undone() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-undo-{}", std::process::id()));
        let mut rt = vault(&dir, &[("a.md", "# A\n"), ("Drafts/b.md", "# B\n"), ("c.md", "# C\n")], crate::watch::EVERY);
        let root = dir.join("notes");
        rt.update(|s, tx| {
            s.open_file("Drafts/b.md", None, tx);
            s.open_file("c.md", None, tx);
            s.open_file("Drafts/b.md", None, tx);
            // An edit not yet saved goes to the trash with the note.
            s.set_text("Drafts/b.md", "# B edited\n".into());
        });
        // The open tabs, the shown note, and whether b and its folder are in the vault.
        let look = |rt: &mut Runtime<crate::app::App>| {
            let mut out = None;
            rt.update(|s, _| {
                let v = s.vault.as_ref().unwrap();
                out = Some((s.tabs.clone(), s.active.clone(), v.files().contains_key("Drafts/b.md"), v.folders.iter().any(|f| f == "Drafts")));
            });
            out.unwrap()
        };
        let tabs = look(&mut rt).0;
        let press = |rt: &mut Runtime<crate::app::App>, key: &str| {
            let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
        };
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..20 {
                rt.frame(DT);
            }
            rt.update(|s, _| s.writer.wait(std::time::Duration::from_secs(5)));
        };

        rt.update(|s, _| s.delete_note());
        settle(&mut rt);
        assert!(!root.join("Drafts/b.md").exists() && root.join(".trash/Drafts - b.md").is_file());
        assert!(rt.find("toast-undo").is_some(), "the toast offers Undo");
        // UT: the button shows its key, so no tooltip repeats it.
        let undo = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toast-undo")).unwrap();
        assert_eq!(undo.help, None, "the Undo button has no tooltip");
        press(&mut rt, "toast-undo");
        settle(&mut rt);
        assert_eq!(std::fs::read_to_string(root.join("Drafts/b.md")).unwrap(), "# B edited\n", "back with its last text");
        assert!(!root.join(".trash/Drafts - b.md").exists());
        let (t, active, ..) = look(&mut rt);
        assert_eq!(t, tabs, "its tab is back in its place");
        assert_eq!(active.as_deref(), Some("Drafts/b.md"), "and shown");
        assert!(rt.find("toast-undo").is_none(), "the toast went");

        // A folder comes back with its notes and its open tab.
        rt.update(|s, _| s.delete_folder("Drafts"));
        settle(&mut rt);
        assert!(!root.join("Drafts").exists());
        assert!(matches!(look(&mut rt), (_, _, false, false)), "b and Drafts left the vault");
        press(&mut rt, "toast-undo");
        settle(&mut rt);
        assert!(root.join("Drafts/b.md").is_file());
        assert_eq!(look(&mut rt), (tabs.clone(), Some("Drafts/b.md".into()), true, true), "both back, b open and shown");

        // The key the toast names does what its button does, even while the editor has the
        // keyboard, and a long name never pushes the button out of a small window.
        rt.update(|s, _| s.delete_folder("Drafts"));
        settle(&mut rt);
        rt.on_key = Some(crate::keys::on_key);
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            s.focus_editor(tx);
        });
        rt.frame(DT);
        assert!(rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some("editor")), "the editor has the keyboard");
        let label = crate::keymap::default_label("app.undo-delete");
        assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some(label.as_str())), "the toast names {label}");
        rt.key(rune::KeyEvent { key: rune::Key::Command('r'), mods: rune::Mods { cmd: true, ctrl: true, shift: true, ..Default::default() } });
        settle(&mut rt);
        assert_eq!(look(&mut rt), (tabs.clone(), Some("Drafts/b.md".into()), true, true), "the key put Drafts back");

        // The offer ends after a while; the note stays in the trash.
        rt.update(|s, _| s.delete_path("a.md"));
        settle(&mut rt);
        assert!(rt.find("toast-undo").is_some());
        assert_eq!(look(&mut rt).1.as_deref(), Some("Drafts/b.md"), "deleting another note leaves the shown one");
        for _ in 0..(crate::folders::UNDO_FOR / DT) as usize {
            rt.frame(DT);
        }
        assert!(rt.find("toast-undo").is_none(), "the offer ended");
        assert!(root.join(".trash/a.md").is_file());

        // The offer belongs to its folder: opening another folder ends it, so Undo can never
        // put a note into a folder it did not come from.
        let other = dir.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("o.md"), "# O\n").unwrap();
        rt.update(|s, _| s.delete_path("c.md"));
        settle(&mut rt);
        assert!(rt.find("toast-undo").is_some());
        rt.update(|s, tx| s.open_vault(other.clone(), tx));
        settle(&mut rt);
        assert!(rt.find("toast-undo").is_none(), "the offer ended with its folder");
        rt.update(|s, tx| s.undo_delete(tx));
        settle(&mut rt);
        rt.update(|s, _| assert!(!s.files().contains_key("c.md") && s.notice.is_none(), "nothing came into the other folder"));
        assert!(!other.join("c.md").exists() && root.join(".trash/c.md").is_file(), "c waits in its own folder's trash");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Settings works from the keyboard: Tab reaches each control in reading order, Space and
    /// Return press one, the arrows move through a group's choices, and Esc still closes it.
    #[test]
    fn settings_work_from_the_keyboard() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-setkeys-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        rt.update(|s, tx| {
            s.open_settings(tx);
            s.set_sec = crate::app::Sec::Editor;
        });
        let key = |rt: &mut Runtime<crate::app::App>, k: Key, shift: bool| {
            rt.key(KeyEvent { key: k, mods: Mods { shift, ..Mods::default() } });
            for _ in 0..4 {
                rt.frame(DT);
            }
        };
        let focused = |rt: &Runtime<crate::app::App>| rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
        let read = |rt: &mut Runtime<crate::app::App>| {
            let mut out = None;
            rt.update(|s, _| out = Some((s.layer_is(crate::layer::Kind::Settings), s.settings().toolbar, s.settings().code_font.clone(), s.settings().size)));
            out.unwrap()
        };
        for _ in 0..10 {
            rt.frame(DT);
        }
        assert_eq!(focused(&rt).as_deref(), Some("nav-Editor"), "the open section's tab has the keyboard");
        let mut seen = Vec::new();
        for _ in 0..7 {
            key(&mut rt, Key::Tab, false);
            seen.push(focused(&rt).unwrap_or_default());
        }
        let font = seen[1].clone();
        assert!(font.starts_with("font-"), "{seen:?}");
        assert_eq!(seen[0], "set-close");
        assert_eq!(&seen[2..], ["code-seg-0", "size-down", "size-up", "width-seg-0", "toolbar-switch"], "one stop per control, chosen ones for groups");
        let (_, toolbar, _, _) = read(&mut rt);
        key(&mut rt, Key::Text(" ".into()), false);
        assert_eq!(read(&mut rt).1, !toolbar, "Space flips the switch");
        key(&mut rt, Key::Enter, false);
        assert_eq!(read(&mut rt).1, toolbar, "and Return flips it back");
        // Back to the stepper: Return makes the text larger.
        for _ in 0..2 {
            key(&mut rt, Key::Tab, true);
        }
        assert_eq!(focused(&rt).as_deref(), Some("size-up"), "Shift+Tab goes back");
        let size = read(&mut rt).3;
        key(&mut rt, Key::Enter, false);
        assert_eq!(read(&mut rt).3, size + 1);
        // In the code font's segments, the arrow picks the next one.
        for _ in 0..2 {
            key(&mut rt, Key::Tab, true);
        }
        assert_eq!(focused(&rt).as_deref(), Some("code-seg-0"));
        let code = read(&mut rt).2;
        key(&mut rt, Key::Right, false);
        assert_eq!(focused(&rt).as_deref(), Some("code-seg-1"));
        assert_ne!(read(&mut rt).2, code, "the arrow chose it");
        key(&mut rt, Key::Escape, false);
        assert!(!read(&mut rt).0, "Esc closes Settings");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Below 680 points the sidebar button opens the sidebar as a drawer over the page, with
    /// New note and Settings at its foot. A press outside, Esc, the button or an opened note
    /// closes it, and the docked sidebar's setting stays as it was. From 680 up the button
    /// docks and undocks the sidebar as before.
    #[test]
    fn the_sidebar_opens_over_a_narrow_page() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-sidedrawer-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nText.\n"), ("other.md", "# O\n")], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let rect = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| e.rect);
        let tap = |rt: &mut Runtime<crate::app::App>, (x, y): (f32, f32)| {
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
        };
        let toggle = |rt: &mut Runtime<crate::app::App>| {
            let at = rt.find("toggle-left").expect("the sidebar button");
            tap(rt, at);
        };
        let docked = |rt: &mut Runtime<crate::app::App>| {
            let mut on = false;
            rt.update_if(|s, _| {
                on = s.persist.left_open;
                false
            });
            on
        };
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        let setting = docked(&mut rt);
        assert!(rect(&rt, "sidebar").is_none(), "no sidebar at first");
        let page = rect(&rt, "doc-area").unwrap();
        toggle(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_some(), "the button opens the drawer");
        for key in ["new-note", "foot-tags", "foot-settings"] {
            let r = rect(&rt, key).unwrap_or_else(|| panic!("{key} shows"));
            assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= 640.0 && r[1] + r[3] <= 480.0, "{key} inside the window: {r:?}");
        }
        assert_eq!(rect(&rt, "doc-area").unwrap(), page, "the page does not reflow under it");
        assert!(rect(&rt, "toggle-left").is_some(), "its button stays over it");
        toggle(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none(), "the button closes it");
        toggle(&mut rt);
        settle(&mut rt);
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none(), "Esc closes it");
        toggle(&mut rt);
        settle(&mut rt);
        tap(&mut rt, (500.0, 300.0));
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none(), "a press outside closes it");
        toggle(&mut rt);
        settle(&mut rt);
        let at = rt.find("file:other.md").expect("the other note in the drawer");
        tap(&mut rt, at);
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none(), "an opened note closes it");
        // Both drawers, one rule: a press on a title bar button closes the open drawer and does
        // what the button does.
        toggle(&mut rt);
        settle(&mut rt);
        let at = rt.find("toggle-right").expect("the outline button");
        tap(&mut rt, at);
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none() && rect(&rt, "outline-over").is_some(), "the outline button swaps the sidebar for the outline");
        toggle(&mut rt);
        settle(&mut rt);
        assert!(rect(&rt, "outline-over").is_none() && rect(&rt, "sidebar-over").is_some(), "and the sidebar button swaps them back");
        let mode = |rt: &mut Runtime<crate::app::App>| {
            let mut m = crate::app::Mode::Source;
            rt.update_if(|s, _| {
                m = s.mode;
                false
            });
            m
        };
        let was = mode(&mut rt);
        let other = if was == crate::app::Mode::Source { "mode-preview" } else { "mode-source" };
        let at = rt.find(other).expect("the other mode's button");
        tap(&mut rt, at);
        settle(&mut rt);
        assert!(rect(&rt, "sidebar-over").is_none() && mode(&mut rt) != was, "another button closes the drawer and acts");
        // A layer over the page closes the drawer, so it never shows through or over it.
        toggle(&mut rt);
        settle(&mut rt);
        let at = rt.find("foot-settings").expect("Settings at the drawer's foot");
        tap(&mut rt, at);
        settle(&mut rt);
        assert!(rect(&rt, "settings-frame").is_some() && rect(&rt, "sidebar-over").is_none(), "Settings closes the sidebar's drawer");
        rt.update(|s, tx| s.close_layer(tx));
        settle(&mut rt);
        let at = rt.find("toggle-right").expect("the outline button");
        tap(&mut rt, at);
        settle(&mut rt);
        assert!(rect(&rt, "outline-over").is_some());
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Commands, tx));
        settle(&mut rt);
        assert!(rect(&rt, "palette-frame").is_some() && rect(&rt, "outline-over").is_none(), "the palette closes the outline's drawer");
        rt.update(|s, tx| s.close_layer(tx));
        assert_eq!(docked(&mut rt), setting, "the docked setting never changed");
        // From 680 up, nothing changed: the button docks and undocks it.
        rt.set_viewport((1000.0, 600.0), 1.0);
        settle(&mut rt);
        toggle(&mut rt);
        settle(&mut rt);
        assert_eq!(docked(&mut rt), !setting);
        assert!(rect(&rt, "sidebar-over").is_none(), "no drawer");
        assert_eq!(rect(&rt, "sidebar").is_some(), !setting);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An open drawer holds the keyboard and the wheel: the keyboard moves into it, Tab stays in
    /// it, typing and scrolling never reach the note behind, and as it closes the keyboard goes
    /// back to the note. Its title bar stays live: the mode button closes it and acts.
    #[test]
    fn a_drawer_holds_the_keyboard_and_the_scroll() {
        use crate::bench::DT;
        use rune::{Key, KeyEvent, Mods};
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-drawerkeys-{}", std::process::id()));
        let long = format!("# A\n\n## B\n\n{}", "A line of text.\n\n".repeat(80));
        let mut rt = vault(&dir, &[("note.md", long.as_str())], crate::watch::EVERY);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_full_keyboard_access(true);
        let settle = |rt: &mut Runtime<crate::app::App>| {
            for _ in 0..40 {
                rt.frame(DT);
            }
        };
        let key = |rt: &mut Runtime<crate::app::App>, k: Key| {
            rt.key(KeyEvent { key: k, mods: Mods::default() });
            settle(rt);
        };
        let focus = |rt: &Runtime<crate::app::App>| rt.elements().into_iter().find(|e| e.focused).map(|e| e.path);
        let shown = |rt: &Runtime<crate::app::App>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).map(|e| (e.rect, e.path));
        let text = |rt: &mut Runtime<crate::app::App>| {
            let mut t = String::new();
            rt.update_if(|s, _| {
                t = s.cur_edit().text.clone();
                false
            });
            t
        };
        rt.update(|s, _| s.mode = crate::app::Mode::Source);
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        let editor = focus(&rt).expect("the note has the keyboard");
        for (button, drawer) in [("toggle-left", "sidebar-over"), ("toggle-right", "outline-over")] {
            let at = rt.find(button).expect("the drawer's button");
            rt.pointer_move(at.0, at.1);
            rt.pointer_down(at.0, at.1);
            rt.pointer_up(at.0, at.1);
            settle(&mut rt);
            let (r, path) = shown(&rt, drawer).unwrap_or_else(|| panic!("{drawer} opens"));
            let inside = |f: Option<String>| f.is_some_and(|f| f.starts_with(&path));
            assert!(inside(focus(&rt)), "{drawer}: the keyboard moves into it, not {:?}", focus(&rt));
            for _ in 0..6 {
                key(&mut rt, Key::Tab);
                assert!(inside(focus(&rt)), "{drawer}: Tab stays in it, not {:?}", focus(&rt));
            }
            let before = text(&mut rt);
            key(&mut rt, Key::Text("Q".into()));
            assert_eq!(text(&mut rt), before, "{drawer}: typing never reaches the note");
            let page = shown(&rt, "editor").unwrap().0;
            let x = if r[0] > 100.0 { 150.0 } else { 500.0 };
            rt.wheel(x, 300.0, -600.0);
            settle(&mut rt);
            assert_eq!(shown(&rt, "editor").unwrap().0, page, "{drawer}: a wheel over the scrim scrolls nothing");
            key(&mut rt, Key::Escape);
            assert!(shown(&rt, drawer).is_none(), "{drawer}: Esc closes it");
            assert_eq!(focus(&rt).as_deref(), Some(editor.as_str()), "{drawer}: the keyboard goes back to the note");
        }
        let at = rt.find("toggle-left").expect("the sidebar button");
        rt.pointer_move(at.0, at.1);
        rt.pointer_down(at.0, at.1);
        rt.pointer_up(at.0, at.1);
        settle(&mut rt);
        let at = rt.find("mode-preview").expect("the Preview button");
        rt.pointer_move(at.0, at.1);
        rt.pointer_down(at.0, at.1);
        rt.pointer_up(at.0, at.1);
        settle(&mut rt);
        let mut mode = crate::app::Mode::Source;
        rt.update_if(|s, _| {
            mode = s.mode;
            false
        });
        assert!(shown(&rt, "sidebar-over").is_none() && mode == crate::app::Mode::Preview, "one press on the mode button closes the drawer and acts");
        // A delete made in the drawer closes it, so the Undo toast shows on the page and works.
        let at = rt.find("toggle-left").expect("the sidebar button");
        rt.pointer_move(at.0, at.1);
        rt.pointer_down(at.0, at.1);
        rt.pointer_up(at.0, at.1);
        settle(&mut rt);
        rt.update(|s, _| s.delete_note());
        settle(&mut rt);
        rt.update(|s, _| s.writer.wait(std::time::Duration::from_secs(5)));
        assert!(shown(&rt, "sidebar-over").is_none(), "the delete closed the drawer");
        let at = rt.find("toast-undo").expect("the toast offers Undo");
        rt.pointer_move(at.0, at.1);
        rt.pointer_down(at.0, at.1);
        rt.pointer_up(at.0, at.1);
        settle(&mut rt);
        rt.update(|s, _| s.writer.wait(std::time::Duration::from_secs(5)));
        assert!(dir.join("notes/note.md").is_file(), "Undo brought the note back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-223: entering zen mode from its button, the floating pill shows where it stays while
    /// the sidebar goes and the page widens.
    #[test]
    fn wr_223_the_zen_pill_stays_put() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-zen-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n\nSome text.\n")], crate::watch::EVERY);
        rt.update(|s, _| s.mode = crate::app::Mode::Source);
        for _ in 0..40 {
            rt.frame(DT);
        }
        let (x, y) = rt.find("focus").expect("the zen button");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        let mut seen = Vec::new();
        for _ in 0..30 {
            rt.frame(DT);
            seen.extend(rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("zen-actions") && !e.exiting).map(|e| e.rect[0]));
        }
        assert!(!seen.is_empty(), "the pill shows");
        assert!(seen.iter().all(|&x| x == seen[0]), "and never moves: {seen:?}");
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
            rt.update(|s, tx| s.center_match(true, tx));
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
            assert!(s.lock_error().is_some_and(|e| e.contains("at least 8")), "a short password is refused");
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
            s.wait_lock(tx);
            assert!(s.layer().is_none());
            assert!(crate::lock::is_locked(&s.files()[&a]), "the folder copy is sealed");
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 4711.", "the note stays open");
            // An edit while open is sealed too.
            s.set_text(&a, "# Secret\n\nThe code is 1234.".into());
            assert!(!s.files()[&a].contains("1234"));
            s.relock();
            assert!(s.note_locked(&a) && s.active_text().is_empty());
            set(s, "unlock-pass", "wrong password");
            s.unlock(tx);
            s.wait_lock(tx);
            assert_eq!(s.unlock_error.as_deref(), Some("The password does not open this note."));
            set(s, "unlock-pass", "tiger lily");
            s.unlock(tx);
            s.wait_lock(tx);
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 1234.");
            s.remove_password(tx);
            assert_eq!(s.files()[&a], "# Secret\n\nThe code is 1234.");
            s.writer.wait(std::time::Duration::from_secs(5));
            let disk = std::fs::read_to_string(s.vault.as_ref().unwrap().path.join(&a)).unwrap();
            assert_eq!(disk, "# Secret\n\nThe code is 1234.");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn locked_note_gates_find_and_copy_all() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-lockact-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (520.0, 400.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        let set = |s: &mut crate::app::App, key: &str, v: &str| s.field(key, String::new).replace_text(v.to_string());
        rt.update(|s, tx| {
            s.ask_lock(tx);
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
            s.wait_lock(tx);
            s.open_find(tx);
            assert!(s.reader.find.open, "find opens while unlocked");
            s.relock();
            assert!(!s.reader.find.open, "relock closes open find bar");
            s.mode = crate::app::Mode::Preview;
            assert!(s.active_locked());
            s.toggle_find(tx);
            s.open_find(tx);
            s.writer_find(tx);
            s.copy_all(tx);
            assert!(!s.reader.find.open, "find stays closed while active note is locked");
            assert!(s.reader.toast.is_none(), "copy_all does not toast while active note is locked");
        });
        rt.frame(1.0 / 60.0);
        let find = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("find")).expect("find button");
        assert_eq!(find.fill.a, 0.0, "find button stays inactive while locked");
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

    /// KBDA11Y: titlebar and Find buttons are keyboard-focusable, `find-input` exposes an
    /// accessible label, and locked Preview `find`/`copy-all` controls do not expose active
    /// button roles.
    #[test]
    fn kbda11y_focusable_chrome_labelled_find_and_disabled_locked_buttons() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-kbda11y-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, "# Hello\n\nBody text.\n", crate::app::Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.update(|s, tx| {
            s.writer_find(tx);
            s.reader.find.q = rune::TextEdit::new("Hello");
            s.reader.find.q.focused = true;
            s.find_changed(tx);
        });
        for _ in 0..8 {
            rt.frame(1.0 / 60.0);
        }
        let els = rt.elements();
        for key in ["toggle-left", "crumb-file", "paragraph-focus", "typewriter", "next-font", "mode-source", "mode-preview", "mode-map", "focus", "toggle-right", "find-prev", "find-next", "find-close"] {
            let el = els.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("missing {key}"));
            assert!(el.focusable, "{key} must be keyboard-focusable");
        }
        let fi = els.iter().find(|e| e.key.as_deref() == Some("find-input") && !e.exiting).expect("find-input");
        assert_eq!(fi.label.as_deref(), Some("Find in note"), "find-input must have an accessible label");
        // Locked Preview note: find & copy-all must not advertise active button role.
        let sealed = format!("{} v1 pbkdf2-sha256 600000 AAAAAAAAAAAAAAAAAAAAAA==\n\nLocked.\n\nAAAA\n", crate::lock::MAGIC);
        let (mut rt_l, _) = crate::bench::open(&dir, &sealed, crate::app::Mode::Preview);
        for _ in 0..8 {
            rt_l.frame(1.0 / 60.0);
        }
        for key in ["find", "copy-all"] {
            let el = rt_l.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("missing locked {key}"));
            assert_ne!(el.role, "button", "locked {key} must not expose active button role");
            assert!(!el.focusable, "locked {key} must not be focusable");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FOCUSMODEL: unified focus ring & Tab routing matrix covering compound-bar ring scoping
    /// (`search-box`, `find-bar`), 0-match Find stepper Tab skips, empty-vault initial Tab to
    /// `first-note`, `mode` button click focus restoration (`note-keys` / `editor`), and 1-Esc
    /// Welcome Create dismissal.
    #[test]
    fn focus_model_matrix() {
        use rune::{Env, Key, KeyEvent, Mods, Runtime};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-focusmodel-{}", std::process::id()));
        let dt = 1.0 / 60.0;
        let step = |rt: &mut Runtime<crate::app::App>, n: usize| {
            for _ in 0..n {
                rt.frame(dt);
            }
        };
        let focused_key = |rt: &Runtime<crate::app::App>| {
            rt.elements().into_iter().find(|e| !e.exiting && e.focused).and_then(|e| e.key)
        };
        let tab = KeyEvent { key: Key::Tab, mods: Mods::default() };

        // 1 & 2. Compound bars (`search-box` & `find-bar`) scope input ring strictly to the text
        // input, and 0-match Find skips `find-prev` / `find-next` straight to `find-close`.
        let (mut rt, _) = crate::bench::open(&dir, "# Hello\n\nBody text with hello.\n", crate::app::Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.update(|s, tx| {
            s.persist.left_open = true;
            s.open_search(tx);
        });
        step(&mut rt, 6);
        rt.key(KeyEvent { key: Key::Text("hello".into()), mods: Mods::default() });
        step(&mut rt, 6);
        let mut q_foc = false;
        rt.update_if(|s, _| {
            q_foc = s.query.focused;
            false
        });
        assert!(q_foc, "search input reports focused before Tab");
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("search-clear"), "Tab moves from search to search-clear");
        rt.update_if(|s, _| {
            q_foc = s.query.focused;
            false
        });
        assert!(!q_foc, "search-box drops input focus ring while search-clear has focus");

        // Find with matches: Tab moves through find-case, find-word to find-prev and drops find-bar's input ring.
        rt.update(|s, tx| {
            s.writer_find(tx);
            s.reader.find.q = rune::TextEdit::new("hello");
            s.reader.find.q.focused = true;
            s.find_changed(tx);
            tx.focus("find-input", false);
        });
        step(&mut rt, 6);
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-case"), "Tab moves from find-input to find-case");
        let mut f_foc = true;
        rt.update_if(|s, _| {
            f_foc = s.reader.find.q.focused;
            false
        });
        assert!(!f_foc, "find-bar drops input focus ring while find-case has focus");
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-word"), "Tab moves from find-case to find-word");
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-prev"), "Tab moves from find-word to find-prev when matches exist");

        // Find with 0 matches: find-prev & find-next are non-focusable, Tab skips them to find-close.
        rt.update(|s, tx| {
            s.reader.find.q = rune::TextEdit::new("zzz_no_match_999");
            s.reader.find.q.focused = true;
            s.find_changed(tx);
            tx.focus("find-input", false);
        });
        step(&mut rt, 6);
        let els = rt.elements();
        for k in ["find-prev", "find-next"] {
            let e = els.iter().find(|e| !e.exiting && e.key.as_deref() == Some(k)).unwrap_or_else(|| panic!("{k}"));
            assert!(!e.focusable, "{k} must not be focusable on 0 matches");
        }
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-case"));
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-word"));
        rt.key(tab.clone());
        step(&mut rt, 6);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-close"), "0-match Tab skips disabled prev/next to find-close");

        // 4. Clicking `mode` into Preview restores focus to `note-keys`, and back to Source -> `editor`.
        rt.update(|s, tx| s.close_find(tx));
        step(&mut rt, 6);
        let (mx, my) = rt.find("mode-preview").expect("the Preview button");
        rt.pointer_move(mx, my);
        rt.pointer_down(mx, my);
        rt.pointer_up(mx, my);
        step(&mut rt, 8);
        assert_eq!(focused_key(&rt).as_deref(), Some(crate::focus::NOTE), "clicking mode into Preview focuses note-keys");
        let (mx, my) = rt.find("mode-source").expect("the Source button in preview");
        rt.pointer_move(mx, my);
        rt.pointer_down(mx, my);
        rt.pointer_up(mx, my);
        step(&mut rt, 8);
        assert_eq!(focused_key(&rt).as_deref(), Some("editor"), "clicking mode back to Source focuses editor");

        // 3. Empty vault initial Tab reaches `first-note` within <= 5 presses.
        let empty = dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let mut rt_e = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt_e.on_key = Some(crate::keys::on_key);
        rt_e.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        step(&mut rt_e, 3);
        rt_e.update(move |s, tx| {
            s.open_vault(empty, tx);
            s.persist.left_open = true;
        });
        step(&mut rt_e, 6);
        let mut reached = false;
        for _ in 0..5 {
            rt_e.key(tab.clone());
            step(&mut rt_e, 3);
            if focused_key(&rt_e).as_deref() == Some("first-note") {
                reached = true;
                break;
            }
        }
        assert!(reached, "empty-vault Tab must reach first-note within <= 5 presses");

        // 5. Welcome Create form dismisses in 1 Esc press and restores focus to `create`.
        let mut rt_w = Runtime::new(crate::app::init(), crate::ui::view, (520.0, 400.0), Env { system_dark: false, scale: 1.0 });
        rt_w.on_key = Some(crate::keys::on_key);
        rt_w.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        rt_w.update(|s, tx| s.go_welcome(tx));
        step(&mut rt_w, 6);
        let (cx, cy) = rt_w.find("create").expect("create button");
        rt_w.pointer_move(cx, cy);
        rt_w.pointer_down(cx, cy);
        rt_w.pointer_up(cx, cy);
        step(&mut rt_w, 6);
        assert!(rt_w.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some("new-vault")), "create form opened");
        rt_w.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        step(&mut rt_w, 6);
        assert!(rt_w.elements().iter().all(|e| e.exiting || e.key.as_deref() != Some("new-vault")), "1 Esc dismissed Welcome Create");
        assert_eq!(focused_key(&rt_w).as_deref(), Some("create"), "1 Esc restored focus to create");

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

    /// RAD: the formatting toolbar and the find bar, on one row, take one corner and one edge
    /// (`widgets::bar_edge`): the toolbar's backdrop, shown while the note scrolls under it,
    /// has the find bar's radius.
    #[test]
    fn the_toolbar_and_the_find_bar_look_alike() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-bar-edge-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, tx| s.toggle_find(tx));
        rt.update(|s, tx| s.scroll_by(400.0, tx));
        for _ in 0..40 {
            rt.frame(DT);
        }
        let back = info(&rt, "toolbar-back").expect("the toolbar's backdrop");
        let find = info(&rt, "find-bar").expect("the find bar");
        assert!(back.opacity > 0.9, "the backdrop shows while the note is under the toolbar: {}", back.opacity);
        assert_eq!(back.radius, crate::ui::widgets::BAR_RADIUS, "the toolbar's corner");
        assert_eq!(find.radius, crate::ui::widgets::BAR_RADIUS, "the find bar's corner");
        assert!((back.rect[3] - find.rect[3]).abs() < 0.5, "one height: {:?} and {:?}", back.rect, find.rect);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// C4: the formatting toolbar's first icon starts on the text column's left edge, at every
    /// window size and both line widths.
    #[test]
    fn the_toolbar_stands_on_the_text_column() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toolbar-x-{}", std::process::id()));
        let (mut rt, _) = open(&dir, "A plain line.\n\nAnother one.\n", crate::app::Mode::Source);
        rt.update(|s, tx| {
            s.cur_edit().set_selection(0, 0);
            s.focus_editor(tx);
        });
        for wide in [false, true] {
            for (w, h) in [(1280.0, 800.0), (1000.0, 700.0), (1600.0, 1000.0), (800.0, 600.0)] {
                rt.set_viewport((w, h), 1.0);
                rt.update(move |s, _| s.reader.wide = Some(wide));
                // The caret eases to its new place, so the frames run until it lands.
                for _ in 0..120 {
                    rt.frame(DT);
                }
                // The column's left edge is where a plain line starts: the caret at the note's start,
                // drawn on whole pixels.
                let caret = info(&rt, "editor").and_then(|e| e.caret).expect("the editor shows a caret");
                let icon = rt.elements().into_iter().find(|e| e.path.split('/').any(|seg| seg == "fmt-bold") && e.role == "icon").expect("the toolbar shows its bold button").rect;
                let col = caret[0];
                assert!((icon[0] - col).abs() < 1.0, "{w}x{h}, wide {wide}: the first icon at {}, the column at {col}", icon[0]);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-47, WR-100: the outline marks the current section by its row's shade and ink alone,
    /// one row only, no dot, and in the reader the shade moves with the scroll.
    #[test]
    fn the_current_section_is_shaded_and_has_no_dot() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-outline-mark-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Preview);
        rt.update(|s, _| s.persist.right_open = true);
        for _ in 0..30 {
            rt.frame(DT);
        }
        // The shaded heading rows, by their place, and what else a row holds.
        let shaded = |rt: &rune::Runtime<crate::app::App>| {
            let all = rt.elements();
            let rows: Vec<usize> = (0..all.len()).filter(|&i| all[i].key.as_deref().is_some_and(|k| k.starts_with("ol-"))).collect();
            assert!(rows.len() > 3, "the outline lists the headings");
            for &r in &rows {
                let kids: Vec<_> = all.iter().filter(|e| e.parent == Some(r)).collect();
                assert!(kids.len() == 1 && kids[0].text.is_some(), "a heading row holds its text and nothing else: {:?}", kids.iter().map(|k| (&k.key, k.rect)).collect::<Vec<_>>());
            }
            assert!(all.iter().all(|e| e.key.as_deref() != Some("dot")), "no dot anywhere");
            rows.iter().filter(|&&r| all[r].fill.a > 0.02).map(|&r| all[r].rect[1]).collect::<Vec<_>>()
        };
        let first = shaded(&rt);
        assert_eq!(first.len(), 1, "one row is shaded: {first:?}");
        let last = crate::md::headings(LONG).iter().filter(|h| h.level <= 4).last().unwrap().line;
        rt.update(move |s, tx| s.jump(last, tx));
        for _ in 0..90 {
            rt.frame(DT);
        }
        let later = shaded(&rt);
        assert_eq!(later.len(), 1, "still one shaded row: {later:?}");
        assert!(later[0] > first[0], "the shade moved down the outline with the page: {first:?} to {later:?}");
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
        // The counts left-align at the outline's 16 pt inset, on one line, and end where `below` begins.
        let counts_end_at = |rt: &rune::Runtime<crate::app::App>, below: f32| {
            let outline = rt.presented("outline").unwrap().0;
            let counts = rt.presented("counts").expect("the counts show").0;
            assert!(inside(counts, outline), "the counts {counts:?} sit in the outline {outline:?}");
            assert!((counts[1] + counts[3] - below).abs() < 1.0, "the counts {counts:?} end at {below}");
            let (wc, rd) = (info(rt, "word-count").unwrap().rect, info(rt, "read-time").unwrap().rect);
            assert!((wc[1] - rd[1]).abs() < 0.5, "the count and the reading time share a line");
            let inset = wc[0] - counts[0];
            assert!((inset - 16.0).abs() <= 2.0, "left-aligned at 16 pt inset: {wc:?}, counts {counts:?}");
            let bar = rt.presented("titlebar").unwrap().0;
            assert!(!inside(wc, bar));
        };
        let outline = rt.presented("outline").unwrap().0;
        let foot = rt.presented("status").expect("the writer's footer shows").0;
        let side_foot = rt.presented("side-foot").expect("the sidebar footer shows").0;
        assert!(inside(foot, outline) && (foot[1] + foot[3] - (outline[1] + outline[3])).abs() < 1.0, "the footer {foot:?} sits at the outline's foot");
        assert!((foot[3] - 41.0).abs() < 0.5, "the footer matches side-foot height: {}", foot[3]);
        assert!((foot[1] - side_foot[1]).abs() <= 1.0 && (foot[3] - side_foot[3]).abs() <= 1.0, "status {foot:?} aligns with side-foot {side_foot:?}");
        let caret_pos = info(&rt, "caret-pos").expect("the caret's position shows").rect;
        assert!(inside(caret_pos, foot));
        let wc = info(&rt, "word-count").unwrap().rect;
        assert!((wc[0] - caret_pos[0]).abs() <= 1.0, "word-count {wc:?} aligns horizontally with caret-pos {caret_pos:?}");
        counts_end_at(&rt, foot[1]);
        rt.update(|s, tx| s.show_mode(crate::app::Mode::Preview, tx));
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(rt.presented("status").is_none(), "no footer in preview");
        assert!(info(&rt, "caret-pos").is_none());
        let outline = rt.presented("outline").unwrap().0;
        counts_end_at(&rt, outline[1] + outline[3]);
        let counts = rt.presented("counts").expect("the counts show").0;
        let side_foot = rt.presented("side-foot").expect("the sidebar footer shows").0;
        assert!((counts[3] - 41.0).abs() < 0.5 && (counts[1] - side_foot[1]).abs() <= 1.0, "preview counts {counts:?} aligns with side-foot {side_foot:?}");
        assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("preview")), "nothing says preview");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// TARGETS24: preview task checkboxes are >= 24×24 pt at default 14 pt body size, the code
    /// block's Copy chip is >= 24 pt tall, and both the chip and language badge scale with zoom.
    #[test]
    fn targets24_preview_targets_and_zoom_scaling() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-targets24-{}", std::process::id()));
        let md = "# Check\n\n```rust\nfn main() {}\n```\n\n- [ ] Open task\n- [x] Done task\n";
        let (mut rt, _) = open(&dir, md, crate::app::Mode::Preview);
        rt.set_viewport((1280.0, 800.0), 1.0);
        for _ in 0..8 {
            rt.frame(DT);
        }
        let markers: Vec<[f32; 4]> = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("marker") && e.rect[2] > 10.0).map(|e| e.rect).collect();
        assert_eq!(markers.len(), 2, "two task checkboxes laid out: {markers:?}");
        for m in &markers {
            assert!(m[2] >= 24.0 && m[3] >= 24.0, "task checkbox target >= 24×24 pt: {m:?}");
        }
        let copy0 = info(&rt, "copy").expect("copy chip").rect;
        let badge0 = info(&rt, "badge").expect("language badge").rect;
        assert!(copy0[3] >= 24.0, "default Copy chip height >= 24 pt: {copy0:?}");
        rt.update(|s, tx| {
            for _ in 0..3 {
                s.zoom(Some(true), tx);
            }
        });
        for _ in 0..8 {
            rt.frame(DT);
        }
        let copy1 = info(&rt, "copy").expect("zoomed copy chip").rect;
        let badge1 = info(&rt, "badge").expect("zoomed language badge").rect;
        assert!(copy1[3] > copy0[3] + 2.0, "Copy chip scales with zoom: {copy0:?} -> {copy1:?}");
        assert!(badge1[2] > badge0[2] + 3.0, "language badge scales with zoom: {badge0:?} -> {badge1:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// TASKOVERLAP: after 3 zoom-out steps, adjacent preview task hit targets never overlap
    /// vertically, and clicking the lower part of task 1 toggles only task 1.
    #[test]
    fn taskoverlap_zoomed_out_checkboxes_never_overlap() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-taskoverlap-{}", std::process::id()));
        let md = "# Tasks\n\n- [ ] First task\n- [ ] Second task\n";
        let (mut rt, _) = open(&dir, md, crate::app::Mode::Preview);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| {
            for _ in 0..3 {
                s.zoom(Some(false), tx);
            }
        });
        for _ in 0..8 {
            rt.frame(DT);
        }
        let marks: Vec<[f32; 4]> = rt.elements().into_iter().filter(|e| e.role == "checkbox").map(|e| e.rect).collect();
        assert_eq!(marks.len(), 2, "two zoomed-out task checkboxes: {marks:?}");
        let (r0, r1) = (marks[0], marks[1]);
        assert!(r0[2] >= 24.0 && r1[2] >= 24.0, "hit width stays >= 24 pt: {r0:?}, {r1:?}");
        let overlap = (r0[1] + r0[3]) - r1[1];
        assert!(overlap <= 0.01, "adjacent zoomed-out task checkboxes must not overlap: overlap={overlap:.2}pt, r0={r0:?}, r1={r1:?}");
        let (cx, cy) = (r0[0] + r0[2] / 2.0, r0[1] + r0[3] - 2.0);
        rt.pointer_move(cx, cy);
        rt.pointer_down(cx, cy);
        rt.pointer_up(cx, cy);
        for _ in 0..6 {
            rt.frame(DT);
        }
        let mut text = String::new();
        rt.update_if(|s, _| {
            text = s.active_text().to_string();
            false
        });
        assert!(text.contains("- [x] First task"), "task 1 toggled: {text:?}");
        assert!(text.contains("- [ ] Second task"), "task 2 stayed unchecked: {text:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CODECLEAR: code block top clearance keeps the Copy chip off long first lines at 1x and 3x
    /// zoom-in, and the language badge meets >= 4.5:1 contrast against the code panel.
    #[test]
    fn codeclear_copy_chip_clears_first_line_and_badge_reads() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-codeclear-{}", std::process::id()));
        let md = "# Code\n\n```rust\nfn very_long_first_line_of_code_that_reaches_all_the_way_to_the_right_edge_of_the_block() -> usize { 42 }\nlet second = 2;\n```\n";
        let (mut rt, _) = open(&dir, md, crate::app::Mode::Preview);
        rt.set_viewport((1280.0, 800.0), 1.0);
        for _ in 0..8 {
            rt.frame(DT);
        }
        let copy1 = info(&rt, "copy").expect("copy chip at 1x").rect;
        let code1 = info(&rt, "code").expect("code at 1x").rect;
        assert!(copy1[1] + copy1[3] <= code1[1] + 0.01, "1x Copy chip {copy1:?} must clear line 1 {code1:?}");
        let badge = info(&rt, "badge").expect("language badge");
        let fg = badge.text_color.expect("badge color");
        let mut panel = rune::Color::BLACK;
        rt.update_if(|s, _| {
            panel = crate::roles::Roles::new(s.palette(false)).panel;
            false
        });
        let cr = crate::theme::contrast(fg, panel);
        assert!(cr >= 4.5, "badge contrast on Paper panel: {cr:.2}:1 (< 4.5)");
        rt.update(|s, tx| {
            for _ in 0..3 {
                s.zoom(Some(true), tx);
            }
        });
        for _ in 0..8 {
            rt.frame(DT);
        }
        let copy3 = info(&rt, "copy").expect("copy chip at 3x").rect;
        let code3 = info(&rt, "code").expect("code at 3x").rect;
        assert!(copy3[1] + copy3[3] <= code3[1] + 0.01, "3x Copy chip {copy3:?} must clear line 1 {code3:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SCROLLFADE: unresolved wikilinks meet >= 4.5:1 contrast on Paper and Ink, and Help,
    /// Settings Shortcuts, and Command Palette share unified `kbd` caps.
    #[test]
    fn scrollfade_unresolved_wikilink_and_unified_kbd() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-scrollfade-{}", std::process::id()));
        let md = "# Wiki\n\nLink to [[Missing target]] right here.\n\n";
        let (mut rt, _) = open(&dir, md, crate::app::Mode::Preview);
        rt.set_viewport((1280.0, 800.0), 1.0);
        for _ in 0..8 {
            rt.frame(DT);
        }
        let wl = info(&rt, "wiki:Missing target").expect("unresolved wikilink");
        assert!(wl.rect[2] > 20.0, "unresolved wikilink laid out: {:?}", wl.rect);
        for (id, dark) in [("paper", false), ("ink", true), ("solarized-light", false), ("nord", true)] {
            let r = crate::roles::Roles::new(crate::theme::theme_pal(id, dark));
            let cr = crate::theme::contrast(r.readable_dim(), r.bg);
            assert!(cr >= 4.5, "{id}: unresolved wikilink contrast {cr:.2}:1 (< 4.5)");
        }
        // Command palette uses shared `kbd` chips.
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Commands, tx));
        for _ in 0..8 {
            rt.frame(DT);
        }
        let pal_kbds = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("kbd") && e.path.contains("/palette/")).count();
        assert!(pal_kbds >= 4, "command palette renders shared kbd caps: {pal_kbds}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PREVFADE: Preview mode renders the matching subtle scroll thumb on overflowing notes
    /// (showing while scrolling and fading after 700 ms) and deepens doc-scroll's top fade to
    /// 40 pt outside zen mode without adding zen-only fade bands.
    #[test]
    fn prevfade_preview_scroll_fade_and_thumb_parity() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-prevfade-{}", std::process::id()));
        let md = format!("# Top\n\n{}", "Dense line of readable prose for Preview fade and thumb parity.\n\n".repeat(45));
        let (mut rt, _) = open(&dir, &md, crate::app::Mode::Preview);
        rt.set_viewport((1280.0, 800.0), 1.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(info(&rt, "fade-top").is_none() && info(&rt, "fade-foot").is_none(), "no zen fade bands outside zen mode");
        let shown = |rt: &rune::Runtime<crate::app::App>| rt.presented("thumb").map_or(0.0, |p| p.1);
        assert!(shown(&rt) < 0.01, "preview thumb rests hidden: {}", shown(&rt));
        let (x, y) = rt.find("doc-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -360.0);
        for _ in 0..20 {
            rt.frame(DT);
        }
        assert!(shown(&rt) > 0.4, "preview thumb shows while scrolling: {}", shown(&rt));
        let thumb = rt.presented("thumb").unwrap().0;
        assert!((thumb[2] - 3.0).abs() < 0.1 && thumb[3] >= 24.0, "preview thumb geometry matches writer: {thumb:?}");
        for _ in 0..90 {
            rt.frame(DT);
        }
        assert!(shown(&rt) < 0.05, "preview thumb fades after 700 ms: {}", shown(&rt));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THUMBFIX: scroll thumb shares `doc-scroll`'s coordinate origin under open Find (top &
    /// bottom aligned in both Preview and toolbar-hidden Source) and snaps immediately to 0.5
    /// for 700 ms then 0.0 under reduced motion.
    #[test]
    fn thumbfix_aligns_under_find_and_shows_under_reduced_motion() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-thumbfix-{}", std::process::id()));
        let md = format!("# Scroll Thumb\n\n{}", "Paragraph of steady reading text to test scroll thumb placement.\n\n".repeat(45));
        let shown = |rt: &rune::Runtime<crate::app::App>| rt.presented("thumb").map_or(0.0, |p| p.1);

        // 1. Open Find alignment in Preview and toolbar-hidden Source at narrow & wide widths.
        for (mode, size) in [(crate::app::Mode::Preview, (760.0, 480.0)), (crate::app::Mode::Source, (520.0, 400.0))] {
            let (mut rt, _) = open(&dir, &md, mode);
            rt.set_viewport(size, 1.0);
            rt.update(|s, tx| {
                s.persist.settings.toolbar = false;
                s.open_find(tx);
            });
            for _ in 0..20 {
                rt.frame(DT);
            }
            let (x, y) = rt.find("doc-scroll").unwrap();
            rt.pointer_move(x, y);
            rt.wheel(x, y, -20.0);
            for _ in 0..12 {
                rt.frame(DT);
            }
            let ds = rt.presented("doc-scroll").unwrap().0;
            let th_top = rt.presented("thumb").unwrap().0;
            assert!(th_top[3] > 0.0 && th_top[1] >= ds[1] - 0.5, "{mode:?} {size:?}: thumb top {} above doc-scroll top {}", th_top[1], ds[1]);

            rt.wheel(x, y, -9000.0);
            for _ in 0..18 {
                rt.frame(DT);
            }
            let ds = rt.presented("doc-scroll").unwrap().0;
            let th_bot = rt.presented("thumb").unwrap().0;
            let gap = (ds[1] + ds[3]) - (th_bot[1] + th_bot[3]);
            assert!(th_bot[3] > 0.0 && gap.abs() <= 2.5, "{mode:?} {size:?}: thumb bottom gap {gap:.1}pt (thumb={th_bot:?}, ds={ds:?})");
        }

        // 2. Reduced motion snaps thumb immediately to 0.5, holds 700 ms, snaps back to 0.0.
        for mode in [crate::app::Mode::Preview, crate::app::Mode::Source] {
            let (mut rt, _) = open(&dir, &md, mode);
            rt.set_system_reduced_motion(true);
            rt.set_viewport((760.0, 480.0), 1.0);
            for _ in 0..20 {
                rt.frame(DT);
            }
            assert!(shown(&rt) < 0.01, "{mode:?}: reduced-motion thumb rests hidden: {}", shown(&rt));
            let (x, y) = rt.find("doc-scroll").unwrap();
            rt.pointer_move(x, y);
            rt.wheel(x, y, -240.0);
            for _ in 0..3 {
                rt.frame(DT);
            }
            assert!(shown(&rt) >= 0.49, "{mode:?}: reduced-motion thumb snaps to 0.5 at 50 ms: {}", shown(&rt));
            for _ in 0..13 {
                rt.frame(DT);
            }
            assert!(shown(&rt) >= 0.49, "{mode:?}: reduced-motion thumb holds 0.5 at 270 ms: {}", shown(&rt));
            for _ in 0..35 {
                rt.frame(DT);
            }
            assert!(shown(&rt) < 0.01, "{mode:?}: reduced-motion thumb snaps to 0.0 after 700 ms: {}", shown(&rt));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDSTATE: empty Find disables steppers (Tab -> find-close), pending Preview debounce
    /// keeps steppers enabled (fast Tab -> find-prev, flushing on step), Ctrl+F preserves scroll
    /// anchor when scrolled in Preview/toolbar-hidden Source (and keeps offset 0 unscrolled), and
    /// Preview offscreen toast-Find centers on the free-room midpoint (`RD-104`).
    #[test]
    fn findstate_pending_empty_anchor_and_toast_centering() {
        use rune::{Key, KeyEvent, Mods};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findstate-{}", std::process::id()));
        let mut md = String::from("# Anchor Heading\n\n");
        for i in 1..36 {
            md.push_str(&format!("Paragraph {i:02} has ordinary prose lines for scroll stability testing.\n\n"));
        }
        md.push_str("Special target needle sits well below the initial viewport.\n\n");
        for i in 36..55 {
            md.push_str(&format!("Trailing paragraph {i:02} keeps plenty of scroll room below.\n\n"));
        }
        let focused_key = |rt: &rune::Runtime<crate::app::App>| {
            rt.elements().into_iter().find(|e| !e.exiting && e.focused).and_then(|e| e.key)
        };
        let tab = KeyEvent { key: Key::Tab, mods: Mods::default() };

        // 1. Empty Find disables find-prev/find-next in both Preview and Source (Tab -> find-close).
        for mode in [crate::app::Mode::Preview, crate::app::Mode::Source] {
            let (mut rt, _) = open(&dir, &md, mode);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((520.0, 400.0), 1.0);
            rt.update(|s, tx| s.open_find(tx));
            for _ in 0..6 {
                rt.frame(DT);
            }
            let els = rt.elements();
            for k in ["find-prev", "find-next"] {
                let e = els.iter().find(|e| !e.exiting && e.key.as_deref() == Some(k)).unwrap();
                assert!(!e.focusable, "{mode:?}: empty Find {k} must be disabled/non-focusable");
            }
            rt.key(tab.clone());
            for _ in 0..6 {
                rt.frame(DT);
            }
            assert_eq!(focused_key(&rt).as_deref(), Some("find-close"), "{mode:?}: empty Find Tab jumps to find-close");
        }

        // 2. Fast Tab during Preview's 40 ms debounce lands on find-prev, never shows "No matches",
        //    and stepping while pending flushes immediately.
        let (mut rt, _) = open(&dir, &md, crate::app::Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((520.0, 400.0), 1.0);
        rt.update(|s, tx| s.open_find(tx));
        for _ in 0..6 {
            rt.frame(DT);
        }
        rt.key(KeyEvent { key: Key::Text("n".into()), mods: Mods::default() });
        rt.frame(DT); // ~16.7 ms < 40 ms debounce
        let fc_pending = rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("find-count")).and_then(|e| e.text).unwrap_or_default();
        assert!(fc_pending.is_empty(), "pending Preview query must not show premature No matches: {fc_pending:?}");
        rt.key(tab.clone());
        rt.frame(DT);
        assert_eq!(focused_key(&rt).as_deref(), Some("find-prev"), "fast Tab during debounce must land on find-prev");
        // Pressing Enter while still pending flushes search immediately and stays on find-prev.
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        for _ in 0..4 {
            rt.frame(DT);
        }
        let mut flushed = false;
        rt.update_if(|s, _| {
            flushed = s.reader.find.searched == "n" && s.reader.find.due.is_none();
            false
        });
        assert!(flushed, "stepping while pending flushes searched query immediately");

        // 3. Ctrl+F open & close while scrolled preserves visible paragraph screen y; offset 0 stays 0.
        let (mut rt, _) = open(&dir, &md, crate::app::Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        for _ in 0..15 {
            rt.frame(DT);
        }
        // Unscrolled: offset remains 0 when Find opens.
        rt.update(|s, tx| s.open_find(tx));
        for _ in 0..10 {
            rt.frame(DT);
        }
        let mut off0 = 999.0;
        rt.update_if(|s, _| {
            off0 = s.reader.scroll.get()[0];
            false
        });
        assert!(off0 <= 0.5, "unscrolled Ctrl+F keeps offset 0: {off0}");
        rt.update(|s, tx| s.close_find(tx));
        for _ in 0..10 {
            rt.frame(DT);
        }
        // Scrolled: opening and closing Find keeps visible paragraph stationary.
        let (x, y) = rt.find("doc-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -320.0);
        for _ in 0..25 {
            rt.frame(DT);
        }
        let p_before = rt
            .elements()
            .into_iter()
            .find(|e| !e.exiting && e.text.as_deref().is_some_and(|t| t.starts_with("Paragraph ")) && e.rect[1] >= 220.0 && e.rect[1] <= 500.0)
            .expect("visible paragraph before Ctrl+F");
        rt.update(|s, tx| s.open_find(tx));
        for _ in 0..12 {
            rt.frame(DT);
        }
        let p_open = rt.elements().into_iter().find(|e| !e.exiting && e.text == p_before.text).expect("paragraph after Ctrl+F");
        let dy_open = (p_open.rect[1] - p_before.rect[1]).abs();
        assert!(dy_open <= 2.0, "opening Ctrl+F shifted visible paragraph by {dy_open:.1}pt");
        rt.update(|s, tx| s.close_find(tx));
        for _ in 0..12 {
            rt.frame(DT);
        }
        let p_close = rt.elements().into_iter().find(|e| !e.exiting && e.text == p_before.text).expect("paragraph after close_find");
        let dy_close = (p_close.rect[1] - p_before.rect[1]).abs();
        assert!(dy_close <= 2.0, "closing Find shifted visible paragraph by {dy_close:.1}pt");

        // 4. Offscreen Preview Find with an active toast centers on the RD-104 free-room midpoint.
        let (mut rt, _) = open(&dir, &md, crate::app::Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((520.0, 400.0), 1.0);
        for _ in 0..15 {
            rt.frame(DT);
        }
        rt.update(|s, tx| {
            s.open_find(tx);
            s.copy_all(tx);
        });
        for _ in 0..10 {
            rt.frame(DT);
        }
        rt.update(|s, tx| {
            s.reader.find.q = rune::TextEdit::new("needle");
            s.find_changed(tx);
        });
        for _ in 0..25 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let ds = els.iter().find(|e| !e.exiting && e.key.as_deref() == Some("doc-scroll")).expect("doc-scroll");
        let tb = els.iter().find(|e| !e.exiting && e.key.as_deref() == Some("toast-band")).expect("toast-band");
        let hit = els.iter().find(|e| !e.exiting && e.text.as_deref().is_some_and(|t| t.contains("Special target needle"))).expect("needle match");
        let free_mid = (ds.rect[1] + tb.rect[1]) / 2.0;
        let hit_mid = hit.rect[1] + hit.rect[3] / 2.0;
        let err = (hit_mid - free_mid).abs();
        assert!(err <= 6.0, "toast+Find match mid {hit_mid:.1} vs free mid {free_mid:.1} (err={err:.1}pt)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WORDSEL: Source Select All excludes front matter from the outline's selected word count,
    /// and Preview task checkboxes are keyboard-focusable, labelled, Space/Enter-activatable,
    /// and pointer-clickable without leaving a stray focus ring.
    #[test]
    fn wordsel_front_matter_selection_and_keyboard_task_checkbox() {
        use rune::{Key, KeyEvent, Mods};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-wordsel-{}", std::process::id()));

        // 1. Source Ctrl+A with front matter -> "4 of 4 words" in right outline.
        let front = "---\ntitle: Front Matter Words\ntags: [alpha, beta]\n---\n\nOne two three four.\n";
        let (mut rt, _) = open(&dir, front, crate::app::Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| {
            s.persist.right_open = true;
            let len = s.active_text().len();
            s.cur_edit().set_selection(0, len);
            s.focus_editor(tx);
        });
        for _ in 0..8 {
            rt.frame(DT);
        }
        let wc = rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("word-count")).and_then(|e| e.text).unwrap_or_default();
        assert_eq!(wc, "4 of 4 words", "Ctrl+A must exclude front matter from selected word count");

        // 2. Preview task checkboxes: focusable, labelled, Space & Enter toggle, pointer leaves no ring.
        let tasks = "# Tasks\n\n- [ ] First task\n- [ ] Second task\n";
        let (mut rt, _) = open(&dir, tasks, crate::app::Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((800.0, 500.0), 1.0);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let cbs: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting && e.role == "checkbox").collect();
        assert_eq!(cbs.len(), 2, "two task checkboxes rendered");
        assert!(cbs.iter().all(|c| c.focusable), "all preview task checkboxes must be focusable");
        assert_eq!(cbs[0].label.as_deref(), Some("First task"));
        assert_eq!(cbs[1].label.as_deref(), Some("Second task"));

        // Pointer click on second checkbox toggles it without stealing focus or lighting a ring.
        let r1 = cbs[1].rect;
        let (cx, cy) = (r1[0] + r1[2] / 2.0, r1[1] + r1[3] / 2.0);
        rt.pointer_move(cx, cy);
        rt.pointer_down(cx, cy);
        rt.pointer_up(cx, cy);
        for _ in 0..8 {
            rt.frame(DT);
        }
        let mut text_now = String::new();
        rt.update_if(|s, _| {
            text_now = s.active_text().to_string();
            false
        });
        assert!(text_now.contains("- [ ] First task") && text_now.contains("- [x] Second task"), "pointer click toggled second task: {text_now:?}");
        let any_cb_ring = rt.elements().into_iter().any(|e| !e.exiting && e.role == "checkbox" && e.focus_visible);
        assert!(!any_cb_ring, "pointer click must not leave a stray focus ring on checkbox");

        // Keyboard Tab into first checkbox, then Space toggles it.
        let tab = KeyEvent { key: Key::Tab, mods: Mods::default() };
        let mut reached = false;
        for _ in 0..12 {
            rt.key(tab.clone());
            for _ in 0..3 {
                rt.frame(DT);
            }
            if rt.elements().into_iter().any(|e| !e.exiting && e.role == "checkbox" && e.focused) {
                reached = true;
                break;
            }
        }
        assert!(reached, "Tab must reach first preview task checkbox");
        rt.key(KeyEvent { key: Key::Text(" ".into()), mods: Mods::default() });
        for _ in 0..8 {
            rt.frame(DT);
        }
        rt.update_if(|s, _| {
            text_now = s.active_text().to_string();
            false
        });
        assert!(text_now.contains("- [x] First task"), "Space must toggle focused task checkbox: {text_now:?}");

        // Because toggling a task keeps the list's block key stable (zero unmount/reflow),
        // the first checkbox keeps focus and Enter untoggles it directly.
        assert!(
            rt.elements().into_iter().any(|e| !e.exiting && e.role == "checkbox" && e.focused),
            "checkbox keeps keyboard focus after toggle"
        );
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        for _ in 0..8 {
            rt.frame(DT);
        }
        rt.update_if(|s, _| {
            text_now = s.active_text().to_string();
            false
        });
        assert!(text_now.contains("- [ ] First task"), "Enter must toggle focused task checkbox: {text_now:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// TYPOPOLISH: menu.rs delegates Cut/Copy/Paste chord labels to keymap, Lock dialog heading
    /// uses 17 pt/600 scale (>= 20 pt box height), Settings White swatch card has a visible border
    /// on Paper, and sidebar folder rows request 600 weight for distinct hierarchy.
    #[test]
    fn typopolish_menu_labels_lock_heading_swatch_border_and_folder_weight() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let menu_src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/menu.rs")).unwrap();
        for lit in ["\"Ctrl+X\"", "\"Ctrl+C\"", "\"Ctrl+V\""] {
            assert!(!menu_src.contains(lit), "menu.rs should format via keymap::cmd_label instead of hardcoded {lit}");
        }
        assert_eq!(crate::keymap::cmd_label('x'), crate::keymap::label_of("Ctrl+X"));
        assert_eq!(crate::ui::styles::medium(12.5).weight, 600.0, "folder row style requests 600 weight");

        let dir = std::env::temp_dir().join(format!("quire-typopolish-{}", std::process::id()));
        let (mut rt, _) = open(&dir, "# Title\n\nBody.\n", crate::app::Mode::Source);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.ask_lock(tx));
        for _ in 0..10 {
            rt.frame(DT);
        }
        let heading = rt
            .elements()
            .into_iter()
            .find(|e| !e.exiting && e.path.contains("/lock-dialog/") && e.text.as_deref() == Some("Lock this note"))
            .expect("lock dialog heading renders");
        assert!(heading.rect[3] >= 20.0, "lock dialog heading uses 17 pt scale (>= 20 pt tall): {:?}", heading.rect);

        rt.update(|s, tx| s.open_settings(tx));
        for _ in 0..10 {
            rt.frame(DT);
        }
        let paper = crate::theme::theme_pal("paper", false);
        let card_border = crate::roles::mix(paper.bg, paper.ink, 0.18);
        let cr = crate::theme::contrast(card_border, paper.bg);
        assert!(cr >= 1.3, "unselected theme swatch border reads clearly on Paper: {cr:.2}:1");
        assert!(info(&rt, "theme-light-white").is_some(), "White swatch card renders in Settings");
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
        let words = rt.elements().into_iter().find(|e| e.text.as_deref().is_some_and(|t| t.contains("Esc to leave zen mode"))).expect("the hint's text");
        assert!(inside(words.rect, bar) && words.opacity > 0.99);
        let mid = bar[0] + bar[2] / 2.0;
        assert!((words.rect[0] + words.rect[2] / 2.0 - mid).abs() < 1.0, "centred: {:?} in {bar:?}", words.rect);
        let page = rt.presented(crate::reader::SCROLL).unwrap().0;
        assert!(page[1] >= bar[1] + bar[3] - 0.5, "the page {page:?} starts under the bar {bar:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An empty note's "Start writing…" sits on the text column, where the caret blinks, not
    /// out in the gutter where marks hang.
    #[test]
    fn the_placeholder_sits_on_the_text_column() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-placeholder-{}", std::process::id()));
        let (mut rt, _) = open(&dir, "", crate::app::Mode::Source);
        rt.set_viewport((640.0, 600.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        for _ in 0..30 {
            rt.frame(DT);
        }
        let field = info(&rt, "editor").expect("the editor");
        let caret = field.caret.expect("the editor shows a caret");
        // Rune draws the placeholder at the field's left edge.
        assert!((field.rect[0] - caret[0]).abs() < 1.0, "the placeholder at {} and the caret at {}", field.rect[0], caret[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-223: the zen hint has one place at each width, in the writer and in the preview, and
    /// keeps 16 pt from the pill, which shows under the pointer.
    #[test]
    fn the_zen_hint_keeps_its_place_and_its_distance() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-zen-place-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, tx| s.toggle_focus(tx));
        let mut bad = Vec::new();
        for w in [480.0, 520.0, 560.0, 600.0, 640.0, 720.0, 900.0, 1280.0] {
            rt.set_viewport((w, 600.0), 1.0);
            let mut at = Vec::new();
            for mode in [crate::app::Mode::Source, crate::app::Mode::Preview] {
                rt.update(|s, tx| s.show_mode(mode, tx));
                for _ in 0..60 {
                    rt.frame(DT);
                }
                let hint = |t: &str| t.split_once(' ').is_some_and(|(n, rest)| n.chars().all(|c| c.is_ascii_digit() || c == ',') && rest.starts_with("word"));
                let words = rt.elements().into_iter().find(|e| e.text.as_deref().is_some_and(hint)).expect("the hint's text").rect;
                let pill = info(&rt, "zen-actions").expect("the pill").rect;
                if pill[0] - (words[0] + words[2]) < 15.5 {
                    bad.push(format!("{w} {mode:?}: the hint {words:?} ends {:.1} pt from the pill {pill:?}", pill[0] - (words[0] + words[2])));
                }
                at.push(words);
            }
            if (at[0][0] - at[1][0]).abs() > 0.5 || (at[0][2] - at[1][2]).abs() > 0.5 {
                bad.push(format!("{w}: the hint at {:?} in the writer, {:?} in the preview", at[0], at[1]));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
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
        for key in ["mode-source", "mode-preview", "mode-map", "focus", "paragraph-focus", "typewriter", "next-font", "find", "help"] {
            assert!(inside(key), "{key} on the editor's pill");
        }
        for key in ["toggle-right", "copy-all", "wide"] {
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
        rt.update(|s, tx| s.show_mode(crate::app::Mode::Preview, tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let r = info(&rt, "zen-actions").expect("the floating bar in preview").rect;
        for key in ["mode-source", "mode-preview", "mode-map", "focus", "find", "copy-all", "wide", "help"] {
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

    /// RD-34, WR-227: the buttons both modes have keep one place in the title bar, so none
    /// moves when the mode changes.
    #[test]
    fn shared_title_bar_buttons_keep_their_place_across_modes() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-slots-{}", std::process::id()));
        let (mut rt, _) = open(&dir, "# A\n\nText.\n", crate::app::Mode::Source);
        let places = |rt: &mut rune::Runtime<crate::app::App>| {
            for _ in 0..60 {
                rt.frame(DT);
            }
            ["mode-source", "mode-preview", "mode-map", "focus", "toggle-right"].map(|k| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).expect(k).rect)
        };
        let source = places(&mut rt);
        rt.update(|s, tx| s.switch_mode(tx));
        let preview = places(&mut rt);
        assert_eq!(source, preview, "mode, zen and outline sit alike in source and preview");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-178, RD-38: the progress bar means one thing in both modes, the share of the note's
    /// lines above the view, so one place in the note reads alike in source and preview, and
    /// the end of the note fills it in both.
    #[test]
    fn wr_178_progress_means_one_thing_in_both_modes() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-progress-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        let lines = LONG.split('\n').count();
        let share = |rt: &rune::Runtime<crate::app::App>| info(rt, "progress").map_or(0.0, |e| e.rect[2]) / info(rt, "doc-scroll").unwrap().rect[2];
        let settle = |rt: &mut rune::Runtime<crate::app::App>| {
            for _ in 0..90 {
                rt.frame(DT);
            }
        };
        settle(&mut rt);
        let line = lines * 2 / 3;
        rt.update(move |s, tx| s.source_jump(line, tx));
        settle(&mut rt);
        let source = share(&rt);
        rt.update(|s, tx| s.switch_mode(tx));
        settle(&mut rt);
        let preview = share(&rt);
        let want = line as f32 / lines as f32;
        assert!((source - want).abs() < 0.08, "source reads {source:.2} at line {line} of {lines}");
        assert!((preview - want).abs() < 0.08, "preview reads {preview:.2} at line {line} of {lines}");
        rt.update(|s, tx| s.switch_mode(tx));
        settle(&mut rt);
        rt.update(move |s, tx| s.source_jump(lines - 2, tx));
        settle(&mut rt);
        assert!(share(&rt) > 0.99, "the note's end fills the bar in source: {:.2}", share(&rt));
        rt.update(|s, tx| s.switch_mode(tx));
        settle(&mut rt);
        rt.update(|s, tx| s.scroll_bottom(tx));
        settle(&mut rt);
        assert!(share(&rt) > 0.99, "and in preview: {:.2}", share(&rt));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WR-108, RD-38: with the caret in the note's first block, preview opens at its top, and
    /// the progress bar reads 0 there as it does in source.
    #[test]
    fn wr_108_the_first_block_leaves_preview_at_the_top() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-top-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        let share = |rt: &rune::Runtime<crate::app::App>| info(rt, "progress").map_or(0.0, |e| e.rect[2]);
        for _ in 0..90 {
            rt.frame(DT);
        }
        assert_eq!(share(&rt), 0.0, "source reads 0 at the top");
        rt.update(|s, tx| s.switch_mode(tx));
        for _ in 0..90 {
            rt.frame(DT);
        }
        assert_eq!(rt.state.reader.scroll.get()[0], 0.0, "preview stays at its top");
        assert_eq!(share(&rt), 0.0, "and reads 0 there");
        let _ = std::fs::remove_dir_all(&dir);
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
        rt.update(|s, tx| s.toggle_left(tx));
        for _ in 0..120 {
            rt.frame(DT);
        }
        let closed = info(&rt, "toggle-left").expect("the toggle shows with the sidebar closed").rect;
        assert!(info(&rt, "sidebar").is_none(), "the sidebar closed");
        assert_eq!(open, closed, "the toggle stays put");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The docked sidebar slides off and back in as one full-height piece, locked edge-to-edge
    /// with `main` across every frame.
    #[test]
    fn sidebar_slides_as_one_full_height_piece() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-side-slide-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Source);
        rt.update(|s, _| s.persist.left_open = true);
        for _ in 0..30 {
            rt.frame(DT);
        }
        let root_h = rt.presented("root").unwrap().0[3];
        let (x, y) = rt.find("toggle-left").expect("toggle-left");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let side = rt.presented("sidebar").expect("sidebar still sliding out").0;
        let main = rt.presented("main").expect("main").0;
        assert_eq!(side[1], 0.0, "sidebar stays pinned to top while closing: {side:?}");
        assert_eq!(side[3], root_h, "sidebar stays full height while closing: {side:?}");
        assert!(side[0] < -10.0, "sidebar slides off-screen to the left: {side:?}");
        assert!(((side[0] + side[2]) - main[0]).abs() < 1.0, "sidebar and main stay locked edge-to-edge while closing: side={side:?} main={main:?}");
        for _ in 0..40 {
            rt.frame(DT);
        }
        assert!(rt.presented("sidebar").is_none(), "sidebar unmounted after closing");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..3 {
            rt.frame(DT);
        }
        let side = rt.presented("sidebar").expect("sidebar sliding in").0;
        let main = rt.presented("main").expect("main").0;
        assert_eq!(side[1], 0.0, "sidebar stays pinned to top while opening: {side:?}");
        assert_eq!(side[3], root_h, "sidebar stays full height while opening: {side:?}");
        assert!(side[0] < -10.0, "sidebar enters from off-screen left: {side:?}");
        assert!(((side[0] + side[2]) - main[0]).abs() < 1.0, "sidebar and main stay locked edge-to-edge while opening: side={side:?} main={main:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Topbar buttons have consistent spacing (4 pt gaps, 8 pt around the divider, 8 pt outer
    /// margins when no macOS traffic lights are drawn).
    #[test]
    fn topbar_icons_have_even_spacing_and_margins() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-bar-space-{}", std::process::id()));
        let (mut rt, _) = open(&dir, LONG, crate::app::Mode::Preview);
        for _ in 0..20 {
            rt.frame(DT);
        }
        let root_w = rt.presented("root").unwrap().0[2];
        let left = info(&rt, "toggle-left").expect("toggle-left").rect;
        if !cfg!(target_os = "macos") {
            assert_eq!(left[0], 8.0, "toggle-left sits 8 pt from the left edge: {left:?}");
        }
        let buttons = ["find", "copy-all", "wide", "help", "mode-source", "mode-preview", "mode-map", "focus", "toggle-right"];
        let rects: Vec<[f32; 4]> = buttons.iter().map(|k| info(&rt, k).expect(k).rect).collect();
        let right_edge = rects.last().unwrap()[0] + rects.last().unwrap()[2];
        assert!((root_w - right_edge - 8.0).abs() < 0.5, "toggle-right ends 8 pt from right edge: {right_edge} vs {root_w}");
        for pair in [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7), (7, 8)] {
            let gap = rects[pair.1][0] - (rects[pair.0][0] + rects[pair.0][2]);
            assert!((gap - 4.0).abs() < 0.5, "4 pt gap between {} and {}: {gap}", buttons[pair.0], buttons[pair.1]);
        }
        let sep = info(&rt, "bar-sep").expect("bar-sep").rect;
        let before = sep[0] - (rects[3][0] + rects[3][2]);
        let after = rects[4][0] - (sep[0] + sep[2]);
        assert!((before - 4.0).abs() < 0.5 && (after - 4.0).abs() < 0.5, "uniform 4 pt around divider: before={before} after={after}");
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

    /// WR-118 (adapted), RD-59: find in source marks every match as preview does, the current
    /// one solid accent with its letters in the page colour, on the letters of the match.
    #[test]
    fn wr_118_find_marks_every_match() {
        use rune::render::{KIND_GLYPH, KIND_RECT};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-find-marks-{}", std::process::id()));
        let text = "# Notes\n\nMost of it isn't. A tool that responds disappears, and a tool that hesitates does not.\n\n## A tool\n";
        let (mut rt, _) = open(&dir, text, crate::app::Mode::Source);
        rt.update(|s, tx| {
            s.reader.find.q = rune::TextEdit::new("tool".to_string());
            s.writer_find(tx);
        });
        for _ in 0..30 {
            rt.frame(DT);
        }
        let mut roles = None;
        rt.update(|s, _| roles = Some(crate::roles::Roles::new(s.palette(false))));
        let c = roles.unwrap();
        // Only the note's: the sidebar's open tab is marked in the accent too.
        let page = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("editor")).expect("the editor").rect;
        let painted = |kind: f32, col: rune::Color| -> Vec<[f32; 4]> {
            let want = [col.premul(1.0), col.premul_srgb(1.0)];
            rt.instances.iter().filter(|q| q.params[1] == kind && q.rect[0] >= page[0] && want.iter().any(|w| (0..4).all(|k| (q.fill[k] - w[k]).abs() < 0.004))).map(|q| q.rect).collect()
        };
        let current = painted(KIND_RECT, c.find_current);
        assert_eq!(current.len(), 1, "one current match: {current:?}");
        assert_eq!(painted(KIND_RECT, c.find).len(), 2, "and the two others marked");
        // Its letters take the page colour, all under its mark, past the editor's gutter.
        let [x, y, w, h] = current[0];
        let letters: Vec<_> = painted(KIND_GLYPH, c.find_current_text).into_iter().filter(|r| r[1] < y + h && r[1] + r[3] > y).collect();
        assert_eq!(letters.len(), 4, "the letters of tool: {letters:?} under {:?}", current[0]);
        assert!(letters.iter().all(|r| r[0] >= x - 1.0 && r[0] + r[2] <= x + w + 1.0), "inside the mark: {letters:?} under {:?}", current[0]);
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
    /// nothing open goes to the preview. Esc there stays in preview (one-way Esc).
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
                seen = (s.mode, s.focus, s.menu().is_some_and(|m| matches!(m.of, crate::layer::MenuOf::Edit(_))));
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
            open = s.menu().is_some_and(|m| matches!(m.of, crate::layer::MenuOf::Edit(_)));
            false
        });
        assert!(open, "the right-click opened the editor's menu");
        assert_eq!(esc(&mut rt), (Mode::Source, false, false), "Esc closes the editor's menu and stays");
        assert_eq!(esc(&mut rt), (Mode::Preview, false, false), "with nothing open, Esc goes to the preview");
        assert_eq!(esc(&mut rt), (Mode::Preview, false, false), "and Esc in the preview stays in preview");
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
        let typed = |rt: &mut Runtime<crate::app::App>, button: &str, ch: &str| {
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

    /// The rule's menus work from the keyboard: Enter on the folder's name opens its menu with
    /// the first item marked, Up and Down move and wrap, Esc closes it and the name has the
    /// keyboard again. A menu the pointer opened marks nothing until a key moves in it; a letter
    /// jumps to its item and Enter picks it.
    #[test]
    fn menus_work_from_the_keyboard() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-key-menu-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, "# Head\n\nBody.\n", crate::app::Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        let key = |rt: &mut Runtime<crate::app::App>, key: Key| {
            rt.key(KeyEvent { key, mods: Mods::default() });
            for _ in 0..3 {
                rt.frame(DT);
            }
        };
        let marked = |rt: &Runtime<crate::app::App>| rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.focus_visible));
        rt.update(|_, tx| tx.focus("label-vault", true));
        rt.frame(DT);
        rt.key(KeyEvent { key: Key::Function(10), mods: Mods { shift: true, ..Mods::default() } });
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert_eq!(marked(&rt), Some(("menu-switch".into(), true)), "Shift+F10 opened the vault's menu, first item marked");
        key(&mut rt, Key::Down);
        assert_eq!(marked(&rt), Some(("menu-new-folder".into(), true)));
        key(&mut rt, Key::Down);
        assert_eq!(marked(&rt), Some(("menu-switch".into(), true)), "Down wraps");
        key(&mut rt, Key::Escape);
        let mut open = true;
        rt.update_if(|s, _| {
            open = s.layer().is_some();
            false
        });
        assert!(!open, "Esc closed it");
        assert_eq!(marked(&rt).map(|m| m.0), Some("label-vault".into()), "and the vault's label has the keyboard");

        // Shift+F10 in the editor opens its menu below the caret, on Paste: with nothing
        // selected Cut and Copy are off. Esc gives the editor the keyboard back.
        rt.update(|s, tx| {
            s.focus_editor(tx);
            s.cur_edit().set_selection(0, 0);
        });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let caret = rt.elements().into_iter().find_map(|e| e.caret).expect("the editor's caret");
        rt.key(KeyEvent { key: Key::Function(10), mods: Mods { shift: true, ..Mods::default() } });
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert_eq!(marked(&rt), Some(("tm-2".into(), true)), "the editor's menu opened on Paste");
        let ([_, top, _, _], _) = rt.presented("edit-menu").expect("the editor's menu");
        assert!((top - (caret[1] + caret[3])).abs() < 20.0, "below the caret: menu at {top}, caret {caret:?}");
        key(&mut rt, Key::Escape);
        assert_eq!(marked(&rt).map(|m| m.0), Some("editor".into()), "Esc gives the editor the keyboard back");

        let (x, y) = rt.find("file:note.md").expect("the note's row");
        rt.pointer_move(x, y);
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        for _ in 0..3 {
            rt.frame(DT);
        }
        assert!(rt.elements().iter().all(|e| !(e.focused && e.focus_visible && e.key.as_deref().is_some_and(|k| k.starts_with("tm-")))), "a menu the pointer opened marks no item");
        key(&mut rt, Key::Text("d".into()));
        assert_eq!(marked(&rt), Some(("tm-4".into(), true)), "d jumps to Delete note");
        key(&mut rt, Key::Enter);
        let mut seen = (true, true);
        rt.update_if(|s, _| {
            seen = (s.layer().is_some(), s.files().contains_key("note.md"));
            false
        });
        assert_eq!(seen, (false, false), "Enter deleted the note and closed the menu");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RD-68: a right-click on a preview block, or in the gap above it, opens Quire's menu for
    /// that block instead of copying at once, and its rows act on that block.
    #[test]
    fn rd_68_a_right_click_in_preview_opens_the_block_menu() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-block-menu-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, "# Head\n\nFirst *one*.\n\nSecond one.\n", crate::app::Mode::Preview);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let blocks: Vec<[f32; 4]> = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("block")).map(|e| e.rect).collect();
        let [x, y, _, _] = blocks[2];
        let open = |rt: &mut Runtime<crate::app::App>, at: (f32, f32)| {
            rt.secondary_down(at.0, at.1);
            rt.secondary_up(at.0, at.1);
            for _ in 0..3 {
                rt.frame(DT);
            }
            let mut got = (None, None);
            rt.update_if(|s, _| {
                got = ((match s.menu().map(|m| &m.of) { Some(crate::layer::MenuOf::Block(l)) => Some(*l), _ => None }), s.reader.toast.as_ref().map(|t| t.0.clone()));
                false
            });
            got
        };
        // 4 points above the block: the gap above it belongs to its row.
        assert_eq!(open(&mut rt, (x + 10.0, y - 4.0)), (Some(4), None), "the menu opens for the block, and nothing is copied yet");
        let press = |rt: &mut Runtime<crate::app::App>, row: &str| {
            let (cx, cy) = rt.find(&format!("block-menu/{row}")).expect(row);
            rt.pointer_move(cx, cy);
            rt.frame(DT);
            rt.pointer_down(cx, cy);
            rt.pointer_up(cx, cy);
            for _ in 0..3 {
                rt.frame(DT);
            }
        };
        press(&mut rt, "tm-1");
        let mut after = (Some(0), None);
        rt.update_if(|s, _| {
            after = ((match s.menu().map(|m| &m.of) { Some(crate::layer::MenuOf::Block(l)) => Some(*l), _ => None }), s.reader.toast.as_ref().map(|t| t.0.clone()));
            false
        });
        assert_eq!(after, (None, Some("Copied the block's text".into())), "Copy text copies and closes the menu");
        let [x, y, w, h] = blocks[1];
        assert_eq!(open(&mut rt, (x + w / 2.0, y + h / 2.0)).0, Some(2));
        press(&mut rt, "tm-2");
        let mut lines = None;
        rt.update_if(|s, _| {
            lines = s.sheet().map(|sh| sh.pick.lines.clone());
            false
        });
        assert_eq!(lines, Some(2..3), "Edit block opens the sheet on the clicked block");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Front matter card shows its title and fields without a "front matter" corner label.
    #[test]
    fn front_matter_card_omits_front_matter_label() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-front-label-{}", std::process::id()));
        let md = "---\ntitle: Architecture Notes\ndate: 2026-11-19\nstatus: active\n---\n\n# Body\n";
        let (mut rt, _) = crate::bench::open(&dir, md, crate::app::Mode::Preview);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let texts: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
        assert!(texts.iter().any(|t| t == "Architecture Notes"), "front matter title renders: {texts:?}");
        assert!(
            !texts.iter().any(|t| t.eq_ignore_ascii_case("front matter")),
            "front matter card must not render the 'front matter' label: {texts:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Preview mode supports mouse drag selection (single- and multi-block) for copying (Ctrl+C)
    /// and editing (`e` edits every selected block; `E` edits their covering section(s)), without
    /// keyboard block selection.
    #[test]
    fn preview_mouse_selection_supports_copy_and_multi_block_edit() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-preview-sel-{}", std::process::id()));
        let md = "# One\n\nFirst paragraph here.\n\n## Two\n\nSecond paragraph here.\n\nThird paragraph here.\n";
        let (mut rt, _) = crate::bench::open(&dir, md, crate::app::Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        for _ in 0..10 {
            rt.frame(DT);
        }
        let blocks: Vec<[f32; 4]> = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("block")).map(|e| e.rect).collect();

        // 1. Shift+Down/Up do not create a keyboard selection in preview mode.
        let shift = rune::Mods { shift: true, ..rune::Mods::default() };
        rt.key(rune::KeyEvent { key: rune::Key::Down, mods: shift });
        rt.frame(DT);
        let mut sel = None;
        rt.update_if(|_, tx| {
            sel = tx.text_selection().map(|s| s.text().to_string());
            false
        });
        assert_eq!(sel, None, "Shift+Down does not select text in preview mode");

        // 2. Partial mouse drag across multiple blocks: from inside block 1 ("First paragraph here.")
        //    through block 2 ("## Two") into block 3 ("Second paragraph here.").
        let [x1, y1, _, h1] = blocks[1];
        let [x3, y3, _, h3] = blocks[3];
        rt.pointer_down(x1 + 35.0, y1 + h1 / 2.0);
        rt.pointer_move(x3 + 65.0, y3 + h3 / 2.0);
        rt.pointer_up(x3 + 65.0, y3 + h3 / 2.0);
        for _ in 0..3 {
            rt.frame(DT);
        }
        rt.update_if(|_, tx| {
            sel = tx.text_selection().map(|s| s.text().to_string());
            false
        });
        assert!(
            sel.as_deref().is_some_and(|t| t.contains("paragraph") && t.contains("Two") && t.contains("Second")),
            "mouse drag selected across multiple blocks: {sel:?}"
        );

        // Ctrl+C copies the selected text.
        let cmd = rune::Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..rune::Mods::default() };
        rt.key(rune::KeyEvent { key: rune::Key::Command('c'), mods: cmd });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut toast = None;
        rt.update_if(|s, _| {
            toast = s.reader.toast.as_ref().map(|t| t.0.clone());
            false
        });
        assert_eq!(toast.as_deref(), Some("Copied"), "Ctrl+C copies the mouse selection");

        // Pressing `e` opens the edit sheet containing ALL selected blocks (blocks 1..=3, lines 2..7),
        // with block 0 as before-context and block 4 as after-context.
        rt.key(rune::KeyEvent { key: rune::Key::Text("e".into()), mods: rune::Mods::default() });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut sheet_state = None;
        rt.update_if(|s, _| {
            sheet_state = s.sheet().map(|sh| (sh.pick.lines.clone(), sh.pick.before, sh.pick.after, sh.orig.clone()));
            false
        });
        assert_eq!(
            sheet_state,
            Some((2..7, Some(0), Some(4), "First paragraph here.\n\n## Two\n\nSecond paragraph here.".to_string())),
            "`e` opens the edit sheet with every block touched by the selection"
        );
        rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
        for _ in 0..3 {
            rt.frame(DT);
        }

        // 3. Ctrl+A selects all preview text, and Esc clears selection without leaving preview.
        rt.key(rune::KeyEvent { key: rune::Key::Command('a'), mods: cmd });
        for _ in 0..3 {
            rt.frame(DT);
        }
        rt.update_if(|_, tx| {
            sel = tx.text_selection().map(|s| s.text().to_string());
            false
        });
        assert!(
            sel.as_deref().is_some_and(|t| t.contains("One") && t.contains("Third paragraph here.")),
            "Ctrl+A selects the whole preview: {sel:?}"
        );
        rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
        for _ in 0..3 {
            rt.frame(DT);
        }
        let mut mode_and_sel = (crate::app::Mode::Source, true);
        rt.update_if(|s, tx| {
            mode_and_sel = (s.mode, tx.text_selection().is_some());
            false
        });
        assert_eq!(mode_and_sel, (crate::app::Mode::Preview, false), "first Esc clears selection and stays in preview");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
            seen = (s.menu().is_some_and(|m| matches!(m.of, crate::layer::MenuOf::Edit(_))), s.edit_menu_selected(), caret);
            false
        });
        assert_eq!(seen, (true, false, 0), "menu open, nothing selected, caret moved to the click");
        rt.update(|s, tx| s.close_layer(tx));
        rt.frame(DT);
        // "alpha" selected; a right-click at its start keeps it.
        rt.update(|s, _| s.cur_edit().set_selection(0, 5));
        rt.frame(DT);
        right_click(&mut rt);
        rt.update_if(|s, _| {
            seen = (s.menu().is_some_and(|m| matches!(m.of, crate::layer::MenuOf::Edit(_))), s.edit_menu_selected(), 0);
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
            open = s.menu().is_some_and(|m| matches!(m.of, crate::layer::MenuOf::Edit(_)));
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
        rt.update(|s, tx| s.toggle_left(tx));
        for _ in 0..60 {
            rt.frame(DT);
        }
        let row = crate::folders::row_key("note", &loose.join("a.md").to_string_lossy());
        assert!(shown(&rt, "open-notes") && shown(&rt, &row), "the Open list holds the note");
        // SB-119: the shelf and a footer with the settings button alone; the Open label does
        // not fold.
        for key in ["vault", "side-top", "search", "new-note", "handle", "foot-tags", "side-scroll"] {
            assert!(!shown(&rt, key), "no {key} in Note mode");
        }
        assert!(shown(&rt, "foot-settings") && shown(&rt, "label-shelf"), "the settings button and the Open label show");
        let folds = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("label-shelf")).is_some_and(|e| e.focusable || e.expanded.is_some());
        assert!(!folds, "the Open label does not fold in Note mode");
        let rect = |k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect).unwrap_or_else(|| panic!("{k}"));
        let (shelf, foot) = (rect("shelf"), rect("side-foot"));
        assert!((shelf[1] + shelf[3] - foot[1]).abs() < 0.5, "the shelf {shelf:?} reaches the footer {foot:?}");

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
            assert!(s.pal().is_none(), "no quick switcher");
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
            // Closing the note shown shows the one above it (SB-20); closing them all leaves none.
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
            s.wait_lock(tx);
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

#[cfg(test)]
mod layer_matrix_tests {
    //! The layer rule (`crate::layer`), run as a matrix: every overlay, opened as a user opens
    //! it, against every input that the rule says what to do with.
    use crate::app::{App, Mode};
    use rune::{Key, KeyEvent, Mods, Runtime};

    const DT: f64 = 1.0 / 60.0;

    struct Overlay {
        name: &'static str,
        /// Opened over the reader, not the writer.
        preview: bool,
        /// The window it needs: a drawer needs one too narrow to dock it.
        size: (f32, f32),
        /// A menu or a drawer: a window shortcut closes it and acts, and a change of mode
        /// closes it.
        light: bool,
        /// The row that has the keyboard again after Esc, for a menu the keyboard opened on
        /// it; `None` gives it back to the note.
        back: Option<&'static str>,
        open: fn(&mut Runtime<App>),
    }

    fn settle(rt: &mut Runtime<App>) {
        for _ in 0..4 {
            rt.frame(DT);
        }
    }

    fn key(rt: &mut Runtime<App>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        settle(rt);
    }

    /// The shortcut modifier, as Rune sends it: Cmd on macOS, Ctrl (which sets `cmd` too)
    /// elsewhere.
    fn ctrl(shift: bool) -> Mods {
        Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), shift, ..Mods::default() }
    }

    fn press(rt: &mut Runtime<App>, at: (f32, f32), secondary: bool) {
        rt.pointer_move(at.0, at.1);
        rt.frame(DT);
        if secondary {
            rt.secondary_down(at.0, at.1);
            rt.secondary_up(at.0, at.1);
        } else {
            rt.pointer_down(at.0, at.1);
            rt.pointer_up(at.0, at.1);
        }
        settle(rt);
    }

    fn press_key(rt: &mut Runtime<App>, el: &str, secondary: bool) {
        let at = rt.find(el).unwrap_or_else(|| panic!("no {el}"));
        press(rt, at, secondary);
    }

    fn overlays() -> Vec<Overlay> {
        let wide = (960.0, 600.0);
        let narrow = (640.0, 480.0);
        vec![
            Overlay { name: "palette", preview: false, size: wide, light: false, back: None, open: |rt| key(rt, Key::Command('k'), ctrl(false)) },
            Overlay { name: "settings", preview: false, size: wide, light: false, back: None, open: |rt| key(rt, Key::Command(','), ctrl(false)) },
            Overlay { name: "lock dialog", preview: false, size: wide, light: false, back: None, open: |rt| press_key(rt, "fmt-lock", false) },
            Overlay { name: "help", preview: true, size: wide, light: false, back: None, open: |rt| key(rt, Key::Text("?".into()), Mods { shift: true, ..Mods::default() }) },
            Overlay { name: "outline drawer", preview: false, size: narrow, light: true, back: None, open: |rt| press_key(rt, "toggle-right", false) },
            Overlay { name: "sidebar drawer", preview: false, size: narrow, light: true, back: None, open: |rt| press_key(rt, "toggle-left", false) },
            Overlay {
                name: "preview menu",
                preview: true,
                size: wide,
                light: true,
                back: None,
                open: |rt| {
                    let block = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("block")).expect("a block").rect;
                    press(rt, (block[0] + 20.0, block[1] + block[3] / 2.0), true);
                },
            },
            Overlay { name: "row menu", preview: false, size: wide, light: true, back: None, open: |rt| press_key(rt, "file:note.md", true) },
            Overlay {
                name: "editor menu",
                preview: false,
                size: wide,
                light: true,
                back: None,
                open: |rt| {
                    let ([x, y, _, _], _) = rt.presented("editor").expect("the editor");
                    press(rt, (x + 40.0, y + 40.0), true);
                },
            },
            Overlay { name: "row menu by keys", preview: false, size: wide, light: true, back: Some("file:note.md"), open: |rt| row_menu_by_keys(rt) },
            Overlay { name: "row menu by keys in preview", preview: true, size: wide, light: true, back: Some("file:note.md"), open: |rt| row_menu_by_keys(rt) },
            Overlay { name: "folder menu", preview: false, size: wide, light: true, back: None, open: |rt| press_key(rt, "label-vault", true) },
        ]
    }

    /// The note's row in the tree takes the keyboard, as the arrows put it there, and
    /// Shift+F10 opens its menu.
    fn row_menu_by_keys(rt: &mut Runtime<App>) {
        rt.update(|_, tx| tx.focus("file:note.md", true));
        settle(rt);
        key(rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
    }

    fn focused(rt: &Runtime<App>) -> Option<String> {
        rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
    }

    /// What a case looks at after its input.
    struct Seen {
        layers: usize,
        text: String,
        mode: Mode,
        palette: bool,
        settings: bool,
        find: bool,
        editor_focused: bool,
        page_y: f32,
    }

    fn seen(rt: &mut Runtime<App>) -> Seen {
        let mut s0 = None;
        rt.update_if(|s, _| {
            s0 = Some((s.open_layers().count(), s.active_text().to_string(), s.mode, s.pal().is_some(), s.layer_is(crate::layer::Kind::Settings), s.reader.find.open));
            false
        });
        let (layers, text, mode, palette, settings, find) = s0.unwrap();
        let editor_focused = rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some("editor"));
        let page_y = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("doc-col")).map_or(f32::NAN, |e| e.rect[1]);
        Seen { layers, text, mode, palette, settings, find, editor_focused, page_y }
    }

    const NOTE: &str = "# Head\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n";

    /// A fresh window on the note, in preview or with the caret at the top of the writer.
    fn window(dir: &std::path::Path, preview: bool, size: (f32, f32)) -> Runtime<App> {
        let text = format!("{NOTE}{}", "More text to scroll by.\n\n".repeat(80));
        let mode = if preview { Mode::Preview } else { Mode::Source };
        let (mut rt, _) = crate::bench::open(dir, &text, mode);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport(size, 1.0);
        settle(&mut rt);
        if !preview {
            rt.update(|s, tx| {
                s.cur_edit().set_selection(0, 0);
                s.focus_editor(tx);
            });
            settle(&mut rt);
        }
        rt
    }

    /// A fresh window with the overlay open, and what it showed then.
    fn opened(dir: &std::path::Path, o: &Overlay) -> (Runtime<App>, Seen) {
        let mut rt = window(dir, o.preview, o.size);
        (o.open)(&mut rt);
        let before = seen(&mut rt);
        assert_eq!(before.layers, 1, "{}: opened", o.name);
        (rt, before)
    }

    #[test]
    fn every_overlay_follows_the_layer_rule() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-layer-matrix-{}", std::process::id()));
        type Input = (&'static str, fn(&mut Runtime<App>));
        let inputs: [Input; 7] = [
            ("typing", |rt| {
                for c in ["Q", "Q", "Q"] {
                    key(rt, Key::Text(c.into()), Mods::default());
                }
            }),
            ("Esc", |rt| key(rt, Key::Escape, Mods::default())),
            ("Ctrl+K", |rt| key(rt, Key::Command('k'), ctrl(false))),
            ("Ctrl+,", |rt| key(rt, Key::Command(','), ctrl(false))),
            ("Ctrl+F", |rt| key(rt, Key::Command('f'), ctrl(false))),
            ("mode switch", |rt| {
                let mut preview = false;
                rt.update_if(|s, _| {
                    preview = s.mode == Mode::Preview;
                    false
                });
                key(rt, Key::Command(if preview { 'e' } else { 'p' }), ctrl(false));
            }),
            ("arrows and the wheel", |rt| {
                for _ in 0..4 {
                    key(rt, Key::Down, Mods::default());
                }
                rt.wheel(300.0, 300.0, -500.0);
                settle(rt);
            }),
        ];
        let mut fails = Vec::new();
        for o in overlays() {
            for (input, run) in &inputs {
                let (mut rt, before) = opened(&dir, &o);
                run(&mut rt);
                let after = seen(&mut rt);
                let case = format!("{} + {input}", o.name);
                let mut fail = |why: &str| fails.push(format!("{case}: {why}"));
                if after.text != before.text {
                    fail("the note changed");
                }
                if after.layers > 1 || (after.layers == 1 && after.find) {
                    fail("two layers up");
                }
                match *input {
                    "Esc" => {
                        if after.layers != 0 {
                            fail("still open");
                        } else if let Some(row) = o.back {
                            if focused(&rt).as_deref() != Some(row) {
                                fail("the row does not have the keyboard back");
                            }
                        } else if !o.preview && !after.editor_focused {
                            fail("the editor does not have the keyboard back");
                        }
                    }
                    "Ctrl+K" | "Ctrl+," | "Ctrl+F" if o.light => {
                        let acted = match *input {
                            "Ctrl+K" => after.palette,
                            "Ctrl+," => after.settings,
                            _ => after.find,
                        };
                        if !acted {
                            fail("the shortcut did nothing");
                        }
                    }
                    "mode switch" if o.light => {
                        if after.layers != 0 || after.mode == before.mode {
                            fail("a mode switch should close it and switch");
                        }
                    }
                    "arrows and the wheel" => {
                        if after.layers == 1 && (after.page_y - before.page_y).abs() > 0.5 {
                            fail("the note behind moved");
                        }
                    }
                    _ => {}
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "{} failed:\n{}", fails.len(), fails.join("\n"));
    }
    /// The reader's single keys reach the reader however preview was entered and whatever
    /// layer just closed over it: no control outside the page took the keyboard on the way.
    #[test]
    fn the_readers_keys_reach_the_reader() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-reader-keys-{}", std::process::id()));
        type Way = (&'static str, fn(&mut Runtime<App>));
        let ways: [Way; 10] = [
            ("Ctrl+P", |rt| key(rt, Key::Command('p'), ctrl(false))),
            ("the Preview button", |rt| press_key(rt, "mode-preview", false)),
            ("Esc", |rt| key(rt, Key::Escape, Mods::default())),
            ("Ctrl+P, the palette, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                key(rt, Key::Command('k'), ctrl(false));
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, Settings, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                key(rt, Key::Command(','), ctrl(false));
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, help, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                key(rt, Key::Text("?".into()), Mods { shift: true, ..Mods::default() });
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, a block menu, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                let block = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("block")).expect("a block").rect;
                press(rt, (block[0] + 20.0, block[1] + block[3] / 2.0), true);
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, a block menu by Shift+F10, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                key(rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, the folder menu by a click, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                press_key(rt, "label-vault", true);
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, the folder menu by a click, a press outside", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                press_key(rt, "label-vault", true);
                press(rt, (700.0, 300.0), false);
            }),
        ];
        let keys: [(&str, Key, bool); 4] = [("Space", Key::Text(" ".into()), true), ("j", Key::Text("j".into()), true), ("Enter", Key::Enter, false), ("n", Key::Text("n".into()), false)];
        let mut fails = Vec::new();
        for (way, enter) in &ways {
            for (name, k, scrolls) in &keys {
                let mut rt = window(&dir, false, (960.0, 600.0));
                enter(&mut rt);
                let before = seen(&mut rt);
                let case = format!("{way}, then {name}");
                if before.mode != Mode::Preview || before.layers != 0 {
                    fails.push(format!("{case}: not in preview with nothing open"));
                    continue;
                }
                key(&mut rt, k.clone(), Mods::default());
                let after = seen(&mut rt);
                if after.layers != 0 {
                    fails.push(format!("{case}: opened a layer"));
                } else if after.mode != Mode::Preview {
                    fails.push(format!("{case}: left preview"));
                } else if *scrolls && after.page_y > before.page_y - 5.0 {
                    fails.push(format!("{case}: the page did not scroll"));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "{} failed:\n{}", fails.len(), fails.join("\n"));
    }

    /// Find and the layers: opening any layer closes find and keeps its query, and as the
    /// layer closes the keyboard goes back to the note, with the caret at the match and
    /// nothing selected in the writer.
    #[test]
    fn a_layer_closes_find_and_gives_the_note_back() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-layer-find-{}", std::process::id()));
        let mut fails = Vec::new();
        // Help opens on a plain key, which find's field takes.
        for o in overlays().into_iter().filter(|o| o.name != "help") {
            let mut rt = window(&dir, o.preview, o.size);
            key(&mut rt, Key::Command('f'), ctrl(false));
            for c in ["p", "a", "r", "a"] {
                key(&mut rt, Key::Text(c.into()), Mods::default());
            }
            let start = seen(&mut rt);
            let mut fail = |why: &str| fails.push(format!("{}: {why}", o.name));
            if !start.find {
                fail("find did not open");
                continue;
            }
            (o.open)(&mut rt);
            let over = seen(&mut rt);
            if over.layers != 1 || over.find {
                fail("find stayed open beside the layer");
            }
            key(&mut rt, Key::Escape, Mods::default());
            let back = seen(&mut rt);
            if back.layers != 0 || back.find {
                fail("Esc left a layer or find open");
                continue;
            }
            if o.preview {
                key(&mut rt, Key::Text(" ".into()), Mods::default());
                if seen(&mut rt).page_y > back.page_y - 5.0 {
                    fail("Space did not reach the reader");
                }
            } else {
                for c in ["Z", "Z"] {
                    key(&mut rt, Key::Text(c.into()), Mods::default());
                }
                let typed = seen(&mut rt).text;
                if typed.replace("ZZ", "") != start.text {
                    fail("typing replaced text");
                } else if !typed.contains("ZZ") {
                    fail("typing went nowhere");
                } else if o.name != "editor menu" && !typed.contains("ZZpara") {
                    fail(&format!("typing did not land at the match ({:?})", typed.lines().nth(2).unwrap_or("")));
                }
            }
            key(&mut rt, Key::Command('f'), ctrl(false));
            let mut q = String::new();
            rt.update_if(|s, _| {
                q = s.reader.find.q.text.clone();
                false
            });
            if q != "para" {
                fail(&format!("Ctrl+F brought back {q:?}, not the query"));
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "{} failed:\n{}", fails.len(), fails.join("\n"));
    }

    /// The delete-folder confirmation answers the keyboard as a dialog does: focus starts on
    /// Cancel with its ring, Tab moves between the two buttons, Enter or Space presses the
    /// focused one, Esc cancels.
    #[test]
    fn the_delete_folder_confirmation_works_from_the_keyboard() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-layer-confirm-{}", std::process::id()));
        let focused = |rt: &Runtime<App>| rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.focus_visible));
        let state = |rt: &mut Runtime<App>| {
            let mut out = (0, 0);
            rt.update_if(|s, _| {
                out = (s.open_layers().count(), s.notes_in("Journal").len());
                false
            });
            out
        };
        let tab = |rt: &mut Runtime<App>| key(rt, Key::Tab, Mods::default());
        let cases: [(&str, fn(&mut Runtime<App>), bool); 4] = [
            ("Esc", |rt| key(rt, Key::Escape, Mods::default()), false),
            ("Enter on Cancel", |rt| key(rt, Key::Enter, Mods::default()), false),
            ("Tab, Space on Delete", |rt| {
                key(rt, Key::Tab, Mods::default());
                key(rt, Key::Text(" ".into()), Mods::default());
            }, true),
            ("Tab, Enter on Delete", |rt| {
                key(rt, Key::Tab, Mods::default());
                key(rt, Key::Enter, Mods::default());
            }, true),
        ];
        for (i, (name, answer, deletes)) in cases.into_iter().enumerate() {
            let mut rt = window(&dir, false, (960.0, 600.0));
            let root = dir.join("notes");
            std::fs::create_dir_all(root.join("Journal")).unwrap();
            std::fs::write(root.join("Journal/day.md"), "A day.\n").unwrap();
            rt.update(move |s, tx| s.open_vault(root, tx));
            settle(&mut rt);
            press_key(&mut rt, "dir:Journal", true);
            press_key(&mut rt, "tm-3", false);
            assert_eq!(state(&mut rt), (1, 1), "{name}: the confirmation is open");
            if i == 0 {
                assert_eq!(focused(&rt), Some(("tm-cancel".into(), true)), "focus starts on Cancel, with its ring");
                tab(&mut rt);
                assert_eq!(focused(&rt).map(|f| f.0), Some("tm-delete".into()), "Tab moves to Delete");
                tab(&mut rt);
                assert_eq!(focused(&rt).map(|f| f.0), Some("tm-cancel".into()), "and back to Cancel");
            }
            answer(&mut rt);
            assert_eq!(state(&mut rt), (0, if deletes { 0 } else { 1 }), "{name}: closed, and the folder {}", if deletes { "deleted" } else { "kept" });
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cases one layer over another, or a layer and something passing, used to miss: a
    /// shortcut over a menu inside a drawer, a resize under a menu, a layer opened during a
    /// drag, and one layer replacing another.
    #[test]
    fn nested_and_passing_cases_follow_the_layer_rule() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-layer-nested-{}", std::process::id()));
        let state = |rt: &mut Runtime<App>| {
            let mut out = (0, false);
            rt.update_if(|s, _| {
                out = (s.open_layers().count(), s.reader.find.open);
                false
            });
            out
        };
        let mut fails: Vec<&str> = Vec::new();

        let mut rt = window(&dir, false, (640.0, 480.0));
        press_key(&mut rt, "toggle-left", false);
        press_key(&mut rt, "file:note.md", true);
        assert_eq!(state(&mut rt).0, 2, "a row's menu over the sidebar's drawer");
        key(&mut rt, Key::Command('f'), ctrl(false));
        if state(&mut rt) != (0, true) {
            fails.push("Ctrl+F over a menu in a drawer closes both and opens find");
        }

        let mut rt = window(&dir, true, (960.0, 600.0));
        let block = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("block")).expect("a block").rect;
        press(&mut rt, (block[0] + 20.0, block[1] + block[3] / 2.0), true);
        assert_eq!(state(&mut rt).0, 1, "the block menu is open");
        rt.set_viewport((560.0, 600.0), 1.0);
        settle(&mut rt);
        if state(&mut rt).0 != 0 {
            fails.push("a resize closes a menu");
        }

        let mut rt = window(&dir, false, (960.0, 600.0));
        let (x, y) = rt.find("file:note.md").expect("the note's row");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        for i in 1..=6 {
            rt.pointer_move(x + 8.0 * i as f32, y + 6.0 * i as f32);
            rt.frame(DT);
        }
        assert!(rt.find("drag-ghost").is_some(), "the note is being dragged");
        key(&mut rt, Key::Command(','), ctrl(false));
        key(&mut rt, Key::Escape, Mods::default());
        rt.pointer_up(x + 48.0, y + 36.0);
        settle(&mut rt);
        if rt.find("drag-ghost").is_some() {
            fails.push("opening Settings ends a drag: no ghost after it closes");
        }

        let mut rt = window(&dir, false, (960.0, 600.0));
        key(&mut rt, Key::Command('k'), ctrl(false));
        assert!(rt.find("palette-frame").is_some(), "the palette is open");
        rt.key(KeyEvent { key: Key::Command(','), mods: ctrl(false) });
        rt.frame(DT);
        let both = rt.presented("palette-frame").is_some_and(|(_, o)| o > 0.0) && rt.presented("settings-frame").is_some();
        if both {
            fails.push("the palette leaves at once as Settings replaces it");
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "{} failed:\n{}", fails.len(), fails.join("\n"));
    }

    /// In preview the Menu key or Shift+F10 opens the block menu from the keyboard: for the
    /// block under the pointer, else the block at the top of the view, below its first line,
    /// with its first item marked.
    #[test]
    fn the_block_menu_opens_from_the_keyboard() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-layer-blockkey-{}", std::process::id()));
        let menu = |rt: &mut Runtime<App>| {
            let mut out = None;
            rt.update_if(|s, _| {
                out = s.menu().and_then(|m| match m.of {
                    crate::layer::MenuOf::Block(line) => Some(line),
                    _ => None,
                });
                false
            });
            out
        };
        let mut rt = window(&dir, true, (960.0, 600.0));
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        assert_eq!(menu(&mut rt), Some(0), "Shift+F10 opens the top block's menu");
        let marked = rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.focus_visible));
        assert_eq!(marked, Some(("tm-0".into(), true)), "its first item is marked");
        let ([_, y, _, _], _) = rt.presented("block-menu").expect("the block menu");
        let head = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("block")).expect("a block").rect;
        assert!(y > head[1] && y < head[1] + 80.0, "below the block's first line: menu at {y}, block {head:?}");
        key(&mut rt, Key::Escape, Mods::default());
        assert_eq!(menu(&mut rt), None, "Esc closes it");

        let third = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("block")).nth(2).expect("a third block").rect;
        rt.pointer_move(third[0] + 20.0, third[1] + third[3] / 2.0);
        settle(&mut rt);
        key(&mut rt, Key::ContextMenu, Mods::default());
        assert_eq!(menu(&mut rt), Some(4), "the Menu key opens the menu of the block under the pointer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every list in the side panels works from the keyboard by one rule (TREE): Tab enters
    /// it at its current row, the arrows move, Enter acts, Shift+F10 opens a row's menu and
    /// Esc gives the keyboard back to the note. A click on a row leaves the keyboard where it
    /// was.
    /// SB-71: the results follow the notes. A note that gains the query while it shows comes
    /// into them, and leaves them when it loses it, with no new search.
    #[test]
    fn the_results_follow_a_saved_note() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-follow-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, "# Note\n\nalpha\n", Mode::Source);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, _| s.query = rune::TextEdit::new("zebra"));
        settle(&mut rt);
        let rows = |rt: &Runtime<App>| rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("res-")).collect::<Vec<_>>();
        assert!(rows(&rt).is_empty(), "nothing matches yet: {:?}", rows(&rt));
        rt.update(|s, _| s.set_text("note.md", "# Note\n\nalpha zebra\n".into()));
        settle(&mut rt);
        assert_eq!(rows(&rt), ["res-text:note.md"], "the edited note comes into the results");
        rt.update(|s, _| s.set_text("note.md", "# Note\n\nalpha\n".into()));
        settle(&mut rt);
        // The row that leaves goes with the list's leave motion, a few frames later.
        for _ in 0..60 {
            rt.frame(DT);
        }
        assert!(rows(&rt).is_empty(), "and leaves them: {:?}", rows(&rt));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What the search tests read: the query, the open note, the mode and whether a drawer is
    /// open.
    fn search_state(rt: &mut Runtime<App>) -> (String, String, Mode, bool) {
        let mut out = None;
        rt.update_if(|s, _| {
            out = Some((s.query.text.clone(), s.active.clone().unwrap_or_default(), s.mode, s.open_layers().next().is_some()));
            false
        });
        out.unwrap()
    }

    /// Types `q` into the sidebar's search field, with the keyboard there.
    fn search_for(rt: &mut Runtime<App>, q: &str) {
        rt.update(|_, tx| tx.focus("search", true));
        settle(rt);
        key(rt, Key::Text(q.into()), Mods::default());
    }

    /// Opens the sidebar's drawer at 640 wide.
    fn sidebar_drawer(rt: &mut Runtime<App>) {
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(rt);
        rt.update(|s, tx| s.open_layer(crate::layer::Layer::Drawer(crate::app::Drawer::Sidebar), tx));
        settle(rt);
    }

    /// DESIGN §3: search reads every note once, then again only the notes that change. With
    /// results showing, 100 frames lower nothing more, and typing in the writer lowers the one
    /// note typed in.
    #[test]
    fn an_edit_relowers_only_its_note() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-relower-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        let lowered = |rt: &mut Runtime<App>| {
            let mut n = 0;
            rt.update_if(|s, _| {
                n = s.search.borrow().lowered();
                false
            });
            n
        };
        rt.update(|s, _| s.query = rune::TextEdit::new("e"));
        settle(&mut rt);
        let first = lowered(&mut rt);
        assert_eq!(first, 3, "the first search lowers each note once");
        for _ in 0..100 {
            rt.frame(DT);
        }
        assert_eq!(lowered(&mut rt), first, "frames lower nothing");
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        for c in ["a", "b", "c"] {
            text_key(&mut rt, c);
            for _ in 0..10 {
                rt.frame(DT);
            }
        }
        assert_eq!(lowered(&mut rt), first + 3, "each keystroke in the note lowers that note alone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-66, SB-69, SB-122: Enter in the field opens the filled row, the first, in the
    /// current mode with the keyboard in the note; in the drawer it closes the drawer. With no
    /// match Enter does nothing.
    #[test]
    fn enter_opens_the_filled_result() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-enter-result-{}", std::process::id()));
        for (mode, drawer) in [(Mode::Source, false), (Mode::Preview, false), (Mode::Source, true)] {
            let mut rt = garden(&dir, mode);
            if drawer {
                sidebar_drawer(&mut rt);
            }
            search_for(&mut rt, "b");
            let mut first = String::new();
            rt.update_if(|s, _| {
                first = s.results().and_then(|r| r.first().and_then(|row| row.note().map(str::to_string))).unwrap_or_default();
                false
            });
            assert_eq!(first, "Box/b.md", "{mode:?}: the name match comes first");
            key(&mut rt, Key::Enter, Mods::default());
            glide(&mut rt);
            let (query, active, now, layer) = search_state(&mut rt);
            assert_eq!((query.as_str(), active.as_str(), now, layer), ("b", "Box/b.md", mode, false), "{mode:?} drawer {drawer}: Enter opens the first result and the results stay");
            let want = if mode == Mode::Source { "editor" } else { crate::focus::NOTE };
            assert!(focused(&rt).is_none_or(|k| k == want) && (mode == Mode::Preview || focused(&rt).is_some()), "{mode:?} drawer {drawer}: with the keyboard in the note: {:?}", focused(&rt));
        }
        let mut rt = garden(&dir, Mode::Source);
        search_for(&mut rt, "zzzq");
        key(&mut rt, Key::Enter, Mods::default());
        assert_eq!(search_state(&mut rt).1, "note.md", "Enter with no match opens nothing");
        assert_eq!(focused(&rt).as_deref(), Some("search"), "and the field keeps the keyboard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-89, SB-92: in the field the first Esc clears the query and the field keeps the
    /// keyboard; the next gives the note the keyboard, or in the drawer closes the drawer.
    #[test]
    fn esc_clears_then_leaves() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-esc-search-{}", std::process::id()));
        for drawer in [false, true] {
            let mut rt = garden(&dir, Mode::Source);
            if drawer {
                sidebar_drawer(&mut rt);
            }
            search_for(&mut rt, "kept");
            key(&mut rt, Key::Escape, Mods::default());
            let (query, .., layer) = search_state(&mut rt);
            assert_eq!((query.as_str(), layer), ("", drawer), "drawer {drawer}: the first Esc clears the query, and the drawer stays");
            assert_eq!(focused(&rt).as_deref(), Some("search"), "drawer {drawer}: the field keeps the keyboard");
            assert!(rt.find("tree").is_some() && rt.find("results").is_none(), "drawer {drawer}: the lists are back");
            key(&mut rt, Key::Escape, Mods::default());
            assert!(!search_state(&mut rt).3, "drawer {drawer}: the next Esc leaves no drawer");
            assert_eq!(focused(&rt).as_deref(), Some("editor"), "drawer {drawer}: and the note has the keyboard");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-90, SB-94: Esc on a result row, with or without Shift, clears the search and puts
    /// the keyboard in the empty field with its ring, docked or in the drawer, where the
    /// drawer stays.
    #[test]
    fn esc_on_a_result_clears_to_the_field() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-esc-result-{}", std::process::id()));
        for (drawer, m) in [(false, Mods::default()), (true, Mods::default()), (false, Mods { shift: true, ..Mods::default() })] {
            let mut rt = garden(&dir, Mode::Preview);
            if drawer {
                sidebar_drawer(&mut rt);
            }
            search_for(&mut rt, "kept");
            key(&mut rt, Key::Down, Mods::default());
            assert!(focused(&rt).is_some_and(|k| k.starts_with("res-")), "Down puts the keyboard on a result: {:?}", focused(&rt));
            key(&mut rt, Key::Escape, m);
            let (query, .., layer) = search_state(&mut rt);
            assert_eq!((query.as_str(), layer), ("", drawer), "drawer {drawer} {m:?}: Esc on a row clears the search, and the drawer stays");
            let ring = rt.elements().into_iter().find(|e| e.focused).is_some_and(|e| e.focus_visible);
            assert!(focused(&rt).as_deref() == Some("search") && ring, "drawer {drawer} {m:?}: the field has the keyboard with its ring: {:?}", focused(&rt));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-11, SB-100: Ctrl+Shift+F puts the keyboard in the search field from the note in
    /// both modes, showing a hidden sidebar first, and selects the text already there.
    #[test]
    fn ctrl_slash_focuses_search() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-ctrl-slash-{}", std::process::id()));
        let chords = [(Key::Command('f'), ctrl(true))];
        for mode in [Mode::Source, Mode::Preview] {
            for (k, m) in chords.clone() {
                let mut rt = garden(&dir, mode);
                rt.update(|s, tx| {
                    s.persist.left_open = false;
                    s.query = rune::TextEdit::new("old");
                    s.keys_to_note(tx);
                });
                settle(&mut rt);
                key(&mut rt, k.clone(), m);
                let mut got = (false, 0, 0);
                rt.update_if(|s, _| {
                    got = (s.persist.left_open, s.query.anchor.min(s.query.focus), s.query.anchor.max(s.query.focus));
                    false
                });
                assert_eq!(focused(&rt).as_deref(), Some("search"), "{mode:?} {k:?}: the field has the keyboard");
                assert_eq!(got, (true, 0, 3), "{mode:?} {k:?}: the sidebar shows and the old text is selected");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-101, SB-102: F1 and Ctrl+/ open and close help in both modes; `?` still opens the
    /// reader's.
    #[test]
    fn f1_opens_help() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-f1-{}", std::process::id()));
        let help = |rt: &mut Runtime<App>| {
            let mut on = false;
            rt.update_if(|s, _| {
                on = s.layer_is(crate::layer::Kind::Help);
                false
            });
            on
        };
        for mode in [Mode::Source, Mode::Preview] {
            let mut rt = garden(&dir, mode);
            rt.update(|s, tx| s.keys_to_note(tx));
            settle(&mut rt);
            key(&mut rt, Key::Function(1), Mods::default());
            assert!(help(&mut rt), "{mode:?}: F1 opens help");
            key(&mut rt, Key::Command('/'), ctrl(false));
            assert!(!help(&mut rt), "{mode:?}: Ctrl+/ closes help");
            key(&mut rt, Key::Command('/'), ctrl(false));
            assert!(help(&mut rt), "{mode:?}: Ctrl+/ opens help");
            key(&mut rt, Key::Function(1), Mods::default());
            assert!(!help(&mut rt), "{mode:?}: F1 closes it");
            if mode == Mode::Source {
                assert_eq!(focused(&rt).as_deref(), Some("editor"), "the writer has the keyboard again");
            } else {
                text_key(&mut rt, "?");
                assert!(help(&mut rt), "? still opens the reader's help");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sidebar_lists_work_from_the_keyboard() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-lists-{}", std::process::id()));
        let garden = |mode: Mode| {
            let (mut rt, _) = crate::bench::open(&dir, &format!("{NOTE}\n## Second\n\n{}### Third\n\nEnd. #kept\n", "More text to scroll by.\n\n".repeat(80)), mode);
            let root = dir.join("notes");
            std::fs::create_dir_all(root.join("Box")).unwrap();
            std::fs::write(root.join("Box/a.md"), "# A\n\nSee [[note]]. #kept\n").unwrap();
            std::fs::write(root.join("Box/b.md"), "# B\n").unwrap();
            std::fs::write(root.join("c.md"), "# C\n\nparagraph #kept\n").unwrap();
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((1280.0, 800.0), 1.0);
            rt.update(move |s, tx| {
                s.open_vault(root, tx);
                s.open_file("note.md", None, tx);
                s.mode = mode;
                s.persist.right_open = true;
                s.focus_editor(tx);
            });
            settle(&mut rt);
            rt
        };
        let down = |rt: &mut Runtime<App>, k: Key| key(rt, k, Mods::default());
        // Tab from where focus is until a row of the list `of` has it.
        let tab_to = |rt: &mut Runtime<App>, of: fn(&str) -> bool| {
            for _ in 0..20 {
                down(rt, Key::Tab);
                if let Some(k) = focused(rt).filter(|k| of(k)) {
                    return k;
                }
            }
            panic!("Tab never reached the list: focus on {:?}", focused(rt));
        };
        let state = |rt: &mut Runtime<App>| {
            let mut out = None;
            rt.update_if(|s, _| {
                out = Some((s.active.clone().unwrap_or_default(), s.mode, s.expanded.contains("Box"), s.query.text.clone(), s.open_layers().count()));
                false
            });
            out.unwrap()
        };
        let ring = |rt: &Runtime<App>| rt.elements().into_iter().find(|e| e.focused).is_some_and(|e| e.focus_visible);

        // The tree, in preview: Tab lands on the open note's row with the focus ring.
        let mut rt = garden(Mode::Preview);
        assert_eq!(tab_to(&mut rt, |k| k.starts_with("file:") || k.starts_with("dir:")), "file:note.md", "Tab enters the tree at the open note");
        assert!(ring(&rt), "the focused row shows the focus ring");
        down(&mut rt, Key::Home);
        assert_eq!(focused(&rt).as_deref(), Some("dir:Box"), "Home goes to the first row");
        down(&mut rt, Key::Right);
        assert!(state(&mut rt).2, "Right opens a closed folder");
        down(&mut rt, Key::Right);
        assert_eq!(focused(&rt).as_deref(), Some("file:Box|a.md"), "Right moves into an open folder");
        down(&mut rt, Key::Down);
        assert_eq!(focused(&rt).as_deref(), Some("file:Box|b.md"), "Down moves to the next row");
        down(&mut rt, Key::Left);
        assert_eq!(focused(&rt).as_deref(), Some("dir:Box"), "Left moves to the row's folder");
        down(&mut rt, Key::Left);
        assert!(!state(&mut rt).2, "Left closes an open folder");
        down(&mut rt, Key::End);
        assert_eq!(focused(&rt).as_deref(), Some("file:note.md"), "End goes to the last row");
        down(&mut rt, Key::Up);
        assert_eq!(focused(&rt).as_deref(), Some("file:c.md"), "Up moves to the row before");
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("file:c.md")).expect("the row").rect;
        let ([mx, my, _, _], _) = rt.presented("tree-menu").expect("Shift+F10 opens the row's menu");
        assert!((mx - row[0]).abs() < 1.0 && my >= row[1] + row[3] && my < row[1] + row[3] + 6.0, "the menu opens under the row: {mx},{my} for {row:?}");
        assert_eq!(focused(&rt).as_deref(), Some("tm-0"), "its first item is marked");
        down(&mut rt, Key::Escape);
        assert_eq!(focused(&rt).as_deref(), Some("file:c.md"), "Esc closes the menu and the row has the keyboard again");
        down(&mut rt, Key::Enter);
        let (active, mode, ..) = state(&mut rt);
        assert_eq!((active.as_str(), mode), ("c.md", Mode::Preview), "Enter opens the note");
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "and the reader has the keyboard: {:?}", focused(&rt));

        // The Open list: Tab lands on the open note, Down and Enter open the next.
        assert_eq!(tab_to(&mut rt, |k| k.starts_with("tab:")), "tab:c.md", "Tab enters the Open list at the open note");
        down(&mut rt, Key::Up);
        down(&mut rt, Key::Enter);
        assert_eq!(state(&mut rt).0, "note.md", "Up and Enter open the other note");
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "and the reader has the keyboard: {:?}", focused(&rt));

        // Esc from a row gives the keyboard back to the reader.
        tab_to(&mut rt, |k| k.starts_with("tab:"));
        down(&mut rt, Key::Escape);
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "Esc gives the keyboard back to the reader: {:?}", focused(&rt));

        // The outline's headings and backlinks.
        let heads = tab_to(&mut rt, |k| k.starts_with("ol-"));
        down(&mut rt, Key::End);
        assert_ne!(focused(&rt), Some(heads), "End moves through the headings");
        let top = seen(&mut rt).page_y;
        down(&mut rt, Key::Enter);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "Enter jumps to the heading and gives the reader the keyboard: {:?}", focused(&rt));
        assert!(seen(&mut rt).page_y < top - 100.0, "the page moved to the heading");
        tab_to(&mut rt, |k| k.starts_with("bl-"));
        down(&mut rt, Key::Enter);
        assert_eq!(state(&mut rt).0, "Box/a.md", "Enter on a backlink opens its note");

        // The tags: Enter on a tag lists its notes, the keyboard follows to the first, and Enter
        // on one opens it in the current mode with the keyboard in the note.
        rt.update(|s, _| s.query = rune::TextEdit::new("#kep"));
        settle(&mut rt);
        assert_eq!(tab_to(&mut rt, |k| k.starts_with("res-tag:")), "res-tag:kept");
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        assert_eq!(state(&mut rt).4, 0, "Shift+F10 on a tag result row opens nothing");
        down(&mut rt, Key::Enter);
        let (.., query, _) = state(&mut rt);
        assert_eq!(query, "#kept", "Enter on a tag searches for it");
        assert_eq!(focused(&rt).as_deref(), Some("res-tagged:Box|a.md"), "the keyboard goes to its first note");
        down(&mut rt, Key::Down);
        assert_eq!(focused(&rt).as_deref(), Some("res-tagged:c.md"), "Down moves through its notes");
        down(&mut rt, Key::Enter);
        let (active, mode, ..) = state(&mut rt);
        assert_eq!((active.as_str(), mode), ("c.md", Mode::Preview), "a result opens in the current mode");
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "with the keyboard in the reader: {:?}", focused(&rt));

        // Shift+F10 on an Open row opens its tab menu (not the preview's block menu), and Esc
        // closes it. The shelf shows again once the search is cleared.
        rt.update(|s, _| {
            s.mode = Mode::Preview;
            s.query = rune::TextEdit::new("");
        });
        settle(&mut rt);
        tab_to(&mut rt, |k| k.starts_with("tab:"));
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        assert!(rt.presented("tab-menu").is_some(), "Shift+F10 on an Open row opens its tab menu");
        down(&mut rt, Key::Escape);
        assert_eq!(state(&mut rt).4, 0, "Esc closes the tab menu");

        // A click on a row leaves the keyboard where it was: with the editor in the writer,
        // with the reader in preview.
        for mode in [Mode::Source, Mode::Preview] {
            let mut rt = garden(mode);
            press_key(&mut rt, "file:c.md", false);
            press_key(&mut rt, "dir:Box", false);
            press_key(&mut rt, "tab:note.md", false);
            let want = if mode == Mode::Source { "editor" } else { crate::focus::NOTE };
            assert!(focused(&rt).is_none_or(|k| k == want) && (mode == Mode::Preview || focused(&rt).is_some()), "{mode:?}: a click on a row keeps the keyboard with the note: {:?}", focused(&rt));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// While a toast shows, in both modes, the page's colour runs from the note area's foot to
    /// above the pill, so no line of the note reads behind or beside it (RD-104).
    #[test]
    fn a_toast_sits_on_the_page_colour() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toast-band-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        for mode in [Mode::Source, Mode::Preview] {
            let (mut rt, _) = crate::bench::open(&dir, &"A line of the note.\n\n".repeat(200), mode);
            rt.set_viewport((1280.0, 800.0), 1.0);
            settle(&mut rt);
            assert_eq!(rect(&rt, "toast-band"), None, "{mode:?}: no band without a toast");
            rt.update(|s, tx| s.toast("Copied", tx));
            settle(&mut rt);
            let band = rect(&rt, "toast-band").unwrap_or_else(|| panic!("{mode:?}: a band under the toast"));
            let pill = rect(&rt, "toast").expect("the toast shows");
            let main = rect(&rt, "main").expect("the note area");
            assert_eq!(band[1] + band[3], main[1] + main[3], "{mode:?}: the band reaches the note area's foot");
            assert!(band[0] <= main[0] && band[0] + band[2] >= main[0] + main[2], "{mode:?}: the band spans the note area: {band:?} {main:?}");
            // Its 32 px fade ends 16 px above the pill, so the pill sits on solid page colour.
            assert!(band[1] + 32.0 <= pill[1] - 16.0, "{mode:?}: band {band:?}, pill {pill:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A toast and the notice centre in the part of the note area no drawer covers, at least
    /// 12 px clear of the drawer's edge and of the window's, on either side, while the toast's
    /// band keeps the note area's full width (RD-98, RD-104).
    #[test]
    fn foot_pills_stay_clear_of_a_drawer() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-foot-pills-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        for (drawer, toggle) in [("sidebar-over", "toggle-left"), ("outline-over", "toggle-right")] {
            let mut rt = garden(&dir, Mode::Preview);
            rt.set_viewport((640.0, 800.0), 1.0);
            settle(&mut rt);
            rt.update(|s, tx| {
                s.toast("Copied the whole note", tx);
                s.notice = Some("A folder named Docs is already there.".into());
            });
            press_key(&mut rt, toggle, false);
            glide(&mut rt);
            let d = rect(&rt, drawer).unwrap_or_else(|| panic!("the {drawer} is open"));
            let (from, to) = if drawer == "sidebar-over" { (d[0] + d[2], 640.0) } else { (0.0, d[0]) };
            for k in ["toast", "notice"] {
                let r = rect(&rt, k).unwrap_or_else(|| panic!("{drawer}: the {k} shows"));
                assert!(r[0] >= from + 12.0 && r[0] + r[2] <= to - 12.0, "{drawer}: the {k} {r:?} sits in x {from} to {to}, 12 px clear");
                assert!((r[0] + r[2] / 2.0 - (from + to) / 2.0).abs() < 1.0, "{drawer}: the {k} {r:?} centres in x {from} to {to}");
            }
            let band = rect(&rt, "toast-band").expect("the toast's band");
            assert!(band[0] <= 0.5 && band[0] + band[2] >= 639.5, "{drawer}: the band keeps the full width: {band:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// As the room narrows, with either drawer open or none, the toast with Undo gives up its
    /// parts in one order: the key hint, then the name's end down to eight letters and "…",
    /// then the name. Its text never runs under Undo or out of the pill, and Undo shows whole
    /// (UNDON).
    #[test]
    fn the_undo_toast_never_runs_under_its_button() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-undo-fit-{}", std::process::id()));
        let long = "On slow software and the long wait for it";
        for drawer in [None, Some(("sidebar-over", "toggle-left")), Some(("outline-over", "toggle-right"))] {
            // What shows at each room: the room is not the window's width, as a docked sidebar
            // takes part of it.
            let mut stages = Vec::new();
            for w in [1280.0, 760.0, 700.0, 640.0, 600.0, 580.0, 560.0, 540.0, 520.0] {
                let mut rt = garden(&dir, Mode::Preview);
                rt.set_viewport((w, 600.0), 1.0);
                settle(&mut rt);
                rt.update(|s, tx| {
                    s.delete_note();
                    s.deleted.as_mut().expect("a delete to undo").name = long.into();
                    s.open_file("Box/a.md", None, tx);
                });
                settle(&mut rt);
                if let Some((over, toggle)) = drawer {
                    // Wide enough, the panel docks and no drawer opens.
                    if w >= if over == "sidebar-over" { crate::app::SIDEBAR_DOCK_W } else { crate::app::OUTLINE_DOCK_W } {
                        continue;
                    }
                    press_key(&mut rt, toggle, false);
                    glide(&mut rt);
                    assert!(rt.find(over).is_some(), "{w}: the {over} is open");
                }
                let els = rt.elements();
                let pill = els.iter().find(|e| e.key.as_deref() == Some("toast") && !e.exiting).expect("the toast shows").rect;
                let undo = els.iter().find(|e| e.key.as_deref() == Some("toast-undo") && !e.exiting).expect("Undo shows");
                let texts: Vec<_> = els.iter().filter(|e| e.path.contains("/toast/") && !e.path.contains("toast-undo") && e.text.is_some()).collect();
                let undo_text = els.iter().find(|e| e.path.starts_with(&undo.path) && e.text.as_deref() == Some("Undo")).expect("Undo's text");
                assert!(undo_text.rect[0] >= undo.rect[0] && undo_text.rect[0] + undo_text.rect[2] <= undo.rect[0] + undo.rect[2], "{w} {drawer:?}: Undo shows whole");
                for t in &texts {
                    let r = t.rect;
                    assert!(r[0] + r[2] <= undo.rect[0] + 0.5, "{w} {drawer:?}: {:?} runs under Undo {:?}", t.text, undo.rect);
                    assert!(r[0] >= pill[0] && r[0] + r[2] <= pill[0] + pill[2], "{w} {drawer:?}: {:?} runs out of the pill {pill:?}", t.text);
                }
                let shown: Vec<&str> = texts.iter().filter_map(|e| e.text.as_deref()).collect();
                let hint = els.iter().any(|e| e.path.starts_with(&undo.path) && e.text.as_deref().is_some_and(|t| t.contains("Ctrl")));
                let stage = match shown.as_slice() {
                    ["Moved ", n, " to the trash"] if *n == long => if hint { 0 } else { 1 },
                    ["Moved ", n, " to the trash"] => {
                        let kept = n.strip_suffix('…').unwrap_or_else(|| panic!("{w} {drawer:?}: a cut name ends in …: {n}"));
                        assert!(long.starts_with(kept) && kept.chars().count() >= 8, "{w} {drawer:?}: the name keeps its first eight letters: {n}");
                        2
                    }
                    ["Moved to the trash"] => 3,
                    other => panic!("{w} {drawer:?}: the toast reads {other:?}"),
                };
                let room = els.iter().find(|e| e.key.as_deref() == Some("toast-row")).expect("the toast's row").rect[2] - 32.0;
                stages.push((room, stage, w));
            }
            stages.sort_by(|a, b| b.0.total_cmp(&a.0));
            for pair in stages.windows(2) {
                assert!(pair[1].1 >= pair[0].1, "{drawer:?}: a narrower room gives back a part it had given up: {pair:?}");
            }
            if drawer.is_some_and(|d| d.0 == "sidebar-over") {
                assert!(stages.iter().any(|s| s.2 == 520.0 && s.1 == 3), "at 520 with the sidebar's drawer the toast reads \"Moved to the trash\": {stages:?}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The notice is a pill in the toast's family (NOTCOV, PILL): on one line the toast's 30 pt
    /// with round ends, as wide as its text and "Dismiss" need; a long one wraps in the room
    /// with 15 pt corners. The toast band sits behind it, alone or stacked over a toast, and
    /// the line being typed stays above the band.
    #[test]
    fn the_notice_is_a_pill_over_the_band() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-notice-pill-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| (e.rect, e.radius));
        let short = "A folder named Docs is already there.";
        let long = "A folder named An exceptionally long folder name reserved for duplicate detection An exceptionally long folder name reserved for duplicate detection is already there.";
        for mode in [Mode::Source, Mode::Preview] {
            for (w, msg) in [(1280.0, short), (1280.0, long), (640.0, short), (640.0, long)] {
                let mut rt = garden(&dir, mode);
                rt.set_viewport((w, 800.0), 1.0);
                rt.update(|s, _| s.notice = Some(msg.into()));
                settle(&mut rt);
                let ((n, radius), (x, _)) = (rect(&rt, "notice").expect("the notice shows"), rect(&rt, "notice-x").expect("its dismiss"));
                let main = rect(&rt, "doc-area").expect("the note area").0;
                assert!(n[0] >= main[0] + 16.0 - 0.5 && n[0] + n[2] <= main[0] + main[2] - 16.0 + 0.5, "{mode:?} {w}: the notice {n:?} stays in its room {main:?}");
                assert_eq!(x[3], 24.0, "{mode:?} {w}: dismiss takes a 24 pt press");
                assert!(x[0] + x[2] <= n[0] + n[2] && x[1] >= n[1] + 3.0, "{mode:?} {w}: dismiss {x:?} sits in the pill {n:?}");
                if msg == short {
                    assert_eq!(n[3], 30.0, "{mode:?} {w}: one line is the toast's height");
                    assert!(radius >= 15.0, "{mode:?} {w}: with round ends: {radius}");
                    let t = rt.elements().into_iter().find(|e| e.text.as_deref() == Some(short)).expect("the message");
                    assert!(n[2] - t.rect[2] - x[2] <= 24.5, "{mode:?} {w}: as wide as its text and dismiss: {n:?}");
                } else {
                    assert!(n[3] > 30.0, "{mode:?} {w}: a long message wraps: {n:?}");
                    assert_eq!(radius, 15.0, "{mode:?} {w}: wrapped, its corners are half the one-line height");
                }
                let (band, _) = rect(&rt, "toast-band").unwrap_or_else(|| panic!("{mode:?} {w}: the band shows behind the notice"));
                assert_eq!(band[1] + 32.0 + 16.0, n[1], "{mode:?} {w}: the band is solid from 16 pt above the notice");
                rt.update(|s, tx| s.toast("Saved", tx));
                glide(&mut rt);
                let ((t, _), (n, _), (band, _)) = (rect(&rt, "toast").unwrap(), rect(&rt, "notice").unwrap(), rect(&rt, "toast-band").unwrap());
                assert_eq!(n[1] + n[3] + 8.0, t[1], "{mode:?} {w}: the notice stacks 8 pt above the toast");
                assert_eq!(band[1] + 32.0 + 16.0, n[1], "{mode:?} {w}: the band covers the stack");
            }
        }
        // In the writer the line being typed stays above the band behind a wrapped notice.
        let (mut rt, _) = crate::bench::open(&dir, &"A line of the note.\n\n".repeat(200), Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        key(&mut rt, Key::End, ctrl(false));
        rt.update(|s, _| s.notice = Some(long.into()));
        glide(&mut rt);
        for c in ["Z", "Z", "Z"] {
            key(&mut rt, Key::Text(c.into()), Mods::default());
        }
        glide(&mut rt);
        let (band, _) = rect(&rt, "toast-band").expect("the band");
        let (caret, _) = rect(&rt, "caret-at").expect("the caret");
        assert!(caret[1] + caret[3] <= band[1], "the line being typed clears the band: caret {caret:?}, band {band:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Esc dismisses a showing notice first, before find, zen and the switch of mode, and the
    /// keyboard stays where it was; an open layer still takes Esc before it (NDIS).
    #[test]
    fn esc_dismisses_the_notice_first() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-notice-esc-{}", std::process::id()));
        let notice = |rt: &mut Runtime<App>| {
            rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
            settle(rt);
        };
        let up = |rt: &mut Runtime<App>| {
            let mut on = false;
            rt.update_if(|s, _| {
                on = s.notice.is_some();
                false
            });
            on
        };
        for mode in [Mode::Source, Mode::Preview] {
            let mut rt = garden(&dir, mode);
            if mode == Mode::Source {
                rt.update(|s, tx| s.focus_editor(tx));
                settle(&mut rt);
            }
            let before = focused(&rt);
            notice(&mut rt);
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            assert!(!up(&mut rt), "{mode:?}: Esc dismisses the notice");
            let mut now = mode;
            rt.update_if(|s, _| {
                now = s.mode;
                false
            });
            assert_eq!(now, mode, "{mode:?}: and leaves the mode alone");
            assert_eq!(focused(&rt), before, "{mode:?}: the keyboard stays where it was");
            // Before find: the first Esc takes the notice, the second closes find.
            key(&mut rt, Key::Command('f'), ctrl(false));
            settle(&mut rt);
            notice(&mut rt);
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            let find_open = |rt: &mut Runtime<App>| {
                let mut open = false;
                rt.update_if(|s, _| {
                    open = s.reader.find.open;
                    false
                });
                open
            };
            assert!(!up(&mut rt) && find_open(&mut rt), "{mode:?}: Esc takes the notice before find");
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            assert!(!find_open(&mut rt), "{mode:?}: the next Esc closes find");
            // Before zen mode.
            rt.update(|s, _| s.focus = true);
            notice(&mut rt);
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            let mut zen = false;
            rt.update_if(|s, _| {
                zen = s.focus;
                false
            });
            assert!(!up(&mut rt) && zen, "{mode:?}: Esc takes the notice before zen mode");
            rt.update(|s, _| s.focus = false);
            // An open layer takes Esc first.
            notice(&mut rt);
            key(&mut rt, Key::Command('k'), ctrl(false));
            settle(&mut rt);
            assert!(rt.find("pal-input").is_some(), "{mode:?}: the palette opened");
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            assert!(up(&mut rt) && rt.find("pal-input").is_none(), "{mode:?}: the palette takes Esc before the notice");
        }
        // One rule wherever the keyboard is on the page: the first Esc takes the notice and the
        // stop or field keeps the keyboard; the next Esc does what Esc does there.
        let places: [(&str, fn(&mut Runtime<App>)); 5] = [
            ("the search field", |rt| rt.update(|s, tx| s.open_search(tx))),
            ("the note's name", |rt| rt.update(|s, tx| s.start_rename(tx))),
            ("a new folder's name", |rt| rt.update(|s, tx| s.start_new_folder("", tx))),
            ("a tree row", |rt| rt.update(|_, tx| tx.focus("file:note.md", true))),
            ("a section label", |rt| rt.update(|_, tx| tx.focus("label-shelf", true))),
        ];
        for (place, go) in places {
            let mut rt = garden(&dir, Mode::Source);
            rt.update(|s, tx| s.focus_editor(tx));
            settle(&mut rt);
            go(&mut rt);
            settle(&mut rt);
            let at = focused(&rt);
            assert!(at.is_some() && at.as_deref() != Some("editor"), "{place}: has the keyboard: {at:?}");
            notice(&mut rt);
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            assert!(!up(&mut rt), "{place}: the first Esc dismisses the notice");
            assert_eq!(focused(&rt), at, "{place}: and keeps the keyboard");
            key(&mut rt, Key::Escape, Mods::default());
            settle(&mut rt);
            assert_eq!(focused(&rt).as_deref(), Some("editor"), "{place}: the next Esc gives the note the keys");
        }
        // A click on Dismiss leaves the keyboard in the note.
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        notice(&mut rt);
        let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("notice-x")).expect("Dismiss").rect;
        rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        rt.pointer_down(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        rt.pointer_up(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        settle(&mut rt);
        assert!(!up(&mut rt), "a click on Dismiss dismisses the notice");
        assert_eq!(focused(&rt).as_deref(), Some("editor"), "and leaves the keyboard in the note");
        // The same with the keyboard in the search field.
        rt.update(|s, tx| s.open_search(tx));
        settle(&mut rt);
        notice(&mut rt);
        let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("notice-x")).expect("Dismiss").rect;
        rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        rt.pointer_down(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        rt.pointer_up(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        settle(&mut rt);
        assert!(!up(&mut rt), "a click on Dismiss dismisses the notice from the search field");
        assert_eq!(focused(&rt).as_deref(), Some("search"), "and leaves the keyboard in the search field");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Esc with Shift, Ctrl or Alt in the search field, find's field and the note does what Esc
    /// does there: it dismisses a showing notice, then gives the keyboard to the note (SHESC).
    /// The field gives up the keyboard on any Esc, so a modifier must not leave it nowhere.
    #[test]
    fn esc_with_a_modifier_is_esc_in_a_field() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-mod-esc-{}", std::process::id()));
        let notice = |rt: &mut Runtime<App>| {
            rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
            settle(rt);
        };
        let state = |rt: &mut Runtime<App>| {
            let mut seen = (false, false, Mode::Source);
            rt.update_if(|s, _| {
                seen = (s.notice.is_some(), s.reader.find.open, s.mode);
                false
            });
            seen
        };
        let shift = Mods { shift: true, ..Mods::default() };
        let alt = Mods { alt: true, ..Mods::default() };
        for mode in [Mode::Source, Mode::Preview] {
            let note = if mode == Mode::Source { "editor" } else { "note-keys" };
            for m in [shift, ctrl(false), alt] {
                let mut rt = garden(&dir, mode);
                if mode == Mode::Source {
                    rt.update(|s, tx| s.focus_editor(tx));
                    settle(&mut rt);
                }
                rt.update(|s, tx| s.open_search(tx));
                settle(&mut rt);
                notice(&mut rt);
                key(&mut rt, Key::Escape, m);
                assert!(!state(&mut rt).0, "{mode:?} {m:?}: Esc in the search field dismisses the notice");
                assert_eq!(focused(&rt).as_deref(), Some("search"), "{mode:?} {m:?}: and the search field keeps the keyboard");
                key(&mut rt, Key::Escape, m);
                assert_eq!(focused(&rt).as_deref(), Some(note), "{mode:?} {m:?}: the next Esc gives the note the keyboard");
                key(&mut rt, Key::Command('f'), ctrl(false));
                notice(&mut rt);
                key(&mut rt, Key::Escape, m);
                assert_eq!(state(&mut rt), (false, true, mode), "{mode:?} {m:?}: Esc in find's field dismisses the notice and keeps find");
                assert_eq!(focused(&rt).as_deref(), Some("find-input"), "{mode:?} {m:?}: and find's field keeps the keyboard");
                key(&mut rt, Key::Escape, m);
                assert_eq!(state(&mut rt), (false, false, mode), "{mode:?} {m:?}: the next Esc closes find");
                assert_eq!(focused(&rt).as_deref(), Some(note), "{mode:?} {m:?}: and gives the note the keyboard");
                if mode == Mode::Source {
                    notice(&mut rt);
                    key(&mut rt, Key::Escape, m);
                    assert_eq!(state(&mut rt), (false, false, mode), "{m:?}: Esc in the note dismisses the notice first");
                    assert_eq!(focused(&rt).as_deref(), Some("editor"), "{m:?}: and the note keeps the keyboard");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MODESC2: Esc with Shift, Ctrl or Alt does what plain Esc does wherever Esc does
    /// something: the Search drawer's field, a backlink, a result in the drawer, the outline, a sidebar
    /// row, a panel tab, the name and new folder fields, the palette. Each place is set up
    /// afresh for each key, and what is open, where the keyboard is and what is being named
    /// after it must match plain Esc's.
    #[test]
    fn esc_with_a_modifier_is_esc_everywhere() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-modesc2-{}", std::process::id()));
        type Setup = fn(&mut Runtime<App>);
        let places: [(&str, Mode, Setup); 10] = [
            ("the Search drawer's field", Mode::Source, |rt| {
                rt.set_viewport((640.0, 480.0), 1.0);
                settle(rt);
                rt.update(|s, tx| s.focus_editor(tx));
                key(rt, Key::Command('f'), ctrl(true));
                key(rt, Key::Text("text".into()), Mods::default());
            }),
            ("a backlink", Mode::Preview, |rt| {
                rt.update(|_, tx| tx.focus("bl-0", true));
                settle(rt);
            }),
            ("the outline", Mode::Preview, |rt| {
                rt.update(|_, tx| tx.focus("ol-1", true));
                settle(rt);
            }),
            ("a result in the drawer", Mode::Source, |rt| {
                rt.set_viewport((640.0, 480.0), 1.0);
                settle(rt);
                rt.update(|s, tx| {
                    s.query = rune::TextEdit::new("#");
                    s.open_layer(crate::layer::Layer::Drawer(crate::app::Drawer::Sidebar), tx);
                });
                settle(rt);
                rt.update(|_, tx| tx.focus("res-tag:kept", true));
                settle(rt);
            }),
            ("a sidebar row", Mode::Source, |rt| {
                rt.update(|s, tx| tx.focus(s.sidebar_stop(), true));
                settle(rt);
            }),
            ("a section label", Mode::Preview, |rt| {
                rt.update(|_, tx| tx.focus("label-shelf", true));
                settle(rt);
            }),
            ("the tags button in the drawer", Mode::Preview, |rt| {
                rt.set_viewport((640.0, 480.0), 1.0);
                settle(rt);
                rt.update(|s, tx| s.open_layer(crate::layer::Layer::Drawer(crate::app::Drawer::Sidebar), tx));
                settle(rt);
                rt.update(|_, tx| tx.focus("foot-tags", true));
                settle(rt);
            }),
            ("the name field", Mode::Source, |rt| {
                rt.update(|s, tx| s.start_rename(tx));
                settle(rt);
            }),
            ("the new folder's field", Mode::Source, |rt| {
                rt.update(|s, tx| s.start_new_folder("", tx));
                settle(rt);
            }),
            ("the palette", Mode::Source, |rt| {
                rt.update(|s, tx| s.open_palette(crate::app::PalMode::Files, tx));
                settle(rt);
            }),
        ];
        let state = |rt: &mut Runtime<App>| {
            let mut seen = String::new();
            rt.update_if(|s, _| {
                let layers: Vec<_> = s.open_layers().map(crate::layer::Layer::kind).collect();
                seen = format!("layers {layers:?}, naming {} {}, find {}, {:?}, query {:?}", s.renaming.is_some(), s.new_folder.is_some(), s.reader.find.open, s.mode, s.query.text);
                false
            });
            (seen, focused(rt))
        };
        let shift = Mods { shift: true, ..Mods::default() };
        let alt = Mods { alt: true, ..Mods::default() };
        for (place, mode, setup) in places {
            let mut want = None;
            for m in [Mods::default(), shift, ctrl(false), alt] {
                let mut rt = garden(&dir, mode);
                setup(&mut rt);
                let before = state(&mut rt);
                assert!(before.1.as_deref().is_some_and(|k| k != "editor" && k != "note-keys"), "{place}: the setup puts the keyboard there: {before:?}");
                key(&mut rt, Key::Escape, m);
                let after = state(&mut rt);
                match &want {
                    None => {
                        assert_ne!(before, after, "{place}: Esc does something there");
                        want = Some(after);
                    }
                    Some(w) => assert_eq!(&after, w, "{place}: Esc with {m:?} does what Esc does"),
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRESC: in a drawer the first Esc dismisses a showing message and the keyboard stays where
    /// it was in the drawer; the next Esc closes the drawer. From a row, a panel tab, the Search
    /// field and the outline's drawer, at 640 wide. A modal layer still takes Esc first: over
    /// the palette, Esc closes the palette and the message stays.
    #[test]
    fn in_a_drawer_esc_takes_the_message_first() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-dresc-{}", std::process::id()));
        use crate::app::Drawer;
        let state = |rt: &mut Runtime<App>| {
            let mut seen = (false, vec![]);
            rt.update_if(|s, _| {
                seen = (s.notice.is_some(), s.open_layers().map(crate::layer::Layer::kind).collect::<Vec<_>>());
                false
            });
            seen
        };
        let places: [(&str, Drawer, Option<&str>); 6] = [
            ("a row", Drawer::Sidebar, Some("file:note.md")),
            ("a button", Drawer::Sidebar, Some("foot-settings")),
            ("a section label", Drawer::Sidebar, Some("label-vault")),
            ("the Search field", Drawer::Sidebar, Some("search")),
            ("the drawer itself", Drawer::Sidebar, None),
            ("the outline's drawer", Drawer::Outline, Some("ol-1")),
        ];
        // Esc with Shift, Ctrl or Alt is Esc from every one of them too.
        let first = [Mods::default(), Mods { shift: true, ..Mods::default() }, Mods { cmd: true, ..Mods::default() }, Mods { alt: true, ..Mods::default() }];
        for ((place, drawer, at), esc) in places.into_iter().flat_map(|p| first.map(|m| (p, m))) {
            let place = format!("{place}, Esc with {esc:?}");
            let mut rt = garden(&dir, Mode::Source);
            rt.set_viewport((640.0, 800.0), 1.0);
            settle(&mut rt);
            rt.update(|s, tx| s.open_layer(crate::layer::Layer::Drawer(drawer), tx));
            settle(&mut rt);
            if let Some(k) = at {
                rt.update(|_, tx| tx.focus(k, true));
                settle(&mut rt);
            }
            let held = focused(&rt);
            assert!(at.is_none() || held.as_deref() == at, "{place}: the setup puts the keyboard there: {held:?}");
            rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
            settle(&mut rt);
            let open = crate::layer::Kind::Drawer(drawer);
            assert_eq!(state(&mut rt), (true, vec![open]), "{place}: the message shows over the open drawer");
            key(&mut rt, Key::Escape, esc);
            assert_eq!(state(&mut rt), (false, vec![open]), "{place}: the first Esc dismisses the message and the drawer stays");
            assert_eq!(focused(&rt), held, "{place}: and the keyboard stays where it was");
            key(&mut rt, Key::Escape, Mods::default());
            assert_eq!(state(&mut rt), (false, vec![]), "{place}: the next Esc closes the drawer");
        }
        // The palette is no drawer: it takes Esc before the message.
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Files, tx));
        settle(&mut rt);
        rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
        settle(&mut rt);
        key(&mut rt, Key::Escape, Mods::default());
        assert_eq!(state(&mut rt), (true, vec![]), "Esc closes the palette and leaves the message");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MODESC: on the note in preview, Esc with Shift, Ctrl or Alt runs the reader's Esc order
    /// as Esc does: a showing notice goes, then find closes, then zen ends, staying in preview.
    #[test]
    fn esc_with_a_modifier_is_esc_on_the_reader_note() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-modesc-{}", std::process::id()));
        let state = |rt: &mut Runtime<App>| {
            let mut seen = (false, false, false, Mode::Source);
            rt.update_if(|s, _| {
                seen = (s.notice.is_some(), s.reader.find.open, s.focus, s.mode);
                false
            });
            seen
        };
        for m in [Mods { shift: true, ..Mods::default() }, ctrl(false), Mods { alt: true, ..Mods::default() }] {
            let mut rt = garden(&dir, Mode::Preview);
            key(&mut rt, Key::Command('f'), ctrl(false));
            rt.update(|s, tx| {
                s.notice = Some("A folder named Docs is already there.".into());
                s.focus = true;
                s.keys_to_note(tx);
            });
            settle(&mut rt);
            assert_eq!(focused(&rt).as_deref(), Some("note-keys"), "{m:?}: the note has the keyboard");
            key(&mut rt, Key::Escape, m);
            assert_eq!(state(&mut rt), (false, true, true, Mode::Preview), "{m:?}: Esc dismisses the notice first");
            key(&mut rt, Key::Escape, m);
            assert_eq!(state(&mut rt), (false, false, true, Mode::Preview), "{m:?}: then closes find");
            key(&mut rt, Key::Escape, m);
            assert_eq!(state(&mut rt), (false, false, false, Mode::Preview), "{m:?}: then leaves zen");
            key(&mut rt, Key::Escape, m);
            assert_eq!(state(&mut rt), (false, false, false, Mode::Preview), "{m:?}: and stays in preview");
            assert_eq!(focused(&rt).as_deref(), Some("note-keys"), "{m:?}: keeping note focus");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A long name being typed in the title bar stays in the title's room: the field stops at
    /// the room less ".md", the name scrolls inside it, and the caret and ".md" stay in view, at
    /// 640 and 1280, in both modes. A short name keeps its own small field (RENFIT).
    #[test]
    fn a_long_name_stays_in_the_title_bar() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-rename-fit-{}", std::process::id()));
        let el = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("no {k}"));
        let long = "A very long name for a note that keeps going and going on and on past every edge there is";
        for mode in [Mode::Source, Mode::Preview] {
            for w in [640.0, 1280.0] {
                let mut rt = garden(&dir, mode);
                rt.set_viewport((w, 800.0), 1.0);
                rt.update(|s, tx| s.start_rename(tx));
                settle(&mut rt);
                key(&mut rt, Key::Text("A".into()), Mods::default());
                settle(&mut rt);
                let short = el(&rt, "rename-box").rect;
                for c in long.chars().skip(1) {
                    key(&mut rt, Key::Text(c.to_string()), Mods::default());
                }
                settle(&mut rt);
                let (title, field) = (el(&rt, "title").rect, el(&rt, "rename"));
                let room = title[0] + title[2];
                let ext = rt.elements().into_iter().find(|e| e.text.as_deref() == Some(".md") && e.rect[1] < 40.0).expect("the .md after the name").rect;
                assert_eq!(field.text.as_deref(), Some(long), "{mode:?} {w}: the name was typed");
                assert!(el(&rt, "rename-box").rect[0] + el(&rt, "rename-box").rect[2] <= room && ext[0] + ext[2] <= room, "{mode:?} {w}: the field and .md {ext:?} stay in the title's room, to x {room}");
                let caret = field.caret.expect("the field's caret");
                assert!(caret[0] >= field.rect[0] && caret[0] + caret[2] <= field.rect[0] + field.rect[2], "{mode:?} {w}: the caret {caret:?} shows in the field {:?}", field.rect);
                assert!((short[2] - (8.0 * 12.0 * 0.6 + 10.0)).abs() < 0.1, "{mode:?} {w}: a one-letter name keeps the field's least width: {short:?}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-39, SB-41: a section label is a button that says whether its section shows. Enter or
    /// Space folds and unfolds its section with the keyboard staying on it, a click does the
    /// same and leaves the keyboard where it was, and a layer that closes gives the label its
    /// focus back.
    #[test]
    fn a_label_folds_on_enter() {
        use crate::sidebar::Section;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-label-fold-{}", std::process::id()));
        let folded = |rt: &mut Runtime<App>| {
            let mut f = None;
            rt.update_if(|s, _| {
                f = Some(s.persist.sidebar.folded);
                false
            });
            f.unwrap()
        };
        let expanded = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).and_then(|e| e.expanded);
        let mut rt = garden(&dir, Mode::Preview);
        assert_eq!((expanded(&rt, "label-shelf"), expanded(&rt, "label-vault")), (Some(true), Some(true)), "both sections show");
        rt.update(|_, tx| tx.focus("label-vault", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, Mods::default());
        settle(&mut rt);
        assert_eq!(folded(&mut rt), Some(Section::Vault), "Enter folds the vault");
        assert_eq!(expanded(&rt, "label-vault"), Some(false));
        assert_eq!(focused(&rt).as_deref(), Some("label-vault"), "the keyboard stays on the label");
        assert!(rt.find("tree").is_none(), "the folded vault hides its tree");
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        settle(&mut rt);
        assert_eq!(folded(&mut rt), None, "Space unfolds it");
        assert_eq!(focused(&rt).as_deref(), Some("label-vault"));
        // A click folds the shelf and leaves the keyboard on the vault's label.
        let (x, y) = rt.find("label-shelf").expect("the Open label");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert_eq!(folded(&mut rt), Some(Section::Shelf), "a click folds the shelf");
        assert_eq!(focused(&rt).as_deref(), Some("label-vault"), "a click leaves the keyboard where it was");
        // A layer that closes gives the label its focus back.
        rt.update(|_, tx| tx.focus("label-shelf", true));
        settle(&mut rt);
        key(&mut rt, Key::Command('k'), ctrl(false));
        settle(&mut rt);
        assert!(rt.find("pal-input").is_some(), "the palette opened");
        key(&mut rt, Key::Escape, Mods::default());
        settle(&mut rt);
        assert_eq!(focused(&rt).as_deref(), Some("label-shelf"), "closing the palette gives the label its focus back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-33, SB-34, SB-36, SB-38: dragging the handle moves the split, a drag stops at the
    /// limits, and the place is kept for the app. The handle hides while a section is folded.
    #[test]
    fn the_handle_moves_the_split() {
        use crate::sidebar::Section;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-handle-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        // The sample folder's tabs leave the shelf first.
        for _ in 0..60 {
            rt.frame(DT);
        }
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let handle = rect(&rt, "handle").expect("the handle");
        let shelf = rect(&rt, "shelf").expect("the shelf");
        assert_eq!(handle[3], 9.0, "the handle is 9 pt tall");
        assert!((handle[1] - (shelf[1] + shelf[3])).abs() < 0.5, "the handle sits under the shelf: {handle:?} {shelf:?}");
        let drag = |rt: &mut Runtime<App>, dy: f32| {
            let h = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("handle")).unwrap().rect;
            let (x, y) = (h[0] + h[2] / 2.0, h[1] + h[3] / 2.0);
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            for i in 1..=4 {
                rt.pointer_move(x, y + dy * i as f32 / 4.0);
                rt.frame(DT);
            }
            rt.pointer_up(x, y + dy);
            settle(rt);
        };
        drag(&mut rt, 80.0);
        let moved = rect(&rt, "shelf").unwrap();
        assert!((moved[3] - (shelf[3] + 80.0)).abs() < 0.5, "the shelf grows by the drag: {} to {}", shelf[3], moved[3]);
        let mut kept = None;
        rt.update_if(|s, _| {
            kept = Some(s.persist.sidebar.shelf_h);
            false
        });
        assert_eq!(kept.flatten(), Some(moved[3]), "state.json keeps the place");
        // Far down, the vault keeps its label and two rows; far up, the shelf its label and a tab.
        drag(&mut rt, 2000.0);
        let vault = rect(&rt, "vault").unwrap();
        assert!(vault[3] >= 24.0 + 2.0 * 27.0, "the vault keeps its least height: {vault:?}");
        drag(&mut rt, -2000.0);
        let up = rect(&rt, "shelf").unwrap();
        assert!(up[3] >= 24.0 + 35.0 && up[3] <= 84.0, "the shelf keeps its label and a tab: {up:?}");
        rt.update(|s, _| s.persist.sidebar.folded = Some(Section::Shelf));
        settle(&mut rt);
        assert!(rect(&rt, "handle").is_none(), "no handle while a section is folded");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tabs from top to bottom as drawn, once the ones that moved have glided to their
    /// places.
    fn tab_order(rt: &mut Runtime<App>) -> Vec<String> {
        for _ in 0..40 {
            rt.frame(DT);
        }
        let mut tabs: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("tab:"))).collect();
        tabs.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
        tabs.into_iter().filter_map(|e| e.key).collect()
    }

    /// `garden` with three tabs, note.md, Box/a.md and Box/b.md, and `active` shown.
    fn three_tabs(dir: &std::path::Path, active: &'static str) -> Runtime<App> {
        let mut rt = garden(dir, Mode::Preview);
        rt.update(move |s, tx| {
            s.open_file("Box/a.md", None, tx);
            s.open_file("Box/b.md", None, tx);
            s.open_file(active, None, tx);
        });
        settle(&mut rt);
        rt
    }

    fn rect_of(rt: &Runtime<App>, k: &str) -> [f32; 4] {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect).unwrap_or_else(|| panic!("no {k}"))
    }

    /// Drags the tab keyed `tab` by `dy` in six steps; Esc before the release when `esc`.
    fn drag_tab(rt: &mut Runtime<App>, tab: &str, dx: f32, dy: f32, esc: bool) {
        let r = rect_of(rt, tab);
        let (x, y) = (r[0] + 40.0, r[1] + r[3] / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        for i in 1..=6 {
            rt.pointer_move(x + dx * i as f32 / 6.0, y + dy * i as f32 / 6.0);
            rt.frame(DT);
        }
        if esc {
            rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
            rt.frame(DT);
        }
        rt.pointer_up(x + dx, y + dy);
        for _ in 0..30 {
            rt.frame(DT);
        }
    }

    fn tabs_and_active(rt: &mut Runtime<App>) -> (Vec<String>, Option<String>) {
        let mut out = None;
        rt.update_if(|s, _| {
            out = Some((s.tabs.clone(), s.active.clone()));
            false
        });
        out.unwrap()
    }

    /// SB-21: a tab dragged down two places, or up past the top, lands there, and the active
    /// tab and the keyboard stay as they were. Nothing moves on disk, and the tree's own drag
    /// never starts. A press that does not move is a click.
    #[test]
    fn a_dragged_tab_moves() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-drag-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "Box/a.md");
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|a.md", "tab:Box|b.md"]);
        rt.update(|_, tx| tx.focus(crate::focus::NOTE, false));
        settle(&mut rt);
        let before = focused(&rt);
        drag_tab(&mut rt, "tab:note.md", 0.0, 74.0, false);
        assert_eq!(tab_order(&mut rt), ["tab:Box|a.md", "tab:Box|b.md", "tab:note.md"], "down two places");
        assert_eq!(tabs_and_active(&mut rt), (vec!["Box/a.md".into(), "Box/b.md".into(), "note.md".into()], Some("Box/a.md".into())), "the order is the session's, and the active tab stays");
        assert_eq!(focused(&rt), before, "the keyboard stays where it was");
        drag_tab(&mut rt, "tab:Box|b.md", 0.0, -500.0, false);
        assert_eq!(tab_order(&mut rt), ["tab:Box|b.md", "tab:Box|a.md", "tab:note.md"], "up past the top: first");
        // Over the tree's folder, the drop moves the tab and no note.
        let target = rect_of(&rt, "dir:Box");
        let from = rect_of(&rt, "tab:note.md");
        drag_tab(&mut rt, "tab:note.md", 0.0, target[1] - from[1], false);
        assert!(dir.join("notes/note.md").is_file() && !dir.join("notes/Box/note.md").exists(), "no note moved");
        rt.update_if(|s, _| {
            assert!(s.dragging.is_none(), "the tree's drag never started");
            false
        });
        drag_tab(&mut rt, "tab:note.md", 0.0, 2.0, false);
        assert_eq!(tab_order(&mut rt), ["tab:Box|b.md", "tab:Box|a.md", "tab:note.md"], "a press that does not move moves nothing");
        assert_eq!(tabs_and_active(&mut rt).1.as_deref(), Some("note.md"), "it is a click: the tab is shown");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-21: while it is lifted the tab follows the pointer and the tabs it passes step aside;
    /// Esc puts it back, and the release that follows moves nothing.
    #[test]
    fn esc_puts_a_lifted_tab_back() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-esc-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "note.md");
        let (a, b) = (rect_of(&rt, "tab:Box|a.md"), rect_of(&rt, "tab:note.md"));
        let (x, y) = (b[0] + 40.0, b[1] + b[3] / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        for i in 1..=6 {
            rt.pointer_move(x, y + 45.0 * i as f32 / 6.0);
            rt.frame(DT);
        }
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!((rect_of(&rt, "tab:note.md")[1] - (b[1] + 45.0)).abs() < 0.5, "the lifted tab follows the pointer");
        assert!((rect_of(&rt, "tab:Box|a.md")[1] - b[1]).abs() < 0.5, "the tab it passed stepped up into its place: {:?} from {a:?}", rect_of(&rt, "tab:Box|a.md"));
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        rt.pointer_up(x, y + 45.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|a.md", "tab:Box|b.md"], "Esc put it back");
        assert_eq!((rect_of(&rt, "tab:note.md"), rect_of(&rt, "tab:Box|a.md")), (b, a), "both in their places");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-85: Alt+Down and Alt+Up move the focused tab a place, the keyboard stays on it, and
    /// at an end it stays put.
    #[test]
    fn alt_arrows_move_a_tab() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-alt-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "Box/b.md");
        let alt = Mods { alt: true, ..Mods::default() };
        rt.update(|_, tx| tx.focus("tab:note.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Down, alt);
        assert_eq!(tab_order(&mut rt), ["tab:Box|a.md", "tab:note.md", "tab:Box|b.md"], "Alt+Down moves it down a place");
        assert_eq!(focused(&rt).as_deref(), Some("tab:note.md"), "the keyboard stays on it");
        key(&mut rt, Key::Up, alt);
        key(&mut rt, Key::Up, alt);
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|a.md", "tab:Box|b.md"], "Alt+Up moves it back, and stops at the top");
        assert_eq!(focused(&rt).as_deref(), Some("tab:note.md"));
        assert_eq!(tabs_and_active(&mut rt).1.as_deref(), Some("Box/b.md"), "the active tab stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-86, SB-20: Delete or Backspace on a focused tab closes it, and the keyboard goes to the
    /// tab that takes its place: the one above, or below for the first. Closing the active tab
    /// shows the one above. With no tab left, the note has the keyboard.
    #[test]
    fn delete_closes_the_focused_tab() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-delete-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "Box/b.md");
        rt.update(|_, tx| tx.focus("tab:Box|a.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Delete, Mods::default());
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|b.md"]);
        assert_eq!(focused(&rt).as_deref(), Some("tab:note.md"), "the keyboard goes to the tab above");
        assert_eq!(tabs_and_active(&mut rt).1.as_deref(), Some("Box/b.md"), "closing another tab leaves the active one");
        rt.update(|_, tx| tx.focus("tab:Box|b.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Backspace, Mods::default());
        assert_eq!(tabs_and_active(&mut rt), (vec!["note.md".into()], Some("note.md".into())), "the active tab closed: the one above shows");
        assert_eq!(focused(&rt).as_deref(), Some("tab:note.md"));
        key(&mut rt, Key::Delete, Mods::default());
        assert_eq!(tabs_and_active(&mut rt), (vec![], None), "the last tab closed");
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "the note has the keyboard: {:?}", focused(&rt));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-20: closing the active tab by its "×" or a middle click shows the tab above it, or the
    /// one below when it was first. Today's rule showed the one below. A keyboard that was in
    /// the closed note goes to the note shown; one elsewhere stays.
    #[test]
    fn a_closed_tab_hands_over_to_the_one_above() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-close-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "Box/a.md");
        let close = |rt: &mut Runtime<App>, tab: &str| {
            let x = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("x") && e.path.split('/').any(|s| s == tab)).expect("its close button").rect;
            press(rt, (x[0] + x[2] / 2.0, x[1] + x[3] / 2.0), false);
        };
        rt.update(|_, tx| tx.focus(crate::focus::NOTE, false));
        settle(&mut rt);
        close(&mut rt, "tab:Box|a.md");
        assert_eq!(tabs_and_active(&mut rt), (vec!["note.md".into(), "Box/b.md".into()], Some("note.md".into())), "the tab above shows");
        assert_eq!(focused(&rt).as_deref(), Some(crate::focus::NOTE), "the keyboard in the closed note goes to the one shown");
        rt.update(|_, tx| tx.focus("tab:Box|b.md", false));
        settle(&mut rt);
        let t = rect_of(&rt, "tab:note.md");
        rt.pointer_move(t[0] + 40.0, t[1] + 10.0);
        rt.button_down(t[0] + 40.0, t[1] + 10.0, rune::PointerButton::Middle);
        rt.button_up(t[0] + 40.0, t[1] + 10.0, rune::PointerButton::Middle);
        settle(&mut rt);
        assert_eq!(tabs_and_active(&mut rt), (vec!["Box/b.md".into()], Some("Box/b.md".into())), "the first closed by a middle click: the one below shows");
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|b.md"), "a keyboard outside the note stays where it was");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-24: when the active tab changes, the shelf scrolls just far enough to show it.
    #[test]
    fn the_shelf_scrolls_to_the_active_tab() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-scroll-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        let names: Vec<String> = (1..=8).map(|i| format!("n{i}.md")).collect();
        for n in &names {
            std::fs::write(dir.join("notes").join(n), "# N\n").unwrap();
        }
        rt.set_viewport((1280.0, 560.0), 1.0);
        let all = names.clone();
        rt.update(move |s, tx| {
            s.open_vault(dir_root(s), tx);
            for n in &all {
                s.open_file(n, None, tx);
            }
        });
        for _ in 0..40 {
            rt.frame(DT);
        }
        let shows = |rt: &Runtime<App>, tab: &str| {
            let (scroll, r) = (rect_of(rt, "shelf-scroll"), rect_of(rt, tab));
            r[1] >= scroll[1] - 0.5 && r[1] + r[3] <= scroll[1] + scroll[3] + 0.5
        };
        assert!(!shows(&rt, "tab:note.md"), "more tabs than fit: the first is out of view");
        assert!(shows(&rt, "tab:n8.md"), "the last opened shows");
        rt.update(|s, tx| s.open_file("note.md", None, tx));
        for _ in 0..40 {
            rt.frame(DT);
        }
        assert!(shows(&rt, "tab:note.md"), "the shelf scrolled up to it");
        rt.update(|s, tx| s.open_file("n5.md", None, tx));
        for _ in 0..40 {
            rt.frame(DT);
        }
        assert!(shows(&rt, "tab:n5.md"), "and down to a tab below");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The open folder's root.
    fn dir_root(s: &App) -> std::path::PathBuf {
        s.vault.as_ref().map(|v| v.path.clone()).expect("a folder")
    }

    /// SB-115, SB-130: deleting the active note closes its tab by SB-20's rule, and Undo puts the
    /// tab back at its place, active again.
    #[test]
    fn undo_puts_a_tab_back_at_its_place() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-undo-{}", std::process::id()));
        let mut rt = three_tabs(&dir, "Box/a.md");
        rt.update(|s, _| s.delete_note());
        settle(&mut rt);
        rt.update(|s, _| s.writer.wait(std::time::Duration::from_secs(5)));
        assert_eq!(tabs_and_active(&mut rt), (vec!["note.md".into(), "Box/b.md".into()], Some("note.md".into())), "the tab above shows");
        rt.update(|s, tx| s.undo_delete(tx));
        settle(&mut rt);
        rt.update(|s, _| s.writer.wait(std::time::Duration::from_secs(5)));
        assert_eq!(tabs_and_active(&mut rt), (vec!["note.md".into(), "Box/a.md".into(), "Box/b.md".into()], Some("Box/a.md".into())), "back in its place and shown");
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|a.md", "tab:Box|b.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-15, SB-127, SB-128: a tab is the note's name over its folder, the vault's name for a
    /// note at the root; two notes of one name are told apart by their folders, and a long
    /// name and a long folder stay in the tab.
    #[test]
    fn a_tab_shows_its_name_over_its_folder() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tab-names-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        let deep = "Quarterly planning/for every team/across every region";
        let long = "A moderately long but perfectly normal note title about planning";
        let root = dir.join("notes");
        for p in ["Box/note.md".to_string(), format!("{deep}/{long}.md")] {
            std::fs::create_dir_all(root.join(&p).parent().unwrap()).unwrap();
            std::fs::write(root.join(&p), "# N\n").unwrap();
        }
        let open = format!("{deep}/{long}.md");
        rt.update(move |s, tx| {
            s.open_vault(dir_root(s), tx);
            s.open_file("note.md", None, tx);
            s.open_file("Box/note.md", None, tx);
            s.open_file(&open, None, tx);
        });
        settle(&mut rt);
        let texts = |rt: &Runtime<App>, tab: &str| -> Vec<(String, [f32; 4])> {
            rt.elements().into_iter().filter(|e| !e.exiting && e.path.split('/').any(|s| s == tab)).filter_map(|e| e.text.map(|t| (t, e.rect))).collect()
        };
        let words = |rt: &Runtime<App>, tab: &str| texts(rt, tab).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
        assert_eq!(words(&rt, "tab:note.md"), ["note", "notes"], "a note at the root: the vault's name");
        assert_eq!(words(&rt, "tab:Box|note.md"), ["note", "Box"], "one name, told apart by the folder");
        let key = crate::folders::row_key("tab", &format!("{deep}/{long}.md"));
        let tab = rect_of(&rt, &key);
        let x = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("x") && e.path.split('/').any(|s| s == key)).expect("its close button").rect;
        let parts = texts(&rt, &key);
        assert_eq!(parts.len(), 4, "a name, a folder, separator dot, and the active tab's count: {parts:?}");
        assert_eq!(parts[2].0, "·");
        assert_eq!(parts[3].0, "1 section");
        for (t, r) in &parts {
            assert!(r[0] >= tab[0] && r[0] + r[2] <= x[0] + 0.5, "'{t}' {r:?} stays left of the close button {x:?}");
        }
        assert!(parts[1].1[1] > parts[0].1[1], "the folder is under the name");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A note with three H1s, a Setext heading and an H1 in a code block, which neither counts.
    const SECTIONS: &str = "# One\n\nText.\n\n```\n# Not a heading\n```\n\nSetext\n======\n\n# Two\n\n## Under two\n\nText.\n\n# Three\n\nEnd.\n";

    /// `garden` with Box/many.md (`SECTIONS`) and Box/none.md (no H1) on the shelf after
    /// note.md (one H1), `active` shown.
    fn sectioned(dir: &std::path::Path, mode: Mode, active: &'static str) -> Runtime<App> {
        let mut rt = garden(dir, mode);
        std::fs::write(dir.join("notes/Box/many.md"), SECTIONS).unwrap();
        std::fs::write(dir.join("notes/Box/none.md"), "## Only an H2\n\nText.\n").unwrap();
        rt.update(move |s, tx| {
            s.open_vault(dir_root(s), tx);
            s.open_file("note.md", None, tx);
            s.open_file("Box/many.md", None, tx);
            s.open_file("Box/none.md", None, tx);
            s.open_file(active, None, tx);
            s.mode = mode;
        });
        for _ in 0..30 {
            rt.frame(DT);
        }
        rt
    }

    /// The count a tab's caption ends with, if any.
    fn sections_of(rt: &Runtime<App>, tab: &str) -> Option<String> {
        let all = rt.elements();
        let sec = all.iter().find(|e| !e.exiting && e.key.as_deref() == Some("sections") && e.path.split('/').any(|s| s == tab))?;
        if let Some(t) = &sec.text {
            return Some(t.clone());
        }
        let mut parts: Vec<_> = all.iter().filter(|e| !e.exiting && e.text.is_some() && e.path.starts_with(&sec.path) && e.path != sec.path).collect();
        parts.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
        Some(parts.into_iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join(" "))
    }

    /// SB-17, SB-27: the active tab's caption counts its note's H1s, "1 section" for one and
    /// nothing for none. A Setext heading and an H1 in a code block do not count, and no other
    /// tab has the count.
    #[test]
    fn the_active_tab_counts_its_sections() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-count-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        assert_eq!(sections_of(&rt, "tab:Box|many.md").as_deref(), Some("· 3 sections"));
        assert_eq!((sections_of(&rt, "tab:note.md"), sections_of(&rt, "tab:Box|none.md")), (None, None), "no other tab has a count");
        rt.update(|s, tx| s.open_file("note.md", None, tx));
        settle(&mut rt);
        assert_eq!(sections_of(&rt, "tab:note.md").as_deref(), Some("· 1 section"));
        assert_eq!(sections_of(&rt, "tab:Box|many.md"), None);
        rt.update(|s, tx| s.open_file("Box/none.md", None, tx));
        settle(&mut rt);
        assert_eq!(sections_of(&rt, "tab:Box|none.md"), None, "a note with no H1 has no count");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The outline's entries under the active tab, top to bottom, by their text.
    fn entries(rt: &Runtime<App>) -> Vec<String> {
        let all = rt.elements();
        let mut rows: Vec<_> = all.iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("toc:"))).collect();
        rows.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
        rows.into_iter().filter_map(|r| all.iter().find(|e| e.text.is_some() && e.path.starts_with(&r.path) && e.path != r.path).and_then(|e| e.text.clone())).collect()
    }

    /// Clicks the middle of the element keyed `k`, left of any button it holds.
    fn click(rt: &mut Runtime<App>, k: &str) {
        let r = rect_of(rt, k);
        press(rt, (r[0] + 30.0, r[1] + r[3] / 2.0), false);
    }

    /// SB-25, SB-26, SB-27, SB-30: every tab starts folded; a click on the active tab folds its
    /// H1s out under it, indented beside a guide line, and the next folds them back. The keyboard
    /// stays where it was.
    #[test]
    fn the_active_tab_folds_its_outline_out() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-fold-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        assert!(entries(&rt).is_empty(), "the outline starts folded");
        let before = focused(&rt);
        click(&mut rt, "tab:Box|many.md");
        assert_eq!(entries(&rt), ["One", "Two", "Three"], "its H1s, in order");
        assert_eq!(focused(&rt), before, "the keyboard stays where it was");
        let (tab, toc, first) = (rect_of(&rt, "tab:Box|many.md"), rect_of(&rt, "toc"), rect_of(&rt, "toc:0"));
        assert!(toc[1] >= tab[1] + tab[3] && toc[1] - (tab[1] + tab[3]) < 2.0, "right under its tab: {toc:?} under {tab:?}");
        assert!((toc[0] - (tab[0] + 16.0)).abs() < 0.5, "its guide 16 in from the tab: {toc:?}, tab {tab:?}");
        assert!(first[0] - 5.0 > toc[0] + 1.0, "the entry's 5 pt focus ring clears the 1 pt guide line: entry {first:?}, toc {toc:?}");
        assert!(first[3] >= 24.0, "an entry is a pointer target: {first:?}");
        assert!(rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("toc:"))).all(|e| e.fill.a == 0.0), "no entry is filled");
        let next = rect_of(&rt, "tab:Box|none.md");
        assert!(next[1] >= toc[1] + toc[3], "the next tab moved down under it");
        click(&mut rt, "tab:Box|many.md");
        assert!(entries(&rt).is_empty(), "the next click folds it back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-26: a click on another tab only switches to it and folds the outline that was out;
    /// back on the first tab it is folded still. Closing the tab folds it too.
    #[test]
    fn switching_tabs_folds_the_outline() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-switch-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        click(&mut rt, "tab:Box|many.md");
        assert_eq!(entries(&rt).len(), 3);
        click(&mut rt, "tab:note.md");
        assert!(entries(&rt).is_empty(), "the other tab shows, its outline folded");
        click(&mut rt, "tab:Box|many.md");
        assert!(entries(&rt).is_empty(), "a click back only switches: the outline stays folded");
        click(&mut rt, "tab:Box|many.md");
        assert_eq!(entries(&rt).len(), 3);
        rt.update(|s, _| s.close_tab("Box/many.md"));
        settle(&mut rt);
        rt.update(|s, tx| s.open_file("Box/many.md", None, tx));
        settle(&mut rt);
        assert!(entries(&rt).is_empty(), "closed and opened again: folded");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-22: a drag that lifts a tab folds the outline back, and a tab lifted from under it stays
    /// under the pointer as the tabs above close up.
    #[test]
    fn a_lift_folds_the_outline() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-lift-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        click(&mut rt, "tab:Box|many.md");
        assert_eq!(entries(&rt).len(), 3);
        let b = rect_of(&rt, "tab:Box|none.md");
        let (x, y) = (b[0] + 40.0, b[1] + b[3] / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        for i in 1..=6 {
            rt.pointer_move(x, y - 30.0 * i as f32 / 6.0);
            rt.frame(DT);
        }
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(entries(&rt).is_empty(), "the lift folded the outline");
        let held = rect_of(&rt, "tab:Box|none.md");
        assert!((held[1] - (b[1] - 30.0)).abs() < 0.5, "the lifted tab stays under the pointer: at {} for {}", held[1], b[1] - 30.0);
        rt.pointer_up(x, y - 30.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert_eq!(tab_order(&mut rt), ["tab:note.md", "tab:Box|many.md", "tab:Box|none.md"], "30 points up over the folded shelf: its own place");
        assert!(entries(&rt).is_empty(), "the drop leaves the outline folded");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-29, VER-32: in preview the entry of the section at the view's top is in the accent, none
    /// above the first H1, and it follows a wheel scroll through its live colour with no view run.
    #[test]
    fn the_current_entry_follows_the_scroll() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-follow-{}", std::process::id()));
        let long = format!("Before.\n\n# One\n\n{}# Two\n\n{}# Three\n\n{}", "Text to scroll by.\n\n".repeat(30), "More.\n\n".repeat(30), "End.\n\n".repeat(30));
        let mut rt = garden(&dir, Mode::Preview);
        std::fs::write(dir.join("notes/Box/long.md"), &long).unwrap();
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            s.open_file("Box/long.md", None, tx);
            s.persist.right_open = false;
        });
        glide(&mut rt);
        let lit = |rt: &mut Runtime<App>| {
            let mut out = (None, vec![]);
            rt.update_if(|s, _| {
                let map = s.reader.map.borrow();
                out = (map.shelf.marked, map.shelf.marks.iter().map(|m| m.ink.get() == m.on[1]).collect());
                false
            });
            out
        };
        click(&mut rt, "tab:Box|long.md");
        assert_eq!(lit(&mut rt), (None, vec![false, false, false]), "above the first H1 no entry is marked");
        click(&mut rt, "tab:Box|long.md");
        let (x, y) = rt.find(crate::reader::SCROLL).expect("the page");
        rt.pointer_move(x, y);
        rt.frame(DT);
        for _ in 0..2 {
            rt.wheel(x, y, -120.0);
            rt.frame(DT);
        }
        click(&mut rt, "tab:Box|long.md");
        rt.pointer_move(x, y);
        glide(&mut rt);
        assert_eq!(lit(&mut rt), (Some(0), vec![true, false, false]), "in the first section, its entry is in the accent");
        // Debug builds otherwise force one extra view commit every 0.25 s (`LIVE_ONLY_CHECK`) to
        // audit `Tx::live_only` handlers.
        rt.check_live_only(false);
        let views = crate::ui::VIEWS.with(|n| n.get());
        let mut seen = vec![Some(0)];
        for _ in 0..62 {
            rt.wheel(x, y, -120.0);
            rt.frame(DT);
            let (m, inks) = lit(&mut rt);
            assert_eq!(inks.iter().filter(|&&i| i).count(), m.map_or(0, |_| 1), "one entry in the accent, the marked one: {m:?} {inks:?}");
            if seen.last() != Some(&m) {
                seen.push(m);
            }
        }
        assert_eq!(seen, [Some(0), Some(1), Some(2)], "the mark walked down the sections");
        assert_eq!(crate::ui::VIEWS.with(|n| n.get()), views, "with no view run from the very first wheel step");
        assert!(rt.live_only_warnings().is_empty(), "no stale live-only warnings: {:?}", rt.live_only_warnings());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-27: a note with no H1 has no outline, and a click on its active tab does nothing.
    #[test]
    fn a_note_with_no_h1_has_no_outline() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-none-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/none.md");
        let before = state(&mut rt);
        click(&mut rt, "tab:Box|none.md");
        assert!(entries(&rt).is_empty(), "no outline");
        assert_eq!(state(&mut rt), before, "the click changed nothing");
        click(&mut rt, "tab:note.md");
        click(&mut rt, "tab:note.md");
        assert_eq!(entries(&rt), ["Head"], "a note with one H1 has a one-line outline");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-133: a locked note's tab has no count and no outline while it is locked.
    #[test]
    fn a_locked_note_has_no_outline() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-locked-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        std::fs::write(dir.join("notes/Box/sealed.md"), crate::lock::seal(&crate::lock::new_key("pw"), SECTIONS)).unwrap();
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            s.open_file("Box/sealed.md", None, tx);
        });
        glide(&mut rt);
        assert_eq!(sections_of(&rt, "tab:Box|sealed.md"), None, "no count");
        click(&mut rt, "tab:Box|sealed.md");
        assert!(entries(&rt).is_empty(), "and a click folds nothing out");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-120: in Note mode the active tab counts its sections, and a click folds its outline out
    /// and back, as in Folder mode.
    #[test]
    fn note_mode_folds_the_outline_out() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-note-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let loose = dir.join("loose");
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(loose.join("many.md"), SECTIONS).unwrap();
        let file = loose.canonicalize().unwrap().join("many.md");
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        let f = file.clone();
        rt.update(move |s, tx| {
            assert!(s.start_note(&f));
            s.toggle_focus(tx);
            s.toggle_left(tx);
        });
        glide(&mut rt);
        let tab = crate::folders::row_key("note", &file.to_string_lossy());
        assert_eq!(sections_of(&rt, &tab).as_deref(), Some("· 3 sections"), "its caption counts the H1s");
        assert!(entries(&rt).is_empty(), "the outline starts folded");
        click(&mut rt, &tab);
        assert_eq!(entries(&rt), ["One", "Two", "Three"], "a click on the tab folds its outline out");
        click(&mut rt, &tab);
        assert!(entries(&rt).is_empty(), "and the next folds it back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-44: while the Open shelf is folded to its label and the active tab, clicking the active
    /// tab still folds its outline out under it and clicking again folds it back, with the vault
    /// moving down cleanly beneath the shelf.
    #[test]
    fn the_folded_shelf_folds_its_outline_out() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-folded-shelf-{}", std::process::id()));
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        click(&mut rt, "label-shelf");
        glide(&mut rt);
        assert_eq!(tab_order(&mut rt), ["tab:Box|many.md"], "folded shelf shows only the active tab");
        assert!(entries(&rt).is_empty(), "its outline starts folded");
        let shelf_closed = rect_of(&rt, "shelf");
        click(&mut rt, "tab:Box|many.md");
        glide(&mut rt);
        assert_eq!(entries(&rt), ["One", "Two", "Three"], "clicking the active tab folds its outline out");
        let shelf_open = rect_of(&rt, "shelf");
        let vault_open = rect_of(&rt, "vault");
        assert!(shelf_open[3] > shelf_closed[3] + 70.0, "the folded shelf grows for the three entries: {shelf_closed:?} -> {shelf_open:?}");
        assert!((vault_open[1] - (shelf_open[1] + shelf_open[3])).abs() < 0.5, "the vault sits directly under the folded shelf: {vault_open:?} under {shelf_open:?}");
        assert_eq!(tab_order(&mut rt), ["tab:Box|many.md"], "the shelf stays folded");
        click(&mut rt, "tab:Box|many.md");
        glide(&mut rt);
        assert!(entries(&rt).is_empty(), "clicking again folds the outline back");
        let shelf_back = rect_of(&rt, "shelf");
        assert!((shelf_back[3] - shelf_closed[3]).abs() < 0.5, "and the shelf returns to its folded height: {shelf_back:?} vs {shelf_closed:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-82, SB-83, SB-84: Right on the active tab folds its outline out and Left folds it back,
    /// with the keyboard staying on the tab; Right on another tab does nothing. Up and Down walk
    /// the tabs and the entries between them, Left on an entry goes to its tab, Enter or Space
    /// on the active tab folds, and Enter on an entry jumps and gives the note the keyboard. Esc
    /// on an entry gives the note the keyboard, as on a tab.
    #[test]
    fn keys_walk_and_fold_the_outline() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-keys-{}", std::process::id()));
        let none = Mods::default();
        let mut rt = sectioned(&dir, Mode::Preview, "Box/many.md");
        rt.update(|_, tx| tx.focus("tab:note.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Right, none);
        assert!(entries(&rt).is_empty(), "Right on another tab folds nothing out");
        assert_eq!(focused(&rt).as_deref(), Some("tab:note.md"));
        key(&mut rt, Key::Down, none);
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|many.md"));
        key(&mut rt, Key::Right, none);
        assert_eq!(entries(&rt), ["One", "Two", "Three"], "Right folds the outline out");
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|many.md"), "and the keyboard stays on the tab");
        let mut walk = vec![];
        for _ in 0..4 {
            key(&mut rt, Key::Down, none);
            walk.push(focused(&rt).unwrap_or_default());
        }
        assert_eq!(walk, ["toc:0", "toc:1", "toc:2", "tab:Box|none.md"], "Down walks the entries, then the next tab");
        key(&mut rt, Key::Up, none);
        assert_eq!(focused(&rt).as_deref(), Some("toc:2"), "Up walks back into them");
        key(&mut rt, Key::Left, none);
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|many.md"), "Left on an entry goes to its tab");
        assert_eq!(entries(&rt).len(), 3, "and leaves the outline out");
        key(&mut rt, Key::Left, none);
        assert!(entries(&rt).is_empty(), "Left on the tab folds it back");
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|many.md"));
        key(&mut rt, Key::Down, none);
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|none.md"), "folded, Down skips to the next tab");
        key(&mut rt, Key::Up, none);
        key(&mut rt, Key::Enter, none);
        assert_eq!(entries(&rt).len(), 3, "Enter on the active tab folds out");
        assert_eq!(focused(&rt).as_deref(), Some("tab:Box|many.md"), "and the keyboard stays");
        key(&mut rt, Key::Text(" ".into()), none);
        assert!(entries(&rt).is_empty(), "Space folds back");
        key(&mut rt, Key::Right, none);
        key(&mut rt, Key::Down, none);
        key(&mut rt, Key::Escape, none);
        assert!(matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)), "Esc on an entry gives the note the keyboard: {:?}", focused(&rt));
        let _ = std::fs::remove_dir_all(&dir);
        let mut rt = sectioned(&dir, Mode::Source, "Box/many.md");
        click(&mut rt, "tab:Box|many.md");
        rt.update(|_, tx| tx.focus("toc:2", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, none);
        let mut caret = (0, 0);
        rt.update_if(|s, _| {
            let at = s.active.as_ref().and_then(|a| s.edits.get(a)).map_or(0, |e| e.focus);
            caret = crate::md::line_col(s.active_text(), at);
            false
        });
        assert_eq!(caret.0, SECTIONS.split('\n').position(|l| l == "# Three").unwrap(), "Enter on an entry jumps to its heading");
        assert_eq!(focused(&rt).as_deref(), Some("editor"), "and gives the note the keyboard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-28: a click on an entry scrolls the note to its heading, in both modes: the reader's
    /// top block is the heading's, and the writer's caret goes to its line.
    #[test]
    fn an_entry_jumps_to_its_heading() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toc-jump-{}", std::process::id()));
        let long = format!("# One\n\n{}# Two\n\n{}# Three\n\n{}", "Text to scroll by.\n\n".repeat(60), "More.\n\n".repeat(60), "End.\n\n".repeat(60));
        let mut rt = garden(&dir, Mode::Preview);
        std::fs::write(dir.join("notes/Box/long.md"), &long).unwrap();
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            s.open_file("Box/long.md", None, tx);
        });
        glide(&mut rt);
        click(&mut rt, "tab:Box|long.md");
        let line = |t: &str| long.split('\n').position(|l| l == t).unwrap();
        let offset = |rt: &mut Runtime<App>| {
            for _ in 0..120 {
                rt.frame(DT);
            }
            let mut y = 0.0;
            rt.update_if(|s, _| {
                y = s.reader.scroll.get()[0];
                false
            });
            y
        };
        // Each from the top of a page already laid out, so both land by the same map.
        click(&mut rt, "toc:2");
        offset(&mut rt);
        rt.update(|s, tx| s.jump(0, tx));
        offset(&mut rt);
        click(&mut rt, "toc:2");
        let by_entry = offset(&mut rt);
        rt.update(|s, tx| s.jump(0, tx));
        offset(&mut rt);
        let l3 = line("# Three");
        rt.update(move |s, tx| s.jump(l3, tx));
        let by_jump = offset(&mut rt);
        assert!(by_entry > 1000.0 && (by_entry - by_jump).abs() < 1.0, "the reader goes where the outline panel's jump goes: {by_entry} and {by_jump}");
        rt.update(|s, tx| s.switch_mode(tx));
        glide(&mut rt);
        click(&mut rt, "toc:1");
        let mut caret = (0, 0);
        rt.update_if(|s, _| {
            let at = s.active.as_ref().and_then(|a| s.edits.get(a)).map_or(0, |e| e.focus);
            caret = crate::md::line_col(s.active_text(), at);
            false
        });
        assert_eq!(caret.0, line("# Two"), "the writer's caret goes to the heading");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tree's rows, top to bottom, by key, once rows that moved have come to rest.
    fn tree_rows(rt: &mut Runtime<App>) -> Vec<String> {
        for _ in 0..40 {
            rt.frame(DT);
        }
        let mut rows: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("dir:") || k.starts_with("file:"))).collect();
        rows.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
        rows.into_iter().filter_map(|e| e.key).collect()
    }

    /// The count a folder row shows.
    fn count_of(rt: &Runtime<App>, folder: &str) -> String {
        let row = crate::folders::row_key("dir", folder);
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some("count") && e.path.split('/').any(|s| s == row)).and_then(|e| e.text).unwrap_or_else(|| panic!("no count on {row}"))
    }

    /// SB-54: at the root and in each folder, the subfolders come first and then the notes,
    /// each by name ignoring case.
    #[test]
    fn subfolders_come_first() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-order-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        let root = dir.join("notes");
        for p in ["zeta.md", "alpha/x.md", "Box/Inner/c.md", "Box/apple.md"] {
            std::fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
            std::fs::write(root.join(p), "# N\n").unwrap();
        }
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            s.expanded.extend(["alpha".to_string(), "Box".to_string(), "Box/Inner".to_string()]);
        });
        settle(&mut rt);
        assert_eq!(
            tree_rows(&mut rt),
            ["dir:alpha", "file:alpha|x.md", "dir:Box", "dir:Box|Inner", "file:Box|Inner|c.md", "file:Box|a.md", "file:Box|apple.md", "file:Box|b.md", "file:note.md", "file:zeta.md"]
        );
        assert_eq!([count_of(&rt, "alpha"), count_of(&rt, "Box"), count_of(&rt, "Box/Inner")], ["1", "4", "1"], "a folder counts its subfolders' notes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-51, SB-52: a folder's count follows its notes: a note made in it, moved into it or
    /// deleted from it changes the number at once.
    #[test]
    fn folder_counts_follow_the_notes() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-counts-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        assert_eq!(count_of(&rt, "Box"), "2");
        rt.update(|s, tx| s.new_note_in(Some("Box"), Some("fresh"), tx));
        settle(&mut rt);
        assert_eq!(count_of(&rt, "Box"), "3", "a note made in the folder");
        rt.update(|s, tx| s.move_note("note.md", "Box", tx));
        settle(&mut rt);
        assert_eq!(count_of(&rt, "Box"), "4", "a note moved into it");
        rt.update(|s, _| s.delete_path("Box/a.md"));
        settle(&mut rt);
        assert_eq!(count_of(&rt, "Box"), "3", "a note deleted from it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The rectangle of the element keyed `k` inside the tree row keyed `row`.
    fn rect_in(rt: &Runtime<App>, row: &str, k: &str) -> Option<[f32; 4]> {
        rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(k) && e.path.split('/').any(|s| s == row)).map(|e| e.rect)
    }

    /// The rectangles of the guide lines in the tree row keyed `row`, left to right.
    fn guides_in(rt: &Runtime<App>, row: &str) -> Vec<[f32; 4]> {
        let mut g: Vec<[f32; 4]> = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("guide")) && e.path.split('/').any(|s| s == row)).map(|e| e.rect).collect();
        g.sort_by(|a, b| a[0].total_cmp(&b[0]));
        g
    }

    /// `garden` with a second level: Box holds Inner, which holds c.md, both open.
    fn two_levels(dir: &std::path::Path) -> Runtime<App> {
        let mut rt = garden(dir, Mode::Preview);
        std::fs::create_dir_all(dir.join("notes/Box/Inner")).unwrap();
        std::fs::write(dir.join("notes/Box/Inner/c.md"), "# C\n").unwrap();
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            s.expanded.extend(["Box".to_string(), "Box/Inner".to_string()]);
        });
        for _ in 0..40 {
            rt.frame(DT);
        }
        rt
    }

    /// SB-49, SB-50: each level steps in 21 pt from the rows' text edge. A folder's icon is
    /// 14 pt with its name 7 pt after it; a note's name starts where a folder's icon starts at
    /// its level. An open folder's guide is 1 pt at its icon's centre and runs from under its
    /// row to its last row, through the gaps between rows.
    #[test]
    fn the_tree_steps_in_a_level_at_a_time() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-steps-{}", std::process::id()));
        let rt = two_levels(&dir);
        let x = |row: &str, k: &str| rect_in(&rt, row, k).unwrap_or_else(|| panic!("no {k} in {row}"));
        let text_x = |row: &str| rt.elements().into_iter().find(|e| e.text.is_some() && e.key.is_none() && e.path.split('/').any(|s| s == row)).unwrap_or_else(|| panic!("no name in {row}")).rect[0];
        assert_eq!(x("dir:Box", "folder-open"), [16.0, x("dir:Box", "folder-open")[1], 14.0, 14.0], "a root folder's icon at the rows' text edge");
        assert_eq!(x("dir:Box|Inner", "folder-open")[0], 37.0, "one level in: 21 pt");
        assert_eq!(text_x("dir:Box"), 37.0, "a folder's name 7 pt after its icon");
        assert_eq!(text_x("dir:Box|Inner"), 58.0);
        assert_eq!(text_x("file:note.md"), 16.0, "a note at the root starts at the text edge");
        assert_eq!(text_x("file:Box|a.md"), 37.0, "a note in Box where Inner's icon starts");
        assert_eq!(text_x("file:Box|Inner|c.md"), 58.0, "two levels in");
        let row = |k: &str| rect_of(&rt, k);
        assert!(guides_in(&rt, "dir:Box").is_empty() && guides_in(&rt, "file:note.md").is_empty(), "root rows have no guide");
        let inner = guides_in(&rt, "dir:Box|Inner");
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0][0], 22.0, "Box's guide at its icon's centre");
        assert_eq!((inner[0][1], inner[0][1] + inner[0][3]), (row("dir:Box|Inner")[1], row("file:Box|Inner|c.md")[1]), "through the gap to the next row");
        let c = guides_in(&rt, "file:Box|Inner|c.md");
        assert_eq!(c.iter().map(|g| g[0]).collect::<Vec<_>>(), [22.0, 43.0], "a row two levels in has both guides");
        assert_eq!(c[1][1] + c[1][3], row("file:Box|Inner|c.md")[1] + 27.0, "Inner's guide ends with its last row");
        let b = guides_in(&rt, "file:Box|b.md");
        assert_eq!((b.len(), b[0][1] + b[0][3]), (1, row("file:Box|b.md")[1] + 27.0), "Box's guide ends with its last row");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-129: past eight levels deep a folder name is cut cleanly and never runs into or pushes
    /// its count out of the sidebar.
    #[test]
    fn a_deep_tree_cuts_names_clear_of_the_count() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-deep-{}", std::process::id()));
        let parts: Vec<String> = (1..=10).map(|i| format!("Level{i}LongName")).collect();
        let deep_path = format!("{}/Deep note.md", parts.join("/"));
        let mut rt = garden(&dir, Mode::Preview);
        let full = dir.join("notes").join(&deep_path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "# Deep\n").unwrap();
        rt.update(|s, tx| {
            s.open_vault(dir_root(s), tx);
            let mut rel = String::new();
            for p in &parts {
                if !rel.is_empty() {
                    rel.push('/');
                }
                rel.push_str(p);
                s.expanded.insert(rel.clone());
            }
        });
        settle(&mut rt);
        let side_right = rect_of(&rt, "sidebar")[2] - 1.0;
        let mut rel = String::new();
        for p in &parts {
            if !rel.is_empty() {
                rel.push('/');
            }
            rel.push_str(p);
            let k = crate::folders::row_key("dir", &rel);
            let row_r = rect_of(&rt, &k);
            let count_r = rect_in(&rt, &k, "count").unwrap_or_else(|| panic!("no count in {k}"));
            let add_r = rect_in(&rt, &k, "add").unwrap_or_else(|| panic!("no add in {k}"));
            let head_r = rect_in(&rt, &k, "head").unwrap_or_else(|| panic!("no head in {k}"));
            assert!(count_r[0] + count_r[2] <= side_right - 8.0 + 0.5, "{k} count {count_r:?} stays inside the sidebar");
            assert!(add_r[0] + add_r[2] <= row_r[0] + row_r[2] + 0.5, "{k} add {add_r:?} stays inside row {row_r:?}");
            assert!(head_r[0] + head_r[2] <= add_r[0] + 0.5, "{k} clipped head {head_r:?} stops before count/add {add_r:?}");
            let name_r = rt.elements().into_iter().find(|e| e.text.as_deref() == Some(p.as_str()) && e.path.split('/').any(|s| s == k)).expect("folder name").rect;
            assert!(name_r[2] == 0.0 || name_r[0] + name_r[2] <= add_r[0] + 0.5, "{k} name {name_r:?} stops before count/add {add_r:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DEEPINDENT: startup QUIRE_FOLDER + QUIRE_NOTE expands every ancestor folder and caps
    /// indentation so 12-level nested rows stay readable (>= 48 pt) with guides clear of text.
    #[test]
    fn deep_startup_note_expands_ancestors_and_stays_readable() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-deepindent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault_dir = dir.join("vault");
        let rel = (1..=12).map(|i| format!("d{i:02}")).collect::<Vec<_>>().join("/") + "/Leaf.md";
        let full = vault_dir.join(&rel);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "# Deep Leaf\n\nBody text.\n").unwrap();
        // SAFETY: ENV lock held.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut app = crate::app::init();
        app.open_startup(&vault_dir, Some(&rel));
        let mut rt = Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        settle(&mut rt);
        let leaf_key = crate::folders::row_key("file", &rel);
        assert!(rt.find(&leaf_key).is_some(), "startup note's ancestors are expanded and leaf row shows");
        let leaf_text = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Leaf") && e.path.split('/').any(|s| s == leaf_key)).expect("Leaf label");
        let room = 239.0 - leaf_text.rect[0];
        assert!(room >= 48.0 && leaf_text.rect[2] >= 20.0, "deep leaf keeps readable room: rect={:?}, room={room}", leaf_text.rect);
        for g in guides_in(&rt, &leaf_key) {
            assert!(g[0] + g[2] < leaf_text.rect[0], "guide {g:?} stays left of label {:?}", leaf_text.rect);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DEEPFIX: restoring a saved session with `expanded: []` keeps folders collapsed (SB-134),
    /// while opening the sidebar drawer on a deep expanded startup note scrolls it into view.
    #[test]
    fn deep_fix_preserves_collapsed_restore_and_scrolls_drawer() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-deepfix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault_dir = dir.join("vault");
        let rel = (1..=12).map(|i| format!("d{i:02}")).collect::<Vec<_>>().join("/") + "/Leaf.md";
        let full = vault_dir.join(&rel);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "# Deep Leaf\n\nBody.\n").unwrap();
        let cfg = dir.join("cfg");
        let quire_cfg = cfg.join("quire");
        std::fs::create_dir_all(&quire_cfg).unwrap();
        let state_json = serde_json::json!({
            "seeded": true,
            "last": vault_dir.to_string_lossy(),
            "sessions": {
                vault_dir.to_string_lossy().to_string(): {
                    "tabs": [rel],
                    "active": rel,
                    "recent": [rel],
                    "expanded": []
                }
            }
        });
        std::fs::write(quire_cfg.join("state.json"), state_json.to_string()).unwrap();
        // SAFETY: ENV lock held.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", &cfg);
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::remove_var("QUIRE_FOLDER");
            std::env::remove_var("QUIRE_NOTE");
            std::env::remove_var("QUIRE_STATE");
        }
        let leaf_key = crate::folders::row_key("file", &rel);
        // 1. Ordinary session restore keeps saved `expanded: []` collapsed.
        let app_restore = crate::app::init();
        let mut rt_restore = Runtime::new(app_restore, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt_restore.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        settle(&mut rt_restore);
        assert!(rt_restore.find("dir:d01").is_some(), "top folder d01 shows");
        assert!(rt_restore.find("dir:d01|d02").is_none(), "saved collapsed d01 stays collapsed");
        assert!(rt_restore.find(&leaf_key).is_none(), "deep leaf stays hidden when saved expanded is empty");
        // 2. Explicit startup note expands ancestors and scrolls into view in short 520x400 and 640x480 drawers.
        for (vw, vh) in [(520.0, 400.0), (640.0, 480.0)] {
            let mut app = crate::app::init();
            app.open_startup(&vault_dir, Some(&rel));
            let mut rt = Runtime::new(app, crate::ui::view, (vw, vh), rune::Env { system_dark: false, scale: 1.0 });
            rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
            rt.on_key = Some(crate::keys::on_key);
            settle(&mut rt);
            rt.key(KeyEvent { key: Key::Command('b'), mods: Mods { cmd: true, ctrl: true, shift: true, ..Mods::default() } });
            settle(&mut rt);
            let port = rect_of(&rt, "side-scroll");
            let leaf = rect_of(&rt, &leaf_key);
            assert!(
                leaf[1] >= port[1] - 1.0 && leaf[1] + leaf[3] <= port[1] + port[3] + 1.0,
                "at {vw}x{vh}, deep leaf {leaf:?} must sit inside side-scroll {port:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-49: a click on a folder's row opens or closes it, and its icon turns with it.
    #[test]
    fn a_click_opens_a_folder_and_turns_its_icon() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-click-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        assert!(rect_in(&rt, "dir:Box", "folder-open").is_some() && tree_rows(&mut rt).contains(&"file:Box|a.md".to_string()), "Box is open");
        let r = rect_of(&rt, "dir:Box");
        press(&mut rt, (r[0] + 60.0, r[1] + 14.0), false);
        assert!(rect_in(&rt, "dir:Box", "folder-shut").is_some(), "a click closes it: the icon is shut");
        assert_eq!(tree_rows(&mut rt), ["dir:Box", "file:note.md"], "its notes are hidden");
        press(&mut rt, (r[0] + 60.0, r[1] + 14.0), false);
        assert!(rect_in(&rt, "dir:Box", "folder-open").is_some(), "and opens it again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The opacity of the element keyed `k` inside the tree row keyed `row`.
    fn opacity_in(rt: &Runtime<App>, row: &str, k: &str) -> f32 {
        rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(k) && e.path.split('/').any(|s| s == row)).map(|e| e.opacity).unwrap_or_else(|| panic!("no {k} in {row}"))
    }

    /// SB-51, SB-53, SB-08: a folder's count shows at rest; under the pointer a "+" in a 24 pt
    /// target takes its place, ending where the count ends. A click on it makes a note in the
    /// folder, opened in a tab, and leaves the folder open.
    #[test]
    fn a_folder_shows_plus_on_hover() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-plus-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        rt.pointer_move(700.0, 400.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(opacity_in(&rt, "dir:Box", "count") > 0.99 && opacity_in(&rt, "dir:Box", "add") < 0.01, "at rest the count, no +");
        let row = rect_of(&rt, "dir:Box");
        rt.pointer_move(row[0] + 60.0, row[1] + 14.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(opacity_in(&rt, "dir:Box", "count") < 0.01 && opacity_in(&rt, "dir:Box", "add") > 0.99, "under the pointer the + and no count");
        let add = rect_in(&rt, "dir:Box", "add").unwrap();
        let count = rect_in(&rt, "dir:Box", "count").unwrap();
        assert_eq!((add[2], add[3]), (24.0, 24.0), "a 24 pt target");
        assert_eq!(add[0] + add[2], row[0] + row[2] - 1.5, "nudged 1.5 pt right so the 14 pt + stroke ends with the count");
        assert!((count[0] + count[2] - (add[0] + add[2] - 6.5)).abs() < 0.5, "the count ends where the + stroke ends: {count:?} {add:?}");
        let before = dir.join("notes/Box").read_dir().unwrap().count();
        press(&mut rt, (add[0] + 12.0, add[1] + 12.0), false);
        settle(&mut rt);
        rt.update(|s, tx| s.cancel_rename(tx));
        for _ in 0..40 {
            rt.frame(DT);
        }
        let mut state = None;
        rt.update_if(|s, _| {
            state = Some((s.active.clone().unwrap_or_default(), s.expanded.contains("Box"), s.files().keys().filter(|p| p.starts_with("Box/")).count()));
            false
        });
        let (active, open, notes) = state.unwrap();
        assert!(active.starts_with("Box/"), "the new note is in Box and shown: {active}");
        assert!(open, "the + does not close the folder");
        assert_eq!(notes, before + 1, "one note more in Box");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SIDEBARPOLISH: tag row counts stay right-aligned in one column, shelf dot has >=4pt gap,
    /// and crumb-file has a >=24pt vertical hit target.
    #[test]
    fn sidebar_polish_tag_alignment_dot_gap_and_crumb_target() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-sbpolish-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| {
            let a = s.active.clone().unwrap();
            s.set_text(&a, "# Title\n\nBody #alpha #beta\n".into());
            s.open_file("Box/a.md", None, tx);
            s.set_text("Box/a.md", "# Section\n\nText #alpha\n".into());
        });
        settle(&mut rt);
        // 1. crumb-file height >= 24pt
        let cf = rect_of(&rt, "crumb-file");
        assert!(cf[3] >= 24.0, "crumb-file height >= 24pt: {cf:?}");
        // 2. Shelf caption dot gap >= 4pt and symmetric around the middle dot
        let sec = rect_of(&rt, "sections");
        let dot = rect_of(&rt, "sections-dot");
        let cnt = rect_of(&rt, "sections-count");
        let folder = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Box") && e.rect[0] < 248.0 && (e.rect[1] - sec[1]).abs() < 4.0).expect("shelf folder");
        let left_gap = dot[0] - (folder.rect[0] + folder.rect[2]);
        let right_gap = cnt[0] - (dot[0] + dot[2]);
        assert!(left_gap >= 4.0 && right_gap >= 4.0, "shelf dot gaps >= 4pt: left={left_gap:.2}, right={right_gap:.2}");
        assert!((left_gap - right_gap).abs() <= 1.0, "shelf dot gaps symmetric: left={left_gap:.2}, right={right_gap:.2}");
        // 3. Tag row counts align vertically on right edge (selected vs unselected)
        rt.update(|s, tx| s.toggle_tags(tx));
        settle(&mut rt);
        let counts: Vec<f32> = rt
            .elements()
            .into_iter()
            .filter(|e| e.rect[0] < 248.0 && e.text.as_deref().is_some_and(|t| t.ends_with(" note") || t.ends_with(" notes")))
            .map(|e| e.rect[0] + e.rect[2])
            .collect();
        assert!(counts.len() >= 2, "at least two tag count labels: {counts:?}");
        let spread = counts.iter().copied().fold(f32::NEG_INFINITY, f32::max) - counts.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(spread <= 2.0, "tag count right edges align: spread={spread:.2}, rights={counts:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// PALFOLD: Command Palette snaps its scrollport height to whole rows (zero sliced rows),
    /// reaches opacity >= 0.95 within <= 140 ms, and Settings Shortcuts hides Ctrl+? and splits
    /// multi-chord shortcuts into separate keycap chips.
    #[test]
    fn palette_fold_modal_speed_and_shortcut_chips() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-palfold-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| s.open_palette(crate::app::PalMode::Commands, tx));
        // Measure modal open fade time (each frame is DT = 1/60s ≈ 16.67ms).
        let mut first_ms: Option<f64> = None;
        let mut settled_ms: Option<f64> = None;
        for step in 0..30 {
            rt.frame(DT);
            let t_ms = step as f64 * DT * 1000.0;
            if let Some(layer) = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("pal-layer") && !e.exiting) {
                if first_ms.is_none() && layer.opacity > 0.01 {
                    first_ms = Some(t_ms);
                }
                if first_ms.is_some() && settled_ms.is_none() && layer.opacity >= 0.95 {
                    settled_ms = Some(t_ms);
                }
            }
        }
        let dur = settled_ms.zip(first_ms).map_or(999.0, |(a, b)| a - b);
        assert!(dur <= 140.0, "pal-layer settles to >=0.95 opacity in {dur:.1}ms (<=140ms)");
        // Check no pal-* row is sliced across pal-scroll's bottom edge, and pal-fade clears
        // the text of the last visible command row (PALFOLD + PALFADE).
        let ps = rect_of(&rt, "pal-scroll");
        let bot = ps[1] + ps[3];
        let mut last_row: Option<(String, [f32; 4])> = None;
        for e in rt.elements() {
            if let Some(k) = &e.key
                && k.starts_with("pal-")
                && k[4..].chars().all(|c| c.is_ascii_digit())
            {
                let (ry, rh) = (e.rect[1], e.rect[3]);
                assert!(!(ry < bot - 0.5 && ry + rh > bot + 0.5), "row {k} sliced at fold: {ry}..{} vs bot {bot}", ry + rh);
                if ry + rh <= bot + 0.5 {
                    last_row = Some((k.clone(), e.rect));
                }
            }
        }
        let (last_k, last_r) = last_row.expect("last visible palette row");
        let fade = rect_of(&rt, "pal-fade");
        let last_text = rt
            .elements()
            .into_iter()
            .find(|e| e.text.is_some() && e.path.split('/').any(|seg| seg == last_k))
            .expect("last visible row text");
        assert!(
            fade[1] >= last_text.rect[1] + last_text.rect[3] - 0.5,
            "pal-fade ({fade:?}) must stay below last visible row text ({:?})",
            last_text.rect
        );
        assert!(fade[1] + fade[3] <= last_r[1] + last_r[3] + 0.5, "pal-fade aligns with scrollport bottom");
        // Settings -> Shortcuts hides Ctrl+? and splits alternate chords into separate chips.
        rt.update(|s, tx| {
            s.open_settings(tx);
            s.set_sec = crate::app::Sec::Keys;
        });
        settle(&mut rt);
        for e in rt.elements() {
            if let Some(t) = &e.text {
                assert!(!t.contains("Ctrl+?"), "internal alias Ctrl+? leaked into Settings Shortcuts: {t:?}");
                if t.len() < 45 && (t.contains("Ctrl+") || t.contains("Cmd+") || t.contains("Alt+") || t.contains("Shift+") || t.contains("F1")) {
                    assert!(!t.contains(" or "), "multi-chord shortcut jammed into one chip: {t:?}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-03, SB-55, SB-107: the active tab's note is marked in the tree by bold text in
    /// strong, the active tab's title colour, with no fill. The active tab is the one filled row.
    #[test]
    fn the_active_note_is_bold_not_filled() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-active-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        rt.pointer_move(700.0, 400.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let name = |row: &str| els.iter().find(|e| !e.exiting && e.text.is_some() && e.key.is_none() && e.path.split('/').any(|s| s == row)).unwrap_or_else(|| panic!("no name in {row}")).clone();
        let filled: Vec<&str> = els.iter().filter(|e| !e.exiting && e.fill.a > 0.0 && e.key.as_deref().is_some_and(|k| k.starts_with("tab:") || k.starts_with("file:") || k.starts_with("dir:"))).filter_map(|e| e.key.as_deref()).collect();
        assert_eq!(filled, ["tab:note.md"], "the active tab is the one filled row");
        let (on, off, title) = (name("file:note.md"), name("file:Box|a.md"), name("tab:note.md"));
        assert_eq!(on.text_color, title.text_color, "the active note in strong, as the active tab's title");
        assert_ne!(on.text_color, off.text_color, "another note in Text");
        let width = |t: &str, bold: bool| rune::text::measure(t, &[], crate::ui::styles::nav(12.5, bold), None).0;
        assert!((on.rect[2] - width("note", true)).abs() < 0.5, "bold: {} for {}", on.rect[2], width("note", true));
        assert!((off.rect[2] - width("a", false)).abs() < 0.5, "another note regular: {} for {}", off.rect[2], width("a", false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Presses the tree row keyed `row` and moves the pointer to `to` in six steps; lets go
    /// there when `release`, else holds the note in mid-drag.
    fn drag_row(rt: &mut Runtime<App>, row: &str, to: (f32, f32), release: bool) {
        for _ in 0..40 {
            rt.frame(DT);
        }
        let r = rect_of(rt, row);
        let (x, y) = (r[0] + 40.0, r[1] + r[3] / 2.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        for i in 1..=6 {
            let f = i as f32 / 6.0;
            rt.pointer_move(x + (to.0 - x) * f, y + (to.1 - y) * f);
            rt.frame(DT);
        }
        for _ in 0..4 {
            rt.frame(DT);
        }
        if release {
            rt.pointer_up(to.0, to.1);
            for _ in 0..30 {
                rt.frame(DT);
            }
        }
    }

    /// SB-116: a note dragged onto the vault's label, or onto the tree's empty space under its
    /// rows, moves to the root. While it is held over the label, the label wears the drop
    /// target's mark and the ghost says where the note goes. The shelf takes no drop. Onto a
    /// note of a folder, it goes into that folder, whose row wears the mark.
    #[test]
    fn a_note_dropped_on_the_label_goes_to_the_root() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tree-root-drop-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        let label = rect_of(&rt, "label-vault");
        let over = (label[0] + 80.0, label[1] + label[3] / 2.0);
        drag_row(&mut rt, "file:Box|a.md", over, false);
        assert!(rt.elements().into_iter().find(|e| e.key.as_deref() == Some("label-vault")).is_some_and(|e| e.fill.a > 0.0), "the label is marked as the target");
        let ghost: Vec<String> = rt.elements().into_iter().filter(|e| e.path.split('/').any(|s| s == "drag-ghost")).filter_map(|e| e.text).collect();
        assert_eq!(ghost, ["a", "to the folder root"], "the ghost names the note and where it goes");
        rt.pointer_up(over.0, over.1);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(dir.join("notes/a.md").is_file() && !dir.join("notes/Box/a.md").exists(), "a.md moved to the root");
        assert!(rt.elements().into_iter().find(|e| e.key.as_deref() == Some("label-vault")).is_some_and(|e| e.fill.a == 0.0), "the mark goes with the drop");

        let tree = rect_of(&rt, "tree");
        drag_row(&mut rt, "file:Box|b.md", (tree[0] + 100.0, tree[1] + tree[3] + 40.0), true);
        assert!(dir.join("notes/b.md").is_file(), "b.md dropped on the empty space under the rows went to the root");

        rt.update(|s, tx| s.move_note("b.md", "Box", tx));
        settle(&mut rt);
        let tab = rect_of(&rt, "tab:note.md");
        drag_row(&mut rt, "file:Box|b.md", (tab[0] + 60.0, tab[1] + 10.0), true);
        assert!(dir.join("notes/Box/b.md").is_file(), "the shelf takes no drop");

        let b = rect_of(&rt, "file:Box|b.md");
        drag_row(&mut rt, "file:a.md", (b[0] + 120.0, b[1] + b[3] / 2.0), false);
        let marked = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(k)).is_some_and(|e| e.fill.a > 0.0);
        assert!(marked(&rt, "dir:Box") && !marked(&rt, "label-vault"), "over a note of Box, Box's row is the target");
        rt.pointer_up(b[0] + 120.0, b[1] + b[3] / 2.0);
        for _ in 0..30 {
            rt.frame(DT);
        }
        assert!(dir.join("notes/Box/a.md").is_file(), "a.md dropped on a note of Box went into Box");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// SB-110, SB-111: a right-click on the vault's label, or Shift+F10 on it, opens its menu
    /// where it was opened: the folder's path, Switch folder and New folder. New folder on a
    /// folded vault unfolds it and opens the name field in the tree.
    #[test]
    fn the_vault_menu_opens_from_its_label() {
        use crate::sidebar::Section;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-vault-menu-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Preview);
        let (x, y) = rt.find("label-vault").expect("the vault's label");
        rt.pointer_move(x, y);
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt);
        let ([mx, my, _, _], _) = rt.presented("vault-menu").expect("a right-click opens the menu");
        assert!((mx - x).abs() < 1.0 && (my - y).abs() < 1.0, "at the pointer: {mx},{my} for {x},{y}");
        let items: Vec<String> = rt.elements().into_iter().filter(|e| e.path.contains("vault-menu") && e.key.as_deref().is_some_and(|k| k.starts_with("menu-"))).filter_map(|e| e.key).collect();
        assert_eq!(items, ["menu-switch", "menu-new-folder"], "Switch folder and New folder, no Settings");
        key(&mut rt, Key::Escape, Mods::default());
        settle(&mut rt);
        rt.update(|s, _| s.persist.sidebar.folded = Some(Section::Vault));
        settle(&mut rt);
        rt.update(|_, tx| tx.focus("label-vault", true));
        settle(&mut rt);
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        settle(&mut rt);
        assert!(rt.presented("vault-menu").is_some(), "Shift+F10 opens it too");
        rt.update(|_, tx| tx.focus("menu-new-folder", true));
        key(&mut rt, Key::Enter, Mods::default());
        settle(&mut rt);
        let mut folded = Some(Section::Vault);
        rt.update_if(|s, _| {
            folded = s.persist.sidebar.folded;
            false
        });
        assert_eq!(folded, None, "New folder unfolds the vault");
        assert_eq!(focused(&rt).as_deref(), Some("new-folder"), "and the name field has the keyboard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The pointer anywhere in a Settings switch's target, up to its edge, gives the track its
    /// hover fill, on and off, and leaving the target takes it back.
    #[test]
    fn a_switch_shows_its_whole_target_on_hover() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-switch-hover-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        key(&mut rt, Key::Command(','), ctrl(false));
        press_key(&mut rt, "nav-Editor", false);
        glide(&mut rt);
        let el = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("no {k}"));
        let sw = el(&rt, "toolbar-switch");
        let track = |rt: &Runtime<App>| rt.elements().into_iter().find(|e| e.path == format!("{}/track", sw.path)).expect("the switch's track");
        assert_eq!(track(&rt).rect, sw.rect, "the track is the whole target");
        let (x, top) = (sw.rect[0] + sw.rect[2] / 2.0, sw.rect[1]);
        for state in ["on", "off"] {
            rt.pointer_move(x - 200.0, top - 100.0);
            glide(&mut rt);
            let rest = track(&rt).fill;
            rt.pointer_move(x, top + 1.5);
            settle(&mut rt);
            assert_ne!(track(&rt).fill, rest, "{state}: the pointer 1.5 pt inside the target's top edge lights the track");
            rt.pointer_move(x - 200.0, top - 100.0);
            glide(&mut rt);
            assert_eq!(track(&rt).fill, rest, "{state}: leaving the target takes the hover fill back");
            press_key(&mut rt, "toolbar-switch", false);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every switch in Settings is drawn as tall as the 28 pt target it takes a press over, 50
    /// across, with a 24 pt knob 2 pt inside the track that travels 22 pt when it flips.
    #[test]
    fn every_switch_is_drawn_as_big_as_its_target() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-switch-fill-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, _| {
            s.settings_mut().mcp.enabled = true;
            s.settings_mut().mcp.access = "write".into();
            s.settings_mut().sync.provider = "s3".into();
        });
        key(&mut rt, Key::Command(','), ctrl(false));
        glide(&mut rt);
        let find = |rt: &Runtime<App>, path: &str| rt.elements().into_iter().find(|e| e.path == path && !e.exiting).unwrap_or_else(|| panic!("no {path}"));
        let find_key = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap_or_else(|| panic!("no {k}"));
        let mut seen = Vec::new();
        for lbl in ["Editor", "Files & links", "Sync & backup", "MCP"] {
            press_key(&mut rt, &format!("nav-{lbl}"), false);
            glide(&mut rt);
            let switches: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.ends_with("-switch"))).collect();
            assert!(!switches.is_empty(), "{lbl} shows a switch");
            for k in switches.into_iter().filter_map(|e| e.key) {
                // Tab's way there scrolls the switch into view; Space flips it.
                rt.update(|_, tx| tx.focus(&k, true));
                glide(&mut rt);
                let sw = find_key(&rt, &k);
                let track = find(&rt, &format!("{}/track", sw.path));
                let knob_of = |rt: &Runtime<App>| find(rt, &format!("{}/track/knob", sw.path));
                let knob = knob_of(&rt);
                assert_eq!(sw.rect[2..], [50.0, 28.0], "{k}: a 50 by 28 target");
                assert_eq!(track.rect, sw.rect, "{k}: the track is the whole target");
                assert_eq!(knob.rect[2..], [24.0, 24.0], "{k}: a 24 pt knob");
                assert_eq!(knob.rect[1] - track.rect[1], 2.0, "{k}: the knob sits 2 pt inside the track");
                let at = |rt: &Runtime<App>| knob_of(rt).rect[0] - find(rt, &format!("{}/track", sw.path)).rect[0];
                let before = at(&rt);
                assert!(before == 2.0 || before == 24.0, "{k}: the knob rests 2 pt from an end, not {before}");
                key(&mut rt, Key::Text(" ".into()), Mods::default());
                glide(&mut rt);
                assert_eq!(at(&rt), 26.0 - before, "{k}: the knob travels to the other end");
                key(&mut rt, Key::Text(" ".into()), Mods::default());
                glide(&mut rt);
                assert_eq!(at(&rt), before, "{k}: and back");
                seen.push(k);
            }
        }
        for k in ["toolbar-switch", "links-switch", "enc-switch", "mcp-switch"] {
            assert!(seen.iter().any(|s| s == k), "{k} was measured: {seen:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// While a toast shows, the line being typed stays above the toast's band, fade included:
    /// typing on the last line the view shows scrolls the few points it needs, the current find
    /// match clears it too, and the top of the view does not change (RD-104).
    #[test]
    fn the_line_being_typed_stays_above_a_toast() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-toast-caret-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let (mut rt, _) = crate::bench::open(&dir, &"A line of the note.\n\n".repeat(200), Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        key(&mut rt, Key::End, ctrl(false));
        glide(&mut rt);
        rt.update(|s, tx| s.toast("Saved", tx));
        settle(&mut rt);
        let pill = rect(&rt, "toast").expect("the toast shows");
        // The wheel puts the caret's line on the last line the view shows, under the band.
        let bottom = |rt: &Runtime<App>| rect(rt, "caret-at").map_or(f32::NAN, |r| r[1] + r[3]);
        let want = pill[1] - 16.0 + 10.0;
        let dy = want - bottom(&rt);
        rt.wheel(640.0, 400.0, dy);
        glide(&mut rt);
        if (bottom(&rt) - want).abs() > (dy.abs() - 1.0).max(1.0) {
            rt.wheel(640.0, 400.0, -2.0 * dy);
            glide(&mut rt);
        }
        let before = bottom(&rt);
        assert!(before > pill[1] - 16.0 && before < pill[1], "the case puts the caret's line under the band: caret to {before}, toast {pill:?}");
        for c in ["Z", "Z", "Z"] {
            key(&mut rt, Key::Text(c.into()), Mods::default());
        }
        glide(&mut rt);
        // The band is solid from 16 px above the pill and fades over the 32 px above that.
        let fade_top = pill[1] - 16.0 - 32.0;
        let caret = rect(&rt, "caret-at").expect("the caret");
        assert!(caret[1] + caret[3] <= fade_top, "the caret's line clears the band's fade: caret {caret:?}, toast {pill:?}");
        assert!(caret[1] + caret[3] > fade_top - 40.0, "only the few points it needs: caret {caret:?}, toast {pill:?}");
        // The current find match clears it too.
        key(&mut rt, Key::Command('f'), ctrl(false));
        for c in ["Z", "Z", "Z"] {
            key(&mut rt, Key::Text(c.into()), Mods::default());
        }
        rt.update(|s, tx| s.toast("Saved", tx));
        key(&mut rt, Key::Enter, Mods::default());
        glide(&mut rt);
        assert!(rect(&rt, "toast").is_some(), "the toast still shows");
        let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
        assert!(y + 12.0 <= fade_top, "the current match clears the band: match at y {y}, fade from {fade_top}");

        // Nothing changes at the top of the view while a toast shows: typing on a line under
        // the formatting toolbar's band scrolls it to the same place as without one (TBAND2).
        let top = |toast: bool| -> (f32, f32) {
            let (mut rt, _) = crate::bench::open(&dir, &"A line of the note.\n\n".repeat(200), Mode::Source);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((1280.0, 800.0), 1.0);
            rt.update(|s, tx| {
                s.cur_edit().set_selection(40 * 21, 40 * 21);
                s.focus_editor(tx);
            });
            glide(&mut rt);
            if toast {
                rt.update(|s, tx| s.toast("Saved", tx));
                settle(&mut rt);
            }
            let view = rect(&rt, "doc-scroll").expect("the writer's view");
            let caret_top = |rt: &Runtime<App>| rect(rt, "caret-at").map_or(f32::NAN, |r| r[1]);
            rt.wheel(640.0, 400.0, view[1] + 30.0 - caret_top(&rt));
            glide(&mut rt);
            let before = caret_top(&rt);
            key(&mut rt, Key::Text("Z".into()), Mods::default());
            glide(&mut rt);
            (before, caret_top(&rt))
        };
        let (plain, with_toast) = (top(false), top(true));
        assert!(plain.1 > plain.0 + 5.0, "the case puts the caret's line under the toolbar's band: {plain:?}");
        assert!((with_toast.0 - plain.0).abs() < 0.5 && (with_toast.1 - plain.1).abs() < 0.5, "the top edge with a toast {with_toast:?}, without {plain:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// HUG: a message that wraps is as wide as its longest line needs, plus its padding and
    /// Dismiss, centred in its room, on two, three or four lines; one on one line is unchanged.
    #[test]
    fn a_wrapped_message_hugs_its_longest_line() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-hug-{}", std::process::id()));
        let style = crate::ui::styles::popup();
        let line = rune::text::measure("A", &[], style, None).1;
        let messages = [
            ("A folder named Docs is already there.", 1.0),
            ("A folder named Quarterly planning notes for the product team is already there.", 2.0),
            ("A folder named Quarterly planning notes for the product and design teams across every region of the world, kept since the first release, is there.", 3.0),
            ("A folder named Quarterly planning notes for the product and design teams across every region of the world, kept since the very first release of the app and never once tidied since then, is already there.", 4.0),
        ];
        for mode in [Mode::Source, Mode::Preview] {
            let mut rt = garden(&dir, mode);
            rt.set_viewport((520.0, 600.0), 1.0);
            glide(&mut rt);
            for (msg, lines) in messages {
                rt.update(|s, _| s.notice = Some(msg.into()));
                glide(&mut rt);
                let get = |k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect).unwrap_or_else(|| panic!("{k} shows"));
                let (n, x, area) = (get("notice"), get("notice-x"), get("doc-area"));
                let t = rt.elements().into_iter().find(|e| e.text.as_deref() == Some(msg)).expect("the message").rect;
                assert_eq!((t[3] / line).round(), lines, "{mode:?}: '{msg}' takes {lines} line(s): {t:?}");
                // The room the message wraps in: the note area's, less 16 pt at each side, the
                // pill's padding, the gap and Dismiss.
                let room = area[2] - 32.0 - 24.0 - x[2];
                if lines > 1.0 {
                    assert!(t[2] <= room - 0.5, "{mode:?}: '{msg}' is {} wide in a room of {room}: it takes its longest line's width", t[2]);
                }
                assert!((n[2] - (14.0 + t[2] + 6.0 + x[2] + 4.0)).abs() < 0.5, "{mode:?}: the pill {n:?} is its text, its padding and Dismiss");
                let mid = area[0] + area[2] / 2.0;
                assert!((n[0] + n[2] / 2.0 - mid).abs() < 0.5, "{mode:?}: the pill {n:?} centres in {area:?}");
                if lines == 1.0 {
                    assert_eq!(n[3], 30.0, "{mode:?}: one line keeps the toast's height");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GROW: a pill that appears, or one that stacks on it, brings what the user is on above
    /// the band at once, with no key, by the least scroll: the caret's line in the writer, the
    /// current find match in either mode. A band that shrinks scrolls nothing back, and a caret
    /// out of view stays out (RD-104).
    #[test]
    fn a_pill_that_appears_brings_the_caret_above_its_band() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-grow-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let offset = |rt: &mut Runtime<App>| {
            let mut at = 0.0;
            rt.update(|s, _| at = s.reader.scroll.get()[0]);
            at
        };
        let text: String = (0..200).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        let line = text.find("Line 100 ").unwrap() + 3;
        rt.update(|s, tx| {
            s.cur_edit().set_selection(line, line);
            s.focus_editor(tx);
        });
        glide(&mut rt);
        let bottom = |rt: &Runtime<App>| rect(rt, "caret-at").map_or(f32::NAN, |r| r[1] + r[3]);
        // The caret's line sits 30 pt above the view's foot: under where a band will come.
        let view = rect(&rt, "doc-scroll").expect("the writer's view");
        rt.wheel(640.0, 400.0, view[1] + view[3] - 30.0 - bottom(&rt));
        glide(&mut rt);
        assert!((bottom(&rt) - (view[1] + view[3] - 30.0)).abs() < 1.0, "the case puts the caret's line low: {}", bottom(&rt));
        rt.update(|s, tx| s.toast("Saved", tx));
        glide(&mut rt);
        let band = rect(&rt, "toast-band").expect("the band");
        let at = bottom(&rt);
        assert!(at <= band[1] + 0.5 && at > band[1] - 1.5, "a toast brings the caret's line just above its band: line to {at}, band from {}", band[1]);
        // A message stacks on the toast: the band grows, and the line comes above it again.
        rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
        glide(&mut rt);
        let grown = rect(&rt, "toast-band").expect("the band");
        let at = bottom(&rt);
        assert!(grown[1] < band[1] - 30.0, "the band grows: {band:?} to {grown:?}");
        assert!(at <= grown[1] + 0.5 && at > grown[1] - 1.5, "a stacked message brings the line above the band: line to {at}, band from {}", grown[1]);
        // The message goes: the band shrinks, and nothing scrolls back.
        let before = offset(&mut rt);
        rt.update(|s, _| s.notice = None);
        glide(&mut rt);
        assert!((offset(&mut rt) - before).abs() < 0.5, "a shrinking band scrolls nothing");
        // The toast goes too; with the caret out of view, the next one scrolls nothing.
        rt.update(|s, _| s.reader.toast = None);
        glide(&mut rt);
        rt.wheel(640.0, 400.0, 2000.0);
        glide(&mut rt);
        assert!(bottom(&rt) > view[1] + view[3] + 40.0, "the case scrolls the caret out of view: {}", bottom(&rt));
        let before = offset(&mut rt);
        rt.update(|s, tx| s.toast("Saved", tx));
        glide(&mut rt);
        assert!((offset(&mut rt) - before).abs() < 0.5, "a caret out of view stays out");

        // The current find match, in both modes, in a window where find leaves it under a
        // stacked band: 520 by 480, the match at 70% of the view.
        for mode in [Mode::Preview, Mode::Source] {
            let (mut rt, _) = crate::bench::open(&dir, &text, mode);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((520.0, 480.0), 1.0);
            rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
            glide(&mut rt);
            rt.update(|s, tx| {
                s.open_find(tx);
                s.reader.find.q = rune::TextEdit::new("Line 120 of");
                s.find_changed(tx);
            });
            glide(&mut rt);
            glide(&mut rt);
            let band = rect(&rt, "toast-band").expect("the band");
            let target_y = band[1] - 24.0;
            // Rows that mount as the view moves measure, so the move is made until it holds, and
            // not again once it does: a row at the view's edge can mount for a move of nothing.
            for _ in 0..4 {
                let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
                let by = y - target_y;
                if by.abs() < 0.5 {
                    break;
                }
                rt.update(|s, tx| {
                    let at = s.reader.scroll.get()[0];
                    tx.scroll_to_instant(crate::reader::SCROLL, 0.0, at + by);
                });
                glide(&mut rt);
            }
            let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
            assert!((y - target_y).abs() < 1.0, "{mode:?}: the case puts the match just above the first band: {y} vs {target_y}");
            assert!(y + 12.0 < band[1], "{mode:?}: the match starts above the message's band: {y}, band {band:?}");
            rt.update(|s, tx| s.toast("Copied the whole note", tx));
            glide(&mut rt);
            let band = rect(&rt, "toast-band").expect("the band");
            let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
            // `find` gives the middle of the match's line, 22 pt tall in the reader.
            assert!(y + 11.0 <= band[1] + 0.5 && y + 11.0 > band[1] - 4.0, "{mode:?}: a stacked toast brings the match just above the band: match at {y}, band {band:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// NOTCHTOAST: a toast that appears while the wheel is turning the page does not move it.
    /// Two notches up at the note's end, then a toast with the caret's line under its band: the
    /// wheel's whole 240 pt arrives and no frame turns the page back. Nothing brings the caret
    /// back until the next key.
    #[test]
    fn a_toast_leaves_a_turning_wheel_alone() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-notchtoast-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let offset = |rt: &mut Runtime<App>| {
            let mut at = 0.0;
            rt.update_if(|s, _| {
                at = s.reader.scroll.get()[0];
                false
            });
            at
        };
        let text: String = (0..200).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        rt.key(KeyEvent { key: Key::End, mods: ctrl(false) });
        glide(&mut rt);
        let notch = || rune::WheelEvent::new(0.0, 120.0).at(640.0, 400.0).precise(false).notch(true);
        let start = offset(&mut rt);
        let mut at = vec![start];
        for i in 0..60 {
            match i {
                0 | 3 => rt.wheel_event(notch()),
                6 => rt.key(KeyEvent { key: Key::Command('s'), mods: ctrl(false) }),
                _ => {}
            }
            rt.frame(DT);
            at.push(offset(&mut rt));
        }
        assert!(rect(&rt, "toast-band").is_some(), "the toast shows");
        let back = at.windows(2).map(|w| w[1] - w[0]).fold(0.0f32, f32::max);
        assert!(back < 0.5, "no frame turns the page back down: {back} pt, {at:?}");
        assert!((start - at[at.len() - 1] - 240.0).abs() < 1.0, "the wheel's whole turn arrives: {start} to {}", at[at.len() - 1]);
        // Nothing brings the caret back while the page rests, not even a message that stacks on
        // the toast; the next key does, above the band.
        let rest = offset(&mut rt);
        rt.update(|s, _| s.notice = Some("A folder named Docs is already there.".into()));
        glide(&mut rt);
        assert!((offset(&mut rt) - rest).abs() < 0.5, "a stacked message leaves the page: {rest} to {}", offset(&mut rt));
        rt.key(KeyEvent { key: Key::Left, mods: Mods::default() });
        glide(&mut rt);
        let caret = rect(&rt, "caret-at").expect("the caret");
        let band = rect(&rt, "toast-band").expect("the band");
        assert!(caret[1] >= 0.0 && caret[1] + caret[3] <= band[1] + 0.5, "the next key brings the caret's line above the band: {caret:?}, band {band:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDREV: with find open in the writer, find's field has the keyboard, so a toast goes by
    /// where the page lays out the current match. The view moves only for a match in view and
    /// under the band, by the least scroll; a match clear of the band or out of view stays put.
    #[test]
    fn a_toast_moves_the_writer_only_for_a_match_under_its_band() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findrev-{}", std::process::id()));
        let text: String = (0..200).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| {
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("Line 120 of");
            s.find_changed(tx);
        });
        glide(&mut rt);
        glide(&mut rt);
        let view = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("doc-scroll")).expect("the writer's view").rect;
        let offset = |rt: &mut Runtime<App>| {
            let mut at = 0.0;
            rt.update(|s, _| at = s.reader.scroll.get()[0]);
            at
        };
        // Puts the match's middle at window y `to`, shows a toast, and gives how far the view
        // moved and where the band starts.
        let mut toast_with_match_at = |to: f32| {
            rt.update(|s, _| s.reader.toast = None);
            glide(&mut rt);
            let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
            rt.update(|s, tx| {
                let at = s.reader.scroll.get()[0];
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, at + y - to);
            });
            glide(&mut rt);
            let before = offset(&mut rt);
            rt.update(|s, tx| s.toast("Saved", tx));
            glide(&mut rt);
            let band = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toast-band") && !e.exiting).expect("the band").rect;
            (offset(&mut rt) - before, band[1], rt.find(crate::reader::FIND_CUR).map(|m| m.1))
        };
        let (moved, ..) = toast_with_match_at(view[1] + 60.0);
        assert!(moved.abs() < 0.5, "a match high in the view stays put: moved {moved}");
        let (moved, ..) = toast_with_match_at(view[1] + view[3] / 2.0);
        assert!(moved.abs() < 0.5, "a match in the middle stays put: moved {moved}");
        let (moved, ..) = toast_with_match_at(view[1] + view[3] + 900.0);
        assert!(moved.abs() < 0.5, "a match out of view below stays out: moved {moved}");
        let (moved, ..) = toast_with_match_at(view[1] - 900.0);
        assert!(moved.abs() < 0.5, "a match out of view above stays out: moved {moved}");
        let (moved, band, y) = toast_with_match_at(view[1] + view[3] - 30.0);
        let y = y.expect("the match is drawn");
        // `find` gives the middle of the match's line, 31 pt tall in the writer at its default size.
        assert!(moved > 0.5 && y < band && y + 20.0 > band, "a match under the band comes just above it: moved {moved}, match at {y}, band from {band}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// BLOCKLIFT: in the reader too a toast goes by the current match's own highlight, not its
    /// block. A match on a paragraph's first line, clear of the band while the paragraph's
    /// later lines run under it, stays put; a match under the band comes just above it, by the
    /// least scroll; a match out of view stays out (RD-104).
    #[test]
    fn a_toast_moves_the_reader_only_for_a_match_under_its_band() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-blocklift-{}", std::process::id()));
        let words = "and the paragraph runs on over more lines ".repeat(8);
        let text: String = (0..80).map(|i| format!("Match {i} stays fast, {words}\n\n")).collect();
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| {
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("Match 40 stays");
            s.find_changed(tx);
        });
        glide(&mut rt);
        glide(&mut rt);
        let view = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("doc-scroll")).expect("the view").rect;
        let offset = |rt: &mut Runtime<App>| {
            let mut at = 0.0;
            rt.update(|s, _| at = s.reader.scroll.get()[0]);
            at
        };
        // Puts the match's middle at window y `to`, shows a toast, and gives how far the view
        // moved, where the band starts and where the match's middle ends.
        let mut toast_with_match_at = |to: f32| {
            rt.update(|s, _| s.reader.toast = None);
            glide(&mut rt);
            // Rows that mount as the view moves measure, so the move is made until it holds.
            for _ in 0..3 {
                let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
                rt.update(|s, tx| {
                    let at = s.reader.scroll.get()[0];
                    tx.scroll_to_instant(crate::reader::SCROLL, 0.0, at + y - to);
                });
                glide(&mut rt);
            }
            let before = offset(&mut rt);
            rt.update(|s, tx| s.toast("Copied the whole note", tx));
            glide(&mut rt);
            let band = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("toast-band") && !e.exiting).expect("the band").rect;
            (offset(&mut rt) - before, band[1], rt.find(crate::reader::FIND_CUR).map(|m| m.1))
        };
        let (moved, band, _) = toast_with_match_at(view[1] + view[3] / 2.0);
        assert!(moved.abs() < 0.5, "a match in the middle stays put: moved {moved}");
        // The match's line, 22 pt tall, ends 20 pt above the band; its paragraph runs under it.
        let (moved, ..) = toast_with_match_at(band - 31.0);
        assert!(moved.abs() < 0.5, "a match clear of the band stays put, whatever its paragraph does: moved {moved}");
        let (moved, ..) = toast_with_match_at(view[1] + view[3] + 900.0);
        assert!(moved.abs() < 0.5, "a match out of view below stays out: moved {moved}");
        let (moved, ..) = toast_with_match_at(view[1] - 900.0);
        assert!(moved.abs() < 0.5, "a match out of view above stays out: moved {moved}");
        let (moved, band, y) = toast_with_match_at(band + 20.0);
        let y = y.expect("the match is drawn");
        assert!(moved > 0.5 && (y + 11.0 - band).abs() < 1.5, "a match under the band comes just above it: moved {moved}, match at {y}, band from {band}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ENDROOM: while a band shows, the page's end room is at least the band's height, so the
    /// last find match comes above a band taller than the reader's room after its last line.
    /// When the band goes, a view inside the extra room rides down with it, and a view above it
    /// does not move (RD-104).
    #[test]
    fn the_last_match_sits_above_a_tall_band_and_its_room_goes_without_a_jump() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-endroom-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let scroll = |rt: &mut Runtime<App>| {
            let mut at = [0.0; 4];
            rt.update(|s, _| at = s.reader.scroll.get());
            at
        };
        let text: String = (0..60).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let long = "A folder named A folder with a name long enough to wrap the message onto a second line is already there.";
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((520.0, 480.0), 1.0);
        rt.update(|s, tx| {
            s.notice = Some(long.into());
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("Line 59 of");
            s.find_changed(tx);
        });
        glide(&mut rt);
        glide(&mut rt);
        let [at, max, ..] = scroll(&mut rt);
        assert!(max > 0.0 && (at - max).abs() < 0.5, "the case puts the reader at the end of its scroll: {at} of {max}");
        rt.update(|s, tx| s.toast("Copied the whole note", tx));
        glide(&mut rt);
        let band = rect(&rt, "toast-band").expect("the band");
        assert!(band[3] > crate::page::bottom_room(480.0) + 8.0, "the case needs a band taller than the page's end room: {band:?}");
        let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the last match is drawn");
        // `find` gives the middle of the match's line, 22 pt tall in the reader.
        assert!(y + 11.0 <= band[1] + 0.5, "the last match comes above the band: match to {}, band from {}", y + 11.0, band[1]);

        // The band goes with the view in its extra room: the view rides down, and never jumps.
        let [at, grown, ..] = scroll(&mut rt);
        rt.update(|s, _| {
            s.reader.toast = None;
            s.notice = None;
        });
        let (mut last, mut step) = (at, 0f32);
        for _ in 0..60 {
            rt.frame(DT);
            let [now, ..] = scroll(&mut rt);
            step = step.max((now - last).abs());
            last = now;
        }
        let [at, max, ..] = scroll(&mut rt);
        assert!(max < grown - 8.0 && (at - max).abs() < 0.5, "the room goes and the view ends at the end of the page: {at} of {max}, was {grown}");
        assert!(step < 6.0, "the view glides out of the room rather than jumping: {step} pt in one frame");

        // Above the extra room, the band coming and going moves nothing.
        rt.update(|_, tx| tx.scroll_to_instant(crate::reader::SCROLL, 0.0, max - 200.0));
        glide(&mut rt);
        let [before, ..] = scroll(&mut rt);
        rt.update(|s, tx| {
            s.notice = Some(long.into());
            s.toast("Copied the whole note", tx);
        });
        glide(&mut rt);
        let [shown, ..] = scroll(&mut rt);
        rt.update(|s, _| {
            s.reader.toast = None;
            s.notice = None;
        });
        let mut moved = (shown - before).abs();
        for _ in 0..60 {
            rt.frame(DT);
            let [now, ..] = scroll(&mut rt);
            moved = moved.max((now - before).abs());
        }
        assert!(moved < 0.5, "a view above the extra room stays put as the band comes and goes: moved {moved}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ROOMREL: the band's extra end room goes a little each frame, however the band goes:
    /// the toast and then the notice 0.1 s later, with or without the wheel still turning down
    /// at the page's end, at 1x and 2x. No frame moves the page more than a glide would, and once
    /// it rests it does not move again (RD-104).
    #[test]
    fn the_extra_end_room_goes_without_a_late_step() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-roomrel-{}", std::process::id()));
        let offset = |rt: &mut Runtime<App>| {
            let mut at = [0.0; 4];
            rt.update(|s, _| at = s.reader.scroll.get());
            at
        };
        let text: String = (0..60).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let long = format!("A folder named {}is already there.", "A long folder name ".repeat(10));
        for scale in [1.0, 2.0] {
            for wheel in [false, true] {
                let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Preview);
                rt.on_key = Some(crate::keys::on_key);
                rt.set_viewport((520.0, 480.0), scale);
                rt.update(|s, tx| {
                    s.notice = Some(long.clone());
                    s.open_find(tx);
                    s.reader.find.q = rune::TextEdit::new("Line 59 of");
                    s.find_changed(tx);
                });
                glide(&mut rt);
                glide(&mut rt);
                rt.update(|s, tx| s.toast("Copied the whole note", tx));
                glide(&mut rt);
                let [start, grown, ..] = offset(&mut rt);
                assert!((start - grown).abs() < 0.5, "{scale}x: the case puts the view at the end of the page: {start} of {grown}");
                let (mut last, mut step, mut still, mut late) = (start, 0f32, 0, 0f32);
                for i in 0..90 {
                    match i {
                        0 => rt.update(|s, _| s.reader.toast = None),
                        6 => rt.update(|s, _| s.notice = None),
                        _ => {}
                    }
                    if wheel && i < 40 {
                        rt.wheel(260.0, 200.0, -40.0);
                    }
                    rt.frame(DT);
                    let [now, ..] = offset(&mut rt);
                    let d = (now - last).abs();
                    step = step.max(d);
                    if still >= 15 {
                        late = late.max(d);
                    }
                    still = if d > 0.01 || i < 7 { 0 } else { still + 1 };
                    last = now;
                }
                let [at, max, ..] = offset(&mut rt);
                assert!(max < grown - 30.0 && (at - max).abs() < 0.5, "{scale}x, wheel {wheel}: the room goes and the view ends at the page's end: {at} of {max}, was {grown}");
                assert!(step <= 5.0, "{scale}x, wheel {wheel}: no frame moves the page more than a glide would: {step} pt");
                assert!(late < 0.01, "{scale}x, wheel {wheel}: the page moves {late} pt after resting");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDTYPE, SAMELINE: a find reveal makes one move, to one place, in both modes. With a
    /// toast's band up, a letter typed that keeps the match and a step to the next hit on its
    /// line move nothing; a search or a step to another line moves the page one way only, and
    /// lands the match between the find bar and the band. After the band goes, a step along the
    /// line still moves nothing.
    #[test]
    fn find_moves_the_page_once_to_one_place() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findtype-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let text: String = (0..80)
            .map(|i| match i {
                30 => "word word word word.\n\n".to_string(),
                50 => "word again.\n\n".to_string(),
                _ => format!("Line {i} of the note.\n\n"),
            })
            .collect();
        // Runs `f` with a toast up (when `toast`), then the frames after it: the page's offset
        // before and at each frame.
        let track = |rt: &mut Runtime<App>, toast: bool, f: &dyn Fn(&mut App, &mut rune::Tx)| {
            let mut at = vec![];
            rt.update(|s, tx| {
                at.push(s.reader.scroll.get()[0]);
                if toast {
                    s.toast("Saved", tx);
                }
                f(s, tx);
            });
            for _ in 0..40 {
                rt.frame(DT);
                rt.update_if(|s, _| {
                    at.push(s.reader.scroll.get()[0]);
                    false
                });
            }
            at
        };
        let travel = |at: &[f32]| at.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>();
        for mode in [Mode::Source, Mode::Preview] {
            let (mut rt, _) = crate::bench::open(&dir, &text, mode);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((1280.0, 800.0), 1.0);
            rt.update(|s, tx| s.toast("Saved", tx));
            glide(&mut rt);
            let search = |q: &'static str| move |s: &mut App, tx: &mut rune::Tx| {
                s.reader.find.q = rune::TextEdit::new(q);
                s.find_changed(tx);
            };
            rt.update(|s, tx| s.open_find(tx));
            let lands = |rt: &Runtime<App>, what: &str| {
                let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
                let (bar, band) = (rect(rt, "find-bar").expect("the find bar"), rect(rt, "toast-band").expect("the band"));
                assert!(y > bar[1] + bar[3] && y < band[1], "{mode:?}, {what}: the match lands between the find bar and the band: {y}, {bar:?}, {band:?}");
            };
            let at = track(&mut rt, true, &search("wo"));
            let net = (at[at.len() - 1] - at[0]).abs();
            assert!(net > 100.0 && travel(&at) <= net + 0.5, "{mode:?}: the search moves the page one way: {net} net, {} travelled", travel(&at));
            lands(&rt, "the search");
            let at = track(&mut rt, true, &search("wor"));
            assert!(travel(&at) < 0.5, "{mode:?}: a letter that keeps the match moves nothing: {at:?}");
            for i in 0..3 {
                let at = track(&mut rt, true, &|s, tx| s.find_step(true, tx));
                assert!(travel(&at) < 0.5, "{mode:?}, step {i}: a step along the line moves nothing: {at:?}");
            }
            let at = track(&mut rt, true, &|s, tx| s.find_step(true, tx));
            let net = (at[at.len() - 1] - at[0]).abs();
            assert!(net > 100.0 && travel(&at) <= net + 0.5, "{mode:?}: a step to another line moves the page one way: {net} net, {} travelled", travel(&at));
            lands(&rt, "the step down");
            let at = track(&mut rt, true, &|s, tx| s.find_step(false, tx));
            let net = (at[at.len() - 1] - at[0]).abs();
            assert!(net > 100.0 && travel(&at) <= net + 0.5, "{mode:?}: a step back moves the page one way: {net} net, {} travelled", travel(&at));
            lands(&rt, "the step back");
            rt.update(|s, _| s.reader.toast = None);
            glide(&mut rt);
            assert!(rect(&rt, "toast-band").is_none(), "{mode:?}: the band has gone");
            let at = track(&mut rt, false, &|s, tx| s.find_step(false, tx));
            assert!(travel(&at) < 0.5, "{mode:?}: after the band goes, a step along the line moves nothing: {at:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDBAND: while a band shows, find places the match it brings up in the room above the
    /// band, in both modes, stepping forward and back; with no band it centres as before. The
    /// reader's shorter line clears the band at 480 high by centring alone, so it runs at 400.
    #[test]
    fn find_lands_its_match_above_a_tall_band() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findband-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let text: String = (0..120).map(|i| format!("Line {i} of the note.\n\n")).collect();
        let long = format!("A folder named {}is already there.", "A long folder name ".repeat(10));
        for mode in [Mode::Source, Mode::Preview] {
            // Half the match's line: 14 pt in the writer, 11 in the reader.
            let half = if mode == Mode::Source { 14.0 } else { 11.0 };
            let (mut rt, _) = crate::bench::open(&dir, &text, mode);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((520.0, if mode == Mode::Source { 480.0 } else { 400.0 }), 1.0);
            rt.update(|s, tx| {
                s.notice = Some(long.clone());
                s.toast("Saved", tx);
            });
            glide(&mut rt);
            let band = rect(&rt, "toast-band").expect("the band");
            let view = rect(&rt, "doc-scroll").expect("the view");
            assert!(band[3] > 150.0, "{mode:?}: the case needs a tall band: {band:?}");
            rt.update(|s, tx| {
                s.open_find(tx);
                s.reader.find.q = rune::TextEdit::new("Line 4");
                s.find_changed(tx);
            });
            glide(&mut rt);
            glide(&mut rt);
            for (i, forward) in [true, true, true, false, true, true].into_iter().enumerate() {
                // A toast again each time, so the band stays its full height.
                rt.update(|s, tx| {
                    s.toast("Saved", tx);
                    s.find_step(forward, tx);
                });
                glide(&mut rt);
                let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
                assert!(y + half <= band[1] + 0.5 && y - half >= view[1], "{mode:?}, step {i}: the match comes above the band: {} to {}, band from {}", y - half, y + half, band[1]);
            }
            // With no band, find centres in the whole view again: the writer as WR-118 has it.
            if mode == Mode::Source {
                rt.update(|s, _| {
                    s.notice = None;
                    s.reader.toast = None;
                });
                glide(&mut rt);
                rt.update(|s, tx| s.find_step(true, tx));
                glide(&mut rt);
                let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
                assert!((y - (view[1] + view[3] / 2.0)).abs() < 30.0, "with no band the match is centred in the view: {y}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FINDSTEP: a find step glides whenever it is pressed, in both modes, with no band. At each
    /// gap from 0.40 to 0.70 s, as the last glide runs, lands or has just landed, no frame moves
    /// the page more than a glide's frame does.
    #[test]
    fn a_find_step_glides_whenever_it_is_pressed() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-findstep-{}", std::process::id()));
        let text: String = (0..120).map(|i| if i >= 30 && i % 2 == 0 { format!("word at line {i}.\n\n") } else { format!("Line {i} of the note.\n\n") }).collect();
        let offset = |rt: &mut Runtime<App>| {
            let mut y = 0.0;
            rt.update_if(|s, _| {
                y = s.reader.scroll.get()[0];
                false
            });
            y
        };
        for mode in [Mode::Source, Mode::Preview] {
            let (mut rt, _) = crate::bench::open(&dir, &text, mode);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((1280.0, 800.0), 1.0);
            rt.update(|s, tx| {
                s.open_find(tx);
                s.reader.find.q = rune::TextEdit::new("word");
                s.find_changed(tx);
            });
            glide(&mut rt);
            let (start, mut moved) = (offset(&mut rt), 0);
            for gap in 24..=42 {
                rt.update(|s, tx| s.find_step(true, tx));
                let (from, mut last, mut most) = (offset(&mut rt), offset(&mut rt), 0.0f32);
                for _ in 0..gap {
                    rt.frame(DT);
                    let y = offset(&mut rt);
                    most = most.max((y - last).abs());
                    last = y;
                }
                moved += usize::from((last - from).abs() > 30.0);
                assert!(most < 30.0, "{mode:?}, a step {gap} frames after the last: the page moves {most} pt in one frame");
            }
            // The reader holds a match between 20% and 75% of the view (RD-60); the writer
            // centres each.
            let least = if mode == Mode::Source { 19 } else { 8 };
            assert!(moved >= least, "{mode:?}: {moved} of 19 steps moved the page, from {start} to {}", offset(&mut rt));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FARRET: in the reader a find step to a match far from the view lands in one move, where
    /// the match rests, centred: no frame after the one that first draws it there moves it.
    /// Hits about 2,100 pt apart, four steps forward and two back, at 520 and 1280 wide.
    #[test]
    fn a_far_find_match_lands_in_one_move() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-farret-{}", std::process::id()));
        let text: String = (0..300).map(|i| if i % 60 == 5 { format!("word at line {i}.\n\n") } else { format!("Line {i} of the note.\n\n") }).collect();
        for (w, h) in [(520.0, 480.0), (1280.0, 800.0)] {
            let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Preview);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((w, h), 1.0);
            rt.update(|s, tx| {
                s.open_find(tx);
                s.reader.find.q = rune::TextEdit::new("word");
                s.find_changed(tx);
            });
            glide(&mut rt);
            glide(&mut rt);
            for (i, forward) in [true, true, true, true, false, false].into_iter().enumerate() {
                rt.update(|s, tx| s.find_step(forward, tx));
                let mut seen = vec![];
                for _ in 0..40 {
                    rt.frame(DT);
                    if let Some((_, y)) = rt.find(crate::reader::FIND_CUR) {
                        seen.push(y);
                    }
                }
                let page = rt.presented(crate::reader::SCROLL).unwrap().0;
                let mid = page[1] + page[3] / 2.0;
                let last = *seen.last().expect("the match is drawn");
                assert!(seen.iter().all(|y| (y - last).abs() < 0.5), "{w}: step {i} draws the match at {seen:?}, not in one place");
                assert!((last - mid).abs() < 1.0, "{w}: step {i} rests the match at {last}, the view's middle is {mid}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REVEALMOD: quoted-heading far find matches at 520×480 centre by their actual heading line
    /// box (<= 1 pt off centre) in one move, with no post-arrival correction.
    #[test]
    fn a_far_quoted_heading_find_match_centres_in_one_move() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-farquote-{}", std::process::id()));
        let text: String = (0..240)
            .map(|i| match i {
                5 => "needle at start.\n\n".to_string(),
                120 => "> # Quoted needle heading\n\n".to_string(),
                210 => "> ## Another needle subhead\n\n".to_string(),
                _ => format!("Line {i} of the note.\n\n"),
            })
            .collect();
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((520.0, 480.0), 1.0);
        rt.update(|s, tx| {
            s.open_find(tx);
            s.reader.find.q = rune::TextEdit::new("needle");
            s.find_changed(tx);
        });
        glide(&mut rt);
        glide(&mut rt);
        for (i, forward) in [true, true, false].into_iter().enumerate() {
            rt.update(|s, tx| s.find_step(forward, tx));
            let mut seen = vec![];
            for _ in 0..40 {
                rt.frame(DT);
                if let Some((_, y)) = rt.find(crate::reader::FIND_CUR) {
                    seen.push(y);
                }
            }
            let page = rt.presented(crate::reader::SCROLL).unwrap().0;
            let mid = page[1] + page[3] / 2.0;
            let last = *seen.last().expect("the match is drawn");
            assert!(seen.iter().all(|y| (y - last).abs() < 0.5), "step {i} drew quoted heading match at {seen:?}, not in one place");
            assert!((last - mid).abs() <= 1.0, "step {i} rested quoted heading match at {last}, view middle is {mid}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REVEALMOD: M2 (toast just before wheel notches never reverses or drops swipe distance),
    /// M3 (deferred caret lift after swipe + toast glides smoothly at <= 24 pt/frame on next key),
    /// and post-typing Save (Enter then Ctrl+S 0.05 s later lifts the caret cleanly above band).
    #[test]
    fn reveal_controller_handles_m2_m3_and_post_typing_save() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-revealmod-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let offset = |rt: &mut Runtime<App>| {
            let mut at = 0.0;
            rt.update_if(|s, _| {
                at = s.reader.scroll.get()[0];
                false
            });
            at
        };
        let text: String = (0..200).map(|i| format!("Line {i} of the note.\n\n")).collect();

        // M2: Ctrl+End, Ctrl+S, then two notched +120 wheel events starting 1 frame later.
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        rt.key(KeyEvent { key: Key::End, mods: ctrl(false) });
        glide(&mut rt);
        let notch = || rune::WheelEvent::new(0.0, 120.0).at(1024.0, 350.0).precise(false).notch(true);
        let start = offset(&mut rt);
        let mut at = vec![start];
        for i in 0..60 {
            match i {
                0 => rt.key(KeyEvent { key: Key::Command('s'), mods: ctrl(false) }),
                1 | 4 => rt.wheel_event(notch()),
                _ => {}
            }
            rt.frame(DT);
            at.push(offset(&mut rt));
        }
        let back = at.windows(2).map(|w| w[1] - w[0]).fold(0.0f32, f32::max);
        let net = start - at[at.len() - 1];
        assert!(back < 3.0, "M2: toast reversed swipe by {back} pt");
        assert!(net >= 225.0, "M2: full swipe distance lost: {net} pt of 240");

        // M3: Ctrl+End, +60 wheel, Ctrl+S, then Right glides smoothly (<= 24 pt/frame).
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        rt.key(KeyEvent { key: Key::End, mods: ctrl(false) });
        glide(&mut rt);
        rt.wheel_event(rune::WheelEvent::new(0.0, 60.0).at(700.0, 400.0).precise(false).notch(true));
        for _ in 0..2 {
            rt.frame(DT);
        }
        rt.key(KeyEvent { key: Key::Command('s'), mods: ctrl(false) });
        glide(&mut rt);
        let mut last = offset(&mut rt);
        let mut max_step = 0.0f32;
        rt.key(KeyEvent { key: Key::Right, mods: Mods::default() });
        for _ in 0..40 {
            rt.frame(DT);
            let now = offset(&mut rt);
            max_step = max_step.max((now - last).abs());
            last = now;
        }
        assert!(max_step <= 24.0, "M3: deferred caret reveal jumped {max_step} pt in one frame");
        let band = rect(&rt, "toast-band").expect("M3: toast band shows");
        let caret = rect(&rt, "caret-at").expect("M3: caret shows");
        assert!(caret[1] + caret[3] <= band[1] + 2.0, "M3: caret {caret:?} stayed under band {band:?}");

        // Post-typing Save: Enter at EOF then Ctrl+S 3 frames (~0.05 s) later lifts caret above band.
        let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.focus_editor(tx));
        rt.key(KeyEvent { key: Key::End, mods: ctrl(false) });
        glide(&mut rt);
        rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        for _ in 0..3 {
            rt.frame(DT);
        }
        rt.key(KeyEvent { key: Key::Command('s'), mods: ctrl(false) });
        glide(&mut rt);
        let band = rect(&rt, "toast-band").expect("post-type Save: toast band shows");
        let caret = rect(&rt, "caret-at").expect("post-type Save: caret shows");
        assert!(caret[1] + caret[3] <= band[1] + 2.0, "post-type Save: caret {caret:?} stayed under band {band:?}");

        // REVEALWEL: rapid deferred typing burst settles within <= 1.5 pt of reduced motion.
        let run_burst = |reduced: bool| -> f32 {
            // SAFETY: held under ENV lock.
            unsafe {
                std::env::set_var("QUIRE_REDUCED_MOTION", if reduced { "1" } else { "0" });
            }
            let (mut rt, _) = crate::bench::open(&dir, &text, Mode::Source);
            rt.on_key = Some(crate::keys::on_key);
            rt.set_viewport((1280.0, 800.0), 1.0);
            rt.update(|s, tx| s.focus_editor(tx));
            rt.key(KeyEvent { key: Key::End, mods: ctrl(false) });
            glide(&mut rt);
            rt.wheel_event(rune::WheelEvent::new(0.0, 60.0).at(640.0, 400.0).precise(false).notch(true));
            for _ in 0..2 {
                rt.frame(DT);
            }
            rt.key(KeyEvent { key: Key::Command('s'), mods: ctrl(false) });
            glide(&mut rt);
            for ch in "hello world".chars() {
                rt.key(KeyEvent { key: Key::Text(ch.to_string()), mods: Mods::default() });
                rt.frame(DT);
            }
            glide(&mut rt);
            let caret = rect(&rt, "caret-at").expect("burst caret");
            // SAFETY: held under ENV lock.
            unsafe {
                std::env::remove_var("QUIRE_REDUCED_MOTION");
            }
            caret[1] + caret[3]
        };
        let norm_bot = run_burst(false);
        let red_bot = run_burst(true);
        assert!((norm_bot - red_bot).abs() <= 1.5, "REVEALWEL: burst settle norm={norm_bot:.2} vs reduced={red_bot:.2} (diff {:.2} pt)", (norm_bot - red_bot).abs());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A focused row's whole ring shows (RING): Home and End in the tree, the search results
    /// and the outline scroll the row's ring, a 3 point gap and a 2 point stroke past the row,
    /// inside the list, and each end of the list leaves room for it.
    #[test]
    fn a_focused_rows_ring_stays_inside_its_list() {
        // The ring's reach past the row, as `ui::focus_ring` draws it: its gap and its stroke.
        const RING: f32 = 5.0;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-ring-{}", std::process::id()));
        // Forty more notes, so the tree and the results scroll; the folder opens again to read
        // them, as `garden` makes the folder new.
        let mut rt = garden(&dir, Mode::Preview);
        let root = dir.join("notes");
        for i in 0..40 {
            std::fs::write(root.join(format!("n{i:02}.md")), format!("# N{i}\n\nA row. #many\n")).unwrap();
        }
        rt.update(move |s, tx| {
            s.open_vault(root, tx);
            s.open_file("note.md", None, tx);
            s.mode = Mode::Preview;
            s.persist.left_open = true;
            s.persist.right_open = true;
            s.expanded.insert("Box".into());
            tx.blur();
        });
        settle(&mut rt);
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        let mut fails = Vec::new();
        let mut look = |rt: &mut Runtime<App>, k: Key, list: &str, case: &str| {
            key(rt, k, Mods::default());
            glide(rt);
            let Some(at) = focused(rt) else { return fails.push(format!("{case}: nothing focused")) };
            let (Some(r), Some(v)) = (rect(rt, &at), rect(rt, list)) else { return fails.push(format!("{case}: no rect for {at} or {list}")) };
            let (top, bottom) = (r[1] - RING, r[1] + r[3] + RING);
            if top < v[1] - 0.5 || bottom > v[1] + v[3] + 0.5 {
                fails.push(format!("{case}: {at} ring y {top} to {bottom}, list shows y {} to {}", v[1], v[1] + v[3]));
            }
        };
        // F6 lands on the active tab (SB-97); the tree's row is the next stop's.
        rt.update(|s, tx| tx.focus(s.tree_stop().unwrap(), true));
        settle(&mut rt);
        look(&mut rt, Key::End, "side-scroll", "the tree's last row");
        look(&mut rt, Key::Home, "side-scroll", "the tree's first row");
        rt.update(|s, tx| s.search_tag("many", tx));
        settle(&mut rt);
        key(&mut rt, Key::Function(6), Mods { shift: true, ..Mods::default() });
        assert!(focused(&rt).is_some_and(|k| k.starts_with("res-")), "Shift+F6 puts the keyboard on a result: {:?}", focused(&rt));
        look(&mut rt, Key::End, "results-scroll", "the last result");
        look(&mut rt, Key::Home, "results-scroll", "the first result");
        // Esc clears the search to the field, and the next gives the note the keyboard.
        key(&mut rt, Key::Escape, Mods::default());
        key(&mut rt, Key::Escape, Mods::default());
        key(&mut rt, Key::Function(6), Mods::default());
        look(&mut rt, Key::End, "outline-scroll", "the outline's last row");
        look(&mut rt, Key::Home, "outline-scroll", "the outline's first row");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "rings cut by a list's edge:\n{}", fails.join("\n"));
    }

    /// A window on a small folder, in preview with the outline docked: the note and a folder
    /// `Box` with two notes, and a tag in each.
    fn garden(dir: &std::path::Path, mode: Mode) -> Runtime<App> {
        let (mut rt, _) = crate::bench::open(dir, &format!("{NOTE}\n## Second\n\n{}### Third\n\nEnd. #kept\n", "More text to scroll by.\n\n".repeat(80)), mode);
        let root = dir.join("notes");
        std::fs::create_dir_all(root.join("Box")).unwrap();
        std::fs::write(root.join("Box/a.md"), "# A\n\nSee [[note]]. #kept\n").unwrap();
        std::fs::write(root.join("Box/b.md"), "# B\n").unwrap();
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(move |s, tx| {
            s.open_vault(root, tx);
            s.open_file("note.md", None, tx);
            s.mode = mode;
            s.persist.right_open = true;
            s.persist.left_open = true;
            s.expanded.insert("Box".into());
            tx.blur();
        });
        settle(&mut rt);
        rt
    }

    /// The key event Rune sends for a chord with Ctrl or Alt.
    fn chord_event(c: &crate::keymap::Chord) -> KeyEvent {
        use crate::keymap::K;
        let key = match c.key {
            K::Char(ch) if c.ctrl => Key::Command(ch),
            K::Char(ch) => Key::Text(ch.to_string()),
            K::Enter => Key::Enter,
            K::Tab => Key::Tab,
            K::Up => Key::Up,
            K::Down => Key::Down,
            K::Left => Key::Left,
            K::Right => Key::Right,
            K::F(n) => Key::Function(n),
            other => panic!("no key event for {other:?}"),
        };
        KeyEvent { key, mods: Mods { cmd: c.ctrl, ctrl: c.ctrl && !cfg!(target_os = "macos"), shift: c.shift, alt: c.alt, ..Mods::default() } }
    }

    /// What a key can change in the app, as one string to compare.
    fn state(rt: &mut Runtime<App>) -> String {
        let mut out = String::new();
        rt.update_if(|s, _| {
            let layers: Vec<_> = s.open_layers().map(crate::layer::Layer::kind).collect();
            let toast = s.reader.toast.as_ref().map(|t| t.0.clone());
            // The MCP token is new in every window.
            let mut persist = s.persist.clone();
            persist.settings.mcp.token.clear();
            let text: String = s.active_text().chars().take(80).collect();
            out = format!("{:?}", (s.mode, &s.active, &s.tabs, layers, s.reader.find.open, &s.reader.find.q.text, toast, text, s.focus, s.reader.wide, persist));
            false
        });
        out
    }

    fn text_key(rt: &mut Runtime<App>, t: &str) {
        let shift = t.chars().all(|c| c.is_ascii_uppercase());
        key(rt, Key::Text(t.into()), Mods { shift, ..Mods::default() });
    }

    /// Lets a glide or a scroll finish.
    fn glide(rt: &mut Runtime<App>) {
        for _ in 0..40 {
            rt.frame(DT);
        }
    }

    /// The focus rule (`crate::focus`) as a matrix: every stop, reached with the keyboard
    /// alone, crossed with the reader's keys, with each way it can go away, and with a layer
    /// that closes over it.
    #[test]
    fn every_stop_follows_the_focus_rule() {
        use crate::focus::{Pane, stop};
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-focus-matrix-{}", std::process::id()));
        let mut fails = Vec::new();

        // Every focusable element in the panes is a stop of the table, in its pane.
        for mode in [Mode::Preview, Mode::Source] {
            let rt = garden(&dir, mode);
            for e in rt.elements().into_iter().filter(|e| e.focusable) {
                let k = e.key.clone().unwrap_or_default();
                let want = if e.path.contains("sidebar/") {
                    Pane::Sidebar
                } else if e.path.contains("/outline/") {
                    Pane::Outline
                } else if e.path.contains("/toolbar/") {
                    Pane::Note
                } else {
                    continue;
                };
                if stop(&k) != Some(want) {
                    fails.push(format!("{mode:?}: {k} ({}) is not a {want:?} stop", e.path));
                }
            }
        }

        // Reachable from the keyboard alone: Tab from the reader reaches every stop, and the
        // panel tabs' arrows, Enter and the search field's Down reach the rest.
        let mut rt = garden(&dir, Mode::Preview);
        let mut reached = Vec::new();
        for _ in 0..40 {
            key(&mut rt, Key::Tab, Mods::default());
            reached.extend(focused(&rt));
        }
        let wants = ["search", "new-note", "label-shelf", "tab:", "label-vault", "file:", "foot-tags", "foot-settings", "ol-", "bl-"];
        for want in wants {
            if !reached.iter().any(|k| k.starts_with(want)) {
                fails.push(format!("Tab never reaches {want}"));
            }
        }
        // The tags button puts "#" in the field with the keyboard there, and Tab reaches the
        // tags. Pressed again it clears the field.
        rt.update(|_, tx| tx.focus("foot-tags", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, Mods::default());
        settle(&mut rt);
        let query = |rt: &mut Runtime<App>| {
            let mut q = String::new();
            rt.update_if(|s, _| {
                q = s.query.text.clone();
                false
            });
            q
        };
        if focused(&rt).as_deref() != Some("search") || query(&mut rt) != "#" {
            fails.push(format!("Enter on the tags button: focus on {:?}, query {:?}", focused(&rt), query(&mut rt)));
        }
        if !(0..4).any(|_| {
            key(&mut rt, Key::Tab, Mods::default());
            focused(&rt).is_some_and(|f| f.starts_with("res-tag:"))
        }) {
            fails.push(format!("Tab from the field never reaches a tag: focus on {:?}", focused(&rt)));
        }
        rt.update(|_, tx| tx.focus("foot-tags", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, Mods::default());
        settle(&mut rt);
        if !query(&mut rt).is_empty() {
            fails.push(format!("the tags button again leaves {:?} in the field", query(&mut rt)));
        }
        rt.update(|_, tx| tx.focus("search", true));
        settle(&mut rt);
        for c in ["k", "e", "p", "t"] {
            text_key(&mut rt, c);
        }
        key(&mut rt, Key::Down, Mods::default());
        let first = focused(&rt).unwrap_or_default();
        if !first.starts_with("res-") {
            fails.push(format!("Down in the search field: focus on {first}"));
        }
        key(&mut rt, Key::Up, Mods::default());
        if focused(&rt).as_deref() != Some("search") {
            fails.push(format!("Up from the first result: focus on {:?}", focused(&rt)));
        }
        let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("res-")).collect();
        let unique: std::collections::HashSet<&String> = keys.iter().collect();
        if keys.len() < 2 || unique.len() != keys.len() {
            fails.push(format!("the results' rows share keys: {keys:?}"));
        }
        // In the writer Tab indents, so F6 leaves the editor for the sidebar, and Tab goes on
        // from there to the toolbar.
        let mut rt = garden(&dir, Mode::Source);
        for _ in 0..2 {
            key(&mut rt, Key::Function(6), Mods::default());
        }
        let mut bar = Vec::new();
        for _ in 0..40 {
            key(&mut rt, Key::Tab, Mods::default());
            if let Some(k) = focused(&rt).filter(|k| k.starts_with("fmt-")) {
                bar.push(k);
                key(&mut rt, Key::Right, Mods::default());
                bar.extend(focused(&rt));
                key(&mut rt, Key::Tab, Mods::default());
                bar.extend(focused(&rt));
                break;
            }
        }
        if bar.len() != 3 || !bar[1].starts_with("fmt-") || bar[1] == bar[0] || bar[2].starts_with("fmt-") {
            fails.push(format!("the toolbar is not one Tab stop with Right inside: {bar:?}"));
        }

        // Every shortcut with Ctrl or Alt acts from a stop as it does with the keyboard on the
        // note (CHORD), in both modes. Quit and Insert an image are left out: they reach the
        // platform, not the app's state.
        for (mode, stops) in [(Mode::Preview, ["file:Box|a.md", "label-vault", "ol-0"]), (Mode::Source, ["file:Box|a.md", "label-vault", "fmt-bold"])] {
            let mut rt = garden(&dir, mode);
            let mut now = None;
            rt.update_if(|s, _| {
                now = Some(crate::keys::page_stack(s));
                false
            });
            let ctxs = now.expect("read");
            let mut chords = Vec::new();
            for a in crate::keymap::TABLE.iter().filter(|a| a.ctx.iter().any(|c| ctxs.contains(c)) && crate::keys::handler(a.id).is_some()) {
                if matches!(a.status, crate::keymap::Status::Deferred | crate::keymap::Status::Dropped) || matches!(a.id, "app.quit" | "app.insert-image") {
                    continue;
                }
                for c in a.chords.iter().filter_map(|s| crate::keymap::parse(s)).filter(|seq| seq.len() == 1).map(|seq| seq[0]) {
                    if (c.ctrl || c.alt) && !chords.iter().any(|(_, d)| *d == c) {
                        chords.push((a.id, c));
                    }
                }
            }
            for (i, (id, c)) in chords.into_iter().enumerate() {
                let ev = chord_event(&c);
                let on = |stop: Option<&'static str>| {
                    let mut rt = garden(&dir, mode);
                    if mode == Mode::Source {
                        rt.update(|s, tx| {
                            s.cur_edit().set_selection(0, 0);
                            s.focus_editor(tx);
                        });
                        settle(&mut rt);
                    }
                    if let Some(k) = stop {
                        rt.update(move |_, tx| tx.focus(k, true));
                        settle(&mut rt);
                    }
                    rt.key(ev.clone());
                    settle(&mut rt);
                    state(&mut rt)
                };
                let stop = stops[i % stops.len()];
                let (note, from_stop) = (on(None), on(Some(stop)));
                if note != from_stop {
                    fails.push(format!("{mode:?}: {id} ({c:?}) from {stop} acts differently than from the note:\n  note {note}\n  stop {from_stop}"));
                }
            }
        }
        // The editor's own chords (undo, redo, select all, the clipboard's) live in its text
        // field, outside the keymap. From a stop that is not a text field they act as from the
        // note: in the writer docked and from the 640 drawer, where the editor then has the
        // keyboard, and in preview. A text field keeps them for itself (CTRLZ).
        let edits = [('z', false), ('z', true), ('y', false), ('a', false), ('x', false), ('c', false), ('v', false)];
        for (mode, width) in [(Mode::Source, 1280.0), (Mode::Source, 640.0), (Mode::Preview, 1280.0)] {
            for (c, shift) in edits {
                let on = |stop: bool| {
                    let mut rt = garden(&dir, mode);
                    rt.set_viewport((width, 800.0), 1.0);
                    settle(&mut rt);
                    if mode == Mode::Source {
                        rt.update(|s, tx| {
                            s.cur_edit().set_selection(0, 0);
                            s.focus_editor(tx);
                        });
                        settle(&mut rt);
                        for t in ["Q", "Q"] {
                            text_key(&mut rt, t);
                        }
                    }
                    if stop && width < 1000.0 {
                        // The drawer opens on the search field, a text field that keeps the
                        // chords (CTRLZ): the case moves to a row in it.
                        key(&mut rt, Key::Command('b'), ctrl(true));
                        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
                        settle(&mut rt);
                    } else if stop {
                        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
                        settle(&mut rt);
                    }
                    let from = focused(&rt).unwrap_or_default();
                    key(&mut rt, Key::Command(c), ctrl(shift));
                    if mode == Mode::Source {
                        text_key(&mut rt, "W");
                    }
                    (from, focused(&rt), state(&mut rt))
                };
                let ((_, _, note), (from, after, from_stop)) = (on(false), on(true));
                if crate::focus::stop(&from).is_none() {
                    fails.push(format!("{mode:?} at {width}: the case did not put the keyboard on a stop ({from})"));
                }
                if note != from_stop {
                    fails.push(format!("{mode:?} at {width}: Ctrl+{}{c} from {from} acts differently than from the note:\n  note {note}\n  stop {from_stop}", if shift { "Shift+" } else { "" }));
                }
                if mode == Mode::Source && after.as_deref() != Some("editor") {
                    fails.push(format!("at {width}: after Ctrl+{c} from {from} the keyboard is on {after:?}, not the editor"));
                }
            }
        }
        let read = |rt: &mut Runtime<App>| {
            let mut got = (String::new(), String::new());
            rt.update_if(|s, _| {
                got = (s.query.text.clone(), s.active_text().to_string());
                false
            });
            got
        };
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| s.open_search(tx));
        settle(&mut rt);
        text_key(&mut rt, "box");
        let (_, before) = read(&mut rt);
        key(&mut rt, Key::Command('a'), ctrl(false));
        text_key(&mut rt, "W");
        let (query, text) = read(&mut rt);
        if focused(&rt).as_deref() != Some("search") || query != "W" || text != before {
            fails.push(format!("Ctrl+A in the search field: focus {:?}, query {query:?}, note changed {}", focused(&rt), text != before));
        }

        // A note deleted from its focused row by keys (Shift+F10, Down to Delete note, Enter):
        // the row is gone, so the note has the keyboard (DEL). In the writer the note deleted is
        // the open one, and the next one shows its text at once, though it never had an
        // editor; in preview the note shown stays and its keys work.
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, _| s.tabs.push("Box/b.md".into()));
        rt.update(|_, tx| tx.focus("file:note.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        for k in [Key::Down, Key::Down, Key::Down, Key::Down, Key::Enter] {
            key(&mut rt, k, Mods::default());
        }
        let field = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("editor")).and_then(|e| e.text).unwrap_or_default();
        if !field.starts_with("# B") {
            fails.push(format!("after deleting the open note, the editor shows {field:?}, not the next note"));
        }
        for c in ["Q", "Q"] {
            text_key(&mut rt, c);
        }
        let after = seen(&mut rt);
        if after.layers != 0 || !after.text.contains("QQ") {
            fails.push(format!("the open note deleted from its row: typing went to {:?} (layers {}, text {:?})", focused(&rt), after.layers, after.text));
        }
        let mut rt = garden(&dir, Mode::Preview);
        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        for k in [Key::Down, Key::Down, Key::Down, Key::Down, Key::Down, Key::Enter] {
            key(&mut rt, k, Mods::default());
        }
        let y0 = seen(&mut rt).page_y;
        text_key(&mut rt, "j");
        glide(&mut rt);
        let after = seen(&mut rt);
        if rt.elements().iter().any(|e| e.key.as_deref() == Some("file:Box|a.md")) || after.layers != 0 || after.page_y > y0 - 5.0 {
            fails.push(format!("a note deleted from its row in preview: then j does nothing (focus {:?}, layers {}, page {y0} to {})", focused(&rt), after.layers, after.page_y));
        }
        // In the drawer too: a tree row focused in it, Ctrl+F closes the drawer and opens find.
        let mut rt = garden(&dir, Mode::Preview);
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        rt.update(|s, tx| s.toggle_left(tx));
        settle(&mut rt);
        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Command('f'), ctrl(false));
        let after = seen(&mut rt);
        if !after.find || after.layers != 0 {
            fails.push(format!("Ctrl+F from a row in the drawer: find {}, layers {}", after.find, after.layers));
        }

        // The stops, as a key and the ways each goes away. Every way leaves the page's
        // reader keys working.
        let ctrl_shift_b = |rt: &mut Runtime<App>| key(rt, Key::Command('b'), ctrl(true));
        let zen = |rt: &mut Runtime<App>| key(rt, Key::Command('.'), ctrl(false));
        let esc = |rt: &mut Runtime<App>| key(rt, Key::Escape, Mods::default());
        let outline_key = |rt: &mut Runtime<App>| text_key(rt, "o");
        type Away = (&'static str, fn(&mut Runtime<App>));
        let side: Vec<Away> = vec![("Ctrl+Shift+B", ctrl_shift_b), ("zen", zen), ("Esc", esc)];
        let outline: Vec<Away> = vec![("o", outline_key), ("zen", zen), ("Esc", esc)];
        let stops: [(&str, &Vec<Away>); 11] = [
            ("tab:note.md", &side),
            ("new-note", &side),
            ("label-shelf", &side),
            ("label-vault", &side),
            ("dir:Box", &side),
            ("file:Box|a.md", &side),
            ("foot-tags", &side),
            ("foot-settings", &side),
            ("search", &side),
            ("ol-0", &outline),
            ("bl-0", &outline),
        ];
        for (k, aways) in stops {
            let mut rt = garden(&dir, Mode::Preview);
            let k2 = k.to_string();
            rt.update(move |_, tx| tx.focus(k2, true));
            settle(&mut rt);
            if focused(&rt).as_deref() != Some(k) {
                fails.push(format!("{k}: cannot take focus ({:?})", focused(&rt)));
                continue;
            }
            // The reader's keys do not reach the reader behind a stop.
            let y0 = seen(&mut rt).page_y;
            for c in ["j", "G", "n", "w"] {
                text_key(&mut rt, c);
            }
            key(&mut rt, Key::PageDown, Mods::default());
            glide(&mut rt);
            let after = seen(&mut rt);
            if (after.page_y - y0).abs() > 0.5 || after.layers != 0 {
                fails.push(format!("{k}: a reader key acted behind it (page {y0} to {}, layers {})", after.page_y, after.layers));
            }
            // A layer that closes gives the stop back with its ring.
            let mut rt = garden(&dir, Mode::Preview);
            let k2 = k.to_string();
            rt.update(move |_, tx| tx.focus(k2, true));
            settle(&mut rt);
            key(&mut rt, Key::Command('k'), ctrl(false));
            key(&mut rt, Key::Escape, Mods::default());
            let ring = rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key, e.focus_visible));
            if ring != Some((Some(k.to_string()), true)) {
                fails.push(format!("{k}: after the palette closes, focus is {ring:?}"));
            }
            // However the stop goes away, the reader has its keys back.
            for (how, away) in aways.iter() {
                for (label, press) in [("Space", " "), ("j", "j")] {
                    let mut rt = garden(&dir, Mode::Preview);
                    let k2 = k.to_string();
                    rt.update(move |_, tx| tx.focus(k2, true));
                    settle(&mut rt);
                    away(&mut rt);
                    glide(&mut rt);
                    let y0 = seen(&mut rt).page_y;
                    text_key(&mut rt, press);
                    glide(&mut rt);
                    let y1 = seen(&mut rt).page_y;
                    if y1 > y0 - 5.0 {
                        fails.push(format!("{k}, then {how}: {label} does nothing ({y0} to {y1})"));
                    }
                }
            }
        }

        // F6 moves between the panes, round; Shift+F6 goes back. The note's links are skipped.
        let mut rt = garden(&dir, Mode::Preview);
        let f6 = |rt: &mut Runtime<App>, back: bool| key(rt, Key::Function(6), Mods { shift: back, ..Mods::default() });
        let mut path = Vec::new();
        for _ in 0..4 {
            f6(&mut rt, false);
            path.push(focused(&rt).filter(|k| k != crate::focus::NOTE).map(|k| stop(&k).map_or(k.clone(), |p| format!("{p:?}"))).unwrap_or_else(|| "Note".into()));
        }
        if path != ["Outline", "Sidebar", "Note", "Outline"] {
            fails.push(format!("F6 goes {path:?}"));
        }
        f6(&mut rt, true);
        f6(&mut rt, true);
        if focused(&rt).is_none_or(|k| stop(&k) != Some(Pane::Sidebar)) {
            fails.push(format!("Shift+F6 twice from the outline goes to {:?}", focused(&rt)));
        }
        let mut rt = garden(&dir, Mode::Source);
        rt.update(|s, tx| s.focus_editor(tx));
        settle(&mut rt);
        f6(&mut rt, false);
        if focused(&rt).is_none_or(|k| stop(&k) != Some(Pane::Outline)) {
            fails.push(format!("F6 in the writer goes to {:?}", focused(&rt)));
        }
        f6(&mut rt, false);
        f6(&mut rt, false);
        if !seen(&mut rt).editor_focused {
            fails.push(format!("F6 twice from the outline in the writer goes to {:?}, not the editor", focused(&rt)));
        }

        // Into the writer by Tab from the toolbar, Shift+Tab or Shift+F6 from the outline, the
        // caret stays where it was, in view.
        let caret_y = |rt: &Runtime<App>| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("caret-at")).map(|e| e.rect[1]);
        let tab = |rt: &mut Runtime<App>| key(rt, Key::Tab, Mods::default());
        let shift_tab = |rt: &mut Runtime<App>| key(rt, Key::Tab, Mods { shift: true, ..Mods::default() });
        let shift_f6 = |rt: &mut Runtime<App>| key(rt, Key::Function(6), Mods { shift: true, ..Mods::default() });
        let ways: [(&str, &str, fn(&mut Runtime<App>)); 3] = [("fmt-bold", "Tab", tab), ("ol-0", "Shift+Tab", shift_tab), ("ol-0", "Shift+F6", shift_f6)];
        for (from, how, press) in ways {
            let mut rt = garden(&dir, Mode::Source);
            rt.update(|s, tx| {
                s.cur_edit().set_selection(0, 0);
                s.focus_editor(tx);
            });
            glide(&mut rt);
            let y0 = caret_y(&rt);
            let from2 = from.to_string();
            rt.update(move |_, tx| tx.focus(from2, true));
            settle(&mut rt);
            press(&mut rt);
            glide(&mut rt);
            let y1 = caret_y(&rt);
            if !seen(&mut rt).editor_focused || y0.is_none() || y1.is_none_or(|y| (y - y0.unwrap()).abs() > 0.5 || !(0.0..800.0).contains(&y)) {
                fails.push(format!("{how} from {from} into the writer: focus {:?}, caret y {y0:?} to {y1:?}", focused(&rt)));
            }
        }

        // Once the note has the keyboard, Tab goes on from the note, into its links, however
        // the note got it: Esc from a stop, or the stop's pane hidden and shown again.
        let hide_show = |rt: &mut Runtime<App>| {
            key(rt, Key::Command('b'), ctrl(true));
            key(rt, Key::Command('b'), ctrl(true));
        };
        let back: [(&str, fn(&mut Runtime<App>)); 2] = [("Esc", esc), ("the sidebar hidden and shown", hide_show)];
        for (how, away) in back {
            let mut rt = garden(&dir, Mode::Preview);
            // Box/a.md has its links at the top, where the reader has them laid out.
            rt.update(|s, tx| s.open_file("Box/a.md", None, tx));
            settle(&mut rt);
            rt.update(|_, tx| tx.focus("file:Box|a.md", true));
            settle(&mut rt);
            away(&mut rt);
            glide(&mut rt);
            tab(&mut rt);
            let in_note = |k: &str| {
                k.starts_with("wiki:")
                    || k.starts_with("tag:")
                    || k.starts_with("page-fold:")
                    || k == "anchor"
                    || k.starts_with("block-edit:")
            };
            if !focused(&rt).is_some_and(|k| in_note(&k)) {
                fails.push(format!("a sidebar row, then {how}: Tab goes to {:?}, not inside the note", focused(&rt)));
            }
            let mut hit_link = focused(&rt).is_some_and(|k| k.starts_with("wiki:") || k.starts_with("tag:"));
            for _ in 0..8 {
                if hit_link {
                    break;
                }
                tab(&mut rt);
                if focused(&rt).is_some_and(|k| k.starts_with("wiki:") || k.starts_with("tag:")) {
                    hit_link = true;
                }
            }
            if !hit_link {
                fails.push(format!("a sidebar row, then {how}: Tab never reaches the note's link ({:?})", focused(&rt)));
            }
        }

        // A text result opens in the current mode (SB-64). In the writer the caret is at the
        // match's start with nothing selected, as find leaves it: the next key types there. In
        // preview the reader's find marks the match, and the reader has the keyboard.
        for mode in [Mode::Source, Mode::Preview] {
            let mut rt = garden(&dir, mode);
            rt.update(|s, _| s.query = rune::TextEdit::new("kept"));
            settle(&mut rt);
            rt.update(|_, tx| tx.focus("res-text:Box|a.md", true));
            settle(&mut rt);
            key(&mut rt, Key::Enter, Mods::default());
            glide(&mut rt);
            let mut got = (0, 0, 0, false, String::new());
            rt.update_if(|s, _| {
                let e = s.cur_edit();
                got = (e.anchor, e.focus, e.text.find("kept").unwrap_or(usize::MAX), s.reader.find.open, s.reader.find.searched.clone());
                false
            });
            let (anchor, caret, at, find, searched) = got;
            let before = seen(&mut rt);
            if mode == Mode::Source {
                text_key(&mut rt, "Q");
                let after = seen(&mut rt);
                if before.mode != mode || !before.editor_focused || anchor != caret || caret != at || !after.text.contains("Qkept") {
                    fails.push(format!("a text result in the writer: mode {:?}, editor focused {}, selection {anchor}..{caret}, match at {at}", before.mode, before.editor_focused));
                }
            } else if before.mode != mode || !find || searched != "kept" || !matches!(focused(&rt).as_deref(), None | Some(crate::focus::NOTE)) {
                fails.push(format!("a text result in preview: mode {:?}, find open {find} for {searched:?}, focus {:?}", before.mode, focused(&rt)));
            }
        }

        // Enter on a note's row in the sidebar's drawer closes the drawer and gives the note
        // the keyboard, whether that note was open or not.
        for mode in [Mode::Preview, Mode::Source] {
            for row in ["file:note.md", "file:Box|a.md"] {
                let mut rt = garden(&dir, mode);
                rt.set_viewport((640.0, 480.0), 1.0);
                settle(&mut rt);
                rt.update(|s, tx| s.toggle_left(tx));
                settle(&mut rt);
                let row2 = row.to_string();
                rt.update(move |_, tx| tx.focus(row2, true));
                settle(&mut rt);
                key(&mut rt, Key::Enter, Mods::default());
                glide(&mut rt);
                let before = seen(&mut rt);
                let at = focused(&rt);
                if mode == Mode::Preview {
                    text_key(&mut rt, "j");
                } else {
                    text_key(&mut rt, "Q");
                }
                glide(&mut rt);
                let after = seen(&mut rt);
                // Box/a.md is too short to scroll: the reader has the keyboard when nothing else does.
                let keys = match mode {
                    Mode::Preview if row == "file:note.md" => after.page_y < before.page_y - 5.0,
                    Mode::Preview => at.is_none() || at.as_deref() == Some(crate::focus::NOTE),
                    _ => after.text.matches('Q').count() > before.text.matches('Q').count(),
                };
                if before.layers != 0 || !keys {
                    fails.push(format!("{mode:?}: Enter on {row} in the drawer: layers {}, focus {at:?}, the next key reached the note: {keys}", before.layers));
                }
            }
        }

        // A menu's item that opens the name field gives it the keyboard (REN): Rename and New
        // note here, picked by keys from a row's menu, in the sidebar's drawer and docked. The
        // drawer closes, the field has focus with its text selected, and a typed name goes
        // into it, never into the note.
        for size in [(640.0, 480.0), (1280.0, 800.0)] {
            for (item, downs) in [("Rename", 1), ("New note here", 0)] {
                let mut rt = garden(&dir, Mode::Source);
                rt.set_viewport(size, 1.0);
                settle(&mut rt);
                if size.0 < 700.0 {
                    rt.update(|s, tx| s.toggle_left(tx));
                    settle(&mut rt);
                }
                rt.update(|_, tx| tx.focus("file:note.md", true));
                settle(&mut rt);
                key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
                for _ in 0..downs {
                    key(&mut rt, Key::Down, Mods::default());
                }
                key(&mut rt, Key::Enter, Mods::default());
                let name = |rt: &mut Runtime<App>| {
                    let mut out = None;
                    rt.update_if(|s, _| {
                        out = s.renaming.as_ref().map(|e| (e.text.clone(), e.anchor.min(e.focus), e.anchor.max(e.focus)));
                        false
                    });
                    out
                };
                let opened = name(&mut rt);
                let before = seen(&mut rt);
                let at = focused(&rt);
                for c in "new name".chars() {
                    text_key(&mut rt, &c.to_string());
                }
                let typed = name(&mut rt).map(|n| n.0);
                let after = seen(&mut rt);
                let whole = opened.as_ref().is_some_and(|(t, a, b)| *a == 0 && *b == t.len() && !t.is_empty());
                if before.layers != 0 || at.as_deref() != Some("rename") || !whole || typed.as_deref() != Some("new name") || after.text != before.text {
                    fails.push(format!("{item} from a row's menu at {}: layers {}, focus {at:?}, the field {opened:?}, then typed {typed:?}, the note changed: {}", size.0, before.layers, after.text != before.text));
                }
            }
        }

        // In the writer, what holds the keyboard going away gives the note the keys (FESC): Esc
        // in the find bar, Esc in the search field, and Ctrl+W on one of several tabs.
        let ways: [(&str, fn(&mut Runtime<App>)); 3] = [
            ("Esc in the find bar", |rt| {
                key(rt, Key::Command('f'), ctrl(false));
                for c in ["m", "o", "r", "e"] {
                    text_key(rt, c);
                }
                key(rt, Key::Escape, Mods::default());
            }),
            ("Esc in the search field, twice: it clears, then leaves", |rt| {
                rt.update(|s, tx| s.open_search(tx));
                settle(rt);
                for c in ["m", "o", "r", "e"] {
                    text_key(rt, c);
                }
                key(rt, Key::Escape, Mods::default());
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+W on one of several tabs", |rt| {
                rt.update(|s, tx| {
                    s.open_file("Box/b.md", None, tx);
                    s.open_file("note.md", None, tx);
                });
                settle(rt);
                key(rt, Key::Command('w'), ctrl(false));
            }),
        ];
        for (how, away) in ways {
            let mut rt = garden(&dir, Mode::Source);
            rt.update(|s, tx| s.focus_editor(tx));
            settle(&mut rt);
            away(&mut rt);
            let before = seen(&mut rt);
            for c in ["Q", "Q"] {
                text_key(&mut rt, c);
            }
            let after = seen(&mut rt);
            if after.text.matches("QQ").count() != before.text.matches("QQ").count() + 1 {
                fails.push(format!("{how}: the next typing goes nowhere (focus {:?}, editor focused {})", focused(&rt), before.editor_focused));
            }
        }

        // A text field keeps the writer's edits at the caret from the note (FLD): Ctrl+B, Ctrl+L,
        // Ctrl+I and Ctrl+Alt+0 in it leave the note as it was and the field focused, docked
        // and in the drawer. A window shortcut still acts from it. Every text field on the page
        // but the editor is one of `focus::FIELDS`.
        let search = |rt: &mut Runtime<App>| {
            rt.update(|s, tx| s.open_search(tx));
            settle(rt);
        };
        let fields: [(&str, &str, (f32, f32), fn(&mut Runtime<App>)); 4] = [
            ("the search field", "search", (1280.0, 800.0), search),
            ("the search field in the drawer", "search", (640.0, 480.0), search),
            ("find's field", "find-input", (1280.0, 800.0), |rt| key(rt, Key::Command('f'), ctrl(false))),
            ("the note's name", "rename", (1280.0, 800.0), |rt| {
                rt.update(|s, tx| s.start_rename(tx));
                settle(rt);
            }),
        ];
        for (name, field, size, open) in fields {
            let mut rt = garden(&dir, Mode::Source);
            rt.set_viewport(size, 1.0);
            settle(&mut rt);
            rt.update(|s, tx| s.focus_editor(tx));
            settle(&mut rt);
            open(&mut rt);
            let role = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(field)).map(|e| e.role);
            for e in rt.elements().into_iter().filter(|e| e.focusable && Some(e.role) == role) {
                let k = e.key.unwrap_or_default();
                if k != "editor" && !crate::focus::field(&k) {
                    fails.push(format!("{k} ({}) is a text field but not one of focus::FIELDS", e.path));
                }
            }
            let before = seen(&mut rt);
            for (c, alt) in [('b', false), ('l', false), ('i', false), ('0', true)] {
                key(&mut rt, Key::Command(c), Mods { alt, ..ctrl(false) });
                let after = seen(&mut rt);
                if after.text != before.text || focused(&rt).as_deref() != Some(field) {
                    let alt = if alt { "Alt+" } else { "" };
                    fails.push(format!("Ctrl+{alt}{} in {name}: the note changed {}, focus on {:?}", c.to_ascii_uppercase(), after.text != before.text, focused(&rt)));
                }
            }
            key(&mut rt, Key::Command(','), ctrl(false));
            if !seen(&mut rt).settings {
                fails.push(format!("Ctrl+, in {name} does not open Settings"));
            }
        }

        // A note that is not the open one, deleted by keys from its row in the drawer: the
        // drawer closes and the note has the keys, as for the open note.
        let mut rt = garden(&dir, Mode::Source);
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        rt.update(|s, tx| s.toggle_left(tx));
        settle(&mut rt);
        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        for k in [Key::Down, Key::Down, Key::Down, Key::Down, Key::Down, Key::Enter] {
            key(&mut rt, k, Mods::default());
        }
        let before = seen(&mut rt);
        for c in ["Q", "Q"] {
            text_key(&mut rt, c);
        }
        let after = seen(&mut rt);
        if rt.find("file:Box|a.md").is_some() || before.layers != 0 || !after.text.contains("QQ") {
            fails.push(format!("another note deleted from its row in the drawer: layers {}, focus {:?}, typing reached the note: {}", before.layers, focused(&rt), after.text.contains("QQ")));
        }

        // A drawer opened by its key shows the ring on the stop that has the keyboard.
        for (name, k, shift) in [("the sidebar's drawer", 'b', true), ("the outline's drawer", 'o', false)] {
            let mut rt = garden(&dir, Mode::Source);
            rt.set_viewport((640.0, 480.0), 1.0);
            settle(&mut rt);
            rt.update(|s, tx| s.focus_editor(tx));
            settle(&mut rt);
            key(&mut rt, Key::Command(k), ctrl(shift));
            let ring = rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.focus_visible));
            if seen(&mut rt).layers != 1 || !ring.as_ref().is_some_and(|(k, shown)| *shown && stop(k).is_some()) {
                fails.push(format!("{name} opened by its key: focus {ring:?}"));
            }
        }

        // Esc in the drawer's search field clears a query and the drawer stays (SB-92); Esc in
        // the empty field is the drawer's: it closes, and what had the keyboard before it
        // opened has it again (SESC).
        for mode in [Mode::Source, Mode::Preview] {
            for query in ["more", ""] {
                let mut rt = garden(&dir, mode);
                rt.set_viewport((640.0, 480.0), 1.0);
                settle(&mut rt);
                rt.update(|s, tx| s.keys_to_note(tx));
                settle(&mut rt);
                let home = focused(&rt);
                rt.update(|s, tx| s.open_search(tx));
                settle(&mut rt);
                for c in query.chars() {
                    text_key(&mut rt, &c.to_string());
                }
                let open = seen(&mut rt).layers;
                if !query.is_empty() {
                    key(&mut rt, Key::Escape, Mods::default());
                    let (q, .., layer) = search_state(&mut rt);
                    if !q.is_empty() || !layer || focused(&rt).as_deref() != Some("search") {
                        fails.push(format!("the first Esc in the drawer's search field ({mode:?}): query {q:?}, drawer {layer}, focus {:?}", focused(&rt)));
                    }
                }
                key(&mut rt, Key::Escape, Mods::default());
                let before = seen(&mut rt);
                for c in ["Q", "Q"] {
                    text_key(&mut rt, c);
                }
                let typed = seen(&mut rt).text.matches("QQ").count() == before.text.matches("QQ").count() + 1;
                if open != 1 || before.layers != 0 || focused(&rt) != home || (mode == Mode::Source && !typed) {
                    fails.push(format!("Esc in the drawer's search field ({mode:?}, query {query:?}): layers {open} then {}, focus {:?} (was {home:?}), typing reached the note: {typed}", before.layers, focused(&rt)));
                }
            }
        }

        // The drawer's "+" opens the note's name behind the drawer: the drawer closes and the
        // typed name goes into the field, not into the new note.
        let mut rt = garden(&dir, Mode::Source);
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        rt.update(|s, tx| s.toggle_left(tx));
        settle(&mut rt);
        rt.update(|_, tx| tx.focus("new-note", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, Mods::default());
        for c in ["Q", "Q"] {
            text_key(&mut rt, c);
        }
        let mut name = None;
        rt.update_if(|s, _| {
            name = s.renaming.as_ref().map(|e| e.text.clone());
            false
        });
        let after = seen(&mut rt);
        if after.layers != 0 || focused(&rt).as_deref() != Some("rename") || name.as_deref() != Some("QQ") || after.text.contains("QQ") {
            fails.push(format!("the drawer's New note button: layers {}, focus {:?}, name {name:?}, note {:?}", after.layers, focused(&rt), after.text));
        }

        // A new folder named in the drawer: the drawer stays, and a row of the tree has the
        // keyboard with its ring: the new folder's after Enter; after Esc the folder whose
        // New folder here opened the field, else the tree's current row.
        for (how, parent, last, want) in [("Enter", "", Key::Enter, Some("dir:Zed")), ("Esc", "", Key::Escape, None), ("Enter here", "Box", Key::Enter, Some("dir:Box|Zed")), ("Esc here", "Box", Key::Escape, Some("dir:Box"))] {
            let mut rt = garden(&dir, Mode::Source);
            rt.set_viewport((640.0, 480.0), 1.0);
            settle(&mut rt);
            rt.update(|s, tx| s.toggle_left(tx));
            settle(&mut rt);
            rt.update(move |s, tx| s.start_new_folder(parent, tx));
            settle(&mut rt);
            let field = focused(&rt);
            for c in ["Z", "e", "d"] {
                text_key(&mut rt, c);
            }
            key(&mut rt, last, Mods::default());
            let ring = rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.focus_visible));
            let right = ring.as_ref().is_some_and(|(k, shown)| *shown && want.map_or(k.starts_with("file:") || k == "tree", |w| k == w));
            if field.as_deref() != Some("new-folder") || seen(&mut rt).layers != 1 || rt.find("new-folder").is_some() || !right {
                fails.push(format!("{how} in a new folder's name in the drawer: field {field:?}, layers {}, focus {ring:?}", seen(&mut rt).layers));
            }
            let _ = std::fs::remove_dir_all(dir.join("notes/Zed"));
            let _ = std::fs::remove_dir_all(dir.join("notes/Box/Zed"));
        }

        let _ = std::fs::remove_dir_all(&dir);
        assert!(fails.is_empty(), "{} failed:\n{}", fails.len(), fails.join("\n"));
    }

    #[test]
    fn task_toggle_keeps_block_key_copy_path_tabs_and_search_sync() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        // 1. Toggling a task checkbox keeps the list's block key stable (no unmount/reflow).
        let before = crate::blocks::build("- [ ] Alpha\n- [ ] Beta\n\nAfter.\n");
        let after = crate::blocks::build("- [x] Alpha\n- [ ] Beta\n\nAfter.\n");
        assert_eq!(before.blocks[0].key, after.blocks[0].key, "toggling a task must keep the list's block key");

        // 2. Copy-path button, tab switching (Ctrl+1..9, Ctrl+Tab, Ctrl+Shift+Tab), and Search selection sync.
        let dir = std::env::temp_dir().join(format!("quire-ux-fixes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut rt = garden(&dir, Mode::Preview);
        rt.update(|s, tx| {
            s.open_file("note.md", None, tx);
            s.open_file("Box/a.md", None, tx);
            s.open_file("Box/b.md", None, tx);
        });
        settle(&mut rt);

        // Copy path button in titlebar breadcrumb copies full path and toasts.
        let copy_xy = rt.find("copy-path").expect("copy-path button in crumb");
        rt.pointer_move(copy_xy.0, copy_xy.1);
        rt.pointer_down(copy_xy.0, copy_xy.1);
        rt.pointer_up(copy_xy.0, copy_xy.1);
        settle(&mut rt);
        let mut toast_msg = None;
        rt.update_if(|s, _| {
            toast_msg = s.reader.toast.as_ref().map(|(m, _)| m.clone());
            false
        });
        assert_eq!(toast_msg.as_deref(), Some("Copied path"));

        // Ctrl+1..3 and Ctrl+Tab / Ctrl+Shift+Tab switch tabs in Open.
        let active = |rt: &mut Runtime<App>| -> Option<String> {
            let mut a = None;
            rt.update_if(|s, _| {
                a = s.active.clone();
                false
            });
            a
        };
        key(&mut rt, Key::Command('1'), ctrl(false));
        assert_eq!(active(&mut rt).as_deref(), Some("note.md"), "Ctrl+1 switches to tab 1");
        key(&mut rt, Key::Tab, ctrl(false));
        assert_eq!(active(&mut rt).as_deref(), Some("Box/a.md"), "Ctrl+Tab switches to next tab");
        key(&mut rt, Key::Tab, ctrl(false));
        assert_eq!(active(&mut rt).as_deref(), Some("Box/b.md"), "Ctrl+Tab advances to tab 3");
        key(&mut rt, Key::Tab, ctrl(false));
        assert_eq!(active(&mut rt).as_deref(), Some("note.md"), "Ctrl+Tab wraps to tab 1");
        key(&mut rt, Key::Tab, ctrl(true));
        assert_eq!(active(&mut rt).as_deref(), Some("Box/b.md"), "Ctrl+Shift+Tab wraps backward to tab 3");
        key(&mut rt, Key::Command('2'), ctrl(false));
        assert_eq!(active(&mut rt).as_deref(), Some("Box/a.md"), "Ctrl+2 switches to tab 2");

        // Search selection follows keyboard outline on the exact same frame (no 1-step poll lag).
        rt.update(|s, tx| s.open_search(tx));
        settle(&mut rt);
        for ch in ["#", "k", "e", "p", "t"] {
            text_key(&mut rt, ch);
        }
        settle(&mut rt);
        // Single-frame Down from search -> first result, then Down -> second result.
        rt.key(KeyEvent { key: Key::Down, mods: Mods::default() });
        rt.frame(1.0 / 60.0);
        let f1 = focused(&rt);
        let mut rf1 = None;
        rt.update_if(|s, _| {
            rf1 = s.res_focus.clone();
            false
        });
        assert_eq!(rf1, f1, "res_focus matches focused row immediately on Down from search");
        rt.key(KeyEvent { key: Key::Down, mods: Mods::default() });
        rt.frame(1.0 / 60.0);
        let f2 = focused(&rt);
        let mut rf2 = None;
        rt.update_if(|s, _| {
            rf2 = s.res_focus.clone();
            false
        });
        assert_ne!(f2, f1, "second Down moved focus to next result");
        assert_eq!(rf2, f2, "res_focus follows keyboard outline in the exact same frame");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sheet_enter_on_last_line_scrolls_caret_into_view() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-sheet-scroll-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let body = (1..=45).map(|i| format!("Line {i:02} of the note inside the edit modal.")).collect::<Vec<_>>().join("\n");
        let (mut rt, _) = crate::bench::open(&dir, &format!("# Long\n\n{body}\n"), Mode::Preview);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        settle(&mut rt);

        // Open section edit sheet (Shift+E), jump to end, and press Enter 3 times.
        text_key(&mut rt, "E");
        key(&mut rt, Key::End, ctrl(false));
        for step in 0..3 {
            key(&mut rt, Key::Enter, Mods::default());
            let els = rt.elements();
            let scroll = els.iter().find(|e| e.key.as_deref() == Some("sheet-scroll")).expect("sheet-scroll").rect;
            let caret = els.iter().find(|e| e.key.as_deref() == Some("sheet-editor")).expect("sheet-editor").caret.expect("sheet caret");
            assert!(
                caret[1] >= scroll[1] - 0.5 && caret[1] + caret[3] <= scroll[1] + scroll[3] + 0.5,
                "enter {step}: caret {caret:?} hidden outside sheet-scroll {scroll:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unified_keymap_modes_bare_keys_reopen_and_sidebar_f2() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-unified-modes-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        let mode_of = |rt: &mut Runtime<App>| {
            let mut m = Mode::Source;
            rt.update_if(|s, _| {
                m = s.mode;
                false
            });
            m
        };
        // Idempotent mode keys: Ctrl+E stays in Source, Ctrl+P goes to Preview (and stays),
        // bare m goes to Map (and Ctrl+M stays), bare p returns to Preview, Esc stays in Preview.
        key(&mut rt, Key::Command('e'), ctrl(false));
        assert_eq!(mode_of(&mut rt), Mode::Source, "Ctrl+E in Source is idempotent");
        key(&mut rt, Key::Command('p'), ctrl(false));
        assert_eq!(mode_of(&mut rt), Mode::Preview, "Ctrl+P switches to Preview");
        key(&mut rt, Key::Command('p'), ctrl(false));
        assert_eq!(mode_of(&mut rt), Mode::Preview, "Ctrl+P in Preview is idempotent");
        text_key(&mut rt, "m");
        assert_eq!(mode_of(&mut rt), Mode::Map, "bare m in Preview switches to Map");
        key(&mut rt, Key::Command('m'), ctrl(false));
        assert_eq!(mode_of(&mut rt), Mode::Map, "Ctrl+M in Map is idempotent");
        text_key(&mut rt, "p");
        assert_eq!(mode_of(&mut rt), Mode::Preview, "bare p in Map switches to Preview");

        // Bare non-edit keys in Preview: b toggles sidebar, z toggles zen, , opens settings, q does not close tab.
        let mut left0 = false;
        rt.update_if(|s, _| {
            left0 = s.persist.left_open;
            false
        });
        text_key(&mut rt, "b");
        let mut left1 = false;
        rt.update_if(|s, _| {
            left1 = s.persist.left_open;
            false
        });
        assert_ne!(left0, left1, "bare b toggles sidebar in Preview");
        text_key(&mut rt, "b");

        text_key(&mut rt, "z");
        let mut zen = false;
        rt.update_if(|s, _| {
            zen = s.focus;
            false
        });
        assert!(zen, "bare z enters zen mode in Preview");
        key(&mut rt, Key::Escape, Mods::default());
        rt.update_if(|s, _| {
            zen = s.focus;
            false
        });
        assert!(!zen && mode_of(&mut rt) == Mode::Preview, "Esc leaves zen and stays in Preview");

        text_key(&mut rt, ",");
        assert!(seen(&mut rt).settings, "bare , opens Settings in Preview");
        key(&mut rt, Key::Escape, Mods::default());

        // Bare q in Preview does not close the tab; Ctrl+W closes it and Ctrl+Shift+T reopens it.
        rt.update(|s, tx| s.open_file("Box/a.md", None, tx));
        settle(&mut rt);
        let tabs = |rt: &mut Runtime<App>| {
            let mut t = Vec::new();
            rt.update_if(|s, _| {
                t = s.tabs.clone();
                false
            });
            t
        };
        let before_tabs = tabs(&mut rt);
        text_key(&mut rt, "q");
        assert_eq!(tabs(&mut rt), before_tabs, "bare q does not close a tab");
        key(&mut rt, Key::Command('w'), ctrl(false));
        assert!(!tabs(&mut rt).contains(&"Box/a.md".to_string()), "Ctrl+W closes active tab");
        key(&mut rt, Key::Command('t'), ctrl(true));
        assert!(tabs(&mut rt).contains(&"Box/a.md".to_string()), "Ctrl+Shift+T reopens closed tab");

        // Map: Ctrl+= zooms in, Ctrl+0 resets zoom to 100%, Ctrl+X cuts selected node.
        text_key(&mut rt, "m");
        assert_eq!(mode_of(&mut rt), Mode::Map);
        let zoom_of = |rt: &mut Runtime<App>| {
            let mut z: f32 = 0.0;
            rt.update_if(|s, _| {
                z = s.map_view().map_or(0.0, |v| v.zoom);
                false
            });
            z
        };
        key(&mut rt, Key::Command('='), ctrl(false));
        glide(&mut rt);
        let z_in = zoom_of(&mut rt);
        key(&mut rt, Key::Command('0'), ctrl(false));
        glide(&mut rt);
        let z_reset = zoom_of(&mut rt);
        assert!((z_reset - 1.0_f32).abs() < 0.01 && z_in > z_reset, "Ctrl+= / Ctrl+0 zoom in and reset Map: z_in={z_in}, z_reset={z_reset}");
        // Map: Ctrl+X cuts a non-root node from the note.
        let before_cut = seen(&mut rt).text;
        key(&mut rt, Key::Right, Mods::default());
        key(&mut rt, Key::Command('x'), ctrl(false));
        let after_cut = seen(&mut rt).text;
        assert!(after_cut.len() < before_cut.len(), "Ctrl+X cuts selected map branch");

        // Sidebar tree F2 starts renaming the focused note row.
        key(&mut rt, Key::Command('e'), ctrl(false));
        rt.update(|_, tx| tx.focus("file:Box|a.md", true));
        settle(&mut rt);
        key(&mut rt, Key::Function(2), Mods::default());
        let mut renaming = (false, None);
        rt.update_if(|s, _| {
            renaming = (s.renaming.is_some(), s.active.clone());
            false
        });
        assert_eq!(renaming, (true, Some("Box/a.md".to_string())), "F2 on sidebar note row starts rename");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writer_new_shortcuts_work() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-writer-new-keys-{}", std::process::id()));
        let (mut rt, _) = crate::bench::open(&dir, "alpha\nbeta\ngamma\n", Mode::Source);
        rt.on_key = Some(crate::keys::on_key);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| {
            s.cur_edit().set_selection(0, 5);
            s.focus_editor(tx);
        });
        settle(&mut rt);
        // Ctrl+Shift+C wraps inline code.
        key(&mut rt, Key::Command('c'), ctrl(true));
        assert!(seen(&mut rt).text.starts_with("`alpha`"), "Ctrl+Shift+C wraps inline code: {:?}", seen(&mut rt).text);
        // Ctrl+Shift+Q toggles blockquote on line 1.
        key(&mut rt, Key::Command('q'), ctrl(true));
        assert!(seen(&mut rt).text.starts_with("> `alpha`"), "Ctrl+Shift+Q toggles blockquote: {:?}", seen(&mut rt).text);
        // Ctrl+Shift+Q again unwraps blockquote.
        key(&mut rt, Key::Command('q'), ctrl(true));
        assert!(seen(&mut rt).text.starts_with("`alpha`"), "Ctrl+Shift+Q removes blockquote: {:?}", seen(&mut rt).text);
        // Alt+Down moves line 1 below line 2.
        key(&mut rt, Key::Down, Mods { alt: true, ..Mods::default() });
        assert!(seen(&mut rt).text.starts_with("beta\n`alpha`\n"), "Alt+Down moves line down: {:?}", seen(&mut rt).text);
        // Alt+Up moves it back up.
        key(&mut rt, Key::Up, Mods { alt: true, ..Mods::default() });
        assert!(seen(&mut rt).text.starts_with("`alpha`\nbeta\n"), "Alt+Up moves line up: {:?}", seen(&mut rt).text);
        // Ctrl+Shift+Enter toggles task checkbox.
        key(&mut rt, Key::Enter, ctrl(true));
        assert!(seen(&mut rt).text.starts_with("- [ ] `alpha`"), "Ctrl+Shift+Enter adds task checkbox: {:?}", seen(&mut rt).text);
        key(&mut rt, Key::Enter, ctrl(true));
        assert!(seen(&mut rt).text.starts_with("- [x] `alpha`"), "Ctrl+Shift+Enter checks task: {:?}", seen(&mut rt).text);
        // Ctrl+Shift+L inserts wiki link around selection.
        rt.update(|s, _| {
            let t = s.active_text().to_string();
            let pos = t.find("beta").unwrap();
            s.cur_edit().set_selection(pos, pos + 4);
        });
        key(&mut rt, Key::Command('l'), ctrl(true));
        assert!(seen(&mut rt).text.contains("[[beta]]"), "Ctrl+Shift+L wraps wiki link: {:?}", seen(&mut rt).text);
        // F9 toggles typewriter scrolling; Ctrl+Shift+D toggles paragraph focus.
        let mut modes0 = (false, false);
        rt.update_if(|s, _| {
            modes0 = (s.settings().typewriter_scroll, s.settings().paragraph_focus);
            false
        });
        key(&mut rt, Key::Function(9), Mods::default());
        key(&mut rt, Key::Command('d'), ctrl(true));
        let mut modes1 = (false, false);
        rt.update_if(|s, _| {
            modes1 = (s.settings().typewriter_scroll, s.settings().paragraph_focus);
            false
        });
        assert_eq!(modes1, (!modes0.0, !modes0.1), "F9 and Ctrl+Shift+D toggle typewriter and paragraph focus");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_keybinding_editor_rebinds_validates_and_persists() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-key-editor-{}", std::process::id()));
        let mut rt = garden(&dir, Mode::Source);
        // Open Settings -> Shortcuts.
        key(&mut rt, Key::Command(','), ctrl(false));
        press_key(&mut rt, "nav-Shortcuts", false);
        glide(&mut rt);
        // Start recording for app.command-palette.
        press_key(&mut rt, "key-edit-app.command-palette", false);
        assert!(rt.find("key-rec-app.command-palette").is_some(), "recording indicator shows");
        // Pressing a conflicting chord (Ctrl+S) keeps recording open with an error.
        key(&mut rt, Key::Command('s'), ctrl(false));
        let mut err = None;
        rt.update_if(|s, _| {
            err = s.key_edit.as_ref().and_then(|e| e.error.clone());
            false
        });
        assert!(err.as_deref().is_some_and(|m| m.contains("Save")), "conflict error names conflicting action: {err:?}");
        // Pressing a free chord (Ctrl+Shift+P) binds and saves to keybindings.json.
        key(&mut rt, Key::Command('p'), ctrl(true));
        let mut bound: Vec<String> = Vec::new();
        rt.update_if(|s, _| {
            bound = s.keymap.chords("app.command-palette").into_iter().map(str::to_string).collect();
            false
        });
        assert_eq!(bound, vec!["Ctrl+Shift+P".to_string()], "chord rebound");
        let reloaded = crate::keymap::Overrides::load();
        assert_eq!(reloaded.chords("app.command-palette"), vec!["Ctrl+Shift+P"], "persisted to keybindings.json");
        // Close Settings and verify the new chord opens the palette while the old Ctrl+K does not.
        key(&mut rt, Key::Escape, Mods::default());
        key(&mut rt, Key::Command('k'), ctrl(false));
        assert!(!seen(&mut rt).palette, "old Ctrl+K no longer opens palette");
        key(&mut rt, Key::Command('p'), ctrl(true));
        assert!(seen(&mut rt).palette, "new Ctrl+Shift+P opens palette");
        key(&mut rt, Key::Escape, Mods::default());
        // Reset all restores default Ctrl+K.
        key(&mut rt, Key::Command(','), ctrl(false));
        press_key(&mut rt, "nav-Shortcuts", false);
        glide(&mut rt);
        press_key(&mut rt, "keys-reset-all", false);
        rt.update_if(|s, _| {
            bound = s.keymap.chords("app.command-palette").into_iter().map(str::to_string).collect();
            false
        });
        assert_eq!(bound, vec!["Ctrl+K".to_string()], "Reset all restores default");
        let _ = std::fs::remove_dir_all(&dir);
    }

}
