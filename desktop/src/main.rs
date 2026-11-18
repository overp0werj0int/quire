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
mod images;
mod keymap;
mod keys;
mod layer;
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

    /// The sidebar's panel tabs read as tabs, not as section labels: the open one carries a mark
    /// and the others are a step brighter than a label. With no note open, the Open label goes
    /// with its list.
    #[test]
    fn panel_tabs_differ_from_section_labels() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-paneltabs-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        rt.set_viewport((1280.0, 800.0), 1.0);
        rt.update(|s, tx| s.open_file("note.md", None, tx));
        for _ in 0..20 {
            rt.frame(DT);
        }
        let els = rt.elements();
        let text = |t: &str| els.iter().find(|e| e.text.as_deref() == Some(t)).unwrap_or_else(|| panic!("{t}")).text_color;
        assert_ne!(text("OPEN"), text("SEARCH"), "an inactive tab is not set as a label");
        let path = |key: &str| els.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("{key}")).path.clone();
        let marks: Vec<String> = els.iter().filter(|e| e.key.as_deref() == Some("tab-mark") && !e.exiting).map(|e| e.path.clone()).collect();
        assert_eq!(marks.len(), 1, "one tab is marked");
        assert!(marks[0].starts_with(&path("panel-files")), "the open one");
        // The three tabs are one segmented control: a faint track behind all three, the open
        // tab's pill inside it with the same radius, the New folder button outside it.
        let rect = |key: &str| els.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("{key}"));
        let track = rect("panel-seg");
        let within = |a: [f32; 4], b: [f32; 4]| a[0] >= b[0] && a[1] >= b[1] && a[0] + a[2] <= b[0] + b[2] && a[1] + a[3] <= b[1] + b[3];
        assert!(track.fill.a > 0.0, "the track has a fill");
        for tab in ["panel-files", "panel-search", "panel-tags", "tab-mark"] {
            assert!(within(rect(tab).rect, track.rect), "{tab} {:?} sits on the track {:?}", rect(tab).rect, track.rect);
        }
        assert_eq!(rect("tab-mark").radius, track.radius, "the pill and the track share a radius");
        let folder = rect("new-folder-button").rect;
        assert!(folder[0] >= track.rect[0] + track.rect[2], "the New folder button {folder:?} is outside the track {:?}", track.rect);
        rt.update(|s, _| s.close_tab("note.md"));
        for _ in 0..20 {
            rt.frame(DT);
        }
        assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("OPEN")), "no Open label over an empty list");
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

    /// KT: a title bar tooltip names its button and its key on one line in the panel family,
    /// whose characters share one width, as the key does in the footer and the palette.
    #[test]
    fn tooltips_name_the_key_in_the_panel_family() {
        use crate::bench::DT;
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-tooltips-{}", std::process::id()));
        let mut rt = vault(&dir, &[("note.md", "# A\n")], crate::watch::EVERY);
        let line = crate::ui::styles::panel_line(11.5);
        let mut per_char = Vec::new();
        for button in ["toggle-right", "toggle-left", "mode"] {
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
        assert!(hi - lo < 0.25, "every character is one width, as in a monospace: {per_char:?}");
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
    /// the search panel's hint wraps inside the sidebar.
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
        let side = rect(&rt, "sidebar-over");
        // The hint is 11 pt in the panel family, 0.6 em a character, inside 12 pt each side,
        // 10 pt above and 4 below: its lines hold every character, and it ends in the sidebar.
        let line = crate::ui::styles::panel_line(11.0);
        for (t, r) in texts(&rt, "search-top") {
            assert!(r[0] + r[2] <= side[0] + side[2] + 0.5, "the search hint '{t}' ends at {} past the sidebar at {}", r[0] + r[2], side[0] + side[2]);
            let (lines, per_line) = (((r[3] - 14.0) / line).round(), ((r[2] - 24.0) / (0.6 * 11.0)).floor());
            assert!(lines * per_line >= t.chars().count() as f32, "the search hint '{t}' runs past its {lines} line(s) of {per_line} characters");
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

    /// The sidebar's small buttons and the panel tabs take a pointer over at least 24 by 24
    /// points, and the rows they sit in keep their height.
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
            if matches!(k, "x" | "add" | "new-folder-button" | "panel-files" | "panel-search" | "panel-tags") {
                assert!(e.rect[2] >= 24.0 && e.rect[3] >= 24.0, "{k} is {:?}", e.rect);
                seen.insert(k.to_string());
            }
            if k.starts_with("dir:") || k.starts_with("tab:") {
                assert_eq!(e.rect[3], 28.0, "{k} keeps its height");
            }
        }
        assert_eq!(seen.len(), 6, "every kind was measured: {seen:?}");
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
        // The tooltips under a row, but for its close button's.
        let tips = |rt: &Runtime<crate::app::App>, row: &str| -> Vec<String> {
            rt.elements().into_iter().filter(|e| e.path.split('/').any(|seg| seg == row) && !e.path.ends_with("/x")).filter_map(|e| e.help).collect()
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
        let pairs = [("toggle-left", "app.toggle-sidebar"), ("toggle-right", "app.toggle-outline"), ("mode", "app.toggle-mode"), ("focus", "app.focus-mode"), ("fmt-image", "app.insert-image"), ("fmt-lock", "")];
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
            for (key, action) in pairs {
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
        let (x, y) = rt.find("mode").expect("the mode button");
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
        for (button, key) in [("mode", "preview"), ("toggle-left", "sidebar")] {
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
                out = Some((s.tabs.clone(), s.active.clone(), v.files.contains_key("Drafts/b.md"), v.folders.iter().any(|f| f == "Drafts")));
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
        let label = crate::keymap::label(crate::keymap::Preset::Omarky, "app.undo-delete");
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
        for key in ["foot-new", "foot-settings"] {
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
        let at = rt.find("mode").expect("the mode button");
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
        let at = rt.find("mode").expect("the mode button");
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
            assert!(s.lock_error().is_some_and(|e| e.contains("at least 8")), "a short password is refused");
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
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
                rt.update(|s, tx| {
                    if s.mode != mode {
                        s.toggle_mode(tx);
                    }
                });
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
            ["mode", "focus", "toggle-right"].map(|k| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).expect(k).rect)
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
        rt.update(|_, tx| tx.focus("vault-button", true));
        rt.frame(DT);
        key(&mut rt, Key::Enter);
        assert_eq!(marked(&rt), Some(("menu-switch".into(), true)), "Enter opened the folder menu, first item marked");
        key(&mut rt, Key::Down);
        assert_eq!(marked(&rt), Some(("menu-settings".into(), true)));
        key(&mut rt, Key::Down);
        assert_eq!(marked(&rt), Some(("menu-switch".into(), true)), "Down wraps");
        key(&mut rt, Key::Escape);
        let mut open = true;
        rt.update_if(|s, _| {
            open = s.layer().is_some();
            false
        });
        assert!(!open, "Esc closed it");
        assert_eq!(marked(&rt).map(|m| m.0), Some("vault-button".into()), "and the folder's name has the keyboard");

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
        assert_eq!(marked(&rt), Some(("tm-2".into(), true)), "d jumps to Delete note");
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
            Overlay { name: "palette", preview: false, size: wide, light: false, back: None, open: |rt| key(rt, Key::Command('k'), ctrl(true)) },
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
            Overlay { name: "folder menu", preview: false, size: wide, light: true, back: None, open: |rt| press_key(rt, "vault-button", false) },
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
            ("Ctrl+Shift+K", |rt| key(rt, Key::Command('k'), ctrl(true))),
            ("Ctrl+,", |rt| key(rt, Key::Command(','), ctrl(false))),
            ("Ctrl+F", |rt| key(rt, Key::Command('f'), ctrl(false))),
            ("Ctrl+P", |rt| key(rt, Key::Command('p'), ctrl(false))),
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
                    "Ctrl+Shift+K" | "Ctrl+," | "Ctrl+F" if o.light => {
                        let acted = match *input {
                            "Ctrl+Shift+K" => after.palette,
                            "Ctrl+," => after.settings,
                            _ => after.find,
                        };
                        if !acted {
                            fail("the shortcut did nothing");
                        }
                    }
                    "Ctrl+P" if o.light => {
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
            ("the mode button", |rt| press_key(rt, "mode", false)),
            ("Esc", |rt| key(rt, Key::Escape, Mods::default())),
            ("Ctrl+P, the palette, Esc", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                key(rt, Key::Command('k'), ctrl(true));
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
                press_key(rt, "vault-button", false);
                key(rt, Key::Escape, Mods::default());
            }),
            ("Ctrl+P, the folder menu by a click, a press outside", |rt| {
                key(rt, Key::Command('p'), ctrl(false));
                press_key(rt, "vault-button", false);
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
            press_key(&mut rt, "tm-2", false);
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
        key(&mut rt, Key::Command('k'), ctrl(true));
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
    #[test]
    fn the_sidebar_lists_work_from_the_keyboard() {
        use crate::app::Panel;
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
            for _ in 0..12 {
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
                out = Some((s.active.clone().unwrap_or_default(), s.mode, s.expanded.contains("Box"), s.left_panel, s.query.text.clone(), s.open_layers().count()));
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

        // The tags: Enter shows the tag's notes, and the keyboard follows to them.
        rt.update(|s, _| s.left_panel = Panel::Tags);
        settle(&mut rt);
        assert_eq!(tab_to(&mut rt, |k| k.starts_with("tag-row:")), "tag-row:kept");
        down(&mut rt, Key::Enter);
        let (.., panel, query, _) = state(&mut rt);
        assert_eq!((panel, query.as_str()), (Panel::Search, "#kept"), "Enter on a tag searches for it");
        let first = focused(&rt).unwrap_or_default();
        assert!(first.starts_with("res:"), "the keyboard goes to the results: {first}");

        // The search results: Down reaches a match, and Enter opens the note at it.
        down(&mut rt, Key::Down);
        assert_eq!(focused(&rt), Some(format!("hit:{}:0", &first["res:".len()..])), "Down moves to a match");
        down(&mut rt, Key::Enter);
        let (_, mode, ..) = state(&mut rt);
        assert_eq!(mode, Mode::Source, "a match opens in the writer");
        assert!(seen(&mut rt).editor_focused, "with the keyboard in the editor");

        // A list with no menu leaves Shift+F10 alone: no block menu opens under it.
        rt.update(|s, _| s.mode = Mode::Preview);
        settle(&mut rt);
        tab_to(&mut rt, |k| k.starts_with("tab:"));
        key(&mut rt, Key::Function(10), Mods { shift: true, ..Mods::default() });
        assert_eq!(state(&mut rt).5, 0, "Shift+F10 on an Open row opens nothing");

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
            key(&mut rt, Key::Command('k'), ctrl(true));
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
            ("a panel tab", |rt| rt.update(|_, tx| tx.focus("panel-files", true))),
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

    /// The panel tabs are one Tab stop, as the toolbar is (TABSTOP): Tab lands on the open tab,
    /// whichever it is, and Tab or Shift+Tab leaves the control. A focused tab's ring follows
    /// its pill and stays inside the track (TABRING), and a closed tab's stands off the open
    /// pill beside it (RINGOV, RINGGAP).
    #[test]
    fn the_panel_tabs_are_one_tab_stop() {
        use crate::app::Panel;
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("quire-panel-stop-{}", std::process::id()));
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).map(|e| e.rect);
        for (panel, open) in [(Panel::Files, "panel-files"), (Panel::Search, "panel-search"), (Panel::Tags, "panel-tags")] {
            let mut rt = garden(&dir, Mode::Preview);
            rt.update(move |s, _| s.left_panel = panel);
            settle(&mut rt);
            let mut tabs = Vec::new();
            let mut next = None;
            for _ in 0..12 {
                key(&mut rt, Key::Tab, Mods::default());
                let f = focused(&rt).unwrap_or_default();
                if let Some(prev) = tabs.last()
                    && next.is_none()
                    && f != *prev
                {
                    next = Some(f.clone());
                }
                if f.starts_with("panel-") {
                    tabs.push(f);
                }
            }
            assert!(!tabs.is_empty() && tabs.iter().all(|k| k == open), "{panel:?}: Tab lands only on the open tab: {tabs:?}");
            assert!(next.is_some_and(|k| !k.starts_with("panel-")), "{panel:?}: Tab leaves the tabs");
            rt.update(move |_, tx| tx.focus(open, true));
            settle(&mut rt);
            key(&mut rt, Key::Tab, Mods { shift: true, ..Mods::default() });
            assert!(!focused(&rt).unwrap_or_default().starts_with("panel-"), "{panel:?}: Shift+Tab leaves the tabs: {:?}", focused(&rt));
            key(&mut rt, Key::Tab, Mods::default());
            assert_eq!(focused(&rt).as_deref(), Some(open), "{panel:?}: Tab comes back to the open tab");
            // The ring is drawn inside the open tab's pill, its 2 pt stroke ending at the pill's
            // edge, so it stays in the track and off the pills beside it (RINGOV).
            let seg = rect(&rt, "panel-seg").expect("the track");
            let ring = |rt: &Runtime<App>, tab: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("tab-ring") && e.path.contains(tab)).expect("the tab's ring").rect;
            let (r, mark) = (ring(&rt, open), rect(&rt, "tab-mark").expect("the open tab's pill"));
            let outer = [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0];
            assert!(outer.iter().zip(mark).all(|(a, b)| (a - b).abs() < 0.01), "{panel:?}: the ring {outer:?} is the open tab's pill {mark:?}");
            assert!(outer[1] > seg[1] && outer[1] + outer[3] < seg[1] + seg[3] && outer[0] > seg[0] && outer[0] + outer[2] < seg[0] + seg[2], "{panel:?}: the ring {outer:?} stays clear of the track's edge {seg:?}");
            // On a tab beside the open one, at least 1 pt of the track parts the ring from the
            // open tab's pill (RINGGAP).
            key(&mut rt, Key::Right, Mods::default());
            let side = focused(&rt).expect("a tab beside");
            let r = ring(&rt, &side);
            let outer = [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0];
            assert!(side != open && (outer[0] + outer[2] <= mark[0] - 1.0 + 0.01 || outer[0] >= mark[0] + mark[2] + 1.0 - 0.01), "{panel:?}: {side}'s ring {outer:?} stands 1 pt off the open pill {mark:?}");
            assert!(outer[1] > seg[1] && outer[1] + outer[3] < seg[1] + seg[3] && outer[0] > seg[0] && outer[0] + outer[2] < seg[0] + seg[2], "{panel:?}: {side}'s ring {outer:?} stays clear of the track's edge {seg:?}");
        }
        // Home and End go to the first and the last tab, and Up and Down do nothing: none of
        // them opens a panel or takes the keyboard out of the tabs.
        // Each case names the open panel too, one the key does not move to, so a key that
        // opened the tab it lands on would show.
        let cases = [
            (Key::Home, "Home", Panel::Search, "panel-tags", "panel-files"),
            (Key::End, "End", Panel::Files, "panel-files", "panel-tags"),
            (Key::End, "End", Panel::Search, "panel-files", "panel-tags"),
            (Key::Up, "Up", Panel::Files, "panel-files", "panel-files"),
            (Key::Down, "Down", Panel::Files, "panel-files", "panel-files"),
            (Key::Down, "Down", Panel::Tags, "panel-search", "panel-search"),
            (Key::Up, "Up", Panel::Search, "panel-tags", "panel-tags"),
        ];
        for (k, name, open, from, to) in cases {
            let mut rt = garden(&dir, Mode::Preview);
            rt.update(move |s, _| s.left_panel = open);
            settle(&mut rt);
            rt.update(move |_, tx| tx.focus(from, true));
            settle(&mut rt);
            key(&mut rt, k, Mods::default());
            settle(&mut rt);
            let mut panel = None;
            rt.update_if(|s, _| {
                panel = Some(s.left_panel);
                false
            });
            assert_eq!(focused(&rt).as_deref(), Some(to), "{name} from {from} with {open:?} open");
            assert_eq!(panel, Some(open), "{name} from {from} opens no panel");
        }
        // A layer that closes gives focus back to the tab it came from, open or not.
        let mut rt = garden(&dir, Mode::Preview);
        rt.update(|_, tx| tx.focus("panel-search", true));
        settle(&mut rt);
        key(&mut rt, Key::Command('k'), ctrl(true));
        settle(&mut rt);
        assert!(rt.find("pal-input").is_some(), "the palette opened");
        key(&mut rt, Key::Escape, Mods::default());
        settle(&mut rt);
        assert_eq!(focused(&rt).as_deref(), Some("panel-search"), "closing the palette gives the tab its focus back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The pointer anywhere in a Settings switch's 28 pt target, not only over its 20 pt track,
    /// gives the track its hover fill, on and off, and leaving the target takes it back.
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
        assert_eq!((sw.rect[3], track(&rt).rect[3]), (28.0, 20.0), "a 20 pt track in a 28 pt target");
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
            let view = rect(&rt, "doc-scroll").expect("the view");
            let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
            let by = y - (view[1] + 0.7 * view[3]);
            rt.update(|s, tx| {
                let at = s.reader.scroll.get()[0];
                tx.scroll_to_instant(crate::reader::SCROLL, 0.0, at + by);
            });
            glide(&mut rt);
            let (_, y) = rt.find(crate::reader::FIND_CUR).expect("the current match is drawn");
            assert!((y - (view[1] + 0.7 * view[3])).abs() < 1.0, "{mode:?}: the case puts the match at 70%: {y}");
            let band = rect(&rt, "toast-band").expect("the band");
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
        key(&mut rt, Key::Function(6), Mods { shift: true, ..Mods::default() });
        look(&mut rt, Key::End, "side-scroll", "the tree's last row");
        look(&mut rt, Key::Home, "side-scroll", "the tree's first row");
        rt.update(|s, tx| s.search_tag("many", tx));
        settle(&mut rt);
        key(&mut rt, Key::Function(6), Mods { shift: true, ..Mods::default() });
        assert!(focused(&rt).is_some_and(|k| k.starts_with("res:")), "Shift+F6 puts the keyboard on a result: {:?}", focused(&rt));
        look(&mut rt, Key::End, "side-scroll", "the last result");
        look(&mut rt, Key::Home, "side-scroll", "the first result");
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
            K::Up => Key::Up,
            K::Down => Key::Down,
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
        let wants = ["tab:", "vault-button", "panel-files", "new-folder-button", "file:", "foot-new", "foot-settings", "ol-", "bl-"];
        for want in wants {
            if !reached.iter().any(|k| k.starts_with(want)) {
                fails.push(format!("Tab never reaches {want}"));
            }
        }
        // The panel tabs are one Tab stop, the open tab (KEY-98).
        if let Some(k) = reached.iter().find(|k| *k == "panel-search" || *k == "panel-tags") {
            fails.push(format!("Tab reaches {k}, which is not the open tab"));
        }
        rt.update(|_, tx| tx.focus("panel-files", true));
        settle(&mut rt);
        key(&mut rt, Key::Right, Mods::default());
        if focused(&rt).as_deref() != Some("panel-search") {
            fails.push(format!("Right on FILES moves to {:?}", focused(&rt)));
        }
        key(&mut rt, Key::Tab, Mods::default());
        if focused(&rt).is_some_and(|k| k.starts_with("panel-")) {
            fails.push(format!("Tab from SEARCH stays in the tabs: {:?}", focused(&rt)));
        }
        rt.update(|_, tx| tx.focus("panel-files", true));
        settle(&mut rt);
        key(&mut rt, Key::Left, Mods::default());
        let mut panel = None;
        rt.update_if(|s, _| {
            panel = Some(s.left_panel);
            false
        });
        if focused(&rt).as_deref() != Some("panel-tags") || panel != Some(crate::app::Panel::Files) {
            fails.push(format!("Left on FILES moves to {:?} and opens {panel:?}", focused(&rt)));
        }
        // Enter or Space on a panel tab opens the panel and puts the keyboard in it (SRCH): the
        // search field, the tree's current row, the first tag.
        for (tab, k, want) in [("panel-files", Key::Text(" ".into()), "file:note.md"), ("panel-tags", Key::Enter, "tag-row:"), ("panel-files", Key::Enter, "file:note.md"), ("panel-search", Key::Text(" ".into()), "search")] {
            let k2 = tab.to_string();
            rt.update(move |_, tx| tx.focus(k2, true));
            settle(&mut rt);
            key(&mut rt, k.clone(), Mods::default());
            if !focused(&rt).is_some_and(|f| f.starts_with(want)) {
                fails.push(format!("{k:?} on {tab}: focus on {:?}, not {want}", focused(&rt)));
            }
        }
        rt.update(|_, tx| tx.focus("panel-search", true));
        settle(&mut rt);
        key(&mut rt, Key::Enter, Mods::default());
        if focused(&rt).as_deref() != Some("search") {
            fails.push(format!("Enter on SEARCH: focus on {:?}", focused(&rt)));
        }
        for c in ["k", "e", "p", "t"] {
            text_key(&mut rt, c);
        }
        key(&mut rt, Key::Down, Mods::default());
        let first = focused(&rt).unwrap_or_default();
        if !first.starts_with("res:") {
            fails.push(format!("Down in the search field: focus on {first}"));
        }
        key(&mut rt, Key::Up, Mods::default());
        if focused(&rt).as_deref() != Some("search") {
            fails.push(format!("Up from the first result: focus on {:?}", focused(&rt)));
        }
        let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("res:") || k.starts_with("hit:")).collect();
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
        for (mode, stops) in [(Mode::Preview, ["file:Box|a.md", "vault-button", "ol-0"]), (Mode::Source, ["file:Box|a.md", "vault-button", "fmt-bold"])] {
            let mut rt = garden(&dir, mode);
            let mut now = None;
            rt.update_if(|s, _| {
                now = Some((s.preset(), crate::keys::page_stack(s)));
                false
            });
            let (preset, ctxs) = now.expect("read");
            let mut chords = Vec::new();
            for a in crate::keymap::TABLE.iter().filter(|a| a.ctx.iter().any(|c| ctxs.contains(c)) && crate::keys::handler(a.id).is_some()) {
                if matches!(a.status, crate::keymap::Status::Deferred | crate::keymap::Status::Dropped) || matches!(a.id, "app.quit" | "app.insert-image") {
                    continue;
                }
                for c in a.chords(preset).iter().filter_map(|s| crate::keymap::parse(s)).filter(|seq| seq.len() == 1).map(|seq| seq[0]) {
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
                        key(&mut rt, Key::Command('b'), ctrl(true));
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
        for k in [Key::Down, Key::Down, Key::Enter] {
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
        for k in [Key::Down, Key::Down, Key::Down, Key::Enter] {
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
            ("vault-button", &side),
            ("panel-files", &side),
            ("panel-tags", &side),
            ("new-folder-button", &side),
            ("dir:Box", &side),
            ("file:Box|a.md", &side),
            ("foot-new", &side),
            ("foot-settings", &side),
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
            key(&mut rt, Key::Command('k'), ctrl(true));
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
            if !focused(&rt).is_some_and(|k| k.starts_with("wiki:") || k.starts_with("tag:")) {
                fails.push(format!("a sidebar row, then {how}: Tab goes to {:?}, not the note's link", focused(&rt)));
            }
        }

        // A search result or one of its matches, opened from preview, opens the writer with the
        // caret at the match and nothing selected, as find leaves it: the next key types there.
        for row in ["res:", "hit:"] {
            let mut rt = garden(&dir, Mode::Preview);
            rt.update(|s, tx| s.search_tag("kept", tx));
            settle(&mut rt);
            let Some(k) = rt.elements().into_iter().filter_map(|e| e.key).find(|k| k.starts_with(row)) else {
                fails.push(format!("no {row} row for #kept"));
                continue;
            };
            rt.update(move |_, tx| tx.focus(k, true));
            settle(&mut rt);
            key(&mut rt, Key::Enter, Mods::default());
            glide(&mut rt);
            let mut sel = (0, 0, 0);
            rt.update_if(|s, _| {
                let e = s.cur_edit();
                sel = (e.anchor, e.focus, e.text.find("#kept").unwrap_or(usize::MAX));
                false
            });
            let before = seen(&mut rt);
            text_key(&mut rt, "Q");
            let after = seen(&mut rt);
            if before.mode != Mode::Source || !before.editor_focused || sel.0 != sel.1 || sel.0 != sel.2 || after.text.matches("#kept").count() != before.text.matches("#kept").count() {
                fails.push(format!("Enter on a {row} row from preview: mode {:?}, editor focused {}, selection {sel:?} (anchor, caret, match), the next key kept the match: {}", before.mode, before.editor_focused, after.text.contains("#kept")));
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
            ("Esc in the search field", |rt| {
                rt.update(|s, tx| s.open_search(tx));
                settle(rt);
                for c in ["m", "o", "r", "e"] {
                    text_key(rt, c);
                }
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

        // A text field keeps the writer's edits at the caret from the note (FLD): Ctrl+B, Ctrl+K,
        // Ctrl+E, Ctrl+I and Ctrl+1 in it leave the note as it was and the field focused, docked
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
            for c in ['b', 'k', 'e', 'i', '1'] {
                key(&mut rt, Key::Command(c), ctrl(false));
                let after = seen(&mut rt);
                if after.text != before.text || focused(&rt).as_deref() != Some(field) {
                    fails.push(format!("Ctrl+{} in {name}: the note changed {}, focus on {:?}", c.to_ascii_uppercase(), after.text != before.text, focused(&rt)));
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
        for k in [Key::Down, Key::Down, Key::Down, Key::Enter] {
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
        for (name, k, shift) in [("the sidebar's drawer", 'b', true), ("the outline's drawer", '\\', false)] {
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

        // Esc in the drawer's search field is the drawer's: it closes, and what had the
        // keyboard before it opened has it again, with or without a query typed (SESC).
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

        // The drawer's New note button opens the note's name behind the drawer: the drawer
        // closes and the typed name goes into the field, not into the new note.
        let mut rt = garden(&dir, Mode::Source);
        rt.set_viewport((640.0, 480.0), 1.0);
        settle(&mut rt);
        rt.update(|s, tx| s.toggle_left(tx));
        settle(&mut rt);
        rt.update(|_, tx| tx.focus("foot-new", true));
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

}
