//! wxwright-core: the pure-Rust engine shared by CLI, MCP and GUI hosts
//! (PRD 3.2). No UI, no network IO assumptions; adapters live in
//! wxwright-mp / wxwright-cli / wxwright-gui.

pub mod agentcard;
pub mod clipboard;
pub mod code;
pub mod error;
pub mod htmlutil;
pub mod i18n;
pub mod img;
pub mod ir;
pub mod normalizer;
pub mod parser;
pub mod platform;
pub mod render;
pub mod roles;
pub mod rules;
pub mod theme;
pub mod util;
pub mod validator;
pub mod walker;

use std::path::PathBuf;
use std::sync::Arc;

use img::{ImageMode, ImageOutcome, ImageTransport, ImgPipeline};
use ir::Stats;
use normalizer::NormalizeOptions;
use theme::Theme;

#[derive(Clone)]
pub struct ConvertOptions {
    pub theme: Theme,
    pub image_mode: ImageMode,
    /// Directory against which relative image paths resolve (usually the
    /// markdown file's parent).
    pub base_dir: Option<PathBuf>,
    pub transport: Option<Arc<dyn ImageTransport>>,
}

impl ConvertOptions {
    pub fn new(theme: Theme) -> Self {
        ConvertOptions {
            theme,
            image_mode: ImageMode::Inline,
            base_dir: None,
            transport: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConvertOutput {
    pub html: String,
    pub images: Vec<ImageOutcome>,
    pub stats: Stats,
    pub links: Vec<(String, String)>,
}

/// Markdown -> dialect HTML (steps 1-3 of the PRD 3.4 pipeline).
pub fn convert_markdown(md: &str, opts: &ConvertOptions) -> error::Result<ConvertOutput> {
    let blocks = parser::parse_markdown(md);
    let img = ImgPipeline::new(
        opts.image_mode,
        opts.base_dir.clone(),
        opts.transport.clone(),
    );
    let rendered = render::render_document(&blocks, &opts.theme, &img);
    let stats = ir::count_stats(&blocks);
    Ok(ConvertOutput {
        html: rendered.html,
        images: img.outcomes(),
        stats,
        links: rendered.links,
    })
}

/// Markdown -> normalized, validated dialect HTML: the full in-process chain
/// (PRD 3.4 steps 1-5). Hosts add the clipboard / draft / file exits.
pub fn pipeline(md: &str, opts: &ConvertOptions) -> error::Result<PipelineOutput> {
    let mut out = convert_markdown(md, opts)?;
    let norm = normalizer::normalize_html(&out.html, NormalizeOptions::default())?;
    out.html = norm.html;
    let violations = validator::validate_html(&out.html);
    Ok(PipelineOutput {
        html: out.html,
        images: out.images,
        stats: out.stats,
        links: out.links,
        fixes: norm.fixes,
        violations,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineOutput {
    pub html: String,
    pub images: Vec<ImageOutcome>,
    pub stats: Stats,
    pub links: Vec<(String, String)>,
    pub fixes: Vec<normalizer::FixRecord>,
    pub violations: Vec<validator::Violation>,
}

impl PipelineOutput {
    pub fn blocking_violations(&self) -> Vec<&validator::Violation> {
        self.violations.iter().filter(|v| v.is_block()).collect()
    }
}

/// HTML -> normalized, validated HTML: the direct-HTML entry (PRD 5.1 -
/// the normalizer as "the fixer for hand-authored HTML"). Arbitrary HTML
/// from any source (AI output, third-party editors, web clipping) goes in;
/// compliance-fixed HTML comes out. No template render, no theme - the
/// input's own inline styles ARE the layout, corrected to the official
/// rules. This is what makes wxwright the compliance + publishing layer
/// for HTML produced anywhere.
pub fn pipeline_html(html: &str) -> error::Result<PipelineOutput> {
    let norm = normalizer::normalize_html(html, NormalizeOptions::default())?;
    let violations = validator::validate_html(&norm.html);
    Ok(PipelineOutput {
        html: norm.html,
        images: Vec::new(),
        stats: ir::Stats::default(),
        links: Vec::new(),
        fixes: norm.fixes,
        violations,
    })
}

/// Wrap dialect HTML in a minimal self-contained document for file export
/// (F-08). Styling stays inline; the wrapper paints the theme's canvas so
/// dark themes export readable pages.
pub fn wrap_document(html: &str, canvas: &str) -> String {
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>wxwright export</title>\n</head>\n<body style=\"margin: 0; background: {};\">\n<section style=\"max-width: 677px; margin: 0 auto; padding: 24px 16px;\">\n{}\n</section>\n</body>\n</html>\n",
        canvas, html
    )
}

#[cfg(test)]
mod html_pipeline_tests {
    use super::*;

    /// The direct-HTML entry: arbitrary inline-styled HTML from any source
    /// goes through the same normalize + validate chain as dialect output.
    #[test]
    fn pipeline_html_normalizes_and_validates() {
        let raw = "<section style=\"width: 2000px; font-family: Arial;\"><p style=\"color: #333\">外部工具的 HTML</p></section>";
        let out = pipeline_html(raw).expect("pipeline_html ok");
        // R-1.4 fixed width auto-fixed, R-3.1 font-family stripped
        assert!(
            !out.html.contains("2000px"),
            "fixed width fixed: {}",
            out.html
        );
        assert!(
            !out.html.contains("Arial"),
            "font-family stripped: {}",
            out.html
        );
        assert!(out.html.contains("外部工具的 HTML"));
        assert_eq!(out.blocking_violations().len(), 0);
    }

    /// Dangerous HTML does not sneak through the direct entry either: the
    /// normalizer neutralizes what it can (script removed, heights fixed)
    /// and records the corrections in `fixes`.
    #[test]
    fn pipeline_html_neutralizes_dangerous_content() {
        let raw = "<section><script>alert(1)</script><p style=\"height: 0\">x</p></section>";
        let out = pipeline_html(raw).expect("pipeline_html ok");
        assert!(
            !out.html.contains("<script"),
            "script must be gone: {}",
            out.html
        );
        assert!(
            !out.html.contains("height: 0"),
            "height:0 must be fixed: {}",
            out.html
        );
        assert!(!out.fixes.is_empty(), "corrections must be recorded");
    }
}
