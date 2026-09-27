//! AI provider layer: OpenAI-compatible chat completions (covers OpenAI,
//! DeepSeek, Qwen, Kimi, GLM, and local Ollama / LM Studio via /v1).
//! Lives in the GUI host on purpose: wxwright-core keeps zero network
//! assumptions.
//!
//! Keys go to the OS keychain (service `wxwright`, user `ai-key-<id>`);
//! the settings file stores provider metadata only. If the keychain is
//! unavailable the key falls back into the settings file (flagged).

use std::io::{BufRead, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use wxwright_core::{pipeline, theme, ConvertOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    /// Present only when the keychain is unavailable (plain fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional custom brand icon as a data URI (overrides the built-in mark).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// Official logo-library key (openai/deepseek/qwen/kimi/...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub key_in_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiSettings {
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub active: Option<String>,
}

fn settings_path() -> std::path::PathBuf {
    wxwright_core::util::config_root().join("settings.json")
}

fn load_settings() -> AiSettings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_settings(s: &AiSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    wxwright_core::util::write_private(&path, &body).map_err(|e| e.to_string())
}

fn key_entry(id: &str) -> Option<keyring::Entry> {
    keyring::Entry::new("wxwright", &format!("ai-key-{}", id)).ok()
}

fn key_set(id: &str, key: &str) -> bool {
    key_entry(id)
        .map(|e| e.set_password(key).is_ok())
        .unwrap_or(false)
}

fn key_get(id: &str) -> Option<String> {
    key_entry(id).and_then(|e| e.get_password().ok())
}

fn key_del(id: &str) {
    if let Some(e) = key_entry(id) {
        let _ = e.delete_credential();
    }
}

/// Settings for the UI: keys masked to a hint string.
pub fn settings() -> serde_json::Value {
    let s = load_settings();
    let providers: Vec<serde_json::Value> = s
        .providers
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id, "name": p.name, "base_url": p.base_url, "model": p.model,
                "key_hint": mask(&effective_key(p).unwrap_or_default()),
                "key_in_file": p.key_in_file,
                "logo": p.logo,
                "brand": p.brand,
            })
        })
        .collect();
    serde_json::json!({ "providers": providers, "active": s.active })
}

fn mask(s: &str) -> String {
    let n = s.chars().count();
    if n == 0 {
        return String::new();
    }
    if n <= 8 {
        return "***".into();
    }
    let head: String = s.chars().take(4).collect();
    let tail: String = s.chars().skip(n - 4).collect();
    format!("{}****{}", head, tail)
}

fn effective_key(p: &Provider) -> Option<String> {
    if p.key_in_file {
        return p.api_key.clone();
    }
    key_get(&p.id).or_else(|| p.api_key.clone())
}

/// Create or update a provider. `api_key` empty = keep existing key.
pub fn save_provider(mut p: Provider, api_key: &str) -> Result<serde_json::Value, String> {
    if p.id.is_empty() {
        p.id = format!("p{}", now_ms());
    }
    p.base_url = p.base_url.trim().trim_end_matches('/').to_string();
    // Hard guard: never persist a blank/unusable provider (the UI "test"
    // button used to create empty entries when the form was not filled).
    if !p.base_url.starts_with("http://") && !p.base_url.starts_with("https://") {
        return Err("Base URL 必须以 http:// 或 https:// 开头".into());
    }
    if p.model.trim().is_empty() {
        return Err("模型名称不能为空".into());
    }
    if p.name.is_empty() {
        p.name = p.id.clone();
    }
    if !api_key.is_empty() {
        if key_set(&p.id, api_key) {
            p.key_in_file = false;
            p.api_key = None;
        } else {
            p.key_in_file = true;
            p.api_key = Some(api_key.to_string());
        }
    }

    let mut s = load_settings();
    if let Some(slot) = s.providers.iter_mut().find(|x| x.id == p.id) {
        let keep_file_key = slot.api_key.clone();
        *slot = p.clone();
        if api_key.is_empty() && slot.key_in_file && slot.api_key.is_none() {
            slot.api_key = keep_file_key;
        }
    } else {
        s.providers.push(p.clone());
    }
    if s.active.is_none() {
        s.active = Some(p.id.clone());
    }
    save_settings(&s)?;
    Ok(settings())
}

pub fn delete_provider(id: &str) -> Result<serde_json::Value, String> {
    key_del(id);
    let mut s = load_settings();
    s.providers.retain(|p| p.id != id);
    if s.active.as_deref() == Some(id) {
        s.active = s.providers.first().map(|p| p.id.clone());
    }
    save_settings(&s)?;
    Ok(settings())
}

pub fn set_active(id: &str) -> Result<serde_json::Value, String> {
    let mut s = load_settings();
    if !s.providers.iter().any(|p| p.id == id) {
        return Err("unknown provider id".into());
    }
    s.active = Some(id.to_string());
    save_settings(&s)?;
    Ok(settings())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Generation registry for the stop button: the newest chat gets a sequence
/// number; `stop()` marks it so the streaming loop breaks and drops the
/// connection at the next chunk boundary.
static GEN_SEQ: AtomicU64 = AtomicU64::new(1);
static STOP_GEN: AtomicU64 = AtomicU64::new(0);

/// The generation a new chat run owns.
///
/// `fetch_add` returns the *previous* value, so the owned generation is the
/// incremented one. The original code used the returned value directly while
/// `stop()` stored the post-increment value, which made the loop's
/// `STOP_GEN == my_gen` test unsatisfiable - the stop button never broke a
/// stream, and a stale `STOP_GEN` then killed the *next* chat on its first
/// chunk. Kept as a free function over `&AtomicU64` so the invariant is
/// unit-testable without the global statics.
fn next_generation(seq: &AtomicU64) -> u64 {
    seq.fetch_add(1, Ordering::Relaxed) + 1
}

/// Mark the currently running generation as stopped.
fn mark_stop_for_current(seq: &AtomicU64, stop: &AtomicU64) {
    stop.store(seq.load(Ordering::Relaxed), Ordering::Relaxed);
}

pub fn stop() {
    mark_stop_for_current(&GEN_SEQ, &STOP_GEN);
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(180))
        .timeout_write(Duration::from_secs(30))
        .build()
}

fn completions_url(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.ends_with("/v1") {
        format!("{}/chat/completions", b)
    } else if b.ends_with("/chat/completions") {
        b.to_string()
    } else {
        format!("{}/v1/chat/completions", b)
    }
}

pub(crate) fn active_provider() -> Result<(Provider, String), String> {
    let s = load_settings();
    let id = s
        .active
        .clone()
        .ok_or_else(|| "未配置 AI Provider：请先在设置中添加".to_string())?;
    let p = s
        .providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("active provider missing")?;
    let key = effective_key(&p)
        .filter(|k| !k.is_empty())
        .ok_or("该 Provider 未配置 API Key")?;
    Ok((p, key))
}

fn extract_error_message(raw: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        for pointer in ["/error/message", "/message", "/errmsg"] {
            if let Some(m) = v.pointer(pointer).and_then(|x| x.as_str()) {
                return m.to_string();
            }
        }
        return truncate(raw, 300);
    }
    truncate(raw, 300)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{}...", t)
    }
}

/// POST one API call with rich error extraction.
pub(crate) fn call_completions_post(
    base_url: &str,
    key: &str,
    path: &str,
    body: &serde_json::Value,
) -> Result<ureq::Response, String> {
    let url = if path == "/chat/completions" {
        completions_url(base_url)
    } else {
        format!(
            "{}/{}",
            base_url.trim().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    };
    match agent()
        .post(&url)
        .set("Authorization", &format!("Bearer {}", key))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
    {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, resp)) => {
            let raw = resp.into_string().unwrap_or_default();
            Err(format!(
                "HTTP {}: {}",
                code,
                truncate(&extract_error_message(&raw), 400)
            ))
        }
        Err(e) => Err(format!("请求失败: {}", e)),
    }
}

/// Connectivity test: minimal non-stream chat call.
pub fn test_provider(id: &str) -> Result<String, String> {
    let s = load_settings();
    let p = s
        .providers
        .iter()
        .find(|p| p.id == id)
        .ok_or("unknown provider")?;
    let key = effective_key(p)
        .filter(|k| !k.is_empty())
        .ok_or("未配置 API Key")?;
    let body = serde_json::json!({
        "model": p.model,
        "messages": [{ "role": "user", "content": "reply with the single word: ok" }],
        "max_tokens": 16,
        "stream": false,
    });
    let resp = call_completions_post(&p.base_url, &key, "/chat/completions", &body)?;
    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("响应解析失败: {}", e))?;
    if let Some(err) = v.get("error") {
        return Err(format!(
            "服务端错误: {}",
            err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
        ));
    }
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(空响应)")
        .trim()
        .to_string();
    Ok(if content.is_empty() {
        "(空响应)".into()
    } else {
        content
    })
}

/// Streaming chat: emits `ai-chunk` {text}, `ai-done` and on failure
/// `ai-error` {message}. Providers that ignore `stream: true` fall back to
/// a single chunk.
pub fn chat(app: AppHandle, messages: serde_json::Value, temperature: f64) -> Result<(), String> {
    let my_gen = next_generation(&GEN_SEQ);
    let (p, key) = active_provider()?;
    let body = serde_json::json!({
        "model": p.model,
        "messages": messages,
        "temperature": temperature,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    let resp = call_completions_post(&p.base_url, &key, "/chat/completions", &body)?;

    let reader = std::io::BufReader::new(resp.into_reader());
    let mut lines: Vec<String> = Vec::new();
    let mut saw_data = false;
    let mut stopped = false;
    let mut usage: Option<serde_json::Value> = None;
    // Single pass: emit chunks as they arrive and check the stop flag
    // between lines — breaking drops the reader, which closes the HTTP
    // connection (stop is a real disconnect, not just a rendering stop).
    for line in reader.lines() {
        if STOP_GEN.load(Ordering::Relaxed) == my_gen {
            stopped = true;
            break;
        }
        let l = match line {
            Ok(l) => l,
            Err(e) => {
                let msg = format!("流中断: {}", e);
                let _ = app.emit("ai-error", msg.clone());
                return Err(msg);
            }
        };
        lines.push(l.clone());
        let data = match l.strip_prefix("data:") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            break;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
            if v.get("usage").and_then(|u| u.get("total_tokens")).is_some() {
                usage = Some(v["usage"].clone());
                continue;
            }
            if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                if !delta.is_empty() {
                    saw_data = true;
                    if app.emit("ai-chunk", delta.to_string()).is_err() {
                        break;
                    }
                }
            }
            if let Some(err) = v.get("error") {
                let msg = format!(
                    "服务端错误: {}",
                    err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
                );
                let _ = app.emit("ai-error", msg.clone());
                return Err(msg);
            }
        }
    }

    if !saw_data {
        let whole = lines.join("\n");
        match serde_json::from_str::<serde_json::Value>(&whole) {
            Ok(v) => {
                if let Some(content) = v["choices"][0]["message"]["content"].as_str() {
                    if !content.is_empty() {
                        let _ = app.emit("ai-chunk", content.to_string());
                    }
                }
            }
            Err(_) => {
                let msg = format!("响应不是 SSE 流也不是 JSON: {}", truncate(&whole, 200));
                let _ = app.emit("ai-error", msg.clone());
                return Err(msg);
            }
        }
    }

    let _ = app.emit(
        "ai-done",
        serde_json::json!({ "model": p.model, "stopped": stopped, "usage": usage }),
    );
    Ok(())
}

/// One-shot non-streaming completion (poster HTML, theme generation, ...).
/// Budget ladder: reasoning models can burn the whole initial budget on
/// thinking, so an empty content with finish_reason=length retries at a
/// doubled budget (capped) before giving up with a clear diagnostic.
#[cfg(test)]
pub fn complete(
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f64,
) -> Result<String, String> {
    let (p, key) = active_provider()?;
    let msgs = vec![
        serde_json::json!({ "role": "system", "content": system }),
        serde_json::json!({ "role": "user", "content": user }),
    ];
    let mut budget = max_tokens.max(1024);
    loop {
        let reply = chat_once(&p.base_url, &key, &p.model, &msgs, budget, temperature)?;
        let content = strip_think(&reply.content).trim().to_string();
        if !content.is_empty() {
            return Ok(content);
        }
        if reply.finish == "length" && budget < COMPLETE_BUDGET_CAP {
            budget = (budget * 2).min(COMPLETE_BUDGET_CAP);
            continue;
        }
        return Err(empty_reply_diagnostic(&reply));
    }
}

pub(crate) const COMPLETE_BUDGET_CAP: u32 = 16384;

pub(crate) struct OnceReply {
    pub(crate) content: String,
    pub(crate) finish: String,
    pub(crate) has_reasoning: bool,
}

/// One raw non-streaming chat call, parsed into content + finish_reason.
#[cfg(test)]
fn chat_once(
    base_url: &str,
    key: &str,
    model: &str,
    msgs: &[serde_json::Value],
    max_tokens: u32,
    temperature: f64,
) -> Result<OnceReply, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": msgs,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": false,
    });
    let resp = call_completions_post(base_url, key, "/chat/completions", &body)?;
    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("响应解析失败: {}", e))?;
    if let Some(err) = v.get("error") {
        return Err(format!(
            "服务端错误: {}",
            err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
        ));
    }
    let msg = &v["choices"][0]["message"];
    Ok(OnceReply {
        content: msg["content"].as_str().unwrap_or("").to_string(),
        finish: v["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        has_reasoning: msg.get("reasoning_content").is_some(),
    })
}

/// Some models inline reasoning as a <think> block inside content.
pub(crate) fn strip_think(s: &str) -> String {
    let Some(start) = s.find("<think>") else {
        return s.to_string();
    };
    match s[start..].find("</think>") {
        Some(rel) => format!("{}{}", &s[..start], &s[start + rel + "</think>".len()..]),
        None => s[..start].to_string(),
    }
}

pub(crate) fn empty_reply_diagnostic(reply: &OnceReply) -> String {
    match reply.finish.as_str() {
        "length" => format!(
            "AI 输出被 max_tokens 截断（content 为空，思考未写完；预算已自动加到 {} 仍不够）。该模型思考占比过高，请换输出预算更大的模型或更短的需求描述",
            COMPLETE_BUDGET_CAP
        ),
        "content_filter" => "AI 输出被服务端内容安全过滤".into(),
        _ if reply.has_reasoning => "AI 只返回了思考内容、正文为空（推理型模型常见）；重试一次或更换模型".into(),
        _ => {
            // Include what actually came back - an empty-content failure with a
            // non-empty body is almost always a formatting problem worth seeing.
            let preview: String = reply.content.chars().take(80).collect();
            if preview.trim().is_empty() {
                format!(
                    "AI 返回的正文为空（finish_reason={}）；可重试或检查该模型的输出格式",
                    reply.finish
                )
            } else {
                format!(
                    "AI 未产出可用正文（finish_reason={}），原始返回片段：{}",
                    reply.finish, preview
                )
            }
        }
    }
}

// ------------------------------------------------------- image generation ---

pub fn image_model() -> String {
    let path = settings_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("image_model")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

pub fn set_image_model(model: &str) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    root["image_model"] = serde_json::json!(model.trim());
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// Cloud text-to-image through the active provider (OpenAI images API).
/// Returns asset file paths.
pub fn generate_image(prompt: &str, w: u32, h: u32) -> Result<Vec<String>, String> {
    let model = image_model();
    if model.is_empty() {
        return Err("请先在 AI 绘图中设置图像模型（如 agnes-image-2.5-flash）".into());
    }
    let (p, key) = active_provider()?;
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "size": format!("{}x{}", w, h),
    });
    let resp = call_completions_post(&p.base_url, &key, "/v1/images/generations", &body)?;
    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("响应解析失败: {}", e))?;
    if let Some(err) = v.get("error") {
        return Err(format!(
            "服务端错误: {}",
            err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
        ));
    }
    let items = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or("images 响应缺少 data 字段")?;
    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(120))
        .build();
    let mut paths = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let b64 = item.get("b64_json").and_then(|x| x.as_str()).unwrap_or("");
        let bytes = if !b64.is_empty() {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| format!("b64 解码失败: {}", e))?
        } else {
            let url = item
                .get("url")
                .and_then(|x| x.as_str())
                .filter(|u| !u.is_empty())
                .ok_or("images 响应缺少 url/b64_json")?;
            let r = agent
                .get(url)
                .call()
                .map_err(|e| format!("下载生成图失败: {}", e))?;
            let mut buf = Vec::new();
            r.into_reader()
                .take(64 * 1024 * 1024)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            buf
        };
        let p2 =
            crate::articles::import_from_bytes(&format!("ai-img-{}-{}.png", now_ms(), i), &bytes)?;
        paths.push(p2.to_string_lossy().to_string());
    }
    Ok(paths)
}

// ------------------------------------------------------- svg components ---

pub(crate) const SVG_EXPERT_SYSTEM: &str = "你是微信公众号 SVG 互动组件专家。只输出一个可直接粘贴进公众号文章的 HTML 片段：一个 <section> 包裹的内联 <svg>，不要任何解释、不要 markdown 围栏。

铁律（违反即废品）：
1. 全部样式内联；严禁 class/id/<style>/外部资源/外链图片；
2. 动画只能用 SMIL <animate> 或 <animateTransform>，且 begin 必须写 begin=\"touchstart; click\"（官方规则 R-1.7，PC 端无 touchstart）；
3. 可用 fill=freeze 做点击后状态保持；宽度用 viewBox + style=\"width: 100%\"；
4. 文字用 <text>；图片只能内嵌 data URI（<image href=\"data:image/...\">）；严禁 foreignObject；
5. 不使用 font-family；颜色避开纯黑（Dark Mode 下 SVG 不转换，建议浅背景深色字）；
6. 交互逻辑要简单可靠：点击渐显/切换/描边/进度点亮等，不要依赖复杂状态机。";

pub(crate) fn extract_svg_snippet(text: &str) -> Result<String, String> {
    let t = text.trim();
    let start = t.find("<section").or_else(|| t.find("<svg"));
    let start = match start {
        Some(i) => i,
        None => return Err("AI 输出中未找到 SVG 片段".into()),
    };
    let end = t
        .rfind("</section>")
        .map(|i| i + 10)
        .or_else(|| t.rfind("</svg>").map(|i| i + 6));
    match end {
        Some(e) if e > start => Ok(t[start..e].to_string()),
        _ => Err("AI 输出的 SVG 片段不完整".into()),
    }
}

pub(crate) fn validate_svg_snippet(snippet: &str) -> Result<(), String> {
    let blocks: Vec<String> = wxwright_core::validator::validate_html(snippet)
        .into_iter()
        .filter(|v| v.is_block())
        .map(|v| format!("{} {}", v.rule_id, v.message))
        .collect();
    if blocks.is_empty() {
        Ok(())
    } else {
        Err(blocks.join("; "))
    }
}

// ------------------------------------------------------- theme generation ---

pub(crate) const THEME_SYSTEM: &str = "你是微信公众号排版主题设计器。只输出一个完整的 TOML 主题文件，不要任何解释文字、不要 markdown 代码围栏。硬性规则：\n\
1. 严禁出现 font-family（官方规范 R-3.1）。即使风格描述要求等宽/特殊字体，也不要输出 font-family，改用 letter-spacing、font-weight、大小写间距与边框去近似那种气质；\n\
2. 所有颜色用 #RRGGBB 十六进制；\n\
3. 对比度以主题自己的 background 为基准：正文颜色与 background 对比度 >= 4.5:1，次级文字 >= 3:1。深色主题完全合规——深底配浅字即可，background 写深色、text 写浅色；\n\
4. 必须包含 [meta]（id 用小写字母数字和连字符）与 [colors]；\n\
5. [block.*] 覆盖只能使用安全 CSS 属性（margin/padding/border/color/font-size/font-weight/letter-spacing/text-align/line-height/background/border-radius）。";

const THEME_SCHEMA_TEMPLATE: &str = r##"可用 color 键（全部可选，未提供的用引擎默认）：
accent, text, text_secondary, text_tertiary, border, border_strong,
quote_bg, quote_text, code_bg, code_text, code_border, inline_code_color,
table_head_bg, table_border, note_bg, note_border, tip_bg, tip_border,
important_bg, important_border, warning_bg, warning_border, caution_bg,
caution_border, keypoint_bg, comment_bg, toc_bg

结构模板（严格按此结构）：
[meta]
id = "my-theme"
name = "My Theme"
name_zh = "我的主题"
author = "AI"
license = "MIT"
description = "..."
description_zh = "..."
link_style = "footnote"
code_theme = "light"

[colors]
accent = "#2F6CEA"
text = "#1F2328"
text_secondary = "#57606A"
text_tertiary = "#8B949E"
border = "#D8DEE4"
border_strong = "#A8B3BD"
quote_bg = "#F7F8FA"
quote_text = "#57606A"
code_bg = "#F6F8FA"
code_text = "#24292F"
code_border = "#E4E7EC"
inline_code_color = "#C2402A"
table_head_bg = "#F6F8FA"
table_border = "#D8DEE4"
note_bg = "#EFF4FE"
note_border = "#2F6CEA"

[block.h2]
border-left = "4px solid #2F6CEA"
padding-left = "10px"

[block.h2_leaf]
color = "#2F6CEA"

[block.card_keypoint]
background = "#EFF4FE"
border-left = "4px solid #2F6CEA"
padding = "14px 16px"
border-radius = "0 8px 8px 0"

[block.card_keypoint_leaf]
color = "#0C447C"
font-size = "15px"

[block.*] 覆盖规则（严格遵守）：
1. role 只能取：{ROLES}；
   可加 _leaf 后缀修饰行内文字；卡片类（card_*）还可加 _title 后缀修饰卡片标题标签；
2. 禁止任何伪类与复杂选择器：没有 a:hover、没有 [block.a]、没有嵌套——写 [block.a:hover] 是非法 TOML，会直接被拒；
3. 想要发光、渐变、悬浮效果，用安全属性近似：border + 鲜明 accent 色、background、letter-spacing、border-radius；
4. 属性只允许：margin / padding / border / color / font-size / font-weight /
   letter-spacing / text-align / line-height / background / border-radius。"##;

/// The theme schema with its role list generated from
/// `wxwright_core::roles::ROLES`. Generating it means the prompt can never
/// advertise a role the renderer does not consume - the drift that used to
/// silently discard every AI-authored card style.
pub(crate) fn theme_schema() -> String {
    THEME_SCHEMA_TEMPLATE.replace("{ROLES}", &wxwright_core::roles::prompt_role_list())
}

const THEME_SAMPLE_DOC: &str = r#"# 标题一

正文段落，包含**加粗**、*斜体*、`行内代码`与[链接](https://example.com)。

## 标题二

> 引用文字。

> [!NOTE]
> 提示卡片。

| 列A | 列B |
|---|---|
| 1 | 2 |

```rust
fn main() {}
```

- 列表项
"#;

pub(crate) fn extract_toml(text: &str) -> Result<String, String> {
    let t = text.trim();
    if t.starts_with("```") {
        let after = &t[t.find("```").unwrap()..];
        let line_end = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let rest = &after[line_end..];
        let end = rest.rfind("```").unwrap_or(rest.len());
        let inner = rest[..end].trim();
        if inner.starts_with("[meta]") {
            return Ok(sanitize_model_toml(inner));
        }
    }
    if let Some(i) = t.find("[meta]") {
        let cut = t[i..].trim().trim_end_matches("```").trim().to_string();
        return Ok(sanitize_model_toml(&cut));
    }
    Err("AI 输出中未找到 TOML 主题".into())
}

/// Models occasionally emit TOML that does not parse. Instead of burning a
/// retry on a parse error, repair the known failure classes:
/// 1. pseudo-class / nested-subtable headers (`[block.a:hover]`, `::before`,
///    `[block.task.completed]`) are dropped with their bodies - the dialect
///    supports only flat `[block.<role>]` tables;
/// 2. duplicate table headers are merged (models think tables append);
/// 3. stray prose before the first table is dropped.
pub(crate) fn sanitize_model_toml(src: &str) -> String {
    let is_valid_header = |inner: &str| {
        // flat tables only: zero dots ([meta]) or one dot ([block.h2],
        // [colors]); two or more dots mean a nested subtable the engine
        // cannot address, and ':' means a pseudo-class selector
        inner.matches('.').count() <= 1
            && !inner.is_empty()
            && inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    };
    let is_key_value = |line: &str| {
        let t = line.trim();
        t.contains('=')
            && t.chars()
                .next()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false)
    };
    let mut preamble: Vec<&str> = Vec::new();
    let mut order: Vec<String> = Vec::new();
    let mut sections: std::collections::HashMap<String, Vec<&str>> =
        std::collections::HashMap::new();
    let mut current: Option<String> = None;
    let mut seen_header = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            seen_header = true;
            let inner = &t[1..t.len() - 1];
            if is_valid_header(inner) {
                let key = inner.to_string();
                if !sections.contains_key(&key) {
                    order.push(key.clone());
                    sections.insert(key.clone(), Vec::new());
                }
                current = Some(key);
            } else {
                current = None; // invalid header: drop it and its body
            }
            continue;
        }
        match &current {
            Some(key) => sections.get_mut(key).unwrap().push(line),
            None => {
                // only genuine preamble (before ANY table) may pass through;
                // bodies of dropped sections must not leak back in
                if !seen_header && (t.is_empty() || is_key_value(line)) {
                    preamble.push(line);
                }
            }
        }
    }
    let mut out = String::new();
    for l in &preamble {
        out.push_str(l);
        out.push('\n');
    }
    for key in &order {
        out.push_str(&format!("[{}]\n", key));
        // de-duplicate keys within a section (models repeat definitions);
        // the LAST occurrence wins, emitted at the first occurrence's slot
        let mut seen_keys: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut emitted: Vec<String> = Vec::new();
        for l in &sections[key] {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(eq) = t.find('=') {
                let k = t[..eq].trim().to_string();
                let valid_key = k
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
                if !valid_key {
                    continue; // stray prose inside a section body
                }
                match seen_keys.get(&k) {
                    Some(&slot) => emitted[slot] = l.trim().to_string(),
                    None => {
                        seen_keys.insert(k, emitted.len());
                        emitted.push(l.trim().to_string());
                    }
                }
            }
            // lines without '=' inside a section body are dropped
        }
        for l in &emitted {
            out.push_str(l);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// Validate a generated theme: parse, render the sample doc, require zero
/// blocking violations. Returns non-blocking warnings.
pub(crate) fn validate_generated_theme(toml_src: &str) -> Result<Vec<String>, String> {
    let t = theme::parse_theme(toml_src).map_err(|e| format!("TOML 解析失败: {}", e))?;

    // A role the renderer does not consume can never have an effect. Reject it
    // loudly instead of accepting a theme that silently does nothing - the
    // failure mode that made AI-authored card styling invisible for so long.
    let unknown: Vec<String> = t
        .blocks
        .keys()
        .filter(|k| !wxwright_core::roles::is_known_key(k))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "以下 [block.*] 角色不会被渲染器读取，请删除或改用受支持的角色: {}。受支持的角色：{}",
            unknown.join(", "),
            wxwright_core::roles::prompt_role_list()
        ));
    }

    let opts = ConvertOptions::new(t);
    let out = pipeline(THEME_SAMPLE_DOC, &opts).map_err(|e| format!("渲染失败: {}", e))?;
    let blocks = out.blocking_violations();
    if !blocks.is_empty() {
        let list = blocks
            .iter()
            .map(|v| format!("{} {}", v.rule_id, v.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("{} 个阻断级违规: {}", blocks.len(), list));
    }
    Ok(out
        .violations
        .iter()
        .filter(|v| !v.is_block())
        .map(|v| format!("{}: {}", v.rule_id, v.message))
        .collect())
}

/// Persist a validated theme TOML into the user themes dir and return the
/// metadata payload shared by the sync and job-based generation paths.
pub(crate) fn save_theme_artifact(toml_src: &str) -> Result<serde_json::Value, String> {
    let path = theme::save_user_theme(toml_src).map_err(|e| e.to_string())?;
    let t = theme::parse_theme(toml_src).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": t.meta.id,
        "name": t.meta.name,
        "name_zh": t.meta.name_zh,
        "file": path.to_string_lossy(),
    }))
}

/// Generate a brand-new theme via the active AI provider, validate it
/// against the official rules, persist it into the user themes dir.
/// One automatic retry feeds the validation errors back to the model.
#[cfg(test)]
pub fn generate_theme(description: &str) -> Result<serde_json::Value, String> {
    let (p, key) = active_provider()?;
    let base_user = format!(
        "请设计一个微信公众号排版主题。风格要求：{}\n\n{}",
        description.trim(),
        theme_schema()
    );
    let mut last_err = String::new();
    let mut last_raw = String::new();
    // theme TOML is a long structured output and reasoning models can burn an
    // entire shared budget on thinking before writing a byte: start high and
    // allow a higher cap than chat (COMPLETE_BUDGET_CAP)
    let mut budget: u32 = 8192;
    const THEME_BUDGET_CAP: u32 = 32768;
    for attempt in 0..4 {
        let user_text = if attempt == 0 || last_err.is_empty() {
            base_user.clone()
        } else if last_err.contains("思考") || last_err.contains("截断") {
            format!(
                "{}\n\n注意：上一次输出出现问题：{}。请直接输出最终 TOML 文件本身，跳过一切思考过程的展开。",
                base_user, last_err
            )
        } else {
            format!(
                "{}\n\n注意：上一次输出出现问题：{}。请修正后重新输出完整 TOML。",
                base_user, last_err
            )
        };
        let msgs = vec![
            serde_json::json!({ "role": "system", "content": THEME_SYSTEM }),
            serde_json::json!({ "role": "user", "content": user_text }),
        ];
        let reply = chat_once(&p.base_url, &key, &p.model, &msgs, budget, 0.8)?;
        let content = strip_think(&reply.content).trim().to_string();
        last_raw = content.clone();
        if content.is_empty() {
            // Surface *why* it was empty instead of the useless "AI 返回为空".
            if reply.finish == "length" && budget < THEME_BUDGET_CAP {
                budget = (budget * 2).min(THEME_BUDGET_CAP);
            }
            last_err = empty_reply_diagnostic(&reply);
            continue;
        }
        let toml_src = match extract_toml(&content) {
            Ok(t) => {
                // dump the sanitized text: parse errors report line numbers
                // against exactly this, so the dump is directly readable
                last_raw = t.clone();
                t
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        match validate_generated_theme(&toml_src) {
            Ok(warnings) => {
                let mut v = save_theme_artifact(&toml_src)?;
                v["warnings"] = serde_json::json!(warnings);
                return Ok(v);
            }
            Err(e) => last_err = e,
        }
    }
    // persist the last raw attempt so failures are diagnosable instead of a
    // dead end; the path rides along in the error message
    let dump = theme::user_themes_dir().join("last-failed-theme.txt");
    if std::fs::write(&dump, &last_raw).is_ok() {
        return Err(format!(
            "AI 主题未通过合规校验：{}（原始输出已存至 {}）",
            last_err,
            dump.display()
        ));
    }
    Err(format!("AI 主题未通过合规校验：{}", last_err))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression for the stop button never working.
    ///
    /// `chat()` owns the generation returned by `fetch_add + 1`, while
    /// `stop()` stores `GEN_SEQ.load()`. When chat used the raw
    /// `fetch_add` result the two could never be equal, so the streaming
    /// loop's `STOP_GEN == my_gen` check was unsatisfiable - and the stale
    /// value left behind by a stop click then killed the *next* chat on its
    /// first chunk. Local atomics keep this deterministic (no shared statics,
    /// no test-order coupling).
    #[test]
    fn stop_marks_the_running_generation_and_never_the_next_one() {
        let seq = AtomicU64::new(1);
        let stop = AtomicU64::new(0);

        let chat1 = next_generation(&seq);
        mark_stop_for_current(&seq, &stop);
        assert_eq!(
            stop.load(Ordering::Relaxed),
            chat1,
            "stop must target the chat that is currently running"
        );

        let chat2 = next_generation(&seq);
        assert_ne!(
            stop.load(Ordering::Relaxed),
            chat2,
            "a stop clicked during chat #1 must not kill chat #2"
        );
    }

    /// The inverse direction: a stop clicked while idle must not arm itself
    /// for whatever runs next.
    #[test]
    fn idle_stop_does_not_poison_the_following_chat() {
        let seq = AtomicU64::new(1);
        let stop = AtomicU64::new(0);
        // Nothing running yet: the user clicks stop anyway.
        mark_stop_for_current(&seq, &stop);
        let chat = next_generation(&seq);
        assert_ne!(
            stop.load(Ordering::Relaxed),
            chat,
            "an idle stop must not be interpreted as stopping the next chat"
        );
    }

    /// Live smoke for the cloud image engine - generates the README banner.
    /// `cargo test -p wxwright-gui banner_generation_smoke -- --ignored --nocapture`
    #[test]
    #[ignore = "live smoke: calls the real AI provider (needs a configured key)"]
    fn banner_generation_smoke() {
        if image_model().is_empty() {
            println!("skip: no image model configured (Settings, AI drawing, cloud source)");
            return;
        }
        match crate::ai::active_provider() {
            Err(e) => {
                println!("skip: no active AI provider configured: {e}");
                return;
            }
            Ok((p, _)) => println!("live provider: {} / {}", p.name, p.model),
        }
        let files = generate_image(
            "Minimal flat editorial banner, wide composition, soft warm paper texture background, a stylized teal-blue fountain pen line flowing from left to right into abstract article layout blocks (rectangles suggesting text columns), vermilion accent dot, generous negative space, no text, no letters, clean vector style",
            1508,
            430,
        )
        .expect("banner generation should succeed");
        println!("banner files: {files:?}");
        assert!(!files.is_empty(), "at least one banner image");
        for f in &files {
            let meta = std::fs::metadata(f).expect("banner file exists");
            assert!(meta.len() > 5000, "banner suspiciously small: {f}");
        }
    }

    #[test]
    fn sanitize_drops_pseudo_class_sections() {
        let src = "[meta]\nid = \"t\"\nname = \"T\"\nname_zh = \"测试\"\n\n[block.h2]\ncolor = \"#FF0000\"\n\n[block.a:hover]\ncolor = \"#00FF00\"\nglow = \"on\"\n\n[colors]\naccent = \"#123456\"\n";
        let out = sanitize_model_toml(src);
        assert!(out.contains("[block.h2]"), "valid header kept");
        assert!(
            out.contains("accent = \"#123456\""),
            "later valid section kept"
        );
        assert!(
            !out.contains("hover"),
            "pseudo-class header and body dropped"
        );
        assert!(!out.contains("glow"), "dropped section body gone");
        // the sanitized output must parse
        theme::parse_theme(&out).expect("sanitized toml parses");
    }

    #[test]
    fn sanitize_keeps_normal_toml_intact() {
        let src = "[meta]\nid = \"t\"\nname = \"T\"\nname_zh = \"测试\"\n\n[colors]\naccent = \"#123456\"\n\n[block.paragraph]\nline-height = \"1.7\"\n";
        let out = sanitize_model_toml(src);
        let t = theme::parse_theme(&out).expect("clean toml still parses");
        assert_eq!(t.meta.id, "t");
        assert_eq!(t.colors.get("accent").map(String::as_str), Some("#123456"));
        assert_eq!(
            t.blocks.get("paragraph").map(|d| d.len()),
            Some(1),
            "block override preserved"
        );
    }

    #[test]
    fn sanitize_merges_duplicate_tables() {
        let src = "[meta]\nid = \"t\"\nname = \"T\"\nname_zh = \"测试\"\n\n[block.gfm_table_cell]\nbackground = \"#000000\"\n\n[colors]\naccent = \"#123456\"\n\n[block.gfm_table_cell]\ncolor = \"#CCCCCC\"\n";
        let out = sanitize_model_toml(src);
        theme::parse_theme(&out).expect("merged toml parses");
        assert_eq!(
            out.matches("[block.gfm_table_cell]").count(),
            1,
            "duplicate header merged"
        );
        assert!(
            out.contains("background = \"#000000\"") && out.contains("color = \"#CCCCCC\""),
            "both bodies preserved in the merged section"
        );
    }

    #[test]
    fn sanitize_drops_preamble_prose() {
        let src = "好的，这是主题：\n\n[meta]\nid = \"t\"\nname = \"T\"\nname_zh = \"测试\"\n";
        let out = sanitize_model_toml(src);
        theme::parse_theme(&out).expect("prose-free toml parses");
        assert!(!out.contains("好的"), "prose dropped from preamble");
    }

    #[test]
    fn sanitize_drops_nested_subtables() {
        let src = "[meta]\nid = \"t\"\nname = \"T\"\nname_zh = \"测试\"\n\n[block.task]\ncolor = \"#123456\"\n\n[block.task.completed]\ncolor = \"#000000\"\n\n[colors]\naccent = \"#123456\"\n";
        let out = sanitize_model_toml(src);
        theme::parse_theme(&out).expect("flat toml parses");
        assert!(out.contains("[block.task]"), "flat role kept");
        assert!(!out.contains("task.completed"), "nested subtable dropped");
    }

    #[test]
    fn url_building() {
        assert_eq!(
            completions_url("http://localhost:11434"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            completions_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            completions_url("https://a.b/c/chat/completions"),
            "https://a.b/c/chat/completions"
        );
    }

    #[test]
    fn mask_hides() {
        assert_eq!(mask(""), "");
        assert_eq!(mask("short"), "***");
        assert!(mask("sk-1234567890abcdefgh").contains("****"));
    }

    #[test]
    fn toml_extraction() {
        assert_eq!(
            extract_toml("```toml\n[meta]\nid = \"x\"\n```\n").unwrap(),
            "[meta]\nid = \"x\""
        );
        assert_eq!(
            extract_toml("[meta]\nid = \"y\"").unwrap(),
            "[meta]\nid = \"y\""
        );
        assert!(extract_toml("no theme here").is_err());
    }

    #[test]
    fn sample_doc_renders_compliant() {
        // The theme-validation sample must itself be compliant for minimal.
        let t = theme::load_builtin_theme("minimal").unwrap();
        let out = pipeline(THEME_SAMPLE_DOC, &ConvertOptions::new(t)).unwrap();
        assert!(out.blocking_violations().is_empty());
    }

    #[test]
    fn strip_think_removes_reasoning_block() {
        assert_eq!(strip_think("<think>blah</think>[meta]"), "[meta]");
        assert_eq!(strip_think("<think>unclosed"), "");
        assert_eq!(strip_think("clean output"), "clean output");
    }
}
