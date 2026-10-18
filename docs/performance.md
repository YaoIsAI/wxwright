# Performance SLOs (PRD 5.7)

Architecture premise: single static binary, single process, zero IPC, zero
serialization — every byte of the budget is spent on computing, not frameworks.

## The 500 ms interaction budget (2026-10-18)

Every local operation must land inside 500 ms wall time; network operations can
never promise that, so the budget they must meet is **feedback**, not
completion: a click changes visible UI state within 100 ms (streaming first
token, button morph, status line, progress), and the long work continues in
the background.

| Class | Operation | Budget | Status |
|---|---|---|---|
| Local compute | engine full chain, 13k chars (P-2) | ≤ 60 ms (SLO) / 500 ms (hard cap) | measured, see below |
| Local compute | platform preview + validate (GUI) | ≤ 30 ms | pass |
| UI interaction | tab switch, panel open, kit card expand | ≤ 16 ms | pass |
| Perceived | keystroke → preview update (180 ms debounce + convert + iframe reload) | ≤ 500 ms | pass |
| Startup | GUI first interactive frame (P-4) | ≤ 400 ms p50 | SLO |
| Startup | GUI first preview render | ≤ 500 ms | init loads editor + converts first, all other steps run in parallel |
| Startup | ComfyUI status probe (offline host) | ≤ 1 s, never blocks first preview | fast probe |
| Feedback | AI chat / image gen / ComfyUI / draft push / X-LinkedIn post | first visible reaction ≤ 100 ms; completion bounded by network | streaming + button morph + status lines |

## SLO table (release build)

| # | Metric | p50 | p95 | Measured by |
|---|---|---|---|---|
| P-1 | CLI cold start (`--version`) | ≤ 8 ms | ≤ 20 ms | wall clock + CI runner |
| P-2 | 10k-char full chain (md → dialect → normalize → validate, no upload) | ≤ 60 ms | ≤ 150 ms | `wxwright bench` |
| P-3 | single 2 MB image decode + dimension probe | ≤ 5 ms | ≤ 15 ms | header-only decode (`image::ImageReader::into_dimensions`) |
| P-4 | GUI first interactive frame | ≤ 400 ms | ≤ 800 ms | Tauri startup marks |
| P-5 | GUI incremental re-render on input | ≤ 8 ms | ≤ 16 ms | 180 ms debounce + sub-ms convert |
| P-6 | engine resident memory | ≤ 50 MB | ≤ 80 MB | task manager sampling |
| P-7 | CLI binary size (single platform) | ≤ 8 MB | ≤ 15 MB | CI size gate |

Reproduce P-2 locally with `wxwright bench` (release build); the JSON output
carries `stages_ms_p50` (parse / render / stats / normalize / validate) so a
regression can be attributed without a profiler.

## Engineering means

- Build: `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="unwind"`, `strip=true` (workspace `Cargo.toml`). Note: unwind (not abort) is a deliberate trade — the MCP stdio session catches handler panics per request (catch_unwind) instead of dropping the agent's connection; the cost is a small landing-pad table, invisible in the budget below.
- Parsing/normalizing: pulldown-cmark streaming + single-pass lol_html rewriter passes; **zero regex** — style attributes are parsed by a hand-written scanner.
- No daemons: every command runs to completion and exits; no background processes, no watchers.
- Small dep surface for the CLI: no tokio, no rayon; uploads use blocking `ureq` with retry/backoff; batch image work is sequential (typical article ≤ 20 images).
- Version-pinned theme rendering: TOML themes compile to inline styles at render time — no template engine in the hot path.

## Measured (2026-09-25, Windows 10 x64, Rust 1.96, release build, local runs)

| Metric | This machine | SLO | Status |
|---|---|---|---|
| P-2 13k-char full chain (`wxwright bench`) | p50 **47.8 ms** / p95 **62.7 ms** | ≤60 / ≤150 ms | pass |
| P-7 CLI binary (`target/release/wxwright.exe`) | **7.4 MB** | ≤8 / ≤15 MB | pass |
| P-1 cold start (wall, 5 runs via PowerShell, incl. process spawn) | ~25 ms per invocation | ≤8 / ≤20 ms (instruction-count budget) | *wall-clock caveat* |

P-1 note: the ≤8ms budget is defined against CI instruction counting
(iai-callgrind), which excludes OS process-creation overhead. The ~25 ms here
is end-to-end wall time including spawn + PowerShell invocation — not
comparable to the budget. CI measures P-1/P-3/P-6/P-4/P-5 on fixed runners;
reproduce P-2/P-7 locally with `wxwright bench --json` and file size.

## Measured (2026-10-19): regression attributed and fixed

By October the same bench doc measured p50 76–100 ms on the same machine —
beyond machine noise. The stage timers attributed the drift:

- `validate` grew to ~44 % of the chain: `check_element_end` cloned the node
  and built its violation hint string for **every** element, compliant or not;
- `render` grew to ~22 %: `css_variant` re-expanded tokens and rebuilt style
  strings for every element with the same (role, variant).

Fixes (0.12.3): the hint string is built only when a rule actually fires, and
`css_variant` memoizes per render. Interleaved A/B (old vs new binary,
alternating runs, 4 rounds) showed **old ≈ 124.6 ms vs new ≈ 101.0 ms mean,
≈ 19–25 % faster**. Stage shares after the fix: validate ~40 %, normalize
~37 % (untouched baseline), render ~13 %, parse ~2 %.

Caveat carried forward: this laptop's absolute numbers swing ±20 %+ with
thermal state — the same old-code binary measured 105–150 ms on a warm
afternoon versus 76 ms earlier the same day. Absolute SLO verdicts belong to
CI's fixed runners; locally, always A/B interleaved.

## Regression policy

Any PR that regresses a benchmark by > 5% must attach a bench diff or be
reverted. `wxwright bench --json` is CI-friendly (stable schema).
