//! The edit sheet (`docs/reader-writer/behavior/edit-sheet.md`, phase 8): `e` and `E` in the
//! reader open one block, or its section, as source lines in the writer's editor over the
//! page, and save them back through Quire's own save path. The view is `ui/sheet.rs`.

use std::ops::Range;
use std::rc::Rc;

use rune::{TextEdit, Tx};

use crate::app::{App, Mode};
use crate::layer::Layer;
use crate::blocks::Doc;

/// The editor's element key.
pub const EDITOR: &str = "sheet-editor";
/// A second Esc within this closes a changed sheet (SHEET-038).
pub const DISCARD: f64 = 2.0;
pub const CHANGED: &str = "The file changed while you were editing. Copy your text and open it again.";
pub const GONE: &str = "The note was deleted or moved. Copy your text.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scope {
    Block,
    Section,
}

/// What a sheet edits: source lines, and the blocks shown faint on either side, by index.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub lines: Range<usize>,
    pub before: Option<usize>,
    pub after: Option<usize>,
}

/// The range for block `i` (SHEET-007 to SHEET-011).
#[cfg(test)]
pub fn pick(doc: &Doc, i: usize, scope: Scope) -> Option<Pick> {
    pick_range(doc, i, i, scope)
}

/// The range for blocks `first..=last` (SHEET-007 to SHEET-011). When a text selection spans
/// multiple blocks, `Scope::Block` edits every block from `first` through `last` (with the
/// neighbours outside that span as context), and `Scope::Section` covers from `first`'s section
/// through the end of `last`'s section.
pub fn pick_range(doc: &Doc, first: usize, last: usize, scope: Scope) -> Option<Pick> {
    let (first, last) = (first.min(last), first.max(last));
    let a = doc.blocks.get(first)?;
    let z = doc.blocks.get(last)?;
    if scope == Scope::Block {
        return Some(Pick {
            lines: a.lines.start..z.lines.end,
            before: first.checked_sub(1),
            after: (last + 1 < doc.blocks.len()).then_some(last + 1),
        });
    }
    let section_of = |i: usize| -> Option<Pick> {
        let b = doc.blocks.get(i)?;
        let end_all = doc.blocks.last().map_or(0, |b| b.lines.end);
        let hs = &doc.headings;
        // The nearest heading at or above the block's first line.
        let Some(h) = hs.iter().rposition(|h| h.line <= b.lines.start) else {
            // Before the first heading, or no headings at all (SHEET-010).
            return Some(match hs.first() {
                Some(head) => Pick { lines: 0..head.line, before: None, after: Some(head.index) },
                None => Pick { lines: 0..end_all, before: None, after: None },
            });
        };
        let level = hs[h].level;
        let next = hs[h + 1..].iter().find(|n| n.level <= level);
        Some(Pick {
            lines: hs[h].line..next.map_or(end_all, |n| n.line),
            before: h.checked_sub(1).map(|p| hs[p].index),
            after: next.map(|n| n.index),
        })
    };
    let mut p = section_of(first)?;
    if last > first {
        let tail = section_of(last)?;
        if tail.lines.end > p.lines.end {
            p.lines.end = tail.lines.end;
            p.after = tail.after;
        }
    }
    Some(p)
}

/// The bytes of `text` that lines `lines` occupy, without the newline after the last.
pub fn line_bytes(text: &str, lines: &Range<usize>) -> Range<usize> {
    // Line k ends at the k-th newline (from 0), or at the end of the text.
    let end = |k: usize| text.match_indices('\n').nth(k).map_or(text.len(), |(i, _)| i);
    let a = if lines.start == 0 { 0 } else { (end(lines.start - 1) + 1).min(text.len()) };
    let b = if lines.end == 0 { a } else { end(lines.end - 1) };
    a..b.max(a)
}

/// `text` with lines `lines` replaced by `with` (SHEET-045). Lines outside the range, and the
/// final newline, stay as they are.
#[cfg(test)]
pub fn splice(text: &str, lines: &Range<usize>, with: &str) -> String {
    let r = line_bytes(text, lines);
    format!("{}{with}{}", &text[..r.start], &text[r.end..])
}

/// An open sheet.
pub struct Sheet {
    /// The note it edits.
    pub rel: String,
    pub scope: Scope,
    pub pick: Pick,
    /// The note's blocks when the sheet opened, which the context boxes draw from, so a change
    /// underneath does not swap the neighbours shown (SHEET-008).
    pub doc: Rc<Doc>,
    /// The note's text when the sheet opened; a save checks it is still the same (SHEET-044).
    pub base: String,
    /// What the editor opened with, so Esc knows whether anything changed (SHEET-037).
    pub orig: String,
    pub edit: TextEdit,
    /// Why the last save was refused, shown in place of the key row (SHEET-047).
    pub problem: Option<&'static str>,
    /// When Esc armed a discard, paired with the text snapshot at arm time (SHEET-038).
    pub armed: Option<(f64, String)>,
}

impl App {
    /// `e` and `E` (SHEET-001 to SHEET-003, SHEET-013).
    pub fn open_sheet(&mut self, scope: Scope, tx: &mut Tx) {
        self.open_sheet_at(None, scope, tx);
    }

    /// Opens the edit sheet on the block at `at` (from a right-click menu), else the selected
    /// block(s), else the hovered or top visible block.
    pub fn open_sheet_at(&mut self, at: Option<usize>, scope: Scope, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        if self.mode != Mode::Preview || self.note_locked(&rel) {
            return;
        }
        let doc = self.doc();
        let Some((first, last)) = at.and_then(|l| doc.blocks.iter().position(|b| b.lines.start == l).map(|i| (i, i))).or_else(|| self.sheet_blocks(&doc, tx)) else {
            return;
        };
        let Some(pick) = pick_range(&doc, first, last, scope) else { return };
        self.show_sheet(pick, scope, tx);
    }

    /// The sheet on source lines `lines` of the note shown, from the map or lightbox (MAP-020).
    pub fn open_sheet_lines(&mut self, lines: Range<usize>, scope: Scope, tx: &mut Tx) {
        if !matches!(self.mode, Mode::Map | Mode::Preview | Mode::Board) || self.active_locked() {
            return;
        }
        let pick = if self.mode == Mode::Preview {
            let doc = self.doc();
            doc.blocks
                .iter()
                .position(|b| b.lines.start == lines.start)
                .and_then(|i| pick_range(&doc, i, i, scope))
                .unwrap_or(Pick { lines, before: None, after: None })
        } else {
            Pick { lines, before: None, after: None }
        };
        self.show_sheet(pick, scope, tx);
    }

    fn show_sheet(&mut self, pick: Pick, scope: Scope, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        let doc = self.doc();
        let base = self.active_text().to_string();
        let orig = base[line_bytes(&base, &pick.lines)].to_string();
        let mut edit = TextEdit::new(orig.clone());
        edit.set_selection(0, 0);
        self.reader.pending = None;
        self.open_layer(Layer::Sheet(Box::new(Sheet { rel, scope, pick, doc, base, orig, edit, problem: None, armed: None })), tx);
        tx.clear_text_selection();
        tx.focus(EDITOR, false);
    }

    /// The block(s) holding the active text selection, else the block under the pointer, else the
    /// top one (SHEET-002).
    fn sheet_blocks(&self, doc: &Doc, tx: &Tx) -> Option<(usize, usize)> {
        if let Some(span) = self.selected_block_span(doc, tx) {
            return Some(span);
        }
        if let Some(l) = self.reader.hover
            && let Some(i) = doc.blocks.iter().position(|b| b.lines.start == l)
        {
            return Some((i, i));
        }
        Some(self.top()).filter(|&i| i < doc.blocks.len()).map(|i| (i, i))
    }

    /// Ctrl+Enter and Ctrl+S (SHEET-033, SHEET-044 to SHEET-048).
    pub fn sheet_save(&mut self, tx: &mut Tx) {
        let Some(rel) = self.sheet().map(|sh| sh.rel.clone()) else { return };
        let gone = !self.files().contains_key(&rel) || !self.tabs.contains(&rel);
        let now = self.note_text(&rel).map(str::to_string);
        let Some(sh) = self.sheet_mut() else { return };
        if gone {
            sh.problem = Some(GONE);
            return;
        }
        if now.as_deref() != Some(sh.base.as_str()) {
            sh.problem = Some(CHANGED);
            return;
        }
        let (lines, r) = (sh.pick.lines.clone(), line_bytes(&sh.base, &sh.pick.lines));
        let with = sh.edit.text.clone();
        self.close_layer(tx);
        // One step in the note's source history (edit-sheet.md, open questions).
        if matches!(self.mode, Mode::Map | Mode::Board) && self.active.as_deref() == Some(rel.as_str()) {
            self.map_replace(lines, &with, tx);
        } else {
            self.splice_note(&rel, r, &with);
        }
        // Written at once, so "Saved" is true by the time anyone looks (open questions).
        self.flush_notes();
        self.toast("Saved", tx);
        if matches!(self.mode, Mode::Map | Mode::Board) {
            self.keys_to_note(tx);
        } else {
            tx.blur();
        }
    }

    /// Esc (SHEET-037 to SHEET-039): close an unchanged sheet, else arm a discard, else
    /// discard.
    pub fn sheet_escape(&mut self, tx: &mut Tx) {
        let now = self.now.get();
        let Some(sh) = self.sheet_mut() else { return };
        let armed = sh.armed.as_ref().is_some_and(|(at, txt)| now - *at < DISCARD && *txt == sh.edit.text);
        if sh.edit.text == sh.orig || armed {
            self.close_layer(tx);
            tx.blur();
        } else {
            sh.armed = Some((now, sh.edit.text.clone()));
            tx.wake_after(std::time::Duration::from_secs_f64(DISCARD + 0.05));
        }
    }

    /// Copies the sheet's current text to the clipboard (used from the conflict/deleted error row).
    pub fn sheet_copy(&mut self, tx: &mut Tx) {
        let Some(txt) = self.sheet().map(|sh| sh.edit.text.clone()) else { return };
        tx.copy(txt);
        self.toast("Copied", tx);
    }

    /// The armed discard lapses (SHEET-038). True when it did.
    pub(crate) fn expire_sheet(&mut self, t: f64) -> bool {
        match self.sheet_mut() {
            Some(sh) if sh.armed.as_ref().is_some_and(|(at, _)| t - *at >= DISCARD) => {
                sh.armed = None;
                true
            }
            _ => false,
        }
    }

    /// The editor the writer's editing keys act on: the sheet's while it is open.
    pub fn sheet_edit(&mut self) -> Option<&mut TextEdit> {
        self.sheet_mut().map(|s| &mut s.edit)
    }

    /// The sheet's field, for its edit hook, which can still run in the frame the sheet
    /// closes; that last edit lands in a scratch field.
    pub fn sheet_field(&mut self) -> &mut TextEdit {
        if self.sheet().is_some() {
            return self.sheet_edit().expect("checked above");
        }
        self.field("sheet-closed", String::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "intro\n\n# One\n\npara a\n\n## Two\n\npara b\n\n# Three\n\npara c\n";

    fn at(doc: &Doc, line: usize) -> usize {
        doc.blocks.iter().position(|b| b.lines.start == line).unwrap()
    }

    #[test]
    fn sheet_007_008_a_block_and_its_neighbours() {
        let doc = crate::blocks::build(NOTE);
        let i = at(&doc, 4);
        let p = pick(&doc, i, Scope::Block).unwrap();
        assert_eq!(p.lines, 4..5);
        assert_eq!((p.before, p.after), (Some(i - 1), Some(i + 1)));
        let first = pick(&doc, 0, Scope::Block).unwrap();
        assert_eq!(first.before, None, "the first block has nothing before it");
        let last = pick(&doc, doc.blocks.len() - 1, Scope::Block).unwrap();
        assert_eq!(last.after, None);
    }

    #[test]
    fn sheet_009_011_a_section_runs_to_the_next_heading_of_its_level_or_higher() {
        let doc = crate::blocks::build(NOTE);
        // "para a" sits under "# One", which runs past "## Two" to "# Three".
        let p = pick(&doc, at(&doc, 4), Scope::Section).unwrap();
        assert_eq!(p.lines, 2..10);
        assert_eq!(p.before, None, "no heading before # One");
        assert_eq!(p.after, Some(at(&doc, 10)));
        // "para b" sits under "## Two", which ends at "# Three"; before it is "# One".
        let p = pick(&doc, at(&doc, 8), Scope::Section).unwrap();
        assert_eq!(p.lines, 6..10);
        assert_eq!((p.before, p.after), (Some(at(&doc, 2)), Some(at(&doc, 10))));
        // The last section runs to the end line of the last block.
        let p = pick(&doc, at(&doc, 12), Scope::Section).unwrap();
        assert_eq!(p.lines, 10..13);
        assert_eq!((p.before, p.after), (Some(at(&doc, 6)), None));
    }

    #[test]
    fn sheet_010_before_the_first_heading_and_with_none() {
        let doc = crate::blocks::build(NOTE);
        let p = pick(&doc, 0, Scope::Section).unwrap();
        assert_eq!(p, Pick { lines: 0..2, before: None, after: Some(at(&doc, 2)) });
        let doc = crate::blocks::build("a\n\nb\n");
        assert_eq!(pick(&doc, 1, Scope::Section).unwrap(), Pick { lines: 0..3, before: None, after: None });
    }

    #[test]
    fn sheet_045_splice_keeps_every_other_line_and_the_final_newline() {
        assert_eq!(splice(NOTE, &(4..5), "para A\nmore"), NOTE.replace("para a", "para A\nmore"));
        assert_eq!(splice("a\nb", &(1..2), "B"), "a\nB");
        assert_eq!(splice("a\nb\n", &(0..1), ""), "\nb\n");
        assert_eq!(splice("a\nb\nc\n", &(0..3), "x"), "x\n");
        assert_eq!(&NOTE[line_bytes(NOTE, &(2..10))], "# One\n\npara a\n\n## Two\n\npara b\n");
    }

    #[test]
    fn pick_range_covers_every_selected_block_and_section() {
        let doc = crate::blocks::build(NOTE);
        // Selecting across "para a" (line 4) through "para b" (line 8) in Block scope edits lines 4..9.
        let p = pick_range(&doc, at(&doc, 4), at(&doc, 8), Scope::Block).unwrap();
        assert_eq!(p.lines, 4..9);
        assert_eq!((p.before, p.after), (Some(at(&doc, 2)), Some(at(&doc, 10))));
        assert_eq!(&NOTE[line_bytes(NOTE, &p.lines)], "para a\n\n## Two\n\npara b");
        // In Section scope, spanning "# One" into "# Three" covers both sections.
        let s = pick_range(&doc, at(&doc, 4), at(&doc, 12), Scope::Section).unwrap();
        assert_eq!(s.lines, 2..13);
        assert_eq!((s.before, s.after), (None, None));
    }

    #[test]
    fn preview_lightbox_edit_opens_edit_sheet() {
        let dir = std::env::temp_dir().join(format!("quire-lb-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Pic.md"), "# Pic\n\n![alt](attachments/a.png)\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Pic.md", None, tx);
            s.mode = crate::app::Mode::Preview;
            let img = crate::md::Img { alt: "alt".into(), src: "attachments/a.png".into(), embed: false };
            s.open_image_lightbox(vec![img], 0, 2..3, tx);
        });
        assert!(matches!(rt.state.layer(), Some(Layer::Lightbox(_))));
        rt.update(|s, tx| s.lightbox_edit(tx));
        let sh = rt.state.sheet().expect("lightbox_edit in Preview must open Edit Sheet");
        assert_eq!(sh.pick.lines, 2..3);
        assert_eq!(sh.edit.text, "![alt](attachments/a.png)");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
