//! Agent hand-off card (PRD 6): a paste-into-system-prompt contract that
//! lets any AI agent operate wxwright immediately. Snapshot-tested against
//! the real tool surface (see tests).

use serde_json::{json, Value};

pub fn card_markdown() -> String {
    r#"# wxwright Agent Card (v1)

You are operating `wxwright`, a local engine that converts Markdown into
WeChat Official Account (微信公众平台) compliant rich text with ZERO style
distortion when pasted into the MP editor.

## Bootstrap (skip only if `wxwright --version` already works)

If this machine has no wxwright yet, install it before anything else:

1. Download a prebuilt binary from
   https://github.com/YaoIsAI/wxwright/releases
   (Windows: wxwright-cli-windows-x64.zip; macOS: arm64/x64 tar.gz;
   Linux: tar.gz), or build from source:
   `git clone https://github.com/YaoIsAI/wxwright && cd wxwright && cargo build --release -p wxwright-cli`
2. Put the binary on PATH and verify with `wxwright doctor` (all checks ok).
3. The desktop GUI installer (wxwright-*-setup.exe / .dmg / .deb) bundles
   the same engine - not required for agent work, recommended for humans.

## Core contract

- Convert:  `wxwright convert article.md --theme minimal --out article.html`
- Validate: `wxwright validate article.md --strict`   (exit 0 pass / 1 violation / 2 error)
- Copy:     `wxwright copy article.md`                (writes text/html to clipboard; user pastes into the 公众号 editor)
- Fix:      `wxwright fix dirty.html --out clean.html`
- Themes:   `wxwright theme list`
- Doctor:   `wxwright doctor`                         (environment check)
- JSON out: append `--json` to any command for a stable machine-readable schema; non-TTY defaults to JSON.
- stdin:    use `-` as input path.
- Platforms: `wxwright platforms` lists the ids (wechat / xhs / zhihu / meta /
  instagram / x / linkedin). `wxwright convert --platform xhs` and the MCP
  `wxwright_export` tool return the artifact each platform actually consumes:
  WeChat = dialect rich text, Xiaohongshu and the western feeds = plain-text
  caption, Zhihu = Markdown.

## Full pipeline (one command)

`wxwright copy article.md --json`
runs: parse -> render(theme) -> image pipeline -> normalize -> validate -> clipboard.
The JSON result contains: warnings, fix records, validation findings, per-image status.
Read it, fix the source, retry. Do not ship while `blocking_violations` is non-empty.

## Rules you must respect in source Markdown

- Write standard GFM. Use `> [!NOTE]`/`[!WARNING]` blockquotes for callout cards,
  `> [!COMMENT]` for a comment card, `> [!KEYPOINT]` for key-point emphasis.
- Tables/code blocks are fully supported and pre-normalized; do not hand-write HTML.
- Images: local paths are fine for `convert` (they get inlined) but clipboard copy
  requires mmbiz or https URLs. Configure 公众号 API credentials (`wxwright login`)
  to unlock automatic mmbiz upload.
- Never inline raw HTML styling: the engine forbids font-family, fixed px widths,
  span[leaf] block nesting (rules R-1.*, R-2.*, R-3.1).

## MCP alternative

Start `wxwright mcp serve` (stdio) or `wxwright mcp install --target claude`.
Tools: wxwright_convert, wxwright_validate, wxwright_copy, wxwright_themes_list,
wxwright_upload_images, wxwright_draft_create, wxwright_draft_list,
wxwright_export.
Resources: wxwright://themes, wxwright://spec/rules.

## Publishing (optional, needs credentials)

- `wxwright draft create --file article.md --title "..."` creates a 草稿箱 (Drafts) entry.
- `wxwright draft update|list` manages existing drafts.
- 群发 (mass send) is intentionally NOT part of the agent surface: it is
  irreversible and reaches real subscribers, so a human must trigger it from
  the desktop app. Do not look for a way to automate it.
"#
    .to_string()
}

pub fn card_json() -> Value {
    json!({
        "name": "wxwright",
        "version": env!("CARGO_PKG_VERSION"),
        "role": "Markdown -> WeChat MP compliant rich text engine (CLI + MCP)",
        "exit_codes": { "0": "success", "1": "rule violations (blocking)", "2": "runtime error" },
        "cli": [
            { "cmd": "convert <input.md|->", "args": ["--theme <name|path>", "--platform <id>", "--out <file.html|->", "--json"], "desc": "markdown to the target platform's artifact (default: MP dialect HTML)" },
            { "cmd": "validate <input.md|html>", "args": ["--json", "--strict", "--official-check"], "desc": "compliance validation" },
            { "cmd": "fix <input.html>", "args": ["--out <fixed.html>"], "desc": "auto-fix violations, report mode" },
            { "cmd": "copy <input.md>", "args": ["--theme", "--dry-run", "--json"], "desc": "full chain, write rich text to clipboard" },
            { "cmd": "theme list|new|validate", "args": [], "desc": "theme scaffolding" },
            { "cmd": "doctor", "args": ["--json"], "desc": "environment health check" },
            { "cmd": "mcp serve", "args": [], "desc": "stdio MCP server" },
            { "cmd": "mcp install", "args": ["--target <claude|cursor|vscode|opencode>"], "desc": "write MCP client config" },
            { "cmd": "login / logout", "args": ["--appid", "--secret"], "desc": "store MP credentials in OS keychain" },
            { "cmd": "image upload <paths...>", "args": ["--json"], "desc": "upload to material library, print mmbiz mapping" },
            { "cmd": "draft create|update|list", "args": ["--file", "--title", "--author", "--digest", "--thumb-media-id"], "desc": "草稿箱 (Drafts) API" },
            { "cmd": "publish <draft_id>", "args": ["--yes"], "desc": "群发 (mass send); HUMAN-ONLY, irreversible, not for agents" },
            { "cmd": "agent-card", "args": ["--md", "--json"], "desc": "this card" }
        ],
        "mcp": {
            "transport": "stdio",
            "start": "wxwright mcp serve",
            "tools": ["wxwright_convert", "wxwright_validate", "wxwright_copy", "wxwright_themes_list", "wxwright_upload_images", "wxwright_draft_create", "wxwright_draft_list", "wxwright_export"],
            "resources": ["wxwright://themes", "wxwright://spec/rules"],
            "prompts": ["wxwright-publish-guide"]
        },
        "rules_redlines": [
            "no font-family anywhere (R-3.1)",
            "no fixed px width on containers (R-1.4)",
            "no span[leaf] containing block elements (R-2.2)",
            "no <pre> for plain text (R-1.8)",
            "no height:0 on text containers (R-1.5.1)",
            "images must be mmbiz or https for clipboard copy (I-03)"
        ],
        "examples": [
            "wxwright convert article.md --theme minimal --out article.html --json",
            "echo '# Hello' | wxwright convert - --json",
            "wxwright copy article.md --dry-run --json",
            "wxwright validate article.md --strict --json",
            "wxwright mcp install --target claude"
        ]
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn card_mentions_real_tools() {
        let j = super::card_json().to_string();
        for tool in [
            "wxwright_convert",
            "wxwright_validate",
            "wxwright_copy",
            "wxwright_themes_list",
            "wxwright_upload_images",
            "wxwright_draft_create",
            "wxwright_draft_list",
            "wxwright_export",
        ] {
            assert!(j.contains(tool), "agent card missing tool {}", tool);
        }
        let md = super::card_markdown();
        assert!(md.contains("wxwright convert"));
        assert!(md.contains("exit 0"));
    }
}
