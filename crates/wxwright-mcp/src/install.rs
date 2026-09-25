//! One-click MCP client installation (PRD 6 `mcp install`).
//! Shared by the CLI and the GUI so both write identical configs.
//!
//! The config command prefers a sibling `wxwright` CLI binary (dist layout);
//! as a fallback the GUI binary itself answers `mcp serve` (see gui main.rs),
//! so a standalone GUI install still works.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub target: String,
    pub config_path: String,
    pub key: String,
    pub command: String,
    /// True when the config file did not exist and was created.
    pub created: bool,
    /// True when a .bak backup of the previous config was written.
    pub backup_created: bool,
    /// Non-fatal hint (e.g. no sibling CLI binary found).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

pub const TARGETS: &[&str] = &["claude", "cursor", "vscode", "opencode"];

/// Install the wxwright MCP server into the given client's config.
/// `target` is one of `TARGETS`.
pub fn install(target: &str) -> Result<InstallOutcome, String> {
    let (path, key, entry) = match target {
        "claude" => (
            claude_config_path(),
            "mcpServers",
            serde_json::json!({ "command": cli_command(), "args": ["mcp", "serve"] }),
        ),
        "cursor" => (
            home_dir().join(".cursor").join("mcp.json"),
            "mcpServers",
            serde_json::json!({ "command": cli_command(), "args": ["mcp", "serve"] }),
        ),
        "vscode" => (
            vscode_mcp_path(),
            "servers",
            serde_json::json!({ "command": cli_command(), "args": ["mcp", "serve"] }),
        ),
        "opencode" => (
            home_dir()
                .join(".config")
                .join("opencode")
                .join("opencode.json"),
            "mcp",
            serde_json::json!({ "type": "local", "command": [cli_command(), "mcp", "serve"] }),
        ),
        other => {
            return Err(format!(
                "unknown target {:?}; supported: {}",
                other,
                TARGETS.join(", ")
            ))
        }
    };
    write_config(target, &path, key, entry)
}

/// Prefer a sibling `wxwright` CLI binary; fall back to this binary (the GUI
/// binary handles `mcp serve` argv directly).
fn cli_command() -> String {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("wxwright"));
    let cli_sibling = exe
        .parent()
        .map(|dir| {
            dir.join(if cfg!(windows) {
                "wxwright.exe"
            } else {
                "wxwright"
            })
        })
        .unwrap_or_else(|| exe.clone());
    if cli_sibling.is_file() {
        return cli_sibling.to_string_lossy().to_string();
    }
    exe.to_string_lossy().to_string()
}

fn no_sibling_warning() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let sibling = exe.parent().map(|dir| {
        dir.join(if cfg!(windows) {
            "wxwright.exe"
        } else {
            "wxwright"
        })
    })?;
    if sibling.is_file() {
        None
    } else {
        Some(
            "sibling wxwright CLI not found; the config points at this binary, \
             which also answers `mcp serve`"
                .into(),
        )
    }
}

fn write_config(
    target: &str,
    path: &PathBuf,
    key: &str,
    entry: serde_json::Value,
) -> Result<InstallOutcome, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    let existed = path.exists();
    let mut backup_created = false;
    let mut root: serde_json::Value = if existed {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        let backup = path.with_extension("json.bak");
        if std::fs::copy(path, &backup).is_ok() {
            backup_created = true;
        }
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let mut section = root
        .get(key)
        .and_then(|s| s.as_object())
        .cloned()
        .unwrap_or_default();
    section.insert("wxwright".to_string(), entry);
    root.as_object_mut()
        .ok_or("config root is not an object")?
        .insert(key.to_string(), serde_json::Value::Object(section));
    let body = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;

    Ok(InstallOutcome {
        target: target.to_string(),
        config_path: path.to_string_lossy().to_string(),
        key: key.to_string(),
        command: cli_command(),
        created: !existed,
        backup_created,
        warning: no_sibling_warning(),
    })
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn claude_config_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        home_dir()
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(home_dir)
            .join("Claude")
            .join("claude_desktop_config.json")
    } else {
        home_dir()
            .join(".config")
            .join("Claude")
            .join("claude_desktop_config.json")
    }
}

fn vscode_mcp_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        home_dir()
            .join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("mcp.json")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(home_dir)
            .join("Code")
            .join("User")
            .join("mcp.json")
    } else {
        home_dir()
            .join(".config")
            .join("Code")
            .join("User")
            .join("mcp.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_target_rejected() {
        assert!(install("not-a-client").is_err());
    }

    #[test]
    fn targets_covered() {
        assert_eq!(TARGETS, &["claude", "cursor", "vscode", "opencode"]);
    }
}
