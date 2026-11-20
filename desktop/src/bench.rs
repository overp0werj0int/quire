//! The budgets (phase 9): the scroll bench (VER-32) and typing latency (WR-224). Both are
//! ignored tests, run on a release build:
//!
//! ```sh
//! cargo test --release bench -- --ignored --nocapture --test-threads 1
//! ```
//!
//! They time `Runtime::frame`, which runs the handlers, the view, layout and the paint list; the
//! GPU's raster is not in the figure.

use std::time::Instant;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{App, Mode};

pub(crate) const LONG: &str = include_str!("../tests/fixtures/omarky/long.md");
pub(crate) const DT: f64 = 1.0 / 60.0;

/// A runtime on a fresh folder holding `note.md` with `text`, opened in `mode`.
pub(crate) fn open(dir: &std::path::Path, text: &str, mode: Mode) -> (Runtime<App>, f64) {
    let _ = std::fs::remove_dir_all(dir);
    let root = dir.join("notes");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.md"), text).unwrap();
    // SAFETY: the ENV lock keeps other tests that read these variables out.
    unsafe {
        std::env::set_var("HOME", dir);
        std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
        std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        std::env::set_var("XDG_STATE_HOME", dir.join("state"));
    }
    let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1400.0, 1000.0), Env { system_dark: false, scale: 1.0 });
    rt.poll = Some(|s: &mut App, t, tx| s.poll(t, tx));
    rt.frame(DT);
    let t = Instant::now();
    rt.update(move |s, tx| {
        s.open_vault(root, tx);
        s.open_file("note.md", None, tx);
        s.mode = mode;
        // The keyboard goes with the mode, as `toggle_mode` sends it.
        s.keys_to_note(tx);
    });
    rt.frame(DT);
    let opened = ms(t);
    for _ in 0..3 {
        rt.frame(DT);
    }
    (rt, opened)
}

/// The frame's phases in ms (view, commit, layout, text, paint), with `--features profile`.
fn phases() -> String {
    #[cfg(feature = "profile")]
    {
        let p = rune::take_frame_phases();
        format!(" [view {:.1}, commit {:.1}, layout {:.1}, text {:.1}, paint {:.1}, apply {:.1}]", p[0], p[1], p[2], p[3], p[4], p[5])
    }
    #[cfg(not(feature = "profile"))]
    String::new()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// The block's type, and a code block's language.
fn kind(b: &crate::blocks::Block) -> String {
    let name = format!("{:?}", b.kind);
    let name = name.split([' ', '(', '{']).next().unwrap_or_default().to_string();
    match &b.kind {
        crate::blocks::Kind::Code { lang, .. } | crate::blocks::Kind::Diff { lang, .. } => format!("{name} {lang}"),
        _ => name,
    }
}

/// VER-32: `long.md` in preview, scrolled from the top in 120 px steps, each step laid out.
#[test]
#[ignore]
fn bench_ver_32_scroll() {
    let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("quire-bench-scroll-{}", std::process::id()));
    let t = Instant::now();
    let doc = crate::blocks::build(LONG);
    let parse = ms(t);
    let (mut rt, opened) = open(&dir, LONG, Mode::Preview);
    let mut steps = Vec::new();
    let mut slow = Vec::new();
    let mut y = 0.0;
    let (x0, y0) = rt.find(crate::reader::SCROLL).expect("the page");
    rt.pointer_move(x0, y0);
    rt.frame(DT);
    let total = Instant::now();
    for _ in 0..5000 {
        let mut max = 0.0;
        rt.update_if(|s, _| {
            max = s.reader.scroll.get()[1];
            false
        });
        if y >= max {
            break;
        }
        y = (y + 120.0f32).min(max);
        // A wheel step, as the platform delivers it: no view re-run unless the page asks.
        rt.wheel(x0, y0, -120.0);
        phases();
        let t = Instant::now();
        rt.frame(DT);
        let took = ms(t);
        let ph = phases();
        steps.push(took);
        if took > 8.3 && slow.len() < 12 {
            let mut at = String::new();
            rt.update_if(|s, _| {
                at = doc.blocks.get(s.top()).map(kind).unwrap_or_default();
                false
            });
            slow.push(format!("step {} at {y} px: {took:.1} ms, {at}{ph}", steps.len()));
        }
    }
    let total = ms(total);
    let worst = steps.iter().copied().fold(0.0, f64::max);
    let mean = steps.iter().sum::<f64>() / steps.len().max(1) as f64;
    let over = |l: f64| steps.iter().filter(|&&s| s > l).count();
    eprintln!("VER-32 scroll bench: {} lines, {} blocks", LONG.lines().count(), doc.blocks.len());
    eprintln!("  parse {parse:.1} ms, open to first full frame {opened:.1} ms");
    eprintln!("  steps {}, worst {worst:.1} ms, mean {mean:.1} ms, over 16 ms {}, over 8.3 ms {}, total {total:.0} ms", steps.len(), over(16.0), over(8.3));
    for s in &slow {
        eprintln!("  {s}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(steps.len() > 100, "the page scrolled");
    assert!(worst < 16.0 && mean < 1.0, "VER-32 budget: worst {worst:.1} ms, mean {mean:.1} ms");
}

/// WR-224: a keystroke to its frame in source mode, at 400, 1,000 and 3,000 lines of mixed
/// Markdown, typed in the middle of the note.
#[test]
#[ignore]
fn bench_wr_224_typing() {
    let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let mut fails = Vec::new();
    for (n, budget) in [(400, 8.0), (1000, 8.0), (3000, 16.0)] {
        let dir = std::env::temp_dir().join(format!("quire-bench-type-{n}-{}", std::process::id()));
        let text: String = LONG.lines().take(n).map(|l| format!("{l}\n")).collect();
        let (mut rt, _) = open(&dir, &text, Mode::Source);
        // The caret goes to the middle of the note.
        let mid = text.match_indices('\n').nth(n / 2).map_or(0, |(i, _)| i);
        rt.update(move |s, _| s.cur_edit().set_selection(mid, mid));
        let t2 = text.clone();
        rt.update_if(move |s, _| {
            let c = crate::roles::Roles::new(s.palette(false));
            let (sp, st, _) = s.sheet_spans(&t2, 16.0, &crate::writer::Ink::new(&c));
            eprintln!("  {n} lines: {} spans, {} strikes, {} paragraphs", sp.len(), st.len(), t2.lines().count());
            false
        });
        rt.frame(DT);
        let mut times = Vec::new();
        for i in 0..60 {
            let ch = if i % 6 == 5 { " " } else { "x" };
            rt.key(KeyEvent { key: Key::Text(ch.into()), mods: Mods::default() });
            phases();
            let t = Instant::now();
            rt.frame(DT);
            times.push(ms(t));
            if i == 30 {
                eprintln!("  a keystroke at {n} lines:{}", phases());
            }
        }
        let mut grew = 0;
        rt.update_if(|s, _| {
            grew = s.active_text().len() - text.len();
            false
        });
        times.sort_by(f64::total_cmp);
        let (median, p95, worst) = (times[times.len() / 2], times[times.len() * 95 / 100], times[times.len() - 1]);
        eprintln!("WR-224 typing at {n} lines: median {median:.1} ms, p95 {p95:.1} ms, worst {worst:.1} ms (budget {budget} ms), {grew} bytes typed");
        assert_eq!(grew, 60, "every key landed");
        if median >= budget {
            fails.push(format!("{n} lines: median {median:.1} ms"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(fails.is_empty(), "WR-224 budget: {fails:?}");
}

/// SB-57 to SB-71's cost (DESIGN §3): a folder of 1,000 notes of about 4 KB, the query typed a
/// letter at a time and taken back a letter at a time. Each keystroke is timed twice: the
/// search alone (`search::Cache::results`, as the first frame after it runs it) and the whole
/// frame that shows the results. Then the frame cost with the results cached: the view run
/// again with nothing changed, and the cache's answer alone.
#[test]
#[ignore]
fn bench_sb_search() {
    let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("quire-bench-search-{}", std::process::id()));
    let (mut rt, _) = open(&dir, "# Note\n", Mode::Source);
    let root = dir.join("notes");
    let words = ["paper", "window", "garden", "search", "letter", "number", "silver", "harbor", "season", "market", "planet", "candle"];
    for i in 0..999 {
        let mut text = format!("# Note {i}\n\n");
        let mut k: usize = i;
        while text.len() < 4096 {
            k = k.wrapping_mul(1_103_515_245).wrapping_add(12345);
            text.push_str(words[(k >> 16) % words.len()]);
            text.push(if k % 13 == 0 { '\n' } else { ' ' });
        }
        std::fs::create_dir_all(root.join(format!("d{}", i / 100))).unwrap();
        std::fs::write(root.join(format!("d{}/n{i:03}.md", i / 100)), text).unwrap();
    }
    rt.update(move |s, tx| {
        s.open_vault(root, tx);
        s.open_file("note.md", None, tx);
        s.persist.left_open = true;
    });
    for _ in 0..4 {
        rt.frame(DT);
    }
    let (files, bytes) = {
        let mut out = (0, 0);
        rt.update_if(|s, _| {
            out = (s.files().len(), s.files().values().map(String::len).sum::<usize>());
            false
        });
        out
    };
    println!("bench_sb_search: {files} notes, {:.1} MB", bytes as f64 / 1e6);
    // One keystroke: the query set, the search timed alone through a cache of the bench's own
    // that has seen the same queries as the app's, then the frame that shows it, which runs
    // the app's cache.
    let warm = std::cell::RefCell::new(crate::search::Cache::default());
    let step = |rt: &mut Runtime<App>, q: &str| -> (f64, f64, usize) {
        let q = q.to_string();
        let mut search = 0.0;
        let mut rows = 0;
        rt.update_if(|s, _| {
            let v = s.vault.as_ref().unwrap();
            let t = Instant::now();
            let r = warm.borrow_mut().results(v, &q);
            search = ms(t);
            rows = r.rows().count();
            false
        });
        let q2 = q.clone();
        rt.update(move |s, _| s.query = rune::TextEdit::new(q2));
        let t = Instant::now();
        rt.frame(DT);
        (search, ms(t), rows)
    };
    // The first search lowers every note.
    let (first, frame, _) = step(&mut rt, "s");
    println!("  first search, lowering every note: {first:.2} ms; its frame {frame:.2} ms");
    let _ = step(&mut rt, "x");
    let query = "season";
    let report = |label: &str, qs: Vec<&str>, rt: &mut Runtime<App>| {
        let mut worst = (0.0f64, 0.0f64);
        let mut sum = (0.0, 0.0);
        for q in &qs {
            let (s, f, rows) = step(rt, q);
            println!("    {q:<8} search {s:6.2} ms, frame {f:6.2} ms, {rows} rows");
            worst = (worst.0.max(s), worst.1.max(f));
            sum = (sum.0 + s, sum.1 + f);
        }
        let n = qs.len() as f64;
        println!("  {label}: search mean {:.2} ms, worst {:.2} ms; frame mean {:.2} ms, worst {:.2} ms", sum.0 / n, worst.0, sum.1 / n, worst.1);
    };
    rt.update(|s, _| s.query = rune::TextEdit::new(""));
    rt.frame(DT);
    println!("  narrowing, a letter at a time:");
    report("narrowing", (1..=query.len()).map(|i| &query[..i]).collect(), &mut rt);
    println!("  widening, a letter taken back at a time:");
    report("widening", (1..query.len()).rev().map(|i| &query[..i]).collect(), &mut rt);
    // Nothing changed: the cache answers with the last results, and a frame that runs the view
    // again costs what the rows cost to build.
    let mut hit = 0.0;
    rt.update_if(|s, _| {
        let t = Instant::now();
        for _ in 0..1000 {
            std::hint::black_box(s.results());
        }
        hit = ms(t);
        false
    });
    let mut view = Vec::new();
    for _ in 0..20 {
        rt.update(|_, _| {});
        let t = Instant::now();
        rt.frame(DT);
        view.push(ms(t));
    }
    view.sort_by(f64::total_cmp);
    println!("  cache hit: {:.2} µs per `App::results`; a frame that runs the view with the results cached: median {:.2} ms", hit, view[view.len() / 2]);
    // A keystroke in the writer while the results show: one note changes, so it is lowered
    // again and searched with the rest, and the rows are built again.
    rt.update(|s, tx| s.focus_editor(tx));
    rt.frame(DT);
    let mut typing = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        rt.key(KeyEvent { key: Key::Text("s".into()), mods: Mods::default() });
        rt.frame(DT);
        typing.push(ms(t));
    }
    typing.sort_by(f64::total_cmp);
    println!("  a keystroke in the writer with {query:?} showing: median {:.2} ms, worst {:.2} ms", typing[typing.len() / 2], typing[typing.len() - 1]);
    // The same with the search cleared and the lists showing, for the floor.
    rt.update(|s, tx| {
        s.query = rune::TextEdit::new("");
        s.focus_editor(tx);
    });
    rt.frame(DT);
    let mut plain = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        rt.key(KeyEvent { key: Key::Text("s".into()), mods: Mods::default() });
        rt.frame(DT);
        plain.push(ms(t));
    }
    plain.sort_by(f64::total_cmp);
    println!("  a keystroke in the writer with no search: median {:.2} ms, worst {:.2} ms", plain[plain.len() / 2], plain[plain.len() - 1]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// LIVEPACE: reproduces the long-note live workload (open in Preview, 4x Ctrl+P,
/// Ctrl+Shift+K Command Palette open/close, typing "livecheck ", Ctrl+S Save) with per-phase
/// timings under `--features profile`, both with panels closed (Note-mode geometry) and with
/// sidebar + outline open (Folder-mode geometry).
#[test]
#[ignore]
fn bench_livepace_long() {
    let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let text_1753: String = LONG.lines().take(1753).map(|l| format!("{l}\n")).collect();
    let run_case = |label: &str, text: &str, left_open: bool, right_open: bool| {
        let dir = std::env::temp_dir().join(format!("quire-bench-livepace-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Long.md"), text).unwrap();
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1920.0, 1080.0), Env { system_dark: false, scale: 1.0 });
        rt.on_key = Some(crate::keys::on_key);
        rt.poll = Some(|s: &mut App, t, tx| s.poll(t, tx));
        rt.frame(DT);
        phases();
        let t = Instant::now();
        rt.update(move |s, tx| {
            s.open_vault(root, tx);
            s.persist.left_open = left_open;
            s.persist.right_open = right_open;
            s.mode = Mode::Preview;
            s.open_file("Long.md", None, tx);
            s.keys_to_note(tx);
        });
        rt.frame(DT);
        let open_ms = ms(t);
        let open_ph = phases();
        for _ in 0..3 {
            rt.frame(DT);
        }
        let query_state = |rt: &mut Runtime<App>| -> (Mode, bool, usize, bool) {
            let mut out = (Mode::Preview, false, 0, false);
            rt.update_if(|s, _| {
                out = (s.mode, s.pal().is_some(), s.active_text().len(), !s.dirty.is_empty());
                false
            });
            out
        };
        assert_eq!(query_state(&mut rt).0, Mode::Preview);
        eprintln!("\n=== LIVEPACE ({label}: left={left_open}, right={right_open}, {} lines, {} bytes) ===", text.lines().count(), text.len());
        eprintln!("  open Preview:           {open_ms:6.2} ms{open_ph}");

        let step_key = |rt: &mut Runtime<App>, name: &str, ev: KeyEvent| {
            phases();
            let t = Instant::now();
            rt.key(ev);
            rt.frame(DT);
            let took = ms(t);
            let ph = phases();
            let t2 = Instant::now();
            rt.frame(DT);
            let tail = ms(t2);
            let ph2 = phases();
            eprintln!("  {name:<23} {took:6.2} ms{ph}  | +1f: {tail:5.2} ms{ph2}");
            took
        };

        let ctrl_p = KeyEvent { key: Key::Command('p'), mods: Mods { cmd: true, ..Mods::default() } };
        let ctrl_shift_k = KeyEvent { key: Key::Command('k'), mods: Mods { cmd: true, shift: true, ..Mods::default() } };
        let ctrl_s = KeyEvent { key: Key::Command('s'), mods: Mods { cmd: true, ..Mods::default() } };
        let esc = KeyEvent { key: Key::Escape, mods: Mods::default() };

        // 4x Ctrl+P: Preview->Source, Source->Preview, Preview->Source, Source->Preview
        for i in 1..=4 {
            let want = if i % 2 == 1 { Mode::Source } else { Mode::Preview };
            let dir_lbl = if i % 2 == 1 { "Preview->Source" } else { "Source->Preview" };
            step_key(&mut rt, &format!("Ctrl+P #{i} ({dir_lbl})"), ctrl_p.clone());
            assert_eq!(query_state(&mut rt).0, want, "Ctrl+P #{i} switched mode");
            for _ in 0..6 {
                rt.frame(DT);
            }
        }

        // Ctrl+Shift+K in Preview, then Esc
        step_key(&mut rt, "Ctrl+Shift+K (Preview)", ctrl_shift_k.clone());
        assert!(query_state(&mut rt).1, "palette opened in Preview");
        step_key(&mut rt, "Esc palette (Preview)", esc.clone());
        assert!(!query_state(&mut rt).1, "palette closed in Preview");

        // Switch to Source, then Ctrl+Shift+K in Source, then Esc
        step_key(&mut rt, "Ctrl+P #5 (Preview->Src)", ctrl_p.clone());
        assert_eq!(query_state(&mut rt).0, Mode::Source);
        for _ in 0..6 {
            rt.frame(DT);
        }
        step_key(&mut rt, "Ctrl+Shift+K (Source)", ctrl_shift_k.clone());
        assert!(query_state(&mut rt).1, "palette opened in Source");
        step_key(&mut rt, "Esc palette (Source)", esc.clone());
        assert!(!query_state(&mut rt).1, "palette closed in Source");

        // Type "livecheck " in Source
        let len_before = query_state(&mut rt).2;
        let mut type_ms = Vec::new();
        let mut type_first = String::new();
        let mut type_mid = String::new();
        for (i, ch) in "livecheck ".chars().enumerate() {
            phases();
            let t = Instant::now();
            rt.key(KeyEvent { key: Key::Text(ch.to_string()), mods: Mods::default() });
            rt.frame(DT);
            let took = ms(t);
            let ph = phases();
            type_ms.push(took);
            if i == 0 {
                type_first = format!("{took:.2} ms{ph}");
            } else if i == 5 {
                type_mid = format!("{took:.2} ms{ph}");
            }
        }
        let len_after = query_state(&mut rt).2;
        assert_eq!(len_after - len_before, "livecheck ".len(), "every typed char landed in Source");
        type_ms.sort_by(f64::total_cmp);
        eprintln!(
            "  type \"livecheck \" (10): median {:5.2} ms, worst {:5.2} ms  (k0: {type_first}, k5: {type_mid})",
            type_ms[type_ms.len() / 2],
            type_ms[type_ms.len() - 1],
        );

        // Ctrl+S Save in Source
        step_key(&mut rt, "Ctrl+S Save (dirty)", ctrl_s.clone());
        assert!(!query_state(&mut rt).3, "Ctrl+S flushed dirty note");
        step_key(&mut rt, "Ctrl+S Save (clean)", ctrl_s.clone());

        let _ = std::fs::remove_dir_all(&dir);
    };

    run_case("1753-panels-closed", &text_1753, false, false);
    run_case("1753-panels-open", &text_1753, true, true);
    run_case("5470-panels-open", LONG, true, true);

    // Measure the idle-prewarmed path (`Tx::prepare_text(ui::prepare_cold)` in live windows)
    // to verify 100% TextSystem cache key parity for cold 1st Ctrl+P and cold 1st Ctrl+Shift+K.
    static WARM_PASS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn bench_view(s: &App, cx: &mut rune::Cx<App>) -> rune::El {
        if WARM_PASS.load(std::sync::atomic::Ordering::Relaxed) {
            crate::ui::prepare_cold(s, cx)
        } else {
            crate::ui::view(s, cx)
        }
    }
    let run_prepared = |label: &str, text: &str| {
        let dir = std::env::temp_dir().join(format!("quire-bench-prep-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Long.md"), text).unwrap();
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        WARM_PASS.store(false, std::sync::atomic::Ordering::Relaxed);
        let mut rt = Runtime::new(crate::app::init(), bench_view, (1920.0, 1080.0), Env { system_dark: false, scale: 1.0 });
        rt.on_key = Some(crate::keys::on_key);
        rt.poll = Some(|s: &mut App, t, tx| s.poll(t, tx));
        rt.frame(DT);
        rt.update(move |s, tx| {
            s.open_vault(root, tx);
            s.persist.left_open = false;
            s.persist.right_open = false;
            s.mode = Mode::Preview;
            s.open_file("Long.md", None, tx);
            s.keys_to_note(tx);
        });
        for _ in 0..4 {
            rt.frame(DT);
        }
        // Simulate idle turn running `ui::prepare_cold` into the window's TextSystem.
        phases();
        let tw = Instant::now();
        WARM_PASS.store(true, std::sync::atomic::Ordering::Relaxed);
        rt.update(|_, _| {});
        rt.frame(DT);
        let warm_ms = ms(tw);
        let warm_ph = phases();
        WARM_PASS.store(false, std::sync::atomic::Ordering::Relaxed);
        rt.update(|_, _| {});
        for _ in 0..3 {
            rt.frame(DT);
        }
        let ctrl_p = KeyEvent { key: Key::Command('p'), mods: Mods { cmd: true, ..Mods::default() } };
        let ctrl_shift_k = KeyEvent { key: Key::Command('k'), mods: Mods { cmd: true, shift: true, ..Mods::default() } };
        let esc = KeyEvent { key: Key::Escape, mods: Mods::default() };

        eprintln!("\n=== LIVEPACE-PREPARED ({label}: {} lines, {} bytes) ===", text.lines().count(), text.len());
        eprintln!("  idle prepare_cold:      {warm_ms:6.2} ms{warm_ph}");
        phases();
        let t = Instant::now();
        rt.key(ctrl_p);
        rt.frame(DT);
        eprintln!("  Ctrl+P #1 (warmed):     {:6.2} ms{}", ms(t), phases());
        for _ in 0..4 {
            rt.frame(DT);
        }
        phases();
        let t = Instant::now();
        rt.key(ctrl_shift_k);
        rt.frame(DT);
        eprintln!("  Ctrl+Shift+K (warmed):  {:6.2} ms{}", ms(t), phases());
        rt.key(esc);
        rt.frame(DT);
        let _ = std::fs::remove_dir_all(&dir);
    };
    run_prepared("1753-panels-closed", &text_1753);
    run_prepared("5470-panels-closed", LONG);
}
