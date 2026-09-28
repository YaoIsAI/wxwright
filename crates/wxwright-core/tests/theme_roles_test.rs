//! Regression guard for the theme role contract (PRD 8).
//!
//! History: the AI theme prompt advertised `card_note` / `card_tip` / ... as
//! valid `[block.*]` roles, but `render_card` computed the role and then
//! discarded it (`let _ = role;`), and `base_leaf` hardcoded `paragraph_leaf`.
//! AI-authored card styling was silently thrown away and nothing failed.
//!
//! This test walks the canonical role table (`wxwright_core::roles::ROLES`),
//! gives each key a distinctive declaration, renders a document that exercises
//! that block type, and asserts the declaration actually reaches the HTML.
//! Any future "advertised but never consumed" role fails here.

use wxwright_core::img::ImageMode;
use wxwright_core::roles;
use wxwright_core::theme::parse_theme;
use wxwright_core::{convert_markdown, ConvertOptions};

/// Exercises every block type the role table covers.
const DOC: &str = r#"# 一级标题

## 二级标题

### 三级标题

#### 四级标题

##### 五级标题

###### 六级标题

正文段落，用来触发 paragraph 与 paragraph_leaf。

> 引用内容。

> [!NOTE]
> 提示卡片内容。

> [!TIP]
> 技巧卡片内容。

> [!IMPORTANT]
> 重要卡片内容。

> [!WARNING]
> 警告卡片内容。

> [!CAUTION]
> 严重警告卡片内容。

> [!COMMENT]
> 评论卡片内容。

> [!KEYPOINT]
> 重点卡片内容。

[TOC]

![示例图](https://example.com/pic.png)

*图注文字*

| 列A | 列B |
|---|---|
| 甲 | 乙 |

```rust
fn main() {}
```

- 列表项一
- 列表项二

$$E = mc^2$$

---
"#;

const PROBE_PROP: &str = "word-spacing";
const PROBE_VALUE: &str = "7.77px";

/// Render DOC with a theme that overrides exactly one role key.
fn render_with_override(key: &str) -> String {
    let src = format!(
        "[meta]\nid = \"probe\"\nname = \"probe\"\n\n[block.{key}]\n{PROBE_PROP} = \"{PROBE_VALUE}\"\n"
    );
    let theme =
        parse_theme(&src).unwrap_or_else(|e| panic!("probe theme for {key} must parse: {e}"));
    let mut opts = ConvertOptions::new(theme);
    // Keep mode leaves http(s) image URLs alone, so the figure renders (and
    // therefore emits its caption) without touching the network.
    opts.image_mode = ImageMode::Keep;
    convert_markdown(DOC, &opts)
        .unwrap_or_else(|e| panic!("convert with {key} override failed: {e}"))
        .html
}

/// True when the probe at byte offset `i` sits inside a `style="..."` of the
/// tag that most recently opened. A declaration that leaks into text content
/// would still satisfy a bare `contains`, so the forward check alone was too
/// weak to catch it.
fn probe_is_in_a_style_attr(html: &str, i: usize) -> bool {
    let before = &html[..i];
    let open = match before.rfind('<') {
        Some(o) => o,
        None => return false,
    };
    // A '>' after the last '<' would mean the probe is outside any tag.
    if before.rfind('>').is_some_and(|c| c > open) {
        return false;
    }
    before[open..].contains("style=\"")
}

#[test]
fn every_advertised_role_is_actually_consumed_by_the_renderer() {
    let mut ignored: Vec<String> = Vec::new();
    let mut misplaced: Vec<String> = Vec::new();
    for key in roles::all_keys() {
        let html = render_with_override(&key);
        let hits: Vec<usize> = html.match_indices(PROBE_VALUE).map(|(i, _)| i).collect();
        if hits.is_empty() {
            ignored.push(key);
            continue;
        }
        // Every occurrence must be a real CSS declaration inside a tag, never
        // loose text. This is what makes the check about *where* the value
        // landed rather than merely that it exists.
        if !hits.iter().all(|i| probe_is_in_a_style_attr(&html, *i)) {
            misplaced.push(key);
        }
    }
    assert!(
        ignored.is_empty(),
        "these theme roles are advertised (and may be written by an AI-generated theme) \
         but the renderer never reads them, so the styling is silently dropped: {ignored:?}"
    );
    assert!(
        misplaced.is_empty(),
        "these theme roles reach the output but not as a style declaration on an element, \
         so the value is rendered as visible text instead of being applied: {misplaced:?}"
    );
}

#[test]
fn card_roles_are_applied_independently() {
    // The exact shape of the original bug: card roles were computed then
    // discarded, so a card-only theme produced byte-identical output.
    let note = render_with_override("card_note");
    let tip = render_with_override("card_tip");
    assert_eq!(
        note.matches(PROBE_VALUE).count(),
        1,
        "a card_note override must reach exactly the note card container"
    );
    assert_eq!(
        tip.matches(PROBE_VALUE).count(),
        1,
        "a card_tip override must reach exactly the tip card container"
    );
    assert_ne!(
        note, tip,
        "card_note and card_tip must affect different cards, not the same one"
    );
}

#[test]
fn paragraph_leaf_still_controls_body_typography() {
    // The other half of the original bug: base_leaf ignored its role argument.
    let html = render_with_override("paragraph_leaf");
    assert!(
        html.contains(PROBE_VALUE),
        "paragraph_leaf override must reach body text"
    );
}

#[test]
fn theme_can_never_smuggle_font_family_through_a_role() {
    // R-3.1 is absolute: parse_theme rejects it outright.
    let src = "[meta]\nid = \"bad\"\nname = \"bad\"\n\n[block.quote]\nfont-family = \"serif\"\n";
    assert!(
        parse_theme(src).is_err(),
        "a theme must not be able to set font-family on any role"
    );
}

/// The reverse direction of the contract.
///
/// The loop above proves "everything declared is consumed". It cannot see a
/// role the renderer honours but the table forgot - which is exactly how h6
/// slipped through: `render_heading` builds `format!("h{}", level)` for
/// levels 1..=6 while `roles.rs` stopped at h5, so the theme validator
/// rejected a key the renderer supported.
///
/// This test drives every heading level the parser can emit and asserts the
/// override lands, so a missing declaration fails loudly.
#[test]
fn every_heading_level_is_themeable_end_to_end() {
    let mut md = String::new();
    for level in 1..=6u8 {
        md.push_str(&"#".repeat(level as usize));
        md.push_str(&format!(" 第{level}级\n\n"));
    }
    for level in 1..=6u8 {
        let id = format!("h{level}");
        let theme = parse_theme(&format!(
            "[meta]\nid = \"probe\"\nname = \"probe\"\n\n[block.{id}]\nword-spacing = \"7.77px\"\n"
        ))
        .unwrap_or_else(|e| panic!("probe theme for {id} must parse: {e}"));
        let mut opts = ConvertOptions::new(theme);
        opts.image_mode = ImageMode::Keep;
        let html = convert_markdown(&md, &opts)
            .unwrap_or_else(|e| panic!("convert for {id} failed: {e}"))
            .html;
        assert!(
            html.contains(PROBE_VALUE),
            "[block.{id}] is honoured by the renderer but does not reach the output"
        );
        assert!(
            roles::is_known_key(&id),
            "{id} is honoured by the renderer but is not declared in roles.rs, \
             so validate_generated_theme would reject a working theme"
        );
    }
}
