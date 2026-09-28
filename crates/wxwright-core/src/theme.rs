//! Theme system: TOML theme packs compiled to inline styles (PRD 4.1 F-02, 8).
//! A theme is data: style dictionaries per semantic role. All styles are
//! expanded to inline CSS at render time; `font-family` is forbidden (R-3.1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkStyle {
    /// Default: link text + numbered reference list at the end (survives
    /// unverified accounts where external hrefs are stripped).
    Footnote,
    /// Keep `<a href>` inline (for verified accounts).
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeTheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ThemeMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub name_zh: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_zh: String,
    #[serde(default)]
    pub link_style: Option<LinkStyle>,
    #[serde(default)]
    pub code_theme: Option<CodeTheme>,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub meta: ThemeMeta,
    /// Color tokens: accent, text, text_secondary, border, code_bg, ...
    pub colors: BTreeMap<String, String>,
    /// Suggested Dark Mode palette (informational, feeds --fix-dark / GUI).
    pub color_dark: BTreeMap<String, String>,
    /// Role -> ordered (property, value) style overrides.
    pub blocks: BTreeMap<String, Vec<(String, String)>>,
    pub source_path: Option<PathBuf>,
}

impl Theme {
    pub fn link_style(&self) -> LinkStyle {
        self.meta.link_style.unwrap_or(LinkStyle::Footnote)
    }

    /// Page canvas colour for file export (wrap_document). Themes may declare
    /// a dark canvas; exports must honour it or light-on-dark text becomes
    /// invisible against the hardcoded white wrapper (found via the AI
    /// generated dark-code-theme rendering unreadable).
    pub fn canvas(&self) -> &str {
        self.colors
            .get("background")
            .map(String::as_str)
            .unwrap_or("#FFFFFF")
    }

    pub fn code_theme(&self) -> CodeTheme {
        self.meta.code_theme.unwrap_or(CodeTheme::Light)
    }

    pub fn color(&self, key: &str) -> String {
        self.colors
            .get(key)
            .cloned()
            .unwrap_or_else(|| default_color(key).to_string())
    }
}

/// Every colour token the engine understands, with its default value.
///
/// The renderer's `{token}` substitution and `default_color` both read
/// this table, so a new token cannot be added in one place and forgotten
/// in the other - which is exactly what happened when the list lived in
/// both render.rs and theme.rs.
pub const COLOR_TOKENS: &[(&str, &str)] = &[
    ("accent", "#2F6CEA"),
    ("text", "#1F2328"),
    ("text_secondary", "#57606A"),
    ("text_tertiary", "#8B949E"),
    ("border", "#D8DEE4"),
    ("border_strong", "#A8B3BD"),
    ("quote_bg", "#F7F8FA"),
    ("quote_text", "#57606A"),
    ("code_bg", "#F6F8FA"),
    ("code_text", "#24292F"),
    ("code_border", "#E4E7EC"),
    ("inline_code_color", "#C2402A"),
    ("table_head_bg", "#F6F8FA"),
    ("table_border", "#D8DEE4"),
    ("note_bg", "#EFF4FE"),
    ("note_border", "#2F6CEA"),
    ("tip_bg", "#ECFDF3"),
    ("tip_border", "#059669"),
    ("important_bg", "#F5F3FF"),
    ("important_border", "#7C3AED"),
    ("warning_bg", "#FEF3E2"),
    ("warning_border", "#D97706"),
    ("caution_bg", "#FEF2F2"),
    ("caution_border", "#DC2626"),
    ("keypoint_bg", "#EFF4FE"),
    ("comment_bg", "#FAFBFC"),
    ("toc_bg", "#FCFCFD"),
];

fn default_color(key: &str) -> &'static str {
    COLOR_TOKENS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or("#1F2328")
}

/// Parse a theme from TOML text.
pub fn parse_theme(toml_src: &str) -> Result<Theme> {
    let value: toml::Value =
        toml::from_str(toml_src).map_err(|e| Error::Theme(format!("invalid TOML: {}", e)))?;

    let meta: ThemeMeta = value
        .get("meta")
        .cloned()
        .map(|m| {
            m.try_into()
                .map_err(|e| Error::Theme(format!("invalid [meta]: {}", e)))
        })
        .transpose()?
        .ok_or_else(|| Error::Theme("missing [meta] section".into()))?;
    if meta.id.is_empty() || meta.name.is_empty() {
        return Err(Error::Theme("[meta] requires id and name".into()));
    }

    let mut colors = BTreeMap::new();
    if let Some(t) = value.get("colors").and_then(|v| v.as_table()) {
        for (k, v) in t {
            let s = v
                .as_str()
                .ok_or_else(|| Error::Theme(format!("colors.{} must be a string", k)))?;
            validate_color(k, s)?;
            colors.insert(k.clone(), s.to_string());
        }
    }

    let mut color_dark = BTreeMap::new();
    if let Some(t) = value.get("color_dark").and_then(|v| v.as_table()) {
        for (k, v) in t {
            let s = v
                .as_str()
                .ok_or_else(|| Error::Theme(format!("color_dark.{} must be a string", k)))?;
            validate_color(k, s)?;
            color_dark.insert(k.clone(), s.to_string());
        }
    }

    let mut blocks = BTreeMap::new();
    if let Some(t) = value.get("block").and_then(|v| v.as_table()) {
        for (role, styles) in t {
            let st = styles
                .as_table()
                .ok_or_else(|| Error::Theme(format!("block.{} must be a table", role)))?;
            let mut decls = Vec::new();
            for (prop, val) in st {
                let s = val.as_str().ok_or_else(|| {
                    Error::Theme(format!("block.{}.{} must be a string", role, prop))
                })?;
                if prop.eq_ignore_ascii_case("font-family") || s.contains("font-family") {
                    return Err(Error::Theme(format!(
                        "block.{}.{}: font-family is forbidden (official rule R-3.1)",
                        role, prop
                    )));
                }
                decls.push((prop.clone(), s.to_string()));
            }
            blocks.insert(role.clone(), decls);
        }
    }

    Ok(Theme {
        meta,
        colors,
        color_dark,
        blocks,
        source_path: None,
    })
}

fn validate_color(key: &str, val: &str) -> Result<()> {
    let ok = val.starts_with('#') || val.starts_with("rgb(") || val.starts_with("rgba(");
    if !ok {
        return Err(Error::Theme(format!(
            "colors.{}: expected hex or rgb() color, got {:?}",
            key, val
        )));
    }
    Ok(())
}

/// Built-in themes embedded at compile time.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("minimal", include_str!("themes/minimal.toml")),
    ("techblue", include_str!("themes/techblue.toml")),
    ("magazine", include_str!("themes/magazine.toml")),
];

pub fn load_builtin_theme(id: &str) -> Option<Theme> {
    BUILTIN_THEMES
        .iter()
        .find(|(tid, _src)| *tid == id)
        .and_then(|(_, src)| parse_theme(src).ok())
}

/// User theme directory: %APPDATA%/wxwright/themes (Windows),
/// ~/.config/wxwright/themes (Unix). AI-generated and community themes live
/// here and are picked up by `load_theme` / `list_user_themes` everywhere.
pub fn user_themes_dir() -> PathBuf {
    crate::util::config_root().join("themes")
}

/// Load a theme by built-in id, user-theme id, or file path.
pub fn load_theme(name_or_path: &str) -> Result<Theme> {
    if let Some(t) = load_builtin_theme(name_or_path) {
        return Ok(t);
    }
    let p = Path::new(name_or_path);
    if p.exists() {
        let src = std::fs::read_to_string(p)?;
        let mut t = parse_theme(&src)?;
        t.source_path = Some(PathBuf::from(name_or_path));
        return Ok(t);
    }
    // User themes directory fallback (id without extension).
    let candidate = user_themes_dir().join(format!("{}.toml", name_or_path));
    if candidate.exists() {
        let src = std::fs::read_to_string(&candidate)?;
        let mut t = parse_theme(&src)?;
        t.source_path = Some(candidate);
        return Ok(t);
    }
    let available: Vec<&str> = BUILTIN_THEMES.iter().map(|(id, _)| *id).collect();
    Err(Error::Theme(format!(
        "theme {:?} not found; built-in themes: {} (or pass a path to a .toml theme file)",
        name_or_path,
        available.join(", ")
    )))
}

/// All themes in the user directory (best-effort; broken files are skipped).
pub fn list_user_themes() -> Vec<Theme> {
    let dir = user_themes_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
            .collect();
        paths.sort();
        for p in paths {
            if let Ok(src) = std::fs::read_to_string(&p) {
                if let Ok(mut t) = parse_theme(&src) {
                    t.source_path = Some(p);
                    out.push(t);
                }
            }
        }
    }
    out
}

/// Validate and persist a theme TOML into the user directory.
/// Returns the written file path. An existing theme with the same id is
/// overwritten (regeneration is the common case).
pub fn save_user_theme(toml_src: &str) -> Result<PathBuf> {
    let theme = parse_theme(toml_src)?;
    let dir = user_themes_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.toml", theme.meta.id));
    std::fs::write(
        &path,
        toml_src.trim().to_string()
            + "
",
    )?;
    Ok(path)
}

/// All built-in themes parsed.
pub fn builtin_themes() -> Result<Vec<Theme>> {
    BUILTIN_THEMES
        .iter()
        .map(|(id, src)| {
            parse_theme(src)
                .map_err(|e| Error::Theme(format!("builtin theme {} broken: {}", id, e)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_parse() {
        for (id, src) in BUILTIN_THEMES {
            let t = parse_theme(src).unwrap_or_else(|e| panic!("theme {}: {}", id, e));
            assert_eq!(t.meta.id, *id);
        }
    }

    #[test]
    fn font_family_rejected() {
        let src = r#"
[meta]
id = "bad"
name = "bad"

[block.paragraph]
font-family = "serif"
"#;
        assert!(parse_theme(src).is_err());
    }
}
