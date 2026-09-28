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

impl ImageOutcome {
    /// True when this image cannot survive a paste into the MP editor or a
    /// draft push (PRD I-03): the payload must reference a mmbiz URL or an
    /// https URL WeChat can fetch. base64 payloads, plain-`http` hosts and
    /// images that failed to load all break after paste.
    ///
    /// One predicate shared by `wxwright copy`, the GUI copy button and the
    /// GUI draft push - previously each site filtered slightly differently and
    /// the draft path had no gate at all.
    pub fn paste_hostile(&self) -> bool {
        if self.mmbiz {
            return false;
        }
        if self.inlined || self.source.starts_with("data:") {
            return true;
        }
        if self.final_src.is_empty() {
            // Read/upload failed: the renderer emits a placeholder, which is
            // not something the reader should ever see in a published article.
            return true;
        }
        self.final_src.to_ascii_lowercase().starts_with("http://")
    }
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
        // Keep this a PathBuf. The old code round-tripped it through String to
        // "avoid the \\?\ prefix", which never stripped the prefix and did
        // substitute U+FFFD for a non-UTF-8 file name on Unix - making the file
        // unopenable.
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

// ------------------------------------------------------------ platform fit ---

/// Per-platform target for article images. Rich-text platforms get a width
/// clamp only (the MP editor scales down by itself); image-note platforms
/// also get their primary cover shape as a centre-crop, so a 3:2 landscape
/// photograph lands on Xiaohongshu as a real 3:4 instead of a broken frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageProfile {
    pub max_width: u32,
    /// (w, h) shape to centre-crop to; `None` keeps the source aspect.
    pub cover_aspect: Option<(u32, u32)>,
}

/// One image fitted to a profile.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FitResult {
    /// The fitted image. When nothing had to change this is the untouched
    /// input buffer, so compliant images never pay a re-encode quality tax.
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// True when the aspect changed (a centre-crop happened).
    pub cropped: bool,
    /// True when the width was clamped down.
    pub resized: bool,
}

/// Centre-crop to the profile's aspect (cover semantics) and clamp the width
/// to `max_width`, never upscaling. PNG stays PNG so transparency survives;
/// everything else re-encodes as JPEG q85. An image that already fits comes
/// back byte-for-byte untouched - compliant inputs pay no quality tax.
/// GIF returns an error rather than silently dropping animation frames; the
/// caller decides whether to pass the original through.
pub fn fit_to_profile(bytes: &[u8], profile: &ImageProfile) -> Result<FitResult> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Image(e.to_string()))?;
    let src_format = reader.format();
    if src_format == Some(image::ImageFormat::Gif) {
        return Err(Error::Image(
            "gif cannot be fitted (animation would be lost); pass it through unfitted".into(),
        ));
    }
    let mut img = reader.decode().map_err(|e| Error::Image(e.to_string()))?;

    let (w, h) = (img.width(), img.height());
    let mut cropped = false;
    if let Some((aw, ah)) = profile.cover_aspect {
        let src_ratio = w as f64 / h as f64;
        let target = aw as f64 / ah as f64;
        if (src_ratio - target).abs() > 0.001 {
            cropped = true;
            if src_ratio > target {
                // Wider than the target: trim the left/right flanks.
                let nw = ((h as f64) * target).round().max(1.0) as u32;
                let x = (w - nw) / 2;
                img = img.crop_imm(x, 0, nw, h);
            } else {
                // Taller than the target: trim top/bottom.
                let nh = ((w as f64) / target).round().max(1.0) as u32;
                let y = (h - nh) / 2;
                img = img.crop_imm(0, y, w, nh);
            }
        }
    }
    let mut resized = false;
    let (w, h) = (img.width(), img.height());
    if w > profile.max_width {
        resized = true;
        let scale = profile.max_width as f64 / w as f64;
        let nh = ((h as f64) * scale).round().max(1.0) as u32;
        img = img.resize_exact(profile.max_width, nh, image::imageops::FilterType::Lanczos3);
    }
    let (w, h) = (img.width(), img.height());
    if !cropped && !resized {
        return Ok(FitResult {
            bytes: bytes.to_vec(),
            width: w,
            height: h,
            cropped,
            resized,
        });
    }
    if w == 0 || h == 0 {
        return Err(Error::Image("fitted image collapsed to zero size".into()));
    }

    let mut out = Vec::new();
    let write_err = |e: image::ImageError| Error::Image(e.to_string());
    if src_format == Some(image::ImageFormat::Png) {
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(write_err)?;
    } else {
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        img.to_rgb8().write_with_encoder(enc).map_err(write_err)?;
    }
    Ok(FitResult {
        bytes: out,
        width: w,
        height: h,
        cropped,
        resized,
    })
}

#[cfg(test)]
mod fit_tests {
    use super::*;

    fn solid_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, _| {
            if x < 4 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 128, 255, 255])
            }
        });
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn landscape_is_cropped_to_the_target_shape() {
        // 1600x900 (16:9) -> 3:4 target: crop flanks to 607x900... the exact
        // crop keeps the full height and narrows the width; output must be
        // portrait with the target ratio.
        let bytes = solid_png(1600, 900);
        let p = ImageProfile {
            max_width: 1080,
            cover_aspect: Some((3, 4)),
        };
        let r = fit_to_profile(&bytes, &p).expect("fit ok");
        assert!(r.cropped && !r.resized, "crop only: {:?}", r);
        let ratio = r.width as f64 / r.height as f64;
        assert!(
            (ratio - 0.75).abs() < 0.01,
            "output must be 3:4, got {}x{}",
            r.width,
            r.height
        );
    }

    #[test]
    fn oversized_width_is_clamped_never_upscaled() {
        // 2000x1500 (4:3), target 1:1 max 1080: crop to 1500x1500, then
        // clamp to 1080x1080.
        let bytes = solid_png(2000, 1500);
        let p = ImageProfile {
            max_width: 1080,
            cover_aspect: Some((1, 1)),
        };
        let r = fit_to_profile(&bytes, &p).expect("fit ok");
        assert!(r.cropped && r.resized);
        assert_eq!((r.width, r.height), (1080, 1080));

        // Below the width clamp nothing is scaled (never upscale), but the
        // cover shape still applies: 800x600 crops to a centred 600x600.
        let small = solid_png(800, 600);
        let r2 = fit_to_profile(&small, &p).expect("fit ok");
        assert!(r2.cropped && !r2.resized);
        assert_eq!((r2.width, r2.height), (600, 600));
    }

    #[test]
    fn compliant_images_pass_through_byte_for_byte() {
        let bytes = solid_png(1080, 1440);
        let p = ImageProfile {
            max_width: 1080,
            cover_aspect: Some((3, 4)),
        };
        let r = fit_to_profile(&bytes, &p).expect("fit ok");
        assert!(!r.cropped && !r.resized);
        // The pass-through returns the original buffer unchanged, so a
        // compliant image never pays a re-encode quality tax.
        assert_eq!(r.bytes, bytes);
        assert_eq!(r.width, 1080);
        assert_eq!(r.height, 1440);
    }

    #[test]
    fn aspect_only_clamp_keeps_the_source_shape() {
        let bytes = solid_png(2000, 900);
        let p = ImageProfile {
            max_width: 1080,
            cover_aspect: None,
        };
        let r = fit_to_profile(&bytes, &p).expect("fit ok");
        assert!(!r.cropped && r.resized);
        assert_eq!((r.width, r.height), (1080, 486));
    }
}
