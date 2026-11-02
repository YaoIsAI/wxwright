//! Platform registry: the "one engine, many dialects" seam (PRD §16).
//!
//! Each social platform gets a descriptor covering what the engine and the
//! GUI need to adapt: whether dialect rich text can be pasted at all, whether
//! the primary flow is an image-note set (Xiaohongshu), what preset canvas
//! sizes the poster/size studios should offer, and which export adapters are
//! actually wired. Descriptors are data, not behaviour — renderers and rule
//! tables stay with the dialect implementations that consume them.

use crate::ir::{Block, Inline, InlineKind};

/// What artifact a platform's primary "copy" action produces. This is the
/// essential per-platform difference: WeChat pastes dialect rich text into
/// the MP editor; Xiaohongshu posts a plain-text caption plus an image set;
/// Zhihu accepts Markdown directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    /// Dialect HTML via the clipboard rich-text flavor (WeChat MP editor).
    RichTextDialect,
    /// Plain-text caption (Xiaohongshu note text, Meta/X/LinkedIn text).
    Caption,
    /// Raw Markdown (Zhihu and other Markdown-friendly hosts).
    Markdown,
}

/// Severity of a caption rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionSeverity {
    Warn,
    Block,
}

/// Which text a violation points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionNode {
    Title,
    Caption,
    None,
}

/// What a caption rule measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionCheck {
    /// The title is longer than this many characters.
    TitleOver(usize),
    /// The caption is longer than this many characters.
    BodyOver(usize),
    /// The caption's platform-weighted length exceeds this budget. X counts
    /// most CJK and emoji as two characters, so a plain `chars().count()`
    /// would wrongly pass a 200-character Chinese post.
    WeightedOver(usize),
    /// The caption carries fewer than this many `#` markers.
    FewerHashtags(usize),
    /// No images are attached.
    NoImages,
    /// More images are attached than the platform accepts.
    TooManyImages(usize),
}

/// X/Twitter weighted length (twitter-text semantics): the Latin, punctuation
/// and common-symbol ranges count 1, everything else counts 2. URLs are not
/// special-cased here; the conservative count still catches an overlong post.
pub fn x_weighted_len(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            if cp <= 4351
                || (8192..=8205).contains(&cp)
                || (8208..=8223).contains(&cp)
                || (8242..=8247).contains(&cp)
            {
                1
            } else {
                2
            }
        })
        .sum()
}

/// A caption rule a platform declares.
///
/// These live in the descriptor on purpose: `validate_platform_caption` used to
/// open with `if platform != "xhs" { return vec![] }`, so the registry was data
/// for its capability flags but a `match` elsewhere for its behaviour. Now a
/// new platform declares its rules and the validator does not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptionRule {
    pub rule_id: &'static str,
    pub severity: CaptionSeverity,
    pub check: CaptionCheck,
    /// `{}` is replaced with the measured value when the check produces one.
    pub message: &'static str,
    pub node: CaptionNode,
}

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
    /// What this platform's primary export produces. Living here (rather than
    /// in a `match id` elsewhere) is what makes "descriptor is data" true for
    /// behaviour as well as for the capability flags.
    pub export_kind: ExportKind,
    /// Caption rules for this platform. Empty for platforms whose caption is
    /// not constrained.
    ///
    /// Skipped in serialisation: `platforms --json` describes the platform to a
    /// caller, it does not need the rule table dumped into the payload.
    #[serde(skip)]
    pub caption_rules: &'static [CaptionRule],
    /// Poster / size-studio presets: (label_zh, width, height).
    pub presets: &'static [(&'static str, u32, u32)],
    /// Honest capability note (surfaced in the GUI switcher tooltip).
    pub note: &'static str,
    /// Editorial guidance for the AI assistant, in the two UI languages. It
    /// belongs on the descriptor so a platform's publishing brief and its
    /// machine-checked rules can never drift apart.
    #[serde(skip)]
    pub editorial_zh: &'static str,
    #[serde(skip)]
    pub editorial_en: &'static str,
}

impl PlatformSpec {
    /// The image target for article images on this platform. Rich-text
    /// platforms get a width clamp only (their editors scale down by
    /// themselves and keep the source aspect); image/caption platforms take
    /// their primary preset's shape as the cover form - the same data the
    /// size studio shows, so "descriptor is data" stays true for behaviour.
    pub fn image_profile(&self) -> crate::img::ImageProfile {
        if self.rich_text {
            crate::img::ImageProfile {
                max_width: 1080,
                cover_aspect: None,
            }
        } else {
            let (w, h) = self
                .presets
                .first()
                .map(|(_, w, h)| (*w, *h))
                .unwrap_or((1080, 1080));
            crate::img::ImageProfile {
                max_width: w,
                cover_aspect: Some((w, h)),
            }
        }
    }
}

/// Xiaohongshu note limits.
const XHS_RULES: &[CaptionRule] = &[
    CaptionRule {
        rule_id: "XHS-1",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::TitleOver(20),
        message: "title is {} chars; the Xiaohongshu title field caps at 20",
        node: CaptionNode::Title,
    },
    CaptionRule {
        rule_id: "XHS-2",
        severity: CaptionSeverity::Block,
        check: CaptionCheck::BodyOver(1000),
        message: "caption is {} chars; Xiaohongshu notes cap at 1000",
        node: CaptionNode::Caption,
    },
    CaptionRule {
        rule_id: "XHS-3",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::FewerHashtags(2),
        message: "no #hashtag# found; Xiaohongshu relies on hashtags for reach",
        node: CaptionNode::None,
    },
    CaptionRule {
        rule_id: "XHS-4",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::NoImages,
        message: "image note without images; export a cover/content image set",
        node: CaptionNode::None,
    },
    CaptionRule {
        rule_id: "XHS-5",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::TooManyImages(9),
        message: "{} images; a Xiaohongshu note carries at most 9",
        node: CaptionNode::None,
    },
];

const X_RULES: &[CaptionRule] = &[
    CaptionRule {
        rule_id: "X-1",
        severity: CaptionSeverity::Block,
        check: CaptionCheck::WeightedOver(280),
        message: "caption is {} weighted chars; X caps a post at 280 (CJK counts as 2)",
        node: CaptionNode::Caption,
    },
    CaptionRule {
        rule_id: "X-2",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::TooManyImages(4),
        message: "{} images; an X post carries at most 4",
        node: CaptionNode::None,
    },
];

const INSTAGRAM_RULES: &[CaptionRule] = &[
    CaptionRule {
        rule_id: "IG-1",
        severity: CaptionSeverity::Block,
        check: CaptionCheck::BodyOver(2200),
        message: "caption is {} chars; Instagram caps a caption at 2200",
        node: CaptionNode::Caption,
    },
    CaptionRule {
        rule_id: "IG-2",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::TooManyImages(10),
        message: "{} images; an Instagram carousel carries at most 10",
        node: CaptionNode::None,
    },
];

const LINKEDIN_RULES: &[CaptionRule] = &[
    CaptionRule {
        rule_id: "LI-1",
        severity: CaptionSeverity::Block,
        check: CaptionCheck::BodyOver(3000),
        message: "post is {} chars; LinkedIn caps a post at 3000",
        node: CaptionNode::Caption,
    },
    CaptionRule {
        rule_id: "LI-2",
        severity: CaptionSeverity::Warn,
        check: CaptionCheck::TooManyImages(9),
        message: "{} images; a LinkedIn post carries at most 9",
        node: CaptionNode::None,
    },
];

const META_RULES: &[CaptionRule] = &[CaptionRule {
    rule_id: "FB-1",
    severity: CaptionSeverity::Block,
    check: CaptionCheck::BodyOver(63206),
    message: "post is {} chars; Facebook caps a post at 63206",
    node: CaptionNode::Caption,
}];

/// Rich-text targets still have a title cap even though their body is not a
/// caption. Declaring it here keeps the AI brief and the audit on one source.
const WECHAT_RULES: &[CaptionRule] = &[CaptionRule {
    rule_id: "WX-T1",
    severity: CaptionSeverity::Block,
    check: CaptionCheck::TitleOver(64),
    message: "title is {} chars; WeChat MP caps the title at 64",
    node: CaptionNode::Title,
}];

const ZHIHU_RULES: &[CaptionRule] = &[CaptionRule {
    rule_id: "ZH-T1",
    severity: CaptionSeverity::Warn,
    check: CaptionCheck::TitleOver(100),
    message: "title is {} chars; Zhihu titles read best under 100",
    node: CaptionNode::Title,
}];

pub const WECHAT: PlatformSpec = PlatformSpec {
    id: "wechat",
    export_kind: ExportKind::RichTextDialect,
    caption_rules: WECHAT_RULES,
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
    editorial_zh: "公众号是长文场景：用 Markdown 源文写作，标题写进标题栏，正文从二级标题开始分层；段落短，每段 2-4 行；表格不超过 4 列；可用 [!NOTE]/[!TIP]/[!WARNING]/[!IMPORTANT] 卡片强调重点；图片独立成段；代码用围栏。不要输出整篇 HTML，富样式片段只使用受支持的 ```html 围栏。标题栏最多 64 字，摘要最多 120 字。",
    editorial_en: "WeChat MP is a long-form channel: write Markdown source, keep the title in the title field, start body headings at level two, keep paragraphs to 2-4 lines, use at most four table columns, use [!NOTE]/[!TIP]/[!WARNING]/[!IMPORTANT] cards for emphasis, put images in their own paragraphs, and fence code. Do not output a whole HTML document; styled fragments only inside supported ```html fences. The title field caps at 64 chars and the digest at 120.",
};

pub const XHS: PlatformSpec = PlatformSpec {
    id: "xhs",
    export_kind: ExportKind::Caption,
    caption_rules: XHS_RULES,
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
    editorial_zh: "小红书是图片笔记：正文是纯文案，口语化、短句、每段 1-2 行；开头 1-2 句给钩子；结尾用提问引导评论；话题写成 #话题#，至少 2 个；不要出现 Markdown 标记（# 标题、**加粗**、[链接]），不要写外链或“点击链接”这类导流话术；图片至少 1 张、最多 9 张，首图就是封面。",
    editorial_en: "Xiaohongshu is an image note: the text is a plain caption, conversational and short, 1-2 lines per paragraph. Open with a 1-2 sentence hook, close with a question, and write hashtags as #topic# (at least two). No Markdown syntax (# headings, **bold**, [links]), no external links or link-bait, and 1-9 images with the first one as the cover.",
};

pub const ZHIHU: PlatformSpec = PlatformSpec {
    id: "zhihu",
    export_kind: ExportKind::Markdown,
    caption_rules: ZHIHU_RULES,
    name_zh: "知乎",
    name_en: "Zhihu",
    rich_text: true,
    image_note: false,
    api_publish: false,
    presets: &[("封面 1920×1080（16:9）", 1920, 1080)],
    note: "Markdown/富文本友好（出口适配待接）",
    editorial_zh: "知乎是问答/专栏长文：Markdown 直接生效，标题层级从二级开始；先给结论再展开论证；列表和表格用来做对比；引用来源；避免营销腔和夸张标题党。",
    editorial_en: "Zhihu is a long-form Q&A/column channel: Markdown passes through, start headings at level two, lead with the conclusion then argue, use lists and tables for comparison, cite sources, and avoid marketing tone and clickbait.",
};

pub const META: PlatformSpec = PlatformSpec {
    id: "meta",
    export_kind: ExportKind::Caption,
    caption_rules: META_RULES,
    name_zh: "Facebook",
    name_en: "Facebook",
    rich_text: false,
    image_note: false,
    api_publish: false,
    presets: &[
        ("横图 1200×630（1.91:1）", 1200, 630),
        ("方图 1080×1080（1:1）", 1080, 1080),
        ("竖图 1080×1350（4:5）", 1080, 1350),
    ],
    note: "文案 + 配图导出；Graph API 发布需资质（v2 出口适配）",
    editorial_zh: "Facebook 是信息流短文：前 1-2 行决定展开率，先给钩子；段落短；话题标签不超过 2 个；链接放正文末尾；避免全大写和纯营销话术。",
    editorial_en: "Facebook is a feed post: the first 1-2 lines decide whether it expands, so lead with the hook, keep paragraphs short, use at most two hashtags, put links at the end, and avoid all-caps or pure sales copy.",
};

pub const INSTAGRAM: PlatformSpec = PlatformSpec {
    id: "instagram",
    export_kind: ExportKind::Caption,
    caption_rules: INSTAGRAM_RULES,
    name_zh: "Instagram",
    name_en: "Instagram",
    rich_text: false,
    image_note: true,
    api_publish: false,
    presets: &[
        ("竖图 1080×1350（4:5）", 1080, 1350),
        ("方图 1080×1080（1:1）", 1080, 1080),
        ("Story 1080×1920（9:16）", 1080, 1920),
    ],
    note: "图片优先（4:5/1:1）；文案带话题标签；Graph API 发布需资质",
    editorial_zh: "Instagram 以图为主：文案第一行是钩子（折叠前只显示约 125 字）；短段落、留白；话题标签 5-10 个放在末尾；正文里的链接不可点击，不要写“点击链接”；图片 1-10 张。",
    editorial_en: "Instagram is image-first: the first caption line is the hook (about 125 chars show before truncation), keep paragraphs short with white space, put 5-10 hashtags at the end, never say \"click the link\" because caption links are not clickable, and attach 1-10 images.",
};

pub const X: PlatformSpec = PlatformSpec {
    id: "x",
    export_kind: ExportKind::Caption,
    caption_rules: X_RULES,
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
    editorial_zh: "X 是短帖：一条最多 280 加权字符（中文和 emoji 各记 2，英文/数字记 1）；先给结论，避免长段；话题标签 1-2 个；需要长文时拆成串（1/n）；配图最多 4 张。",
    editorial_en: "X is a short post: one post is at most 280 weighted characters (CJK and emoji count 2, Latin/digits 1), lead with the conclusion, avoid long paragraphs, use 1-2 hashtags, split long pieces into a 1/n thread, and attach at most 4 images.",
};

pub const LINKEDIN: PlatformSpec = PlatformSpec {
    id: "linkedin",
    export_kind: ExportKind::Caption,
    caption_rules: LINKEDIN_RULES,
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
    editorial_zh: "LinkedIn 是职业内容：开头两行给结论或洞见；短段落、空行分隔；专业但不堆术语；话题标签 3-5 个；文末留一个讨论问题；最多 9 张图。",
    editorial_en: "LinkedIn is professional content: open with a conclusion or insight in the first two lines, use short paragraphs separated by blank lines, stay professional without jargon, add 3-5 hashtags, close with a discussion question, and attach at most 9 images.",
};

/// All platforms in switcher order (WeChat first: the default target).
pub fn list_platforms() -> Vec<PlatformSpec> {
    vec![
        WECHAT.clone(),
        XHS.clone(),
        ZHIHU.clone(),
        META.clone(),
        INSTAGRAM.clone(),
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

/// Same lookup, but reports whether the id was recognised. Anywhere the
/// platform comes from user input (frontmatter `platform:`, CLI `--platform`)
/// callers should surface a warning on `false` - a one-character typo
/// otherwise silently renders the whole article as WeChat dialect.
pub fn resolve_platform(id: &str) -> (PlatformSpec, bool) {
    match list_platforms().into_iter().find(|p| p.id == id) {
        Some(p) => (p, true),
        None => (WECHAT, false),
    }
}

// ------------------------------------------------------- export adapters --

/// The export artifact for a platform id. Reads the registry, so a new
/// platform only has to declare its kind once.
pub fn export_kind(id: &str) -> ExportKind {
    get_platform(id).export_kind
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
    // Collapse the blank lines the block renderers emit down to a single
    // paragraph break - caption hosts render every extra newline literally.
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
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
                    // Checkmark glyphs, written as escapes so the no-emoji
                    // source scan stays clean; caption hosts render them fine.
                    Some(true) => out.push_str("\u{2713} "),
                    Some(false) => out.push_str("\u{25A1} "),
                    None => {}
                }
                push_blocks(&item.blocks, out);
                out.push('\n');
            }
            out.push('\n');
        }
        Block::Blockquote { blocks } => push_blocks(blocks, out),
        Block::Card { kind, blocks } => {
            // The dialect renders a labelled card; a caption gets the label as
            // its own line so the reader still sees "this is a note/tip/
            // warning" after the card styling is gone.
            out.push_str(&format!("【{}】\n", crate::i18n::t(kind.i18n_key())));
            push_blocks(blocks, out);
            out.push('\n');
        }
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
        // A caption is plain text: markup must not leak into it.
        Block::RawHtml { html } => {
            out.push_str(&crate::htmlutil::strip_tags(html));
            out.push_str("\n\n");
        }
        Block::HtmlFence { html } => {
            out.push_str(&crate::htmlutil::strip_tags(html));
            out.push_str("\n\n");
        }
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
        Inline::RawHtml(h) => out.push_str(&crate::htmlutil::strip_tags(h)),
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
    // The rules come from the descriptor, so this function does not know or
    // care which platform it is looking at.
    let spec = get_platform(platform);
    let mut v = Vec::new();
    for rule in spec.caption_rules {
        if let Some(measured) = rule_fires(rule, title, caption, images) {
            v.push(rule_violation(rule, measured, title, caption));
        }
    }
    v
}

/// Title-only rules for rich-text platforms. Their article pipeline has no
/// caption, but the title cap is still a publishing limit; use the same
/// descriptor rules instead of a second hard-coded table.
pub fn validate_platform_title(platform: &str, title: &str) -> Vec<crate::validator::Violation> {
    let spec = get_platform(platform);
    spec.caption_rules
        .iter()
        .filter(|r| matches!(r.check, CaptionCheck::TitleOver(_)))
        .filter_map(|r| {
            rule_fires(r, title, "", 0).map(|measured| rule_violation(r, measured, title, ""))
        })
        .collect()
}

fn rule_fires(
    rule: &CaptionRule,
    title: &str,
    caption: &str,
    images: usize,
) -> Option<Option<usize>> {
    let (fired, measured): (bool, Option<usize>) = match rule.check {
        CaptionCheck::TitleOver(n) => {
            let c = title.chars().count();
            (c > n, Some(c))
        }
        CaptionCheck::BodyOver(n) => {
            let c = caption.chars().count();
            (c > n, Some(c))
        }
        CaptionCheck::WeightedOver(n) => {
            let c = x_weighted_len(caption);
            (c > n, Some(c))
        }
        CaptionCheck::FewerHashtags(n) => {
            let c = caption.matches('#').count();
            (c < n, Some(c))
        }
        CaptionCheck::NoImages => (images == 0, None),
        CaptionCheck::TooManyImages(n) => (images > n, Some(images)),
    };
    fired.then_some(measured)
}

fn rule_violation(
    rule: &CaptionRule,
    measured: Option<usize>,
    title: &str,
    caption: &str,
) -> crate::validator::Violation {
    let message = match measured {
        Some(m) => rule.message.replacen("{}", &m.to_string(), 1),
        None => rule.message.to_string(),
    };
    let node = match rule.node {
        CaptionNode::Title => truncate_node(title),
        CaptionNode::Caption => truncate_node(caption),
        CaptionNode::None => String::new(),
    };
    crate::validator::Violation {
        rule_id: rule.rule_id.into(),
        severity: match rule.severity {
            CaptionSeverity::Block => "block".into(),
            CaptionSeverity::Warn => "warn".into(),
        },
        message,
        node,
        fixable: false,
    }
}

fn truncate_node(s: &str) -> String {
    s.chars().take(60).collect()
}

/// The publishing brief the AI assistant (both chat and one-click repair)
/// must follow for a platform.
///
/// Limits are generated from the same `caption_rules` the validator consumes,
/// so the model is never told a different budget than the one it is judged
/// against. Editorial guidance lives on the descriptor too, which keeps this
/// function free of per-platform branches.
pub fn agent_brief(platform: &str, lang: &str) -> String {
    let (spec, known) = resolve_platform(platform);
    let en = lang.to_ascii_lowercase().starts_with("en");
    let mut out = String::new();
    if !known {
        out.push_str(if en {
            "Unknown platform id; falling back to WeChat MP rules.\n\n"
        } else {
            "未知平台 id，按微信公众号规则处理。\n\n"
        });
    }

    if en {
        out.push_str(&format!(
            "# Publishing brief: {} ({})\n\n",
            spec.name_en, spec.id
        ));
        out.push_str("Format: ");
        out.push_str(match spec.export_kind {
            ExportKind::RichTextDialect => {
                "Markdown source rendered into the platform's rich text; styled fragments only through the supported ```html allowlist.\n\n"
            }
            ExportKind::Markdown => {
                "Markdown passes through unchanged; use standard CommonMark.\n\n"
            }
            ExportKind::Caption => {
                "plain-text caption only; Markdown syntax (# headings, **bold**, [links]) is not rendered.\n\n"
            }
        });
    } else {
        out.push_str(&format!("# 发布简报：{}（{}）\n\n", spec.name_zh, spec.id));
        out.push_str("内容形态：");
        out.push_str(match spec.export_kind {
            ExportKind::RichTextDialect => {
                "Markdown 源文，由引擎排版为平台富文本；富样式片段只能写在受支持的 ```html 围栏里。\n\n"
            }
            ExportKind::Markdown => "Markdown 原样发布，使用标准 CommonMark 语法。\n\n",
            ExportKind::Caption => {
                "纯文本文案；Markdown 标记（# 标题、**加粗**、[链接]）不会被渲染。\n\n"
            }
        });
    }

    let mut limits: Vec<String> = Vec::new();
    for rule in spec.caption_rules {
        let line = match rule.check {
            CaptionCheck::TitleOver(n) => {
                if en {
                    format!("Title: at most {n} characters.")
                } else {
                    format!("标题：不超过 {n} 字。")
                }
            }
            CaptionCheck::BodyOver(n) => {
                if en {
                    format!("Post/caption: at most {n} characters.")
                } else {
                    format!("正文/文案：不超过 {n} 字。")
                }
            }
            CaptionCheck::WeightedOver(n) => {
                if en {
                    format!("Post: at most {n} weighted characters (CJK and emoji count as 2).")
                } else {
                    format!("正文：不超过 {n} 加权字符（中文和 emoji 各记 2）。")
                }
            }
            CaptionCheck::FewerHashtags(n) => {
                if en {
                    format!("Hashtags: at least {n}.")
                } else {
                    format!("话题标签：至少 {n} 个。")
                }
            }
            CaptionCheck::NoImages => {
                if en {
                    "Images: at least 1.".to_string()
                } else {
                    "图片：至少 1 张。".to_string()
                }
            }
            CaptionCheck::TooManyImages(n) => {
                if en {
                    format!("Images: at most {n}.")
                } else {
                    format!("图片：最多 {n} 张。")
                }
            }
        };
        limits.push(line);
    }
    if !limits.is_empty() {
        out.push_str(if en {
            "## Hard limits (machine-checked)\n"
        } else {
            "## 硬性限制（会被校验器逐条检查）\n"
        });
        for line in limits {
            out.push_str("- ");
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str(if en {
        "## Editorial guidance\n"
    } else {
        "## 写作要求\n"
    });
    out.push_str(if en {
        spec.editorial_en
    } else {
        spec.editorial_zh
    });
    out.push('\n');
    out
}

/// What a platform that does not paste styled HTML consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextArtifact {
    /// Markdown passed through unchanged (Zhihu).
    Markdown(String),
    /// A plain-text caption (Xiaohongshu and the western feeds).
    Caption(String),
}

impl TextArtifact {
    pub fn kind(&self) -> &'static str {
        match self {
            TextArtifact::Markdown(_) => "markdown",
            TextArtifact::Caption(_) => "caption",
        }
    }

    pub fn text(&self) -> &str {
        match self {
            TextArtifact::Markdown(t) | TextArtifact::Caption(t) => t,
        }
    }
}

/// Build the text artifact a platform consumes, plus any violations of that
/// platform's own caption rules.
///
/// Returns `None` for rich-text platforms: their artifact depends on a theme
/// and the image pipeline, which each host configures differently, so the host
/// builds it and this function stays out of the way.
///
/// Why this exists: the CLI, the MCP server and the GUI each re-implemented the
/// same `match export_kind` dispatch, and the copies disagreed.
/// - the CLI reported no caption violations at all;
/// - every caller passed `images = 0`, so XHS-4 ("image note without images")
///   fired on articles full of images while XHS-5 ("more than nine") could
///   never fire;
/// - the MCP tool validated `render_caption(&doc, None)` but returned
///   `render_caption(&doc, Some(title))`, so it checked a different string than
///   the one it handed back.
pub fn export_text_artifact(
    platform: &str,
    markdown: &str,
    title: Option<&str>,
) -> Option<(TextArtifact, Vec<crate::validator::Violation>)> {
    let spec = get_platform(platform);
    match spec.export_kind {
        ExportKind::RichTextDialect => None,
        ExportKind::Markdown => Some((TextArtifact::Markdown(markdown.to_string()), Vec::new())),
        ExportKind::Caption => {
            let doc = crate::parser::parse_markdown(markdown);
            let text = render_caption(&doc, title);
            // The number of images the note actually carries. Inline images are
            // not counted - they are decoration inside a paragraph, not part of
            // the note's image set.
            let images = crate::ir::count_stats(&doc).images;
            let violations = validate_platform_caption(spec.id, title.unwrap_or(""), &text, images);
            Some((TextArtifact::Caption(text), violations))
        }
    }
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
            // Matches render.rs: raw HTML is escaped, never injected.
            Inline::RawHtml(h) => self.out.push_str(&crate::htmlutil::escape_text(h)),
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
                self.out.push_str(&format!(
                    "<p><code>{}</code></p>",
                    crate::htmlutil::escape_text(latex)
                ));
            }
            Block::Rule => self.out.push_str("<hr/>"),
            Block::Toc => {}
            Block::RawHtml { html } => self.out.push_str(&crate::htmlutil::escape_text(html)),
            Block::HtmlFence { html } => self.out.push_str(&crate::htmlutil::escape_text(html)),
            Block::SvgEmbed { html } => self.out.push_str(html),
            Block::Chart { spec } => {
                self.out.push_str(&crate::render::render_chart(spec));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Raw HTML must never be injected verbatim. render.rs escapes it for the
    /// WeChat dialect; the caption and plain-HTML exporters used to pass it
    /// straight through, so a `<script>` in the source reached the exported
    /// HTML as live markup and the caption as visible tag soup.
    ///
    /// Only the *block* case is covered here. Inline HTML that is not on the
    /// allowlist is turned into `Inline::Text` by the parser (a deliberate v1
    /// choice, see parser.rs), so `<b>x</b>` inside a paragraph reaches every
    /// exporter as literal text - that is a separate decision, not this guard.
    #[test]
    fn raw_html_is_never_passed_through_verbatim() {
        let md = "正文\n\n<script>alert(1)</script>\n\n结尾段落\n";

        let doc = crate::parser::parse_markdown(md);
        let html = render_plain_html(&doc, &[]);
        assert!(
            !html.contains("<script"),
            "the plain-HTML exporter must not emit live markup: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "raw HTML should be escaped, not dropped: {html}"
        );

        let cap = render_caption(&doc, None);
        assert!(
            !cap.contains('<') && !cap.contains('>'),
            "a caption is plain text and must not carry markup: {cap}"
        );
        // A script's body is not text and must not end up in a caption.
        assert!(
            !cap.contains("alert(1)"),
            "script content must be dropped, got: {cap}"
        );
        // The surrounding paragraphs must still be there and separated.
        assert!(
            cap.contains("正文") && cap.contains("结尾段落"),
            "got: {cap}"
        );
        assert!(
            cap.contains("正文\n\n结尾段落"),
            "the raw-HTML block must not glue the paragraphs together: {cap:?}"
        );
    }

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
        assert!(
            cap.starts_with("我的标题\n\n正文加粗和链接文字。"),
            "got: {cap}"
        );
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
        let by_id = |id: &str| {
            list_platforms()
                .into_iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("platform {id} missing from registry"))
        };
        let xhs = by_id("xhs");
        assert!(xhs.image_note && !xhs.rich_text);
        let wechat = by_id("wechat");
        assert!(wechat.rich_text && !wechat.image_note);
        let instagram = by_id("instagram");
        assert!(instagram.image_note && !instagram.rich_text);
        let meta = by_id("meta");
        assert!(
            !meta.image_note,
            "Facebook is a caption feed, not image-note"
        );
        assert_eq!(meta.name_zh, "Facebook");
    }

    /// The validator must read the descriptor, not the platform id. Every
    /// declared rule is driven with an input that must trip it; a platform
    /// that declares nothing must stay silent even on input that trips every
    /// rule in the registry.
    #[test]
    fn caption_rules_come_from_the_descriptor() {
        for p in list_platforms() {
            for rule in p.caption_rules {
                let (title, body, images) = match rule.check {
                    CaptionCheck::TitleOver(n) => ("标".repeat(n + 1), "正文".to_string(), 1),
                    CaptionCheck::BodyOver(n) => ("短标题".to_string(), "字".repeat(n + 1), 1),
                    CaptionCheck::WeightedOver(n) => {
                        ("短标题".to_string(), "字".repeat(n / 2 + 1), 1)
                    }
                    CaptionCheck::FewerHashtags(_) => {
                        ("短标题".to_string(), "正文没有标签".to_string(), 1)
                    }
                    CaptionCheck::NoImages => ("短标题".to_string(), "正文".to_string(), 0),
                    CaptionCheck::TooManyImages(n) => {
                        ("短标题".to_string(), "正文".to_string(), n + 1)
                    }
                };
                let v = validate_platform_caption(p.id, &title, &body, images);
                assert!(
                    v.iter().any(|x| x.rule_id == rule.rule_id),
                    "{} declares {} but it never fired: {:?}",
                    p.id,
                    rule.rule_id,
                    v
                );
            }
        }

        let long_title = "标题".repeat(50);
        let long_body = "正文".repeat(800);
        // Title rules live in the same descriptor table even for rich-text
        // platforms, so the title-only check must follow it for every id.
        for p in list_platforms() {
            let long_t = "标".repeat(200);
            let v = validate_platform_title(p.id, &long_t);
            let declares_title = p
                .caption_rules
                .iter()
                .any(|r| matches!(r.check, CaptionCheck::TitleOver(_)));
            assert_eq!(
                !v.is_empty(),
                declares_title,
                "{} title verdict disagrees with its declared rules: {:?}",
                p.id,
                v
            );
            assert!(
                validate_platform_title(p.id, "短").is_empty(),
                "{} flagged a short title",
                p.id
            );
        }
        // An unknown id falls back to WeChat, so its 64-char title cap applies.
        let fallback = validate_platform_caption("not-a-platform", &long_title, &long_body, 0);
        assert!(
            fallback.iter().any(|v| v.rule_id == "WX-T1"),
            "unknown ids must fall back to the WeChat title rule: {fallback:?}"
        );

        // The rich-text title caps are declared, machine-checked, and exactly
        // at the published budget.
        let wx_over = validate_platform_title("wechat", &"标".repeat(65));
        assert!(wx_over.iter().any(|v| v.rule_id == "WX-T1" && v.is_block()));
        assert!(validate_platform_title("wechat", &"标".repeat(64)).is_empty());
        let zh_over = validate_platform_title("zhihu", &"标".repeat(101));
        assert!(zh_over
            .iter()
            .any(|v| v.rule_id == "ZH-T1" && !v.is_block()));
        assert!(validate_platform_title("zhihu", &"标".repeat(100)).is_empty());

        // Rule ids are unique across the whole registry, so a violation can be
        // traced back to exactly one declaration.
        let mut seen = std::collections::BTreeSet::new();
        for p in list_platforms() {
            for r in p.caption_rules {
                assert!(
                    seen.insert(r.rule_id),
                    "duplicate caption rule id {}",
                    r.rule_id
                );
                assert!(!r.message.is_empty(), "{} has an empty message", r.rule_id);
            }
        }
    }

    #[test]
    fn x_weighted_length_counts_cjk_as_two() {
        assert_eq!(x_weighted_len("hello"), 5);
        assert_eq!(x_weighted_len("a中"), 3);
        assert_eq!(x_weighted_len("中文"), 4);

        // 200 CJK chars fit a naive 280-char count but not X's weighted budget.
        let v = validate_platform_caption("x", "t", &"字".repeat(200), 1);
        assert!(
            v.iter().any(|x| x.rule_id == "X-1" && x.is_block()),
            "a 200-character Chinese post must trip X-1: {v:?}"
        );
        // Exactly at the budget still fits.
        assert!(validate_platform_caption("x", "t", &"字".repeat(140), 1).is_empty());
    }

    /// The brief handed to the model must carry the same hard limits the
    /// validator enforces, in both UI languages.
    #[test]
    fn agent_brief_matches_the_machine_checked_rules() {
        let xhs = agent_brief("xhs", "zh");
        assert!(xhs.contains("小红书"), "got: {xhs}");
        assert!(xhs.contains("1000"), "the body cap must be stated: {xhs}");
        assert!(
            xhs.contains("至少 2 个"),
            "the hashtag floor must be stated: {xhs}"
        );

        let x = agent_brief("x", "en");
        assert!(x.to_ascii_lowercase().contains("weighted"), "got: {x}");
        assert!(x.contains("280"), "the weighted cap must be stated: {x}");

        for p in list_platforms() {
            assert!(
                !p.editorial_zh.is_empty() && !p.editorial_en.is_empty(),
                "{} is missing editorial guidance",
                p.id
            );
            for lang in ["zh", "en"] {
                let brief = agent_brief(p.id, lang);
                assert!(brief.contains(p.id), "{} brief misses the id", p.id);
                assert!(
                    !brief.contains("{}"),
                    "{} brief has an unfilled placeholder",
                    p.id
                );
            }
        }

        let fallback = agent_brief("not-a-platform", "en");
        assert!(
            fallback.to_ascii_lowercase().contains("unknown"),
            "got: {fallback}"
        );
        assert!(
            fallback.contains("wechat"),
            "must name the fallback: {fallback}"
        );
    }

    /// The image count is what makes XHS-4 and XHS-5 meaningful. Every caller
    /// used to pass `images = 0`, so "image note without images" fired on
    /// articles full of images and "more than nine" could never fire.
    #[test]
    fn export_counts_the_images_a_note_actually_carries() {
        let title = Some("标题");

        let (artifact, with_image) = export_text_artifact(
            "xhs",
            "正文

![图](https://example.com/a.png)
",
            title,
        )
        .expect("xhs is a caption platform");
        assert_eq!(artifact.kind(), "caption");
        assert!(
            !with_image.iter().any(|v| v.rule_id == "XHS-4"),
            "an article with an image must not be reported as image-less: {with_image:?}"
        );

        let (_, without) = export_text_artifact(
            "xhs",
            "正文，没有图。
",
            title,
        )
        .unwrap();
        assert!(
            without.iter().any(|v| v.rule_id == "XHS-4"),
            "an article with no image must trip XHS-4: {without:?}"
        );

        let mut many = String::from(
            "正文

",
        );
        for i in 0..10 {
            many.push_str(&format!(
                "![图{i}](https://example.com/{i}.png)

"
            ));
        }
        let (_, too_many) = export_text_artifact("xhs", &many, title).unwrap();
        assert!(
            too_many.iter().any(|v| v.rule_id == "XHS-5"),
            "ten images must trip the nine-image cap: {too_many:?}"
        );

        // Zhihu passes Markdown through and declares no caption rules.
        let (md, none) = export_text_artifact(
            "zhihu",
            "# 标题
",
            None,
        )
        .expect("markdown");
        assert_eq!(md.kind(), "markdown");
        assert!(none.is_empty());

        // Rich-text platforms are host-side, so core returns nothing.
        assert!(export_text_artifact(
            "wechat",
            "# 标题
",
            None
        )
        .is_none());
    }

    #[test]
    fn every_platform_has_at_least_one_preset() {
        for p in list_platforms() {
            assert!(!p.presets.is_empty(), "{} has no presets", p.id);
        }
    }

    #[test]
    fn export_kind_comes_from_the_descriptor() {
        for p in list_platforms() {
            assert_eq!(
                export_kind(p.id),
                p.export_kind,
                "export_kind({}) must read the descriptor, not a separate match",
                p.id
            );
        }
        assert_eq!(export_kind("wechat"), ExportKind::RichTextDialect);
        assert_eq!(export_kind("xhs"), ExportKind::Caption);
        assert_eq!(export_kind("zhihu"), ExportKind::Markdown);
        // Unknown ids fall back to the WeChat dialect, same as get_platform.
        assert_eq!(export_kind("wechta"), ExportKind::RichTextDialect);
    }

    #[test]
    fn resolve_platform_reports_unknown_ids() {
        let (spec, known) = resolve_platform("xhs");
        assert!(known);
        assert_eq!(spec.id, "xhs");
        let (fallback, known) = resolve_platform("not-a-platform");
        assert!(!known, "a typo must be reported, not silently accepted");
        assert_eq!(fallback.id, "wechat");
    }

    #[test]
    fn export_kinds_match_the_platforms_actual_intake() {
        // Explicit table rather than a derived invariant: Zhihu is
        // rich-text friendly but still takes Markdown as its primary export,
        // so "rich_text implies dialect" would be wrong.
        let expected = [
            ("wechat", ExportKind::RichTextDialect),
            ("xhs", ExportKind::Caption),
            ("zhihu", ExportKind::Markdown),
            ("meta", ExportKind::Caption),
            ("instagram", ExportKind::Caption),
            ("x", ExportKind::Caption),
            ("linkedin", ExportKind::Caption),
        ];
        for (id, kind) in expected {
            let spec = get_platform(id);
            assert_eq!(spec.id, id, "{id} must be registered");
            assert_eq!(spec.export_kind, kind, "wrong export kind for {id}");
        }
        // Image-note platforms must never claim to paste styled HTML.
        for p in list_platforms() {
            if p.image_note {
                assert_eq!(
                    p.export_kind,
                    ExportKind::Caption,
                    "{} is an image-note platform, so its text export is a caption",
                    p.id
                );
            }
        }
    }
}
