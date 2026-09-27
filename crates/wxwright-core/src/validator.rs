//! Validator: static compliance checking against the official editor spec
//! (PRD 5.3 "检测" column). Streaming single pass, no browser needed.
//! Exit-code semantics align with the official verify CLI: 0 pass / 1 block
//! violations (warnings do not fail).

use std::cell::RefCell;
use std::rc::Rc;

use lol_html::{element, DocumentContentHandlers, Settings};

use crate::htmlutil::{contrast_ratio, parse_style, px_value};
use crate::walker::{self, Stack};

pub const MAX_VIOLATIONS: usize = 500;

/// Blocking findings get their own, much higher ceiling. The warn budget must
/// never be able to hide a block: a report truncated at 500 entries made
/// `validate` answer `compliant: true` (exit 0) for an article that contained
/// a `<script>`, because the blocking finding was simply never recorded.
pub const MAX_BLOCK_VIOLATIONS: usize = 4000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Violation {
    pub rule_id: String,
    /// "block" or "warn".
    pub severity: String,
    /// Machine-readable English message (never localized, PRD 3.7-B).
    pub message: String,
    /// Snippet of the offending element.
    pub node: String,
    pub fixable: bool,
}

impl Violation {
    pub fn is_block(&self) -> bool {
        self.severity == "block"
    }
}

#[derive(Default)]
struct VState {
    stack: Stack,
    violations: Vec<Violation>,
}

impl VState {
    fn add(&mut self, rule_id: &str, message: String, node: &str) {
        let info = crate::rules::rule_info(rule_id);
        let is_block = info.map(|i| i.is_block()).unwrap_or(false);
        // Warns are dropped once the budget is spent; blocks are not, so a
        // noisy document can never mask the finding that actually blocks it.
        let cap = if is_block {
            MAX_BLOCK_VIOLATIONS
        } else {
            MAX_VIOLATIONS
        };
        if self.violations.len() >= cap {
            return;
        }
        self.violations.push(Violation {
            rule_id: rule_id.to_string(),
            severity: if is_block {
                "block".to_string()
            } else {
                "warn".to_string()
            },
            message,
            node: truncate(node, 120),
            fixable: info.map(|i| i.auto_fixable).unwrap_or(false),
        });
    }

    fn on_element(&mut self, el: &mut lol_html::html_content::Element) -> bool {
        let tag = el.tag_name().to_ascii_lowercase();
        let node_hint = node_hint(el);
        let ignore = el.has_attribute("data-ignore-width") || self.stack.in_ignore_subtree();

        // Payload hygiene: blocking elements.
        if matches!(
            tag.as_str(),
            "script"
                | "style"
                | "iframe"
                | "form"
                | "link"
                | "meta"
                | "object"
                | "embed"
                | "noscript"
        ) {
            self.add(
                "HYGIENE",
                format!("<{}> is not allowed in the payload", tag),
                &node_hint,
            );
        }
        if tag == "font" && el.has_attribute("face") {
            self.add("R-3.1", "<font face> sets a font-family".into(), &node_hint);
        }

        // img-specific rules.
        if tag == "img" {
            if el.has_attribute("style") {
                let decls = parse_style(&el.get_attribute("style").unwrap_or_default());
                if decls.iter().any(|(p, v)| {
                    p == "opacity" && v.trim().parse::<f64>().map(|f| f < 0.05).unwrap_or(false)
                }) {
                    self.add(
                        "R-1.1",
                        "img with opacity:0 (hidden-image trick) blocks re-editing in the MP editor".into(),
                        &node_hint,
                    );
                }
            }
            if !el.has_attribute("data-w") {
                self.add(
                    "R-1.4w",
                    "img is missing data-w (original pixel width); load timeouts will be misreported".into(),
                    &node_hint,
                );
            }
            if let Some(alt) = el.get_attribute("alt") {
                if alt.chars().count() > 30 {
                    self.add(
                        "R-4.3",
                        "img alt carries long plain text; images should not carry text content"
                            .into(),
                        &node_hint,
                    );
                }
            }
        }

        // SVG with background image: half of the R-1.1 trick.
        if tag == "svg" {
            if let Some(style) = el.get_attribute("style") {
                if parse_style(&style).iter().any(|(p, v)| {
                    (p == "background" || p == "background-image") && v.contains("url(")
                }) {
                    self.add(
                        "R-1.1",
                        "svg carries a background-image; combined with a transparent img this blocks re-editing".into(),
                        &node_hint,
                    );
                }
            }
        }

        // R-1.7.
        if tag == "animate" || tag == "set" {
            if let Some(begin) = el.get_attribute("begin") {
                if begin.contains("touchstart") && !begin.contains("click") {
                    self.add(
                        "R-1.7",
                        format!(
                            "svg <{}> begin listens to touchstart only (broken on PC)",
                            tag
                        ),
                        &node_hint,
                    );
                }
            }
        }

        let decls = el
            .get_attribute("style")
            .map(|s| parse_style(&s))
            .unwrap_or_default();

        // R-3.1: font-family anywhere - except inside <svg> subtrees
        // (SVG typesetting keeps font-family; official practice).
        let in_svg_ctx = tag == "svg" || self.stack.nodes.iter().any(|n| n.in_svg);
        if decls.iter().any(|(p, _)| p == "font-family") && !in_svg_ctx {
            self.add(
                "R-3.1",
                "font-family declared (breaks editor/mobile consistency)".into(),
                &node_hint,
            );
        }

        // R-1.2.
        if decls.iter().any(|(p, v)| {
            p == "caret-color"
                && (v.to_ascii_lowercase().contains("transparent")
                    || v.replace(' ', "") == "rgba(0,0,0,0)")
        }) {
            self.add(
                "R-1.2",
                "caret-color: transparent is forbidden".into(),
                &node_hint,
            );
        }

        // R-1.4: fixed px width / large margin offsets.
        if !ignore {
            for (p, v) in &decls {
                if (p == "width" || p == "max-width") && px_value(v).is_some() {
                    self.add(
                        "R-1.4",
                        format!("fixed pixel {} breaks responsive layout", p),
                        &node_hint,
                    );
                }
                if (p == "margin-left" || p == "margin-right")
                    && px_value(v).map(|f| f > 339.0).unwrap_or(false)
                {
                    self.add(
                        "R-1.4",
                        format!("{} pushes content out of the mobile viewport", p),
                        &node_hint,
                    );
                }
            }
        }

        // R-1.6.
        for (p, v) in &decls {
            if p == "text-align" && (v.trim() == "start" || v.trim() == "end") {
                self.add(
                    "R-1.6",
                    format!("text-align: {} is not iOS-safe", v.trim()),
                    &node_hint,
                );
            }
        }

        // R-4.2: absolute positioning on layout containers.
        if matches!(tag.as_str(), "section" | "div" | "p" | "span") {
            if let Some((_, v)) = decls.iter().find(|(p, _)| p == "position") {
                if v.contains("absolute") || v.contains("fixed") {
                    self.add(
                        "R-4.2",
                        "absolute positioning breaks the Dark Mode traversal order".into(),
                        &node_hint,
                    );
                }
            }
        }

        // Structural rules that need ancestors: check BEFORE pushing self.
        if walker::is_block_tag(&tag) && self.stack.nearest_leaf_ancestor().is_some() {
            self.add(
                "R-2.2",
                "block-level element inside span[leaf]".into(),
                &node_hint,
            );
        }
        if !walker::nodeleaf_child_allowed(&tag) && self.stack.nearest_nodeleaf_ancestor().is_some()
        {
            self.add(
                "R-2.3",
                "non-official element inside section[nodeleaf]".into(),
                &node_hint,
            );
        }

        if el.can_have_content() {
            let parent_in_svg = self.stack.nodes.last().map(|n| n.in_svg).unwrap_or(false);
            self.stack.push(walker::NodeInfo {
                tag: tag.clone(),
                decls,
                leaf: el.has_attribute("leaf"),
                nodeleaf: el.has_attribute("nodeleaf"),
                ignore: el.has_attribute("data-ignore-width"),
                has_text: false,
                direct_text: false,
                in_svg: parent_in_svg || tag == "svg",
                child_elems: 0,
                leaf_index: None,
                nodeleaf_index: None,
            });
            return true;
        }
        false
    }

    fn on_text(&mut self, chunk: &mut lol_html::html_content::TextChunk) {
        if chunk.text_type() != lol_html::html_content::TextType::Data {
            return;
        }
        if chunk.as_str().trim().is_empty() {
            return;
        }
        // Only the innermost open element carries text directly; ancestor
        // containers inherit it when the child pops (Stack::pop).
        if let Some(top) = self.stack.top_mut() {
            top.has_text = true;
            top.direct_text = true;
        }
        if self.stack.nearest_nodeleaf_ancestor().is_some() {
            self.add(
                "R-2.3",
                "text directly inside section[nodeleaf]".into(),
                "<text>",
            );
        }
    }

    /// Drain remaining open elements at document end (unclosed tags).
    fn on_document_end(&mut self) {
        while !self.stack.nodes.is_empty() {
            self.check_element_end(self.stack.nodes.len() - 1, true);
        }
    }

    fn on_end(&mut self, end: &mut lol_html::html_content::EndTag) {
        let name = end.name().to_ascii_lowercase();
        if walker::is_void_tag(&name) {
            return;
        }
        if let Some(pos) = self.stack.nodes.iter().rposition(|n| n.tag == name) {
            while self.stack.nodes.len() > pos {
                let idx = self.stack.nodes.len() - 1;
                self.check_element_end(idx, false);
            }
        }
    }

    /// Run text-conditional checks for stack[idx], then pop it.
    fn check_element_end(&mut self, _idx: usize, _draining: bool) {
        let e = match self.stack.nodes.last() {
            Some(n) => n.clone(),
            None => return,
        };
        let node = format!(
            "<{} style=\"{}\">",
            e.tag,
            crate::htmlutil::build_style(&e.decls)
        );
        if e.has_text {
            if e.tag == "pre" {
                self.add(
                    "R-1.8",
                    "plain text inside <pre> truncates on mobile".into(),
                    &node,
                );
            }
            if walker::is_text_tag(&e.tag) && e.tag != "pre" {
                if walker::Stack::line_height_violation(&e.decls).is_some() {
                    self.add(
                        "R-1.3",
                        "line-height smaller than font size stacks text lines on mobile".into(),
                        &node,
                    );
                }
                if e.decls.iter().any(|(p, v)| {
                    (p == "background" || p == "background-image") && v.contains("gradient(")
                }) {
                    self.add(
                        "R-4.1.2",
                        "gradient background under text is flattened in Dark Mode".into(),
                        &node,
                    );
                }
                let color = e
                    .decls
                    .iter()
                    .find(|(p, _)| p == "color")
                    .map(|(_, v)| v.clone());
                let bg = e
                    .decls
                    .iter()
                    .find(|(p, _)| p == "background-color")
                    .map(|(_, v)| v.clone());
                if let (Some(c), Some(b)) = (color, bg) {
                    if let Some(ratio) = contrast_ratio(&c, &b) {
                        if ratio < 3.0 {
                            self.add(
                                "R-4.1.1",
                                format!(
                                    "contrast ratio {:.1}:1 is too low; Dark Mode will rewrite it",
                                    ratio
                                ),
                                &node,
                            );
                        } else if ratio > 19.5 {
                            self.add(
                                "R-4.1.1",
                                format!(
                                    "contrast ratio {:.1}:1 is suspiciously high (invisible text?)",
                                    ratio
                                ),
                                &node,
                            );
                        }
                    }
                }
            }
            if e.tag == "svg" {
                self.add(
                    "R-4.4",
                    "svg contains text content; Dark Mode does not convert SVG".into(),
                    &node,
                );
            }
            let height = e
                .decls
                .iter()
                .find(|(p, _)| p == "height")
                .and_then(|(_, v)| px_value(v));
            if let Some(h) = height {
                if e.tag != "svg" && e.tag != "iframe" {
                    if h <= 0.5 {
                        self.add(
                            "R-1.5.1",
                            "text container with height:0 is invisible on mobile".into(),
                            &node,
                        );
                    } else if h < 40.0 {
                        let clipped = e.decls.iter().any(|(p, v)| {
                            p == "overflow" && (v.contains("hidden") || v.contains("clip"))
                        });
                        if clipped {
                            self.add(
                                "R-1.5.2",
                                "fixed small height clips text content".into(),
                                &node,
                            );
                        }
                    }
                }
            }
        }
        // R-2.1: identical wrapper chains (single element child per level).
        if e.child_elems <= 1 {
            let depth = self.stack.identical_chain_depth();
            if depth > 10 {
                self.add(
                    "R-2.1",
                    format!("identical wrapper chain is {} levels deep (max 10)", depth),
                    &node,
                );
            }
        }
        self.stack.pop();
    }
}

fn node_hint(el: &lol_html::html_content::Element) -> String {
    let mut s = format!("<{}", el.tag_name());
    if let Some(style) = el.get_attribute("style") {
        s.push_str(&format!(" style=\"{}\"", style));
    }
    if el.tag_name() == "img" {
        if let Some(src) = el.get_attribute("src") {
            s.push_str(&format!(" src=\"{}\"", truncate(&src, 60)));
        }
    }
    s.push('>');
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{}...", t)
    }
}

/// Validate HTML and return all findings (blocks + warnings).
pub fn validate_html(html: &str) -> Vec<Violation> {
    let shared = Rc::new(RefCell::new(VState::default()));
    let s_el = shared.clone();
    let s_txt = shared.clone();
    let s_end = shared.clone();
    let _ = lol_html::rewrite_str(
        html,
        Settings {
            element_content_handlers: vec![element!("*", move |el| {
                let pushed = s_el.borrow_mut().on_element(el);
                if pushed {
                    if let Some(handlers) = el.end_tag_handlers() {
                        let st = s_el.clone();
                        handlers.push(Box::new(move |end| {
                            st.borrow_mut().on_end(end);
                            Ok(())
                        }));
                    }
                }
                Ok(())
            })],
            document_content_handlers: vec![
                DocumentContentHandlers::default().text(move |chunk| {
                    s_txt.borrow_mut().on_text(chunk);
                    Ok(())
                }),
                DocumentContentHandlers::default().end(move |_doc_end| {
                    s_end.borrow_mut().on_document_end();
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
    );
    let violations = shared.borrow().violations.clone();
    violations
}

/// True when there are no block-level violations.
pub fn is_compliant(html: &str) -> bool {
    validate_html(html).iter().all(|v| !v.is_block())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 600 warnings, then a blocking payload. Regression: the flat 500-entry
    /// cap dropped the HYGIENE block, so `validate` answered
    /// `compliant: true` / exit 0 for an article containing `<script>`.
    #[test]
    fn the_warn_cap_cannot_hide_a_blocking_violation() {
        let mut html = String::new();
        for _ in 0..(MAX_VIOLATIONS + 100) {
            html.push_str("<section style=\"position: absolute\">x</section>");
        }
        html.push_str("<script>alert(1)</script>");

        let v = validate_html(&html);
        assert!(
            v.iter().any(|x| x.rule_id == "HYGIENE" && x.is_block()),
            "the blocking HYGIENE finding must survive the warn cap (got {} findings)",
            v.len()
        );
        assert!(
            !is_compliant(&html),
            "a <script> payload is never compliant"
        );
    }

    /// The cap still bounds the warn list, so a pathological document cannot
    /// grow the report without limit.
    #[test]
    fn warns_are_still_capped() {
        let mut html = String::new();
        for _ in 0..(MAX_VIOLATIONS * 3) {
            html.push_str("<section style=\"position: absolute\">x</section>");
        }
        let v = validate_html(&html);
        assert!(
            v.iter().filter(|x| !x.is_block()).count() <= MAX_VIOLATIONS,
            "warn count must stay bounded"
        );
    }

    /// A clean document stays clean - the severity split must not invent
    /// findings.
    #[test]
    fn compliant_html_reports_nothing_blocking() {
        let html = "<section style=\"line-height: 1.6;\"><span leaf=\"1\" style=\"color: #333333; line-height: 1.6;\">ok</span></section>";
        assert!(is_compliant(html), "got: {:?}", validate_html(html));
    }
}
