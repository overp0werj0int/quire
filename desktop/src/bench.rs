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
