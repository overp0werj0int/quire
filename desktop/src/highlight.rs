//! Code colours for the reader (PAGE-49 to PAGE-53): Rune's `rune-highlights` lexers, with their
//! Zed token names mapped onto the KSyntaxHighlighting style names omarky colours
//! (`roles::Style`). Results are cached per language and code, so a redraw lexes nothing.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use rune_highlights::{Highlighter, Lang, Token};

use crate::roles::Style;

/// A fence's language name to a lexer, by omarky's aliases (PAGE-52) and then the lexers' own.
pub fn lang(name: &str) -> Option<Lang> {
    let n = name.trim().to_ascii_lowercase();
    let n = match n.as_str() {
        "sh" | "shell" | "console" => "bash",
        "math" | "tex" => "latex",
        "jsonc" => "json",
        "patch" => "diff",
        other => other,
    };
    Lang::from_name(n).filter(|l| l.name() != "plain")
}

/// One Zed token name to a style (PAGE-50).
pub fn style(token: &str) -> Style {
    use Style::*;
    match token {
        "comment" | "predictive" | "hint" => Comment,
        "comment.doc" => Documentation,
        "keyword.jsdoc" | "type.jsdoc" | "variable.jsdoc" => CommentVar,
        "keyword.control" => ControlFlow,
        "keyword.import" => Import,
        "keyword.operator" | "operator" | "punctuation.list_marker" | "punctuation.markup" => Operator,
        "preproc" | "tag.doctype" => Preprocessor,
        "string" => String,
        "text.literal" => VerbatimString,
        "string.regex" | "string.special" | "string.special.symbol" => SpecialString,
        "string.escape" | "punctuation.special" => SpecialChar,
        "number" => DecVal,
        "boolean" | "constant" | "constant.builtin" | "variant" => Constant,
        "type" | "type.class" | "enum" | "namespace" => DataType,
        "type.builtin" | "variable.special" => BuiltIn,
        "attribute" | "attribute.jsx" => Attribute,
        "function" | "function.definition" | "function.method" | "constructor" | "title" => Function,
        "label" => Others,
        "link_uri" => String,
        t if t.starts_with("keyword") || t.starts_with("tag") || t.starts_with("selector") || t == "property.json_key" => Keyword,
        _ => Normal,
    }
}

type Spans = Rc<[(Range<usize>, Style)]>;

thread_local! {
    static LEXER: RefCell<Highlighter> = RefCell::new(Highlighter::new());
    static CACHE: RefCell<HashMap<(String, String), Spans>> = RefCell::new(HashMap::new());
}

/// The styled ranges of `code` in fence language `name`, or none when no lexer knows it
/// (PAGE-53): such code is plain text in the text colour.
pub fn highlight(name: &str, code: &str) -> Spans {
    let Some(l) = lang(name) else { return Rc::from(Vec::new()) };
    let key = (l.name().to_string(), code.to_string());
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let out: Spans = LEXER.with(|h| h.borrow_mut().highlight(l, code)).into_iter().filter(|(_, t)| *t != Token::NONE).map(|(r, t)| (r, style(t.name()))).collect();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        // Notes come and go; a bounded cache is enough to keep redraws free.
        if c.len() > 512 {
            c.clear();
        }
        c.insert(key, out.clone());
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_52_aliases() {
        for (alias, name) in [("sh", "bash"), ("console", "bash"), ("js", "javascript"), ("ts", "typescript"), ("py", "python"), ("rs", "rust"), ("yml", "yaml"), ("jsonc", "json"), ("cpp", "c++"), ("tex", "latex")] {
            assert_eq!(lang(alias).map(|l| l.name()), lang(name).map(|l| l.name()), "{alias}");
            assert!(lang(alias).is_some(), "{alias} has a lexer");
        }
        assert!(lang("klingon").is_none() && lang("").is_none());
    }

    #[test]
    fn page_50_rust_tokens_get_ksh_styles() {
        let code = "// note\nfn main() { let x = \"s\"; return 42; }";
        let got = highlight("rust", code);
        // A token's range takes the spaces after it.
        let at = |s: &str| got.iter().find(|(r, _)| code[r.clone()].trim_end() == s).map(|x| x.1);
        assert_eq!(at("// note"), Some(Style::Comment));
        assert_eq!(at("\"s\""), Some(Style::String));
        assert_eq!(at("42"), Some(Style::DecVal));
        assert!(matches!(at("fn"), Some(Style::Keyword | Style::ControlFlow)));
        assert!(matches!(at("return"), Some(Style::Keyword | Style::ControlFlow)));
        assert!(highlight("klingon", code).is_empty());
    }
}
