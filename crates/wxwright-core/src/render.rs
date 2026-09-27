//! Dialect renderer: IR -> WeChat MP compliant HTML (PRD 5.1).
//!
//! Hard invariants, enforced here and re-checked by the validator:
//! - block-level containers are `<section>`; text runs are `<span leaf>`;
//! - every style is inline; no class/id/<style>/script/external refs;
//! - no `font-family` anywhere (R-3.1);
//! - text containers carry line-height >= 1.5 (R-1.3);
//! - no fixed pixel widths; width:100% / min-width only (R-1.4).

use std::cell::RefCell;
use std::collections::HashMap;

use crate::htmlutil::{build_style, escape_attr, escape_text};
use crate::i18n;
use crate::img::ImgPipeline;
use crate::ir::{Align, Block, CardKind, ChartSpec, Inline, InlineKind, ListItem};
use crate::theme::{CodeTheme, LinkStyle, Theme};

pub struct RenderResult {
    pub html: String,
    /// (url, text) in first-occurrence order, for footnote mode.
    pub links: Vec<(String, String)>,
}

pub fn render_document(doc: &[Block], theme: &Theme, img: &ImgPipeline) -> RenderResult {
    let toc_items = collect_toc(doc);
    let ctx = Ctx {
        theme,
        img,
        links: RefCell::new(LinksState::default()),
        toc_items,
    };
    let mut body = render_blocks(doc, &ctx, &BodyCtx::root());
    if theme.link_style() == LinkStyle::Footnote {
        body.push_str(&render_footnotes(&ctx));
    }
    let links = ctx.links.borrow().ordered.clone();
    RenderResult { html: body, links }
}

#[derive(Default)]
struct LinksState {
    ordered: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

struct Ctx<'a> {
    theme: &'a Theme,
    img: &'a ImgPipeline,
    links: RefCell<LinksState>,
    toc_items: Vec<(u8, String)>,
}

/// Styling context for a nesting scope (root / quote / card / list item).
#[derive(Clone)]
struct BodyCtx {
    /// Base inline style for text leaves.
    leaf: String,
    /// Margin for paragraphs in this scope.
    para_margin: String,
}

impl BodyCtx {
    fn root() -> Self {
        BodyCtx {
            leaf: String::new(),
            para_margin: "margin: 0 0 16px;".into(),
        }
    }
}

/// Compose a style string: token-expanded defaults, overlaid by the theme's
/// role overrides.
fn css(theme: &Theme, role: &str, defaults: &[(&str, &str)]) -> String {
    let decls: Vec<(String, String)> = defaults
        .iter()
        .map(|(p, v)| (p.to_string(), expand(v, theme)))
        .collect();
    apply_role_overrides(theme, role, decls)
}

/// Same contract as `css`, for defaults computed at runtime (theme colours,
/// per-column widths) that cannot live in a `&'static` slice. Values still go
/// through token expansion, so `"{accent}"` works here too.
fn css_owned(theme: &Theme, role: &str, mut decls: Vec<(String, String)>) -> String {
    for (_, v) in decls.iter_mut() {
        *v = expand(v, theme);
    }
    apply_role_overrides(theme, role, decls)
}

/// Theme overrides win per property; `font-family` is dropped even if a theme
/// asks for it (R-3.1 is absolute).
fn apply_role_overrides(theme: &Theme, role: &str, mut decls: Vec<(String, String)>) -> String {
    if let Some(over) = theme.blocks.get(role) {
        for (p, v) in over {
            if p.eq_ignore_ascii_case("font-family") {
                continue;
            }
            let v = expand(v, theme);
            match decls.iter_mut().find(|(ep, _)| ep == p) {
                Some(slot) => slot.1 = v,
                None => decls.push((p.clone(), v)),
            }
        }
    }
    build_style(&decls)
}

fn expand(v: &str, theme: &Theme) -> String {
    if !v.contains('{') {
        return v.to_string();
    }
    let mut out = v.to_string();
    for key in [
        "accent",
        "text",
        "text_secondary",
        "text_tertiary",
        "border",
        "border_strong",
        "quote_bg",
        "quote_text",
        "code_bg",
        "code_text",
        "code_border",
        "inline_code_color",
        "table_head_bg",
        "table_border",
        "note_bg",
        "note_border",
        "tip_bg",
        "tip_border",
        "important_bg",
        "important_border",
        "warning_bg",
        "warning_border",
        "caution_bg",
        "caution_border",
        "keypoint_bg",
        "comment_bg",
        "toc_bg",
    ] {
        let token = format!("{{{}}}", key);
        if out.contains(&token) {
            out = out.replace(&token, &theme.color(key));
        }
    }
    out
}

/// Concatenate style fragments with proper separators.
fn cat(base: &str, extra: &str) -> String {
    let base = base.trim().trim_end_matches(';');
    if base.is_empty() {
        extra.to_string()
    } else {
        format!("{}; {}", base, extra)
    }
}

fn section(style: &str, inner: &str) -> String {
    if style.is_empty() {
        format!("<section>{}</section>", inner)
    } else {
        format!(
            "<section style=\"{}\">{}</section>",
            escape_attr(style),
            inner
        )
    }
}

fn leaf(style: &str, inner: &str) -> String {
    format!(
        "<span leaf style=\"{}\">{}</span>",
        escape_attr(style),
        inner
    )
}

// ---------------------------------------------------------------- blocks ---

fn render_blocks(blocks: &[Block], ctx: &Ctx, scope: &BodyCtx) -> String {
    let mut out = String::new();
    for b in blocks {
        out.push_str(&render_block(b, ctx, scope));
    }
    out
}

fn render_block(b: &Block, ctx: &Ctx, scope: &BodyCtx) -> String {
    match b {
        Block::Heading {
            level,
            center,
            inlines,
        } => render_heading(*level, *center, inlines, ctx, scope),
        Block::Paragraph { inlines } => render_paragraph(inlines, ctx, scope),
        Block::Code { lang, code } => render_code(lang.as_deref(), code, ctx),
        Block::Blockquote { blocks } => render_quote(blocks, ctx),
        Block::Card { kind, blocks } => render_card(*kind, blocks, ctx),
        Block::List {
            ordered,
            start,
            items,
        } => render_list(*ordered, *start, items, ctx, 0),
        Block::Table {
            aligns,
            header,
            rows,
        } => render_table(aligns, header, rows, ctx),
        Block::Image(img) => render_standalone_image(img, ctx, None),
        Block::Figure { image, caption } => render_standalone_image(image, ctx, Some(caption)),
        Block::Rule => render_rule(ctx),
        Block::Toc => render_toc(ctx),
        Block::Formula { latex } => render_formula(latex, ctx),
        Block::RawHtml { html } => {
            let base = base_leaf(ctx, scope, "paragraph");
            section(
                &format!(
                    "{} color: {}; font-size: 14px;",
                    scope.para_margin,
                    ctx.theme.color("text_secondary")
                ),
                &leaf(&base, &escape_text(html)),
            )
        }
        Block::SvgEmbed { html } => html.clone(),
        Block::Chart { spec } => render_chart(spec),
    }
}

// ---------------------------------------------------------------- charts ---
// Data charts (```chart fenced blocks, JSON spec) rendered as compliant
// inline SVG: brand palette, no font-family, no external resources. AI can
// emit these directly via the system prompt schema.

const CHART_PALETTE: [&str; 6] = [
    "#2F6CEA", "#7B9EF5", "#B45309", "#059669", "#7C3AED", "#DC2626",
];

fn fmt_value(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{:.1}", v)
    }
}

pub fn render_chart(spec: &ChartSpec) -> String {
    let n = spec.labels.len().max(spec.values.len()).max(1);
    let labels: Vec<String> = (0..n)
        .map(|i| spec.labels.get(i).cloned().unwrap_or_default())
        .collect();
    let values: Vec<f64> = (0..n)
        .map(|i| spec.values.get(i).copied().unwrap_or(0.0))
        .collect();
    let max = values.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
    let w = 677.0_f64;
    let mut out = String::new();
    out.push_str(
        "<section style=\"margin: 20px 0;\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 677 430\" style=\"width: 100%; display: block;\">",
    );
    if !spec.title.is_empty() {
        out.push_str(&format!(
            "<text x=\"338\" y=\"36\" font-size=\"20\" font-weight=\"600\" fill=\"#1F2328\" text-anchor=\"middle\">{}</text>",
            escape_text(&spec.title)
        ));
    }
    match spec.kind.as_str() {
        "line" => {
            let px0 = 50.0;
            let px1 = w - 30.0;
            let py0 = 70.0;
            let py1 = 360.0;
            let step = if n > 1 {
                (px1 - px0) / (n - 1) as f64
            } else {
                0.0
            };
            for g in 0..5 {
                let gy = py0 + (py1 - py0) * g as f64 / 4.0;
                out.push_str(&format!(
                    "<line x1=\"{px0:.1}\" y1=\"{gy:.1}\" x2=\"{px1:.1}\" y2=\"{gy:.1}\" stroke=\"#E4E7EC\" stroke-width=\"1\"/>"
                ));
                let gv = max * (4 - g) as f64 / 4.0;
                out.push_str(&format!(
                    "<text x=\"44\" y=\"{:.1}\" font-size=\"12\" fill=\"#8B949E\" text-anchor=\"end\">{}</text>",
                    gy + 4.0,
                    escape_text(&fmt_value(gv))
                ));
            }
            let pts: Vec<(f64, f64)> = values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let x = if n > 1 {
                        px0 + step * i as f64
                    } else {
                        (px0 + px1) / 2.0
                    };
                    let y = py1 - (v / max) * (py1 - py0);
                    (x, y)
                })
                .collect();
            let poly: String = pts
                .iter()
                .map(|(x, y)| format!("{:.1},{:.1}", x, y))
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!(
                "<polyline points=\"{poly}\" fill=\"none\" stroke=\"#2F6CEA\" stroke-width=\"3\" stroke-linejoin=\"round\"/>"
            ));
            for (i, (x, y)) in pts.iter().enumerate() {
                out.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4.5\" fill=\"#FFFFFF\" stroke=\"#2F6CEA\" stroke-width=\"2.5\"/>",
                    x, y
                ));
                out.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" fill=\"#1F2328\" text-anchor=\"middle\">{}</text>",
                    x,
                    y - 12.0,
                    escape_text(&fmt_value(values[i]))
                ));
                if let Some(lb) = labels.get(i) {
                    out.push_str(&format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"13\" fill=\"#57606A\" text-anchor=\"middle\">{}</text>",
                        x,
                        py1 + 24.0,
                        escape_text(lb)
                    ));
                }
            }
        }
        "pie" => {
            let cx = w * 0.32;
            let cy = 225.0;
            let r = 150.0;
            let total: f64 = values.iter().map(|v| v.max(0.0)).sum();
            let total = if total <= 0.0 { 1.0 } else { total };
            let mut angle = -std::f64::consts::FRAC_PI_2;
            for (i, v) in values.iter().enumerate() {
                let frac = (v.max(0.0) / total).min(1.0);
                let sweep = frac * std::f64::consts::TAU;
                let a0 = angle;
                let a1 = angle + sweep;
                angle = a1;
                let x0 = cx + r * a0.cos();
                let y0 = cy + r * a0.sin();
                let x1 = cx + r * a1.cos();
                let y1 = cy + r * a1.sin();
                let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
                let color = CHART_PALETTE[i % CHART_PALETTE.len()];
                if frac >= 0.9999 {
                    out.push_str(&format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\"/>",
                        cx, cy, r, color
                    ));
                } else {
                    out.push_str(&format!(
                        "<path d=\"M{:.1} {:.1} L{:.1} {:.1} A{:.1} {:.1} 0 {} 1 {:.1} {:.1} Z\" fill=\"{}\" stroke=\"#FFFFFF\" stroke-width=\"2\"/>",
                        cx, cy, x0, y0, r, r, large, x1, y1, color
                    ));
                }
            }
            let ly0 = 120.0;
            for (i, v) in values.iter().enumerate() {
                let ly = ly0 + i as f64 * 40.0;
                let color = CHART_PALETTE[i % CHART_PALETTE.len()];
                let name = labels.get(i).cloned().unwrap_or_default();
                let pct = (v / total * 100.0).round() as i64;
                out.push_str(&format!(
                    "<rect x=\"419\" y=\"{:.1}\" width=\"16\" height=\"16\" rx=\"4\" fill=\"{}\"/>",
                    ly, color
                ));
                out.push_str(&format!(
                    "<text x=\"445\" y=\"{:.1}\" font-size=\"15\" fill=\"#1F2328\">{} · {}%</text>",
                    ly + 13.0,
                    escape_text(&name),
                    pct
                ));
            }
        }
        _ => {
            let px0 = 50.0;
            let px1 = w - 30.0;
            let py0 = 70.0;
            let py1 = 360.0;
            let slot = (px1 - px0) / n as f64;
            let bw = (slot * 0.55).max(12.0);
            for g in 0..5 {
                let gy = py0 + (py1 - py0) * g as f64 / 4.0;
                out.push_str(&format!(
                    "<line x1=\"{px0:.1}\" y1=\"{gy:.1}\" x2=\"{px1:.1}\" y2=\"{gy:.1}\" stroke=\"#E4E7EC\" stroke-width=\"1\"/>"
                ));
                let gv = max * (4 - g) as f64 / 4.0;
                out.push_str(&format!(
                    "<text x=\"44\" y=\"{:.1}\" font-size=\"12\" fill=\"#8B949E\" text-anchor=\"end\">{}</text>",
                    gy + 4.0,
                    escape_text(&fmt_value(gv))
                ));
            }
            for (i, v) in values.iter().enumerate() {
                let cx = px0 + slot * (i as f64 + 0.5);
                let bh = (v / max) * (py1 - py0);
                let color = CHART_PALETTE[i % CHART_PALETTE.len()];
                out.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" fill=\"{}\"><animate attributeName=\"height\" values=\"0;{:.1}\" dur=\"0.8s\" begin=\"touchstart; click\" fill=\"freeze\"/></rect>",
                    cx - bw / 2.0,
                    py1 - bh,
                    bw,
                    bh,
                    color,
                    bh
                ));
                out.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"13\" font-weight=\"600\" fill=\"#1F2328\" text-anchor=\"middle\">{}</text>",
                    cx,
                    py1 - bh - 10.0,
                    escape_text(&fmt_value(*v))
                ));
                if let Some(lb) = labels.get(i) {
                    out.push_str(&format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"13\" fill=\"#57606A\" text-anchor=\"middle\">{}</text>",
                        cx,
                        py1 + 24.0,
                        escape_text(lb)
                    ));
                }
            }
        }
    }
    out.push_str("</svg></section>");
    out
}

/// Inline style for text runs. A nested scope (quote / card / list item) has
/// already resolved its own leaf style, so it wins; at the document root the
/// style comes from the theme's `<role>_leaf` entry (PRD 8: the theme is the
/// single place a role's typography is decided).
fn base_leaf(ctx: &Ctx, scope: &BodyCtx, role: &str) -> String {
    if !scope.leaf.is_empty() {
        return scope.leaf.clone();
    }
    css(
        ctx.theme,
        &format!("{}_leaf", role),
        &[
            ("font-size", "15px"),
            ("color", "{text}"),
            ("line-height", "1.75"),
            ("letter-spacing", "0.3px"),
        ],
    )
}

fn render_heading(
    level: u8,
    center: bool,
    inlines: &[Inline],
    ctx: &Ctx,
    scope: &BodyCtx,
) -> String {
    let role = format!("h{}", level);
    type StyleDefaults = &'static [(&'static str, &'static str)];
    let (sec_defaults, leaf_defaults): (StyleDefaults, StyleDefaults) = match level {
        1 => (
            &[
                ("margin", "32px 0 20px"),
                ("padding-bottom", "10px"),
                ("border-bottom", "1px solid {border}"),
            ],
            &[
                ("font-size", "22px"),
                ("font-weight", "600"),
                ("color", "{text}"),
                ("line-height", "1.4"),
            ],
        ),
        2 => (
            &[("margin", "28px 0 16px")],
            &[
                ("font-size", "19px"),
                ("font-weight", "600"),
                ("color", "{text}"),
                ("line-height", "1.4"),
            ],
        ),
        3 => (
            &[("margin", "24px 0 12px")],
            &[
                ("font-size", "16px"),
                ("font-weight", "600"),
                ("color", "{text}"),
                ("line-height", "1.5"),
            ],
        ),
        4 => (
            &[("margin", "20px 0 10px")],
            &[
                ("font-size", "15px"),
                ("font-weight", "600"),
                ("color", "{text}"),
                ("line-height", "1.5"),
            ],
        ),
        _ => (
            &[("margin", "16px 0 8px")],
            &[
                ("font-size", "14px"),
                ("font-weight", "600"),
                ("color", "{text_secondary}"),
                ("line-height", "1.5"),
            ],
        ),
    };
    let mut sec_style = css(ctx.theme, &role, sec_defaults);
    let leaf_style = css(ctx.theme, &format!("{}_leaf", role), leaf_defaults);
    if center {
        sec_style.push_str("; text-align: center;");
    }
    let inner_scope = BodyCtx {
        leaf: leaf_style.clone(),
        para_margin: scope.para_margin.clone(),
    };
    section(&sec_style, &render_inlines(inlines, ctx, &inner_scope))
}

fn render_paragraph(inlines: &[Inline], ctx: &Ctx, scope: &BodyCtx) -> String {
    if inlines.is_empty() {
        return String::new();
    }
    let inner = render_inlines(inlines, ctx, scope);
    // `scope.para_margin` arrives as a ready declaration ("margin: 0 0 16px;").
    // Split it back into a property/value pair so a theme can still override
    // `margin` through [block.paragraph] - otherwise the advertised
    // `paragraph` role would be the one key the renderer never read.
    let style = css_owned(ctx.theme, "paragraph", vec![split_decl(&scope.para_margin)]);
    section(&style, &inner)
}

/// Split a single declaration ("margin: 0 0 16px;" / "margin: 0 0 16px")
/// into (property, value). Falls back to a paragraph margin when the input is
/// empty or malformed.
fn split_decl(decl: &str) -> (String, String) {
    let d = decl.trim().trim_end_matches(';');
    match d.split_once(':') {
        Some((p, v)) if !p.trim().is_empty() && !v.trim().is_empty() => {
            (p.trim().to_string(), v.trim().to_string())
        }
        _ => ("margin".to_string(), "0 0 16px".to_string()),
    }
}

fn render_inlines(inlines: &[Inline], ctx: &Ctx, scope: &BodyCtx) -> String {
    let base = base_leaf(ctx, scope, "paragraph");
    let mut out = String::new();
    for il in inlines {
        out.push_str(&render_inline(il, ctx, &base));
    }
    out
}

fn render_inline(il: &Inline, ctx: &Ctx, base: &str) -> String {
    match il {
        Inline::Text(t) => {
            if t.is_empty() {
                String::new()
            } else {
                leaf(base, &escape_text(t))
            }
        }
        Inline::Code(c) => {
            let style = cat(
                base,
                &format!(
                    "background: {}; color: {}; padding: 2px 5px; border-radius: 4px; font-size: 14px;",
                    ctx.theme.color("code_bg"),
                    ctx.theme.color("inline_code_color")
                ),
            );
            leaf(&style, &escape_text(c))
        }
        Inline::Math { latex, display } => {
            if *display {
                render_formula(latex, ctx)
            } else {
                let style = cat(
                    base,
                    &format!(
                        "background: {}; padding: 1px 6px; border-radius: 4px; color: {};",
                        ctx.theme.color("code_bg"),
                        ctx.theme.color("text")
                    ),
                );
                leaf(&style, &escape_text(latex))
            }
        }
        Inline::Break => "<br/>".to_string(),
        Inline::RawHtml(h) => escape_text(h),
        Inline::Image(img) => render_img_tag(img, ctx),
        Inline::Styled { kind, children } => match kind {
            InlineKind::Strong => {
                format!(
                    "<strong style=\"font-weight: 600;\">{}</strong>",
                    render_children(children, ctx, base)
                )
            }
            InlineKind::Emphasis => {
                format!(
                    "<em style=\"font-style: italic;\">{}</em>",
                    render_children(children, ctx, base)
                )
            }
            InlineKind::Strike => {
                format!(
                    "<span style=\"text-decoration: line-through; color: {};\">{}</span>",
                    ctx.theme.color("text_secondary"),
                    render_children(children, ctx, base)
                )
            }
            InlineKind::Link { url, .. } => render_link(url, children, ctx, base),
        },
    }
}

/// Render inlines WITHOUT wrapping each run in its own leaf span; used
/// inside a leaf that already carries the base style (R-2.1 hygiene).
fn render_children_plain(children: &[Inline], ctx: &Ctx) -> String {
    let mut out = String::new();
    for c in children {
        match c {
            Inline::Text(t) => out.push_str(&escape_text(t)),
            Inline::Break => out.push_str("<br/>"),
            Inline::Code(code) => out.push_str(&format!(
                "<span style=\"color: {};\">{}</span>",
                ctx.theme.color("inline_code_color"),
                escape_text(code)
            )),
            Inline::Styled { kind, children } => match kind {
                InlineKind::Strong => out.push_str(&format!(
                    "<strong style=\"font-weight: 600;\">{}</strong>",
                    render_children_plain(children, ctx)
                )),
                InlineKind::Emphasis => out.push_str(&format!(
                    "<em style=\"font-style: italic;\">{}</em>",
                    render_children_plain(children, ctx)
                )),
                InlineKind::Strike => out.push_str(&format!(
                    "<span style=\"text-decoration: line-through;\">{}</span>",
                    render_children_plain(children, ctx)
                )),
                InlineKind::Link { .. } => out.push_str(&render_children_plain(children, ctx)),
            },
            other => out.push_str(&render_inline(other, ctx, "")),
        }
    }
    out
}

fn render_children(children: &[Inline], ctx: &Ctx, base: &str) -> String {
    let mut out = String::new();
    for c in children {
        out.push_str(&render_inline(c, ctx, base));
    }
    out
}

fn render_link(url: &str, children: &[Inline], ctx: &Ctx, base: &str) -> String {
    // An image wrapped in a link: keep the image, drop the link.
    if children.len() == 1 && matches!(children[0], Inline::Image(_)) {
        return render_inline(&children[0], ctx, base);
    }
    let flat_text = flatten_text(children);
    let accent = ctx.theme.color("accent");
    match ctx.theme.link_style() {
        LinkStyle::Inline => {
            let style = format!("color: {}; text-decoration: underline;", accent);
            let inner_base = cat(base, &format!("color: {};", accent));
            format!(
                "<a href=\"{}\" style=\"{}\">{}</a>",
                escape_attr(url),
                escape_attr(&style),
                render_children(children, ctx, &inner_base)
            )
        }
        LinkStyle::Footnote => {
            let idx = ctx.links.borrow_mut().index.get(url).copied();
            let idx = match idx {
                Some(i) => i,
                None => {
                    let mut ls = ctx.links.borrow_mut();
                    ls.ordered.push((url.to_string(), flat_text.clone()));
                    let i = ls.ordered.len();
                    ls.index.insert(url.to_string(), i);
                    i
                }
            };
            let link_style = cat(base, &format!("color: {};", accent));
            let marker_style = format!(
                "font-size: 12px; color: {}; vertical-align: super; line-height: 1.5;",
                accent
            );
            format!(
                "{}{}",
                leaf(&link_style, &render_children_plain(children, ctx)),
                leaf(&marker_style, &format!("[{}]", idx))
            )
        }
    }
}

fn flatten_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) => out.push_str(t),
            Inline::Code(c) => out.push_str(c),
            Inline::Styled { children, .. } => out.push_str(&flatten_text(children)),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn render_img_tag(img: &crate::ir::ImageRef, ctx: &Ctx) -> String {
    let resolved = ctx.img.resolve(&img.src);
    if resolved.failed || resolved.src.is_empty() {
        let label = if img.alt.is_empty() {
            i18n::t("image.missing")
        } else {
            img.alt.clone()
        };
        return section(
            &format!(
                "margin: 20px 0; padding: 20px; border: 1px dashed {}; border-radius: 8px; text-align: center; line-height: 1.5;",
                ctx.theme.color("border_strong")
            ),
            &leaf(
                &format!("color: {}; font-size: 14px;", ctx.theme.color("text_tertiary")),
                &escape_text(&format!("[{}]", label)),
            ),
        );
    }
    let mut attrs = format!(
        "src=\"{}\" style=\"width: 100%; border-radius: 8px; display: block;\"",
        escape_attr(&resolved.src)
    );
    if let Some(w) = resolved.data_w {
        attrs.push_str(&format!(" data-w=\"{}\"", w));
    }
    if let Some(r) = &resolved.data_ratio {
        attrs.push_str(&format!(" data-ratio=\"{}\"", escape_attr(r)));
    }
    if !img.alt.is_empty() {
        attrs.push_str(&format!(" alt=\"{}\"", escape_attr(&img.alt)));
    }
    if let Some(t) = &img.title {
        attrs.push_str(&format!(" title=\"{}\"", escape_attr(t)));
    }
    format!("<img {} />", attrs)
}

fn render_standalone_image(
    img: &crate::ir::ImageRef,
    ctx: &Ctx,
    caption: Option<&[Inline]>,
) -> String {
    let mut out = section(
        "margin: 20px 0; text-align: center;",
        &render_img_tag(img, ctx),
    );
    if let Some(cap) = caption {
        let style = css(
            ctx.theme,
            "figure_caption",
            &[
                ("margin-top", "8px"),
                ("text-align", "center"),
                ("font-size", "13px"),
                ("color", "{text_tertiary}"),
                ("line-height", "1.6"),
            ],
        );
        let inner = render_inlines(
            cap,
            ctx,
            &BodyCtx {
                leaf: css(
                    ctx.theme,
                    "figure_caption_leaf",
                    &[("font-size", "13px"), ("color", "{text_tertiary}")],
                ),
                para_margin: String::new(),
            },
        );
        out.push_str(&section(&style, &inner));
    }
    out
}

fn render_code(lang: Option<&str>, code: &str, ctx: &Ctx) -> String {
    let dark = ctx.theme.code_theme() == CodeTheme::Dark;
    let lines = crate::code::highlight(code, lang, dark);
    let mut inner = String::new();

    if let Some(l) = lang {
        let chip = section(
            "margin-bottom: 8px;",
            &leaf(
                &format!(
                    "font-size: 12px; color: {}; letter-spacing: 1px; line-height: 1.5;",
                    ctx.theme.color("text_tertiary")
                ),
                &escape_text(&l.to_ascii_uppercase()),
            ),
        );
        inner.push_str(&chip);
    }

    let line_style = format!(
        "display: block; font-size: 13px; line-height: 1.65; color: {}; white-space: pre-wrap; word-break: break-all;",
        ctx.theme.color("code_text")
    );
    let mut body = String::new();
    for line in &lines {
        if line.tokens.is_empty() {
            body.push_str(&leaf(&line_style, ""));
            body.push_str("<br/>");
            continue;
        }
        let mut line_html = String::new();
        for tok in &line.tokens {
            let mut sty = String::new();
            if !tok.color.is_empty() {
                sty.push_str(&format!("color: {};", tok.color));
            }
            if tok.bold {
                sty.push_str("font-weight: 600;");
            }
            if tok.italic {
                sty.push_str("font-style: italic;");
            }
            if sty.is_empty() {
                line_html.push_str(&escape_text(&tok.text));
            } else {
                line_html.push_str(&format!(
                    "<span style=\"{}\">{}</span>",
                    sty,
                    escape_text(&tok.text)
                ));
            }
        }
        body.push_str(&leaf(&line_style, &line_html));
    }
    inner.push_str(&section("padding: 2px 0; min-width: 0;", &body));

    section(
        &css(
            ctx.theme,
            "code",
            &[
                ("margin", "20px 0"),
                ("background", "{code_bg}"),
                ("border", "1px solid {code_border}"),
                ("border-radius", "8px"),
                ("padding", "12px 14px"),
                ("overflow-x", "auto"),
            ],
        ),
        &inner,
    )
}

fn render_quote(blocks: &[Block], ctx: &Ctx) -> String {
    let scope = BodyCtx {
        leaf: css(
            ctx.theme,
            "quote_leaf",
            &[
                ("font-size", "14px"),
                ("color", "{quote_text}"),
                ("line-height", "1.7"),
            ],
        ),
        para_margin: "margin: 0 0 8px;".into(),
    };
    let inner = render_blocks(blocks, ctx, &scope);
    section(
        &css(
            ctx.theme,
            "quote",
            &[
                ("margin", "20px 0"),
                ("padding", "12px 16px"),
                ("background", "{quote_bg}"),
                ("border-left", "3px solid {border_strong}"),
                ("border-radius", "0 8px 8px 0"),
            ],
        ),
        &inner,
    )
}

fn render_card(kind: CardKind, blocks: &[Block], ctx: &Ctx) -> String {
    // The role is the theme's handle on this card type. It used to be computed
    // and then discarded (`let _ = role;`), so every `[block.card_note]` entry
    // an AI-generated theme wrote was silently dropped - the single biggest
    // reason "AI layout" looked like it did nothing. See roles.rs.
    let role = kind.role();
    let border_key = match kind {
        CardKind::Note => "note_border",
        CardKind::Tip => "tip_border",
        CardKind::Important => "important_border",
        CardKind::Warning => "warning_border",
        CardKind::Caution => "caution_border",
        CardKind::Comment => "border_strong",
        CardKind::Keypoint => "accent",
    };
    let bg_key = match kind {
        CardKind::Note => "note_bg",
        CardKind::Tip => "tip_bg",
        CardKind::Important => "important_bg",
        CardKind::Warning => "warning_bg",
        CardKind::Caution => "caution_bg",
        CardKind::Comment => "comment_bg",
        CardKind::Keypoint => "keypoint_bg",
    };
    let (border_prop, border_val) = match kind {
        CardKind::Comment => (
            "border",
            format!("1px dashed {}", ctx.theme.color("border_strong")),
        ),
        _ => (
            "border-left",
            format!("4px solid {}", ctx.theme.color(border_key)),
        ),
    };
    let container = css_owned(
        ctx.theme,
        role,
        vec![
            ("margin".into(), "20px 0".into()),
            ("padding".into(), "12px 16px".into()),
            ("background".into(), ctx.theme.color(bg_key)),
            (border_prop.into(), border_val),
            ("border-radius".into(), "0 8px 8px 0".into()),
        ],
    );
    let title = css_owned(
        ctx.theme,
        &format!("{}_title", role),
        vec![
            ("font-size".into(), "13px".into()),
            ("font-weight".into(), "600".into()),
            ("color".into(), ctx.theme.color(border_key)),
            ("line-height".into(), "1.5".into()),
            ("letter-spacing".into(), "0.5px".into()),
        ],
    );
    let scope = BodyCtx {
        leaf: css_owned(
            ctx.theme,
            &format!("{}_leaf", role),
            vec![
                ("font-size".into(), "14px".into()),
                ("color".into(), ctx.theme.color("text")),
                ("line-height".into(), "1.7".into()),
            ],
        ),
        para_margin: "margin: 0 0 8px;".into(),
    };
    // Body blocks must stay inside the container section, or the card
    // background only covers the title strip.
    let mut inner = section(
        "margin-bottom: 6px;",
        &leaf(&title, &escape_text(&i18n::t(kind.i18n_key()))),
    );
    inner.push_str(&render_blocks(blocks, ctx, &scope));
    section(&container, &inner)
}

fn render_list(ordered: bool, start: u64, items: &[ListItem], ctx: &Ctx, depth: usize) -> String {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            (start + i as u64).to_string()
        } else {
            String::new()
        };
        out.push_str(&render_list_item(item, ordered, &marker, ctx, depth));
    }
    out
}

fn render_list_item(
    item: &ListItem,
    ordered: bool,
    marker: &str,
    ctx: &Ctx,
    depth: usize,
) -> String {
    let indent = 1.6 + depth as f64 * 1.4;
    let mut inner = String::new();

    // Marker prefix.
    match item.checked {
        Some(done) => {
            let mut box_style = format!(
                "display: inline-block; width: 15px; height: 15px; border: 1.5px solid {}; border-radius: 4px; vertical-align: -2px; margin-right: 6px; box-sizing: border-box; line-height: 1.5;",
                ctx.theme.color("border_strong")
            );
            if done {
                box_style.push_str(&format!(
                    " background: {}; border-color: {};",
                    ctx.theme.color("accent"),
                    ctx.theme.color("accent")
                ));
            }
            inner.push_str(&format!(
                "<span style=\"{}\" data-ignore-width=\"1\"></span>",
                escape_attr(&box_style)
            ));
        }
        None => {
            let mstyle = if ordered {
                format!(
                    "color: {}; font-weight: 600; line-height: 1.75;",
                    ctx.theme.color("text_secondary")
                )
            } else {
                format!("color: {}; line-height: 1.75;", ctx.theme.color("accent"))
            };
            let text = if ordered {
                format!("{}.&nbsp;&nbsp;", escape_text(marker))
            } else {
                "&bull;&nbsp;&nbsp;".to_string()
            };
            inner.push_str(&leaf(&mstyle, &text));
        }
    }

    // Content: first paragraph inline with marker, further blocks below.
    let base = css(
        ctx.theme,
        "list_item_leaf",
        &[
            ("font-size", "15px"),
            ("color", "{text}"),
            ("line-height", "1.75"),
            ("letter-spacing", "0.3px"),
        ],
    );
    let mut first_done = false;
    for b in &item.blocks {
        match b {
            Block::Paragraph { inlines } if !first_done => {
                first_done = true;
                for il in inlines {
                    inner.push_str(&render_inline(il, ctx, &base));
                }
            }
            other => {
                let nested_scope = BodyCtx {
                    leaf: base.clone(),
                    para_margin: "margin: 8px 0 0;".into(),
                };
                let rendered = match other {
                    Block::List {
                        ordered,
                        start,
                        items,
                    } => render_list(*ordered, *start, items, ctx, depth + 1),
                    _ => render_block(other, ctx, &nested_scope),
                };
                inner.push_str(&section(
                    &format!("margin: 8px 0 0; padding-left: {:.1}em;", 0.0),
                    &rendered,
                ));
            }
        }
    }

    section(
        &css_owned(
            ctx.theme,
            "list_item",
            vec![
                ("margin".into(), "6px 0".into()),
                ("padding-left".into(), format!("{:.2}em", indent)),
                ("line-height".into(), "1.75".into()),
            ],
        ),
        &inner,
    )
}

fn render_table(
    aligns: &[Align],
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    ctx: &Ctx,
) -> String {
    let ncols = header.len().max(rows.first().map(|r| r.len()).unwrap_or(0));
    let align_of = |i: usize| -> &'static str {
        match aligns.get(i) {
            Some(Align::Center) => "center",
            Some(Align::Right) => "right",
            _ => "left",
        }
    };
    // Column min-width heuristic (F-05): longest plain text drives min-width.
    let mut widths = vec![0usize; ncols];
    let mut scan = |cells: &[Vec<Inline>]| {
        for (i, cell) in cells.iter().enumerate() {
            let len = flatten_text(cell).chars().count().max(1);
            if i < ncols {
                widths[i] = widths[i].max(len);
            }
        }
    };
    scan(header);
    for r in rows {
        scan(r);
    }
    let minw = |i: usize| -> String {
        if i >= ncols {
            return "56px".into();
        }
        let px = (widths[i] * 14).clamp(56, 220);
        format!("{}px", px)
    };

    let cell_base = |i: usize, head: bool| -> String {
        let mut decls: Vec<(String, String)> = vec![
            ("min-width".into(), minw(i)),
            (
                "border".into(),
                format!("1px solid {}", ctx.theme.color("table_border")),
            ),
            ("padding".into(), "8px 10px".into()),
            ("text-align".into(), align_of(i).to_string()),
            ("font-size".into(), "14px".into()),
            ("line-height".into(), "1.6".into()),
            ("color".into(), ctx.theme.color("text")),
        ];
        if head {
            decls.push(("background".into(), ctx.theme.color("table_head_bg")));
            decls.push(("font-weight".into(), "600".into()));
        }
        let role = if head { "table_head" } else { "table_cell" };
        css_owned(ctx.theme, role, decls)
    };
    // Leaf runs inside cells carry only typography, not the cell layout.
    let cell_leaf = |head: bool| -> String {
        if head {
            css(
                ctx.theme,
                "table_head_leaf",
                &[("font-weight", "600"), ("color", "{text}")],
            )
        } else {
            css(ctx.theme, "table_cell_leaf", &[("color", "{text}")])
        }
    };

    let mut thead = String::new();
    if !header.is_empty() {
        thead.push_str("<thead><tr>");
        for (i, cell) in header.iter().enumerate() {
            let base = cell_base(i, true);
            thead.push_str(&format!(
                "<th style=\"{}\">{}</th>",
                escape_attr(&base),
                render_inlines(
                    cell,
                    ctx,
                    &BodyCtx {
                        leaf: cell_leaf(true),
                        para_margin: String::new()
                    }
                )
            ));
        }
        thead.push_str("</tr></thead>");
    }

    let mut tbody = String::new();
    if !rows.is_empty() {
        tbody.push_str("<tbody>");
        for row in rows {
            tbody.push_str("<tr>");
            for (i, cell) in row.iter().enumerate() {
                let base = cell_base(i, false);
                tbody.push_str(&format!(
                    "<td style=\"{}\">{}</td>",
                    escape_attr(&base),
                    render_inlines(
                        cell,
                        ctx,
                        &BodyCtx {
                            leaf: cell_leaf(false),
                            para_margin: String::new()
                        }
                    )
                ));
            }
            if row.len() < ncols {
                for i in row.len()..ncols {
                    let base = cell_base(i, false);
                    tbody.push_str(&format!("<td style=\"{}\"></td>", escape_attr(&base)));
                }
            }
            tbody.push_str("</tr>");
        }
        tbody.push_str("</tbody>");
    }

    section(
        &css(
            ctx.theme,
            "table",
            &[("margin", "20px 0"), ("overflow-x", "auto")],
        ),
        &format!(
            "<table style=\"width: 100%; border-collapse: collapse;\">{}{}</table>",
            thead, tbody
        ),
    )
}

fn render_rule(ctx: &Ctx) -> String {
    section(
        &css(
            ctx.theme,
            "rule",
            &[
                ("margin", "28px 0"),
                ("border-top", "1px solid {border}"),
                ("line-height", "1px"),
                ("font-size", "0"),
            ],
        ),
        "",
    )
}

fn render_toc(ctx: &Ctx) -> String {
    let mut items = String::new();
    let mut seq = 0;
    let item_leaf = css(
        ctx.theme,
        "toc_item_leaf",
        &[
            ("font-size", "14px"),
            ("color", "{text}"),
            ("line-height", "1.7"),
        ],
    );
    for (level, text) in &ctx.toc_items {
        seq += 1;
        if seq > 25 {
            break;
        }
        let indent = if *level >= 3 { "1.4em" } else { "0em" };
        items.push_str(&section(
            &css_owned(
                ctx.theme,
                "toc_item",
                vec![
                    ("margin".into(), "4px 0".into()),
                    ("padding-left".into(), indent.to_string()),
                    ("line-height".into(), "1.7".into()),
                ],
            ),
            &leaf(&item_leaf, &escape_text(&format!("{}. {}", seq, text))),
        ));
    }
    section(
        &css(
            ctx.theme,
            "toc",
            &[
                ("margin", "20px 0"),
                ("padding", "14px 18px"),
                ("border", "1px solid {border}"),
                ("border-radius", "8px"),
                ("background", "{toc_bg}"),
            ],
        ),
        &format!(
            "{}{}",
            section(
                "margin-bottom: 8px;",
                &leaf(
                    &css(
                        ctx.theme,
                        "toc_heading",
                        &[
                            ("font-size", "13px"),
                            ("font-weight", "600"),
                            ("color", "{text_tertiary}"),
                            ("letter-spacing", "2px"),
                            ("line-height", "1.5"),
                        ],
                    ),
                    &escape_text(&i18n::t("toc.title")),
                ),
            ),
            items
        ),
    )
}

fn render_formula(latex: &str, ctx: &Ctx) -> String {
    section(
        &css(
            ctx.theme,
            "formula",
            &[
                ("margin", "16px 0"),
                ("padding", "12px 14px"),
                ("background", "{code_bg}"),
                ("border-radius", "8px"),
                ("text-align", "center"),
            ],
        ),
        &leaf(
            &css(
                ctx.theme,
                "formula_leaf",
                &[
                    ("color", "{text}"),
                    ("line-height", "1.6"),
                    ("letter-spacing", "0.5px"),
                    ("font-size", "15px"),
                ],
            ),
            &escape_text(latex),
        ),
    )
}

fn render_footnotes(ctx: &Ctx) -> String {
    let ls = ctx.links.borrow();
    if ls.ordered.is_empty() {
        return String::new();
    }
    let mut items = String::new();
    for (i, (url, text)) in ls.ordered.iter().enumerate() {
        let label = if text.is_empty() {
            url.clone()
        } else {
            format!("{} - {}", text, url)
        };
        items.push_str(&section(
            "margin: 4px 0; line-height: 1.6;",
            &leaf(
                &format!(
                    "font-size: 13px; color: {}; word-break: break-all; line-height: 1.6;",
                    ctx.theme.color("text_secondary")
                ),
                &escape_text(&format!("[{}] {}", i + 1, label)),
            ),
        ));
    }
    section(
        &format!(
            "margin: 28px 0 0; padding-top: 12px; border-top: 1px solid {};",
            ctx.theme.color("border")
        ),
        &format!(
            "{}{}",
            section(
                "margin-bottom: 8px;",
                &leaf(
                    &format!(
                        "font-size: 13px; font-weight: 600; color: {}; letter-spacing: 1px; line-height: 1.5;",
                        ctx.theme.color("text_tertiary")
                    ),
                    &escape_text(&i18n::t("links.title")),
                ),
            ),
            items
        ),
    )
}

/// Collect headings (h1-h3) for the [TOC] card.
fn collect_toc(doc: &[Block]) -> Vec<(u8, String)> {
    let mut out = Vec::new();
    fn walk(blocks: &[Block], out: &mut Vec<(u8, String)>) {
        for b in blocks {
            match b {
                Block::Heading { level, inlines, .. } if *level <= 3 => {
                    let text = inlines
                        .iter()
                        .map(|i| match i {
                            Inline::Text(t) => t.clone(),
                            Inline::Code(c) => c.clone(),
                            Inline::Styled { children, .. } => children
                                .iter()
                                .filter_map(|c| match c {
                                    Inline::Text(t) => Some(t.clone()),
                                    _ => None,
                                })
                                .collect::<String>(),
                            _ => String::new(),
                        })
                        .collect::<String>()
                        .trim()
                        .to_string();
                    if !text.is_empty() {
                        out.push((*level, text));
                    }
                }
                Block::Blockquote { blocks } | Block::Card { blocks, .. } => walk(blocks, out),
                Block::List { items, .. } => {
                    for it in items {
                        walk(&it.blocks, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(doc, &mut out);
    out
}
