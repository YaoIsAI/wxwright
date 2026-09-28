# wxwright agent integration handbook

Machine-friendly reference for AI agents. The snapshot contract in
`wxwright agent-card --json` is generated from the same source of truth as
this document; if they drift, file a bug.

## Golden path (clipboard flow, zero credentials)

```
1. agent writes article.md (standard GFM; use blockquote alert syntax for cards)
2. wxwright validate article.md --strict      -> exit 0 required
3. wxwright copy article.md --json            -> read the JSON result
4. tell the human: open the MP editor, Ctrl+V
```

If step 3 returns `"copied": false`, the JSON tells you why:

- `blocking violations` — fix the markdown source, go to step 2. Never copy a
  non-compliant payload; the engine will not do it either.
- `paste-hostile images` — local/base64 images break on paste. Either enable
  credentials (below) or drop local images.

## Golden path (unattended draft flow, needs credentials)

```
0. wxwright login --appid <APPID> --secret <SECRET>   # stored in OS keychain
1. wxwright draft create --file article.md --title "..." --json
   -> images upload to the permanent material library (mmbiz rewrite),
      first uploaded image becomes the cover thumb
2. report draft_media_id; the human reviews in 公众号 -> 草稿箱 (Drafts)
```

`wxwright publish <draft_id> --yes` performs 群发 (mass send). Never run it
without the human's explicit instruction.

## Command surface

| Command | Key args | Exit codes |
|---|---|---|
| `convert <input\|->` | `--theme <name\|path>`, `--out <file\|->`, `--json` | 0 / 2 |
| `validate <input\|->` | `--strict`, `--official-check`, `--json` | 0 / 1 / 2 |
| `fix <input.html>` | `--out <file>`, `--json` | 0 / 1 / 2 |
| `copy <input\|->` | `--theme`, `--dry-run`, `--json` | 0 / 1 / 2 |
| `image upload <paths...>` | `--json` | 0 / 1 / 2 |
| `draft create\|update\|list` | `--file --title --author --digest --theme --thumb-media-id` | 0 / 1 / 2 |
| `publish <draft_id>` | `--yes` | 0 / 2 |
| `theme list\|new\|validate` | | 0 / 1 / 2 |
| `doctor` | `--json` | 0 |
| `mcp serve` / `mcp install --target <t>` | | |
| `agent-card` | `--md`, `--json` | 0 |
| `login` / `logout` | `--appid --secret [--no-keyring]` | 0 / 2 |
| `bench` | `--iters <n>` | 0 |

Global flags (accepted anywhere): `--json`, `--lang <en|zh-CN>`, `--no-color`,
`--verbose`. Non-TTY stdout defaults to JSON.

## JSON result shapes

`convert` / `copy` / `draft create` share the report payload:

```json
{
  "stats":  { "chars": 0, "words": 0, "headings": 0, "images": 0, "code_blocks": 0, "tables": 0 },
  "images": [ { "source": "...", "final_src": "...", "data_w": 1024, "data_ratio": "0.5625",
                "media_id": null, "warning": null, "inlined": false, "mmbiz": true } ],
  "fixes":     [ { "rule_id": "R-1.4", "count": 2, "detail": "fixed style on <section>" } ],
  "violations":[ { "rule_id": "R-3.1", "severity": "block", "message": "...", "node": "<p ...>", "fixable": true } ],
  "blocking_count": 0
}
```

Rule IDs are stable English constants (`R-1.1` … `R-4.4`, `HYGIENE`). The
`wxwright://spec/rules` MCP resource carries the full bilingual table.

## MCP server

Start: `wxwright mcp serve` (stdio, newline-delimited JSON-RPC 2.0) or install
into a client with `wxwright mcp install --target claude|cursor|vscode|opencode`.

- Tools: `wxwright_convert`, `wxwright_validate`, `wxwright_copy`,
  `wxwright_themes_list`, `wxwright_draft_create`, `wxwright_draft_list`,
  `wxwright_export`
- Resources: `wxwright://themes`, `wxwright://spec/rules`
- Prompts: `wxwright-publish-guide`
- Tool results carry both `content[0].text` (JSON string) and
  `structuredContent` (parsed object).

## GUI-only capabilities (note for agents)

- **Poster rasterization (HTML -> PNG)** runs inside the desktop GUI (SVG
  foreignObject); the CLI cannot rasterize. Agents should emit a
  self-contained poster HTML (fixed body size, inline CSS, no external
  resources) and hand it to the human, or use the GUI Poster Studio
  directly. All standard MP sizes are preset: 1080x460 (cover 2.35:1),
  1080x1080, 500x500, 1280x720, 1080x1440, custom.
- **ComfyUI text-to-image / image-to-image** is a GUI feature: wxwright
  auto-detects a locally running ComfyUI (127.0.0.1:8188 by default) and
  saves outputs into the managed asset library. Agents cannot reach it via
  MCP yet; ask the human to click the wand button.
- **AI theme generation** and the **AI assistant** use whatever
  OpenAI-compatible provider the user configured in GUI settings.

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

Everything else is standard GFM: tables, task lists, fenced code (syntect
server-side highlighting, inline colored spans), strikethrough, autolinks
(footnote-style by default: link text + numbered reference list, survives
unverified accounts; set `link_style = "inline"` in a theme to keep `<a>`).
