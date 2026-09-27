//! Shared HTTP agent construction.
//!
//! `ureq` deliberately ignores the environment and the Windows system proxy:
//! its `proxy-from-env` feature is opt-in, has no `NO_PROXY` support and
//! prefers `ALL_PROXY` over the scheme-specific `HTTPS_PROXY`. On a machine
//! behind a local proxy (Clash/Mihomo on 127.0.0.1:7897 and the like) every
//! foreign call therefore failed with `os error 10060` while `curl` - which
//! does read `HTTPS_PROXY` - worked fine. The AI assistant, cloud image
//! generation and the X/LinkedIn OAuth bindings were all dead there.
//!
//! Only the foreign endpoints opt in. ComfyUI (127.0.0.1:8188) and the WeChat
//! MP API (domestic) keep their direct connections on purpose: routing loopback
//! through a proxy is pointless, and pushing a domestic API through a foreign
//! tunnel would make a working path worse.

use std::time::Duration;

/// Apply the environment proxy, if one is configured, to an agent builder.
pub fn with_env_proxy(b: ureq::AgentBuilder) -> ureq::AgentBuilder {
    match wxwright_core::util::proxy_url_from_env() {
        Some(url) => match ureq::Proxy::new(&url) {
            Ok(proxy) => b.proxy(proxy),
            Err(e) => {
                eprintln!("wxwright: ignoring unparsable proxy {url:?}: {e}");
                b
            }
        },
        None => b,
    }
}

/// An agent with a read timeout and the environment proxy applied.
pub fn agent_with_read_timeout(read_timeout: Duration) -> ureq::Agent {
    with_env_proxy(ureq::AgentBuilder::new().timeout_read(read_timeout)).build()
}
