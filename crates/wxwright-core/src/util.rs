//! Small shared utilities.

/// Query parameters whose values must never reach a log, an error message or
/// a status payload (PRD 5.6: access_token always masked).
const SECRET_KEYS: &[&str] = &[
    "secret",
    "appsecret",
    "access_token",
    "client_secret",
    "refresh_token",
    "session_key",
    "code",
    "code_verifier",
    "ticket",
];

/// Base directory for wxwright user state (settings.json, config.toml,
/// themes/, assets/). Mirrors `dirs::config_dir()` without pulling that
/// dependency into the engine: %APPDATA% on Windows, $XDG_CONFIG_HOME or
/// ~/.config elsewhere. Single definition shared by the CLI, the MP adapter
/// and the GUI, so `doctor` probes exactly the directory the app writes to.
pub fn config_root() -> std::path::PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|h| std::path::PathBuf::from(h).join("AppData").join("Roaming"))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        });
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".config"))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        });
    base.join("wxwright")
}

/// Replace the value of every credential-bearing query parameter with `***`.
///
/// Why this exists: `ureq`'s `Error::Status` Display renders as
/// `"{full_url}: status code {code}"`, so any 4xx/5xx on a WeChat API call
/// would otherwise print `...&secret=<AppSecret>` straight into CLI stdout,
/// JSON output and GUI toasts. Hand-written scanner - no regex on this path
/// (PRD 5.7-B keeps the engine regex-free).
///
/// A key only counts at a parameter boundary (start of string, or after
/// `?`/`&`/`;`/whitespace/quote) so that `errcode=` is never mistaken for
/// `code=`. Matching is ASCII-case-insensitive: `?SECRET=` leaks just as well
/// as `?secret=`.
pub fn redact_secrets(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev = '?';
    let mut rest = s;
    while !rest.is_empty() {
        let ch = match rest.chars().next() {
            Some(c) => c,
            None => break,
        };
        // `;` is a legal parameter separator (and shows up in cookie-shaped
        // strings), so it has to count as a boundary too.
        let at_boundary = matches!(
            prev,
            '?' | '&' | ';' | ' ' | '\t' | '\n' | '\r' | '"' | '\''
        );
        if at_boundary {
            if let Some(n) = match_secret_key(rest) {
                out.push_str(&rest[..n]); // keep the caller's own casing
                out.push_str("=***");
                let mut tail = &rest[n + 1..];
                while let Some(c) = tail.chars().next() {
                    if c == '&'
                        || c == ';'
                        || c.is_whitespace()
                        || c == '"'
                        || c == '\''
                        || c == ')'
                    {
                        break;
                    }
                    tail = &tail[c.len_utf8()..];
                }
                rest = tail;
                prev = '&';
                continue;
            }
        }
        out.push(ch);
        prev = ch;
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// Length of a credential parameter name at the head of `rest`, when it is
/// immediately followed by `=`. Case-insensitive; returns `None` otherwise.
fn match_secret_key(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    SECRET_KEYS.iter().find_map(|k| {
        let n = k.len();
        if bytes.len() > n && bytes[..n].eq_ignore_ascii_case(k.as_bytes()) && bytes[n] == b'=' {
            Some(n)
        } else {
            None
        }
    })
}

/// Write a file that holds credentials with owner-only permissions on Unix
/// (0600). Windows inherits the per-user ACL of %APPDATA%, which already
/// restricts other local accounts; there is no portable equivalent of chmod
/// through std, so the difference is documented rather than faked.
pub fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// Split text into lines, keeping line endings out of the returned strings.
pub fn split_lines(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        out.push(line.to_string());
    }
    if out.last().map(|l| l.is_empty()).unwrap_or(false) && out.len() > 1 {
        // Trailing newline produces one empty trailing line; drop it.
        out.pop();
    }
    out
}

/// Quick TCP reachability probe (used by `doctor` for local services like
/// ComfyUI on 127.0.0.1:8188). Any successful connect counts.
pub fn probe_tcp(host: &str, port: u16, timeout_ms: u64) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    if let Ok(addrs) = (host, port).to_socket_addrs() {
        for addr in addrs {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)).is_ok() {
                return true;
            }
        }
    }
    false
}

/// Strip YAML frontmatter from a Markdown document.
/// Returns (metadata pairs, body). Files without frontmatter pass through.
pub fn strip_frontmatter(md: &str) -> (Vec<(String, String)>, String) {
    let trimmed = md.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let header = &rest[..end];
            let body = rest[end + 4..].trim_start_matches('\n').to_string();
            let mut pairs = Vec::new();
            for line in header.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    pairs.push((
                        k.trim().to_string(),
                        v.trim().trim_matches('"').trim_matches('\'').to_string(),
                    ));
                }
            }
            return (pairs, body);
        }
    }
    (Vec::new(), md.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_frontmatter_roundtrip() {
        let (pairs, body) = strip_frontmatter("---\ntitle: \"你好\"\ntheme: minimal\n---\n\n# H\n");
        assert_eq!(pairs.iter().find(|(k, _)| k == "title").unwrap().1, "你好");
        assert_eq!(body.trim_start(), "# H\n");
        let (pairs2, body2) = strip_frontmatter("no frontmatter here");
        assert!(pairs2.is_empty());
        assert_eq!(body2, "no frontmatter here");
    }

    #[test]
    fn probe_tcp_closed_port() {
        // Port 1 on localhost is practically never open.
        assert!(!super::probe_tcp("127.0.0.1", 1, 300));
    }

    #[test]
    fn split_lines_basic() {
        assert_eq!(super::split_lines("a\nb\r\nc"), vec!["a", "b", "c"]);
        assert_eq!(super::split_lines("a\n"), vec!["a"]);
        assert_eq!(super::split_lines(""), vec![""]);
    }

    #[test]
    fn redact_strips_wechat_secrets_from_ureq_style_messages() {
        // The exact shape ureq produces: "<full url>: status code <code>".
        let msg = "http: https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid=wx1234567890&secret=SUPERSECRETVALUE: status code 401";
        let red = super::redact_secrets(msg);
        assert!(!red.contains("SUPERSECRETVALUE"), "got: {red}");
        assert!(red.contains("secret=***"), "got: {red}");
        assert!(red.contains("status code 401"), "keep the diagnosis: {red}");
        // appid is a public identifier (it is embedded in every article URL),
        // so it stays readable for debugging.
        assert!(red.contains("appid=wx1234567890"), "got: {red}");
    }

    #[test]
    fn redact_strips_access_token_and_oauth_material() {
        let msg = "https://api.weixin.qq.com/cgi-bin/draft/add?access_token=62_abcdefghijklmn";
        let red = super::redact_secrets(msg);
        assert!(!red.contains("62_abcdefghijklmn"), "got: {red}");
        assert!(red.contains("access_token=***"), "got: {red}");

        let oauth = "https://api.twitter.com/2/oauth2/token?code=AUTHCODE&code_verifier=VERIFIER123&client_id=abc";
        let red2 = super::redact_secrets(oauth);
        assert!(!red2.contains("AUTHCODE"), "got: {red2}");
        assert!(!red2.contains("VERIFIER123"), "got: {red2}");
        assert!(
            red2.contains("client_id=abc"),
            "client_id is public: {red2}"
        );
    }

    #[test]
    fn redact_does_not_mangle_errcode() {
        // "errcode" ends with "code" - a naive substring replace would corrupt it.
        let msg = "MP API error: errcode=40001 errmsg=invalid credential";
        assert_eq!(super::redact_secrets(msg), msg);
        let q = "https://x.y/cb?errcode=40001&state=abc";
        let red = super::redact_secrets(q);
        assert!(red.contains("errcode=40001"), "got: {red}");
        assert!(red.contains("state=abc"), "got: {red}");
    }

    #[test]
    fn redact_handles_multibyte_and_leaves_plain_text_alone() {
        assert_eq!(super::redact_secrets("普通错误信息"), "普通错误信息");
        let mixed = "上传失败: secret=密钥值&x=1";
        let red = super::redact_secrets(mixed);
        assert!(!red.contains("密钥值"), "got: {red}");
        assert!(red.contains("上传失败"), "got: {red}");
    }

    #[test]
    fn redact_covers_semicolon_separators_and_uppercase_keys() {
        // Both gaps were found by an independent verifier after the first fix
        // shipped: the boundary set only had `&`/`?` and matching was
        // case-sensitive, so these two shapes leaked in full.
        let semi = "?a=1;secret=LEAK;b=2";
        let red = super::redact_secrets(semi);
        assert!(
            !red.contains("LEAK"),
            "semicolon separator must redact: {red}"
        );
        assert!(red.contains("?a=1"), "unrelated params survive: {red}");

        for key in ["SECRET", "Secret", "ACCESS_TOKEN", "Client_Secret"] {
            let s = format!("?{key}=LEAKVALUE&ok=1");
            let red = super::redact_secrets(&s);
            assert!(
                !red.contains("LEAKVALUE"),
                "{key} must be redacted case-insensitively, got: {red}"
            );
        }
    }

    #[test]
    fn redact_still_ignores_substrings_of_other_names() {
        // Case-insensitive matching must not start eating unrelated names that
        // merely end with a secret key.
        for s in [
            "errcode=40001",
            "?errcode=40001&errmsg=bad",
            "?mysecret=not-a-credential",
            "secret_sauce=tasty",
        ] {
            assert_eq!(super::redact_secrets(s), s, "must not touch: {s}");
        }
    }
}
