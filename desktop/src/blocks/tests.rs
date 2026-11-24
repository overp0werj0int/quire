//! omarky's `test/tst_blocks.qml`, ported (VER-38), plus Quire's own syntax (BLK-101) and the
//! inline rules the parser owns (PAGE-80 to PAGE-82).

use super::*;

macro_rules! sample {
    ($name:literal) => {
        include_str!(concat!("../../tests/fixtures/omarky/", $name))
    };
}

fn types(d: &Doc) -> Vec<&'static str> {
    d.blocks.iter().map(|b| name(&b.kind)).collect()
}

fn name(k: &Kind) -> &'static str {
    match k {
        Kind::FrontMatter { .. } => "frontmatter",
        Kind::Heading { .. } => "heading",
        Kind::Paragraph { .. } => "paragraph",
        Kind::List(_) => "list",
        Kind::Quote { .. } => "quote",
        Kind::Alert { .. } => "alert",
        Kind::Code { .. } => "code",
        Kind::Diff { .. } => "diff",
        Kind::Table(_) => "table",
        Kind::Rule => "hr",
        Kind::Image { .. } => "image",
        Kind::Gallery { .. } => "gallery",
        Kind::Html { .. } => "html",
        Kind::Footnotes { .. } => "footnotes",
    }
}

fn runs(b: &Block) -> &[Run] {
    match &b.kind {
        Kind::Paragraph { runs } | Kind::Heading { runs, .. } => runs,
        k => panic!("no runs in {}", name(k)),
    }
}

fn text(b: &Block) -> String {
    plain(runs(b))
}

fn list(b: &Block) -> &List {
    match &b.kind {
        Kind::List(l) => l,
        k => panic!("not a list: {}", name(k)),
    }
}

fn children(b: &Block) -> &[Block] {
    match &b.kind {
        Kind::Quote { blocks } | Kind::Alert { blocks, .. } => blocks,
        k => panic!("no children in {}", name(k)),
    }
}

fn fields(d: &Doc) -> (&str, HashMap<&str, &Field>) {
    let Kind::FrontMatter { title, fields } = &d.blocks[0].kind else { panic!("no front matter") };
    (title, fields.iter().map(|f| (f.key.as_str(), f)).collect())
}

#[test]
fn paragraphs_and_line_map() {
    let d = build("One line.\n\nTwo\nlines.\n");
    assert_eq!(types(&d), ["paragraph", "paragraph"]);
    assert_eq!(d.blocks[0].lines, 0..1);
    assert_eq!(d.blocks[1].lines, 2..4);
    assert_eq!(text(&d.blocks[1]), "Two lines.");
    assert_eq!(d.line_count, 5);
}

#[test]
fn headings_levels_and_slugs() {
    let d = build("# Hello, World!\n\n## Hello, World!\n\n### `code` and _emph_\n\n#### Café Ünïcode — dash\n\n##### Hello, World!\n");
    assert_eq!(types(&d), ["heading"; 5]);
    let lv: Vec<usize> = d.headings.iter().map(|h| h.level).collect();
    assert_eq!(lv, [1, 2, 3, 4, 5]);
    let slugs: Vec<&str> = d.headings.iter().map(|h| h.slug.as_str()).collect();
    assert_eq!(slugs, ["hello-world", "hello-world-1", "code-and-emph", "café-ünïcode--dash", "hello-world-2"]);
    assert_eq!(d.headings[2].text, "code and emph");
}

#[test]
fn outline_lists_headings_with_block_indexes() {
    let d = build("# A\n\ntext\n\n## B\n\n### C\n");
    assert_eq!(d.headings.len(), 3);
    assert_eq!((d.headings[1].text.as_str(), d.headings[1].level, d.headings[1].index), ("B", 2, 2));
    assert_eq!(d.headings[2].slug, "c");
}

#[test]
fn front_matter_card() {
    let d = build(sample!("frontmatter.md"));
    assert_eq!(types(&d)[0], "frontmatter");
    assert_eq!(d.blocks[0].lines, 0..18);
    let (title, by) = fields(&d);
    assert_eq!(title, "Render each Markdown block as its own native component");
    assert!(!by.contains_key("title"), "title is lifted out of the rows");
    assert_eq!((by["status"].kind, by["status"].value.as_str()), (FieldKind::Pill, "accepted"));
    assert_eq!(by["date"].kind, FieldKind::Pill);
    assert_eq!(by["number"].value, "0001");
    assert_eq!(by["deciders"].kind, FieldKind::List);
    assert_eq!(by["deciders"].items, ["OmarkyReader maintainers"]);
    assert_eq!(by["tags"].items, ["rendering", "performance", "typography"]);
    assert_eq!(by["summary"].kind, FieldKind::Text);
    assert!(by["summary"].value.starts_with("The document is a flat list"));
    assert!(!by["summary"].value.contains('\n'), "folded scalar joins lines");
    assert_eq!(by["links"].kind, FieldKind::Link);
    assert_eq!(by["draft"].value, "false");
    assert_eq!(types(&d)[1], "heading");
    assert_eq!(d.blocks[1].lines.start, 19, "the heading after the front matter keeps its line");
}

#[test]
fn bare_front_matter_card() {
    let d = build(sample!("bare-frontmatter.md"));
    assert_eq!(types(&d)[0], "frontmatter");
    assert_eq!(d.blocks[0].lines, 0..9, "the block runs through the blank line that ends it");
    let (title, by) = fields(&d);
    assert_eq!(title, "Bare front matter");
    assert!(!by.contains_key("title"));
    assert_eq!(by["id"].kind, FieldKind::Text, "a UUID is too long for a pill");
    assert_eq!(by["id"].value, "3f2a9c1e-7b4d-4e8a-9c01-5d6e7f8a9b0c");
    assert_eq!((by["published_at"].kind, by["published_at"].value.as_str()), (FieldKind::Pill, "2026-10-04 09:30"));
    assert_eq!(by["teaser"].kind, FieldKind::Text);
    assert_eq!(by["author"].value, "OmarkyReader maintainers");
    assert_eq!((by["thread"].kind, by["thread"].value.as_str()), (FieldKind::Empty, ""));
    assert_eq!(types(&d)[1], "heading");
    assert_eq!(d.blocks[1].lines.start, 9);
}

#[test]
fn front_matter_edges() {
    let d = build("title: Only metadata\nstatus: draft\n");
    assert_eq!(types(&d), ["frontmatter"]);
    assert_eq!(d.blocks[0].lines.end, 3);
    assert_eq!(types(&build("Note: remember this\n\n# Heading\n")), ["paragraph", "heading"], "one key line is prose");
    assert_eq!(types(&build("id: 1\ntitle: x\nThen a sentence of prose.\n\n# Heading\n")), ["paragraph", "heading"]);
    assert_eq!(types(&build("Dear reader: hello\nOne more: line\n\nBody.\n")), ["paragraph", "paragraph"], "keys with spaces");
    assert!(!types(&build("Intro\n\n---\ntitle: nope\n---\n")).contains(&"frontmatter"), "only at the top");
    let d = build("---\ntitle: x\n\nNo closing fence.\n");
    assert!(!types(&d).contains(&"frontmatter") && !d.blocks.is_empty(), "unclosed is not front matter");
}

#[test]
fn html_comments_hidden_other_html_literal() {
    let d = build("<!-- SNAPSHOT abc -->\n\nText\n\n<div class=\"x\">hi</div>\n\n<!-- a\nmultiline comment -->\n");
    assert_eq!(types(&d), ["paragraph", "html"]);
    assert!(matches!(&d.blocks[1].kind, Kind::Html { text } if text == "<div class=\"x\">hi</div>"));
}

fn gallery(b: &Block) -> &[GalleryItem] {
    match &b.kind {
        Kind::Gallery { items } => items,
        k => panic!("not a gallery: {}", name(k)),
    }
}

#[test]
fn html_images_become_a_gallery() {
    let d = build("<p><img src=\"a.png\" style=\"height:260px;width:auto;display:inline-block\" alt=\"First\"> <img src='b b.jpg' alt=\"Second &amp; last\" title=\"T\"></p>\n");
    assert_eq!(types(&d), ["gallery"]);
    let g = gallery(&d.blocks[0]);
    assert_eq!(g.len(), 2);
    assert_eq!((g[0].img.src.as_str(), g[0].img.alt.as_str(), g[0].h, g[0].w), ("a.png", "First", 260.0, 0.0));
    assert_eq!((g[1].img.src.as_str(), g[1].img.alt.as_str(), g[1].title.as_str(), g[1].h), ("b b.jpg", "Second & last", "T", 0.0));
}

#[test]
fn html_image_wrappers_and_sizes() {
    let d = build("<p align=\"center\">\n  <a href=\"x\"><img src=\"one.png\" width=\"300\"></a><br>\n  <img src=\"two.png\" height=\"120px\" />\n</p>\n\n<div><img src=images/three.png/></div>\n");
    assert_eq!(types(&d), ["gallery", "gallery"]);
    let g = gallery(&d.blocks[0]);
    assert_eq!((g.len(), g[0].w, g[1].h), (2, 300.0, 120.0));
    assert_eq!(gallery(&d.blocks[1])[0].img.src, "images/three.png");
}

#[test]
fn html_with_anything_but_images_stays_literal() {
    let d = build("<p><img src=\"a.png\"> and some words</p>\n\n<p><img alt=\"no src\"></p>\n\n<p><script src=\"x.js\"></script></p>\n");
    assert_eq!(types(&d), ["html", "html", "html"]);
}

#[test]
fn unclosed_comment_stays_visible() {
    let d = build("<!-- never closed\n\n# Still here\n");
    assert_eq!(types(&d), ["html"]);
    assert!(matches!(&d.blocks[0].kind, Kind::Html { text } if text.contains("Still here")));
}

#[test]
fn lists_tight_loose_ordered_nested() {
    let d = build("- a\n- b\n  - b1\n    - b2\n- c\n\n8. x\n9. y\n\n- p\n\n- q\n");
    assert_eq!(types(&d), ["list", "list", "list"]);
    let ul = list(&d.blocks[0]);
    assert_eq!((ul.ordered, ul.tight, ul.items.len(), ul.depth), (false, true, 3, 0));
    let nested = list(&ul.items[1].blocks[1]);
    assert_eq!(nested.depth, 1);
    assert_eq!(list(&nested.items[0].blocks[1]).depth, 2);
    assert_eq!(text(&ul.items[1].blocks[0]), "b");
    let ol = list(&d.blocks[1]);
    assert_eq!((ol.ordered, ol.start, ol.items[1].number), (true, 8, 9));
    assert!(!list(&d.blocks[2]).tight);
}

#[test]
fn task_items() {
    let d = build("- [x] done\n- [ ] todo\n- [X] Done too\n- not a task\n- [y] nor this\n");
    let l = list(&d.blocks[0]);
    let t: Vec<Task> = l.items.iter().map(|i| i.task).collect();
    assert_eq!(t, [Task::Done, Task::Todo, Task::Done, Task::None, Task::None]);
    assert_eq!(text(&l.items[0].blocks[0]), "done");
    assert_eq!(text(&l.items[4].blocks[0]), "[y] nor this");
    assert!(l.tasks);
}

#[test]
fn task_lines() {
    let d = build("---\ntitle: T\n---\n\n- [ ] one\n- [x] two\n  - [ ] nested\n\n> 1. [ ] quoted\n");
    let lists: Vec<&List> = d.blocks.iter().filter_map(|b| if let Kind::List(l) = &b.kind { Some(l) } else { None }).collect();
    let lines: Vec<usize> = lists[0].items.iter().map(|i| i.line).collect();
    assert_eq!(lines, [4, 5]);
    assert_eq!(list(&lists[0].items[1].blocks[1]).items[0].line, 6);
    let q = d.blocks.iter().find(|b| matches!(b.kind, Kind::Quote { .. })).unwrap();
    assert_eq!(list(&children(q)[0]).items[0].line, 8);
}

#[test]
fn toggle_task_changes_one_character() {
    let src = "- [ ] one\n  * [X] nested\n> 2) [ ] quoted\n- [ ]\n- plain [ ] not\n10. [x] ten\r\n";
    let line = |s: Option<String>, n: usize| s.unwrap().split('\n').nth(n).unwrap().to_string();
    assert_eq!(line(toggle_task(src, 0, true), 0), "- [x] one");
    assert_eq!(line(toggle_task(src, 1, false), 1), "  * [ ] nested");
    assert_eq!(line(toggle_task(src, 2, true), 2), "> 2) [x] quoted");
    assert_eq!(line(toggle_task(src, 3, true), 3), "- [x]");
    assert_eq!(toggle_task(src, 4, true), None);
    assert_eq!(line(toggle_task(src, 5, false), 5), "10. [ ] ten\r");
    assert_eq!(toggle_task(src, 99, true), None);
    let t = toggle_task(src, 0, true).unwrap();
    assert_eq!((t.len(), &t[6..]), (src.len(), &src[6..]));
}

#[test]
fn blockquote_nesting() {
    let d = build("> outer\n>\n> > inner\n");
    assert_eq!(types(&d), ["quote"]);
    let q = children(&d.blocks[0]);
    assert_eq!((name(&q[0].kind), name(&q[1].kind)), ("paragraph", "quote"));
    assert_eq!(text(&children(&q[1])[0]), "inner");
}

#[test]
fn alerts() {
    let d = build(sample!("alerts.md"));
    let alerts: Vec<(&Alert, &Vec<Block>)> = d.blocks.iter().filter_map(|b| if let Kind::Alert { kind, blocks } = &b.kind { Some((kind, blocks)) } else { None }).collect();
    let kinds: Vec<Alert> = alerts.iter().map(|a| *a.0).collect();
    assert_eq!(kinds, [Alert::Note, Alert::Tip, Alert::Important, Alert::Warning, Alert::Caution]);
    assert_eq!(alerts[0].0.title(), "Note");
    assert_eq!(text(&alerts[0].1[0]), "Useful information that users should know, even when skimming content.");
    assert_eq!(name(&alerts[2].1[1].kind), "list");
    assert_eq!(name(&alerts[4].1[1].kind), "code");
    let quotes: Vec<&Block> = d.blocks.iter().filter(|b| matches!(b.kind, Kind::Quote { .. })).collect();
    assert_eq!(quotes.len(), 3);
    assert!(text(&children(quotes[0])[0]).starts_with("[!UNKNOWN]"));
}

#[test]
fn code_blocks() {
    let d = build("```python\ndef f():\n    pass\n```\n\n```\nplain\n```\n\n    indented\n\n```mermaid\ngraph LR\n```\n");
    assert_eq!(types(&d), ["code"; 4]);
    let code: Vec<(&str, &str)> = d.blocks.iter().map(|b| if let Kind::Code { lang, text } = &b.kind { (lang.as_str(), text.as_str()) } else { unreachable!() }).collect();
    assert_eq!(code[0], ("python", "def f():\n    pass"));
    assert_eq!(code[1].0, "");
    assert_eq!(code[2].1, "indented");
    assert_eq!(code[3].0, "mermaid");
}

#[test]
fn diff_blocks() {
    let d = build("```diff\ndiff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n same\n-old\n+new\n+newer\n```\n");
    let Kind::Diff { lines, added, removed, .. } = &d.blocks[0].kind else { panic!("not a diff") };
    assert_eq!((*added, *removed), (2, 1));
    let kinds: Vec<DiffKind> = lines.iter().map(|l| l.kind).collect();
    use DiffKind::*;
    assert_eq!(kinds, [Meta, Meta, Meta, Hunk, Ctx, Del, Add, Add]);
    assert_eq!((lines[5].text.as_str(), lines[4].text.as_str()), ("old", "same"));
}

#[test]
fn tables() {
    let d = build("| L | C | R | N |\n|:--|:-:|--:|---|\n| a | **b** | 1.5 | 10 |\n| c | d | 22 | 3 |\n| only |\n");
    let Kind::Table(t) = &d.blocks[0].kind else { panic!("not a table") };
    assert_eq!(t.aligns, [Align::Left, Align::Center, Align::Right, Align::None]);
    let head: Vec<String> = t.header.iter().map(|c| plain(c)).collect();
    assert_eq!(head, ["L", "C", "R", "N"]);
    assert_eq!(t.rows.len(), 3);
    assert_eq!(t.rows[2].len(), 4, "ragged rows are padded");
    assert_eq!(plain(&t.rows[0][1]), "b");
    assert_eq!(t.numeric, [false, false, true, true]);
}

#[test]
fn numbers_in_tables() {
    for n in ["1", "-2.5", "$1,200", "€3k", "45%", "12:30", "1:02.5", "3x", "120ms", "5s", "2h", "−7"] {
        assert!(is_numeric(n), "{n}");
    }
    for n in ["a1", "1.", "1.2.3", "12:", "1 2", "$", "1kb"] {
        assert!(!is_numeric(n), "{n}");
    }
}

#[test]
fn rule_and_images() {
    let d = build("---\n\n![Alt text](pic.png \"Title\")\n\nText with ![inline](i.png) image.\n");
    assert_eq!(types(&d), ["hr", "image", "paragraph"]);
    let Kind::Image { img, title } = &d.blocks[1].kind else { unreachable!() };
    assert_eq!((img.src.as_str(), img.alt.as_str(), title.as_str()), ("pic.png", "Alt text", "Title"));
}

#[test]
fn footnotes() {
    let d = build("First[^b] and second[^a], again[^b].\n\n[^a]: Note A.\n[^b]: Note B\n    continues.\n\n[^unused]: Never cited.\n");
    assert_eq!(types(&d), ["paragraph", "footnotes"]);
    let cites: Vec<&Run> = runs(&d.blocks[0]).iter().filter(|r| r.note).collect();
    let nums: Vec<&str> = cites.iter().map(|r| r.t.as_str()).collect();
    assert_eq!(nums, ["1", "2", "1"]);
    assert_eq!(cites[0].link, Some(Link::Anchor("fn-1".into())));
    assert_eq!(cites[2].anchor.as_deref(), Some("fnref-1-2"));
    let Kind::Footnotes { items } = &d.blocks[1].kind else { unreachable!() };
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["b", "a", "unused"]);
    assert_eq!((items[0].number, items[0].refs), (1, 2));
    assert_eq!(text(&items[0].blocks[0]), "Note B continues.");
    assert_eq!(items[2].refs, 0);
}

#[test]
fn anchor_map() {
    let d = build("# Top\n\nCited[^n].\n\n- in a list[^m]\n\n## Top\n\n[^n]: Note.\n[^m]: Other.\n");
    let a = |k: &str| d.anchors.get(k).copied();
    assert_eq!((a("top"), a("top-1")), (Some(0), Some(3)));
    assert_eq!((a("fnref-1-1"), a("fnref-2-1")), (Some(1), Some(2)));
    assert_eq!((a("fn-1"), a("fn-2")), (Some(4), Some(4)));
}

#[test]
fn find_index_stamps_piece_ids_in_document_order() {
    let d = build("# Beta\n\nalpha beta\n\n- beta one\n- two\n\n| h | beta |\n|---|---|\n| beta | x |\n\n```\nbeta = 1\nbeta\n```\n");
    let texts: Vec<&str> = d.pieces.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["Beta", "alpha beta", "beta one", "two", "h", "beta", "beta", "x", "beta = 1\nbeta"]);
    let blocks: Vec<usize> = d.pieces.iter().map(|p| p.block).collect();
    assert_eq!(blocks, [0, 1, 2, 2, 3, 3, 3, 3, 4]);
    assert_eq!(d.blocks[0].pid, Some(0));
    assert_eq!(list(&d.blocks[2]).items[1].blocks[0].pid, Some(3));
    let Kind::Table(t) = &d.blocks[3].kind else { unreachable!() };
    assert_eq!((t.header_pids.clone(), t.row_pids.clone()), (vec![4, 5], vec![vec![6, 7]]));
    assert_eq!(d.blocks[4].pid, Some(8));
    let m = find_all(&d, "BETA");
    assert_eq!(m.len(), 7);
    assert_eq!(m[0].pid, 0);
    assert_eq!(m[5], Match { pid: 8, block: 4, k: 0 });
    assert_eq!(m[6], Match { pid: 8, block: 4, k: 1 });
    assert!(find_all(&d, "").is_empty());
}

const SAMPLES: [(&str, &str); 10] = [
    ("kitchen-sink.md", sample!("kitchen-sink.md")),
    ("prose.md", sample!("prose.md")),
    ("code.md", sample!("code.md")),
    ("tables.md", sample!("tables.md")),
    ("alerts.md", sample!("alerts.md")),
    ("frontmatter.md", sample!("frontmatter.md")),
    ("bare-frontmatter.md", sample!("bare-frontmatter.md")),
    ("malformed.md", sample!("malformed.md")),
    ("empty.md", sample!("empty.md")),
    ("real/reader-spec.md", sample!("real/reader-spec.md")),
];

#[test]
fn every_sample_builds() {
    for (n, src) in SAMPLES {
        let d = build(src);
        if n != "empty.md" {
            assert!(!d.blocks.is_empty(), "{n} has blocks");
        }
        for (j, b) in d.blocks.iter().enumerate() {
            assert!(b.lines.end >= b.lines.start, "{n} block {j} line map");
        }
    }
}

#[test]
fn kitchen_sink_has_every_block_type() {
    let d = build(sample!("kitchen-sink.md"));
    let seen = types(&d);
    for want in ["frontmatter", "heading", "paragraph", "list", "quote", "alert", "code", "diff", "table", "hr", "image", "gallery", "html", "footnotes"] {
        assert!(seen.contains(&want), "kitchen sink has {want}");
    }
}

#[test]
fn malformed_input() {
    let d = build(sample!("malformed.md"));
    assert!(d.blocks.len() > 10);
    assert_eq!(name(&d.blocks.last().unwrap().kind), "code", "an unclosed fence runs to the end");
    assert!(build("").blocks.is_empty());
    assert_eq!(build("\u{0}\u{1}\u{fffd} binary").blocks.len(), 1);
}

#[test]
fn long_document_is_fast() {
    let src = sample!("long.md");
    let t0 = std::time::Instant::now();
    let d = build(src);
    let ms = t0.elapsed().as_millis();
    assert!(d.line_count > 5000);
    assert!(d.blocks.len() > 1000, "blocks: {}", d.blocks.len());
    // omarky's bound is 900 ms; an unoptimised test build is slower than a release one.
    assert!(ms < 900, "long.md built in {ms} ms");
}

#[test]
fn keys_survive_edits_elsewhere() {
    let a = build("# A\n\nsame\n\nx\n\nsame\n");
    let b = build("# A\n\nnew first\n\nsame\n\nx\n\nsame\n");
    let ka: Vec<(u64, u32)> = a.blocks.iter().map(|b| b.key).collect();
    let kb: Vec<(u64, u32)> = b.blocks.iter().map(|b| b.key).collect();
    assert_eq!(ka[0], kb[0]);
    assert_eq!(ka[1], kb[2], "the first \"same\" keeps its key");
    assert_eq!(ka[3], kb[4], "the second keeps its ordinal");
    assert_ne!(ka[1], ka[3]);
}

// ---------- Quire's syntax and the inline rules ----------

#[test]
fn blk_101_wiki_links_tags_and_embeds() {
    let d = build("See [[Ideas]] and [[Reading list|the list]] with #tag and a#b.\n\n![[Photos/cat.png|300]]\n\n![[Note]]\n");
    let r = runs(&d.blocks[0]);
    let wiki: Vec<(&str, &Link)> = r.iter().filter_map(|r| r.link.as_ref().map(|l| (r.t.as_str(), l))).collect();
    assert_eq!(wiki[0], ("Ideas", &Link::Wiki("Ideas".into())));
    assert_eq!(wiki[1], ("the list", &Link::Wiki("Reading list".into())));
    assert_eq!(wiki[2], ("#tag", &Link::Tag("tag".into())));
    assert_eq!(wiki.len(), 3, "a#b is not a tag");
    let Kind::Image { img, .. } = &d.blocks[1].kind else { panic!("{:?}", d.blocks[1].kind) };
    assert_eq!((img.src.as_str(), img.alt.as_str(), img.embed), ("Photos/cat.png", "cat", true));
    assert_eq!(name(&d.blocks[2].kind), "paragraph", "an embed of a note is its link");
}

#[test]
fn blk_101_a_line_of_images_is_a_gallery() {
    let d = build("![a](a.png) ![b](b%20c.png)\n");
    let g = gallery(&d.blocks[0]);
    assert_eq!((g[0].img.src.as_str(), g[1].img.src.as_str()), ("a.png", "b c.png"));
}

#[test]
fn page_80_br_and_comments() {
    let d = build("a<br>b<BR/>c <!-- gone --> d <span>x</span>\n");
    let r = runs(&d.blocks[0]);
    assert_eq!(r.iter().filter(|r| r.br).count(), 2);
    assert_eq!(plain(r), "a\nb\nc  d <span>x</span>");
    assert!(r.iter().any(|r| r.raw && r.t == "<span>"));
}

#[test]
fn page_81_typographer() {
    let d = build("\"Quotes\" and 'single' -- en --- em... (c) (TM) +- wait.. really?.... no!!!!! a,,b\n");
    assert_eq!(text(&d.blocks[0]), "“Quotes” and ‘single’ – en — em… © ™ ± wait… really?.. no!!! a,b");
    let c = build("`(c) -- \"x\"`\n");
    assert_eq!(text(&c.blocks[0]), "(c) -- \"x\"", "code is untouched");
}

#[test]
fn page_82_autolinks() {
    let urls = |s: &str| autolinks(s).into_iter().map(|(r, u)| (s[r].to_string(), u)).collect::<Vec<_>>();
    assert_eq!(urls("see https://a.com/x."), [("https://a.com/x".into(), "https://a.com/x".into())]);
    assert_eq!(urls("(www.a.com/b_(c))"), [("www.a.com/b_(c)".into(), "http://www.a.com/b_(c)".into())]);
    assert_eq!(urls("mail me@x.org!"), [("me@x.org".into(), "mailto:me@x.org".into())]);
    assert!(urls("open file.md and foowww.a.com or xhttps://a.b").is_empty(), "bare domains and glued matches stay text");
    assert_eq!(urls("ftp://h/f, then"), [("ftp://h/f".into(), "ftp://h/f".into())]);
    let d = build("Go to https://example.com now and [a link](https://b.com).\n");
    let links: Vec<Option<&Link>> = runs(&d.blocks[0]).iter().map(|r| r.link.as_ref()).collect();
    assert_eq!(links.iter().flatten().count(), 2);
}

#[test]
fn inline_highlight_marks_parse_and_respect_escapes_and_code() {
    let d = build("Some ==highlighted text== and ==**bold** mark== here.\n");
    let r = runs(&d.blocks[0]);
    assert_eq!(plain(r), "Some highlighted text and bold mark here.");
    assert!(r.iter().any(|x| x.t == "highlighted text" && x.mark));
    assert!(r.iter().any(|x| x.t == "bold" && x.b && x.mark));
    assert!(r.iter().any(|x| x.t == "Some " && !x.mark));

    let esc = build("Escaped \\==literal== and `==in code==` and a == b == c.\n");
    let re = runs(&esc.blocks[0]);
    assert!(!re.iter().any(|x| x.mark), "escaped, code, and spaced == must not highlight: {re:?}");
    assert_eq!(plain(re), "Escaped ==literal== and ==in code== and a == b == c.");
}
