//! Golden property tests (PRD 9-1/9-4): the full pipeline over a
//! comprehensive sample document must produce dialect HTML that is
//! structurally compliant for every built-in theme.

use std::sync::Arc;

use wxwright_core::img::{ImageMode, ImageTransport};
use wxwright_core::theme::{builtin_themes, load_theme};
use wxwright_core::validator::validate_html;
use wxwright_core::{convert_markdown, pipeline, ConvertOptions};

pub const SAMPLE: &str = r#"# wxwright 示例文章

这是一段**加粗**、*斜体*、~~删除线~~和`行内代码`的正文，附带一个[链接](https://example.com/docs)。

## 二级标题

- 无序列表项一
- 无序列表项二
    - 嵌套项
- [x] 已完成任务
- [ ] 待办任务

1. 有序第一
2. 有序第二

> 这是一段引用文字。

> [!NOTE]
> 这是一个提示卡片。

> [!KEYPOINT] 关键结论一句话。

| 功能 | 状态 | 备注 |
|:-----|:----:|-----:|
| 表格 | 支持 | 自适应 |
| 代码 | 支持 | 高亮 |

```rust
fn main() {
    println!("hello wxwright");
}
```

公式：质能方程 $E = mc^2$ 与下式：

$$\int_0^1 x^2 dx = \frac{1}{3}$$

---

## 结尾

段落结束。
"#;

/// No fixed pixel `width:` / `max-width:` declarations in output
/// (min-width on table cells is explicitly allowed, F-05).
fn assert_no_fixed_px_width(html: &str) {
    let mut rest = html;
    while let Some(pos) = rest.find("-width:") {
        let after = &rest[pos + "-width:".len()..];
        let value: String = after
            .chars()
            .take_while(|c| *c != ';')
            .collect::<String>()
            .trim()
            .to_string();
        let is_min = rest[..pos].ends_with("min");
        if !is_min && value.ends_with("px") {
            panic!(
                "fixed px width found: ...{}...",
                &rest[pos.saturating_sub(40)..pos + 30]
            );
        }
        rest = after;
    }
}

fn assert_dialect_invariants(html: &str, theme_id: &str) {
    assert!(
        !html.contains("font-family"),
        "{}: font-family leaked",
        theme_id
    );
    assert!(
        !html.contains("class=\""),
        "{}: class attribute leaked",
        theme_id
    );
    assert!(
        !html.to_lowercase().contains("<script"),
        "{}: script leaked",
        theme_id
    );
    assert!(!html.contains("<pre"), "{}: pre leaked", theme_id);
    assert!(html.contains("<section"), "{}: no sections", theme_id);
    assert!(html.contains("span leaf"), "{}: no leaf spans", theme_id);
    assert_no_fixed_px_width(html);
    let blocks: Vec<_> = validate_html(html)
        .into_iter()
        .filter(|v| v.is_block())
        .collect();
    assert!(
        blocks.is_empty(),
        "{}: engine output has blocking violations: {:?}",
        theme_id,
        blocks
            .iter()
            .map(|v| (&v.rule_id, &v.node))
            .collect::<Vec<_>>()
    );
}

#[test]
fn all_builtin_themes_produce_compliant_html() {
    for theme in builtin_themes().unwrap() {
        let out = pipeline(SAMPLE, &ConvertOptions::new(theme.clone())).unwrap();
        assert_dialect_invariants(&out.html, &theme.meta.id);
        assert!(out.stats.chars > 100);
        assert!(out.html.contains("References") || out.html.contains("参考链接"));
    }
}

#[test]
fn pipeline_features_render() {
    let theme = load_theme("minimal").unwrap();
    let out = pipeline(SAMPLE, &ConvertOptions::new(theme)).unwrap();
    let html = &out.html;
    assert!(html.contains("Note") || html.contains("笔记"));
    assert!(html.contains("Key Points") || html.contains("划重点"));
    assert!(html.contains("<table") && html.contains("min-width"));
    assert!(html.contains("color: #"), "expected highlighted tokens");
    assert!(html.contains("data-ignore-width"));
    assert!(html.contains("[1]") && html.contains("https://example.com/docs"));
    assert!(html.contains("E = mc^2"));
    assert!(html.contains("border-top"));
    assert!(out.stats.headings >= 2);
    assert!(out.stats.code_blocks == 1);
    assert!(out.stats.tables == 1);
}

#[test]
fn local_images_get_data_w_and_inline_base64() {
    let dir = std::env::temp_dir().join(format!("wxwright-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img_path = dir.join("pic.png");
    let mut img = image::RgbaImage::new(3, 2);
    for p in img.pixels_mut() {
        *p = image::Rgba([47, 108, 234, 255]);
    }
    img.save(&img_path).unwrap();

    let md = "# T\n\n![alt text](pic.png)\n";
    let opts = ConvertOptions {
        theme: load_theme("minimal").unwrap(),
        image_mode: ImageMode::Inline,
        base_dir: Some(dir.clone()),
        transport: None,
    };
    let out = convert_markdown(md, &opts).unwrap();
    assert!(
        out.html.contains("data-w=\"3\""),
        "data-w missing: {}",
        out.html
    );
    assert!(
        out.html.contains("data-ratio=\"0.6667\""),
        "ratio missing: {}",
        out.html
    );
    assert!(
        out.html.contains("data:image/png;base64,"),
        "base64 missing"
    );
    assert_eq!(out.images.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

struct NoopTransport;

impl ImageTransport for NoopTransport {
    fn fetch(&self, _url: &str) -> wxwright_core::error::Result<Vec<u8>> {
        Ok(vec![0x89, 0x50, 0x4E, 0x47])
    }
    fn upload_material(
        &self,
        _bytes: Vec<u8>,
        _filename: &str,
    ) -> wxwright_core::error::Result<(String, String)> {
        Ok((
            "MEDIA_ID_1".into(),
            "https://mmbiz.qpic.cn/mm/1?wx_fmt=png".into(),
        ))
    }
}

#[test]
fn upload_mode_rewrites_to_mmbiz() {
    let md = "# T\n\n![pic](https://example.com/pic.png)\n";
    let opts = ConvertOptions {
        theme: load_theme("minimal").unwrap(),
        image_mode: ImageMode::Upload,
        base_dir: None,
        transport: Some(Arc::new(NoopTransport)),
    };
    let out = convert_markdown(md, &opts).unwrap();
    assert!(out.html.contains("mmbiz.qpic.cn"), "got {}", out.html);
    assert!(out.images[0].mmbiz);
}

#[test]
fn url_image_kept_in_degrade_mode_with_warning() {
    let md = "# T\n\n![pic](https://example.com/pic.png)\n";
    let opts = ConvertOptions::new(load_theme("minimal").unwrap());
    let out = convert_markdown(md, &opts).unwrap();
    assert!(out.html.contains("https://example.com/pic.png"));
    assert!(out.images[0].warning.is_some());
}

#[test]
fn normalize_is_idempotent_on_engine_output() {
    let theme = load_theme("techblue").unwrap();
    let out = pipeline(SAMPLE, &ConvertOptions::new(theme)).unwrap();
    let again = wxwright_core::normalizer::normalize_html(&out.html, Default::default()).unwrap();
    assert_eq!(
        again.fixes.len(),
        0,
        "second pass should be a no-op: {:?}",
        again.fixes
    );
}

#[test]
fn toc_and_center_heading() {
    let md = "[TOC]\n\n# One\n\n## Two {.center}\n\nbody\n";
    let theme = load_theme("minimal").unwrap();
    let out = convert_markdown(md, &ConvertOptions::new(theme)).unwrap();
    assert!(out.html.contains("1. One"));
    assert!(out.html.contains("2. Two"));
    assert!(out.html.contains("text-align: center"));
}

#[test]
fn svg_component_passthrough_renders_verbatim() {
    let md = "# T

<section style=\"margin: 16px 0;\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 677 140\" style=\"width: 100%; display: block;\"><rect x=\"2\" y=\"2\" width=\"673\" height=\"136\" rx=\"14\" fill=\"#F7F8FA\" stroke=\"#2F6CEA\" stroke-width=\"3\"/><defs><clipPath id=\"wclip\"><rect x=\"8\" y=\"8\" width=\"100\" height=\"100\"/></clipPath></defs><text x=\"30\" y=\"62\" font-size=\"24\" font-family=\"sans-serif\">点击展开</text></svg></section>
";
    let theme = load_theme("minimal").unwrap();
    let out = pipeline(md, &ConvertOptions::new(theme)).unwrap();
    // verbatim pass-through: svg, clipPath id and font-family survive
    assert!(
        out.html.contains("<svg"),
        "svg escaped: {}",
        &out.html[..400]
    );
    assert!(out.html.contains("wclip"), "clipPath id stripped");
    assert!(out.html.contains("font-family"), "svg font-family stripped");
    assert!(
        !out.html.contains("&lt;svg"),
        "escaped instead of passed through"
    );
    // and the embedded block itself must remain compliant
    assert!(
        out.blocking_violations().is_empty(),
        "svg embed introduced violations"
    );
}

#[test]
fn dangerous_svg_block_is_escaped() {
    let md = "# T

<svg onload=\"alert(1)\"><rect width=\"10\" height=\"10\"/></svg>
";
    let theme = load_theme("minimal").unwrap();
    let out = convert_markdown(md, &ConvertOptions::new(theme)).unwrap();
    assert!(out.html.contains("&lt;svg"), "dangerous svg passed through");
}

#[test]
fn raw_html_block_is_escaped_not_passed_through() {
    let md = "# T\n\n<div onclick=\"evil()\">raw</div>\n";
    let theme = load_theme("minimal").unwrap();
    let out = convert_markdown(md, &ConvertOptions::new(theme)).unwrap();
    assert!(!out.html.contains("<div"));
    assert!(out.html.contains("&lt;div"));
}
