//! The file-export wrapper must paint the theme's canvas: an AI-generated
//! dark theme (background #121212, light text) exported onto the hardcoded
//! white wrapper rendered body text invisible.

use wxwright_core::{pipeline, wrap_document, ConvertOptions};

/// The canvas colour the wrapper actually painted on <body>.
///
/// Reading the declaration back out makes the assertion independent of
/// whitespace and quoting. The previous check compared a whole literal string
/// (`background: #FFFFFF;">\n<section`), so changing a space or an attribute
/// order would have turned it into a test that can never fail.
fn canvas_of(doc: &str) -> Option<String> {
    let after = doc.split("<body style=\"").nth(1)?;
    let style = after.split('"').next()?;
    style.split(';').find_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        (k.trim() == "background").then(|| v.trim().to_string())
    })
}

fn opts_for(theme_id: &str) -> ConvertOptions {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/themes")
            .join(format!("{}.toml", theme_id)),
    )
    .expect("built-in theme file");
    ConvertOptions::new(wxwright_core::theme::parse_theme(&src).unwrap())
}

#[test]
fn builtin_themes_keep_the_neutral_canvas() {
    // built-in themes declare no "background" key: exports stay on white
    let opts = opts_for("techblue");
    let out = pipeline("# 标题\n\n正文一段。", &opts).unwrap();
    let doc = wrap_document(&out.html, opts.theme.canvas());
    assert_eq!(canvas_of(&doc).as_deref(), Some("#FFFFFF"));
}

#[test]
fn wrap_document_defaults_to_white_without_theme_canvas() {
    // a theme without a "background" key keeps the neutral white canvas
    let mut opts = opts_for("minimal");
    opts.theme.colors.remove("background");
    let out = pipeline("# 标题", &opts).unwrap();
    let doc = wrap_document(&out.html, opts.theme.canvas());
    assert_eq!(canvas_of(&doc).as_deref(), Some("#FFFFFF"));
}

#[test]
fn dark_theme_export_is_self_consistent() {
    // simulate the AI generated dark theme: dark canvas key present
    let mut opts = opts_for("minimal");
    opts.theme
        .colors
        .insert("background".to_string(), "#121212".to_string());
    opts.theme
        .colors
        .insert("text".to_string(), "#e0e0e0".to_string());
    let out = pipeline("# 标题\n\n正文。", &opts).unwrap();
    let doc = wrap_document(&out.html, opts.theme.canvas());
    // The wrapper must paint the theme's own canvas, and a dark theme must
    // never end up on the hardcoded white one - that is what made body text
    // invisible. Asserted on the parsed declaration, not on a literal string.
    assert_eq!(canvas_of(&doc).as_deref(), Some("#121212"));
    assert_ne!(
        canvas_of(&doc).as_deref(),
        Some("#FFFFFF"),
        "a dark theme must not export on a white canvas"
    );
}
