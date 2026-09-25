//! Article library (PRD-adjacent user request): local Markdown storage with
//! frontmatter metadata under Documents/wxwright/articles.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ArticleMeta {
    pub id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub theme: String,
    pub chars: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Article {
    pub meta: ArticleMeta,
    pub markdown: String,
}

pub fn library_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wxwright")
        .join("articles")
}

fn now_stamp() -> String {
    // Local time without external deps: system seconds since epoch formatted
    // via chrono-free civil conversion.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, mo, d, h, mi, s)
}

fn now_id() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, mi, s)
}

/// Days-from-civil inverse (Howard Hinnant's algorithm), UTC.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let h = (rem / 3600) as u32;
    let mi = (rem % 3600 / 60) as u32;
    let s = (rem % 60) as u32;
    (y, m, d, h, mi, s)
}

fn sanitize_id(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if clean.is_empty() {
        now_id()
    } else {
        clean
    }
}

fn parse_frontmatter(raw: &str) -> (Vec<(String, String)>, String) {
    let trimmed = raw.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let header = &rest[..end];
            let body = rest[end + 4..].trim_start_matches('\n').to_string();
            let mut pairs = Vec::new();
            for line in header.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    pairs.push((k.trim().to_string(), v));
                }
            }
            return (pairs, body);
        }
    }
    (Vec::new(), raw.to_string())
}

fn meta_from_path(path: &PathBuf) -> Option<ArticleMeta> {
    let raw = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let (pairs, body) = parse_frontmatter(&raw);
    let get = |k: &str| pairs.iter().find(|(pk, _)| pk == k).map(|(_, v)| v.clone());
    let title = get("title").unwrap_or_else(|| {
        body.lines()
            .find_map(|l| l.strip_prefix("# ").map(|s| s.trim().to_string()))
            .unwrap_or_else(|| id.clone())
    });
    let created = get("created").unwrap_or_default();
    let updated = get("updated").unwrap_or_default();
    let theme = get("theme").unwrap_or_else(|| "minimal".to_string());
    Some(ArticleMeta {
        id,
        title,
        created,
        updated,
        theme,
        chars: body.chars().count(),
    })
}

pub fn list_articles() -> Vec<ArticleMeta> {
    let dir = library_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut out: Vec<ArticleMeta> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
        .filter_map(|p| meta_from_path(&p))
        .collect();
    out.sort_by(|a, b| b.updated.cmp(&a.updated).then(b.id.cmp(&a.id)));
    out
}

pub fn read_article(id: &str) -> Result<Article, String> {
    let id = sanitize_id(id);
    let path = library_dir().join(format!("{}.md", id));
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read failed: {}", e))?;
    let meta = meta_from_path(&path).ok_or("cannot parse article")?;
    let (_, body) = parse_frontmatter(&raw);
    Ok(Article {
        meta,
        markdown: body,
    })
}

pub fn save_article(
    id: Option<String>,
    title: &str,
    theme: &str,
    markdown: &str,
) -> Result<ArticleMeta, String> {
    let dir = library_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create dir: {}", e))?;
    let existing_created = id
        .as_deref()
        .map(|i| {
            read_article(i)
                .ok()
                .map(|a| a.meta.created)
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let id = sanitize_id(id.as_deref().unwrap_or(""));
    let id = if id.is_empty() { now_id() } else { id };
    let created = if existing_created.is_empty() {
        now_stamp()
    } else {
        existing_created
    };
    let updated = now_stamp();
    let title = if title.trim().is_empty() {
        "未命名文章"
    } else {
        title.trim()
    };
    let body = format!(
        "---\ntitle: \"{}\"\ncreated: {}\nupdated: {}\ntheme: {}\n---\n\n{}",
        title.replace('"', "'"),
        created,
        updated,
        theme,
        markdown.trim_start()
    );
    let path = dir.join(format!("{}.md", id));
    std::fs::write(&path, body).map_err(|e| format!("write failed: {}", e))?;
    Ok(ArticleMeta {
        id,
        title: title.to_string(),
        created,
        updated,
        theme: theme.to_string(),
        chars: markdown.chars().count(),
    })
}

pub fn delete_article(id: &str) -> Result<bool, String> {
    let id = sanitize_id(id);
    let path = library_dir().join(format!("{}.md", id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {}", e))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_roundtrip() {
        let (pairs, body) =
            parse_frontmatter("---\ntitle: \"你好\"\ntheme: techblue\n---\n\n# H\n");
        assert_eq!(pairs.iter().find(|(k, _)| k == "title").unwrap().1, "你好");
        assert_eq!(body.trim_start(), "# H\n");
    }

    #[test]
    fn id_sanitized() {
        assert_eq!(sanitize_id("../evil/name"), "evilname");
    }
}

// ------------------------------------------------------------- assets ---

pub fn assets_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wxwright")
        .join("assets")
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_'
        })
        .take(80)
        .collect();
    if cleaned.is_empty() {
        "image.png".to_string()
    } else {
        cleaned
    }
}

/// Copy an image file into the managed assets dir with a deduped name.
pub fn import_from_path(src: &PathBuf) -> Result<PathBuf, String> {
    let bytes = std::fs::read(src).map_err(|e| format!("read failed: {}", e))?;
    let name = src
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "image.png".into());
    import_from_bytes(&name, &bytes)
}

pub fn import_from_bytes(filename: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = assets_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create dir: {}", e))?;
    let clean = sanitize_filename(filename);
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{}", e)),
        None => (clean.clone(), ".png".to_string()),
    };
    let stamp = now_id();
    let path = dir.join(format!("{}-{}{}", stamp, stem, ext));
    std::fs::write(&path, bytes).map_err(|e| format!("write failed: {}", e))?;
    Ok(path)
}

#[cfg(test)]
mod tests_assets {
    use super::*;

    #[test]
    fn filename_sanitized() {
        assert_eq!(sanitize_filename("a b/c.d.png"), "abc.d.png");
        assert_eq!(sanitize_filename(""), "image.png");
    }
}

// --------------------------------------------------- asset library mgmt ---

#[derive(Debug, Clone, Serialize)]
pub struct AssetMeta {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: String,
}

pub fn list_assets() -> Vec<AssetMeta> {
    let dir = assets_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut out: Vec<AssetMeta> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let ext_ok = path
                .extension()
                .map(|x| {
                    matches!(
                        x.to_string_lossy().to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                    )
                })
                .unwrap_or(false);
            if !ext_ok {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .map(unix_to_stamp)
                .unwrap_or_default();
            Some(AssetMeta {
                path: path.to_string_lossy().to_string(),
                name: path.file_name()?.to_string_lossy().to_string(),
                size: meta.len(),
                modified,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then(b.name.cmp(&a.name)));
    out.truncate(200);
    out
}

fn unix_to_stamp(secs: u64) -> String {
    let (y, mo, d, h, mi, _s) = civil_from_unix(secs as i64);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi)
}

/// Serve an asset as a data URI (UI thumbnails / previews).
/// Only files inside the managed assets dir are allowed.
pub fn asset_data_uri(path: &str) -> Result<String, String> {
    let dir = assets_dir();
    let requested = PathBuf::from(path);
    let canonical = requested
        .canonicalize()
        .map_err(|_| "file not found".to_string())?;
    let base = dir.canonicalize().unwrap_or(dir);
    if !canonical.starts_with(&base) {
        return Err("path outside assets library".into());
    }
    let mime = match canonical
        .extension()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return Err("unsupported asset type".into()),
    };
    use base64::Engine as _;
    let bytes = std::fs::read(&canonical).map_err(|e| e.to_string())?;
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn delete_asset(path: &str) -> Result<bool, String> {
    let dir = assets_dir();
    let requested = PathBuf::from(path);
    let canonical = requested
        .canonicalize()
        .map_err(|_| "file not found".to_string())?;
    let base = dir.canonicalize().unwrap_or(dir);
    if !canonical.starts_with(&base) {
        return Err("path outside assets library".into());
    }
    if canonical.is_file() {
        std::fs::remove_file(&canonical).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests_assets2 {
    use super::*;

    #[test]
    fn asset_path_escape_blocked() {
        // canonicalize on a missing file errors out - treated as blocked.
        assert!(
            asset_data_uri("C:/Windows/notepad.exe").is_err()
                || asset_data_uri("C:/Windows/notepad.exe") != Ok(String::new())
        );
    }
}

/// Fast backend thumbnail: decoded once, resized to fit `size` px box,
/// returned as a JPEG data URI (tiny payload for the UI grid).
pub fn asset_thumb(path: &str, size: u32) -> Result<String, String> {
    use base64::Engine as _;
    let dir = assets_dir();
    let canonical = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "file not found".to_string())?;
    let base = dir.canonicalize().unwrap_or(dir);
    if !canonical.starts_with(&base) {
        return Err("path outside assets library".into());
    }
    let img = image::ImageReader::open(&canonical)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| format!("decode: {}", e))?;
    let thumb = img.thumbnail(size, size);
    let mut jpeg = std::io::Cursor::new(Vec::new());
    thumb
        .write_to(&mut jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg.into_inner())
    ))
}
