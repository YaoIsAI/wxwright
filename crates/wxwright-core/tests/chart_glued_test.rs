//! The chart-json isolation preprocessor (parser::isolate_chart_lines): a
//! model drops spec lines straight under a table or glues them to prose, and
//! CommonMark used to absorb them into the previous block - readers saw raw
//! JSON. These pin the live shapes end to end.

use wxwright_core::parser::parse_markdown;

fn chart_count(blocks: &[wxwright_core::ir::Block]) -> usize {
    blocks
        .iter()
        .filter(|b| matches!(b, wxwright_core::ir::Block::Chart { .. }))
        .count()
}

#[test]
fn chart_line_after_a_table_is_isolated_and_rendered() {
    let md = "| 年 | 量 |\n| --- | --- |\n| 2023 | 120 |\n趋势图\n{\"kind\":\"bar\",\"title\":\"t\",\"labels\":[\"a\",\"b\"],\"values\":[1,2]}\n";
    let blocks = parse_markdown(md);
    assert_eq!(
        chart_count(&blocks),
        1,
        "the chart line must survive the table above it: {:?}",
        blocks
    );
}

#[test]
fn two_spec_lines_in_one_reply_render_two_charts() {
    let bar = "{\"kind\":\"bar\",\"title\":\"b\",\"labels\":[\"a\",\"b\"],\"values\":[1,2]}";
    let pie = "{\"kind\":\"pie\",\"title\":\"p\",\"labels\":[\"a\",\"b\"],\"values\":[1,2]}";
    let md = format!("趋势图\n{bar}\n占比\n{pie}\n备注。\n");
    let blocks = parse_markdown(&md);
    assert_eq!(chart_count(&blocks), 2, "both specs render: {:?}", blocks);
    // prose survives around them
    assert!(
        blocks.len() >= 5,
        "prose lines stay as paragraphs: {:?}",
        blocks
    );
}
