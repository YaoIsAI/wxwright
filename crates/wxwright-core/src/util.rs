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

#[cfg(test)]
mod tests {
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
