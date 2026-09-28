//! Design governance scan (PRD 9-8): zero emoji across product surfaces.
//! Scans all source/UI/docs files in the repository for emoji codepoints.
//! PRD.md is excluded: it is the upstream input spec, not a product artifact.

use std::path::Path;

/// Codepoints outside the big blocks that still carry Emoji_Presentation.
///
/// Listed individually rather than by range on purpose: the surrounding blocks
/// are full of legitimate typography (U+2318 PLACE OF INTEREST SIGN, U+25AA
/// BLACK SMALL SQUARE, the whole CJK-enclosed range), and banning those would
/// make the guard reject correct files.
const EMOJI_OUTLIERS: &[u32] = &[
    0x231A, 0x231B, // watch, hourglass
    0x23E9, 0x23EA, 0x23EB, 0x23EC, // fast-forward / rewind
    0x23F0, 0x23F3, // alarm clock, hourglass with flowing sand
    0x25B6, 0x25C0, // play / reverse button
    0x25FB, 0x25FC, 0x25FD, 0x25FE, // medium squares
    0x3297, 0x3299, // CJK "congratulations" / "secret"
];

fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    (0x1F000..=0x1FAFF).contains(&cp)
        || (0x2600..=0x27BF).contains(&cp)
        || (0x2B00..=0x2BFF).contains(&cp)
        || (0x1F1E6..=0x1F1FF).contains(&cp)
        || cp == 0xFE0F
        || cp == 0x2764
        || (0x1F900..=0x1F9FF).contains(&cp)
        || EMOJI_OUTLIERS.contains(&cp)
}

const SCAN_EXTENSIONS: &[&str] = &[
    "rs", "toml", "md", "js", "css", "html", "json", "svg", "yml", "yaml", "txt", "lock", "py",
    "sh", "bat", "ps1", "mjs", "cjs", "ts", "tsx", "jsx",
];

/// Directories that are never published, so they are outside the "product
/// surface" this test guards (PRD 3.8-B). `.workbuddy-ai/` and `review/` are
/// gitignored local artifacts - agent work logs and generated reports - so
/// scanning them produced a failure that only ever reproduced on a developer
/// machine (CI checks out from git and never sees them). A guard that is red
/// locally and green in CI trains people to ignore it.
const SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    "node_modules",
    "dist",
    ".workbuddy-ai",
    "review",
];

const SKIP_FILES: &[&str] = &["PRD.md"];

#[test]
fn no_emoji_anywhere_in_product_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut offenders: Vec<String> = Vec::new();
    scan(root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "emoji found in product files (PRD 3.8-B forbids them):\n{}",
        offenders.join("\n")
    );
}

fn scan(dir: &Path, offenders: &mut Vec<String>) {
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
            scan(&path, offenders);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if !SCAN_EXTENSIONS.contains(&ext) || SKIP_FILES.contains(&name.as_str()) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_no, line) in content.lines().enumerate() {
                    if let Some(c) = line.chars().find(|c| is_emoji(*c)) {
                        offenders.push(format!(
                            "{}:{}: U+{:04X} {:?}",
                            path.display(),
                            line_no + 1,
                            c as u32,
                            c
                        ));
                        break;
                    }
                }
            }
        }
    }
}
