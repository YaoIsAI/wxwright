//! Agent hand-off card (PRD 6): a paste-into-system-prompt contract that
//! lets any AI agent operate wxwright immediately. Snapshot-tested against
//! the real tool surface (see tests).

use serde_json::{json, Value};

pub fn card_markdown() -> String {
    r##"# wxwright Agent Card (v1)

You are operating `wxwright`, the local engine behind the wxwright desktop app.
One Markdown source, one engine, and per platform its own artifact:

- `wechat` - WeChat Official Account (微信公众号) dialect rich text, styled so a
  paste into the MP editor has zero distortion.
- `xhs`, `meta`, `instagram`, `x`, `linkedin` - a plain-text caption (the image
  set is exported separately by the desktop app).
- `zhihu` - Markdown.

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

- Convert:  `wxwright convert article.md --theme minimal --platform wechat --out article.html`
- Validate: `wxwright validate article.md --platform xhs --strict`
- Copy:     `wxwright copy article.md`   (text/html to the clipboard; the user pastes into the MP editor)
- Fix:      `wxwright fix dirty.html --out clean.html`   (Markdown input is repaired too)
- Images:   `wxwright image upload a.png b.jpg` / `wxwright image fit article.md --platform xhs --out-dir out/`
- Themes:   `wxwright theme list` / `theme new mytheme` / `theme validate mytheme.toml`
- Doctor:   `wxwright doctor --strict` and `wxwright doctor --verify` (live credential check)
- Platforms: `wxwright platforms` (ids, limits, export kind per platform)
- Card:     `wxwright agent-card` (Markdown) or `wxwright agent-card --json`
- stdin:    use `-` as the input path.

## Output and exit-code contract

- `--json` forces machine output; a non-TTY stdout defaults to JSON.
- In JSON mode stdout is exactly one JSON document. Status lines, warnings and
  `--verbose` diagnostics go to stderr, so a pipe never mixes artifact and log.
- Every JSON document has `ok`. Exit codes:
  - `exit 0` success.
  - `exit 1` the command ran and the result is negative: blocking violations,
    violations left after `fix`, warnings under `--strict`, failed image
    uploads, doctor problems.
  - `exit 2` the command could not run: usage error, unreadable input, unknown
    platform id, missing credentials, a WeChat API failure.

## Artifacts per platform

- `convert --platform wechat` returns `html` (WeChat dialect document).
- `convert --platform xhs|meta|instagram|x|linkedin` returns `text` (caption)
  plus `title`; a blocking caption rule exits 1 (Xiaohongshu caps a note at
  1000 chars, X weights CJK as 2 chars against a 280 budget).
- `convert --platform zhihu` returns `text` (the Markdown body) plus `title`.
- `validate` reports the same rule set without producing an artifact, and its
  JSON includes the rendered `caption` for caption platforms.
- Frontmatter is honoured: `title`, `theme`, `platform`, `format: html`.
  An unknown `--platform` is exit 2; an unknown `platform:` in frontmatter
  warns on stderr and falls back to `wechat`.
- `format: html` (or an `.html` file, or stdin starting with `<`) skips the
  template renderer and runs normalize + validate. That entry is WeChat-only.

## Rules you must respect in source Markdown

- Write standard GFM. Use `> [!NOTE]`/`[!WARNING]` blockquotes for callout cards,
  `> [!COMMENT]` for a comment card, `> [!KEYPOINT]` for key-point emphasis.
- Tables/code blocks are fully supported and pre-normalized; do not hand-write HTML.
- Unclosed code fences and empty `]()` targets are reported as MD-1/MD-2: they
  only warn, so a CI gate should use `--strict`.
- Images: local paths are fine for `convert` (they get inlined) but clipboard
  copy requires mmbiz or https URLs. Configure 公众号 API credentials
  (`wxwright login`) to unlock automatic mmbiz upload.
- Never inline raw HTML styling: the engine forbids font-family, fixed px widths,
  span[leaf] block nesting (rules R-1.*, R-2.*, R-3.1).

## Full pipeline (one command)

`wxwright copy article.md --json`
runs: parse -> render(theme) -> image pipeline -> normalize -> validate -> clipboard.
`copy --dry-run` runs the same chain but writes neither the clipboard nor the
material library: with credentials it predicts that local images will upload.
Read the JSON, fix the source, retry. Do not ship while `blocking_count` > 0.

## MCP alternative

Start `wxwright mcp serve` (stdio) or `wxwright mcp install --target claude`.
Tools: wxwright_convert, wxwright_validate, wxwright_fix, wxwright_copy,
wxwright_themes_list, wxwright_upload_images, wxwright_draft_create,
wxwright_draft_update, wxwright_image_fit, wxwright_draft_list,
wxwright_draft_delete, wxwright_export.
Resources: wxwright://themes, wxwright://spec/rules.
Contract: every MCP `markdown` argument takes the article's TEXT CONTENT -
read the file first and pass its text; file paths are rejected with an error
(the CLI commands are the ones that accept paths).

## Publishing (optional, needs credentials)

- `wxwright login --appid <id> --secret-stdin` stores the Secret in the OS
  keychain and verifies it first; a rejected AppID/Secret stores nothing.
  `--no-verify` is for offline/CI, `--no-keyring` falls back to the config file.
- `wxwright draft create --file article.md` creates a 草稿箱 (Drafts) entry.
  Title, author, digest and theme default to the frontmatter; `--cover` or
  `--thumb-media-id` sets the cover (otherwise the first uploaded image).
- `wxwright draft update --media-id <id> --file article.md [--index N]` replaces
  the body; metadata you do not pass is preserved from the existing draft.
- `wxwright draft list` / `wxwright draft delete --media-id <id> --yes`.
- Blocking violations and paste-hostile images stop a draft before any API call.
- `wxwright publish <draft_media_id> --yes` calls freepublish (发布): it makes
  the article public on the account. It is NOT 群发 (mass send) and it is not
  reversible, so never run it without the human operator's explicit approval.
"##
    .to_string()
}

pub fn card_json() -> Value {
    json!({
        "name": "wxwright",
        "version": env!("CARGO_PKG_VERSION"),
        "role": "Markdown -> per-platform publishing artifacts (WeChat MP dialect rich text, captions, Markdown) with compliance validation; CLI + MCP + desktop app share the engine",
        "exit_codes": {
            "0": "success",
            "1": "ran with a negative result: blocking violations, violations left after fix, warnings under --strict, failed uploads, doctor problems",
            "2": "could not run: usage error, unreadable input, unknown platform id, missing credentials, WeChat API failure"
        },
        "global_flags": ["--json", "--lang <en|zh-cn>", "--no-color", "--verbose", "--version"],
        "output_contract": "in JSON mode stdout is one JSON document with an ok field; status lines and --verbose diagnostics go to stderr; a non-TTY stdout defaults to JSON",
        "cli": [
            { "cmd": "convert <input.md|input.html|->", "args": ["--theme <name|path>", "--platform <id>", "--input-format <markdown|html>", "--out <file|->", "--json"], "desc": "produce the target platform's artifact: html for wechat, text + title for caption/Markdown platforms" },
            { "cmd": "validate <input.md|input.html|->", "args": ["--platform <id>", "--strict", "--official-check", "--json"], "desc": "official editor spec + platform title/caption rules + Markdown structure (MD-1/MD-2); exit 1 on blocks, or on warnings with --strict" },
            { "cmd": "fix <input.md|input.html|->", "args": ["--out <file|->", "--json"], "desc": "normalizer repair for HTML and Markdown; exit 1 when blocking issues remain" },
            { "cmd": "copy <input.md|->", "args": ["--theme <name|path>", "--dry-run", "--json"], "desc": "full chain to the clipboard; --dry-run neither uploads nor touches the clipboard" },
            { "cmd": "image upload <paths...>", "args": ["--json"], "desc": "upload to the permanent material library, print media_id + mmbiz URL per file" },
            { "cmd": "image fit <input.md|-> --platform <id> --out-dir <dir>", "args": ["--json"], "desc": "centre-crop the article's local images to the platform cover shape and clamp width (never upscales); remote/inline images are skipped, not failures" },
            { "cmd": "draft create --file <md|html|->", "args": ["--title", "--author", "--digest", "--theme", "--source-url", "--thumb-media-id", "--cover <file>", "--json"], "desc": "create a 草稿箱 (Drafts) entry; metadata defaults to frontmatter" },
            { "cmd": "draft update --media-id <id> --file <md|->", "args": ["--title", "--author", "--digest", "--theme", "--source-url", "--thumb-media-id", "--cover <file>", "--index <n>", "--json"], "desc": "replace a draft body; omitted metadata is preserved from the existing draft" },
            { "cmd": "draft list", "args": ["--offset <n>", "--count <n>", "--json"], "desc": "list drafts" },
            { "cmd": "draft delete --media-id <id>", "args": ["--yes", "--json"], "desc": "irreversible; refuses without --yes" },
            { "cmd": "publish <draft_media_id>", "args": ["--yes", "--json"], "desc": "freepublish (发布, makes the article public; NOT 群发 mass send); refuses without --yes and needs explicit human approval" },
            { "cmd": "theme list|new <name>|validate <path>", "args": ["--force (new)"], "desc": "theme registry, TOML scaffold, and a render-and-validate check" },
            { "cmd": "platforms", "args": ["--json"], "desc": "platform ids, caption limits, export kind and image profile" },
            { "cmd": "doctor", "args": ["--strict", "--verify", "--json"], "desc": "environment health check; --verify does a live token check and reports the account nickname" },
            { "cmd": "mcp serve", "args": [], "desc": "stdio MCP server (newline-delimited JSON-RPC 2.0)" },
            { "cmd": "mcp install --target <claude|cursor|vscode|opencode>", "args": ["--json"], "desc": "write the MCP client config, backing up and preserving the previous file" },
            { "cmd": "login", "args": ["--appid <id>", "--secret <s> | --secret-stdin", "--no-verify", "--no-keyring", "--json"], "desc": "verify then store MP credentials in the OS keychain; a rejected AppID/Secret stores nothing; WXWRIGHT_MP_SECRET is the env fallback" },
            { "cmd": "logout", "args": ["--json"], "desc": "remove stored credentials" },
            { "cmd": "agent-card", "args": ["--md", "--json"], "desc": "this card (Markdown in a terminal, JSON when piped; --md forces Markdown)" },
            { "cmd": "bench", "args": ["--iters <n>", "--json"], "desc": "stage timings of the full in-process chain against the P-2 SLO budget (release build)" }
        ],
        "mcp": {
            "transport": "stdio",
            "start": "wxwright mcp serve",
            "tools": ["wxwright_convert", "wxwright_validate", "wxwright_fix", "wxwright_copy", "wxwright_themes_list", "wxwright_upload_images", "wxwright_draft_create", "wxwright_draft_update", "wxwright_image_fit", "wxwright_draft_list", "wxwright_draft_delete", "wxwright_export"],
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
            "wxwright validate article.md --platform xhs --strict --json",
            "wxwright convert article.md --platform xhs --out caption.txt --json",
            "wxwright image fit article.md --platform xhs --out-dir out/ --json",
            "wxwright copy article.md --dry-run --json",
            "wxwright doctor --verify --json",
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
            "wxwright_fix",
            "wxwright_copy",
            "wxwright_themes_list",
            "wxwright_upload_images",
            "wxwright_image_fit",
            "wxwright_draft_create",
            "wxwright_draft_update",
            "wxwright_draft_list",
            "wxwright_draft_delete",
            "wxwright_export",
        ] {
            assert!(j.contains(tool), "agent card missing tool {}", tool);
        }
        let md = super::card_markdown();
        assert!(md.contains("wxwright convert"));
        assert!(md.contains("exit 0"));
    }

    /// The card is the only contract an outside agent reads. A command that
    /// exists but is not on the card, or a flag that was renamed, silently
    /// strands the agent - so the surface is pinned here.
    #[test]
    fn card_covers_the_real_cli_surface() {
        let j = super::card_json();
        let cli = j["cli"].as_array().expect("cli array");
        let listed: Vec<&str> = cli
            .iter()
            .map(|c| c["cmd"].as_str().expect("cmd"))
            .collect();
        for required in [
            "convert",
            "validate",
            "fix",
            "copy",
            "image upload",
            "image fit",
            "draft create",
            "draft update",
            "draft list",
            "draft delete",
            "publish",
            "doctor",
            "platforms",
            "mcp serve",
            "mcp install",
            "login",
            "logout",
            "agent-card",
            "bench",
        ] {
            assert!(
                listed.iter().any(|c| c.starts_with(required)),
                "agent card does not document `{}`",
                required
            );
        }
        let md = super::card_markdown();
        for phrase in [
            "--platform",
            "--secret-stdin",
            "--dry-run",
            "--verify",
            "image fit",
            "freepublish",
        ] {
            assert!(md.contains(phrase), "markdown card missing {}", phrase);
        }
        // The old card called `publish` a 群发 mass send. It is freepublish.
        assert!(
            !md.contains("群发 (mass send); HUMAN-ONLY"),
            "markdown card still describes publish as mass send"
        );
        assert!(
            !j.to_string().contains("群发 (mass send); HUMAN-ONLY"),
            "json card still describes publish as mass send"
        );
    }

    /// A raw string that closes early truncates the card silently, which is
    /// exactly how the 群发 wording survived a rewrite once before.
    #[test]
    fn card_is_not_truncated() {
        let md = super::card_markdown();
        assert!(
            md.trim_end().ends_with("approval."),
            "markdown tail: {:?}",
            &md[md.len().saturating_sub(80)..]
        );
        let j = super::card_json();
        assert_eq!(j["cli"].as_array().expect("cli").len(), 20);
        assert!(j["examples"].as_array().expect("examples").len() >= 5);
    }
}
