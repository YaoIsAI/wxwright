//! Platform registry: the "one engine, many dialects" seam (PRD §16).
//!
//! Each social platform gets a descriptor covering what the engine and the
//! GUI need to adapt: whether dialect rich text can be pasted at all, whether
//! the primary flow is an image-note set (Xiaohongshu), what preset canvas
//! sizes the poster/size studios should offer, and which export adapters are
//! actually wired. Descriptors are data, not behaviour — renderers and rule
//! tables stay with the dialect implementations that consume them.

use crate::ir::{Block, Inline, InlineKind};
use crate::validator;

/// One social-media platform the engine can target.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlatformSpec {
    /// Stable id used in settings/CLI (`wechat`, `xhs`, ...).
    pub id: &'static str,
    /// Chinese display name.
    pub name_zh: &'static str,
    /// English display name.
    pub name_en: &'static str,
    /// Dialect rich text (styled HTML) can be pasted into the platform's
    /// editor; false means the primary output is images and/or plain text.
    pub rich_text: bool,
    /// Primary flow is an image-note set (cover + content images).
    pub image_note: bool,
    /// A publishing API adapter is implemented (draft push or equivalent).
    pub api_publish: bool,
    /// Poster / size-studio presets: (label_zh, width, height).
    pub presets: &'static [(&'static str, u32, u32)],
    /// Honest capability note (surfaced in the GUI switcher tooltip).
    pub note: &'static str,
}

pub const WECHAT: PlatformSpec = PlatformSpec {
    id: "wechat",
    name_zh: "微信公众号",
    name_en: "WeChat MP",
    rich_text: true,
    image_note: false,
    api_publish: true,
    presets: &[
        ("头图 1080×460（2.35:1）", 1080, 460),
        ("次图 1080×1080（1:1）", 1080, 1080),
        ("小方图 500×500（1:1）", 500, 500),
        ("正文横图 1280×720（16:9）", 1280, 720),
        ("正文竖图 1080×1440（3:4）", 1080, 1440),
        ("贴图 900×383", 900, 383),
        ("贴图 383×383", 383, 383),
    ],
    note: "完整支持：方言富文本 + 草稿箱 API",
};

pub const XHS: PlatformSpec = PlatformSpec {
    id: "xhs",
    name_zh: "小红书",
    name_en: "Xiaohongshu",
    rich_text: false,
    image_note: true,
    api_publish: false,
    presets: &[
        ("封面 1080×1440（3:4）", 1080, 1440),
        ("正文图 1080×1440（3:4）", 1080, 1440),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "图片笔记路径：海报/图组导出 + 文案复制；官方发布器贴文",
};

pub const ZHIHU: PlatformSpec = PlatformSpec {
    id: "zhihu",
    name_zh: "知乎",
    name_en: "Zhihu",
    rich_text: true,
    image_note: false,
    api_publish: false,
    presets: &[("封面 1920×1080（16:9）", 1920, 1080)],
    note: "Markdown/富文本友好（出口适配待接）",
};

pub const META: PlatformSpec = PlatformSpec {
    id: "meta",
    name_zh: "Meta (Facebook/Instagram)",
    name_en: "Meta (Facebook/Instagram)",
    rich_text: false,
    image_note: true,
    api_publish: false,
    presets: &[
        ("Instagram 方图 1080×1080", 1080, 1080),
        ("Instagram 竖图 1080×1350（4:5）", 1080, 1350),
        ("Story 1080×1920（9:16）", 1080, 1920),
    ],
    note: "需 Graph API 资质（v2 出口适配）",
};

pub const X: PlatformSpec = PlatformSpec {
    id: "x",
    name_zh: "X (Twitter)",
    name_en: "X (Twitter)",
    rich_text: false,
    image_note: false,
    api_publish: false,
    presets: &[
        ("横图 1600×900（16:9）", 1600, 900),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "API v2 发文，280 字摘要策略（v2 出口适配）",
};

pub const LINKEDIN: PlatformSpec = PlatformSpec {
    id: "linkedin",
    name_zh: "LinkedIn",
    name_en: "LinkedIn",
    rich_text: false,
    image_note: false,
    api_publish: false,
    presets: &[
        ("横图 1200×627", 1200, 627),
        ("方图 1080×1080（1:1）", 1080, 1080),
    ],
    note: "文章 API 成熟但富文本有限（v2 出口适配）",
};

/// All platforms in switcher order (WeChat first: the default target).
pub fn list_platforms() -> Vec<PlatformSpec> {
    vec![
        WECHAT.clone(),
        XHS.clone(),
        ZHIHU.clone(),
        META.clone(),
        X.clone(),
        LINKEDIN.clone(),
    ]
}

/// Look a platform up by id; unknown ids fall back to WeChat so callers can
/// treat platform as a hint rather than a hard switch.
pub fn get_platform(id: &str) -> PlatformSpec {
    list_platforms()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap_or(WECHAT)
}


// ------------------------------------------------------- export adapters --

/// What artifact a platform's primary "copy" action produces. This is the
/// essential per-platform difference: WeChat pastes dialect rich text into
/// the MP editor; Xiaohongshu posts a plain-text caption plus an image set;
/// Zhihu accepts Markdown directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// Dialect HTML via the clipboard rich-text flavor (WeChat MP editor).
    RichTextDialect,
    /// Plain-text caption (Xiaohongshu note text, Meta/X/LinkedIn text).
    Caption,
    /// Raw Markdown (Zhihu and other Markdown-friendly hosts).
    Markdown,
}

pub fn export_kind(id: &str) -> ExportKind {
    match id {
        "wechat" => ExportKind::RichTextDialect,
        "xhs" => ExportKind::Caption,
        "zhihu" => ExportKind::Markdown,
        _ => ExportKind::Caption,
    }
}

/// Render the document as the platform's plain-text caption: structure is
/// linearised (headings become short lines, lists become bullets, links keep
/// only their text since most caption hosts do not autolink), images become
/// [图片] placeholders (they travel in the image set, not the text).
pub fn render_caption(doc: &[Block], title: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
        out.push_str(t);
        out.push_str("\n\n");
    }
    push_blocks(doc, &mut out);
    while out.contains("\n\n\n\n") {
        out = out.replace("\n\n\n\n", "\n\n\n");
    }
    out.trim().to_string()
}

fn push_blocks(blocks: &[Block], out: &mut String) {
    for b in blocks {
        push_block(b, out);
    }
}

fn push_block(b: &Block, out: &mut String) {
    match b {
        Block::Heading { level, inlines, .. } => {
            if *level == 1 {
                // H1 usually duplicates the caption title; keep it out.
                return;
            }
            push_inlines(inlines, out);
            out.push_str("\n\n");
        }
        Block::Paragraph { inlines } => {
            push_inlines(inlines, out);
            out.push_str("\n\n");
        }
        Block::List { ordered, items, .. } => {
            for (i, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", i + 1)
                } else {
                    "• ".to_string()
                };
                out.push_str(&bullet);
                match item.checked {
                    Some(true) => out.push_str("[x] "),
                    Some(false) => out.push_str("[ ] "),
                    None => {}
                }
                push_blocks(&item.blocks, out);
                out.push('\n');
            }
            out.push('\n');
        }
        Block::Blockquote { blocks } | Block::Card { blocks, .. } => push_blocks(blocks, out),
        Block::Table { header, rows, .. } => {
            for cell in header {
                push_inlines(cell, out);
                out.push_str(" | ");
            }
            out.push('\n');
            for row in rows {
                for cell in row {
                    push_inlines(cell, out);
                    out.push_str(" | ");
                }
                out.push('\n');
            }
            out.push('\n');
        }
        Block::Image(img) => {
            out.push_str(&format!("[图片:{}]\n", img.alt));
        }
        Block::Figure { image, caption } => {
            out.push_str(&format!("[图片:{}] ", image.alt));
            push_inlines(caption, out);
            out.push_str("\n\n");
        }
        Block::Code { code, .. } => {
            out.push_str(code.trim());
            out.push_str("\n\n");
        }
        Block::Formula { latex } => {
            out.push_str(latex);
            out.push_str("\n\n");
        }
        Block::Rule => out.push_str("\n---\n\n"),
        Block::Toc => {}
        Block::RawHtml { html } => out.push_str(html.trim()),
        Block::SvgEmbed { .. } | Block::Chart { .. } => {
            out.push_str("[互动组件在公众号版本中]\n");
        }
    }
}

fn push_inlines(inlines: &[Inline], out: &mut String) {
    for i in inlines {
        push_inline(i, out);
    }
}

fn push_inline(i: &Inline, out: &mut String) {
    match i {
        Inline::Text(t) => out.push_str(t),
        Inline::Code(c) => out.push_str(c),
        Inline::Math { latex, .. } => out.push_str(latex),
        Inline::Image(img) => out.push_str(&format!("[图片:{}]", img.alt)),
        Inline::Break => out.push('\n'),
        Inline::RawHtml(h) => out.push_str(h),
        Inline::Styled { children, .. } => push_inlines(children, out),
    }
}

// -------------------------------------------------------- platform rules --

/// Platform-specific compliance rules (PRD §16: each platform brings its own
/// rule table). Xiaohongshu notes have hard length limits and conventions.
pub fn validate_platform_caption(
    platform: &str,
    title: &str,
    caption: &str,
    images: usize,
) -> Vec<crate::validator::Violation> {
    if platform != "xhs" {
        return Vec::new();
    }
    let mut v = Vec::new();
    if title.chars().count() > 20 {
        v.push(crate::validator::Violation {
            rule_id: "XHS-1".into(),
            severity: "warn".into(),
            message: format!(
                "title is {} chars; the Xiaohongshu title field caps at 20",
                title.chars().count()
            ),
            node: truncate_node(title),
            fixable: false,
        });
    }
    let body_chars = caption.chars().count();
    if body_chars > 1000 {
        v.push(crate::validator::Violation {
            rule_id: "XHS-2".into(),
            severity: "block".into(),
            message: format!("caption is {body_chars} chars; Xiaohongshu notes cap at 1000"),
            node: truncate_node(caption),
            fixable: false,
        });
    }
    if caption.matches('#').count() < 2 {
        v.push(crate::validator::Violation {
            rule_id: "XHS-3".into(),
            severity: "warn".into(),
            message: "no #hashtag# found; Xiaohongshu relies on hashtags for reach".into(),
            node: String::new(),
            fixable: false,
        });
    }
    if images == 0 {
        v.push(crate::validator::Violation {
            rule_id: "XHS-4".into(),
            severity: "warn".into(),
            message: "image note without images; export a cover/content image set".into(),
            node: String::new(),
            fixable: false,
        });
    } else if images > 9 {
        v.push(crate::validator::Violation {
            rule_id: "XHS-5".into(),
            severity: "warn".into(),
            message: format!("{images} images; a Xiaohongshu note carries at most 9"),
            node: String::new(),
            fixable: false,
        });
    }
    v
}

fn truncate_node(s: &str) -> String {
    s.chars().take(60).collect()
}

// ------------------------------------------------------ plain article ----

/// Minimal typographic HTML for Markdown-friendly hosts (Zhihu): real
/// headings/lists/tables instead of the MP dialect. `resolved` carries the
/// image pipeline's final_src per ImageRef in document order (from a
/// PipelineResult), so local/asset images show up inlined like everywhere
/// else.
pub fn render_plain_html(doc: &[Block], resolved: &[String]) -> String {
    let mut r = PlainRenderer {
        resolved,
        idx: 0,
        out: String::new(),
    };
    r.blocks(doc);
    r.out
}

struct PlainRenderer<'a> {
    resolved: &'a [String],
    idx: usize,
    out: String,
}

impl<'a> PlainRenderer<'a> {
    fn next_img(&mut self, alt: &str) -> String {
        let src = self.resolved.get(self.idx).cloned().unwrap_or_default();
        self.idx += 1;
        if src.is_empty() {
            format!("[图片:{}]", crate::htmlutil::escape_text(alt))
        } else {
            format!(
                "<img src=\"{}\" alt=\"{}\" style=\"max-width: 100%; border-radius: 4px;\" />",
                crate::htmlutil::escape_attr(&src),
                crate::htmlutil::escape_attr(alt)
            )
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for b in blocks {
            self.block(b);
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        for i in inlines {
            self.inline(i);
        }
    }

    fn inline(&mut self, i: &Inline) {
        match i {
            Inline::Text(t) => self.out.push_str(&crate::htmlutil::escape_text(t)),
            Inline::Code(c) => {
                self.out.push_str(&format!(
                    "<code style=\"background: #F2F4F7; padding: 1px 5px; border-radius: 3px;\">{}</code>",
                    crate::htmlutil::escape_text(c)
                ));
            }
            Inline::Math { latex, .. } => self.out.push_str(&crate::htmlutil::escape_text(latex)),
            Inline::Image(img) => {
                let tag = self.next_img(&img.alt);
                self.out.push_str(&tag);
            }
            Inline::Break => self.out.push_str("<br/>"),
            Inline::RawHtml(h) => self.out.push_str(h),
            Inline::Styled { kind, children } => match kind {
                InlineKind::Strong => {
                    self.out.push_str("<strong>");
                    self.inlines(children);
                    self.out.push_str("</strong>");
                }
                InlineKind::Emphasis => {
                    self.out.push_str("<em>");
                    self.inlines(children);
                    self.out.push_str("</em>");
                }
                InlineKind::Strike => {
                    self.out.push_str("<del>");
                    self.inlines(children);
                    self.out.push_str("</del>");
                }
                InlineKind::Link { url, .. } => {
                    self.out.push_str(&format!(
                        "<a href=\"{}\">",
                        crate::htmlutil::escape_attr(url)
                    ));
                    self.inlines(children);
                    self.out.push_str("</a>");
                }
            },
        }
    }

    fn block(&mut self, b: &Block) {
        match b {
            Block::Heading { level, inlines, .. } => {
                let h = (*level).clamp(1, 6);
                self.out.push_str(&format!("<h{h}>"));
                self.inlines(inlines);
                self.out.push_str(&format!("</h{h}>"));
            }
            Block::Paragraph { inlines } => {
                self.out.push_str("<p>");
                self.inlines(inlines);
                self.out.push_str("</p>");
            }
            Block::List { ordered, items, .. } => {
                let tag = if *ordered { "ol" } else { "ul" };
                self.out.push_str(&format!("<{tag}>"));
                for item in items {
                    self.out.push_str("<li>");
                    self.blocks(&item.blocks);
                    self.out.push_str("</li>");
                }
                self.out.push_str(&format!("</{tag}>"));
            }
            Block::Blockquote { blocks } => {
                self.out.push_str("<blockquote>");
                self.blocks(blocks);
                self.out.push_str("</blockquote>");
            }
            Block::Card { blocks, .. } => self.blocks(blocks),
            Block::Table { header, rows, .. } => {
                self.out
                    .push_str("<table border=\"1\" cellspacing=\"0\" cellpadding=\"6\">");
                self.out.push_str("<tr>");
                for cell in header {
                    self.out.push_str("<th>");
                    self.inlines(cell);
                    self.out.push_str("</th>");
                }
                self.out.push_str("</tr>");
                for row in rows {
                    self.out.push_str("<tr>");
                    for cell in row {
                        self.out.push_str("<td>");
                        self.inlines(cell);
                        self.out.push_str("</td>");
                    }
                    self.out.push_str("</tr>");
                }
                self.out.push_str("</table>");
            }
            Block::Image(img) => {
                let tag = self.next_img(&img.alt);
                self.out.push_str(&tag);
            }
            Block::Figure { image, caption } => {
                self.out.push_str("<figure>");
                let tag = self.next_img(&image.alt);
                self.out.push_str(&tag);
                self.out.push_str("<figcaption>");
                self.inlines(caption);
                self.out.push_str("</figcaption></figure>");
            }
            Block::Code { code, .. } => {
                self.out.push_str(&format!(
                    "<pre style=\"background: #F6F8FA; padding: 12px; overflow-x: auto;\"><code>{}</code></pre>",
                    crate::htmlutil::escape_text(code)
                ));
            }
            Block::Formula { latex } => {
                self.out.push_str(&format!("<p><code>{}</code></p>", crate::htmlutil::escape_text(latex)));
            }
            Block::Rule => self.out.push_str("<hr/>"),
            Block::Toc => {}
            Block::RawHtml { html } => self.out.push_str(html),
            Block::SvgEmbed { html } => self.out.push_str(html),
            Block::Chart { spec } => {
                self.out
                    .push_str(&crate::render::render_chart(spec));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

#[test]
fn plain_renderer_emits_real_typography_with_resolved_images() {
    let doc = crate::parser::parse_markdown(
        "# 标题\n\n正文 **加粗** 与 ![截图](assets/a.png)\n\n- 甲\n- 乙\n",
    );
    let html = render_plain_html(&doc, &["data:image/png;base64,QQ==".to_string()]);
    assert!(html.contains("<h1>标题</h1>"));
    assert!(html.contains("<strong>加粗</strong>"));
    assert!(html.contains("src=\"data:image/png;base64,QQ==\""));
    assert!(html.contains("<ul>") && html.contains("甲") && html.contains("乙"));
}

#[test]
fn caption_renderer_linearises_structure() {
    let doc = crate::parser::parse_markdown(
        "# 我的标题\n\n正文**加粗**和[链接文字](https://x.y)。\n\n- 甲\n- 乙\n",
    );
    let cap = render_caption(&doc, Some("我的标题"));
    assert!(cap.starts_with("我的标题\n\n正文加粗和链接文字。"), "got: {cap}");
    assert!(cap.contains("• 甲"));
    assert!(!cap.contains("https://x.y"), "links keep text only");
}

#[test]
fn xhs_rules_fire_on_limits_and_conventions() {
    let long_title = "一个超过二十个字的小红书标题肯定是会被截断的哦";
    let v = validate_platform_caption("xhs", long_title, "正文 #标签#", 0);
    assert!(v.iter().any(|x| x.rule_id == "XHS-1"));
    assert!(v.iter().any(|x| x.rule_id == "XHS-4"));
    let v2 = validate_platform_caption("xhs", "短标题", "没有话题标签的正文", 3);
    assert!(v2.iter().any(|x| x.rule_id == "XHS-3"));
    let big = "字".repeat(1001);
    let v3 = validate_platform_caption("xhs", "短标题", &big, 3);
    assert!(v3.iter().any(|x| x.rule_id == "XHS-2" && x.is_block()));
    assert!(validate_platform_caption("wechat", "t", "c", 1).is_empty());
}

    #[test]
    fn ids_are_unique_and_wechat_is_default() {
        let all = list_platforms();
        assert!(all.len() >= 5);
        let mut ids: Vec<_> = all.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        assert_eq!(get_platform("wechat").id, "wechat");
        assert_eq!(get_platform("nonexistent").id, "wechat");
        assert_eq!(get_platform("xhs").name_zh, "小红书");
    }

    #[test]
    fn xhs_is_image_note_without_rich_text() {
        assert!(XHS.image_note && !XHS.rich_text);
        assert!(WECHAT.rich_text && !WECHAT.image_note);
    }

    #[test]
    fn every_platform_has_at_least_one_preset() {
        for p in list_platforms() {
            assert!(!p.presets.is_empty(), "{} has no presets", p.id);
        }
    }
}
