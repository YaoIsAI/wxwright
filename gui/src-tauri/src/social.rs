//! Overseas publish bindings: bring-your-own OAuth (the tool ships no cloud
//! service). Users register their own developer app per platform, paste the
//! client id (and secret where the platform requires one), then one-click
//! login opens the system browser and the authorization code lands on a local
//! loopback listener. Tokens live in the OS keyring (file fallback, same
//! policy as wxwright-mp). Publish exits are layered on top per platform
//! later; this module only owns configuration, login and token storage.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

/// Static per-platform contract. `flow_ready` marks platforms whose login
/// flow is implemented end to end (x/linkedin); meta/instagram keep the
/// credential interface but their flows are gated on review/图床 prerequisites
/// (docs/social-publish-oauth-feasibility.md).
#[derive(Clone, Copy)]
pub struct SocialSpec {
    pub id: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub port: u16,
    pub needs_secret: bool,
    pub flow_ready: bool,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scope: &'static str,
}

pub const SPECS: &[SocialSpec] = &[
    SocialSpec {
        id: "x",
        name_zh: "X (Twitter)",
        name_en: "X (Twitter)",
        port: 8761,
        needs_secret: false,
        flow_ready: true,
        auth_url: "https://x.com/i/oauth2/authorize",
        token_url: "https://api.x.com/2/oauth2/token",
        scope: "tweet.read tweet.write users.read offline.access",
    },
    SocialSpec {
        id: "linkedin",
        name_zh: "LinkedIn",
        name_en: "LinkedIn",
        port: 8762,
        needs_secret: true,
        flow_ready: true,
        auth_url: "https://www.linkedin.com/oauth/v2/authorization",
        token_url: "https://www.linkedin.com/oauth/v2/accessToken",
        scope: "openid profile w_member_social",
    },
    SocialSpec {
        id: "meta",
        name_zh: "Facebook",
        name_en: "Facebook",
        port: 8763,
        needs_secret: true,
        flow_ready: false,
        auth_url: "",
        token_url: "",
        scope: "",
    },
    SocialSpec {
        id: "instagram",
        name_zh: "Instagram",
        name_en: "Instagram",
        port: 8764,
        needs_secret: true,
        flow_ready: false,
        auth_url: "",
        token_url: "",
        scope: "",
    },
];

pub fn spec(id: &str) -> Option<&'static SocialSpec> {
    SPECS.iter().find(|s| s.id == id)
}

pub fn redirect_uri(spec: &SocialSpec) -> String {
    format!("http://127.0.0.1:{}/callback", spec.port)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/* ------------------------------------------------------------------ PKCE */

/// RFC 7636 S256 pair: 43-char base64url verifier, challenge = b64url(sha256(verifier)).
pub fn pkce_pair() -> (String, String) {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("system entropy unavailable");
    let verifier = B64URL.encode(buf);
    let challenge = B64URL.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn new_state() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("system entropy unavailable");
    B64URL.encode(buf)
}

/// Minimal percent-encoding for query parameters (RFC 3986 unreserved stays).
fn qp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn build_auth_url(spec: &SocialSpec, client_id: &str, state: &str, challenge: &str) -> String {
    let redirect = qp(&redirect_uri(spec));
    let base = match spec.id {
        // X uses PKCE and does not take scope at token-exchange time
        "x" => format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope={}",
            spec.auth_url,
            qp(client_id),
            redirect,
            qp(state),
            qp(challenge),
            qp(spec.scope)
        ),
        _ => format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}&scope={}",
            spec.auth_url,
            qp(client_id),
            redirect,
            qp(state),
            qp(spec.scope)
        ),
    };
    base
}

/// Parse an OAuth redirect target ("/callback?code=..&state=.."). Platform
/// error responses (error=access_denied&...) surface as a readable message.
pub fn parse_callback(target: &str) -> Result<(String, String), String> {
    let q = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "code" => code = Some(v.replace("%2F", "/")),
            "state" => state = Some(v.to_string()),
            "error" => error = Some(v.to_string()),
            _ => {}
        }
    }
    if let Some(e) = error {
        return Err(match e.as_str() {
            "access_denied" => "用户在浏览器里取消了授权 / user denied the authorization".into(),
            other => format!("授权失败 / authorization error: {other}"),
        });
    }
    match (code, state) {
        (Some(c), Some(s)) => Ok((c, s)),
        _ => Err("回调缺少 code/state / callback missing code or state".into()),
    }
}

/* ------------------------------------------------------------- storage */

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SocialBinding {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// unix seconds; 0 = unknown / not applicable
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub updated_at: u64,
}

fn fallback_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wxwright")
        .join("social-bindings.json")
}

fn keyring_entry(platform: &str) -> Option<keyring::Entry> {
    keyring::Entry::new("wxwright", &format!("social-{platform}")).ok()
}

fn read_file_map(path: &PathBuf) -> HashMap<String, SocialBinding> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_file_map(path: &PathBuf, map: &HashMap<String, SocialBinding>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

pub fn load_binding(platform: &str) -> Option<SocialBinding> {
    if let Some(entry) = keyring_entry(platform) {
        if let Ok(s) = entry.get_password() {
            if let Ok(b) = serde_json::from_str::<SocialBinding>(&s) {
                return Some(b);
            }
        }
    }
    read_file_map(&fallback_path()).get(platform).cloned()
}

pub fn save_binding(platform: &str, b: &SocialBinding) -> Result<(), String> {
    if let Some(entry) = keyring_entry(platform) {
        let s = serde_json::to_string(b).map_err(|e| e.to_string())?;
        if entry.set_password(&s).is_ok() {
            return Ok(());
        }
    }
    // keyring unavailable: local-file fallback (outside any repo, gitignored dir)
    let mut map = read_file_map(&fallback_path());
    map.insert(platform.to_string(), b.clone());
    write_file_map(&fallback_path(), &map)
}

pub fn clear_binding(platform: &str) {
    if let Some(entry) = keyring_entry(platform) {
        let _ = entry.delete_credential();
    }
    let mut map = read_file_map(&fallback_path());
    if map.remove(platform).is_some() {
        let _ = write_file_map(&fallback_path(), &map);
    }
}

/* ------------------------------------------------------------ flow core */

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Bind a loopback listener and wait for the OAuth redirect. Non-callback
/// paths (favicon etc.) are answered and skipped; the callback must carry the
/// exact state we issued.
pub fn wait_callback(
    port: u16,
    expected_state: &str,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<String, String> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("端口 {port} 监听失败: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消登录 / login cancelled".into());
        }
        if Instant::now() > deadline {
            return Err("等待授权超时（5 分钟）/ timed out waiting for authorization".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if let Some(result) = handle_conn(&mut stream, expected_state) {
                    return result;
                }
                // not a callback path: answered and looped on
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.to_string()),
        }
        std::thread::sleep(Duration::from_millis(120));
    }
}

/// Serve one connection. Returns Some(result) when this was the callback.
fn handle_conn(stream: &mut TcpStream, expected_state: &str) -> Option<Result<String, String>> {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let target = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    let (status, html, result) = if target.starts_with("/callback") {
        match parse_callback(&target) {
            Ok((code, state)) if state == expected_state => (
                "200 OK",
                "<html><body style=\"font-family:system-ui;background:#f6f8fa;color:#1f2328;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h2>绑定成功</h2><p>已授权，请回到 wxwright 窗口。</p><p style=\"color:#57606a\">Authorized. You can close this tab and return to wxwright.</p></div></body></html>".to_string(),
                Some(Ok(code)),
            ),
            Ok(_) => (
                "400 Bad Request",
                "<html><body style=\"font-family:system-ui\"><h2>state 校验失败</h2><p>State mismatch - please restart the login from wxwright.</p></body></html>".to_string(),
                Some(Err("state 校验失败，请重新发起登录 / state mismatch, restart the login".into())),
            ),
            Err(e) => (
                "400 Bad Request",
                format!("<html><body style=\"font-family:system-ui\"><h2>授权未完成</h2><p>{}</p></body></html>", html_escape(&e)),
                Some(Err(e)),
            ),
        }
    } else {
        ("404 Not Found", "<html><body></body></html>".to_string(), None)
    };
    let body = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn exchange_code(
    spec: &SocialSpec,
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
) -> Result<Value, String> {
    let redirect = redirect_uri(spec);
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &redirect),
        ("client_id", client_id),
    ];
    if !client_secret.is_empty() {
        form.push(("client_secret", client_secret));
    }
    if spec.id == "x" {
        form.push(("code_verifier", verifier));
    }
    let resp = ureq::post(spec.token_url)
        .timeout(Duration::from_secs(30))
        .send_form(&form)
        .map_err(|e| format!("令牌交换失败 / token exchange failed: {e}"))?;
    resp.into_json::<Value>()
        .map_err(|e| format!("令牌响应解析失败 / bad token response: {e}"))
}

/* -------------------------------------------------------------- commands */

fn status_of(platform: &str) -> Value {
    let spec = spec(platform);
    let b = load_binding(platform);
    let configured = b.as_ref().map(|b| !b.client_id.is_empty()).unwrap_or(false);
    let bound = b.as_ref().map(|b| !b.access_token.is_empty()).unwrap_or(false);
    json!({
        "platform": platform,
        "name_zh": spec.map(|s| s.name_zh).unwrap_or(platform),
        "name_en": spec.map(|s| s.name_en).unwrap_or(platform),
        "needs_secret": spec.map(|s| s.needs_secret).unwrap_or(false),
        "flow_ready": spec.map(|s| s.flow_ready).unwrap_or(false),
        "port": spec.map(|s| s.port).unwrap_or(0),
        "redirect_uri": spec.map(redirect_uri).unwrap_or_default(),
        // public identifier, safe to render back into the form; the secret
        // and tokens are never included in any status payload
        "client_id": b.as_ref().map(|b| b.client_id.clone()).unwrap_or_default(),
        "configured": configured,
        "bound": bound,
        "expires_at": b.as_ref().map(|b| b.expires_at).unwrap_or(0),
        "updated_at": b.as_ref().map(|b| b.updated_at).unwrap_or(0),
    })
}

#[tauri::command]
pub fn social_bind_status() -> Value {
    Value::Array(SPECS.iter().map(|s| status_of(s.id)).collect())
}

#[tauri::command]
pub fn social_save_config(
    platform: String,
    client_id: String,
    client_secret: String,
) -> Result<Value, String> {
    let spec = spec(&platform).ok_or("unknown platform")?;
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Client ID 不能为空 / client id is required".into());
    }
    if spec.needs_secret && client_secret.trim().is_empty() {
        return Err("该平台需要 Client Secret / this platform requires a client secret".into());
    }
    let mut b = load_binding(&platform).unwrap_or_default();
    b.client_id = client_id;
    b.client_secret = client_secret.trim().to_string();
    b.updated_at = now_secs();
    save_binding(&platform, &b)?;
    Ok(status_of(&platform))
}

#[tauri::command]
pub fn social_unbind(platform: String) -> Result<Value, String> {
    spec(&platform).ok_or("unknown platform")?;
    clear_binding(&platform);
    Ok(status_of(&platform))
}

#[tauri::command]
pub async fn social_oauth_start<R: Runtime>(app: AppHandle<R>, platform: String) -> Result<Value, String> {
    let spec = *spec(&platform).ok_or("unknown platform")?;
    if !spec.flow_ready {
        return Err(format!(
            "{} 的一键登录尚未开放（见配置引导页说明）/ one-click login for {} is not available yet",
            spec.name_en, spec.name_en
        ));
    }
    let binding = load_binding(&platform).ok_or("请先填写 Client ID 并保存 / save a client id first")?;
    if binding.client_id.is_empty() {
        return Err("请先填写 Client ID 并保存 / save a client id first".into());
    }
    let (verifier, challenge) = pkce_pair();
    let state = new_state();
    let url = build_auth_url(&spec, &binding.client_id, &state, &challenge);
    CANCEL.store(false, Ordering::Relaxed);
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("无法打开浏览器 / cannot open browser: {e}"))?;
    let code = tauri::async_runtime::spawn_blocking(move || {
        wait_callback(spec.port, &state, &CANCEL, Duration::from_secs(300))
    })
    .await
    .map_err(|e| e.to_string())??;
    let binding2 = binding.clone();
    let token = tauri::async_runtime::spawn_blocking(move || {
        exchange_code(&spec, &binding2.client_id, &binding2.client_secret, &code, &verifier)
    })
    .await
    .map_err(|e| e.to_string())??;
    let expires_in = token["expires_in"].as_u64().unwrap_or(0);
    let mut out = binding.clone();
    out.access_token = token["access_token"].as_str().unwrap_or("").to_string();
    out.refresh_token = token["refresh_token"].as_str().unwrap_or("").to_string();
    out.expires_at = if expires_in > 0 { now_secs() + expires_in } else { 0 };
    out.scope = token["scope"].as_str().unwrap_or(spec.scope).to_string();
    out.updated_at = now_secs();
    if out.access_token.is_empty() {
        return Err("平台返回的令牌为空 / platform returned an empty token".into());
    }
    save_binding(&platform, &out)?;
    Ok(status_of(&platform))
}

#[tauri::command]
pub fn social_oauth_cancel() -> Result<(), String> {
    CANCEL.store(true, Ordering::Relaxed);
    Ok(())
}

/* ----------------------------------------------------------------- tests */

#[cfg(test)]
mod tests {
    use super::*;

    fn load_binding_from(platform: &str, path: &PathBuf) -> Option<SocialBinding> {
        read_file_map(path).get(platform).cloned()
    }

    fn save_binding_to(platform: &str, b: &SocialBinding, path: &PathBuf) -> Result<(), String> {
        let mut map = read_file_map(path);
        map.insert(platform.to_string(), b.clone());
        write_file_map(path, &map)
    }

    #[test]
    fn pkce_pair_is_rfc7636_s256() {
        let (verifier, challenge) = pkce_pair();
        assert_eq!(verifier.len(), 43, "b64url of 32 bytes is 43 chars");
        assert!(
            verifier.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "verifier is base64url without padding"
        );
        let expected = B64URL.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
    }

    #[test]
    fn auth_urls_carry_all_contract_params() {
        let x = spec("x").unwrap();
        let url = build_auth_url(x, "cid-1", "st-1", "ch-1");
        assert!(url.starts_with("https://x.com/i/oauth2/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=cid-1"));
        assert!(url.contains(&format!("redirect_uri={}", qp(&redirect_uri(x)))));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("scope=tweet.read%20tweet.write%20users.read%20offline.access"));
        assert!(url.contains("state=st-1"));

        let li = spec("linkedin").unwrap();
        let url = build_auth_url(li, "cid-2", "st-2", "unused");
        assert!(url.starts_with("https://www.linkedin.com/oauth/v2/authorization?"));
        assert!(url.contains("scope=openid%20profile%20w_member_social"));
        assert!(!url.contains("code_challenge"), "no PKCE for plain code flow");
    }

    #[test]
    fn callback_parsing() {
        let (code, state) = parse_callback("/callback?code=abc&state=st1").unwrap();
        assert_eq!((code.as_str(), state.as_str()), ("abc", "st1"));
        let err = parse_callback("/callback?error=access_denied&error_description=no").unwrap_err();
        assert!(err.contains("取消") || err.contains("denied"));
        assert!(parse_callback("/callback?state=only").is_err());
    }

    #[test]
    fn loopback_listener_serves_and_extracts_code() {
        use std::sync::Arc;
        let port = 18761u16; // test-only port, never a real binding port
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = Arc::clone(&cancel);
        let waiter = std::thread::spawn(move || {
            wait_callback(port, "st-ok", &c2, Duration::from_secs(10))
        });
        // give the listener a moment to bind
        std::thread::sleep(Duration::from_millis(300));
        let mut c1 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        // a stray favicon request must be skipped, not treated as the callback
        c1.write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut buf = [0u8; 512];
        let _ = c1.read(&mut buf);
        drop(c1);
        let mut c2 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c2.write_all(b"GET /callback?code=the-code&state=st-ok HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        let _ = c2.read_to_string(&mut resp);
        assert!(resp.contains("200 OK"), "callback gets a success page");
        assert!(resp.contains("绑定成功"));
        let code = waiter.join().unwrap().expect("callback extracted");
        assert_eq!(code, "the-code");
    }

    #[test]
    fn binding_storage_roundtrip_in_fallback_file() {
        let dir = std::env::temp_dir().join(format!("wxwright-social-test-{}", std::process::id()));
        let path = dir.join("social-bindings.json");
        let b = SocialBinding {
            client_id: "cid".into(),
            client_secret: "sec".into(),
            access_token: "tok".into(),
            expires_at: 12345,
            updated_at: 1,
            ..Default::default()
        };
        save_binding_to("x", &b, &path).unwrap();
        let loaded = load_binding_from("x", &path).expect("roundtrip");
        assert_eq!(loaded.client_id, "cid");
        assert_eq!(loaded.expires_at, 12345);
        assert!(load_binding_from("linkedin", &path).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn status_never_leaks_tokens() {
        let s = status_of("x");
        let s = serde_json::to_string(&s).unwrap();
        assert!(!s.contains("access_token"));
        assert!(!s.contains("client_secret"));
    }
}
