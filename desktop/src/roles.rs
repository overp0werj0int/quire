//! The page's colour roles and code colours, derived from the active palette with omarky's
//! factors (`docs/reader-writer/behavior/page-and-typography.md`, sections 4 and 5, and
//! decision 0005). Quire's chrome keeps the palette's own tones; only the page uses these.
//!
//! omarky mixes colours per sRGB channel, as Qt and CSS do, so `mix` here works on encoded
//! channels, not in linear light as `Color::mix` does. Roles are cheap to derive, so views
//! derive them from the current palette and a theme switch recolours the page at once.

// The page takes up the rest of the roles and the code styles as phase 5 of the reader plan
// sets each block and adds a highlighter.
#![cfg_attr(not(test), allow(dead_code))]

use rune::Color;
use rune::color::{srgb_decode, srgb_encode};

use crate::theme::{Pal, contrast};

/// The least contrast of the side panels' faintest text against the panel.
pub const PANEL_FAINT: f32 = 4.5;
/// How much more contrast the panels' rows (dim) have than their faintest text, on every fill:
/// 1.4, and a little over so 8-bit colour keeps it.
pub const PANEL_STEP: f32 = 1.43;
/// How much more contrast the panels' text (the current row, headings) has than their rows.
pub const PANEL_TEXT: f32 = 1.15;

fn enc(c: Color) -> [f32; 3] {
    [srgb_encode(c.r), srgb_encode(c.g), srgb_encode(c.b)]
}

fn dec([r, g, b]: [f32; 3], a: f32) -> Color {
    Color { r: srgb_decode(r), g: srgb_decode(g), b: srgb_decode(b), a }
}

/// `a + (b - a) × t` per sRGB channel.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (x, y) = (enc(a), enc(b));
    dec([0, 1, 2].map(|i| x[i] + (y[i] - x[i]) * t), a.a + (b.a - a.a) * t)
}

/// Qt's `darker(f)`: the HSV value divided by `f`. Hue and saturation stay, so every channel
/// scales alike.
fn darker(c: Color, f: f32) -> Color {
    dec(enc(c).map(|v| v / f), c.a)
}

/// RGB Euclidean distance in 0–255 sRGB units.
fn distance(a: Color, b: Color) -> f32 {
    let (x, y) = (enc(a), enc(b));
    (0..3).map(|i| ((x[i] - y[i]) * 255.0).powi(2)).sum::<f32>().sqrt()
}

#[derive(Clone, Copy, Debug)]
pub struct Roles {
    pub bg: Color,
    pub accent: Color,
    pub text: Color,
    pub strong: Color,
    pub dim: Color,
    pub faint: Color,
    pub ghost: Color,
    pub raised: Color,
    pub sunken: Color,
    /// The fill of code and diff blocks.
    pub panel: Color,
    pub card: Color,
    /// The side panel's fill, a faint wash of the text colour (decision 0008).
    pub wash: Color,
    /// The colour of a panel row under the pointer.
    pub hover: Color,
    /// The sidebar's Open shelf and its search field, opaque: the sunken tone over the sidebar,
    /// darker than the sidebar in both themes (SB-02).
    pub shelf: Color,
    /// The one filled row of the sidebar: the active tab, the result Enter opens (SB-03).
    pub selected: Color,
    /// The accent used as text (SB-06). Equal to the accent here; `panel` lifts it to 4.5:1.
    pub accent_text: Color,
    pub hairline: Color,
    pub hairline_soft: Color,
    pub selection: Color,
    pub chip_fill: Color,
    pub chip_edge: Color,
    pub green: Color,
    pub red: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub orange: Color,
    pub link: Color,
    /// List markers, quote marks, task boxes, footnotes and tags in the source (WR-205).
    pub mark: Color,
    /// The paragraph focus fade in the source (WR-201).
    pub editor_ghost: Color,
    pub code_text: Color,
    pub code_bg: Color,
    /// Code's own dim and faint, a little stronger than the page's (PAGE-51).
    pub code_dim: Color,
    pub code_faint: Color,
    pub find: Color,
    pub find_current: Color,
    pub find_current_text: Color,
}

impl Roles {
    /// `c` flattened over the page. Rune draws a border over the box's own fill, where Qt draws
    /// it beside the fill, over what is behind; a filled box on the page gets omarky's border
    /// pixels from its border colour flattened so.
    pub fn on_page(&self, c: Color) -> Color {
        mix(self.bg, Color { a: 1.0, ..c }, c.a)
    }

    /// `fg`, moved toward black on a light surface or white on a dark one only when needed so
    /// it reaches at least `min` contrast against `bg` while preserving its hue (`CONTRAST4`).
    pub fn readable_on(fg: Color, bg: Color, min: f32) -> Color {
        if contrast(fg, bg) >= min {
            return fg;
        }
        let toward = if contrast(Color::WHITE, bg) > contrast(Color::BLACK, bg) { Color::WHITE } else { Color::BLACK };
        let mut lo = 0.0f32;
        let mut hi = 1.0f32;
        for _ in 0..16 {
            let mid = (lo + hi) * 0.5;
            if contrast(mix(fg, toward, mid), bg) >= min {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        mix(fg, toward, hi)
    }

    /// `self.dim`, nudged toward `self.text` only when needed so small markers on the page
    /// or code panel (ordered-list numerals, language badges, unresolved wiki links) reach
    /// WCAG AA with anti-aliasing headroom (`>= 5.0:1` nominal so 10 pt glyphs rasterize >= 4.5:1).
    pub fn readable_dim(&self) -> Color {
        let fills = [self.bg, self.panel];
        let mut out = self.dim;
        let mut t = 0.0;
        while fills.iter().any(|&f| contrast(out, f) < 5.0) && t < 1.0 {
            t += 0.02;
            out = mix(self.dim, self.text, t);
        }
        out
    }

    /// Page link ink (`self.link`) lifted only when needed to reach `>= 4.9:1` on `self.bg` so
    /// resolved wikilinks and web links rasterize >= 4.5:1 in every theme (`CONTRAST4`).
    pub fn readable_link(&self) -> Color {
        Self::readable_on(self.link, self.bg, 4.9)
    }

    /// Diff addition total ink (`self.green`) lifted only when needed to reach `>= 4.6:1` on
    /// `self.panel` across every theme (`CONTRAST4`).
    pub fn readable_green(&self) -> Color {
        Self::readable_on(self.green, self.panel, 4.6)
    }

    /// Diff deletion total ink (`self.red`) lifted only when needed to reach `>= 4.6:1` on
    /// `self.panel` across every theme (`CONTRAST4`).
    pub fn readable_red(&self) -> Color {
        Self::readable_on(self.red, self.panel, 4.6)
    }

    pub fn new(p: &Pal) -> Roles {
        let (fg, bg, accent, dark, n) = (p.ink, p.bg, p.acc, p.dark, p.named);
        let ld = |light: f32, dark_v: f32| if dark { dark_v } else { light };
        let flat = |c: Color, a: f32| mix(bg, c, a);
        let named = |c: Option<Color>| c.unwrap_or(accent);
        let blue = named(n.blue);
        let sunken = (if dark { Color::BLACK } else { fg }).alpha(ld(0.035, 0.22));
        Roles {
            bg,
            accent,
            text: fg,
            strong: n.strong.unwrap_or(fg),
            dim: mix(fg, bg, 0.34),
            faint: mix(fg, bg, 0.58),
            ghost: mix(fg, bg, 0.8),
            raised: fg.alpha(ld(0.045, 0.04)),
            sunken,
            panel: n.panel.unwrap_or_else(|| darker(bg, ld(1.04, 1.2))),
            card: fg.alpha(0.035),
            wash: fg.alpha(ld(0.025, 0.02)),
            hover: fg.alpha(0.06),
            shelf: mix(p.side, Color { a: 1.0, ..sunken }, sunken.a),
            selected: fg.alpha(0.1),
            accent_text: accent,
            hairline: fg.alpha(ld(0.14, 0.12)),
            hairline_soft: fg.alpha(ld(0.09, 0.075)),
            selection: accent.alpha(ld(0.22, 0.3)),
            chip_fill: fg.alpha(ld(0.07, 0.085)),
            chip_edge: fg.alpha(ld(0.07, 0.06)),
            green: named(n.green),
            red: n.red.unwrap_or(p.err),
            yellow: named(n.yellow),
            blue,
            magenta: named(n.magenta),
            cyan: named(n.cyan),
            orange: n.orange.unwrap_or(p.err),
            link: if distance(accent, fg) <= 24.0 { blue } else { accent },
            mark: mix(accent, bg, 0.15),
            editor_ghost: mix(fg, bg, 0.72),
            code_text: mix(fg, accent, 0.12),
            code_bg: flat(fg, ld(0.07, 0.085)),
            code_dim: mix(fg, bg, 0.3),
            code_faint: mix(fg, bg, 0.52),
            find: flat(accent, ld(0.22, 0.3)),
            find_current: accent,
            find_current_text: bg,
        }
    }

    /// The side panels' roles. Their text is small, so dim and faint move toward the text
    /// until they read on every fill a panel sits on: the window, the sidebar and the outline's
    /// wash. Faint reaches 4.5:1 (WCAG AA), and dim, the rows, has 1.4 times faint's contrast on
    /// each fill (`PANEL_STEP`), so rows and hints read as two levels. Where the theme's text leaves no room
    /// for that step and one more above it (Catppuccin Latte, Rosé Pine Dawn), the text first
    /// moves toward black or white. The panels have no fainter level: ghost is faint, so the
    /// title bar's "/" reads at 4.5:1 too.
    pub fn panel(p: &Pal) -> Roles {
        let mut r = Roles::new(p);
        // The sidebar's four surface steps (shelf, panel, hover, selected) stay clearly apart in
        // every theme so only the active tab or filled result reads as filled (SB-02, SB-03).
        r.shelf = if p.dark { mix(p.side, Color::BLACK, 0.32) } else { mix(p.side, p.ink, 0.045) };
        r.hover = p.ink.alpha(if p.dark { 0.045 } else { 0.04 });
        r.selected = p.ink.alpha(if p.dark { 0.13 } else { 0.11 });
        let fills = r.panel_fills(p);
        let lift = |c: Color, toward: Color, min: &dyn Fn(Color) -> f32| {
            let mut out = c;
            let mut t = 0.0;
            while fills.iter().any(|f| contrast(out, *f) < min(*f)) && t < 1.0 {
                t += 0.02;
                out = mix(c, toward, t);
            }
            out
        };
        r.faint = lift(r.faint, r.text, &|_| PANEL_FAINT);
        r.ghost = r.faint;
        let faint = r.faint;
        let extreme = if p.dark { Color::WHITE } else { Color::BLACK };
        r.text = lift(r.text, extreme, &|f| contrast(faint, f) * PANEL_STEP * PANEL_TEXT);
        r.dim = lift(r.dim, r.text, &|f| contrast(faint, f) * PANEL_STEP);
        let dim = r.dim;
        r.text = lift(r.text, extreme, &|f| contrast(dim, f) * PANEL_TEXT);
        r.accent_text = lift(r.accent, r.text, &|_| PANEL_FAINT);
        r
    }

    /// Every fill the side panels' text sits on: the window, the sidebar, the outline's wash,
    /// the shelf, and the selected fill on the shelf and on the sidebar.
    pub fn panel_fills(&self, p: &Pal) -> [Color; 6] {
        let over = |under: Color, c: Color| mix(under, Color { a: 1.0, ..c }, c.a);
        [p.bg, p.side, self.on_page(self.wash), self.shelf, over(self.shelf, self.selected), over(p.side, self.selected)]
    }

    /// The colour and face of one highlighter style (PAGE-50).
    pub fn code(&self, s: Style) -> Ink {
        use Style::*;
        let plain = |color| Ink { color, italic: false, bold: false, underline: false };
        match s {
            Normal | Variable => plain(self.text),
            Keyword | ControlFlow | Import => plain(self.magenta),
            Function | Extension | Information => plain(self.blue),
            String | VerbatimString => plain(self.green),
            SpecialString | BuiltIn | Others => plain(self.cyan),
            DecVal | BaseN | Float | Constant | Char | SpecialChar | Preprocessor => plain(self.orange),
            DataType | Attribute | Warning => plain(self.yellow),
            Operator | Annotation => plain(self.code_dim),
            Comment | Documentation => Ink { italic: true, ..plain(self.code_faint) },
            CommentVar => Ink { italic: true, ..plain(self.code_dim) },
            RegionMarker => plain(self.code_faint),
            Alert => Ink { bold: true, ..plain(self.red) },
            Error => Ink { underline: true, ..plain(self.red) },
        }
    }
}

/// The highlighter's style names, as KSyntaxHighlighting, which omarky uses, calls them. The
/// highlighter Quire picks maps its own token kinds onto these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Style {
    Normal,
    Keyword,
    Function,
    Variable,
    ControlFlow,
    Operator,
    BuiltIn,
    Extension,
    Preprocessor,
    Attribute,
    Char,
    SpecialChar,
    String,
    VerbatimString,
    SpecialString,
    Import,
    DataType,
    DecVal,
    BaseN,
    Float,
    Constant,
    Comment,
    Documentation,
    Annotation,
    CommentVar,
    RegionMarker,
    Information,
    Warning,
    Alert,
    Error,
    Others,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    pub color: Color,
    pub italic: bool,
    pub bold: bool,
    pub underline: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{hex, omarchy_pal, theme_pal, themed};

    /// `#rrggbb`, without the alpha that `Color::hex` adds.
    fn hx(c: Color) -> String {
        c.hex()[..7].to_string()
    }

    /// PAGE-34: the mixes run per sRGB channel, as in omarky. Paper's ink #2a2723 into its
    /// background #f8f5ef: red 0x2a + (0xf8 - 0x2a) × 0.34 = 0x70, and so on.
    #[test]
    fn page_34_mixes_are_per_srgb_channel() {
        let r = Roles::new(theme_pal("paper", false));
        assert_eq!(hx(r.dim), "#706d68");
        assert_eq!(hx(r.faint), "#a19e99");
        assert_eq!(hx(r.ghost), "#cfccc6");
        assert_eq!(hx(mix(hex("#000000"), hex("#ffffff"), 0.4)), "#666666", "not the linear-light #aaaaaa");
    }

    /// PAGE-35 to PAGE-40 and PAGE-44, PAGE-45: each light and dark pair follows the palette's
    /// flag.
    #[test]
    fn page_35_light_and_dark_factors_follow_the_flag() {
        let (l, d) = (Roles::new(theme_pal("paper", false)), Roles::new(theme_pal("ink", true)));
        assert_eq!((l.raised.a, d.raised.a), (0.045, 0.04));
        assert_eq!(hx(l.sunken), hx(theme_pal("paper", false).ink), "light sunken is the ink");
        assert_eq!((hx(d.sunken), l.sunken.a, d.sunken.a), ("#000000".into(), 0.035, 0.22));
        assert_eq!((l.card.a, l.hover.a), (0.035, 0.06));
        assert_eq!((l.hairline.a, d.hairline.a, l.hairline_soft.a, d.hairline_soft.a), (0.14, 0.12, 0.09, 0.075));
        assert_eq!((l.selection.a, d.selection.a), (0.22, 0.3));
        assert_eq!((l.chip_fill.a, d.chip_fill.a, l.chip_edge.a, d.chip_edge.a), (0.07, 0.085, 0.07, 0.06));
        assert_eq!(hx(l.code_bg), hx(mix(l.bg, l.text, 0.07)));
        assert_eq!(hx(d.code_bg), hx(mix(d.bg, d.text, 0.085)));
        assert_eq!(hx(l.find), hx(mix(l.bg, l.accent, 0.22)));
        assert_eq!(hx(d.find), hx(mix(d.bg, d.accent, 0.3)));
        assert_eq!((hx(l.find_current), hx(l.find_current_text)), (hx(l.accent), hx(l.bg)));
        assert_eq!(hx(l.code_text), hx(mix(l.text, l.accent, 0.12)));
    }

    /// PAGE-36: without a `dark_background`, the panel is the background darkened as Qt does.
    #[test]
    fn page_36_panel_darkens_the_background() {
        // #f8f5ef / 1.04 = #eeece6 and #1f1d1a / 1.2 = #1a1816, channel by channel.
        assert_eq!(hx(Roles::new(theme_pal("paper", false)).panel), "#eeece6");
        assert_eq!(hx(Roles::new(theme_pal("ink", true)).panel), "#1a1816");
        let om = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"\ndark_background = \"#1e1e1e\"\nbright_foreground = \"#ffffff\"").unwrap();
        let r = Roles::new(&om);
        assert_eq!((hx(r.panel), hx(r.strong)), ("#1e1e1e".into(), "#ffffff".into()), "PAGE-33, PAGE-36: Omarchy's own keys win");
        assert_eq!(hx(Roles::new(theme_pal("paper", false)).strong), hx(theme_pal("paper", false).ink));
    }

    /// PAGE-41, PAGE-42: named colours resolve from the theme, then the accent, with red and
    /// orange falling back to the error colour.
    #[test]
    fn page_41_named_colours_fall_back_in_order() {
        let nord = Roles::new(theme_pal("nord", true));
        assert_eq!((hx(nord.green), hx(nord.magenta), hx(nord.orange)), ("#a3be8c".into(), "#b48ead".into(), "#d5967a".into()));
        let paper = Roles::new(theme_pal("paper", false));
        assert_eq!((hx(paper.green), hx(paper.blue)), ("#5f7a3a".into(), "#3d6a9c".into()), "paper has its own");
        let bare = crate::theme::Pal { named: Default::default(), ..*theme_pal("paper", false) };
        let r = Roles::new(&bare);
        assert_eq!((hx(r.green), hx(r.cyan)), (hx(bare.acc), hx(bare.acc)), "no named colours: the accent");
        assert_eq!((hx(r.red), hx(r.orange)), (hx(bare.err), hx(bare.err)), "red and orange: the error colour");
        assert_eq!(hx(Roles::new(theme_pal("rose-pine", true)).orange), "#f6c177", "no orange: the theme's yellow");
        // Omarchy: the named key, else the numbered colour; orange, else yellow, else color3.
        let om = omarchy_pal("background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#3366ff\"\ncolor2 = \"#00ff00\"\nred = \"#ff0000\"\ncolor1 = \"#880000\"\ncolor3 = \"#ffff00\"").unwrap();
        let r = Roles::new(&om);
        assert_eq!((hx(r.green), hx(r.red), hx(r.yellow), hx(r.orange), hx(r.blue)), ("#00ff00".into(), "#ff0000".into(), "#ffff00".into(), "#ffff00".into(), "#3366ff".into()));
    }

    /// PAGE-43: a link turns blue when the accent is within distance 24 of the text.
    #[test]
    fn page_43_link_avoids_an_accent_like_the_text() {
        let om = omarchy_pal("background = \"#ffffff\"\nforeground = \"#101010\"\naccent = \"#1a1a1a\"\nblue = \"#2244cc\"").unwrap();
        assert_eq!(hx(Roles::new(&om).link), "#2244cc");
        let p = theme_pal("paper", false);
        assert_eq!(hx(Roles::new(p).link), hx(p.acc));
        // PAGE-89: a picked accent replaces the theme's in every accent role.
        let picked = Roles::new(themed("paper", false, "#1f8a80"));
        assert_eq!(hx(picked.link), hx(themed("paper", false, "#1f8a80").acc));
        assert_ne!(hx(picked.selection), hx(Roles::new(p).selection));
    }

    /// The side panels' small text reads in every theme: faint at 4.5:1 or more against
    /// the window, the sidebar and the outline's wash, dim (the rows) 1.4 times faint's contrast,
    /// and the text above both, so the open tab of Files, Search and Tags and the current row
    /// still read as the strongest. Nothing in them is fainter: ghost, and with it the title
    /// bar's "/", reaches 4.5:1 too.
    #[test]
    fn panel_text_reads_on_every_theme() {
        let omarchy = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"").unwrap();
        let mut pals: Vec<(&str, &Pal)> = crate::theme::THEMES.iter().map(|t| (t.id, theme_pal(t.id, t.dark))).collect();
        pals.push(("omarchy", &omarchy));
        assert_eq!(pals.len(), 15);
        for (id, p) in pals {
            let r = Roles::panel(p);
            for fill in r.panel_fills(p) {
                let (text, dim, faint) = (contrast(r.text, fill), contrast(r.dim, fill), contrast(r.faint, fill));
                assert!(faint >= PANEL_FAINT, "{id}: faint {faint:.2} on {}", hx(fill));
                assert!(dim >= faint * PANEL_STEP, "{id}: rows {dim:.2} not a step above faint {faint:.2} on {}", hx(fill));
                assert!(text >= dim * PANEL_TEXT * 0.99, "{id}: text {text:.2} not above rows {dim:.2} on {}", hx(fill));
                let ghost = contrast(r.ghost, fill);
                assert!(ghost >= PANEL_FAINT, "{id}: the \"/\" at {ghost:.2} on {}", hx(fill));
            }
        }
    }

    /// SB-02, SB-03, SB-06: the shelf is darker than the sidebar in every theme, hover and
    /// selected stay clearly separate steps on the shelf so only the selected row reads as filled,
    /// and the accent as text reads at 4.5:1 on every fill the sidebar has.
    #[test]
    fn the_shelf_reads_in_every_theme() {
        let lum = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        let over = |under: Color, c: Color| mix(under, Color { a: 1.0, ..c }, c.a);
        let omarchy = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"").unwrap();
        let mut pals: Vec<(&str, &Pal)> = crate::theme::THEMES.iter().map(|t| (t.id, theme_pal(t.id, t.dark))).collect();
        pals.push(("omarchy", &omarchy));
        for (id, p) in pals {
            let r = Roles::panel(p);
            assert!(lum(r.shelf) < lum(p.side), "{id}: the shelf {} is not darker than the sidebar {}", hx(r.shelf), hx(p.side));
            let [.., shelf, on_shelf, _] = r.panel_fills(p);
            let hov = over(shelf, r.hover);
            assert!(contrast(shelf, on_shelf) > 1.05, "{id}: the selected fill does not show on the shelf");
            assert!(distance(hov, on_shelf) > distance(shelf, hov), "{id}: hover {} sits too close to selected {} on shelf {}", hx(hov), hx(on_shelf), hx(shelf));
            for fill in r.panel_fills(p) {
                let a = contrast(r.accent_text, fill);
                assert!(a >= PANEL_FAINT, "{id}: the accent as text {a:.2} on {}", hx(fill));
            }
        }
    }

    /// PAGE-50, PAGE-51: the style table.
    #[test]
    fn page_50_code_styles_map_to_roles() {
        let r = Roles::new(theme_pal("tokyo-night", true));
        assert_eq!(r.code(Style::Keyword).color, r.magenta);
        assert_eq!(r.code(Style::String).color, r.green);
        assert_eq!(r.code(Style::Float).color, r.orange);
        assert_eq!(r.code(Style::DataType).color, r.yellow);
        assert_eq!(r.code(Style::BuiltIn).color, r.cyan);
        assert_eq!(r.code(Style::Function).color, r.blue);
        assert_eq!(r.code(Style::Variable).color, r.text);
        let c = r.code(Style::Comment);
        assert!(c.italic && c.color == r.code_faint);
        assert!(r.code(Style::CommentVar).italic && r.code(Style::CommentVar).color == r.code_dim);
        assert!(r.code(Style::Alert).bold && r.code(Style::Error).underline);
        assert_eq!(hx(r.code_dim), hx(mix(r.text, r.bg, 0.3)));
        assert_eq!(hx(r.code_faint), hx(mix(r.text, r.bg, 0.52)));
    }

    /// CONTRAST: titlebar crumb, find placeholder/count, lock/dialog placeholders (>=4.5:1)
    /// and preview ordered-list numerals (>=3.0:1) read across every built-in theme, and no
    /// hardcoded `#9a9184` placeholder remains in `desktop/src/ui/`.
    #[test]
    fn secondary_ui_and_list_numeral_contrast() {
        let omarchy = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"").unwrap();
        let mut pals: Vec<(&str, &Pal)> = crate::theme::THEMES.iter().map(|t| (t.id, theme_pal(t.id, t.dark))).collect();
        pals.push(("omarchy", &omarchy));
        for (id, p) in pals {
            let panel = Roles::panel(p);
            let page = Roles::new(p);
            let ph = crate::ui::styles::placeholder_ink(p);
            for fill in [p.bg, p.side, page.bg] {
                assert!(contrast(panel.faint, fill) >= 4.5, "{id}: panel.faint {:.2}:1 on {}", contrast(panel.faint, fill), hx(fill));
                assert!(contrast(ph, fill) >= 4.5, "{id}: placeholder_ink {:.2}:1 on {}", contrast(ph, fill), hx(fill));
            }
            assert!(contrast(page.dim, page.bg) >= 3.0, "{id}: ordered-list numeral (dim) {:.2}:1 on {}", contrast(page.dim, page.bg), hx(page.bg));
            assert!(contrast(page.readable_dim(), page.bg) >= 4.5, "{id}: readable_dim {:.2}:1 on {}", contrast(page.readable_dim(), page.bg), hx(page.bg));
            assert!(contrast(page.readable_dim(), page.panel) >= 4.5, "{id}: readable_dim {:.2}:1 on panel {}", contrast(page.readable_dim(), page.panel), hx(page.panel));
        }
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let mut stack = vec![ui];
        let mut found = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) == Some("rs")
                    && std::fs::read_to_string(&path).unwrap().to_ascii_lowercase().contains("9a9184")
                {
                    found.push(path.display().to_string());
                }
            }
        }
        assert!(found.is_empty(), "hardcoded #9a9184 found in: {found:?}");
    }

    /// CONTRAST4: resolved links on page, diff +N/-N totals on panel, and zoomed-out code badges
    /// reach WCAG AA (>= 4.5:1) across all 15 light/dark themes while preserving hue character.
    #[test]
    fn contrast4_links_and_diff_totals_meet_wcag_aa_across_all_themes() {
        let omarchy = omarchy_pal("background = \"#282828\"\nforeground = \"#d4be98\"\naccent = \"#7daea3\"").unwrap();
        let mut pals: Vec<(&str, &Pal)> = crate::theme::THEMES.iter().map(|t| (t.id, theme_pal(t.id, t.dark))).collect();
        pals.push(("omarchy", &omarchy));
        assert_eq!(pals.len(), 15);
        for (id, p) in pals {
            let r = Roles::new(p);
            let link_cr = contrast(r.readable_link(), r.bg);
            let green_cr = contrast(r.readable_green(), r.panel);
            let red_cr = contrast(r.readable_red(), r.panel);
            let dim_cr = contrast(r.readable_dim(), r.panel);
            assert!(link_cr >= 4.5, "{id}: readable_link {link_cr:.2}:1 (< 4.5) on {}", hx(r.bg));
            assert!(green_cr >= 4.5, "{id}: readable_green {green_cr:.2}:1 (< 4.5) on {}", hx(r.panel));
            assert!(red_cr >= 4.5, "{id}: readable_red {red_cr:.2}:1 (< 4.5) on {}", hx(r.panel));
            assert!(dim_cr >= 4.8, "{id}: readable_dim {dim_cr:.2}:1 (< 4.8) on {}", hx(r.panel));
        }
        // Hue preservation on the three lifted themes (Solarized Light, Paper, Nord).
        let sol = Roles::new(theme_pal("solarized-light", false));
        let sl = sol.readable_link();
        let sg = sol.readable_green();
        let sr = sol.readable_red();
        assert!(sl.b > sl.r && sl.b > sl.g, "Solarized Light link stays blue: {}", hx(sl));
        assert!(sg.g > sg.b, "Solarized Light diff +N stays green/olive: {}", hx(sg));
        assert!(sr.r > sr.g && sr.r > sr.b, "Solarized Light diff -N stays red: {}", hx(sr));
        let nord = Roles::new(theme_pal("nord", true));
        let nr = nord.readable_red();
        assert!(nr.r > nr.g && nr.r > nr.b, "Nord diff -N stays red: {}", hx(nr));
    }
}
