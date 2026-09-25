//! Shared streaming-DOM walk machinery for the normalizer and validator.
//! A hand-maintained element stack fed by lol_html document-level handlers:
//! element start, text chunks, element end. Zero regex (PRD 5.7-B).

pub const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub const BLOCK_TAGS: &[&str] = &[
    "section",
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "blockquote",
    "hr",
    "pre",
    "figure",
    "figcaption",
];

/// Tags that typically carry visible text (used for R-1.3/1.5 checks).
pub const TEXT_TAGS: &[&str] = &[
    "p",
    "span",
    "div",
    "section",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "td",
    "th",
    "li",
    "blockquote",
    "a",
    "em",
    "strong",
    "b",
    "i",
    "u",
    "label",
    "button",
    "figcaption",
    "sup",
    "sub",
    "font",
];

/// Children allowed inside section[nodeleaf] (official components or img, R-2.3).
pub fn nodeleaf_child_allowed(tag: &str) -> bool {
    tag == "img"
        || tag == "br"
        || tag.starts_with("mp")
        || tag.starts_with("qq")
        || tag == "mlivecard"
        || tag == "wx-open-launch-app"
        || tag == "wx-open-launch-weapp"
}

pub fn is_void_tag(t: &str) -> bool {
    VOID_TAGS.contains(&t)
}

pub fn is_block_tag(t: &str) -> bool {
    BLOCK_TAGS.contains(&t)
}

pub fn is_text_tag(t: &str) -> bool {
    TEXT_TAGS.contains(&t)
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub tag: String,
    pub decls: Vec<(String, String)>,
    pub leaf: bool,
    pub nodeleaf: bool,
    pub ignore: bool,
    /// Subtree contains visible text.
    pub has_text: bool,
    /// Text directly inside this element (not via descendants).
    pub direct_text: bool,
    /// True when inside an <svg> subtree: SVG keeps its id/class/url()
    /// references and font-family (official MP SVG-typesetting practice),
    /// so hygiene and R-3.1 are scoped out there.
    pub in_svg: bool,
    pub child_elems: usize,
    /// Occurrence index among span[leaf] elements (for index-based strip pass).
    pub leaf_index: Option<usize>,
    /// Occurrence index among section[nodeleaf] elements.
    pub nodeleaf_index: Option<usize>,
}

impl NodeInfo {
    pub fn style_sig(&self) -> String {
        let mut s = String::new();
        for (p, v) in &self.decls {
            s.push_str(p);
            s.push(':');
            s.push_str(v);
            s.push(';');
        }
        s
    }
}

#[derive(Debug, Default)]
pub struct Stack {
    pub nodes: Vec<NodeInfo>,
    leaf_counter: usize,
    nodeleaf_counter: usize,
}

impl Stack {
    pub fn new() -> Self {
        Stack::default()
    }

    pub fn push(&mut self, mut info: NodeInfo) {
        if info.leaf {
            info.leaf_index = Some(self.leaf_counter);
            self.leaf_counter += 1;
        }
        if info.nodeleaf {
            info.nodeleaf_index = Some(self.nodeleaf_counter);
            self.nodeleaf_counter += 1;
        }
        self.nodes.push(info);
    }

    pub fn pop(&mut self) -> Option<NodeInfo> {
        let n = self.nodes.pop()?;
        if let Some(parent) = self.nodes.last_mut() {
            parent.child_elems += 1;
            parent.has_text = parent.has_text || n.has_text;
        }
        Some(n)
    }

    pub fn top_mut(&mut self) -> Option<&mut NodeInfo> {
        self.nodes.last_mut()
    }

    pub fn nearest_leaf_ancestor(&self) -> Option<&NodeInfo> {
        self.nodes.iter().rev().find(|n| n.leaf)
    }

    pub fn nearest_nodeleaf_ancestor(&self) -> Option<&NodeInfo> {
        self.nodes.iter().rev().find(|n| n.nodeleaf)
    }

    pub fn in_ignore_subtree(&self) -> bool {
        self.nodes.iter().any(|n| n.ignore)
    }

    /// Depth of an identical (tag, style, single-child, no-text) wrapper
    /// chain ending at the top of the stack (R-2.1). Computed when the top
    /// element closes: ancestors in the chain each have zero closed children
    /// (their only child is the chain below) and carry no text.
    pub fn identical_chain_depth(&self) -> u32 {
        let n = self.nodes.len();
        if n == 0 {
            return 0;
        }
        let top = &self.nodes[n - 1];
        let sig = top.style_sig();
        let mut depth = 1u32;
        let mut i = n - 1;
        while i > 0 {
            let parent = &self.nodes[i - 1];
            if parent.tag == top.tag
                && parent.style_sig() == sig
                && parent.child_elems == 0
                && !parent.direct_text
            {
                depth += 1;
                i -= 1;
            } else {
                break;
            }
        }
        depth
    }

    /// Effective line-height in px given the element's own font-size (approx:
    /// parent font-size is unknown; unitless/em resolve against own size).
    pub fn line_height_violation(decls: &[(String, String)]) -> Option<(String, String)> {
        let font_px = decls
            .iter()
            .find(|(p, _)| p == "font-size")
            .and_then(|(_, v)| crate::htmlutil::px_value(v));
        let lh = decls.iter().find(|(p, _)| p == "line-height")?;
        let v = lh.1.trim().to_string();
        if v.ends_with("px") {
            let lh_px = crate::htmlutil::px_value(&v)?;
            let f = font_px.unwrap_or(16.0);
            if lh_px < f - 0.01 {
                return Some((lh.0.clone(), lh.1.clone()));
            }
            return None;
        }
        // unitless / em / % : resolve against font size, default 16px.
        let mult = if let Ok(n) = v.parse::<f64>() {
            n
        } else if let Some(em) = v.strip_suffix("em") {
            em.trim().parse::<f64>().ok()?
        } else if let Some(pc) = v.strip_suffix('%') {
            pc.trim().parse::<f64>().ok()? / 100.0
        } else {
            return None;
        };
        let f = font_px.unwrap_or(16.0);
        if mult * f < f - 0.01 || mult < 1.0 {
            return Some((lh.0.clone(), lh.1.clone()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(tag: &str, style: &str) -> NodeInfo {
        NodeInfo {
            tag: tag.into(),
            decls: crate::htmlutil::parse_style(style),
            leaf: false,
            nodeleaf: false,
            ignore: false,
            has_text: false,
            direct_text: false,
            in_svg: false,
            child_elems: 0,
            leaf_index: None,
            nodeleaf_index: None,
        }
    }

    #[test]
    fn chain_depth() {
        let mut s = Stack::new();
        for _ in 0..12 {
            s.push(node("section", "margin: 0;"));
        }
        assert_eq!(s.identical_chain_depth(), 12);
        s.nodes[5].direct_text = true;
        // Chain above the text node still counts from the top run.
        assert_eq!(s.identical_chain_depth(), 6);
    }

    #[test]
    fn lh_violation() {
        assert!(Stack::line_height_violation(&crate::htmlutil::parse_style(
            "font-size: 16px; line-height: 12px"
        ))
        .is_some());
        assert!(Stack::line_height_violation(&crate::htmlutil::parse_style(
            "font-size: 16px; line-height: 1.5"
        ))
        .is_none());
    }
}
