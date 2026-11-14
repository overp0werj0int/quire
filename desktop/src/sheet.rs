//! The edit sheet (`docs/reader-writer/behavior/edit-sheet.md`, phase 8): `e` and `E` in the
//! reader open one block, or its section, as source lines in the writer's editor over the
//! page, and save them back through Quire's own save path. The view is `ui/sheet.rs`.

use std::ops::Range;
use std::rc::Rc;

use rune::{TextEdit, Tx};

use crate::app::{App, Mode};
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
pub fn pick(doc: &Doc, i: usize, scope: Scope) -> Option<Pick> {
    let b = doc.blocks.get(i)?;
    if scope == Scope::Block {
        return Some(Pick { lines: b.lines.clone(), before: i.checked_sub(1), after: (i + 1 < doc.blocks.len()).then_some(i + 1) });
    }
    let end_all = doc.blocks.last().map_or(0, |b| b.lines.end);
    let hs = &doc.headings;
    // The nearest heading at or above the block's first line.
    let Some(h) = hs.iter().rposition(|h| h.line <= b.lines.start) else {
        // Before the first heading, or no headings at all (SHEET-010).
        return Some(match hs.first() {
            Some(first) => Pick { lines: 0..first.line, before: None, after: Some(first.index) },
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
    /// When Esc armed a discard (SHEET-038).
    pub armed: Option<f64>,
}

impl App {
    /// `e` and `E` (SHEET-001 to SHEET-003, SHEET-013).
    pub fn open_sheet(&mut self, scope: Scope, tx: &mut Tx) {
        let Some(rel) = self.active.clone() else { return };
        if self.mode != Mode::Preview || self.note_locked(&rel) {
            return;
        }
        let doc = self.doc();
        let Some(i) = self.sheet_block(&doc, tx) else { return };
        let Some(pick) = pick(&doc, i, scope) else { return };
        let base = self.active_text().to_string();
        let orig = base[line_bytes(&base, &pick.lines)].to_string();
        let mut edit = TextEdit::new(orig.clone());
        edit.set_selection(0, 0);
        self.reader.pending = None;
        self.sheet = Some(Sheet { rel, scope, pick, doc, base, orig, edit, problem: None, armed: None });
        tx.clear_text_selection();
        tx.focus(EDITOR, false);
    }

    /// The block under the pointer, else the one holding the selection, else the top one
    /// (SHEET-002).
    fn sheet_block(&self, doc: &Doc, tx: &mut Tx) -> Option<usize> {
        if let Some(l) = self.reader.hover
            && let Some(i) = doc.blocks.iter().position(|b| b.lines.start == l)
        {
            return Some(i);
        }
        let held = tx.text_selection().and_then(|s| {
            s.texts().iter().filter(|t| t.is_selected()).find_map(|t| t.path().split('/').find(|seg| seg.starts_with("pb-")).map(str::to_string))
        });
        if let Some(k) = held
            && let Some(i) = doc.blocks.iter().position(|b| crate::reader::block_key(b) == k)
        {
            return Some(i);
        }
        Some(self.top()).filter(|&i| i < doc.blocks.len())
    }

    /// Ctrl+Enter and Ctrl+S (SHEET-033, SHEET-044 to SHEET-048).
    pub fn sheet_save(&mut self, tx: &mut Tx) {
        let Some(rel) = self.sheet.as_ref().map(|sh| sh.rel.clone()) else { return };
        let gone = !self.files().contains_key(&rel) || !self.tabs.contains(&rel);
        let now = if let Some(u) = self.unlocked.get(&rel) {
            Some(u.text.clone())
        } else {
            self.files().get(&rel).filter(|t| !crate::lock::is_locked(t)).cloned()
        };
        let Some(sh) = &mut self.sheet else { return };
        if gone {
            sh.problem = Some(GONE);
            return;
        }
        if now.as_deref() != Some(sh.base.as_str()) {
            sh.problem = Some(CHANGED);
            return;
        }
        let r = line_bytes(&sh.base, &sh.pick.lines);
        let with = sh.edit.text.clone();
        let text = splice(&sh.base, &sh.pick.lines, &with);
        self.sheet = None;
        // One step in the note's source history (edit-sheet.md, open questions).
        self.ensure_edit();
        if let Some(e) = self.edits.get_mut(&rel) {
            let (a, f) = (e.anchor, e.focus);
            let shift = |p: usize| if p <= r.start { p } else if p >= r.end { p + with.len() - r.len() } else { r.start };
            e.undo_group(|e| {
                e.set_selection(r.start, r.end);
                e.insert(&with);
            });
            e.set_selection(shift(a), shift(f));
        }
        self.set_text(&rel, text);
        // Written at once, so "Saved" is true by the time anyone looks (open questions).
        self.flush_notes();
        self.toast("Saved", tx);
        tx.blur();
    }

    /// Esc (SHEET-037 to SHEET-039): close an unchanged sheet, else arm a discard, else
    /// discard.
    pub fn sheet_escape(&mut self, tx: &mut Tx) {
        let now = self.now.get();
        let Some(sh) = &mut self.sheet else { return };
        let armed = sh.armed.is_some_and(|at| now - at < DISCARD);
        if sh.edit.text == sh.orig || armed {
            self.sheet = None;
            tx.blur();
        } else {
            sh.armed = Some(now);
            tx.wake_after(std::time::Duration::from_secs_f64(DISCARD + 0.05));
        }
    }

    /// The armed discard lapses (SHEET-038). True when it did.
    pub(crate) fn expire_sheet(&mut self, t: f64) -> bool {
        match &mut self.sheet {
            Some(sh) if sh.armed.is_some_and(|at| t - at >= DISCARD) => {
                sh.armed = None;
                true
            }
            _ => false,
        }
    }

    /// The editor the writer's editing keys act on: the sheet's while it is open.
    pub fn sheet_edit(&mut self) -> Option<&mut TextEdit> {
        self.sheet.as_mut().map(|s| &mut s.edit)
    }

    /// The sheet's field, for its edit hook, which can still run in the frame the sheet
    /// closes; that last edit lands in a scratch field.
    pub fn sheet_field(&mut self) -> &mut TextEdit {
        if self.sheet.is_some() {
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
}
