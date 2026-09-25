//! Image pipeline (PRD 4.2): resolve every image to a paste-safe source.
//!
//! - local files: real pixel width via the image crate -> data-w / data-ratio
//!   (I-01), then inline base64 or upload to mmbiz depending on mode;
//! - http(s) URLs: kept as-is in degrade mode, uploaded in upload mode;
//! - outcomes and warnings are collected for the conversion report (I-03).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;

use crate::error::{Error, Result};

/// Transport abstraction: core never does network IO itself (PRD 3.1).
/// Implemented by wxwright-mp (fetch + material upload).
pub trait ImageTransport: Send + Sync {
    fn fetch(&self, url: &str) -> Result<Vec<u8>>;
    /// Returns (media_id, mmbiz_url).
    fn upload_material(&self, bytes: Vec<u8>, filename: &str) -> Result<(String, String)>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    /// Local files become base64 data URIs (self-contained HTML export).
    Inline,
    /// Upload everything possible to mmbiz (requires credentials).
    Upload,
    /// Keep http(s) URLs; local files are inlined with a warning.
    Keep,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageOutcome {
    pub source: String,
    pub final_src: String,
    pub data_w: Option<u32>,
    pub data_ratio: Option<String>,
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// True when the image ended up inlined as base64 (clipboard-unsafe).
    pub inlined: bool,
    /// True when the image is now a mmbiz.qpic.cn URL.
    pub mmbiz: bool,
}

pub struct Resolved {
    pub src: String,
    pub data_w: Option<u32>,
    pub data_ratio: Option<String>,
    pub failed: bool,
}

pub struct ImgPipeline {
    mode: ImageMode,
    base_dir: Option<PathBuf>,
    transport: Option<Arc<dyn ImageTransport>>,
    outcomes: RefCell<Vec<ImageOutcome>>,
}

impl ImgPipeline {
    pub fn new(
        mode: ImageMode,
        base_dir: Option<PathBuf>,
        transport: Option<Arc<dyn ImageTransport>>,
    ) -> Self {
        ImgPipeline {
            mode,
            base_dir,
            transport,
            outcomes: RefCell::new(Vec::new()),
        }
    }

    pub fn outcomes(&self) -> Vec<ImageOutcome> {
        self.outcomes.borrow().clone()
    }

    /// Resolve one image source into a final src + dimension metadata.
    pub fn resolve(&self, src: &str) -> Resolved {
        let outcome = self.resolve_outcome(src);
        let failed = outcome
            .warning
            .as_ref()
            .map(|w| w.contains("unavailable") || w.contains("无法加载"))
            .unwrap_or(false)
            && outcome.final_src.is_empty();
        Resolved {
            src: if failed {
                String::new()
            } else {
                outcome.final_src.clone()
            },
            data_w: outcome.data_w,
            data_ratio: outcome.data_ratio,
            failed,
        }
    }

    fn record(&self, o: ImageOutcome) {
        self.outcomes.borrow_mut().push(o);
    }

    fn resolve_outcome(&self, src: &str) -> ImageOutcome {
        let mut o = ImageOutcome {
            source: src.to_string(),
            final_src: String::new(),
            data_w: None,
            data_ratio: None,
            media_id: None,
            warning: None,
            inlined: false,
            mmbiz: false,
        };

        if src.starts_with("data:") {
            match dims_from_data_uri(src) {
                Ok((w, h)) => {
                    o.data_w = Some(w);
                    o.data_ratio = Some(ratio(h, w));
                }
                Err(e) => o.warning = Some(e),
            }
            o.final_src = src.to_string();
            o.inlined = true;
            o.warning = Some(o.warning.clone().unwrap_or_else(|| {
                "base64 inline image: paste-hostile, upload to mmbiz for reliable rendering".into()
            }));
            self.record(o.clone());
            return o;
        }

        let lower = src.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            if self.mode == ImageMode::Upload {
                if let Some(t) = &self.transport {
                    match t
                        .fetch(src)
                        .and_then(|bytes| upload_bytes(t.as_ref(), bytes, src))
                    {
                        Ok((media_id, url)) => {
                            o.final_src = url.clone();
                            o.media_id = Some(media_id);
                            o.mmbiz = url.contains("mmbiz.qpic.cn");
                        }
                        Err(e) => {
                            o.final_src = src.to_string();
                            o.warning = Some(format!("upload failed, kept original URL: {}", e));
                        }
                    }
                } else {
                    o.final_src = src.to_string();
                    o.warning = Some(
                        "no credentials configured, kept original URL (hotlink may fail)".into(),
                    );
                }
            } else {
                o.final_src = src.to_string();
                o.warning = Some("external URL image: WeChat may block or expire it; upload to mmbiz for zero-distortion".into());
            }
            self.record(o.clone());
            return o;
        }

        // Local file (absolute or relative to the markdown file).
        let path = match &self.base_dir {
            Some(base) => base.join(src),
            None => PathBuf::from(src),
        };
        let path = PathBuf::from(dunce_normal(&path));
        match std::fs::read(&path) {
            Ok(bytes) => {
                match dims_from_bytes(&bytes) {
                    Ok((w, h)) => {
                        o.data_w = Some(w);
                        o.data_ratio = Some(ratio(h, w));
                    }
                    Err(e) => o.warning = Some(format!("cannot read dimensions: {}", e)),
                }
                if self.mode == ImageMode::Upload {
                    if let Some(t) = &self.transport {
                        let fname = path
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| "image.png".into());
                        let for_upload = bytes.clone();
                        match upload_bytes(t.as_ref(), for_upload, &fname) {
                            Ok((media_id, url)) => {
                                o.final_src = url;
                                o.media_id = Some(media_id);
                                o.mmbiz = true;
                            }
                            Err(e) => {
                                o.final_src = inline_data_uri(&path, bytes);
                                o.inlined = true;
                                o.warning =
                                    Some(format!("upload failed, inlined as base64: {}", e));
                            }
                        }
                    } else {
                        o.final_src = inline_data_uri(&path, bytes);
                        o.inlined = true;
                        o.warning = Some("no credentials: inlined as base64 (clipboard-unsafe, file export only)".into());
                    }
                } else {
                    o.final_src = inline_data_uri(&path, bytes);
                    o.inlined = true;
                }
            }
            Err(e) => {
                o.warning = Some(format!("image unavailable: {} ({})", path.display(), e));
            }
        }
        self.record(o.clone());
        o
    }
}

fn upload_bytes(t: &dyn ImageTransport, bytes: Vec<u8>, name: &str) -> Result<(String, String)> {
    t.upload_material(bytes, name)
        .map_err(|e| Error::Upload(e.to_string()))
}

fn dunce_normal(p: &Path) -> String {
    // Avoid the \\?\ prefix on Windows for friendlier error messages.
    p.to_string_lossy().to_string()
}

fn ratio(h: u32, w: u32) -> String {
    if w == 0 {
        return "1".into();
    }
    let r = h as f64 / w as f64;
    let s = format!("{:.4}", r);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn mime_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn inline_data_uri(path: &Path, bytes: Vec<u8>) -> String {
    let mime = mime_of(path);
    format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

pub fn dims_from_bytes(bytes: &[u8]) -> Result<(u32, u32)> {
    let cursor = std::io::Cursor::new(bytes);
    let reader = image::ImageReader::new(cursor)
        .with_guessed_format()
        .map_err(|e| Error::Image(e.to_string()))?;
    reader
        .into_dimensions()
        .map_err(|e| Error::Image(e.to_string()))
}

pub fn dims_from_path(path: &Path) -> Result<(u32, u32)> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| Error::Image(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| Error::Image(e.to_string()))?;
    reader
        .into_dimensions()
        .map_err(|e| Error::Image(e.to_string()))
}

fn dims_from_data_uri(src: &str) -> std::result::Result<(u32, u32), String> {
    let b64 = src
        .split(";base64,")
        .nth(1)
        .ok_or("data URI without base64 payload")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("invalid base64: {}", e))?;
    dims_from_bytes(&bytes).map_err(|e| e.to_string())
}
