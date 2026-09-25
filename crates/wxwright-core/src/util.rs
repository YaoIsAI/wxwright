//! Small shared utilities.

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
}
