//! The file-export wrapper must paint the theme's canvas: an AI-generated
//! dark theme (background #121212, light text) exported onto the hardcoded
//! white wrapper rendered body text invisible.

use wxwright_core::{pipeline, wrap_document, ConvertOptions};

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
    assert!(doc.contains("background: #FFFFFF"));
}

#[test]
fn wrap_document_defaults_to_white_without_theme_canvas() {
    // a theme without a "background" key keeps the neutral white canvas
    let mut opts = opts_for("minimal");
    opts.theme.colors.remove("background");
    let out = pipeline("# 标题", &opts).unwrap();
    let doc = wrap_document(&out.html, opts.theme.canvas());
    assert!(doc.contains("background: #FFFFFF"));
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
    assert!(doc.contains("background: #121212"));
    assert!(!doc.contains("background: #FFFFFF;\">\n<section"));
}
