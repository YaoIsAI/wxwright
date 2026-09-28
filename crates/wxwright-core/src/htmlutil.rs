//! HTML / CSS utilities: escaping, inline style parsing and building.
//! Style parsing is a hand-written state machine, zero regex (PRD 5.7-B).

/// Escape text for HTML body content.
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a value for use inside a double-quoted attribute.
/// Drop markup and return just the words.
///
/// The caption exporters need this: a social caption must not carry markup,
/// but it must not show `&lt;p&gt;` either - it wants the text. Tags become a
/// space so words do not run together, and runs of whitespace collapse.
///
/// The content of `script` / `style` / `head` / `title` is dropped entirely:
/// it is not text, and letting it through put `alert(1)` in the middle of a
/// caption.
pub fn strip_tags(s: &str) -> String {
    const DROP_CONTENT: &[&str] = &["script", "style", "head", "title"];
    let mut out = String::with_capacity(s.len());
    let mut tag = String::new();
    let mut in_tag = false;
    let mut dropping: Option<String> = None;
    for c in s.chars() {
        match c {
            '<' => {
                if in_tag {
                    // A '<' inside an unclosed tag means the earlier one was
                    // literal text ("1 < 2"); keep it.
                    out.push('<');
                    out.push_str(&tag);
                }
                in_tag = true;
                tag.clear();
            }
            '>' => {
                if !in_tag {
                    out.push('>');
                    continue;
                }
                in_tag = false;
                let closing = tag.starts_with('/');
                let name = tag
                    .trim_start_matches('/')
                    .split(|ch: char| ch.is_whitespace() || ch == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if closing {
                    if dropping.as_deref() == Some(name.as_str()) {
                        dropping = None;
                    }
                } else if dropping.is_none() && DROP_CONTENT.contains(&name.as_str()) {
                    dropping = Some(name);
                }
                out.push(' ');
            }
            _ if in_tag => tag.push(c),
            _ if dropping.is_some() => {}
            _ => out.push(c),
        }
    }
    if in_tag {
        out.push('<');
        out.push_str(&tag);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Parse an inline `style` attribute into ordered (property, value) pairs.
/// Hand-rolled scanner: no regex, tolerant of url(...) and quoted strings
/// containing ';'.
pub fn parse_style(style: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = style.as_bytes();
    let mut depth = 0usize; // parenthesis depth (url(...))
    let mut quote: Option<u8> = None;
    let mut start = 0usize;
    let n = bytes.len();
    for i in 0..=n {
        let at_end = i == n;
        let c = if at_end { b';' } else { bytes[i] };
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            b'\'' | b'"' => quote = Some(c),
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                let decl = &style[start..i];
                if let Some(pair) = split_decl(decl) {
                    out.push(pair);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

fn split_decl(decl: &str) -> Option<(String, String)> {
    let idx = decl.find(':')?;
    let prop = decl[..idx].trim().to_ascii_lowercase();
    let val = decl[idx + 1..].trim().to_string();
    if prop.is_empty() || val.is_empty() {
        return None;
    }
    Some((prop, val))
}

/// Build a `style` attribute value from ordered pairs.
pub fn build_style(decls: &[(String, String)]) -> String {
    let mut out = String::new();
    for (p, v) in decls {
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(p);
        out.push_str(": ");
        out.push_str(v);
    }
    out
}

/// Convert a px value like "312px" to f64.
pub fn px_value(v: &str) -> Option<f64> {
    let t = v.trim();
    let t = t.strip_suffix("px").unwrap_or(t);
    t.trim().parse::<f64>().ok()
}

/// Width baseline used by the official editor design canvas (PRD 5.3 R-1.4).
pub const DESIGN_WIDTH_PX: f64 = 677.0;

/// Convert a px width to a percentage string relative to the design width,
/// clamped to [1%, 100%].
pub fn px_to_percent(px: f64) -> String {
    let pct = (px / DESIGN_WIDTH_PX * 100.0).round().clamp(1.0, 100.0);
    format!("{}%", trim_float(pct))
}

fn trim_float(f: f64) -> String {
    let s = format!("{:.2}", f);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Relative luminance of a CSS color (#rgb, #rrggbb, rgb(r,g,b)).
pub fn css_color_luminance(color: &str) -> Option<f64> {
    let (r, g, b) = css_color_rgb(color)?;
    fn lin(c: f64) -> f64 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    Some(0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b))
}

/// Parse a CSS color into (r, g, b) floats in 0..1.
pub fn css_color_rgb(color: &str) -> Option<(f64, f64, f64)> {
    let c = color.trim().to_ascii_lowercase();
    if let Some(hex) = c.strip_prefix('#') {
        // A hex color is ASCII by definition. Without this guard the 6-char
        // branch below slices by byte index and panics on a non-char boundary
        // for a malformed value of exactly six bytes (`#中ab` is 6 bytes with
        // the split landing inside the 3-byte character), which took down
        // `wxwright validate` with exit code 101.
        if !hex.is_ascii() {
            return None;
        }
        let (r, g, b) = match hex.len() {
            3 => {
                let v: Vec<u32> = hex.chars().filter_map(|ch| ch.to_digit(16)).collect();
                if v.len() != 3 {
                    return None;
                }
                (
                    (v[0] << 4 | v[0]) as f64 / 255.0,
                    (v[1] << 4 | v[1]) as f64 / 255.0,
                    (v[2] << 4 | v[2]) as f64 / 255.0,
                )
            }
            6 => {
                let r = u32::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u32::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u32::from_str_radix(&hex[4..6], 16).ok()?;
                (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
            }
            _ => return None,
        };
        return Some((r, g, b));
    }
    if let Some(rest) = c.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<f64> = rest
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if parts.len() == 3 {
            return Some((parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0));
        }
    }
    None
}

/// WCAG contrast ratio between two CSS colors.
pub fn contrast_ratio(a: &str, b: &str) -> Option<f64> {
    let la = css_color_luminance(a)?;
    let lb = css_color_luminance(b)?;
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_style() {
        let s = "color: red; font-family: 'a;b', serif; width: 300px";
        let decls = parse_style(s);
        assert_eq!(decls.len(), 3);
        assert_eq!(decls[1].0, "font-family");
        assert_eq!(decls[1].1, "'a;b', serif");
    }

    #[test]
    fn test_px_to_percent() {
        assert_eq!(px_to_percent(677.0), "100%");
        assert_eq!(px_to_percent(338.5), "50%");
        assert_eq!(px_to_percent(2000.0), "100%");
    }

    #[test]
    fn test_contrast() {
        let r = contrast_ratio("#000000", "#ffffff").unwrap();
        assert!((r - 21.0).abs() < 0.1);
        let r2 = contrast_ratio("#ffffff", "#ffffff").unwrap();
        assert!((r2 - 1.0).abs() < 0.01);
    }

    #[test]
    fn malformed_hex_colors_return_none_instead_of_panicking() {
        // Regression: the 6-byte branch used to slice by byte index, so a value
        // of exactly six bytes containing a multi-byte character split inside
        // that character and panicked. `#中ab` = 1 + 3 + 1 + 1 = 6 bytes.
        assert_eq!(css_color_rgb("#中ab"), None);
        assert_eq!("#中ab".len(), 6, "the probe must stay in the 6-byte branch");
        // Other malformed shapes must also be rejected, not panic.
        assert_eq!(css_color_rgb("#"), None);
        assert_eq!(css_color_rgb("#中"), None);
        assert_eq!(css_color_rgb("#中文"), None);
        assert_eq!(css_color_rgb("#zzzzzz"), None);
        assert_eq!(css_color_rgb("#1234567"), None);
        // Valid values keep working, and the uppercase form is normalised.
        assert_eq!(css_color_rgb("#FFFFFF"), css_color_rgb("#ffffff"));
        assert!(css_color_rgb("#123456").is_some());
    }
}
