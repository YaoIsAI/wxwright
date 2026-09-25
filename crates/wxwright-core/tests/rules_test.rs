//! Rule coverage tests (PRD 9-5): every rule in the 5.3 table must have at
//! least one violation sample that is detected, and auto-fixable rules must
//! be fixed by the normalizer.

use wxwright_core::normalizer::{normalize_html, NormalizeOptions};
use wxwright_core::validator::validate_html;

fn rule_ids(html: &str) -> Vec<String> {
    validate_html(html).into_iter().map(|v| v.rule_id).collect()
}

fn assert_detected(html: &str, rule: &str) {
    let ids = rule_ids(html);
    assert!(
        ids.iter().any(|i| i == rule),
        "expected {} to be detected in {:?}; got {:?}",
        rule,
        html,
        ids
    );
}

#[test]
fn r_1_1_opacity_zero_image() {
    assert_detected(
        r#"<section><img src="a.png" style="opacity:0"><svg style="background-image: url(b.png)"></svg></section>"#,
        "R-1.1",
    );
    let fixed = normalize_html(
        r#"<section><img src="a.png" style="opacity:0"></section>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains("opacity"));
}

#[test]
fn r_1_2_transparent_caret() {
    assert_detected(
        r#"<span style="caret-color: transparent">x</span>"#,
        "R-1.2",
    );
    let fixed = normalize_html(
        r#"<span style="caret-color: transparent">x</span>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains("caret-color"));
}

#[test]
fn r_1_3_line_height_below_font_size() {
    assert_detected(
        r#"<section style="font-size: 16px; line-height: 12px"><span leaf>some multi word text</span></section>"#,
        "R-1.3",
    );
    let fixed = normalize_html(
        r#"<section style="font-size: 16px; line-height: 12px"><span leaf>text</span></section>"#,
        Default::default(),
    )
    .unwrap();
    assert!(fixed.html.contains("line-height: 1.5"));
}

#[test]
fn r_1_4_fixed_px_width() {
    assert_detected(r#"<section style="width: 400px">text</section>"#, "R-1.4");
    let fixed = normalize_html(
        r#"<section style="width: 400px">text</section>"#,
        Default::default(),
    )
    .unwrap();
    assert!(fixed.html.contains("width: 59%"), "got {}", fixed.html);
    // data-ignore-width exempts the subtree.
    let ids = rule_ids(
        r#"<section data-ignore-width="1"><section style="width: 400px">text</section></section>"#,
    );
    assert!(!ids.contains(&"R-1.4".to_string()));
}

#[test]
fn r_1_4w_missing_data_w_is_warning() {
    let vs = validate_html(r#"<img src="https://example.com/a.png">"#);
    let v = vs
        .iter()
        .find(|v| v.rule_id == "R-1.4w")
        .expect("R-1.4w expected");
    assert_eq!(v.severity, "warn");
}

#[test]
fn r_1_5_1_height_zero_with_text() {
    assert_detected(r#"<p style="height: 0">invisible body text</p>"#, "R-1.5.1");
    let fixed =
        normalize_html(r#"<p style="height: 0">invisible</p>"#, Default::default()).unwrap();
    assert!(!fixed.html.contains("height: 0"));
}

#[test]
fn r_1_5_2_small_clip_height() {
    assert_detected(
        r#"<p style="height: 20px; overflow: hidden">clipped text content</p>"#,
        "R-1.5.2",
    );
    let fixed = normalize_html(
        r#"<p style="height: 20px; overflow: hidden">clipped</p>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains("height: 20px"));
}

#[test]
fn r_1_6_text_align_start_end() {
    assert_detected(r#"<p style="text-align: start">x</p>"#, "R-1.6");
    let fixed =
        normalize_html(r#"<p style="text-align: start">x</p>"#, Default::default()).unwrap();
    assert!(fixed.html.contains("text-align: left"));
}

#[test]
fn r_1_7_animate_touchstart_only() {
    assert_detected(
        r#"<svg><animate begin="touchstart" attributeName="opacity" to="1"></animate></svg>"#,
        "R-1.7",
    );
    let fixed = normalize_html(
        r#"<svg><animate begin="touchstart" attributeName="opacity"></animate></svg>"#,
        Default::default(),
    )
    .unwrap();
    assert!(fixed.html.contains("touchstart; click"));
}

#[test]
fn r_1_8_pre_with_text() {
    assert_detected("<pre>plain body text</pre>", "R-1.8");
    let fixed = normalize_html("<pre>plain body text</pre>", Default::default()).unwrap();
    assert!(!fixed.html.contains("<pre"));
    assert!(fixed.html.contains("white-space: pre-wrap"));
}

#[test]
fn r_2_1_deep_identical_chain() {
    let mut html = String::from("<span leaf>deep</span>");
    for _ in 0..12 {
        html = format!(r#"<section style="margin: 0">{}</section>"#, html);
    }
    let vs = validate_html(&html);
    let v = vs
        .iter()
        .find(|v| v.rule_id == "R-2.1")
        .expect("R-2.1 expected");
    assert!(!v.fixable, "R-2.1 has no auto-fix in v1");
}

#[test]
fn r_2_2_block_inside_leaf() {
    assert_detected(
        r#"<span leaf><section>block content here</section></span>"#,
        "R-2.2",
    );
    let fixed = normalize_html(
        r#"<span leaf><section>block content here</section></span>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains(" leaf"), "got {}", fixed.html);
}

#[test]
fn r_2_3_nonofficial_in_nodeleaf() {
    assert_detected(r#"<section nodeleaf><p>text</p></section>"#, "R-2.3");
    let fixed = normalize_html(
        r#"<section nodeleaf><img src="a.png"></section>"#,
        Default::default(),
    )
    .unwrap();
    assert!(fixed.html.contains("nodeleaf"), "img child keeps nodeleaf");
    let fixed = normalize_html(
        r#"<section nodeleaf><p>text</p></section>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains("nodeleaf"));
}

#[test]
fn r_3_1_font_family() {
    assert_detected(r#"<p style="font-family: serif !important">x</p>"#, "R-3.1");
    let fixed =
        normalize_html(r#"<p style="font-family: serif">x</p>"#, Default::default()).unwrap();
    assert!(!fixed.html.contains("font-family"));
}

#[test]
fn r_4_1_1_low_contrast_warn() {
    let vs =
        validate_html(r#"<span style="color: #999999; background-color: #888888">dim text</span>"#);
    let v = vs
        .iter()
        .find(|v| v.rule_id == "R-4.1.1")
        .expect("R-4.1.1 expected");
    assert_eq!(v.severity, "warn");
}

#[test]
fn r_4_1_2_gradient_under_text_warn() {
    let vs = validate_html(
        r#"<p style="background: linear-gradient(#ffffff, #000000)">gradient text</p>"#,
    );
    let v = vs
        .iter()
        .find(|v| v.rule_id == "R-4.1.2")
        .expect("R-4.1.2 expected");
    assert_eq!(v.severity, "warn");
    assert!(v.fixable);
    let fixed = normalize_html(
        r#"<p style="background: linear-gradient(#ffffff, #000000)">transition text</p>"#,
        NormalizeOptions { fix_dark: true },
    )
    .unwrap();
    assert!(
        !fixed.html.contains("linear-gradient"),
        "got {}",
        fixed.html
    );
    assert!(fixed.html.contains("#ffffff"));
}

#[test]
fn r_4_2_absolute_position_warn() {
    let vs = validate_html(r#"<section style="position: absolute; top: 0">x</section>"#);
    assert!(vs.iter().any(|v| v.rule_id == "R-4.2"));
}

#[test]
fn r_4_3_image_carries_text_warn() {
    let vs = validate_html(
        r#"<img src="https://example.com/a.png" alt="a very long paragraph of text baked into the image pixels">"#,
    );
    assert!(vs.iter().any(|v| v.rule_id == "R-4.3"));
}

#[test]
fn r_4_4_svg_text_warn() {
    let vs = validate_html(r#"<svg><text>hello</text></svg>"#);
    assert!(vs.iter().any(|v| v.rule_id == "R-4.4"));
}

#[test]
fn hygiene_script_removed() {
    assert_detected(
        r#"<section>ok</section><script>alert(1)</script>"#,
        "HYGIENE",
    );
    let fixed = normalize_html(
        r#"<section class="a" id="b" onclick="x">ok</section><script>alert(1)</script>"#,
        Default::default(),
    )
    .unwrap();
    assert!(!fixed.html.contains("script"));
    assert!(!fixed.html.contains("class="));
    assert!(!fixed.html.contains("onclick"));
}

#[test]
fn compliant_html_is_clean() {
    let html = r#"<section style="margin: 0 0 16px;"><span leaf style="font-size: 15px; line-height: 1.75;">clean text</span></section>"#;
    let blocks: Vec<_> = validate_html(html)
        .into_iter()
        .filter(|v| v.is_block())
        .collect();
    assert!(blocks.is_empty(), "{:?}", blocks);
}
