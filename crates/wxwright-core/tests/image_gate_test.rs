//! Iron law 12 (AGENTS.md): the clipboard / draft image gate has exactly one
//! implementation, `ImageOutcome::paste_hostile()`.
//!
//! History: the gate was hand-rolled in three slightly different ways. The CLI
//! copy path blocked local/base64 images; the GUI copy path re-implemented the
//! same filter; the MCP copy path wrote a third variant that additionally let
//! plain-http URLs and images that failed to load through; and the MCP
//! `wxwright_draft_create` path had no gate at all. Same article, four
//! different answers about whether it is safe to ship.
//!
//! This test is a governance guard rather than a behavioural one: it scans the
//! sources so a future hand-rolled filter cannot reappear unnoticed.

use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules", "dist", "review"];

/// Every call site that must go through the single predicate.
const REQUIRED_SITES: &[(&str, usize)] = &[
    ("crates/wxwright-cli/src/main.rs", 1),
    ("crates/wxwright-mcp/src/lib.rs", 2),
    // push draft + update draft: two write paths, one gate each
    ("gui/src-tauri/src/commands.rs", 3),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            rust_sources(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Strip line comments so prose that *mentions* the anti-pattern does not
/// trip the scan.
fn code_only(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

#[test]
fn every_write_path_uses_the_single_image_gate() {
    let root = repo_root();
    for (rel, expected) in REQUIRED_SITES {
        let src = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
        let found = src
            .lines()
            .map(code_only)
            .filter(|l| l.contains("paste_hostile()"))
            .count();
        assert_eq!(
            found, *expected,
            "{rel} should call paste_hostile() exactly {expected} time(s); \
             a write path probably lost its gate or grew a duplicate"
        );
    }
}

#[test]
fn no_hand_rolled_image_gate_exists() {
    let mut sources = Vec::new();
    rust_sources(&repo_root(), &mut sources);
    assert!(
        sources.len() > 20,
        "the source scan found suspiciously little"
    );

    let mut offenders = Vec::new();
    for path in sources {
        // This file necessarily contains the patterns it looks for.
        if path.ends_with("image_gate_test.rs") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in content.lines().enumerate() {
            let code = code_only(line);
            // The hand-rolled variants that diverged from paste_hostile().
            // `source.starts_with("data:")` is deliberately narrower than a
            // bare `starts_with("data:")`: SSE parsing legitimately tests the
            // same prefix on a response line, and flagging that would make the
            // guard cry wolf.
            if code.contains("inlined && !") || code.contains("source.starts_with(\"data:\")") {
                // The definition itself is the one legitimate place.
                if path.ends_with("img.rs") {
                    continue;
                }
                offenders.push(format!("{}:{}: {}", path.display(), n + 1, code.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "hand-rolled image gates found - use ImageOutcome::paste_hostile() instead:\n{}",
        offenders.join("\n")
    );
}
