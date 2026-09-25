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

/// Wrap dialect HTML in a minimal self-contained document for file export
/// (F-08). Styling stays inline; the wrapper only sets a neutral canvas.
pub fn wrap_document(html: &str) -> String {
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>wxwright export</title>\n</head>\n<body style=\"margin: 0; background: #FFFFFF;\">\n<section style=\"max-width: 677px; margin: 0 auto; padding: 24px 16px;\">\n{}\n</section>\n</body>\n</html>\n",
        html
    )
}
