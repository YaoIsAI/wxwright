//! Documentation drift guard (iron law 9).
//!
//! The agent card is generated, so it cannot rot silently. The hand-written
//! docs around it can: `docs/agent-integration.md` once claimed `publish` was
//! a 群发 mass send, listed 9 MCP tools instead of 12, and gave a command table
//! with wrong exit codes - an agent that trusted it would have mis-operated
//! the CLI. These assertions pin the prose to the generated contract.

use std::path::Path;

use wxwright_core::agentcard::card_json;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

/// Markdown emphasis and soft line wraps must not hide a stale sentence, so
/// every doc is compared through this normaliser.
fn read(rel: &str) -> String {
    let raw = std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", rel, e));
    raw.replace("**", "")
        .replace('`', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The leading command words of a card entry: `image upload <paths...>` ->
/// ["image", "upload"], `theme list|new <name>` -> ["theme", "list"].
fn command_words(cmd: &str) -> Vec<String> {
    let head = cmd.split('<').next().unwrap_or(cmd);
    head.split_whitespace()
        .filter_map(|w| w.split('|').next().map(str::to_string))
        .filter(|w| !w.is_empty() && !w.starts_with('-'))
        .take(2)
        .collect()
}

fn tools() -> Vec<String> {
    card_json()["mcp"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t.as_str().expect("tool name").to_string())
        .collect()
}

#[test]
fn handbook_documents_the_whole_cli_surface() {
    let doc = read("docs/agent-integration.md");
    let cli = card_json()["cli"].as_array().expect("cli array").clone();
    assert_eq!(cli.len(), 20, "the card pins a 20-command surface");
    let mut missing: Vec<String> = Vec::new();
    for entry in &cli {
        let cmd = entry["cmd"].as_str().expect("cmd");
        for word in command_words(cmd) {
            if !doc.contains(&word) {
                missing.push(format!("{} (needed by `{}`)", word, cmd));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "docs/agent-integration.md does not document: {}\nRebuild the command table from `wxwright agent-card --json`.",
        missing.join(", ")
    );
}

#[test]
fn handbook_lists_every_mcp_tool_with_its_count() {
    let doc = read("docs/agent-integration.md");
    let list = tools();
    for tool in &list {
        assert!(
            doc.contains(tool),
            "docs/agent-integration.md omits MCP tool {}",
            tool
        );
    }
    assert_eq!(list.len(), 12, "the MCP surface is 12 tools");
    assert!(
        doc.contains(&format!("Tools ({}):", list.len())),
        "the handbook must state the tool count, not just list them"
    );
    for resource in card_json()["mcp"]["resources"]
        .as_array()
        .expect("resources array")
    {
        let r = resource.as_str().expect("resource");
        assert!(doc.contains(r), "handbook omits resource {}", r);
    }
}

/// The accident this guard exists for: prose that teaches an agent that
/// `publish` is 群发, when it is freepublish (发布).
#[test]
fn publish_is_never_documented_as_mass_send() {
    let doc = read("docs/agent-integration.md");
    assert!(
        doc.contains("freepublish"),
        "the handbook must name the API behind publish"
    );
    assert!(
        doc.contains("not 群发"),
        "the handbook must distinguish 发布 from 群发"
    );
    assert!(
        !doc.contains("publishes 群发") && !doc.contains("performs 群发"),
        "the handbook claims publish performs 群发"
    );
    for rel in ["README.md", "README.zh-CN.md", "README_GITEE.md", "PRD.md"] {
        let text = read(rel);
        // The stale phrasing put the API name after the word for mass send:
        // "群发 (publish)" / "群发（publish）" / "# 群发（P2）".
        for alias in [
            "群发 (publish",
            "群发(publish",
            "群发（publish",
            "群发（P2",
            "Mass publish (publish",
            "mass publish (publish",
        ] {
            assert!(
                !text.contains(alias),
                "{} still labels publish a 群发 mass send ({})",
                rel,
                alias
            );
        }
    }
}

#[test]
fn readmes_and_tutorial_state_the_generated_counts() {
    let count = tools().len();
    let platforms = wxwright_core::platform::list_platforms().len();
    assert!(
        read("README.md").contains(&format!("{} tools:", count)),
        "README.md tool count is stale"
    );
    for rel in ["README.zh-CN.md", "README_GITEE.md"] {
        assert!(
            read(rel).contains(&format!("{} 个工具", count)),
            "{} tool count is stale",
            rel
        );
    }
    let tutorial = read("docs/tutorial.md");
    assert!(
        tutorial.contains(&format!("{} 个 MCP 工具", count)),
        "docs/tutorial.md MCP tool count is stale"
    );
    assert!(
        tutorial.contains(&format!("\"values\":[{},{},", platforms, count)),
        "docs/tutorial.md overview chart no longer matches {} platforms / {} tools",
        platforms,
        count
    );
}

/// The exit-code contract is what an agent branches on; a table that says
/// `convert` can only exit 0/2 teaches it to ignore blocking violations.
#[test]
fn handbook_keeps_the_output_and_exit_contract() {
    let doc = read("docs/agent-integration.md");
    for phrase in [
        "stdout is exactly one JSON document",
        "artifact is written before the report",
        "0 / 1 / 2",
        "unknown --platform id",
        "--secret-stdin",
        "uploads_skipped",
        "--dry-run",
        "--input-format",
    ] {
        assert!(
            doc.contains(phrase),
            "handbook dropped the output/exit contract detail: {}",
            phrase
        );
    }
    let card = card_json();
    let codes = card["exit_codes"].as_object().expect("exit codes");
    let spelled = [
        ("0", "0 success"),
        ("1", "1 the command ran and the result is negative"),
        ("2", "2 the command could not run"),
    ];
    for (code, phrase) in spelled {
        assert!(
            codes.contains_key(code),
            "agent card lost exit code {}",
            code
        );
        assert!(
            doc.contains(phrase),
            "handbook dropped exit code {} ({})",
            code,
            phrase
        );
    }
}
