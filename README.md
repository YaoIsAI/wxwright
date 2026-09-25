# wxwright

Let any AI agent (or human) turn Markdown into WeChat Official Account (微信公众平台) articles with zero style distortion — one command, one MCP call, one paste.

Rust · CLI · MCP · Desktop GUI · Single binary

> 简体中文：见 [README.zh-CN.md](README.zh-CN.md)

## 30-second start

```bash
# 1. convert (no credentials needed)
wxwright convert article.md --theme minimal --out article.html

# 2. paste-safe full chain: convert -> images -> normalize -> validate -> clipboard
wxwright copy article.md
# then open the MP editor and press Ctrl+V
```

## For AI Agents

Paste the whole card from `wxwright agent-card --md` into any agent's system prompt and it can operate wxwright immediately. Machine-readable contract: `wxwright agent-card --json`.

MCP one-line integration:

```bash
wxwright mcp install --target claude   # or cursor | vscode | opencode
wxwright mcp serve                     # stdio MCP server
```

MCP tools: `wxwright_convert`, `wxwright_validate`, `wxwright_copy`, `wxwright_themes_list`, `wxwright_upload_images`, `wxwright_draft_create`, `wxwright_draft_list`.
Resources: `wxwright://themes`, `wxwright://spec/rules` (the full rule table, bilingual).
Prompts: `wxwright-publish-guide`.

Exit codes: `0` pass · `1` rule violations · `2` runtime error. All commands accept `--json` (stable schema; non-TTY defaults to JSON) and `-` for stdin.

## Why wxwright is different

The WeChat MP editor is a hardened ProseMirror variant: external stylesheets are dropped, `class`/`id` stop working, and "magic styles" (opacity-hidden images, fixed pixel widths, `height:0` collapsing) look fine in the editor but break after publishing or on mobile. Most converters optimize for the editor view; wxwright optimizes for all four views: editor, published desktop, mobile, Dark Mode.

1. **Engine, not scripts** — one pure-Rust core (`wxwright-core`) shared by CLI, MCP and GUI. Mobile FFI reuses the same brain later.
2. **Generate-then-verify** — themes render directly into the official dialect (`section` blocks + `span[leaf]` inline runs, all styles inline); the normalizer and validator are the second line of defense, not the only one.
3. **Full rule engine** — every rule of the official editor spec (PRD §5.3: R-1.1 … R-4.4) has generation strategy + auto-fix + detection, each backed by a violation-sample test. The built-in validator aligns with `verify-article-structure-spec` (official puppeteer CLI) which serves as the CI truth gate.
4. **Extreme local performance** — single static binary, zero daemon, zero IPC. Release SLO: 10k-char full chain ≤ 60 ms p50, ≤ 8 MB CLI binary (see `docs/performance.md`).

## Clipboard copy, the four zero-distortion conditions

`wxwright copy` succeeds only when all four hold (PRD §3.5):

1. Payload: self-contained `text/html`, all CSS inline, no `<style>`/`class`/`script`/external fonts.
2. Structure: the HTML already lives inside the official whitelist.
3. Images: mmbiz URLs (or explicitly accepted https with warnings); local/base64 images are **blocked** with a clear list — they would break on paste.
4. Verification: built-in validator + official CLI bridge in CI.

With credentials (`wxwright login`), local images upload to the permanent material library and are rewritten to mmbiz automatically; `wxwright draft create` then publishes straight to the 草稿箱 (Drafts).

## Themes

Three built-in themes (`minimal` 素黑 / `techblue` 科技蓝 / `magazine` 杂志). A theme is data: TOML color tokens + optional per-role style overrides, compiled to inline styles at render time. `wxwright theme new my-theme` scaffolds one; `wxwright theme validate` proves its output compliant before it ships. `font-family` is forbidden engine-wide (official rule R-3.1) — themes cannot smuggle it in.

Components (Markdown-native triggers): GitHub alerts `> [!NOTE]` / `[!TIP]` / `[!IMPORTANT]` / `[!WARNING]` / `[!CAUTION]` become callout cards; `> [!COMMENT]` a comment card; `> [!KEYPOINT] text` a key-point card; `[TOC]` builds a directory card; an image followed by an italic line becomes a figure with caption; headings accept `{.center}`.

## CLI reference

```
wxwright convert  <input.md|->   --theme <name|path>  --out <file.html|->  [--json]
wxwright validate <input.md|html> [--json] [--strict] [--official-check]
wxwright fix      <input.html>   --out <fixed.html>
wxwright copy     <input.md>     --theme <name> [--dry-run]
wxwright image    upload <paths...>
wxwright draft    create|update|list          (needs credentials)
wxwright publish  <draft_id> --yes            (群发; explicit confirmation)
wxwright theme    list|new|validate
wxwright doctor
wxwright mcp      serve | install --target <claude|cursor|vscode|opencode>
wxwright agent-card [--md|--json]
wxwright login --appid <id> --secret <key>    # OS keychain (PRD 5.6)
wxwright logout
wxwright bench
```

## Desktop GUI

`wxwright-gui` (Tauri 2):

- **Three-zone layout**: article library (left) / editor (center) / phone-frame preview (right), Dark Mode simulation, one-click rich copy, HTML export, `.md` drag-and-drop.
- **Article library**: local Markdown store (`Documents/wxwright/articles`) with frontmatter metadata; auto-save on switch, click to reopen, delete, new article.
- **AI assistant**: Codex-style drawer under the editor - streaming chat with any OpenAI-compatible provider (OpenAI / DeepSeek / Qwen / Kimi / GLM, or local Ollama / LM Studio). API keys live in the OS keychain. Quick prompts: polish / continue / titles / outline; every reply offers insert / replace / copy.
- **Agent integration panel** (robot icon): one-click MCP install into Claude Desktop / Cursor / VS Code / OpenCode, one-click copy of the agent card, CLI cheat sheet. Same engine, same compliance.
- **AI theme generator**: describe a style, get a brand-new theme validated against the official rules and saved for CLI + GUI.
- **Poster Studio**: HTML -> PNG locally (SVG foreignObject, offline) with all standard MP cover sizes; AI can generate the poster HTML; export-and-insert into the article.
- **Realistic device frames**: iPhone 15 Pro (Dynamic Island, physical side keys, home indicator) and Pixel 8 (punch-hole) rendered as true hardware with an under-device switcher; dark/light toggle sits next to it. The preview scales to fit while keeping the real logical resolution.
- **Asset library**: posters, AI paintings, pasted screenshots and dropped images all land in one managed grid (`Documents/wxwright/assets`) - thumbnail browse, one-click insert into the article, delete.
- **ComfyUI integration (local AI painting)**: wxwright auto-detects a locally running ComfyUI (default `127.0.0.1:8188`, configurable in Settings). When present, the wand button drives its API for text-to-image and image-to-image (SD workflows, queue + history polling), and outputs flow straight into the asset library and the article. No cloud, no keys.
- **Compliance badge** lives in the stats row (colored block/warn counts); the violations list is a static panel that never overlaps the AI drawer.
- **Writing pet**: 墨仔 (Mozai) - an actual sitting cat with whiskers, blinking eyes and a curled tail. Bounces while you type, sleeps after 45s, hearts on click, party easter egg on triple-clicking the logo. Clicking 码聋 in the status bar pops the WeChat QR code.

The GUI binary also answers `wxwright-gui.exe mcp serve`, so a standalone GUI install works as the MCP server too.

## Architecture

```
            ┌────────────────────────────────────────────┐
 humans ─GUI─►  hosts                                     │
 agents ─CLI─►  wxwright-gui(Tauri2)  wxwright-cli  mcp    │
 agents ─MCP─► ├────────────────────────────────────────────┤
               │  wxwright-core (pure Rust, no IO assumptions)│
               │  parser · ir · theme · render · normalizer  │
               │  validator · clipboard · imgpipeline        │
               ├────────────────────────────────────────────┤
               │  adapters: wxwright-mp (API), clipboard, img│
               └────────────────────────────────────────────┘
```

Single process, single binary, zero resident services. Boundaries are compile-time (traits + versioned DTOs); the only out-of-process exception is the optional official puppeteer check in CI.

## Security

- AppSecret lives in the OS keychain (Windows Credential Manager / macOS Keychain / libsecret); the config file stores only a reference. `WXWRIGHT_MP_APPID` / `WXWRIGHT_MP_SECRET` env vars override for CI.
- Access tokens are always masked in output.
- `publish` (群发) requires explicit `--yes`; the default write path is the draft box.
- Zero telemetry, zero cloud dependencies.

## Repository map

- `crates/wxwright-core` — engine (parser, IR, dialect renderer, themes, normalizer, validator, clipboard, image pipeline, i18n, agent card)
- `crates/wxwright-cli` — CLI (also `wxwright mcp serve`)
- `crates/wxwright-mcp` — stdio MCP server (tools/resources/prompts)
- `crates/wxwright-mp` — WeChat MP API adapter (token, material upload, drafts, freepublish)
- `gui/` — Tauri 2 desktop app + vanilla-JS frontend (`ui/`)
- `tools/icongen` — icon master → platform icons pipeline
- `docs/` — agent integration, performance SLOs, manual test matrix
- `examples/sample-article.md` — a feature-complete sample article

## Known limitations (v1)

- Formula rendering keeps LaTeX as styled text cards (image-mode requires a KaTeX bridge, planned).
- `R-2.1` deep identical-wrapper chains are detected but not auto-unwrapped.
- Official CLI bridge (`--official-check`) prints guidance and CI instructions; the actual puppeteer run belongs to CI.
- Rich clipboard HTML flavor is attempted on Windows/macOS/Linux alike; it
  degrades to plain text only where the clipboard cannot accept HTML (headless).
- clap built-in help text is English; reports are localized (en / zh-CN).

## Author

**AI Yao** (AI瑶) - WeChat Official Account: **码聋 (Code-Deaf)** · [github.com/YaoIsAI](https://github.com/YaoIsAI)

Say hi to 墨仔 (Mozai), the ink cat living in the GUI status bar - and click 码聋 in the status bar to get the WeChat QR code.

## Release automation

Tag a version (`git tag v1.0.1 && git push origin v1.0.1`) and GitHub Actions builds everything automatically: Windows NSIS installer + portable exe, macOS dmg (universal) + CLI binaries (arm64/x64), Linux deb + AppImage + static musl CLI, plus SHA256SUMS - all attached to the Release. See `docs/release-automation.md` for signing/notarization setup.

## License

MIT OR Apache-2.0
