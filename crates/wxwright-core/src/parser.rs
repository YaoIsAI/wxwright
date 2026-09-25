//! Markdown (GFM) -> IR parser built on pulldown-cmark.
//! Handles: headings (+{.center} attribute), paragraphs, bold/italic/strike,
//! links, ordered/unordered/task lists, quotes, GitHub alerts + manual
//! [!COMMENT]/[!KEYPOINT] markers, tables, fenced/indented code, images,
//! figures (image + italic caption), rules, [TOC], formulas, raw HTML blocks.

use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

use crate::ir::{Align, Block, CardKind, ImageRef, Inline, InlineKind, ListItem};

pub fn parse_markdown(md: &str) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_GFM);
    opts.insert(Options::ENABLE_MATH);

    let mut p = ParserState {
        stack: Vec::new(),
        root: Vec::new(),
    };
    for ev in Parser::new_ext(md, opts) {
        p.feed(ev);
    }
    let extra = p.drain_frames();
    p.root.extend(extra);
    postprocess(&mut p.root);
    p.root
}

enum Frame {
    Quote {
        card: Option<CardKind>,
        blocks: Vec<Block>,
    },
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        center: bool,
        inlines: Vec<Inline>,
    },
    Code {
        lang: Option<String>,
        code: String,
    },
    HtmlBuf(String),
    Swallow, // footnote definitions, metadata, anything not rendered in v1
    Table {
        aligns: Vec<Align>,
        in_head: bool,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
        row: Vec<Vec<Inline>>,
    },
    List {
        ordered: bool,
        start: u64,
        next_index: u64,
        items: Vec<ListItem>,
    },
    Item {
        checked: Option<bool>,
        blocks: Vec<Block>,
        /// Tight lists emit item content without a Paragraph wrapper; these
        /// inlines become the item's first paragraph.
        pending: Vec<Inline>,
    },
    Inline {
        kind: InlineKind,
        children: Vec<Inline>,
    },
    /// Swallows events until the image tag ends (alt text already in tag).
    Image(ImageRef),
}

struct ParserState {
    stack: Vec<Frame>,
    root: Vec<Block>,
}

impl ParserState {
    /// Route a completed block into the innermost block sink.
    fn push_block(&mut self, b: Block) {
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Quote { blocks, .. } => {
                    blocks.push(b);
                    return;
                }
                Frame::Item { blocks, .. } => {
                    blocks.push(b);
                    return;
                }
                Frame::Swallow => return,
                _ => {}
            }
        }
        self.root.push(b);
    }

    fn drain_frames(&mut self) -> Vec<Block> {
        let mut out = Vec::new();
        while let Some(f) = self.stack.pop() {
            match f {
                Frame::Quote { blocks, .. } => out.extend(blocks),
                Frame::Item { blocks, .. } => out.extend(blocks),
                _ => {}
            }
        }
        out
    }

    /// Push an inline into the innermost inline sink.
    fn push_inline(&mut self, il: Inline) {
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Paragraph(v) | Frame::Heading { inlines: v, .. } => {
                    merge_inline(v, il);
                    return;
                }
                Frame::Inline { children, .. } => {
                    merge_inline(children, il);
                    return;
                }
                Frame::Image(img) => {
                    // Alt text streams as inner text events.
                    if let Inline::Text(t) = &il {
                        img.alt.push_str(t);
                    }
                    return;
                }
                Frame::Code { code, .. } => {
                    // Code block content arrives as plain text events.
                    if let Inline::Text(t) = &il {
                        code.push_str(t);
                    }
                    return;
                }
                Frame::Item { pending, .. } => {
                    merge_inline(pending, il);
                    return;
                }
                Frame::Swallow => return,
                _ => {}
            }
        }
    }

    fn feed(&mut self, ev: Event) {
        match ev {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag_end) => self.end_tag(tag_end),
            Event::Text(t) => self.push_inline(Inline::Text(t.to_string())),
            Event::Code(c) => self.push_inline(Inline::Code(c.to_string())),
            Event::InlineMath(m) => self.push_inline(Inline::Math {
                latex: m.to_string(),
                display: false,
            }),
            Event::DisplayMath(m) => self.push_inline(Inline::Math {
                latex: m.to_string(),
                display: true,
            }),
            Event::SoftBreak => self.push_inline(Inline::Text(" ".into())),
            Event::HardBreak => self.push_inline(Inline::Break),
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item { checked, .. }) = self.stack.last_mut() {
                    *checked = Some(done);
                }
            }
            Event::Rule => self.push_block(Block::Rule),
            Event::InlineHtml(h) => {
                let t = h.trim().to_ascii_lowercase();
                if t == "<br>" || t == "<br/>" || t == "<br />" {
                    self.push_inline(Inline::Break);
                } else {
                    // v1: non-allowlisted inline HTML is escaped as text.
                    self.push_inline(Inline::Text(h.to_string()));
                }
            }
            Event::Html(h) => {
                if let Some(Frame::HtmlBuf(b)) = self.stack.last_mut() {
                    b.push_str(&h);
                }
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.stack.push(Frame::Paragraph(Vec::new())),
            Tag::Heading { level, classes, .. } => {
                let center = classes.iter().any(|c| c.as_ref() == "center");
                self.stack.push(Frame::Heading {
                    level: heading_num(level),
                    center,
                    inlines: Vec::new(),
                });
            }
            Tag::BlockQuote(kind) => {
                self.stack.push(Frame::Quote {
                    card: kind.map(card_from_gfm),
                    blocks: Vec::new(),
                });
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        let s = info.trim();
                        let first = s.split([',', ' ']).next().unwrap_or("");
                        if first.is_empty() {
                            None
                        } else {
                            Some(first.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.stack.push(Frame::Code {
                    lang,
                    code: String::new(),
                });
            }
            Tag::HtmlBlock => self.stack.push(Frame::HtmlBuf(String::new())),
            Tag::List(start) => {
                self.stack.push(Frame::List {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    next_index: start.unwrap_or(1),
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.stack.push(Frame::Item {
                    checked: None,
                    blocks: Vec::new(),
                    pending: Vec::new(),
                });
            }
            Tag::Table(aligns) => {
                let aligns = aligns
                    .into_iter()
                    .map(|a| match a {
                        pulldown_cmark::Alignment::None => Align::None,
                        pulldown_cmark::Alignment::Left => Align::Left,
                        pulldown_cmark::Alignment::Center => Align::Center,
                        pulldown_cmark::Alignment::Right => Align::Right,
                    })
                    .collect();
                self.stack.push(Frame::Table {
                    aligns,
                    in_head: false,
                    header: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableHead => {
                if let Some(Frame::Table { in_head, .. }) = self.stack.last_mut() {
                    *in_head = true;
                }
            }
            Tag::TableRow => {}
            Tag::TableCell => self.stack.push(Frame::Paragraph(Vec::new())),
            Tag::Emphasis => self.push_inline_frame(InlineKind::Emphasis),
            Tag::Strong => self.push_inline_frame(InlineKind::Strong),
            Tag::Strikethrough => self.push_inline_frame(InlineKind::Strike),
            Tag::Link {
                dest_url, title, ..
            } => {
                self.push_inline_frame(InlineKind::Link {
                    url: dest_url.to_string(),
                    title: if title.is_empty() {
                        None
                    } else {
                        Some(title.to_string())
                    },
                });
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack.push(Frame::Image(ImageRef {
                    src: dest_url.to_string(),
                    alt: String::new(),
                    title: if title.is_empty() {
                        None
                    } else {
                        Some(title.to_string())
                    },
                }));
            }
            _ => self.stack.push(Frame::Swallow),
        }
    }

    fn push_inline_frame(&mut self, kind: InlineKind) {
        self.stack.push(Frame::Inline {
            kind,
            children: Vec::new(),
        });
    }

    fn end_tag(&mut self, end: pulldown_cmark::TagEnd) {
        // Table-internal tags share the Table frame; handle them first.
        match end {
            pulldown_cmark::TagEnd::TableHead => {
                if let Some(Frame::Table { in_head, .. }) = self.stack.last_mut() {
                    // Cells were routed to header while in_head was set.
                    *in_head = false;
                }
                return;
            }
            pulldown_cmark::TagEnd::TableRow => {
                if let Some(Frame::Table { rows, row, .. }) = self.stack.last_mut() {
                    let taken = std::mem::take(row);
                    if !taken.is_empty() {
                        rows.push(taken);
                    }
                }
                return;
            }
            pulldown_cmark::TagEnd::TableCell => {
                let inlines = match self.stack.pop() {
                    Some(Frame::Paragraph(v)) => v,
                    Some(other) => {
                        self.stack.push(other);
                        Vec::new()
                    }
                    None => Vec::new(),
                };
                if let Some(Frame::Table {
                    in_head,
                    header,
                    row,
                    ..
                }) = self.stack.last_mut()
                {
                    if *in_head {
                        header.push(inlines);
                    } else {
                        row.push(inlines);
                    }
                }
                return;
            }
            _ => {}
        }
        let frame = match self.stack.pop() {
            Some(f) => f,
            None => return,
        };
        match frame {
            Frame::Quote { card, blocks } => self.finish_quote(card, blocks),
            Frame::Paragraph(inlines) => {
                self.push_block(Block::Paragraph { inlines });
            }
            Frame::Heading {
                level,
                center,
                inlines,
            } => {
                self.push_block(Block::Heading {
                    level,
                    center,
                    inlines,
                });
            }
            Frame::Code { lang, code } => self.push_block(Block::Code { lang, code }),
            Frame::HtmlBuf(buf) => {
                let html = buf.trim().to_string();
                if html.is_empty() {
                    return;
                }
                // Whitelisted inline-SVG components pass through verbatim
                // (the SVG kit / AI components); everything else is escaped.
                let lower = html.to_lowercase();
                let starts_ok = lower.starts_with("<section") || lower.starts_with("<svg");
                if starts_ok && svg_embed_allowed(&lower) {
                    self.push_block(Block::SvgEmbed { html });
                } else {
                    self.push_block(Block::RawHtml { html });
                }
            }
            Frame::Swallow => {}
            Frame::Table {
                mut aligns,
                header,
                rows,
                row,
                ..
            } => {
                if !row.is_empty() {
                    // Unterminated trailing row: keep it.
                    let mut rows = rows;
                    rows.push(row);
                    self.push_block(Block::Table {
                        aligns,
                        header,
                        rows,
                    });
                    return;
                }
                if aligns.len() < header.len().max(rows.first().map(|r| r.len()).unwrap_or(0)) {
                    while aligns.len() < 8 {
                        aligns.push(Align::None);
                    }
                }
                self.push_block(Block::Table {
                    aligns,
                    header,
                    rows,
                });
            }
            Frame::List {
                ordered,
                start,
                items,
                ..
            } => {
                self.push_block(Block::List {
                    ordered,
                    start,
                    items,
                });
            }
            Frame::Item {
                checked,
                mut blocks,
                pending,
            } => {
                if !pending.is_empty() {
                    blocks.insert(0, Block::Paragraph { inlines: pending });
                }
                if let Some(Frame::List {
                    next_index, items, ..
                }) = self.stack.last_mut()
                {
                    items.push(ListItem { checked, blocks });
                    *next_index += 1;
                } else {
                    self.push_block(Block::Paragraph {
                        inlines: Vec::new(),
                    });
                }
            }
            Frame::Inline { kind, children } => {
                self.push_inline(Inline::Styled { kind, children });
            }
            Frame::Image(img) => self.push_inline(Inline::Image(img)),
        }
    }

    /// A blockquote body completed. GFM alerts arrive with `card` set and the
    /// marker already stripped; manual markers ([!COMMENT], [!KEYPOINT]) are
    /// detected from the first paragraph text.
    fn finish_quote(&mut self, card: Option<CardKind>, blocks: Vec<Block>) {
        if let Some(kind) = card {
            self.push_block(Block::Card { kind, blocks });
            return;
        }
        if let Some(Block::Paragraph { inlines }) = blocks.first() {
            if let Some(Inline::Text(t)) = inlines.first() {
                let tt = t.trim_start();
                if let Some(rest) = tt.strip_prefix("[!") {
                    if let Some(close) = rest.find(']') {
                        if let Some(kind) = CardKind::from_marker(&rest[..close]) {
                            let mut new_blocks = blocks.clone();
                            let remainder = rest[close + 1..].trim_start().to_string();
                            if let Some(Block::Paragraph { inlines }) = new_blocks.first_mut() {
                                if remainder.is_empty() {
                                    inlines.remove(0);
                                } else {
                                    inlines[0] = Inline::Text(remainder);
                                }
                            }
                            new_blocks.retain(|b| {
                                !matches!(b, Block::Paragraph { inlines } if inlines.is_empty())
                            });
                            self.push_block(Block::Card {
                                kind,
                                blocks: new_blocks,
                            });
                            return;
                        }
                    }
                }
            }
        }
        self.push_block(Block::Blockquote { blocks });
    }
}

/// Safety gate for SVG passthrough: no scripts, frames, foreignObject,
/// external resources or inline handlers. Event-handler attributes and
/// class/id stripping are the normalizer's job; this gate only rejects
/// content we never want to embed.
fn svg_embed_allowed(lower: &str) -> bool {
    !(lower.contains("<script")
        || lower.contains("<iframe")
        || lower.contains("<style")
        || lower.contains("<foreignobject")
        || lower.contains("javascript:")
        || lower.contains("onerror")
        || lower.contains("onload")
        || lower.contains("onclick=")
        || lower.contains("src=\"http")
        || lower.contains("href=\"http"))
}

fn merge_inline(v: &mut Vec<Inline>, il: Inline) {
    if let (Some(Inline::Text(last)), Inline::Text(new)) = (v.last_mut(), &il) {
        last.push_str(new);
        return;
    }
    v.push(il);
}

fn heading_num(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn card_from_gfm(k: BlockQuoteKind) -> CardKind {
    match k {
        BlockQuoteKind::Note => CardKind::Note,
        BlockQuoteKind::Tip => CardKind::Tip,
        BlockQuoteKind::Important => CardKind::Important,
        BlockQuoteKind::Warning => CardKind::Warning,
        BlockQuoteKind::Caution => CardKind::Caution,
    }
}

/// Structural post-pass over completed blocks:
/// 1. paragraph with a single image -> standalone Image;
/// 2. image followed by italic-only paragraph -> Figure with caption;
/// 3. paragraph with a single display formula -> Formula;
/// 4. paragraph exactly "[TOC]" -> Toc.
fn postprocess(blocks: &mut Vec<Block>) {
    let mut out: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut i = 0;
    while i < blocks.len() {
        match &blocks[i] {
            Block::Paragraph { inlines } if inlines.len() == 1 => match &inlines[0] {
                Inline::Image(img) => {
                    let caption = blocks.get(i + 1).and_then(|next| {
                        if let Block::Paragraph { inlines } = next {
                            if is_italic_only(inlines) {
                                return Some(inlines.clone());
                            }
                        }
                        None
                    });
                    match caption {
                        Some(cap) => {
                            out.push(Block::Figure {
                                image: img.clone(),
                                caption: cap,
                            });
                            i += 2;
                        }
                        None => {
                            out.push(Block::Image(img.clone()));
                            i += 1;
                        }
                    }
                }
                Inline::Math {
                    latex,
                    display: true,
                } => {
                    out.push(Block::Formula {
                        latex: latex.clone(),
                    });
                    i += 1;
                }
                Inline::Text(t) if t.trim() == "[TOC]" => {
                    out.push(Block::Toc);
                    i += 1;
                }
                _ => {
                    out.push(blocks[i].clone());
                    i += 1;
                }
            },
            Block::Paragraph { .. } => {
                out.push(blocks[i].clone());
                i += 1;
            }
            Block::Blockquote { blocks: _inner } | Block::Card { blocks: _inner, .. } => {
                let mut b = blocks.remove(i);
                if let Block::Blockquote { blocks: inner } | Block::Card { blocks: inner, .. } =
                    &mut b
                {
                    postprocess(inner);
                }
                out.push(b);
            }
            Block::List { items: _, .. } => {
                let mut b = blocks.remove(i);
                if let Block::List { items, .. } = &mut b {
                    for it in items {
                        postprocess(&mut it.blocks);
                    }
                }
                out.push(b);
            }
            _ => {
                out.push(blocks[i].clone());
                i += 1;
            }
        }
    }
    *blocks = out;
}

fn is_italic_only(inlines: &[Inline]) -> bool {
    !inlines.is_empty()
        && inlines.iter().all(|i| match i {
            Inline::Styled {
                kind: InlineKind::Emphasis,
                children,
            } => children.iter().all(|c| matches!(c, Inline::Text(_))),
            Inline::Text(t) => t.trim().is_empty(),
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_blocks() {
        let blocks = parse_markdown("# Title\n\nHello **world**\n\n- a\n- b\n");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&blocks[1], Block::Paragraph { .. }));
        assert!(matches!(&blocks[2], Block::List { ordered: false, .. }));
    }

    #[test]
    fn gfm_alert_card() {
        let blocks = parse_markdown("> [!NOTE]\n> content here\n");
        assert!(matches!(
            &blocks[0],
            Block::Card {
                kind: CardKind::Note,
                ..
            }
        ));
    }

    #[test]
    fn manual_marker_cards() {
        let blocks = parse_markdown("> [!COMMENT]\n> tell me what you think\n");
        assert!(matches!(
            &blocks[0],
            Block::Card {
                kind: CardKind::Comment,
                ..
            }
        ));
        let blocks = parse_markdown("> [!KEYPOINT] inline key text\n");
        match &blocks[0] {
            Block::Card {
                kind: CardKind::Keypoint,
                blocks,
            } => {
                assert!(matches!(&blocks[0], Block::Paragraph { .. }));
            }
            other => panic!("expected keypoint card, got {:?}", other),
        }
    }

    #[test]
    fn toc_and_formula() {
        let blocks = parse_markdown("[TOC]\n\n$$E = mc^2$$\n");
        assert!(matches!(blocks[0], Block::Toc));
        assert!(matches!(blocks[1], Block::Formula { .. }));
    }

    #[test]
    fn figure_caption() {
        let blocks = parse_markdown("![alt](https://example.com/x.png)\n\n*caption text*\n");
        match &blocks[0] {
            Block::Figure { caption, .. } => {
                assert!(is_italic_only(caption));
            }
            other => panic!("expected figure, got {:?}", other),
        }
    }

    #[test]
    fn task_list() {
        let blocks = parse_markdown("- [x] done\n- [ ] todo\n");
        match &blocks[0] {
            Block::List { items, .. } => {
                assert_eq!(items[0].checked, Some(true));
                assert_eq!(items[1].checked, Some(false));
            }
            other => panic!("expected list, got {:?}", other),
        }
    }

    #[test]
    fn table_parse() {
        let blocks = parse_markdown("| a | b |\n|---|:-:|\n| 1 | 2 |\n");
        match &blocks[0] {
            Block::Table {
                aligns,
                header,
                rows,
            } => {
                assert_eq!(aligns[1], Align::Center);
                assert_eq!(header.len(), 2);
                assert_eq!(rows.len(), 1);
            }
            other => panic!("expected table, got {:?}", other),
        }
    }
}
