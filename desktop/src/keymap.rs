//! The keymap: every key the app answers, as a table from a context and a chord to a named
//! action, in two presets (`docs/reader-writer/behavior/keymap.md`, decision 0003).
//!
//! Chords are written as the spec writes them ("Ctrl+Shift+K", "j", "G", "g g", "Space"), so
//! the table reads against the spec line by line, and a test checks that they agree. What each
//! action does is `keys::handler`; an action with no handler yet falls through.

use rune::{Key, KeyEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ctx {
    Global,
    Reader,
    Writer,
    Sheet,
    Find,
    Palette,
    Field,
    Help,
    Rewrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Omarky,
    Quire,
}

impl Preset {
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [Preset; 2] = [Preset::Omarky, Preset::Quire];

    pub fn from_id(id: &str) -> Preset {
        if id == "quire" { Preset::Quire } else { Preset::Omarky }
    }

    pub fn id(self) -> &'static str {
        match self {
            Preset::Omarky => "omarky",
            Preset::Quire => "quire",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    In,
    Adapted,
    Quire,
    Deferred,
    Dropped,
}

pub struct Action {
    /// The KEY item in the spec, which the spec test reads.
    #[cfg_attr(not(test), allow(dead_code))]
    pub item: u16,
    pub id: &'static str,
    /// How Settings and the help sheets name it.
    pub name: &'static str,
    pub ctx: &'static [Ctx],
    pub status: Status,
    pub omarky: &'static [&'static str],
    pub quire: &'static [&'static str],
    /// Two actions may share a chord in one context only when both have guards and the guards
    /// exclude each other. The conflict test reads it; the feature checks the condition.
    #[cfg_attr(not(test), allow(dead_code))]
    pub guard: Option<&'static str>,
}

impl Action {
    pub fn chords(&self, p: Preset) -> &'static [&'static str] {
        match p {
            Preset::Omarky => self.omarky,
            Preset::Quire => self.quire,
        }
    }
}

const fn a(item: u16, id: &'static str, name: &'static str, ctx: &'static [Ctx], status: Status, omarky: &'static [&'static str], quire: &'static [&'static str]) -> Action {
    Action { item, id, name, ctx, status, omarky, quire, guard: None }
}

const fn same(item: u16, id: &'static str, name: &'static str, ctx: &'static [Ctx], status: Status, chords: &'static [&'static str]) -> Action {
    a(item, id, name, ctx, status, chords, chords)
}

use Ctx::*;
use Status::*;

const W: &[Ctx] = &[Writer, Sheet];

pub const TABLE: &[Action] = &[
    a(8, "app.command-palette", "Command palette", &[Global], Adapted, &["Ctrl+Shift+K"], &["Ctrl+P"]),
    same(9, "app.quick-switcher", "Quick switcher", &[Global], Quire, &["Ctrl+O"]),
    a(10, "app.search-notes", "Search all notes", &[Global], Adapted, &["Ctrl+/", "Ctrl+?", "Ctrl+Shift+G"], &["Ctrl+/", "Ctrl+?", "Ctrl+Shift+F"]),
    a(11, "app.toggle-sidebar", "Toggle sidebar", &[Global], Adapted, &["Ctrl+Shift+B"], &["Ctrl+\\"]),
    a(12, "app.toggle-outline", "Toggle outline", &[Global], Adapted, &["Ctrl+\\"], &["Ctrl+Shift+O"]),
    a(13, "app.toggle-mode", "Source / preview", &[Global], Adapted, &["Ctrl+P", "Ctrl+Shift+P"], &["Ctrl+E"]),
    same(14, "app.settings", "Settings", &[Global], Quire, &["Ctrl+,"]),
    same(15, "app.focus-mode", "Zen mode", &[Global], Quire, &["Ctrl+."]),
    same(16, "app.new-note", "New note", &[Global], Quire, &["Ctrl+N"]),
    same(17, "app.quit", "Quit", &[Global], Quire, &["Ctrl+Q"]),
    same(18, "app.insert-image", "Insert an image", &[Global], Quire, &["Ctrl+Shift+I"]),
    same(19, "app.save", "Save now", &[Global], In, &["Ctrl+S"]),
    same(20, "app.close-tab", "Close tab", &[Global], In, &["Ctrl+W"]),
    same(21, "app.zoom-in", "Larger text", &[Global], In, &["Ctrl+=", "Ctrl++"]),
    same(22, "app.zoom-out", "Smaller text", &[Global], In, &["Ctrl+-"]),
    same(23, "app.escape", "Close or leave", &[Global], Quire, &["Esc"]),
    same(94, "app.undo-delete", "Undo the delete", &[Global], Quire, &["Ctrl+Shift+R"]),
    same(96, "app.next-pane", "Next pane", &[Global], Quire, &["F6"]),
    same(97, "app.previous-pane", "Previous pane", &[Global], Quire, &["Shift+F6"]),
    same(99, "app.tab-1", "Tab 1", &[Global], Quire, &["Ctrl+1"]),
    same(99, "app.tab-2", "Tab 2", &[Global], Quire, &["Ctrl+2"]),
    same(99, "app.tab-3", "Tab 3", &[Global], Quire, &["Ctrl+3"]),
    same(99, "app.tab-4", "Tab 4", &[Global], Quire, &["Ctrl+4"]),
    same(99, "app.tab-5", "Tab 5", &[Global], Quire, &["Ctrl+5"]),
    same(99, "app.tab-6", "Tab 6", &[Global], Quire, &["Ctrl+6"]),
    same(99, "app.tab-7", "Tab 7", &[Global], Quire, &["Ctrl+7"]),
    same(99, "app.tab-8", "Tab 8", &[Global], Quire, &["Ctrl+8"]),
    same(99, "app.tab-9", "Tab 9", &[Global], Quire, &["Ctrl+9"]),
    same(100, "app.next-tab", "Next tab", &[Global], Quire, &["Ctrl+Tab"]),
    same(101, "app.previous-tab", "Previous tab", &[Global], Quire, &["Ctrl+Shift+Tab"]),
    // Reader.
    same(24, "reader.scroll-down", "Scroll down", &[Reader], In, &["j", "Down"]),
    same(25, "reader.scroll-up", "Scroll up", &[Reader], In, &["k", "Up"]),
    same(26, "reader.page-down", "Page down", &[Reader], In, &["Space", "PageDown"]),
    same(27, "reader.page-up", "Page up", &[Reader], In, &["Shift+Space", "PageUp"]),
    same(28, "reader.top", "Top of the note", &[Reader], In, &["g g", "Home"]),
    same(29, "reader.bottom", "End of the note", &[Reader], In, &["G", "End"]),
    same(30, "reader.toggle-outline", "Toggle outline", &[Reader], In, &["o", "O"]),
    same(31, "reader.toggle-wide", "Readable or full width", &[Reader], In, &["w", "W"]),
    same(32, "reader.find", "Find", &[Reader], In, &["Ctrl+F", "/"]),
    Action { guard: Some("a find query exists"), ..same(33, "reader.find-next", "Next match", &[Reader], In, &["n"]) },
    same(34, "reader.find-previous", "Previous match", &[Reader], In, &["N"]),
    same(35, "reader.copy-selection", "Copy the selection", &[Reader], In, &["Ctrl+C"]),
    same(36, "reader.copy-block-under-pointer", "Copy the block's Markdown", &[Reader], In, &["Ctrl+Shift+C"]),
    same(37, "reader.copy-all", "Copy the whole note", &[Reader], In, &["Ctrl+Shift+A"]),
    same(38, "reader.block-menu", "Copy or edit a block", &[Reader], In, &["Right-click"]),
    same(39, "reader.read-from-disk", "Read from disk", &[Reader], In, &["Ctrl+R"]),
    same(40, "reader.zoom-reset", "Text size 14 px", &[Reader], In, &["Ctrl+0"]),
    same(41, "reader.edit-block", "Edit this block", &[Reader], In, &["e"]),
    same(42, "reader.edit-section", "Edit this section", &[Reader], In, &["E"]),
    same(43, "reader.help", "Keyboard help", &[Reader], Adapted, &["?", "F1"]),
    same(44, "reader.close-tab", "Close tab", &[Reader], In, &["q"]),
    same(45, "reader.escape", "Close find or clear the selection", &[Reader], In, &["Esc"]),
    same(46, "reader.versions", "Versions", &[Reader], Deferred, &["v", "V"]),
    same(47, "reader.fold", "Fold unchanged text", &[Reader], Deferred, &["f", "F"]),
    Action { guard: Some("changes mode with no find query"), ..same(48, "reader.next-change", "Next change", &[Reader], Deferred, &["n"]) },
    same(49, "reader.previous-change", "Previous change", &[Reader], Deferred, &["p"]),
    // Writer, and the sheet, whose editor is the writer's.
    same(50, "writer.bold", "Bold", W, In, &["Ctrl+B"]),
    same(51, "writer.italic", "Italic", W, In, &["Ctrl+I"]),
    a(52, "writer.inline-code", "Inline code", W, Adapted, &["Ctrl+E"], &["Ctrl+Shift+E"]),
    same(53, "writer.strikethrough", "Strikethrough", W, In, &["Ctrl+Shift+X"]),
    same(54, "writer.link", "Link", W, In, &["Ctrl+K"]),
    same(55, "writer.heading-1", "Heading 1", W, Adapted, &["Ctrl+Alt+1"]),
    same(55, "writer.heading-2", "Heading 2", W, Adapted, &["Ctrl+Alt+2"]),
    same(55, "writer.heading-3", "Heading 3", W, Adapted, &["Ctrl+Alt+3"]),
    same(55, "writer.heading-4", "Heading 4", W, Adapted, &["Ctrl+Alt+4"]),
    same(55, "writer.heading-5", "Heading 5", W, Adapted, &["Ctrl+Alt+5"]),
    same(55, "writer.heading-6", "Heading 6", W, Adapted, &["Ctrl+Alt+6"]),
    same(56, "writer.plain-paragraph", "Plain paragraph", W, In, &["Ctrl+0"]),
    same(57, "writer.indent", "Indent", W, In, &["Tab"]),
    same(58, "writer.outdent", "Outdent", W, In, &["Shift+Tab"]),
    same(59, "writer.continue-block", "Continue a list or quote", W, In, &["Enter"]),
    same(60, "writer.follow-link", "Follow the link at the caret", &[Writer], Quire, &["Ctrl+Enter"]),
    same(61, "writer.follow-link-click", "Follow a [[link]]", &[Writer], Quire, &["Ctrl+click"]),
    same(62, "writer.find", "Find", &[Writer], In, &["Ctrl+F"]),
    same(63, "writer.find-next", "Next match", &[Writer], In, &["Ctrl+G", "F3"]),
    same(64, "writer.find-previous", "Previous match", &[Writer], In, &["Shift+F3"]),
    same(65, "writer.recent-notes", "Recent notes", &[Writer], In, &["Ctrl+R"]),
    same(66, "writer.help", "Keyboard help", &[Writer], Adapted, &["F1"]),
    a(67, "writer.paragraph-focus", "Paragraph focus", &[Writer], Adapted, &["Ctrl+Shift+F"], &["Ctrl+Shift+D"]),
    same(68, "writer.typewriter", "Typewriter scrolling", &[Writer], In, &["Ctrl+Shift+T"]),
    same(69, "writer.next-font", "Next font", &[Writer], In, &["Ctrl+Shift+M"]),
    same(70, "writer.escape", "Close a menu, help or find, else show the preview", &[Writer], In, &["Esc"]),
    same(71, "writer.spelling", "Spell checking", &[Writer], Deferred, &["Ctrl+Shift+L"]),
    same(72, "writer.rewrite", "Rewrite", W, Deferred, &["Ctrl+J"]),
    same(73, "writer.rewrite-section", "Rewrite the section", W, Deferred, &["Ctrl+Shift+J"]),
    same(74, "writer.save-as", "Save as", &[Writer], Dropped, &[]),
    // Sheet.
    same(75, "sheet.save", "Save the sheet", &[Sheet], In, &["Ctrl+Enter", "Ctrl+S"]),
    same(76, "sheet.cancel", "Close the sheet", &[Sheet], In, &["Esc"]),
    // Find.
    same(77, "find.next", "Next match", &[Find], In, &["Enter"]),
    same(78, "find.previous", "Previous match", &[Find], In, &["Shift+Enter"]),
    same(79, "find.close", "Close find", &[Find], In, &["Esc"]),
    same(80, "find.select-all", "Select the query", &[Find], In, &["Ctrl+F"]),
    // Palette.
    same(81, "palette.next", "Next row", &[Palette], Quire, &["Down"]),
    same(82, "palette.previous", "Previous row", &[Palette], Quire, &["Up"]),
    same(83, "palette.run", "Open or run", &[Palette], Quire, &["Enter"]),
    same(84, "palette.back-to-notes", "Back to notes", &[Palette], Quire, &["Backspace"]),
    same(85, "palette.close", "Close the palette", &[Palette], Quire, &["Esc"]),
    // Field.
    same(86, "field.submit", "Submit", &[Field], Quire, &["Enter"]),
    same(87, "field.cancel", "Cancel", &[Field], Quire, &["Esc"]),
    // Help.
    same(88, "help.close", "Close help", &[Help], Adapted, &["Esc", "?", "q", "F1"]),
    // Rewrite.
    same(89, "rewrite.send", "Send", &[Rewrite], Deferred, &["Enter"]),
    same(90, "rewrite.close", "Close", &[Rewrite], Deferred, &["Esc"]),
    same(91, "rewrite.earlier", "Earlier rewrite", &[Rewrite], Deferred, &["Alt+Up"]),
    same(92, "rewrite.later", "Later rewrite", &[Rewrite], Deferred, &["Alt+Down"]),
    same(93, "rewrite.recall", "Earlier instructions", &[Rewrite], Deferred, &["Up", "Down"]),
];

pub fn action(id: &str) -> Option<&'static Action> {
    TABLE.iter().find(|a| a.id == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum K {
    /// A character, lowercase for letters.
    Char(char),
    Enter,
    Tab,
    Esc,
    Space,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

/// One key with its modifiers. Ctrl is Cmd on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: K,
}

/// A chord as the spec writes it, or None for a mouse gesture. "G" is Shift+g, and "Ctrl++"
/// ends in the "+" key. A sequence ("g g") gives one chord per step.
pub fn parse(spec: &str) -> Option<Vec<Chord>> {
    if spec.contains("click") {
        return None;
    }
    spec.split(' ').map(parse_one).collect()
}

fn parse_one(s: &str) -> Option<Chord> {
    let (mods, key) = match s.strip_suffix("++") {
        Some(m) => (m, "+"),
        None if s.len() > 1 && s.contains('+') => s.rsplit_once('+')?,
        None => ("", s),
    };
    let mut c = Chord { ctrl: false, shift: false, alt: false, key: K::Space };
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        match m {
            "Ctrl" => c.ctrl = true,
            "Shift" => c.shift = true,
            "Alt" => c.alt = true,
            _ => return None,
        }
    }
    c.key = match key {
        "Enter" => K::Enter,
        "Tab" => K::Tab,
        "Esc" => K::Esc,
        "Space" => K::Space,
        "Backspace" => K::Backspace,
        "Delete" => K::Delete,
        "Up" => K::Up,
        "Down" => K::Down,
        "Left" => K::Left,
        "Right" => K::Right,
        "Home" => K::Home,
        "End" => K::End,
        "PageUp" => K::PageUp,
        "PageDown" => K::PageDown,
        f if f.len() > 1 && f.starts_with('F') => K::F(f[1..].parse().ok()?),
        ch => {
            let mut it = ch.chars();
            let (ch, None) = (it.next()?, it.next()) else { return None };
            // A plain capital is the letter typed with Shift; with Ctrl the spec writes
            // letters as capitals and says Shift outright.
            if ch.is_ascii_uppercase() && !c.ctrl {
                c.shift = true;
            }
            K::Char(ch.to_ascii_lowercase())
        }
    };
    Some(c)
}

fn named(key: &Key) -> Option<K> {
    Some(match key {
        Key::Enter => K::Enter,
        Key::Tab => K::Tab,
        Key::Escape => K::Esc,
        Key::Backspace => K::Backspace,
        Key::Delete => K::Delete,
        Key::Up => K::Up,
        Key::Down => K::Down,
        Key::Left => K::Left,
        Key::Right => K::Right,
        Key::Home => K::Home,
        Key::End => K::End,
        Key::PageUp => K::PageUp,
        Key::PageDown => K::PageDown,
        Key::Function(n) => K::F(*n),
        Key::Text(t) if t == " " => K::Space,
        Key::Command(' ') => K::Space,
        _ => return None,
    })
}

/// Whether `ev` is `c`, by KEY-03's rules.
pub fn matches(c: &Chord, ev: &KeyEvent) -> bool {
    let m = ev.mods;
    // Alt never joins a chord that does not name it, so Ctrl+Alt (AltGr) types text.
    if m.alt != c.alt || (m.logo && !cfg!(target_os = "macos")) {
        return false;
    }
    let ctrl_ok = m.cmd == c.ctrl || (cfg!(target_os = "macos") && c.key == K::Tab && c.ctrl && m.ctrl && !m.cmd);
    if !ctrl_ok {
        return false;
    }
    match c.key {
        K::Char(want) => {
            let got = match &ev.key {
                Key::Command(ch) => *ch,
                Key::Text(t) if t.chars().count() == 1 => t.chars().next().unwrap_or(' '),
                _ => return false,
            };
            if want.is_ascii_alphanumeric() {
                if c.ctrl {
                    got.to_ascii_lowercase() == want && m.shift == c.shift
                } else {
                    // The character typed: "G" is Shift+g.
                    got == if c.shift { want.to_ascii_uppercase() } else { want }
                }
            } else {
                // A symbol is the character the key types, whatever Shift did to get it.
                got == want
            }
        }
        k => named(&ev.key) == Some(k) && m.shift == c.shift,
    }
}

/// The action `ev` runs in `ctx`, for chords of a single step.
pub fn lookup(p: Preset, ctx: Ctx, ev: &KeyEvent) -> Vec<&'static Action> {
    TABLE
        .iter()
        .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
        .filter(|a| a.chords(p).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 1 && matches(&seq[0], ev)))
        .collect()
}

/// The actions whose two-key chord is `first` then `ev` in `ctx` (`g g`, RD-23).
pub fn lookup_seq(p: Preset, ctx: Ctx, first: &KeyEvent, ev: &KeyEvent) -> Vec<&'static Action> {
    TABLE
        .iter()
        .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
        .filter(|a| a.chords(p).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 2 && matches(&seq[0], first) && matches(&seq[1], ev)))
        .collect()
}

/// Whether `ev` starts a two-key chord in `ctx`.
pub fn starts_seq(p: Preset, ctx: Ctx, ev: &KeyEvent) -> bool {
    TABLE
        .iter()
        .filter(|a| a.ctx.contains(&ctx) && !matches!(a.status, Deferred | Dropped))
        .any(|a| a.chords(p).iter().filter_map(|s| parse(s)).any(|seq| seq.len() == 2 && matches(&seq[0], ev)))
}

/// A chord's label on this platform: as the spec writes it, or with ⌘⇧⌥ run together on macOS.
pub fn label_of(spec: &str) -> String {
    if !cfg!(target_os = "macos") {
        return spec.to_string();
    }
    if spec == "Ctrl+click" {
        return "⌘-click".into();
    }
    spec.replace("Ctrl+", "⌘").replace("Shift+", "⇧").replace("Alt+", "⌥")
}

/// A platform-formatted primary-modifier chord label for a built-in editing key (e.g. `'x'` ->
/// `"Ctrl+X"` on Linux/Windows, `"⌘X"` on macOS) (`TYPOPOLISH`).
pub fn cmd_label(ch: char) -> String {
    label_of(&format!("Ctrl+{}", ch.to_ascii_uppercase()))
}

/// The first chord of `id` in preset `p`, labelled, or "" when it has none.
pub fn label(p: Preset, id: &str) -> String {
    action(id).and_then(|a| a.chords(p).first()).map(|s| label_of(s)).unwrap_or_default()
}

/// A button's tooltip: its name, then its first key in brackets when it has one.
pub fn tip(p: Preset, name: &str, id: &str) -> String {
    match label(p, id) {
        k if k.is_empty() => name.to_string(),
        k => format!("{name} ({k})"),
    }
}

/// Internal shifted-key aliases kept for key matching (`Ctrl+?` for `Ctrl+Shift+/`) that
/// should not clutter displayed shortcut lists.
fn is_display_alias(spec: &str) -> bool {
    spec == "Ctrl+?"
}

/// Every user-facing chord of `id` in preset `p`, labelled individually (hiding internal
/// shifted-key aliases like `Ctrl+?`).
pub fn display_labels(p: Preset, id: &str) -> Vec<String> {
    action(id)
        .map(|a| a.chords(p).iter().filter(|s| !is_display_alias(s)).map(|s| label_of(s)).collect())
        .unwrap_or_default()
}

/// Every user-facing chord of `id` in preset `p`, labelled and joined.
#[cfg(test)]
pub fn labels(p: Preset, id: &str) -> String {
    display_labels(p, id).join(" or ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;
    use std::collections::HashMap;

    fn ev(key: Key, ctrl: bool, shift: bool) -> KeyEvent {
        KeyEvent { key, mods: Mods { cmd: ctrl, ctrl: ctrl && !cfg!(target_os = "macos"), shift, ..Default::default() } }
    }

    fn ids(p: Preset, ctx: Ctx, e: &KeyEvent) -> Vec<&'static str> {
        lookup(p, ctx, e).iter().map(|a| a.id).collect()
    }

    fn live(a: &Action) -> bool {
        !matches!(a.status, Dropped)
    }

    /// The chords of each context, for one preset: chord to actions.
    fn by_chord(p: Preset) -> HashMap<(Ctx, Vec<Chord>), Vec<&'static Action>> {
        let mut m: HashMap<(Ctx, Vec<Chord>), Vec<&'static Action>> = HashMap::new();
        for a in TABLE.iter().filter(|a| live(a)) {
            for s in a.chords(p) {
                let Some(seq) = parse(s) else { continue };
                for c in a.ctx {
                    m.entry((*c, seq.clone())).or_default().push(a);
                }
            }
        }
        m
    }

    #[test]
    fn every_chord_parses() {
        for a in TABLE {
            for p in Preset::ALL {
                for s in a.chords(p) {
                    assert!(parse(s).is_some() || s.contains("click"), "{} {s}", a.id);
                }
            }
        }
        assert_eq!(parse("G"), Some(vec![Chord { ctrl: false, shift: true, alt: false, key: K::Char('g') }]));
        assert_eq!(parse("Ctrl++").unwrap()[0].key, K::Char('+'));
        assert_eq!(parse("g g").unwrap().len(), 2);
        assert_eq!(parse("Shift+F3").unwrap()[0], Chord { ctrl: false, shift: true, alt: false, key: K::F(3) });
    }

    /// Checks: no context binds one chord to two actions, unless both have guards that differ.
    #[test]
    fn no_chord_does_two_things() {
        for p in Preset::ALL {
            for ((ctx, seq), acts) in by_chord(p) {
                if acts.len() > 1 {
                    let guarded = acts.iter().all(|a| a.guard.is_some()) && acts[0].guard != acts[1].guard;
                    assert!(guarded && acts.len() == 2, "{p:?} {ctx:?} {seq:?}: {:?}", acts.iter().map(|a| a.id).collect::<Vec<_>>());
                }
            }
        }
    }

    /// Checks: every action has a chord in each preset, apart from mouse-only and dropped ones.
    #[test]
    fn every_action_has_a_chord() {
        for a in TABLE.iter().filter(|a| live(a)) {
            for p in Preset::ALL {
                assert!(!a.chords(p).is_empty(), "{} has no chord in {p:?}", a.id);
            }
        }
    }

    /// Checks: no chord in a context that falls through to global shadows a global chord.
    #[test]
    fn no_context_shadows_a_global_chord() {
        for p in Preset::ALL {
            let m = by_chord(p);
            for ((ctx, seq), acts) in &m {
                if !matches!(ctx, Reader | Writer | Find | Palette | Field) || seq[0].key == K::Esc {
                    continue;
                }
                assert!(!m.contains_key(&(Global, seq.clone())), "{p:?} {ctx:?} {} shadows a global chord", acts[0].id);
            }
        }
    }

    /// Checks: the writer, sheet, find, palette and field contexts leave the text-field keys to
    /// Rune (KEY-06), apart from the palette's Up, Down and Backspace.
    #[test]
    fn text_field_keys_stay_with_the_field() {
        let field_keys = ["Up", "Down", "Left", "Right", "Home", "End", "PageUp", "PageDown", "Backspace", "Delete", "Ctrl+A", "Ctrl+C", "Ctrl+X", "Ctrl+V", "Ctrl+Z", "Ctrl+Shift+Z", "Ctrl+Y"];
        let field: Vec<Vec<Chord>> = field_keys.iter().map(|s| parse(s).unwrap()).collect();
        for p in Preset::ALL {
            for ((ctx, seq), acts) in by_chord(p) {
                if !matches!(ctx, Writer | Sheet | Find | Palette | Field) || !field.contains(&seq) {
                    continue;
                }
                assert!(matches!(acts[0].id, "palette.next" | "palette.previous" | "palette.back-to-notes"), "{p:?} {} takes a field key", acts[0].id);
            }
        }
    }

    /// Checks: the table agrees with the spec. Each KEY item's chords in each preset, its
    /// contexts and its status match the action with that item number.
    #[test]
    fn the_table_agrees_with_the_spec() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/reader-writer/behavior/keymap.md");
        let spec = std::fs::read_to_string(path).unwrap();
        let mut seen = 0;
        for line in spec.lines().filter(|l| l.starts_with("- **KEY-")) {
            let item: u16 = line.strip_prefix("- **KEY-").unwrap().split_once("**").unwrap().0.parse().unwrap();
            let Some(rest) = line.split_once("omarky preset ").map(|x| x.1) else { continue };
            seen += 1;
            let (om, rest) = rest.split_once("; quire preset ").unwrap();
            let qu = rest.split_once(". ").map_or(rest, |x| x.0);
            let ticks = |s: &str| -> Vec<String> { s.split('`').skip(1).step_by(2).map(String::from).collect() };
            let ctxs: Vec<&str> = line.split_once("): omarky").unwrap().0.rsplit_once('(').unwrap().1.split(", ").collect();
            let status = line.split_once("** [").unwrap().1.split(&[']', ':'][..]).next().unwrap();
            let acts: Vec<&Action> = TABLE.iter().filter(|a| a.item == item).collect();
            assert!(!acts.is_empty(), "KEY-{item} is not in the table");
            if item == 55 {
                // Six actions, Ctrl+Alt+1 to Ctrl+Alt+6.
                assert_eq!(acts.len(), 6);
                for (i, a) in acts.iter().enumerate() {
                    assert_eq!(a.omarky, &[format!("Ctrl+Alt+{}", i + 1).as_str()][..]);
                    assert_eq!(a.quire, &[format!("Ctrl+Alt+{}", i + 1).as_str()][..]);
                }
                continue;
            }
            if item == 99 {
                // Nine actions, Ctrl+1 to Ctrl+9.
                assert_eq!(acts.len(), 9);
                for (i, a) in acts.iter().enumerate() {
                    assert_eq!(a.omarky, &[format!("Ctrl+{}", i + 1).as_str()][..]);
                    assert_eq!(a.quire, &[format!("Ctrl+{}", i + 1).as_str()][..]);
                }
                continue;
            }
            let a = acts[0];
            assert_eq!(ticks(om), a.omarky.to_vec(), "KEY-{item} omarky chords");
            assert_eq!(ticks(qu), a.quire.to_vec(), "KEY-{item} quire chords");
            let names: Vec<String> = a.ctx.iter().map(|c| format!("{c:?}").to_lowercase()).collect();
            assert_eq!(ctxs, names, "KEY-{item} contexts");
            let want = match a.status {
                In => "in",
                Adapted => "adapted",
                Quire => "quire",
                Deferred => "deferred",
                Dropped => "dropped",
            };
            assert_eq!(status, want, "KEY-{item} status");
        }
        assert_eq!(seen, 92, "every KEY item with chords was read");
        let items: std::collections::HashSet<u16> = TABLE.iter().map(|a| a.item).collect();
        assert_eq!(items.len(), seen, "the table has no item the spec lacks");
    }

    /// KEY-03: letters and digits need their exact modifiers, symbols ignore Shift, plain
    /// reader keys match the character typed, and Alt is never part of a command.
    #[test]
    fn key_03_chords_match_as_the_spec_says() {
        let o = Preset::Omarky;
        assert_eq!(ids(o, Global, &ev(Key::Command('k'), true, true)), ["app.command-palette"]);
        assert_eq!(ids(o, Writer, &ev(Key::Command('k'), true, false)), ["writer.link"]);
        assert!(ids(o, Writer, &ev(Key::Command('k'), true, true)).is_empty(), "Ctrl+K never fires on Ctrl+Shift+K");
        assert_eq!(ids(o, Global, &ev(Key::Command('+'), true, true)), ["app.zoom-in"], "Ctrl+Shift+= types +");
        assert_eq!(ids(o, Global, &ev(Key::Command('='), true, false)), ["app.zoom-in"]);
        assert_eq!(ids(o, Global, &ev(Key::Command('?'), true, true)), ["app.search-notes"], "Ctrl+Shift+/ types ?");
        assert_eq!(ids(o, Reader, &ev(Key::Text("G".into()), false, true)), ["reader.bottom"]);
        assert_eq!(ids(o, Reader, &ev(Key::Text("?".into()), false, true)), ["reader.help"]);
        assert_eq!(ids(o, Reader, &ev(Key::Text(" ".into()), false, true)), ["reader.page-up"]);
        let mut alt = ev(Key::Text("j".into()), false, false);
        alt.mods.alt = true;
        assert!(ids(o, Reader, &alt).is_empty(), "reader keys with Alt are ignored");
        assert_eq!(ids(o, Reader, &ev(Key::Text("n".into()), false, false)), ["reader.find-next"], "deferred actions never run");
    }

    /// SB-100 to SB-102: Ctrl+/ and Ctrl+? search in both presets beside each preset's own
    /// key, and F1 opens help in both modes and closes it; Ctrl+/ no longer reaches help.
    #[test]
    fn ctrl_slash_searches_and_f1_is_help() {
        let f1 = KeyEvent { key: Key::Function(1), mods: Mods::default() };
        for (p, own) in [(Preset::Omarky, 'g'), (Preset::Quire, 'f')] {
            assert_eq!(ids(p, Global, &ev(Key::Command('/'), true, false)), ["app.search-notes"], "{p:?}");
            assert_eq!(ids(p, Global, &ev(Key::Command('?'), true, true)), ["app.search-notes"], "{p:?}");
            assert_eq!(ids(p, Global, &ev(Key::Command(own), true, true)), ["app.search-notes"], "{p:?} keeps its own key");
            assert_eq!(label(p, "app.search-notes"), if cfg!(target_os = "macos") { "⌘/" } else { "Ctrl+/" }, "{p:?}: the field's hint");
            assert_eq!(ids(p, Writer, &f1), ["writer.help"], "{p:?}");
            assert_eq!(ids(p, Reader, &f1), ["reader.help"], "{p:?}");
            assert_eq!(ids(p, Help, &f1), ["help.close"], "{p:?}");
            assert!(ids(p, Writer, &ev(Key::Command('/'), true, false)).is_empty(), "{p:?}: Ctrl+/ is not the writer's");
            assert!(ids(p, Help, &ev(Key::Command('/'), true, false)).is_empty(), "{p:?}: Ctrl+/ does not close help");
        }
    }

    /// KEY-05: labels follow the preset.
    #[test]
    fn key_05_labels_follow_the_preset() {
        let mac = cfg!(target_os = "macos");
        assert_eq!(label(Preset::Omarky, "app.command-palette"), if mac { "⌘⇧K" } else { "Ctrl+Shift+K" });
        assert_eq!(label(Preset::Quire, "app.command-palette"), if mac { "⌘P" } else { "Ctrl+P" });
        assert_eq!(labels(Preset::Omarky, "app.zoom-in"), if mac { "⌘= or ⌘+" } else { "Ctrl+= or Ctrl++" });
        assert_eq!(display_labels(Preset::Omarky, "app.search-notes"), if mac { vec!["⌘/", "⌘⇧G"] } else { vec!["Ctrl+/", "Ctrl+Shift+G"] });
        assert_eq!(display_labels(Preset::Quire, "app.search-notes"), if mac { vec!["⌘/", "⌘⇧F"] } else { vec!["Ctrl+/", "Ctrl+Shift+F"] });
        assert!(!labels(Preset::Omarky, "app.search-notes").contains("?"));
        assert_eq!(label(Preset::Omarky, "no.such-action"), "");
    }
}
