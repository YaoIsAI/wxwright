# wxwright agent integration handbook

Machine-friendly reference for AI agents. `wxwright agent-card --json` is the
authoritative, versioned snapshot of the same surface; this handbook is the
prose around it. If the two ever disagree, the card wins - file a bug.

Everything below was verified against the shipped binary by
`crates/wxwright-cli/tests/cli_behavior.rs` (20 end-to-end tests that run the
real CLI in a sandboxed config root with no network).

## Golden path (clipboard flow, zero credentials)

```
1. agent writes article.md (standard GFM; use blockquote alert syntax for cards)
2. wxwright validate article.md --strict      -> exit 0 required
3. wxwright copy article.md --json            -> read the JSON result
4. tell the human: open the MP editor, Ctrl+V
```

Check the chain before touching a real clipboard with
`wxwright copy article.md --dry-run --json`. A dry run runs the same parse ->
render -> image -> normalize -> validate chain but writes neither the clipboard
nor the material library; with credentials present it *predicts* that local
images would upload (`uploads_skipped: true`) instead of skipping the image
gate and reporting a false pass.

If step 3 returns `"copied": false`, the JSON says why (`reason`):

- `blocking violations` - fix the markdown source, go to step 2. Never copy a
  non-compliant payload; the engine refuses to do it either.
- `paste-hostile images` - local/base64 images break on paste. Either enable
  credentials (below) or drop local images.

## Golden path (unattended draft flow, needs credentials)

```
0. printf '%s' "$SECRET" | wxwright login --appid <APPID> --secret-stdin --json
   # verified first, then stored in the OS keychain; a rejected pair stores nothing
1. wxwright draft create --file article.md --json
   -> images upload to the permanent material library (mmbiz rewrite),
      first uploaded image becomes the cover thumb
2. report draft_media_id; the human reviews in 公众号 -> 草稿箱 (Drafts)
```

Prefer `--secret-stdin` over `--secret`: argv leaks into shell history and the
process list. `WXWRIGHT_MP_SECRET` is the env fallback. A secret is never
echoed - every response that mentions it uses `mask()`.

`wxwright publish <draft_media_id> --yes` calls **freepublish (发布)**: it makes
the article public on the account. It is **not** 群发 (mass send) - mass send
has no API and is still performed by the human in the MP backend. freepublish
is not reversible either, so never run it without the human operator's explicit
instruction. Without `--yes` it refuses (exit 2) before any API call.

## Command surface

`wxwright --help` lists these; the agent card pins the same 20 entries in
`cli[]`, so a renamed flag turns CI red.

| Command | Key args | Exit codes |
|---|---|---|
| `convert <input\|->` | `--theme <name\|path>`, `--platform <id>`, `--input-format <markdown\|html>`, `--out <file\|->` | 0 / 1 / 2 |
| `validate <input\|->` | `--platform <id>`, `--strict`, `--official-check` | 0 / 1 / 2 |
| `fix <input\|->` | `--out <file\|->` | 0 / 1 / 2 |
| `copy <input\|->` | `--theme <name\|path>`, `--dry-run` | 0 / 1 / 2 |
| `image upload <paths...>` | - | 0 / 1 / 2 |
| `image fit <input\|-> --platform <id> --out-dir <dir>` | - | 0 / 1 / 2 |
| `draft create --file <input\|->` | `--title --author --digest --theme --source-url --thumb-media-id --cover <file>` | 0 / 1 / 2 |
| `draft update --media-id <id> --file <input\|->` | same, plus `--index <n>`; omitted metadata is preserved | 0 / 1 / 2 |
| `draft list` | `--offset <n>` (0), `--count <n>` (10) | 0 / 2 |
| `draft delete --media-id <id>` | `--yes` (irreversible) | 0 / 2 |
| `publish <draft_media_id>` | `--yes` (freepublish, not 群发) | 0 / 2 |
| `theme list` | - | 0 |
| `theme new <name>` | `--force` (overwrite) | 0 / 2 |
| `theme validate <path>` | - | 0 / 1 / 2 |
| `platforms` | - | 0 |
| `doctor` | `--strict`, `--verify` | 0 / 1 |
| `mcp serve` | stdio MCP server | 0 |
| `mcp install --target <claude\|cursor\|vscode\|opencode>` | - | 0 / 2 |
| `login` / `logout` | `--appid <id>`, `--secret <s>` \| `--secret-stdin`, `--no-verify`, `--no-keyring` | 0 / 2 |
| `bench` | `--iters <n>` (30) | 0 / 2 |
| `agent-card` | `--md` (force Markdown under `--json`) | 0 |

Global flags (accepted anywhere): `--json`, `--lang <en|zh-CN>`, `--no-color`
(`NO_COLOR` also honored), `--verbose`.

## Output contract

Read this before parsing anything.

- `--json` forces machine output; when stdout is **not** a TTY, JSON is the
  default. Piping stdout therefore always yields JSON.
- In JSON mode stdout is **exactly one** JSON document. Status lines, warnings,
  violation lists and `--verbose` diagnostics all go to stderr, so a pipe never
  mixes an artifact with a log.
- Every JSON document carries `ok`. Failure to run is
  `{"ok": false, "error": "..."}` - still one document.
- An `--out <file>` artifact is written **before** the report is printed. A
  destination that cannot be written is a refusal (exit 2): the error document
  is the only output and no partial file is left behind.
- `convert --out -` prints the artifact itself on stdout, so in JSON mode the
  artifact is the `html` / `text` field instead - never both.

### Exit codes

- `0` success.
- `1` the command ran and the result is negative: blocking violations,
  violations left after `fix`, warnings under `--strict`, failed image uploads
  or fits, `doctor` problems, a draft stopped by a gate.
- `2` the command could not run: clap usage error, unreadable input,
  unwritable output, unknown `--platform` id, unknown theme, missing
  credentials, an unconfirmed destructive operation, a WeChat API failure.

## JSON result shapes

`convert --platform wechat` (Markdown entry):

```json
{
  "ok": true, "platform": "wechat", "platform_known": true, "platform_source": "default",
  "input_format": "markdown", "export_kind": "rich_text_dialect",
  "out": "article.html", "theme": "minimal", "image_mode": "inline",
  "html": "<section ...>",
  "stats": { "chars": 0, "words": 0, "headings": 0, "images": 0, "code_blocks": 0, "tables": 0 },
  "images": [ { "source": "img/a.png", "final_src": "data:image/png;base64,...",
                "data_w": 1024, "data_ratio": "0.5625",
                "media_id": null, "inlined": true, "mmbiz": false } ],
  "paste_hostile_images": ["img/a.png"],
  "fixes":      [ { "rule_id": "R-1.4", "count": 2, "detail": "fixed style on <section>" } ],
  "violations": [ { "rule_id": "R-3.1", "severity": "block", "message": "...", "node": "<p ...>", "fixable": true } ],
  "blocking_count": 0
}
```

- `warning` is omitted per image unless something went wrong.
- Direct HTML input (`--input-format html`, frontmatter `format: html`, an
  `.html`/`.htm` file, or stdin starting with `<`) reports `fixes` and
  `violations` but no `stats`/`images` - the template renderer is skipped.
- Caption and Markdown platforms return `text` plus `title` and `export_kind`
  (`caption` / `markdown`); no `html`, no image pipeline, no theme.
- `copy` / `copy --dry-run` / `draft create` share the `report` payload
  (`stats`, `images`, `fixes`, `violations`, `blocking_count`) plus their own
  `copied` / `dry_run` / `draft_media_id` fields.
- `validate` adds the platform `caption` (rendered) for caption platforms, and
  `doctor` returns `checks[]` of `{check, status, detail}`.

Rule IDs are stable English constants (`R-1.1` ... `R-4.4`, `HYGIENE`, `I-03`,
`MD-1`/`MD-2`, plus platform title/caption ids like `XHS-2`, `X-1`, `WX-T1`).
Messages are never localized; `wxwright://spec/rules` carries the full bilingual table.

## Platform artifacts

Seven ids (`wxwright platforms` is the live registry): `wechat`, `xhs`, `zhihu`,
`meta`, `instagram`, `x`, `linkedin`.

- `wechat` - WeChat MP dialect rich text (`html`), styled so an MP-editor paste has zero distortion.
- `xhs`, `meta`, `instagram`, `x`, `linkedin` - plain-text caption (`text`).
  Their own limits are enforced as blocking rules: Xiaohongshu caps a note at
  1000 chars (XHS-2), X weights CJK as 2 units against a 280 budget (X-1).
- `zhihu` - the Markdown body (`text`) with a title-length warning (ZH-T1).
- Titles: WeChat 64 chars blocks (WX-T1).
- The image pipeline only runs for the WeChat entry; `image fit` is how any
  platform's image shape is produced (`wxwright image fit article.md
  --platform xhs --out-dir out/`). Remote/inline images are reported as
  `skipped`, not as failures - only a local file that fails to fit or write
  makes the command exit 1.

Platform selection order: `--platform` (explicit; an unknown id is exit 2),
then frontmatter `platform:` (an unknown value warns on stderr and falls back
to `wechat`), then the `wechat` default. `platform_source` in the JSON says
which branch was taken.

## MCP server

Start: `wxwright mcp serve` (stdio, newline-delimited JSON-RPC 2.0) or install
into a client with `wxwright mcp install --target claude|cursor|vscode|opencode`
(backs up the previous config to `<file>.json.bak`, preserves foreign servers,
and aborts instead of overwriting an unparseable file).

- Tools (12): `wxwright_convert`, `wxwright_validate`, `wxwright_fix`,
  `wxwright_copy`, `wxwright_themes_list`, `wxwright_upload_images`,
  `wxwright_draft_create`, `wxwright_draft_update`, `wxwright_image_fit`,
  `wxwright_draft_list`, `wxwright_draft_delete`, `wxwright_export`
- Resources: `wxwright://themes`, `wxwright://spec/rules`
- Prompts: `wxwright-publish-guide`
- Tool results carry both `content[0].text` (JSON string) and
  `structuredContent` (parsed object).
- Contract: every MCP `markdown` argument takes the article's **text content**.
  Read the file first and pass its text; file paths are rejected. The CLI is the
  surface that accepts paths.
- The tool surface is pinned by a drift test against the agent card, so adding
  a tool without updating the card and this file fails CI.

## GUI-only capabilities (note for agents)

- **Poster rasterization (HTML -> PNG)** runs inside the desktop GUI (SVG
  foreignObject); the CLI cannot rasterize. Agents should emit a
  self-contained poster HTML (fixed body size, inline CSS, no external
  resources) and hand it to the human, or use the GUI Poster Studio
  directly. All standard MP sizes are preset: 1080x460 (cover 2.35:1),
  1080x1080, 500x500, 1280x720, 1080x1440, custom.
- **ComfyUI text-to-image / image-to-image** is a GUI feature: wxwright
  auto-detects a locally running ComfyUI (127.0.0.1:8188 by default; `doctor`
  reports the probe) and saves outputs into the managed asset library. Agents
  cannot reach it via MCP yet; ask the human to click the wand button.
- **AI theme generation**, **AI illustration** and the **AI assistant** use
  whatever OpenAI-compatible provider the user configured in GUI settings. The
  CLI never calls an LLM - `doctor` reports a missing provider as a warning,
  not a failure.

## Markdown extensions understood by the engine

| Syntax | Effect |
|---|---|
| `> [!NOTE]` / `[!TIP]` / `[!IMPORTANT]` / `[!WARNING]` / `[!CAUTION]` | callout cards (GitHub alerts) |
| `> [!COMMENT]` / `> [!KEYPOINT] <text>` | comment card / key-point card |
| `[TOC]` paragraph | directory card built from h1-h3 |
| heading `{.center}` (e.g. `## Title {.center}`) | centered heading |
| image line + italic line right after | figure with caption |
| `$$...$$` / `$...$` | formula cards (styled text in v1) |
| ` ```html ` fence | rich-block channel: markup that passes the tag allowlist goes into the article verbatim (then normalized like everything else); anything else degrades to a plain code block. Allowed: section, div, p, span, strong, em, b, i, u, s, br, hr, table, thead, tbody, tfoot, tr, td, th, ul, ol, li, blockquote, h1-h6, figure, figcaption, center - no `img`, no `a`, no event handlers, no executable URL schemes. Inline `style` attributes are honoured. |
| ` ```chart ` fence / bare chart JSON | chart cards; the engine isolates glued or bare JSON because models reliably forget the fence |

Everything else is standard GFM: tables, task lists, fenced code (syntect
server-side highlighting, inline colored spans), strikethrough, autolinks
(footnote-style by default: link text + numbered reference list, survives
unverified accounts; set `link_style = "inline"` in a theme to keep `<a>`).

Frontmatter is honoured on every Markdown entry: `title`, `theme`, `author`,
`digest`, `platform`, `format: html`. CRLF line endings are normalised before
parsing, so a Windows-authored file is not silently treated as body text.

## Red lines for agents

- Never run `publish` (or the GUI's 群发) without the human's explicit approval.
- Never pass a secret on argv when `--secret-stdin` exists; never echo one into
  a log, a chat, or a document. Only masked forms ever appear in output.
- Never hand-write HTML styling into the source markdown - the validator blocks
  `font-family` (R-3.1), fixed px widths on containers (R-1.4) and `span[leaf]`
  nesting block elements (R-2.2). Use a theme or the `html` fence.
- Never ship while `blocking_count` > 0.
