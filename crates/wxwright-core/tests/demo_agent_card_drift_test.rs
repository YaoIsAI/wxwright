//! Lock the browser-demo fallback card to the drift-protected source (AUD-006).
//! The GUI demo fetches gui/ui/demo-agent-card.md; its MCP tool paragraph must
//! always match agentcard.rs, the single source of truth iron law 9 guards.

use std::path::Path;

/// The "Tools:" paragraph, including its wrapped continuation lines
/// (the 7-tool list does not fit on one line).
fn tools_block(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut started = false;
    for line in text.lines() {
        if line.trim_start().starts_with("Tools:") {
            started = true;
        }
        if started {
            if line.trim().is_empty() {
                break;
            }
            lines.push(line.trim());
        }
    }
    lines.join(" ")
}

#[test]
fn demo_agent_card_matches_core_tool_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let demo = std::fs::read_to_string(root.join("gui/ui/demo-agent-card.md"))
        .expect("gui/ui/demo-agent-card.md exists in the repo");
    let real = wxwright_core::agentcard::card_markdown();
    let demo_tools = tools_block(&demo);
    assert!(
        demo_tools.starts_with("Tools:"),
        "demo card has a Tools: paragraph"
    );
    assert_eq!(
        demo_tools,
        tools_block(&real),
        "demo-agent-card.md drifted from agentcard.rs; regenerate with:\n  wxwright agent-card --md > gui/ui/demo-agent-card.md"
    );
    assert_eq!(
        demo_tools.matches("wxwright_").count(),
        9,
        "the MCP tool surface is 9 tools (iron law 9)"
    );
}
