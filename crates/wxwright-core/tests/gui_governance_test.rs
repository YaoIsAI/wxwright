//! Governance guards for the two iron laws that had no automated check
//! (AGENTS.md section 4).
//!
//! Iron law 2 - every `#[tauri::command]` must be listed in `lib.rs`'s
//! `generate_handler!`. A command that is defined but not registered is dead
//! code that still compiles: the frontend calls it, gets "command not found"
//! at runtime, and nothing fails until a user clicks the button. That is
//! recorded as pitfall 4 (`extract_document_text` shipped dead for months).
//!
//! Iron law 4 - a new UI string needs an entry in BOTH the zh-CN and the en
//! dictionary, and `t()` falls back to the key name rather than showing
//! `undefined`. Only the engine's 11 keys were asserted; the GUI's ~180 keys
//! had no guard at all, so a missing translation would ship silently as a raw
//! key like `push_draft_ok` rendered in the middle of the UI.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

// --------------------------------------------------------------- iron law 2 --

/// Names of `#[tauri::command]` functions in a source file.
fn declared_commands(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let lines: Vec<&str> = src.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.trim().starts_with("#[tauri::command]") {
            continue;
        }
        // The signature follows, possibly after more attributes.
        for l in lines.iter().skip(i + 1).take(4) {
            let t = l.trim();
            if t.starts_with("#[") {
                continue;
            }
            let rest = t
                .strip_prefix("pub async fn ")
                .or_else(|| t.strip_prefix("pub fn "));
            if let Some(rest) = rest {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.insert(name);
                }
            }
            break;
        }
    }
    out
}

/// Names listed inside `generate_handler![...]` in lib.rs.
fn registered_commands(src: &str) -> BTreeSet<String> {
    let start = src
        .find("generate_handler![")
        .expect("lib.rs must contain a generate_handler! block");
    let rest = &src[start..];
    let end = rest.find(']').expect("generate_handler! must be closed");
    rest[..end]
        .lines()
        .filter_map(|l| {
            let t = l.trim().trim_end_matches(',');
            // entries look like `commands::name` or `social::name`
            let (_, name) = t.split_once("::")?;
            let name = name.trim();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

#[test]
fn every_tauri_command_is_registered() {
    let mut declared = declared_commands(&read("gui/src-tauri/src/commands.rs"));
    declared.extend(declared_commands(&read("gui/src-tauri/src/social.rs")));
    let registered = registered_commands(&read("gui/src-tauri/src/lib.rs"));

    assert!(
        declared.len() > 40,
        "the command scan found suspiciously few commands ({}); did the attribute \
         or signature style change?",
        declared.len()
    );

    let missing: Vec<&String> = declared.difference(&registered).collect();
    assert!(
        missing.is_empty(),
        "these #[tauri::command] functions are not registered in lib.rs's \
         generate_handler!, so calling them from the frontend fails at runtime: {missing:?}"
    );

    let phantom: Vec<&String> = registered.difference(&declared).collect();
    assert!(
        phantom.is_empty(),
        "lib.rs registers commands that no longer exist (compile error?) or whose \
         scan failed: {phantom:?}"
    );
}

// --------------------------------------------------------------- iron law 4 --

/// Replace every JS string / template literal with an empty literal, so the
/// remaining text can be scanned for `ident:` key positions without tripping
/// over a colon inside a value (a URL, "提示: ...", or a `${...}` interpolation).
fn strip_string_literals(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' || c == '`' {
            out.push_str("\"\"");
            while let Some(d) = chars.next() {
                if d == '\\' {
                    chars.next();
                    continue;
                }
                if d == c {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Keys of a JS object literal body: an identifier at a key position followed
/// by `:`. Values may be strings, template literals or arrow functions, so the
/// scan only looks at the stripped text.
fn object_keys(stripped: &str) -> BTreeSet<String> {
    let b = stripped.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let mut j = i;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let prev = if start == 0 { b'{' } else { b[start - 1] };
            if j < b.len()
                && b[j] == b':'
                && matches!(prev, b'{' | b',' | b'\n' | b' ' | b'\t' | b'(' | b';')
            {
                out.insert(stripped[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The zh-CN and en dictionaries of `gui/ui/app.js`.
fn gui_dictionaries() -> (BTreeSet<String>, BTreeSet<String>) {
    let src = read("gui/ui/app.js");
    let start = src
        .find("const I18N = {")
        .expect("app.js must declare `const I18N = {`");
    let region = &src[start..];
    let end = region
        .find("\n};")
        .expect("the I18N object literal must be closed at column 0");
    let region = &region[..end];

    // Skip past the `en: {` marker itself, otherwise the top-level `en` key
    // is counted as a member of the en dictionary and the comparison fails.
    let marker = "\n  en: {";
    let split = region
        .find(marker)
        .expect("the I18N object must contain an `en:` dictionary");
    let zh = strip_string_literals(&region[..split]);
    let en = strip_string_literals(&region[split + marker.len()..]);
    (object_keys(&zh), object_keys(&en))
}

#[test]
fn gui_dictionaries_have_identical_keys() {
    let (zh, en) = gui_dictionaries();
    assert!(
        zh.len() > 100,
        "the zh-CN dictionary scan found only {} keys; did the literal format change?",
        zh.len()
    );
    let missing_in_en: Vec<&String> = zh.difference(&en).collect();
    let missing_in_zh: Vec<&String> = en.difference(&zh).collect();
    assert!(
        missing_in_en.is_empty() && missing_in_zh.is_empty(),
        "the GUI i18n dictionaries drifted (iron law 4).\n  \
         missing from en: {missing_in_en:?}\n  missing from zh-CN: {missing_in_zh:?}"
    );
}

#[test]
fn every_static_i18n_attribute_resolves() {
    let (zh, en) = gui_dictionaries();
    let html = read("gui/ui/index.html");
    let mut checked = 0usize;
    let mut missing = Vec::new();
    for attr in [
        "data-i18n=\"",
        "data-i18n-title=\"",
        "data-i18n-placeholder=\"",
        "data-desc-i18n=\"",
    ] {
        let mut rest = html.as_str();
        while let Some(p) = rest.find(attr) {
            rest = &rest[p + attr.len()..];
            let Some(q) = rest.find('"') else { break };
            let key = &rest[..q];
            if key.is_empty() {
                continue;
            }
            checked += 1;
            if !zh.contains(key) || !en.contains(key) {
                missing.push(key.to_string());
            }
        }
    }
    assert!(
        checked > 100,
        "only {checked} static i18n attributes were found; the scan is probably wrong"
    );
    assert!(
        missing.is_empty(),
        "index.html references i18n keys that are not in both dictionaries, so the \
         UI would render the raw key name: {missing:?}"
    );
}
