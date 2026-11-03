# wxwright Agent Card (v1)

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

