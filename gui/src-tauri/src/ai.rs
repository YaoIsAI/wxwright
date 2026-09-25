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
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("wxwright")
        .join("settings.json")
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
    std::fs::write(&path, body).map_err(|e| e.to_string())
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

pub fn stop() {
    STOP_GEN.store(GEN_SEQ.load(Ordering::Relaxed), Ordering::Relaxed);
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

fn active_provider() -> Result<(Provider, String), String> {
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
fn call_completions_post(
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
    let my_gen = GEN_SEQ.fetch_add(1, Ordering::Relaxed);
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
    for line in reader.lines() {
        match line {
            Ok(l) => lines.push(l),
            Err(e) => {
                let msg = format!("流中断: {}", e);
                let _ = app.emit("ai-error", msg.clone());
                return Err(msg);
            }
        }
    }

    let mut saw_data = false;
    let mut stopped = false;
    let mut usage: Option<serde_json::Value> = None;
    for line in &lines {
        if STOP_GEN.load(Ordering::Relaxed) == my_gen {
            stopped = true;
            break;
        }
        let data = match line.strip_prefix("data:") {
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

const COMPLETE_BUDGET_CAP: u32 = 16384;

struct OnceReply {
    content: String,
    finish: String,
    has_reasoning: bool,
}

/// One raw non-streaming chat call, parsed into content + finish_reason.
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
        finish: v["choices"][0]["finish_reason"].as_str().unwrap_or("?").to_string(),
        has_reasoning: msg.get("reasoning_content").is_some(),
    })
}

/// Some models inline reasoning as a <think> block inside content.
fn strip_think(s: &str) -> String {
    let Some(start) = s.find("<think>") else { return s.to_string() };
    match s[start..].find("</think>") {
        Some(rel) => format!("{}{}", &s[..start], &s[start + rel + "</think>".len()..]),
        None => s[..start].to_string(),
    }
}

fn empty_reply_diagnostic(reply: &OnceReply) -> String {
    match reply.finish.as_str() {
        "length" => format!(
            "AI 输出被 max_tokens 截断（content 为空，思考未写完；预算已自动加到 {} 仍不够）。该模型思考占比过高，请换输出预算更大的模型或更短的需求描述",
            COMPLETE_BUDGET_CAP
        ),
        "content_filter" => "AI 输出被服务端内容安全过滤".into(),
        _ if reply.has_reasoning => "AI 只返回了思考内容、正文为空（推理型模型常见）；重试一次或更换模型".into(),
        _ => format!("AI 返回的正文为空（finish_reason={}）；可重试或检查该模型的输出格式", reply.finish),
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

const SVG_EXPERT_SYSTEM: &str = "你是微信公众号 SVG 互动组件专家。只输出一个可直接粘贴进公众号文章的 HTML 片段：一个 <section> 包裹的内联 <svg>，不要任何解释、不要 markdown 围栏。

铁律（违反即废品）：
1. 全部样式内联；严禁 class/id/<style>/外部资源/外链图片；
2. 动画只能用 SMIL <animate> 或 <animateTransform>，且 begin 必须写 begin=\"touchstart; click\"（官方规则 R-1.7，PC 端无 touchstart）；
3. 可用 fill=freeze 做点击后状态保持；宽度用 viewBox + style=\"width: 100%\"；
4. 文字用 <text>；图片只能内嵌 data URI（<image href=\"data:image/...\">）；严禁 foreignObject；
5. 不使用 font-family；颜色避开纯黑（Dark Mode 下 SVG 不转换，建议浅背景深色字）；
6. 交互逻辑要简单可靠：点击渐显/切换/描边/进度点亮等，不要依赖复杂状态机。";

pub fn generate_svg_component(desc: &str) -> Result<String, String> {
    let mut last_err = String::new();
    for attempt in 0..2 {
        let user_text = if attempt == 0 {
            format!("组件效果需求：{}", desc)
        } else {
            format!(
                "组件效果需求：{}

注意：上一次输出未通过合规校验，错误：{}。请修正后重新输出完整片段。",
                desc, last_err
            )
        };
        let content = complete(SVG_EXPERT_SYSTEM, &user_text, 4096, 0.8)?;
        let snippet = extract_svg_snippet(&content)?;
        match validate_svg_snippet(&snippet) {
            Ok(()) => return Ok(snippet),
            Err(e) => last_err = e,
        }
    }
    Err(format!("AI 组件未通过合规校验：{}", last_err))
}

fn extract_svg_snippet(text: &str) -> Result<String, String> {
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

fn validate_svg_snippet(snippet: &str) -> Result<(), String> {
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

const THEME_SYSTEM: &str = "你是微信公众号排版主题设计器。只输出一个完整的 TOML 主题文件，不要任何解释文字、不要 markdown 代码围栏。硬性规则：\n\
1. 严禁出现 font-family（官方规范 R-3.1）；\n\
2. 所有颜色用 #RRGGBB 十六进制；\n\
3. 正文颜色与白色背景对比度 >= 4.5:1，次级文字 >= 3:1；\n\
4. 必须包含 [meta]（id 用小写字母数字和连字符）与 [colors]；\n\
5. [block.*] 覆盖只能使用安全 CSS 属性（margin/padding/border/color/font-size/font-weight/letter-spacing/text-align/line-height/background/border-radius）。";

const THEME_SCHEMA: &str = r##"可用 color 键（全部可选，未提供的用引擎默认）：
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
color = "#2F6CEA""##;

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

fn extract_toml(text: &str) -> Result<String, String> {
    let t = text.trim();
    if t.starts_with("```") {
        let after = &t[t.find("```").unwrap()..];
        let line_end = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let rest = &after[line_end..];
        let end = rest.rfind("```").unwrap_or(rest.len());
        let inner = rest[..end].trim();
        if inner.starts_with("[meta]") {
            return Ok(inner.to_string());
        }
    }
    if let Some(i) = t.find("[meta]") {
        let cut = t[i..].trim().trim_end_matches("```").trim().to_string();
        return Ok(cut);
    }
    Err("AI 输出中未找到 TOML 主题".into())
}

/// Validate a generated theme: parse, render the sample doc, require zero
/// blocking violations. Returns non-blocking warnings.
fn validate_generated_theme(toml_src: &str) -> Result<Vec<String>, String> {
    let t = theme::parse_theme(toml_src).map_err(|e| format!("TOML 解析失败: {}", e))?;
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

/// Generate a brand-new theme via the active AI provider, validate it
/// against the official rules, persist it into the user themes dir.
/// One automatic retry feeds the validation errors back to the model.
pub fn generate_theme(description: &str) -> Result<serde_json::Value, String> {
    let (p, key) = active_provider()?;
    let base_user = format!(
        "请设计一个微信公众号排版主题。风格要求：{}\n\n{}",
        description.trim(),
        THEME_SCHEMA
    );
    let mut last_err = String::new();
    let mut budget: u32 = 4096;
    for attempt in 0..4 {
        let user_text = if attempt == 0 || last_err.is_empty() {
            base_user.clone()
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
        if content.is_empty() {
            // Surface *why* it was empty instead of the useless "AI 返回为空".
            if reply.finish == "length" && budget < COMPLETE_BUDGET_CAP {
                budget = (budget * 2).min(COMPLETE_BUDGET_CAP);
            }
            last_err = empty_reply_diagnostic(&reply);
            continue;
        }
        let toml_src = extract_toml(&content)?;
        match validate_generated_theme(&toml_src) {
            Ok(warnings) => {
                let path = theme::save_user_theme(&toml_src).map_err(|e| e.to_string())?;
                let t = theme::parse_theme(&toml_src).map_err(|e| e.to_string())?;
                return Ok(serde_json::json!({
                    "id": t.meta.id,
                    "name": t.meta.name,
                    "name_zh": t.meta.name_zh,
                    "file": path.to_string_lossy(),
                    "warnings": warnings,
                }));
            }
            Err(e) => last_err = e,
        }
    }
    Err(format!("AI 主题未通过合规校验：{}", last_err))
}

#[cfg(test)]
mod tests {
    use super::*;

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
