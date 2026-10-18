//! wxwright-mp: WeChat MP platform API adapter (PRD 3.2 适配器层).
//! Implements wxwright-core's `ImageTransport` with the permanent material
//! upload endpoint, and provides draft box / freepublish calls.
//!
//! Security (PRD 5.6): secrets live in the OS keychain (config file only
//! holds a reference); access_token is never logged (always masked).

use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use wxwright_core::error::{Error, Result};
use wxwright_core::img::ImageTransport;

const API_BASE: &str = "https://api.weixin.qq.com";

// ------------------------------------------------------------------ creds ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub appid: String,
    pub secret: String,
}

/// One row of the `[mp.accounts]` config table. Exactly one of `secret_ref`
/// (keyring pointer) or `secret` (no-keyring fallback) is populated.
/// `nickname` / `avatar` (data URI) come from the account-info endpoint and
/// are display-only enrichment - binding works without them.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountEntry {
    appid: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    nickname: String,
    #[serde(default)]
    avatar: String,
    #[serde(default)]
    secret_ref: String,
    #[serde(default)]
    secret: String,
}

/// Display-safe projection for status surfaces: never carries secrets.
#[derive(Debug, Clone, Serialize)]
pub struct AccountInfo {
    pub appid: String,
    pub label: String,
    pub nickname: String,
    pub avatar: String,
    pub active: bool,
}

pub fn config_path() -> std::path::PathBuf {
    wxwright_core::util::config_root().join("config.toml")
}

fn keyring_entry() -> Option<keyring::Entry> {
    keyring::Entry::new("wxwright", "mp-secret").ok()
}

/// Per-account keyring slot: service "wxwright", user "mp-secret:<appid>".
fn keyring_entry_for(appid: &str) -> Option<keyring::Entry> {
    keyring::Entry::new("wxwright", &format!("mp-secret:{}", appid)).ok()
}

fn read_config() -> toml::Value {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|raw| toml::from_str(&raw).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()))
}

/// All configured accounts. Legacy single-account configs (`[mp] appid` +
/// `secret_ref = "keyring"`) migrate on read: they become one entry pointing
/// at the same keyring slot, so the existing secret is never moved or copied.
fn read_accounts(cfg: &toml::Value) -> Vec<AccountEntry> {
    let mp = cfg.get("mp");
    if let Some(list) = mp
        .and_then(|m| m.get("accounts"))
        .and_then(|a| a.as_array())
    {
        return list
            .iter()
            .filter_map(|v| AccountEntry::deserialize(v.clone()).ok())
            .filter(|a| !a.appid.is_empty())
            .collect();
    }
    let mp = match mp {
        Some(m) => m,
        None => return Vec::new(),
    };
    let appid = match mp.get("appid").and_then(|x| x.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return Vec::new(),
    };
    let secret_ref = mp
        .get("secret_ref")
        .and_then(|x| x.as_str())
        .unwrap_or("keyring")
        .to_string();
    let secret = mp
        .get("secret")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    vec![AccountEntry {
        appid,
        label: String::new(),
        nickname: String::new(),
        avatar: String::new(),
        secret_ref,
        secret,
    }]
}

fn read_active(cfg: &toml::Value, accounts: &[AccountEntry]) -> Option<String> {
    if let Some(a) = cfg
        .get("mp")
        .and_then(|m| m.get("active"))
        .and_then(|x| x.as_str())
    {
        if accounts.iter().any(|e| e.appid == a) {
            return Some(a.to_string());
        }
    }
    accounts.first().map(|e| e.appid.clone())
}

fn account_secret(entry: &AccountEntry) -> Option<String> {
    if !entry.secret.is_empty() {
        return Some(entry.secret.clone());
    }
    let user = entry
        .secret_ref
        .strip_prefix("keyring:")
        .unwrap_or("mp-secret");
    keyring::Entry::new("wxwright", user)
        .ok()?
        .get_password()
        .ok()
}

/// Rewrite the `[mp]` table, preserving every other section of config.toml.
fn persist_accounts(cfg: &mut toml::Value, accounts: &[AccountEntry], active: &str) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    if !cfg.is_table() {
        *cfg = toml::Value::Table(toml::map::Map::new());
    }
    let list: Vec<toml::Value> = accounts
        .iter()
        .map(|a| toml::Value::try_from(a).map_err(|e| Error::Upload(e.to_string())))
        .collect::<Result<_>>()?;
    let mut mp = toml::map::Map::new();
    mp.insert("active".into(), toml::Value::String(active.to_string()));
    mp.insert("accounts".into(), toml::Value::Array(list));
    cfg.as_table_mut()
        .unwrap()
        .insert("mp".into(), toml::Value::Table(mp));
    let body = toml::to_string_pretty(cfg).map_err(|e| Error::Upload(e.to_string()))?;
    // Owner-only permissions on Unix (the no-keyring fallback stores secrets
    // in this file; the keyring path stores only references).
    wxwright_core::util::write_private(&path, &body).map_err(Error::Io)?;
    Ok(())
}

/// Load the ACTIVE account's credentials: env overrides > config+keychain.
/// Every consumer (CLI, MCP, GUI push/upload/drafts) follows the active
/// account automatically - switching is a config write, zero code paths.
pub fn load_credentials() -> Option<Credentials> {
    if let (Ok(appid), Ok(secret)) = (
        std::env::var("WXWRIGHT_MP_APPID"),
        std::env::var("WXWRIGHT_MP_SECRET"),
    ) {
        if !appid.is_empty() && !secret.is_empty() {
            return Some(Credentials { appid, secret });
        }
    }
    let cfg = read_config();
    let accounts = read_accounts(&cfg);
    let active = read_active(&cfg, &accounts)?;
    let entry = accounts.iter().find(|e| e.appid == active)?;
    let secret = account_secret(entry)?;
    if secret.is_empty() {
        return None;
    }
    Some(Credentials {
        appid: entry.appid.clone(),
        secret,
    })
}

/// Display-safe account list for status surfaces.
pub fn list_accounts() -> Vec<AccountInfo> {
    let cfg = read_config();
    let accounts = read_accounts(&cfg);
    let active = read_active(&cfg, &accounts);
    accounts
        .into_iter()
        .map(|e| AccountInfo {
            active: active.as_deref() == Some(e.appid.as_str()),
            appid: e.appid,
            label: e.label,
            nickname: e.nickname,
            avatar: e.avatar,
        })
        .collect()
}

/// Add or update one account and make it the active one. The secret lands in
/// the OS keychain under `mp-secret:<appid>`; the config file only holds a
/// reference (PRD 5.6). `use_keyring = false` keeps the documented file
/// fallback for headless/CI setups.
pub fn upsert_account(creds: &Credentials, label: &str, use_keyring: bool) -> Result<()> {
    let appid = creds.appid.trim().to_string();
    let secret = creds.secret.trim().to_string();
    if appid.is_empty() || secret.is_empty() {
        return Err(Error::Upload("appid and secret are both required".into()));
    }
    let mut secret_field = String::new();
    let secret_ref = if use_keyring {
        let entry = keyring_entry_for(&appid)
            .ok_or_else(|| Error::Upload("OS keychain unavailable".into()))?;
        entry
            .set_password(&secret)
            .map_err(|e| Error::Upload(format!("keychain write failed: {}", e)))?;
        format!("keyring:mp-secret:{}", appid)
    } else {
        secret_field = secret;
        String::new()
    };
    let mut cfg = read_config();
    let mut accounts = read_accounts(&cfg);
    let label = label.trim().to_string();
    if let Some(e) = accounts.iter_mut().find(|e| e.appid == appid) {
        e.label = label;
        e.secret_ref = secret_ref;
        e.secret = secret_field;
    } else {
        accounts.push(AccountEntry {
            appid: appid.clone(),
            label,
            nickname: String::new(),
            avatar: String::new(),
            secret_ref,
            secret: secret_field,
        });
    }
    persist_accounts(&mut cfg, &accounts, &appid)
}

/// Point the active account at another already-bound appid.
pub fn set_active_account(appid: &str) -> Result<()> {
    let appid = appid.trim();
    let mut cfg = read_config();
    let accounts = read_accounts(&cfg);
    if !accounts.iter().any(|e| e.appid == appid) {
        return Err(Error::Upload(format!("account {} is not bound", appid)));
    }
    persist_accounts(&mut cfg, &accounts, appid)
}

/// Remove one account (keyring slot included). If it was active, the first
/// remaining account becomes active.
pub fn remove_account(appid: &str) -> Result<()> {
    let appid = appid.trim();
    let mut cfg = read_config();
    let mut accounts = read_accounts(&cfg);
    let removed = match accounts.iter().position(|e| e.appid == appid) {
        Some(idx) => accounts.remove(idx),
        None => return Err(Error::Upload(format!("account {} is not bound", appid))),
    };
    if removed.secret_ref.starts_with("keyring:mp-secret:") {
        if let Some(entry) = keyring_entry_for(appid) {
            let _ = entry.delete_credential();
        }
    } else if removed.secret_ref == "keyring" {
        if let Some(entry) = keyring_entry() {
            let _ = entry.delete_credential();
        }
    }
    let next = accounts
        .first()
        .map(|e| e.appid.clone())
        .unwrap_or_default();
    persist_accounts(&mut cfg, &accounts, &next)
}

/// Store display-only enrichment (nickname + avatar data URI) for one account.
/// Never touches the secret or the active flag.
pub fn set_account_display(appid: &str, nickname: &str, avatar: &str) -> Result<()> {
    let appid = appid.trim();
    let mut cfg = read_config();
    let mut accounts = read_accounts(&cfg);
    let entry = accounts
        .iter_mut()
        .find(|e| e.appid == appid)
        .ok_or_else(|| Error::Upload(format!("account {} is not bound", appid)))?;
    entry.nickname = nickname.trim().to_string();
    entry.avatar = avatar.trim().to_string();
    let active = read_active(&cfg, &accounts).unwrap_or_default();
    persist_accounts(&mut cfg, &accounts, &active)
}

/// Credentials for ANY bound account (not just the active one) - used by the
/// account-info refresh so every bound account gets its nickname/avatar.
pub fn account_credentials(appid: &str) -> Result<Credentials> {
    let appid = appid.trim();
    let cfg = read_config();
    let accounts = read_accounts(&cfg);
    let entry = accounts
        .iter()
        .find(|e| e.appid == appid)
        .ok_or_else(|| Error::Upload(format!("account {} is not bound", appid)))?;
    let secret = account_secret(entry).unwrap_or_default();
    if secret.is_empty() {
        return Err(Error::Upload(format!(
            "account {} has no readable secret",
            appid
        )));
    }
    Ok(Credentials {
        appid: appid.to_string(),
        secret,
    })
}

/// Persist credentials. `use_keyring` stores the secret in the OS keychain
/// and only a reference in the config file (PRD 5.6); otherwise the secret
/// lands in the config file with 0600-ish discretion (documented risk).
/// Single-account entry point (CLI `wxwright login`): upserts + activates.
pub fn save_credentials(creds: &Credentials, use_keyring: bool) -> Result<()> {
    upsert_account(creds, "", use_keyring)
}

/// Unbind everything (CLI `wxwright logout` semantics).
pub fn clear_credentials() -> Result<()> {
    let mut cfg = read_config();
    for entry in read_accounts(&cfg) {
        if entry.secret_ref.starts_with("keyring:mp-secret:") {
            if let Some(e) = keyring_entry_for(&entry.appid) {
                let _ = e.delete_credential();
            }
        } else if entry.secret_ref == "keyring" {
            if let Some(e) = keyring_entry() {
                let _ = e.delete_credential();
            }
        }
    }
    if let Some(table) = cfg.as_table_mut() {
        table.remove("mp");
    }
    let path = config_path();
    if path.exists() {
        let body = toml::to_string_pretty(&cfg).map_err(|e| Error::Upload(e.to_string()))?;
        wxwright_core::util::write_private(&path, &body).map_err(Error::Io)?;
    }
    Ok(())
}

/// Mask a secret for logs (PRD 5.6: access_token always masked).
/// Mask a credential for display. The one implementation lives in
/// `wxwright_core::util`; this used to be a near-identical copy that disagreed
/// on the empty case.
pub fn mask(s: &str) -> String {
    wxwright_core::util::mask_secret(s)
}

// ------------------------------------------------------------------ client ---

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    errcode: Option<i64>,
    errmsg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenericResponse {
    errcode: Option<i64>,
    errmsg: Option<String>,
    #[serde(default)]
    media_id: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

struct TokenCache {
    token: String,
    expires_at: Instant,
}

pub struct MpClient {
    appid: String,
    secret: String,
    agent: ureq::Agent,
    cache: Mutex<Option<TokenCache>>,
}

/// Build the `multipart/form-data` body WeChat's material endpoint expects.
///
/// Pulled out of `upload_material` so it can be asserted on directly: the test
/// it replaces only checked that a hardcoded boundary string was alphanumeric,
/// which says nothing about the body that is actually sent.
fn multipart_body(boundary: &str, filename: &str, mime: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"media\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
            // A quote in the name would close the header early.
            filename.replace('"', ""),
            mime
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
    body
}

impl MpClient {
    pub fn new(creds: Credentials) -> Self {
        MpClient {
            appid: creds.appid,
            secret: creds.secret,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(60))
                .build(),
            cache: Mutex::new(None),
        }
    }

    pub fn appid(&self) -> &str {
        &self.appid
    }

    /// Fresh or cached access_token. Cached value is reused until 5 minutes
    /// before expiry; on token-expiry errors callers may force refresh.
    pub fn access_token(&self, force_refresh: bool) -> Result<String> {
        if !force_refresh {
            if let Ok(guard) = self.cache.lock() {
                if let Some(c) = guard.as_ref() {
                    if Instant::now() < c.expires_at {
                        return Ok(c.token.clone());
                    }
                }
            }
        }
        let url = format!(
            "{}/cgi-bin/token?grant_type=client_credential&appid={}&secret={}",
            API_BASE, self.appid, self.secret
        );
        let resp: TokenResponse = self
            .agent
            .get(&url)
            .call()
            .map_err(ureq_err)?
            .into_json()
            .map_err(|e| Error::Upload(format!("json: {}", e)))?;
        match resp.access_token {
            Some(tok) if !tok.is_empty() => {
                let ttl = resp.expires_in.unwrap_or(7200).saturating_sub(300);
                if let Ok(mut guard) = self.cache.lock() {
                    *guard = Some(TokenCache {
                        token: tok.clone(),
                        expires_at: Instant::now() + Duration::from_secs(ttl),
                    });
                }
                Ok(tok)
            }
            _ => Err(Error::Upload(format!(
                "token fetch failed: errcode={:?} errmsg={:?}",
                resp.errcode,
                resp.errmsg.unwrap_or_default()
            ))),
        }
    }

    /// Upload an image to the permanent material library.
    /// Returns (media_id, mmbiz_url).
    pub fn upload_material(&self, bytes: Vec<u8>, filename: &str) -> Result<(String, String)> {
        self.upload_material_retry(bytes, filename, 2)
    }

    fn upload_material_retry(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        attempts: u32,
    ) -> Result<(String, String)> {
        let mut last_err = String::new();
        for attempt in 0..attempts {
            if attempt > 0 {
                // Exponential backoff, capped: 500ms, 1s, 2s, 4s...
                let backoff = 500u64.saturating_mul(1u64 << attempt.min(3));
                std::thread::sleep(Duration::from_millis(backoff));
            }
            match self.upload_material_once(&bytes, filename) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = e.to_string(),
            }
        }
        Err(Error::Upload(last_err))
    }

    fn upload_material_once(&self, bytes: &[u8], filename: &str) -> Result<(String, String)> {
        let v = self.with_token_retry(|force| self.upload_material_post(bytes, filename, force))?;
        let resp: GenericResponse =
            serde_json::from_value(v).map_err(|e| Error::Upload(format!("decode: {}", e)))?;
        check(&resp)?;
        match (resp.media_id, resp.url) {
            (Some(id), Some(u)) => Ok((id, u)),
            _ => Err(Error::Upload(
                "material upload: missing media_id/url".into(),
            )),
        }
    }

    fn upload_material_post(
        &self,
        bytes: &[u8],
        filename: &str,
        force_refresh: bool,
    ) -> Result<serde_json::Value> {
        let token = self.access_token(force_refresh)?;
        let url = format!(
            "{}/cgi-bin/material/add_material?access_token={}&type=image",
            API_BASE, token
        );
        let boundary = "wxwright boundary 7d3a9c1f";
        let mime = mime_of_filename(filename);
        let body = multipart_body(boundary, filename, mime, bytes);

        self.agent
            .post(&url)
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={}", boundary),
            )
            .send_bytes(&body)
            .map_err(ureq_err)?
            .into_json()
            .map_err(|e| Error::Upload(format!("json: {}", e)))
    }

    /// Create a draft in the 草稿箱 (Drafts). `thumb_media_id` is required by
    /// the API; pass the media_id of one of the uploaded article images.
    pub fn draft_add(&self, article: &DraftArticle) -> Result<String> {
        let payload = serde_json::json!({
            "articles": [{
                "title": article.title,
                "author": article.author,
                "digest": article.digest,
                "content": article.content_html,
                "content_source_url": article.content_source_url,
                "thumb_media_id": article.thumb_media_id,
                "need_open_comment": 0,
                "only_fans_can_comment": 0,
            }]
        });
        let resp: GenericResponse = self.post_with_token("/cgi-bin/draft/add", &payload)?;
        check(&resp)?;
        resp.media_id
            .ok_or_else(|| Error::Upload("draft add: missing media_id".into()))
    }

    pub fn draft_list(&self, offset: u64, count: u64) -> Result<serde_json::Value> {
        let payload =
            serde_json::json!({ "offset": offset, "count": count.min(20), "no_content": 1 });
        // Official endpoint is batchget (/cgi-bin/draft/batch does not exist -
        // WeChat answered 40066 "invalid url", so this tool had never worked
        // against the live API).
        self.post_with_token_value("/cgi-bin/draft/batchget", &payload)
    }

    /// Account nickname + avatar URL, for display surfaces (account switcher).
    /// Subject to the same IP whitelist as every other API call; callers must
    /// tolerate failure (the appid fallback stays).
    pub fn account_basic_info(&self) -> Result<(String, String)> {
        let token = self.access_token(false)?;
        let url = format!(
            "{}/cgi-bin/account/getaccountbasicinfo?access_token={}",
            API_BASE, token
        );
        let v: serde_json::Value = self
            .agent
            .get(&url)
            .call()
            .map_err(ureq_err)?
            .into_json()
            .map_err(|e| Error::Upload(format!("json: {}", e)))?;
        let gr: GenericResponse =
            serde_json::from_value(v.clone()).map_err(|e| Error::Upload(e.to_string()))?;
        check(&gr)?;
        let nickname = v
            .get("nickname")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        // The avatar lives at head_image_info.head_image_url (getAccountBasicInfo);
        // tolerate the flat head_img spelling older docs used.
        let head_img = v
            .pointer("/head_image_info/head_image_url")
            .or_else(|| v.get("head_img"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        Ok((nickname, head_img))
    }

    /// GET an arbitrary (WeChat CDN) URL and return the body bytes - used to
    /// download account avatars so they display offline as data URIs.
    pub fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.agent
            .get(url)
            .call()
            .map_err(ureq_err)?
            .into_reader()
            .take(5 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| Error::Upload(format!("avatar download: {}", e)))?;
        Ok(buf)
    }

    /// Delete one draft by media_id. The complements of create: without it,
    /// a mistaken push can only be cleaned up by hand in the web console.
    pub fn draft_delete(&self, media_id: &str) -> Result<()> {
        let payload = serde_json::json!({ "media_id": media_id });
        let resp: GenericResponse = self.post_with_token("/cgi-bin/draft/delete", &payload)?;
        check(&resp)
    }

    pub fn draft_update(&self, media_id: &str, index: u64, article: &DraftArticle) -> Result<()> {
        let payload = serde_json::json!({
            "media_id": media_id,
            "index": index,
            "articles": {
                "title": article.title,
                "author": article.author,
                "digest": article.digest,
                "content": article.content_html,
                "content_source_url": article.content_source_url,
                "thumb_media_id": article.thumb_media_id,
                "need_open_comment": 0,
                "only_fans_can_comment": 0,
            }
        });
        let resp: GenericResponse = self.post_with_token("/cgi-bin/draft/update", &payload)?;
        check(&resp)?;
        Ok(())
    }

    /// 群发 (mass send) via freepublish. Caller must gate on explicit --yes.
    pub fn freepublish_submit(&self, draft_media_id: &str) -> Result<serde_json::Value> {
        let payload = serde_json::json!({ "media_id": draft_media_id });
        self.post_with_token_value("/cgi-bin/freepublish/submit", &payload)
    }

    /// POST a JSON payload with the cached token. `force_refresh` bypasses the
    /// cache (used by the retry path). Returns the raw body so callers decode
    /// into the shape they need.
    fn post_json(
        &self,
        path: &str,
        payload: &serde_json::Value,
        force_refresh: bool,
    ) -> Result<serde_json::Value> {
        let token = self.access_token(force_refresh)?;
        let url = format!("{}{}?access_token={}", API_BASE, path, token);
        self.agent
            .post(&url)
            .send_json(payload.clone())
            .map_err(ureq_err)?
            .into_json()
            .map_err(|e| Error::Upload(format!("json: {}", e)))
    }

    /// Run a token-authenticated request, transparently refreshing the token
    /// once when WeChat reports it as stale. Every authenticated call goes
    /// through here - previously only `post_with_token_value` retried, so a
    /// long session (batch upload, then draft create) died on token expiry.
    fn with_token_retry<F>(&self, call: F) -> Result<serde_json::Value>
    where
        F: Fn(bool) -> Result<serde_json::Value>,
    {
        let resp = call(false)?;
        if is_token_expired(&resp) {
            return call(true);
        }
        Ok(resp)
    }

    fn post_with_token(&self, path: &str, payload: &serde_json::Value) -> Result<GenericResponse> {
        let v = self.with_token_retry(|force| self.post_json(path, payload, force))?;
        serde_json::from_value(v).map_err(|e| Error::Upload(format!("decode: {}", e)))
    }

    fn post_with_token_value(
        &self,
        path: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.with_token_retry(|force| self.post_json(path, payload, force))
    }
}

#[derive(Debug, Clone, Default)]
pub struct DraftArticle {
    pub title: String,
    pub author: String,
    pub digest: String,
    pub content_html: String,
    pub content_source_url: String,
    pub thumb_media_id: String,
}

impl ImageTransport for MpClient {
    fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.agent.get(url).call().map_err(ureq_err)?;
        let mut buf = Vec::new();
        resp.into_reader()
            .take(64 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| Error::Upload(e.to_string()))?;
        Ok(buf)
    }

    fn upload_material(&self, bytes: Vec<u8>, filename: &str) -> Result<(String, String)> {
        MpClient::upload_material(self, bytes, filename)
    }
}

fn check(resp: &GenericResponse) -> Result<()> {
    match resp.errcode {
        Some(code) if code != 0 => Err(Error::Upload(format!(
            "MP API error {}: {}",
            code,
            resp.errmsg.clone().unwrap_or_default()
        ))),
        _ => Ok(()),
    }
}

/// WeChat error codes that mean "the access_token is stale": 40001 invalid
/// credential, 40014 illegal token, 42001 token expired. Every call site
/// funnels through `with_token_retry` so a long-running session (a batch of
/// image uploads, then a draft create) cannot die half way through.
fn is_token_expired(v: &serde_json::Value) -> bool {
    matches!(
        v.get("errcode").and_then(|c| c.as_i64()),
        Some(40001) | Some(40014) | Some(42001)
    )
}

fn ureq_err(e: ureq::Error) -> Error {
    // ureq renders status errors as "{full_url}: status code {code}", and the
    // full URL carries the AppSecret / access_token as query parameters.
    // Redact before the message can reach stdout, a --json payload or a toast
    // (PRD 5.6: secrets are never logged).
    Error::Upload(format!(
        "http: {}",
        wxwright_core::util::redact_secrets(&e.to_string())
    ))
}

fn mime_of_filename(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "application/octet-stream"
    }
}

/// Network reachability probe for `wxwright doctor` (no credentials needed).
/// Any HTTP response counts as reachable - the endpoint answers 4xx to
/// parameterless requests, which still proves the network path works.
pub fn probe_reachable() -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build();
    match agent.get(&format!("{}/cgi-bin/token", API_BASE)).call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Redirect config_root() at a throwaway dir for the duration of one test.
    /// config_root() reads APPDATA on Windows but XDG_CONFIG_HOME / HOME on
    /// Unix - all three must be pointed at the sandbox or CI Linux/macOS
    /// resolves the real home dir (whose wxwright/ subdir does not exist).
    /// These are process-global: the lock keeps config tests serial.
    struct ConfigIsolation {
        _guard: std::sync::MutexGuard<'static, ()>,
        prev: Vec<(String, Option<std::ffi::OsString>)>,
        dir: std::path::PathBuf,
    }
    impl ConfigIsolation {
        fn new(tag: &str) -> Self {
            static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let dir = std::env::temp_dir().join(format!(
                "wxwright-mp-test-{}-{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("wxwright")).unwrap();
            let mut prev: Vec<(String, Option<std::ffi::OsString>)> = Vec::new();
            for key in ["APPDATA", "XDG_CONFIG_HOME", "HOME"] {
                prev.push((key.to_string(), std::env::var_os(key)));
                std::env::set_var(key, &dir);
            }
            Self {
                _guard: guard,
                prev,
                dir,
            }
        }
    }
    impl Drop for ConfigIsolation {
        fn drop(&mut self) {
            for (key, val) in &self.prev {
                match val {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn multi_account_bind_switch_remove_with_legacy_migration() {
        let _iso = ConfigIsolation::new("multi");
        // Legacy single-account config: appid + plain secret (no keyring touch).
        std::fs::write(
            config_path(),
            "[mp]\nappid = \"wxold1111\"\nsecret = \"s1\"\n",
        )
        .unwrap();

        // Migration on read: the old pair is one active account.
        let accs = list_accounts();
        assert_eq!(accs.len(), 1, "legacy config migrates to one account");
        assert_eq!(accs[0].appid, "wxold1111");
        assert!(accs[0].active, "the single legacy account is active");
        assert_eq!(
            load_credentials().unwrap().secret,
            "s1",
            "legacy plain secret resolves"
        );

        // Bind a second account (no-keyring fallback) - it becomes active.
        upsert_account(
            &Credentials {
                appid: "wxnew2222".into(),
                secret: "s2".into(),
            },
            "测试号",
            false,
        )
        .unwrap();
        let accs = list_accounts();
        assert_eq!(accs.len(), 2);
        let active = accs.iter().find(|a| a.active).unwrap();
        assert_eq!(active.appid, "wxnew2222");
        assert_eq!(active.label, "测试号");
        let creds = load_credentials().unwrap();
        assert_eq!(creds.appid, "wxnew2222");
        assert_eq!(creds.secret, "s2");

        // Switch back to the migrated legacy account.
        set_active_account("wxold1111").unwrap();
        assert_eq!(load_credentials().unwrap().appid, "wxold1111");

        // Re-binding with a new secret updates in place (no duplicate row).
        upsert_account(
            &Credentials {
                appid: "wxold1111".into(),
                secret: "s1b".into(),
            },
            "老号",
            false,
        )
        .unwrap();
        let accs = list_accounts();
        assert_eq!(accs.len(), 2, "upsert must not duplicate");
        assert_eq!(load_credentials().unwrap().secret, "s1b");
        let old = accs.iter().find(|a| a.appid == "wxold1111").unwrap();
        assert_eq!(old.label, "老号");

        // Remove the active one: the other account takes over.
        remove_account("wxold1111").unwrap();
        let accs = list_accounts();
        assert_eq!(accs.len(), 1);
        assert_eq!(accs[0].appid, "wxnew2222");
        assert!(accs[0].active);
        assert_eq!(load_credentials().unwrap().appid, "wxnew2222");

        // Removing the last account leaves nothing bound.
        remove_account("wxnew2222").unwrap();
        assert!(list_accounts().is_empty());
        assert!(load_credentials().is_none());

        // Switching to a non-existent account is a hard error, never a silent
        // fallthrough to some other account.
        assert!(set_active_account("wxghost").is_err());
    }

    #[test]
    fn other_config_sections_survive_account_writes() {
        let _iso = ConfigIsolation::new("preserve");
        std::fs::write(
            config_path(),
            "[mp]\nappid = \"wxkeep\"\nsecret = \"s\"\n\n[future]\nkey = \"v\"\n",
        )
        .unwrap();
        upsert_account(
            &Credentials {
                appid: "wxkeep".into(),
                secret: "s2".into(),
            },
            "",
            false,
        )
        .unwrap();
        let raw = std::fs::read_to_string(config_path()).unwrap();
        assert!(raw.contains("[future]"), "foreign sections survive: {raw}");
        assert!(raw.contains("key = \"v\""));
        assert!(raw.contains("[[mp.accounts]]"), "new format present: {raw}");
    }

    #[test]
    fn mask_hides_secret() {
        assert_eq!(mask("short"), "***");
        let m = mask("abcdefghij12345678");
        assert!(m.starts_with("abcd"));
        assert!(m.ends_with("5678"));
        assert!(m.contains("****"));
        assert!(!m.contains("abcdefghij12345678"));
    }

    #[test]
    fn mime_detection() {
        assert_eq!(mime_of_filename("a.PNG"), "image/png");
        assert_eq!(mime_of_filename("b.gif"), "image/gif");
        assert_eq!(mime_of_filename("c.weird"), "application/octet-stream");
    }

    #[test]
    fn multipart_body_shape() {
        // Replaces a test that only checked the boundary string was
        // alphanumeric - it never looked at the body at all.
        let boundary = "wxwright boundary 7d3a9c1f";
        let payload: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x00, 0xFF];
        let body = multipart_body(boundary, "shot.png", "image/png", payload);
        let text = String::from_utf8_lossy(&body);

        // Opens with the boundary, closes with the terminator.
        assert!(
            text.starts_with(&format!("--{}\r\n", boundary)),
            "must open with the boundary: {text:?}"
        );
        assert!(
            text.ends_with(&format!("\r\n--{}--\r\n", boundary)),
            "must close with the terminator: {text:?}"
        );
        // The part headers name the field WeChat expects and carry the MIME.
        assert!(text.contains("name=\"media\""), "got: {text:?}");
        assert!(text.contains("filename=\"shot.png\""), "got: {text:?}");
        assert!(text.contains("Content-Type: image/png"), "got: {text:?}");
        assert!(text.contains("\r\n\r\n"), "headers end with a blank line");
        // The payload must survive byte-for-byte, including non-UTF-8 bytes.
        let start = body
            .windows(payload.len())
            .position(|w| w == payload)
            .expect("the payload bytes must appear verbatim in the body");
        assert!(start > 0, "the payload must come after the headers");
        // A quote in the file name must not be able to close the header early.
        let evil = multipart_body(boundary, "a\"b.png", "image/png", payload);
        let evil_text = String::from_utf8_lossy(&evil);
        assert!(
            !evil_text.contains("filename=\"a\"b.png\""),
            "an embedded quote must be stripped: {evil_text:?}"
        );
    }

    #[test]
    fn token_expiry_codes_are_recognised() {
        for code in [40001, 40014, 42001] {
            assert!(
                is_token_expired(&serde_json::json!({ "errcode": code })),
                "errcode {code} must trigger a refresh"
            );
        }
        assert!(!is_token_expired(&serde_json::json!({ "errcode": 0 })));
        assert!(!is_token_expired(&serde_json::json!({ "errcode": 48001 })));
        assert!(!is_token_expired(&serde_json::json!({ "media_id": "x" })));
    }

    #[test]
    fn http_errors_never_leak_the_app_secret() {
        // Reproduce the exact string ureq produces for a status error, then
        // run it through the same redaction the client applies.
        let raw = format!(
            "https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid=wx0000&secret={}: status code 401",
            "THE-ACTUAL-APP-SECRET"
        );
        let redacted = wxwright_core::util::redact_secrets(&raw);
        assert!(
            !redacted.contains("THE-ACTUAL-APP-SECRET"),
            "the AppSecret must never survive redaction: {redacted}"
        );
        assert!(redacted.contains("status code 401"), "keep the diagnosis");
    }

    #[test]
    fn config_path_is_inside_the_user_config_dir() {
        let p = config_path();
        assert!(p.to_string_lossy().contains("wxwright"));
        assert!(p.to_string_lossy().ends_with("config.toml"));
    }
}
