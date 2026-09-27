# wxwright Agent Card (v1)

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

