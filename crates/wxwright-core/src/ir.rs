//! Intermediate representation: semantic blocks produced by the parser,
//! consumed by the dialect renderer.

/// An image reference inside the document.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub src: String,
    pub alt: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
    Comment,
    Keypoint,
}

impl CardKind {
    pub fn i18n_key(&self) -> &'static str {
        match self {
            CardKind::Note => "card.note",
            CardKind::Tip => "card.tip",
            CardKind::Important => "card.important",
            CardKind::Warning => "card.warning",
            CardKind::Caution => "card.caution",
            CardKind::Comment => "card.comment",
            CardKind::Keypoint => "card.keypoint",
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            CardKind::Note => "card_note",
            CardKind::Tip => "card_tip",
            CardKind::Important => "card_important",
            CardKind::Warning => "card_warning",
            CardKind::Caution => "card_caution",
            CardKind::Comment => "card_comment",
            CardKind::Keypoint => "card_keypoint",
        }
    }

    /// Map a manual marker like `[!COMMENT]` / `[!KEYPOINT]` to a kind.
    pub fn from_marker(marker: &str) -> Option<CardKind> {
        match marker.to_ascii_uppercase().as_str() {
            "NOTE" => Some(CardKind::Note),
            "TIP" => Some(CardKind::Tip),
            "IMPORTANT" => Some(CardKind::Important),
            "WARNING" => Some(CardKind::Warning),
            "CAUTION" => Some(CardKind::Caution),
            "COMMENT" => Some(CardKind::Comment),
            "KEYPOINT" => Some(CardKind::Keypoint),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineKind {
    Strong,
    Emphasis,
    Strike,
    Link { url: String, title: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Math {
        latex: String,
        display: bool,
    },
    Image(ImageRef),
    Break,
    Styled {
        kind: InlineKind,
        children: Vec<Inline>,
    },
    /// Allow-listed raw inline HTML (e.g. <br>), already sanitized.
    RawHtml(String),
}

/// Chart specification parsed from a ```chart fenced block (JSON).
/// Rendered as inline SVG by the engine; AI can emit these directly.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChartSpec {
    #[serde(default = "default_chart_kind")]
    pub kind: String, // bar | line | pie
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub values: Vec<f64>,
    #[serde(default)]
    pub unit: String,
}

fn default_chart_kind() -> String {
    "bar".into()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        center: bool,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    Code {
        lang: Option<String>,
        code: String,
    },
    Blockquote {
        blocks: Vec<Block>,
    },
    Card {
        kind: CardKind,
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    Table {
        aligns: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Image(ImageRef),
    Figure {
        image: ImageRef,
        caption: Vec<Inline>,
    },
    Rule,
    Toc,
    Formula {
        latex: String,
    },
    /// Raw HTML block: escaped and rendered as plain text (v1 does not
    /// pass arbitrary HTML through, per official whitelist constraints).
    RawHtml {
        html: String,
    },
    /// An ```html fence whose markup passed the allowlist: emitted verbatim,
    /// then normalized by the pipeline like the rest of the document. This is
    /// the AI's rich-block channel - the way a generated card, badge or grid
    /// reaches the article without the dialect having to know about it.
    ///
    /// Distinct from `RawHtml`, which is always escaped.
    HtmlFence {
        html: String,
    },
    /// Safety-checked inline SVG component (from the SVG kit or AI): passed
    /// through verbatim, then re-cleaned by the normalizer and re-checked by
    /// the validator. Never produced for arbitrary user HTML.
    SvgEmbed {
        html: String,
    },
    /// Data chart rendered as compliant inline SVG (bar/line/pie), from a
    /// ```chart fenced block (JSON spec). AI can emit these directly.
    Chart {
        spec: ChartSpec,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

/// Document statistics reported alongside conversion.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Stats {
    pub chars: usize,
    pub words: usize,
    pub headings: usize,
    pub images: usize,
    pub code_blocks: usize,
    pub tables: usize,
}

pub fn count_stats(blocks: &[Block]) -> Stats {
    let mut s = Stats::default();
    walk_blocks(blocks, &mut s);
    s
}

fn walk_blocks(blocks: &[Block], s: &mut Stats) {
    for b in blocks {
        match b {
            Block::Heading { inlines, .. } => {
                s.headings += 1;
                s.chars += inline_chars(inlines);
            }
            Block::Paragraph { inlines } => {
                let n = inline_chars(inlines);
                s.chars += n;
                s.words += inline_words(inlines);
            }
            Block::Code { code, .. } => {
                s.code_blocks += 1;
                s.chars += code.chars().count();
            }
            Block::Blockquote { blocks } | Block::Card { blocks, .. } => walk_blocks(blocks, s),
            Block::List { items, .. } => {
                for it in items {
                    walk_blocks(&it.blocks, s);
                }
            }
            Block::Table { header, rows, .. } => {
                s.tables += 1;
                for cell in header {
                    s.chars += inline_chars(cell);
                }
                for row in rows {
                    for cell in row {
                        s.chars += inline_chars(cell);
                    }
                }
            }
            Block::Image(_) => s.images += 1,
            Block::Figure { image, caption } => {
                s.images += 1;
                s.chars += inline_chars(caption);
                let _ = image;
            }
            Block::Rule
            | Block::Toc
            | Block::RawHtml { .. }
            | Block::HtmlFence { .. }
            | Block::SvgEmbed { .. } => {}
            Block::Chart { spec } => s.chars += spec.title.chars().count() + 8,
            Block::Formula { latex } => s.chars += latex.chars().count(),
        }
    }
}

fn inline_chars(inlines: &[Inline]) -> usize {
    let mut n = 0;
    for i in inlines {
        match i {
            Inline::Text(t) => n += t.chars().count(),
            Inline::Code(c) => n += c.chars().count(),
            Inline::Math { latex, .. } => n += latex.chars().count(),
            Inline::Image(_) => n += 1,
            Inline::Break | Inline::RawHtml(_) => {}
            Inline::Styled { children, .. } => n += inline_chars(children),
        }
    }
    n
}

fn inline_words(inlines: &[Inline]) -> usize {
    let mut n = 0;
    for i in inlines {
        if let Inline::Text(t) = i {
            n += t.split_whitespace().count();
        }
    }
    n
}
