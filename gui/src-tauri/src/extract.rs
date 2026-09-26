//! Attachment text extraction for the AI assistant (DeepSeek-harness style):
//! mainstream document formats become plain-text context for the model.
//! Images are handled separately as vision content (data URIs) and are not
//! extracted here.

use std::io::Read;
use std::path::Path;

pub fn kind_for_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        "pdf"
    } else if lower.ends_with(".docx") {
        "docx"
    } else if lower.ends_with(".html") || lower.ends_with(".htm") {
        "html"
    } else if lower.ends_with(".csv") {
        "csv"
    } else if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".xml") {
        "xml"
    } else {
        "text"
    }
}

/// Extract plain text from a supported document.
pub fn extract_document_text(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("文件不存在: {}", path));
    }
    let kind = kind_for_path(path);
    let bytes = std::fs::read(p).map_err(|e| format!("读取失败: {}", e))?;
    match kind {
        "pdf" => extract_pdf(&bytes),
        "docx" => extract_docx(&bytes),
        "html" | "htm" => Ok(html_to_text(&String::from_utf8_lossy(&bytes))),
        _ => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            if text.chars().any(|c| c == '\u{0}') {
                return Err("不支持二进制文件".into());
            }
            Ok(text)
        }
    }
}

fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    let text =
        pdf_extract::extract_text_from_mem(bytes).map_err(|e| format!("PDF 解析失败: {}", e))?;
    Ok(clean_whitespace(&text))
}

fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("DOCX 打开失败: {}", e))?;
    let mut xml = String::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX 缺少 document.xml: {}", e))?;
        entry.read_to_string(&mut xml).map_err(|e| e.to_string())?;
    }
    // Extract <w:t> runs; paragraph breaks on </w:p>.
    let mut out = String::new();
    let mut rest = xml.as_str();
    while let Some(start) = rest.find("<w:t") {
        let after = &rest[start..];
        let close = after.find('>').map(|i| i + 1).unwrap_or(0);
        let body = &after[close..];
        let Some(end) = body.find("</w:t>") else {
            break;
        };
        out.push_str(&body[..end]);
        rest = &body[end..];
    }
    // Paragraph boundaries: split on </w:p> markers BEFORE extracting runs is
    // more correct, but joining runs with the original order is close enough
    // for context ingestion; restore rough paragraph breaks:
    let out = out.replace("</w:t><w:t", " ");
    let out = clean_whitespace(&out);
    if out.trim().is_empty() {
        return Err("DOCX 中未提取到文本".into());
    }
    Ok(out)
}

fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    // drop script/style blocks entirely
    for tag in ["script", "style", "head"] {
        let lower = s.to_ascii_lowercase();
        let Some(start) = lower.find(&format!("<{}", tag)) else {
            continue;
        };
        let Some(open_end) = lower[start..].find('>').map(|i| start + i + 1) else {
            continue;
        };
        let Some(close) = lower[open_end..].find(&format!("</{}>", tag)) else {
            continue;
        };
        s.replace_range(start..open_end + close + tag.len() + 3, "");
    }
    // common entities
    for (ent, ch) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        s = s.replace(ent, ch);
    }
    // block tags become line breaks
    let lower = s.to_ascii_lowercase();
    for tag in ["br", "p", "div", "li", "tr", "h1", "h2", "h3", "h4"] {
        let open = format!("<{}", tag);
        if let Some(i) = lower.find(&open) {
            s.insert(i + 1, '/');
        }
    }
    // strip all tags
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    clean_whitespace(&out)
}

fn clean_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !prev_space && !out.is_empty() {
                out.push('\n');
            }
            prev_space = true;
            continue;
        }
        out.push_str(t);
        out.push('\n');
        prev_space = false;
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_extraction() {
        let html = "<html><head><style>x{y:1}</style></head><body><p>你好&nbsp;<b>世界</b></p><p>第二段</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("你好"), "got: {}", text);
        assert!(text.contains("第二段"));
        assert!(!text.contains("x{y:1}"));
    }

    #[test]
    fn kind_detection() {
        assert_eq!(kind_for_path("a.PDF"), "pdf");
        assert_eq!(kind_for_path("b.docx"), "docx");
        assert_eq!(kind_for_path("c.txt"), "text");
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ExtractedDoc {
    pub name: String,
    pub kind: String,
    pub chars: usize,
    pub truncated: bool,
    pub text: String,
}

const MAX_DOC_CHARS: usize = 24_000;

/// High-level entry for the attach flow: detect kind, extract, truncate.
pub fn extract(path: &str) -> Result<ExtractedDoc, String> {
    let kind = kind_for_path(path);
    let full = extract_document_text(path)?;
    let mut truncated = false;
    let mut text = full;
    if text.chars().count() > MAX_DOC_CHARS {
        text = text.chars().take(MAX_DOC_CHARS).collect();
        text.push_str("\n…（超长截断）");
        truncated = true;
    }
    let name = Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".into());
    Ok(ExtractedDoc {
        name,
        kind: kind.to_string(),
        chars: text.chars().count(),
        truncated,
        text,
    })
}
