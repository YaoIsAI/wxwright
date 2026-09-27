# Changelog

All notable changes to wxwright are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/).

This project had no git history when the changelog was normalized (2026-09-25),
so earlier entries describe the feature set as it stood rather than an
invent­ed commit timeline. `docs/` is the source of truth for the current spec.

## [Unreleased]

> The entries below the second heading come from a multi-agent review round
> (four independent reviewers: correctness, engineering governance,
> architecture, plus a dedicated verifier whose job was to falsify the other
> three). Three blocking defects were confirmed by hand and by black-box
> reproduction; every fix ships with a test that fails without it.

### Fixed
- **The chat "stop" button never worked.** `chat()` took the *previous* value
  of `GEN_SEQ.fetch_add(1)` as its generation while `stop()` stored the
  post-increment value, so the streaming loop's `STOP_GEN == my_gen` test was
  unsatisfiable - clicking stop never broke a stream. Worse, the stale
  `STOP_GEN` left behind then matched the *next* chat's generation and killed
  it on its first chunk. A commit titled "real chat stop" had claimed this
  earlier but only changed the disconnect path, not the comparison. The
  generation arithmetic now lives in `next_generation()` /
  `mark_stop_for_current()` (free functions over `&AtomicU64`) so it is
  unit-testable; two tests cover both directions and both fail if the `+ 1`
  is removed.
- **Malformed hex colors panicked the validator.** `css_color_rgb` sliced
  `#RRGGBB` by byte index, so a value of exactly six bytes containing a
  multi-byte character (`#中ab` = 1 + 3 + 1 + 1) split inside that character:
  `wxwright validate` died with exit code 101. A hex color is ASCII by
  definition, so a non-ASCII value is now rejected up front.
- **The 500-entry violation cap could hide a blocking finding.** `add()`
  dropped everything past `MAX_VIOLATIONS` regardless of severity, so 500
  warnings followed by a `<script>` produced `compliant: true` and exit 0 -
  a non-compliant article reported as shippable. Warns are still capped at
  500; blocking findings now have their own ceiling (4000) and can never be
  truncated away. Affected `wxwright validate`, the MCP `wxwright_validate`
  tool and the GUI badge alike.
- **Two more image gates were missing.** The MCP `wxwright_copy` handler
  hand-rolled `inlined && !mmbiz` instead of the shared predicate (so
  plain-http URLs and failed loads slipped through while the CLI blocked
  them), and `wxwright_draft_create` had **no gate at all** - an agent could
  push a draft with base64 images and blocking violations. Both now use
  `ImageOutcome::paste_hostile()`, and the new `tests/image_gate_test.rs`
  scans the sources so a hand-rolled filter cannot reappear.
- **`[block.h6]` was rejected by the theme validator.** `render_heading`
  builds `format!("h{}", level)` for every level the parser can emit (1..=6),
  but `roles.rs` only declared h1-h5, so `validate_generated_theme` refused a
  key the renderer honoured. h6 is declared now, and the contract test grew
  the reverse direction it was missing: it drives every heading level
  end-to-end and asserts both that the override lands *and* that the key is
  declared. `toc_title` was renamed `toc_heading` because it looked
  indistinguishable from a `toc` + `_title` suffix expansion.
- **A stop that landed before the first chunk looked like a transport
  error.** `chat()` treats "stream ended with no `data:` line" as "the
  provider ignored `stream: true`, try to parse a whole JSON body". A stop
  click that arrived before the first token therefore fell into that branch,
  failed to parse an empty body and surfaced `响应不是 SSE 流也不是 JSON: ` -
  a user who stopped a slow generation saw a confusing protocol error instead
  of a clean stop. The fallback now runs only when the run was not stopped.
  Found by driving the real provider end to end: the stop worked (the loop
  broke on its first iteration) but the reporting did not.
- **Every foreign HTTP call was dead on a machine behind a proxy.** `ureq`
  reads neither the environment nor the Windows system proxy (its
  `proxy-from-env` feature is opt-in, has no `NO_PROXY` support, and prefers
  `ALL_PROXY` over the scheme-specific `HTTPS_PROXY`), so with Clash/Mihomo on
  127.0.0.1:7897 the AI assistant, cloud image generation and the X/LinkedIn
  token exchange all failed with `os error 10060` while `curl` worked fine.
  The foreign endpoints now build their agents through `net::with_env_proxy`
  (`util::proxy_url_from_env` honours `HTTPS_PROXY`/`ALL_PROXY`/`HTTP_PROXY`
  in that order, plus a `NO_PROXY=*` opt-out). ComfyUI and the WeChat MP API
  stay direct on purpose: loopback through a proxy is pointless, and routing a
  domestic API through a foreign tunnel would make a working path worse.
- **`redact_secrets` missed two shapes.** The parameter-boundary set had no
  `;` separator and key matching was case-sensitive, so `?a=1;secret=LEAK`
  and `?SECRET=LEAK` passed through untouched. Both are covered now, with a
  guard test that unrelated names (`errcode`, `mysecret`, `secret_sauce`)
  are still left alone.

### Added
- **Automated guards for the two iron laws that had none.** The review found
  that iron law 2 (a `#[tauri::command]` must be registered in `lib.rs`) and
  iron law 4 (every UI string needs both a zh-CN and an en entry) were enforced
  only by discipline. Both are now locked by `tests/gui_governance_test.rs`:
  - it diffs the functions carrying `#[tauri::command]` against the
    `generate_handler!` list, in both directions, so an unregistered command
    (pitfall 4: `extract_document_text` shipped dead) or a stale registration
    fails the build;
  - it compares the key sets of the two `I18N` dictionaries and checks that
    every `data-i18n*` attribute in `index.html` resolves in both. The parser
    strips JS string and template literals before scanning for `ident:`, so a
    URL or a `${...}` interpolation inside a value is never mistaken for a key.
  Both guards were proven by breaking the code on purpose (removing a
  registration and deleting an `en` key) and confirming they go red.
- **CI: JS syntax gate and `--locked`.** The UI is ~4k lines of hand-written
  vanilla JS with no bundler, so `node --check` now runs over every
  `gui/ui/*.js` on all three platforms. Cargo invocations in CI pass
  `--locked` so a stale `Cargo.lock` fails loudly instead of silently
  resolving new versions.

### Changed
- **Documentation caught up with the code** (the review's biggest single
  finding: 10 of 12 spot-checked claims did not match reality).
  `AGENTS.md` corrected the app.js line count (~2900 -> ~4100), a
  non-existent `crates/wxwright-gui/` path, the MCP tool count (7 -> 8), the
  platform count (6 -> 7, Instagram was missing) and the test counts;
  iron law 12 now says five write paths, not three. `CHANGELOG.md` corrected
  the role/key arithmetic (26 roles, 54 keys - verified by a new test rather
  than by hand).

- **The article sidebar never noticed externally created articles.** `refreshLibrary`
  ran only after in-app actions, so an article written by the MCP server or the
  CLI showed up in the GUI's library list only after a restart (and re-opening
  the window). The sidebar now polls `list_articles` every 4 seconds and
  re-renders only when the id/updated signature changes, plus an immediate
  check on window focus. The refresh touches the list only - the open editor
  keeps its content, so an external rewrite can never clobber unsaved edits.
- **Callout card bodies rendered outside the card.** `render_card` closed the
  card container right after the title strip and appended the body blocks as
  sibling sections, so on every theme the `[!WARNING]` / `[!IMPORTANT]` /
  `[!TIP]` / `[!NOTE]` backgrounds covered only the label line while the body
  sat on the plain page background - cards looked like thin colored strips.
  Body blocks now render inside the container section, on the card background.
- **AppSecret could reach logs, JSON output and toasts.** `wxwright-mp` builds
  its request URLs with the AppSecret / access_token in the query string, and
  `ureq`'s status-error `Display` renders as `"{full_url}: status code {code}"`.
  Any 4xx/5xx (proxy, WAF, CDN) therefore printed the credential in clear text.
  All engine error paths now run through `util::redact_secrets`, a hand-written
  scanner (no regex) that masks the value of `secret`, `appsecret`,
  `access_token`, `client_secret`, `refresh_token`, `session_key`, `code`,
  `code_verifier` and `ticket` - only at parameter boundaries, so `errcode=`
  is never mistaken for `code=`. Four unit tests cover the ureq-shaped string,
  the OAuth case, the `errcode` trap and multibyte input.
- **AI-authored card styling was silently discarded.** The theme prompt
  advertised `card_note` / `card_tip` / `card_keypoint` and friends, but
  `render_card` computed the role and then threw it away (`let _ = role;`), and
  `base_leaf` hardcoded `paragraph_leaf`. A theme that only styled cards
  produced byte-identical output and nothing failed. `render_card`,
  `base_leaf`, `render_paragraph`, `render_quote`, `render_code`,
  `render_table`, `render_list_item`, `render_standalone_image`, `render_toc`,
  `render_rule` and `render_formula` now all consult `theme.blocks` through
  `css()` / `css_owned()`. The `paragraph` role was ignored the same way and is
  fixed too.
- **The role contract can no longer drift.** New `wxwright_core::roles`
  module holds the canonical table (26 roles, 54 expanded keys); the renderer,
  the AI theme prompt (`theme_schema()` builds the advertised list from it) and
  `validate_generated_theme` (which now rejects any `[block.*]` key the
  renderer would never read, instead of accepting a theme that silently
  no-ops) all read the same source. `tests/theme_roles_test.rs` gives every key
  a distinctive declaration and asserts it reaches the rendered HTML.
- **The GUI draft push had no I-03 gate.** `wx_push_draft` checked blocking
  violations only, so a failed image upload silently degraded to a base64 data
  URI and the draft shipped with images the MP editor renders as broken. It now
  shares `ImageOutcome::paste_hostile()` with the CLI copy path. The predicate
  also covers plain-`http` hosts and images that failed to load, both of which
  previously slipped past the CLI filter too.
- **`wxwright doctor` always exited 0**, so it could not gate a CI or SCP run.
  It now returns 1 when any check fails, gains `--strict` (warnings fail too),
  and reports two new checks: a writable state directory (`err` when read-only)
  and whether the desktop app has an AI provider configured (`warn`).
- **A fast AI job could hang the trigger button forever.** The backend inserts
  the job, spawns it and only then returns the id, so a job that failed
  immediately emitted `done`/`error` before `ai-jobs.js` had registered a
  handler; the event was dropped and the promise never settled.
  `ai-jobs.js` now buffers events that arrive before their handler exists and
  replays them on registration.
- **A panicking job leaked its registry entry.** `jobs.rs` removed the job as
  the last statement of the worker closure, so an unwind skipped it and
  `ai_job_stop` kept answering `true` for a job that was gone. A `JobGuard`
  with `Drop` now does the cleanup, the worker body is wrapped in
  `catch_unwind`, and a panic emits a terminal `error` event so the UI
  recovers.
- **Token expiry only retried on one of three call paths.**
  `upload_material_once` and `post_with_token` (draft add/update) never
  refreshed, so a long session died half way through. All authenticated calls
  now funnel through `with_token_retry`. Upload backoff is exponential
  (500ms/1s/2s/4s) instead of a flat `500ms * attempt`.
- **Cache-bust versions had drifted** (`app.js`, `pet.js`, `ai-jobs.js` were
  all edited after their `?v=` was set, so a WebView could serve the old
  script). Bumped to app v25 / pet v20 / ai-jobs v20.
- **Fallback credential files were world-readable on Unix.** `config.toml`
  (which holds the AppSecret in clear text under `--no-keyring`),
  `settings.json` and `social-bindings.json` are now written through
  `util::write_private`, which applies 0600. Windows keeps the per-user
  %APPDATA% ACL; the difference is documented in the function.
- **7 dead Tauri commands removed** (`ai_complete`, `ai_generate_svg`,
  `ai_generate_theme`, `ai_image`, `comfy_txt2img`, `comfy_img2img`,
  `validate_md`). The frontend never invoked any of them after the unified
  `ai_job_start` runtime landed, leaving two generation runtimes side by side
  with different budget ladders. `ai::complete` / `chat_once` /
  `generate_theme` are now `#[cfg(test)]`-only (the opt-in live smoke tests
  still drive them); `generate_svg_component` was genuinely dead and is gone.
- **`platform` id dispatch moved into the descriptor.** `export_kind(id)`
  matched on strings while the registry claimed "descriptors are data"; the
  kind is now a `PlatformSpec` field. `resolve_platform(id)` reports whether an
  id was recognised, so a typo in `frontmatter` or `--platform` warns instead
  of silently rendering WeChat dialect.
- **The state directory had three definitions** (`theme::user_themes_dir`,
  `mp::config_path`, `ai::settings_path`, plus three copies in `comfy.rs`).
  They all read `wxwright_core::util::config_root()` now, which is what
  `doctor` probes.
- **Caption exports emitted stray blank lines** - the collapser only reduced
  runs of four newlines to three. Captions now collapse to a single paragraph
  break.

### Added
- **Push-draft cover picker**: articles without an inline image now ask for
  a local cover image (file dialog) when pushing to the MP drafts box; the
  picked file uploads to the material library as the cover thumb. Backend
  accepts `coverImagePath`; on the NO_COVER failure the UI now asks for a
  cover file and retries once, and the error surfaces the per-image upload
  warnings (the classic cause: the MP IP whitelist, errcode 40164).
- **GUI draft-box push**: the editor status row gains a Push-draft button
  (WeChat platform only) that runs the same chain as `wxwright draft create`:
  dialect render with the article theme, local images uploaded to mmbiz,
  blocking-violation gate, then the MP drafts API. Closes the
  "promise-without-button" gap found in the release review.
- **`wxwright convert --platform <id>`** and the MCP **`wxwright_export`** tool:
  one Markdown source now reaches every registered platform from the CLI and
  from an agent, not just from the desktop app. WeChat returns dialect rich
  text, Xiaohongshu / Facebook / Instagram / X / LinkedIn return the plain-text
  caption they actually consume, Zhihu returns Markdown unchanged.
- **AI replies gained a "Copy rich text" action**: the reply can go straight
  into the 公众号 editor without first replacing the article body. The copy
  chain is shared with the topbar button (`copyRichMarkdown`), so the blocking
  and paste-hostile reporting is identical.
- **Draft metadata**: `wx_push_draft` now reads `author` and
  `digest`/`summary`/`description` from the article frontmatter (digest capped
  at the WeChat 120-character limit) instead of sending empty strings.

### Changed
- The agent card no longer hands agents `wxwright publish <draft_id> --yes`.
  群发 is irreversible and reaches real subscribers; the card now states
  explicitly that a human must trigger it from the desktop app.
- `wxwright-mp` no longer depends on `dirs` directly (the CLI dropped it too).

### Docs
- README / README.zh-CN / docs/agent-integration.md list the 8th MCP tool.

## [0.10.0] - 2026-09-26

### Added
- **AI theme generation hardening** (found by the live theme matrix test -
  three wildly different styles generated against the real provider):
  - `sanitize_model_toml` repairs the model-output failure classes before
    parsing: pseudo-class and nested-subtable headers (`[block.a:hover]`,
    `[block.task.completed]`) are dropped with their bodies, duplicate table
    headers are merged, duplicate keys inside a section collapse (last
    wins), and stray prose before the first table is removed.
  - generate_theme gets its own budget ladder (start 8192, cap 32768) -
    reasoning models burned the shared 16384 cap on thinking alone and
    never emitted a byte of TOML; the retry prompt now asks the model to
    skip the thinking expansion, and failures persist the last attempt to
    `themes/last-failed-theme.txt` (the path rides in the error message)
    so line numbers match the dump exactly.
  - THEME_SYSTEM prompt: contrast is now measured against the theme's own
    background (dark themes were effectively forbidden by the old "vs
    white" wording) and font-family demands are answered with safe
    approximations; THEME_SCHEMA enumerates the exact `[block.*]` role
    whitelist.
  - `live_theme_matrix_smoke` (opt-in) generates the three-style matrix on
    every run.
- **Default article is now a full user manual**: the built-in sample
  (first-run / load-sample) and a fresh library entry carry a complete
  bilingual-audience user manual covering writing and preview, the article
  library, the 7-platform channel trio, themes, the AI assistant, the four
  studios, BYO publish bindings, Agent/CLI/MCP access, key security and the
  FAQ. Passes the strict validator with zero violations.
- **Live article-generation smoke** (`live_article_generation_smoke`,
  opt-in like the theme smoke): drives the AI assistant's generation engine
  (sync complete() path with the budget ladder) against the real provider
  and asserts a structured article comes back.
- **Setup guide + BYO publish bindings**: a question-mark button in the topbar
  opens a bilingual setup center covering every integration (AI providers,
  WeChat MP API, ComfyUI, the X / LinkedIn / Facebook / Instagram publish
  bindings and Agent MCP) with step-by-step guides and official-portal links.
  Settings gains a Publish bindings pane: per-platform bring-your-own client
  credentials stored in the OS keyring (local-file fallback, never in any
  repo). One-click login is implemented end to end for X (OAuth 2.0 PKCE
  public client, fixed loopback callback port 8761) and LinkedIn (3-legged
  code flow, port 8762): the system browser opens, a local loopback listener
  captures the redirect, tokens are exchanged and stored - no cloud service
  involved anywhere. Facebook/Instagram keep the credential interface while
  their login flows stay gated (Meta review wall, Instagram public-URL media
  requirement); the research behind this lives in
  docs/social-publish-oauth-feasibility.md. Six new unit tests cover the
  PKCE pair, auth-URL contracts, callback parsing, the loopback listener end
  to end (including stray-request skipping), storage round-trip and
  token-leak safety of status payloads (102 tests total).

### Fixed
- **Theme canvas in file exports**: the HTML export wrapper hardcoded a
  white page background, so AI-generated dark themes (which declare
  `background = "#121212"` with light text) exported unreadable pages -
  caught while rendering the new user manual with the dark-code AI theme.
  The wrapper now paints the theme's canvas colour; built-in themes, which
  declare no background key, keep the neutral white page. Three new tests
  lock the behaviour (105 total).
- **UI audit fixes** (2026-09-26, full ledger in docs/button-audit-findings.md):
  15 issues from a button-by-button audit of all 16 UI zones. Highlights:
  - `t()` now returns the key name on a missing i18n key (iron law 4 was
    documented but never implemented); `saved_new_ok` (new-article save toast
    rendered a literal "undefined") and `f_logo` (EN mode kept a Chinese
    label) were missing from both dictionaries and are now dual-language.
  - Duplicate ids removed: the AI-drawing dialog launch-path input was dead
    (every read/write hit the settings-pane twin) - renamed to
    `imggen-launch-path` with two-way sync; the SVG-kit detail span
    (`svgkit-desc-view`) is now populated on component selection.
  - `demo-agent-card.md` regenerated from `agentcard.rs` (it listed 4 of the
    7 MCP tools); a drift test now locks the demo card to the core source.
  - Escape closes the topmost visible modal (the prompt modal goes through
    cancel so a pending promise resolves cleanly).
  - Demo fidelity: status bar no longer sticks at "Converting...", character/
    word/image stats compute locally, library search filters, the theme
    dropdown mirrors the 3 built-in themes, asset-stub buttons answer with a
    demo toast instead of silently doing nothing, poster/fit preset labels
    translate their size vocabulary to English.
- **Code-review sweep over all prior requests** (2026-09-25) found and fixed:
  - `addAttachment` was defined twice in app.js; the later md/txt-only
    definition shadowed the multi-format version, so PDF/DOCX/HTML/images
    attachments never actually worked despite the backend being complete.
    Deduped to the multi-format implementation (extract_document_text +
    vision image parts).
  - Drag-dropping documents (txt/pdf/docx/html/csv/json/xml) onto the window
    now routes them into AI attachments (previously only images and .md were
    handled); attachment button tooltip updated accordingly.
  - The wxwright-logo triple-click easter egg (Mozai party) had been lost in
    an earlier init rewrite; restored.
  - AI chat "stop" now really disconnects: the SSE read loop checks the stop
    flag between lines and drops the connection instead of only halting
    rendering after the full download.

### Added
- **Pixel-level per-platform preview shells**: the single shared "note" shell
  (accent-colour-only difference) became one faithful layout per platform,
  researched against each platform's real feed anatomy - Xiaohongshu note
  detail (full-bleed 3:4 cover, on-image action rail, fixed author/comment
  bottom bar), X post (40px avatar rail, handle header, 16:9 rounded media,
  reply/repost/like/views metric row, blue hashtags), Facebook card (#F0F2F5
  page, 8px white card, reaction cluster, three-column like/comment/share
  bar), Instagram post (script wordmark, gradient story-ring avatar, 4:5
  media, action row + likes + username-prefixed caption), LinkedIn card
  (#F4F2EE page, 48px avatar + headline + globe, reaction cluster,
  four-column like/comment/repost/send bar) and a Zhihu article page (blue
  follow pill, author row, justified 15px body, upvote action bar). Each
  shell renders its strings in the active UI language.
- **Instagram as its own platform**: "Meta (Facebook/Instagram)" split into
  Facebook (id `meta`, kept for stored articles) and Instagram (id
  `instagram`, image-note flow with 4:5 / 1:1 / 9:16 presets); both got
  official Simple Icons marks. Demo mode (http.server 8742) now carries the
  full platform set and renders channel shells too, so every platform shape
  is reviewable in a plain browser.
- **AGENTS.md**: a takeover guide for any AI agent (architecture map, commands,
  hard invariants, the full pitfalls ledger, verification protocol and a
  pre-delivery checklist). The pitfalls table records every historical
  incident (silent patch failures, duplicate definitions shadowing features,
  unregistered commands, stale dist, cache-bust misses, ...) so they are not
  repeated.
- **Platform switcher with official marks**: the native select became a
  custom dropdown showing each platform's official logo tile (WeChat green,
  XHS red, Zhihu, X, LinkedIn - Simple Icons CC0 plus the official LinkedIn
  icon SVG from Wikimedia Commons); the toolbar trigger carries the active
  platform's mark and colour (evaluated: tinted trigger beats a full
  colour-changing top bar on clash-risk and consistency).
- **Language toggle actually wired**: btn-lang had no click handler (the
  dictionary and applyI18n existed but were never reachable). It now flips
  zh-CN/en, persists, re-renders every dynamic surface and gained a
  data-i18n-title / data-desc-i18n mechanism; 73 hardcoded strings across
  toolbar tooltips, provider form, ComfyUI section, about, theme presets,
  poster/size/asset/svgkit panels and the QR modal got keys in both locales;
  a duplicate `comfy-start` id (dead second button) was fixed on the way.

### Fixed
- **Mozai form switching**: the cross-fade layering introduced earlier read
  as a harsh transition; removed entirely (two-layer system, CSS and markup
  included) back to a direct form swap.
- **XHS note shell now uses the official 3:4 card geometry** (1080x1440
  aspect-ratio for the image strip and the no-image title card) instead of
  fixed-height crops.
- **AI generation buttons are reused as stop buttons**: the job console /
  thinking box / separate stop button are gone; while a job runs its trigger
  button shows a spinner plus 停止 and clicking it cancels (capture-phase
  handler, backend real disconnect unchanged). Console mounts and CSS were
  removed from all four panels.
- **Channel preview: the platform's own layout in the phone mockup** (the
  missing half of platform switching). One article, one shape per channel:
  WeChat keeps the dialect article scroll; image-note platforms (Xiaohongshu
  first) render inside the same device as a feed-note shell - image strip
  (swiper dots, up to 9), note title, author row, caption text with
  highlighted hashtags and a like/save/comment action bar, accent-coloured
  per platform; Markdown-friendly hosts (Zhihu) get plain typographic HTML
  (`platform::render_plain_html`, real headings/lists/tables with resolved
  inline images). Backed by the `platform_preview` command returning a
  per-channel preview model (article / note / plain) built on the caption
  renderer and the image pipeline's resolved sources.
- **Platform export adapters & per-platform rule tables** (the essential
  platform difference, PRD §16): the primary copy action now produces the
  target platform's own artifact - WeChat keeps dialect rich text,
  Xiaohongshu copies a plain-text caption (markdown linearised via the IR:
  bullets, image placeholders, links keep text only since caption hosts do
  not autolink), Zhihu copies raw Markdown. The compliance chip follows the
  platform rule table: Xiaohongshu gains XHS-1..5 (title <=20 chars, note
  body <=1000 chars blocking, hashtag convention, image-set presence, <=9
  images); non-WeChat targets show the platform verdict instead of the
  dialect verdict. Platform is persisted per article in frontmatter
  (`platform:`), restored on open, and switching marks the article dirty.
  Core: `platform::render_caption` + `platform::validate_platform_caption`
  + `ExportKind`; commands `platform_export_text` / `platform_validate`;
  copy button label morphs per platform.
- **Unified AI generation runtime (`jobs.rs` + `ai-jobs.js`)**: every GUI
  generation panel (AI theme, SVG component, poster HTML, cloud image,
  ComfyUI t2i/i2i) now runs as a *job* — a registry entry with an id and a
  cancel flag, streaming progress over one `ai-job` event channel. The
  frontend renders a shared console widget (status + elapsed + stop button +
  collapsible stream log, DeepSeek-style thinking indicator). Stop is real:
  the SSE read loop checks the cancel flag between chunks and drops the
  connection; ComfyUI runs additionally POST `/interrupt`. Multiple jobs can
  run concurrently without interfering.
- **Generate-validate-repair as one task table**: theme / svg / poster share
  a single spec-driven loop (system prompt, output extractor, MP-compliance
  validator, bounded retries that feed errors back to the model). The poster
  path gains what it never had: a self-containment gate (no script/iframe/
  link/@import/external url or src) enforced *before* rasterization, with
  violations fed back for repair. Poster prompt moved to the backend and is
  platform-aware.
- **Reasoning-model capability heuristic**: models whose id suggests a
  thinking phase (o1/o3/r1/reasoner/glm-z/qwq/...) start generation with a
  larger token budget; empty-but-truncated replies ladder the budget
  (cap 16384) before failing; `<think>` blocks are stripped. Fixes "AI 主题
  未通过合规校验：AI 输出被 max_tokens 截断" on reasoning-heavy models.
- **Platform registry (`wxwright-core::platform`, PRD §16)**: six platform
  descriptors (WeChat MP / Xiaohongshu / Zhihu / Meta / X / LinkedIn) with
  capabilities (rich text? image-note? API publish?), preset canvas sizes
  and honest capability notes. `wxwright platforms` CLI lists them (JSON in
  `--json` mode). The GUI top bar gains a platform switcher: poster and
  size-studio presets, the AI poster copy style, and the Xiaohongshu
  "export image set" batch action (rasterizes the current poster at every
  preset size into the asset library) all follow the switch. Untouched
  poster templates follow the platform's canvas; edited HTML is preserved.
- **Chart engine**: a ` ```chart ` fenced JSON block (`kind: bar|line|pie`,
  `title`, `labels[]`, `values[]`, optional `unit`) renders as an inline
  self-contained SVG chart (677x430, 6-colour palette) that passes the MP
  validator like any other component - malformed JSON degrades to a plain
  code block. The AI assistant system prompt teaches the schema so models
  can output data visualisations directly.
- Official vendor logos grown to 17 marks: added xAI (official Wikimedia
  SVG, path-traced to 24x24) and Zhipu GLM (official logo PNG embedded as a
  data URI, same path as Agnes).
- WeChat MP API binding UI: the Settings modal is restructured into a
  left-nav layout with four panes (AI Providers / MP API / ComfyUI / About).
  The MP pane binds AppID + AppSecret via `wx_bind` (stored in the OS
  keychain by `wxwright-mp::save_credentials`, file fallback under
  %APPDATA%), shows masked bind status, and supports unbinding.
- Status bar: the compliance-check button moved next to Save (pill styles
  for both) instead of the top toolbar.
- SVG kit colour params (background/text/accent) now ship a native
  `<input type="color">` picker next to the hex field, two-way synced.
- AI landing guidance: inserting/replacing content from the AI drawer
  selects the affected editor range with a one-shot glow, and the preview
  pulses its newest last block once the re-render lands.
- Mozai the writing pet now cross-fades between forms (two stacked layers,
  0.35s opacity ease) instead of popping.
- Official vendor logos: a bundled logo library (Simple Icons, CC0) with 14
  official marks (OpenAI, Anthropic, Google Gemini, DeepSeek, Moonshot Kimi,
  Qwen, Meta, Mistral, Ollama, OpenRouter, Perplexity, HuggingFace, LM Studio,
  xAI-adjacent aliases) rendered inline - zero external requests. Provider
  rows, the model quick-select pill and the model menu all show the official
  mark; keyword auto-match (name/base_url/model) plus an explicit `brand`
  field; custom uploaded logos still override. A vendor preset dropdown in
  the provider form auto-fills name/base URL/model/brand for 13 common
  providers (one click instead of typing).
- MCP tool `wxwright_upload_images` (mmbiz upload decoupled from draft
  creation), closing the gap between the PRD §7 tool contract and the server.
- Test `tool_surface_matches_agent_card`: the MCP implementation, the agent
  card, and the README now fail CI if the tool list drifts apart.

### Changed
- App icons regenerated by a supersampling pipeline (1024 px render ->
  Lanczos3 downsample -> 8-frame ICO) for crisp small sizes.
- Clipboard: the `text/html` rich flavor is attempted on Windows, macOS and
  Linux alike (arboard 3.6 supports all three) and only degrades to plain
  text where the clipboard cannot accept HTML (e.g. headless CI). Previously
  it hard-coded rich copy to Windows only.
- `panic = "unwind"` in the release profile + per-request `catch_unwind` in
  the MCP stdio loop, so one handler panic answers a JSON-RPC error instead
  of dropping the agent's whole session.
- GUI AI theme generation: `max_tokens` 2000 -> 4096, and an empty AI reply
  now reports *why* (length-truncated / content-filter / reasoning-only)
  instead of the useless "AI 返回为空".
- Version normalized to `0.9.0` (workspace + tauri.conf); `repository`
  unified to the `YaoIsAI/wxwright` namespace.

### Fixed
- `.gitignore`: the blanket `assets/` rule was silently excluding the icon
  source-of-truth `assets/icon/master.svg` from commits; re-included it while
  still ignoring runtime asset output.
- Cleared all `clippy -D warnings` lints (manual_clamp, let_unit_value,
  into_iter_on_ref, type_complexity, len_zero, needless_return) so CI is green.

## [0.9.0] - 2026-09-25

First consolidated release: one pure-Rust engine shared by three hosts,
enforcing the official WeChat editor spec with a generate-then-verify
pipeline.

### Added
- **Engine (`wxwright-core`)**: GFM parser (headings/tables/task lists/alert
  cards/TOC/figures/inline code), dialect renderer (`section` + `span[leaf]`,
  all styles inline), TOML theme system (minimal / techblue / magazine),
  syntect-based code highlighting, image pipeline (`data-w`/`data-ratio`,
  base64 inline / mmbiz upload via a swappable `ImageTransport`), clipboard
  payload builder, i18n (en / zh-CN, machine-facing keys stay English).
- **Official spec rule engine** (PRD §5.3): normalizer (auto-fix) + validator
  (detect) for R-1.1 … R-4.4 plus payload-hygiene rules; `data-ignore-width`
  subtree exemption honored on both sides; every rule backed by a
  violation-sample test.
- **CLI** (`wxwright`): convert / validate / fix / copy / image upload / draft
  create|update|list / publish / theme list|new|validate / doctor / mcp serve /
  mcp install / agent-card / login / logout / bench. Exit codes 0/1/2 aligned
  with the official verify CLI; `--json` on every command; stdin `-`;
  `--lang` / `NO_COLOR` / `--no-color`. Credentials via OS keychain with
  env-var override; `publish` requires explicit `--yes`.
- **MCP server** (stdio JSON-RPC): 7 tools, 2 resources (incl. the bilingual
  rule table), a publish-guide prompt; one-click `mcp install` for
  Claude / Cursor / VS Code / OpenCode.
- **Desktop GUI (Tauri 2)**: three-pane layout (article library / editor /
  phone-frame preview), Dark Mode simulation, one-click rich copy, HTML
  export, .md drag-and-drop; local article + asset libraries; AI assistant
  drawer (OpenAI-compatible providers incl. local Ollama / LM Studio, keys in
  the OS keychain, streaming with stop); agent integration panel; AI theme
  generator (auto-validated against the rules); Poster Studio (HTML -> PNG,
  offline); ComfyUI text/image-to-image integration; iPhone/Pixel device
  frames. The GUI binary also answers `mcp serve`.
- **Agent hand-off card** (`agent-card`): paste-into-system-prompt contract,
  snapshot-tested against the real tool surface.
- **Design governance**: single icon master -> platform icons via `icongen`;
  zero-emoji repo scan test; ASCII CLI prefixes; SVG-only icons.
- **CI**: three-platform build/test/clippy/fmt matrix + the official
  `verify-article-structure-spec` puppeteer truth gate; tag-driven
  multi-platform release workflow (Windows NSIS+portable, macOS dmg+CLI
  arm64/x64, Linux deb/AppImage+musl CLI, SHA256SUMS) — see
  `docs/release-automation.md`.
- **Docs**: `agent-integration.md`, `performance.md` (SLO table + local
  `bench`), `manual-test-matrix.md` (four-view regression), README +
  README.zh-CN.

### Known limitations
- Formula rendering keeps LaTeX as styled text (image mode needs a bridge).
- `R-2.1` deep identical-wrapper chains are detected but not auto-unwrapped.
- `--official-check` prints guidance; the puppeteer run itself lives in CI.
- clap help text is English; reports are localized (en / zh-CN).

[Unreleased]: https://github.com/YaoIsAI/wxwright/compare/v0.10.0...HEAD
[0.10.0]: https://github.com/YaoIsAI/wxwright/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/YaoIsAI/wxwright/releases/tag/v0.9.0
