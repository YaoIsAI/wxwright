//! Server-side syntax highlighting via syntect (PRD F-04).
//! Output is fully inline: per-line spans with inline color declarations.
//! `font-family` is never emitted (R-3.1).

use std::sync::OnceLock;

use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct CodeToken {
    pub text: String,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
}

pub struct CodeLine {
    pub tokens: Vec<CodeToken>,
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub const LIGHT_THEME: &str = "InspiredGitHub";
pub const DARK_THEME: &str = "base16-ocean.dark";

/// Highlight `code` into per-line token spans. Falls back to plain text when
/// the language is unknown.
pub fn highlight(code: &str, lang: Option<&str>, dark: bool) -> Vec<CodeLine> {
    let ss = syntax_set();
    let ts = theme_set();
    let theme_name = if dark { DARK_THEME } else { LIGHT_THEME };
    let theme = match ts.themes.get(theme_name) {
        Some(t) => t,
        None => return plain_lines(code),
    };

    let syntax = lang
        .and_then(|l| ss.find_syntax_by_token(l))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut hl = syntect::easy::HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    for line in crate::util::split_lines(code) {
        let ranges = match hl.highlight_line(&line, ss) {
            Ok(r) => r,
            Err(_) => return plain_lines(code),
        };
        let mut tokens = Vec::new();
        for (style, text) in ranges {
            if text.is_empty() {
                continue;
            }
            let fg = style.foreground;
            let color = format!("#{:02X}{:02X}{:02X}", fg.r, fg.g, fg.b);
            let bold = style.font_style.contains(FontStyle::BOLD);
            let italic = style.font_style.contains(FontStyle::ITALIC);
            tokens.push(CodeToken {
                text: text.to_string(),
                color,
                bold,
                italic,
            });
        }
        out.push(CodeLine { tokens });
    }
    if out.is_empty() {
        out.push(CodeLine { tokens: Vec::new() });
    }
    out
}

fn plain_lines(code: &str) -> Vec<CodeLine> {
    crate::util::split_lines(code)
        .into_iter()
        .map(|l| CodeLine {
            tokens: vec![CodeToken {
                text: l,
                color: String::new(),
                bold: false,
                italic: false,
            }],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust() {
        let lines = highlight("fn main() {\n    let x = 1;\n}\n", Some("rust"), false);
        assert_eq!(lines.len(), 3);
        // Some token should carry a color.
        assert!(lines
            .iter()
            .any(|l| l.tokens.iter().any(|t| !t.color.is_empty())));
    }

    #[test]
    fn unknown_lang_falls_back() {
        let lines = highlight("hello\nworld\n", Some("nosuchlang"), false);
        assert_eq!(lines.len(), 2);
    }
}
