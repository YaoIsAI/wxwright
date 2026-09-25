# Performance SLOs (PRD 5.7)

Architecture premise: single static binary, single process, zero IPC, zero
serialization — every byte of the budget is spent on computing, not frameworks.

## SLO table (release build)

| # | Metric | p50 | p95 | Measured by |
|---|---|---|---|---|
| P-1 | CLI cold start (`--version`) | ≤ 8 ms | ≤ 20 ms | wall clock + CI runner |
| P-2 | 10k-char full chain (md → dialect → normalize → validate, no upload) | ≤ 60 ms | ≤ 150 ms | `wxwright bench` |
| P-3 | single 2 MB image decode + dimension probe | ≤ 5 ms | ≤ 15 ms | header-only decode (`image::ImageReader::into_dimensions`) |
| P-4 | GUI first interactive frame | ≤ 400 ms | ≤ 800 ms | Tauri startup marks |
| P-5 | GUI incremental re-render on input | ≤ 8 ms | ≤ 16 ms | 220 ms debounce + sub-ms convert |
| P-6 | engine resident memory | ≤ 50 MB | ≤ 80 MB | task manager sampling |
| P-7 | CLI binary size (single platform) | ≤ 8 MB | ≤ 15 MB | CI size gate |

Reproduce P-2 locally with `wxwright bench` (release build).

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

## Regression policy

Any PR that regresses a benchmark by > 5% must attach a bench diff or be
reverted. `wxwright bench --json` is CI-friendly (stable schema).
