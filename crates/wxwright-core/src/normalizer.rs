//! Normalizer: streaming rewrite passes that auto-fix official-rule
//! violations (PRD 5.3 "归一化" column) plus payload hygiene (PRD 3.5-1).
//!
//! Pass 1: attribute-level fixes + hygiene + structural violation recording.
//! Pass 2: strip leaf/nodeleaf attributes at the recorded occurrence indices.
//! Idempotent: normalizing compliant HTML is a no-op.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use lol_html::{element, DocumentContentHandlers, Settings};

use crate::error::Result;
use crate::htmlutil::{build_style, parse_style, px_to_percent, px_value};
use crate::walker::{self, NodeInfo, Stack};

#[derive(Debug, Clone, serde::Serialize)]
pub struct FixRecord {
    pub rule_id: String,
    pub count: usize,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NormalizeOptions {
    /// Also apply Dark Mode conversions (R-4.1.2 gradient -> solid).
    pub fix_dark: bool,
}

pub struct NormalizeResult {
    pub html: String,
    pub fixes: Vec<FixRecord>,
}

#[derive(Default)]
struct Pass1 {
    fix_dark: bool,
    stack: Stack,
    records: Vec<(String, String, usize)>, // (rule, detail, count)
    leaf_strip: BTreeSet<usize>,
    nodeleaf_strip: BTreeSet<usize>,
    leaf_seq: usize,
    nodeleaf_seq: usize,
}

impl Pass1 {
    fn record(&mut self, rule: &str, detail: &str) {
        if let Some(entry) = self
            .records
            .iter_mut()
            .find(|(r, d, _)| r == rule && d == detail)
        {
            entry.2 += 1;
        } else {
            self.records.push((rule.to_string(), detail.to_string(), 1));
        }
    }

    fn on_element(&mut self, el: &mut lol_html::html_content::Element) -> bool {
        let tag = el.tag_name().to_ascii_lowercase();

        // ---- payload hygiene ----
        match tag.as_str() {
            "script" | "style" | "link" | "meta" | "iframe" | "noscript" | "object" | "embed" => {
                el.remove();
                self.record("HYGIENE", &format!("<{}> removed", tag));
                return false;
            }
            "form" => {
                el.remove_and_keep_content();
                self.record("HYGIENE", "<form> unwrapped");
            }
            _ => {}
        }

        // SVG subtrees keep id/class (clipPath url(#id) references) and
        // font-family (official SVG-typesetting practice).
        let in_svg_ctx = tag == "svg" || self.stack.nodes.iter().any(|n| n.in_svg);
        if !in_svg_ctx {
            for attr in ["class", "id"] {
                if el.has_attribute(attr) {
                    el.remove_attribute(attr);
                    self.record(
                        "HYGIENE",
                        &format!("attribute {} removed from <{}>", attr, tag),
                    );
                }
            }
        }
        let on_attrs: Vec<String> = el
            .attributes()
            .iter()
            .filter_map(|a| {
                let name = a.name().to_ascii_lowercase();
                if name.starts_with("on") || name == "srcdoc" {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        for name in on_attrs {
            el.remove_attribute(name.as_str());
            self.record("HYGIENE", &format!("event attribute {} removed", name));
        }
        if tag == "a" {
            if let Some(href) = el.get_attribute("href") {
                if href.trim().to_ascii_lowercase().starts_with("javascript:") {
                    let _ = el.set_attribute("href", "#");
                    self.record("HYGIENE", "javascript: href neutralized");
                }
            }
        }

        // ---- R-1.7: SVG animate begin must include click ----
        if tag == "animate" || tag == "set" {
            if let Some(begin) = el.get_attribute("begin") {
                if begin.contains("touchstart") && !begin.contains("click") {
                    let _ = el.set_attribute("begin", "touchstart; click");
                    self.record("R-1.7", &format!("<{}> begin gained click", tag));
                }
            }
        }

        // ---- R-1.8: <pre> -> section with wrapping whitespace ----
        if tag == "pre" {
            let _ = el.set_tag_name("section");
            let mut style = el.get_attribute("style").unwrap_or_default();
            if !style.contains("white-space") {
                if !style.trim().is_empty() {
                    style.push_str("; ");
                }
                style.push_str("white-space: pre-wrap; overflow-x: auto;");
                let _ = el.set_attribute("style", &style);
            }
            self.record(
                "R-1.8",
                "<pre> converted to section (white-space: pre-wrap)",
            );
            return self.finish_element("section", parse_style(&style), el);
        }

        // ---- attribute-level rule fixes ----
        let mut decls = el
            .get_attribute("style")
            .map(|s| parse_style(&s))
            .unwrap_or_default();
        let before = decls.clone();
        let mut notes: Vec<&str> = Vec::new();

        // R-3.1: strip every font-family declaration (even !important) -
        // except inside <svg> subtrees (SVG typesetting keeps them).
        let in_svg_ctx2 = tag == "svg" || self.stack.nodes.iter().any(|n| n.in_svg);
        let had_ff = decls.iter().any(|(p, _)| p == "font-family");
        if !in_svg_ctx2 {
            decls.retain(|(p, _)| p != "font-family");
        }
        if had_ff && !in_svg_ctx2 {
            notes.push("R-3.1");
        }

        // R-1.2: caret-color transparent.
        if decls.iter().any(|(p, v)| {
            p == "caret-color"
                && (v.to_ascii_lowercase().contains("transparent")
                    || v.replace(' ', "") == "rgba(0,0,0,0)")
        }) {
            decls.retain(|(p, _)| p != "caret-color");
            notes.push("R-1.2");
        }

        // R-1.1: opacity:0 on img (hidden-image trick).
        if tag == "img" {
            if let Some((_, v)) = decls.iter().find(|(p, _)| p == "opacity") {
                if v.trim().parse::<f64>().map(|f| f < 0.05).unwrap_or(false) {
                    decls.retain(|(p, _)| p != "opacity");
                    notes.push("R-1.1");
                }
            }
        }

        // R-1.4: fixed px widths -> percentages (width-type only, min-width exempt).
        if !el.has_attribute("data-ignore-width") && !self.stack.in_ignore_subtree() {
            for prop in ["width", "max-width"] {
                if let Some((_, v)) = decls.iter().find(|(p, _)| p == prop) {
                    if let Some(px) = px_value(v) {
                        let pct = px_to_percent(px);
                        if let Some(slot) = decls.iter_mut().find(|(p, _)| p == prop) {
                            slot.1 = pct;
                        }
                        notes.push("R-1.4");
                    }
                }
            }
        }

        // R-1.6: text-align start/end.
        for (p, v) in decls.iter_mut() {
            if p == "text-align" {
                let nv = match v.trim() {
                    "start" => Some("left".to_string()),
                    "end" => Some("right".to_string()),
                    _ => None,
                };
                if let Some(nv) = nv {
                    *v = nv;
                    notes.push("R-1.6");
                }
            }
        }

        // R-1.3: line-height below font size -> 1.5.
        if walker::Stack::line_height_violation(&decls).is_some() {
            if let Some(slot) = decls.iter_mut().find(|(p, _)| p == "line-height") {
                slot.1 = "1.5".to_string();
                notes.push("R-1.3");
            }
        }

        // R-1.5.1 / R-1.5.2: fixed heights that hide or clip text.
        if walker::is_text_tag(&tag) {
            let height_zero = decls
                .iter()
                .any(|(p, v)| p == "height" && px_value(v).map(|f| f <= 0.5).unwrap_or(false));
            if height_zero {
                decls.retain(|(p, _)| p != "height");
                notes.push("R-1.5.1");
            }
            let small_clip = decls.iter().any(|(p, v)| {
                p == "height" && px_value(v).map(|f| f < 40.0 && f > 0.5).unwrap_or(false)
            }) && decls
                .iter()
                .any(|(p, v)| p == "overflow" && (v.contains("hidden") || v.contains("clip")));
            if small_clip {
                decls.retain(|(p, _)| p != "height" && p != "overflow");
                notes.push("R-1.5.2");
            }
        }

        // R-4.1.2 (opt-in): gradient under text -> first solid color.
        if self.fix_dark && walker::is_text_tag(&tag) {
            for prop in ["background", "background-image"] {
                if let Some(slot) = decls.iter_mut().find(|(p, _)| p == prop) {
                    if slot.1.contains("gradient(") {
                        if let Some(color) = first_gradient_color(&slot.1) {
                            slot.1 = color;
                            notes.push("R-4.1.2");
                        }
                    }
                }
            }
        }

        if before != decls {
            let _ = el.set_attribute("style", &build_style(&decls));
        }
        for n in notes {
            self.record(n, &format!("fixed style on <{}>", tag));
        }

        self.finish_element(&tag, decls, el)
    }

    /// Push onto the stack, then run structural checks that need ancestors.
    /// Returns true when a stack entry was pushed (caller registers the
    /// end-tag popper).
    fn finish_element(
        &mut self,
        tag: &str,
        decls: Vec<(String, String)>,
        el: &lol_html::html_content::Element,
    ) -> bool {
        // Structural violations are detected against ancestors only.
        if walker::is_block_tag(tag) {
            let leaf_hit = self
                .stack
                .nodes
                .iter()
                .rev()
                .find(|n| n.leaf)
                .and_then(|n| n.leaf_index);
            if let Some(idx) = leaf_hit {
                self.leaf_strip.insert(idx);
                self.record("R-2.2", "block element inside span[leaf]: leaf stripped");
            }
        }
        if !walker::nodeleaf_child_allowed(tag) {
            let nl_hit = self
                .stack
                .nodes
                .iter()
                .rev()
                .find(|n| n.nodeleaf)
                .and_then(|n| n.nodeleaf_index);
            if let Some(idx) = nl_hit {
                self.nodeleaf_strip.insert(idx);
                self.record(
                    "R-2.3",
                    "non-official child inside section[nodeleaf]: nodeleaf stripped",
                );
            }
        }

        if el.can_have_content() {
            let parent_in_svg = self.stack.nodes.last().map(|n| n.in_svg).unwrap_or(false);
            let mut info = NodeInfo {
                tag: tag.to_string(),
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
            };
            // Assign occurrence indices for the strip pass.
            if info.leaf {
                info.leaf_index = Some(self.leaf_seq);
                self.leaf_seq += 1;
            }
            if info.nodeleaf {
                info.nodeleaf_index = Some(self.nodeleaf_seq);
                self.nodeleaf_seq += 1;
            }
            self.stack.nodes.push(info);
            return true;
        }
        false
    }

    fn on_text(&mut self, chunk: &mut lol_html::html_content::TextChunk) {
        if chunk.text_type() != lol_html::html_content::TextType::Data {
            return;
        }
        let meaningful = !chunk.as_str().trim().is_empty();
        if meaningful {
            if let Some(top) = self.stack.nodes.last_mut() {
                top.has_text = true;
                top.direct_text = true;
            }
            if let Some(idx) = self
                .stack
                .nodes
                .iter()
                .rev()
                .find(|n| n.nodeleaf)
                .and_then(|n| n.nodeleaf_index)
            {
                if self.nodeleaf_strip.insert(idx) {
                    self.record("R-2.3", "text inside section[nodeleaf]: nodeleaf stripped");
                }
            }
        }
    }

    fn on_end(&mut self, end: &mut lol_html::html_content::EndTag) {
        let name = end.name().to_ascii_lowercase();
        if walker::is_void_tag(&name) {
            return;
        }
        if let Some(pos) = self.stack.nodes.iter().rposition(|n| n.tag == name) {
            while self.stack.nodes.len() > pos {
                self.stack.pop();
            }
        }
    }
}

fn first_gradient_color(v: &str) -> Option<String> {
    let open = v.find('(')?;
    let inner = &v[open + 1..];
    // Take content up to the matching close paren.
    let mut depth = 1usize;
    let mut end = inner.len();
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = &inner[..end];
    // First top-level comma-separated token is the start color.
    let mut depth = 0usize;
    let mut first_end = inner.len();
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                first_end = i;
                break;
            }
            _ => {}
        }
    }
    let first = inner[..first_end].trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

/// Run the normalizer over HTML.
pub fn normalize_html(html: &str, opts: NormalizeOptions) -> Result<NormalizeResult> {
    let shared = Rc::new(RefCell::new(Pass1 {
        fix_dark: opts.fix_dark,
        ..Default::default()
    }));

    let s_el = shared.clone();
    let s_txt = shared.clone();
    let out = lol_html::rewrite_str(
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
            document_content_handlers: vec![DocumentContentHandlers::default().text(
                move |chunk| {
                    s_txt.borrow_mut().on_text(chunk);
                    Ok(())
                },
            )],
            ..Settings::default()
        },
    )
    .map_err(|e| crate::error::Error::Rewrite(e.to_string()))?;

    let p1 = shared.borrow();
    let fixes: Vec<FixRecord> = p1
        .records
        .iter()
        .map(|(r, d, c)| FixRecord {
            rule_id: r.clone(),
            count: *c,
            detail: d.clone(),
        })
        .collect();

    if p1.leaf_strip.is_empty() && p1.nodeleaf_strip.is_empty() {
        return Ok(NormalizeResult { html: out, fixes });
    }

    // Pass 2: strip leaf/nodeleaf by occurrence index.
    let li: Rc<RefCell<BTreeSet<usize>>> = Rc::new(RefCell::new(p1.leaf_strip.clone()));
    let ni: Rc<RefCell<BTreeSet<usize>>> = Rc::new(RefCell::new(p1.nodeleaf_strip.clone()));
    let lc: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let nc: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let (li2, lc2, ni2, nc2) = (li.clone(), lc.clone(), ni.clone(), nc.clone());
    let html2 = lol_html::rewrite_str(
        &out,
        Settings {
            element_content_handlers: vec![
                element!("span[leaf]", move |el| {
                    let i = *lc2.borrow();
                    *lc2.borrow_mut() = i + 1;
                    if li2.borrow().contains(&i) {
                        el.remove_attribute("leaf");
                    }
                    Ok(())
                }),
                element!("section[nodeleaf]", move |el| {
                    let i = *nc2.borrow();
                    *nc2.borrow_mut() = i + 1;
                    if ni2.borrow().contains(&i) {
                        el.remove_attribute("nodeleaf");
                    }
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
    )
    .map_err(|e| crate::error::Error::Rewrite(e.to_string()))?;

    Ok(NormalizeResult { html: html2, fixes })
}
