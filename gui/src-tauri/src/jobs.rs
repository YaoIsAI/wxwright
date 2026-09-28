//! Unified AI generation runtime.
//!
//! Every AI generation in the GUI (theme / SVG component / poster HTML /
//! cloud image / ComfyUI) runs as a *job*: a registry entry with an id and a
//! cancel flag, streaming progress to the frontend over one `ai-job` event
//! channel (`{ id, kind, ev, data }`, ev ∈ status|delta|think|done|error).
//! This mirrors mainstream agent harnesses: streamable, cancellable,
//! validate-and-repair loops, budget ladders — instead of opaque one-shot
//! calls that can neither be watched nor stopped.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

/// Sentinel error returned when the user cancels a job.
pub const CANCELLED: &str = "__wxwright_cancelled__";

pub struct Job {
    pub id: u64,
    pub kind: String,
    pub cancel: AtomicBool,
}

static JOBS: OnceLock<Mutex<HashMap<u64, Arc<Job>>>> = OnceLock::new();
static JOB_SEQ: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Mutex<HashMap<u64, Arc<Job>>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lock the job registry, recovering from poisoning instead of panicking.
///
/// Poisoning means some thread panicked while holding the lock; the map itself
/// is still usable and `JobGuard::drop` already recovers. The two call sites
/// used `.expect("job registry poisoned")` instead, so a single failed job
/// could take the whole app down - the lock had two different policies.
fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<u64, Arc<Job>>> {
    registry().lock().unwrap_or_else(|e| e.into_inner())
}

fn emit<R: tauri::Runtime>(app: &AppHandle<R>, id: u64, kind: &str, ev: &str, data: Value) {
    let _ = app.emit(
        "ai-job",
        json!({ "id": id, "kind": kind, "ev": ev, "data": data }),
    );
}

/// Removes a job from the registry when its worker ends - including when the
/// worker unwinds. Previously the removal was the last statement of the
/// closure, so a panicking job leaked its `Arc<Job>` for the lifetime of the
/// process and `ai_job_stop` kept answering `true` for a job that was gone.
struct JobGuard(u64);

impl Drop for JobGuard {
    fn drop(&mut self) {
        // Drop must never panic (that would abort the process); the shared
        // helper recovers from poisoning for every caller.
        lock_registry().remove(&self.0);
    }
}

/// Register a job, spawn its work on the blocking pool, return its id.
pub fn spawn(
    kind: &str,
    app: AppHandle,
    work: impl FnOnce(Arc<Job>, AppHandle) + Send + 'static,
) -> u64 {
    let id = JOB_SEQ.fetch_add(1, Ordering::Relaxed);
    let job = Arc::new(Job {
        id,
        kind: kind.to_string(),
        cancel: AtomicBool::new(false),
    });
    lock_registry().insert(id, job.clone());
    let kind_owned = job.kind.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = JobGuard(id);
        let panic_app = app.clone();
        let panic_kind = kind_owned.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            work(job, app);
        }));
        if outcome.is_err() {
            // Without this the frontend would never see a terminal event and
            // the trigger button would sit on "Stop" forever.
            emit(
                &panic_app,
                id,
                &panic_kind,
                "error",
                Value::String("生成任务异常终止（内部错误）".into()),
            );
        }
    });
    id
}

/// Cooperative cancel: set the flag; streaming loops check it between
/// chunks (dropping the HTTP reader closes the connection).
pub fn stop(id: u64) -> bool {
    match lock_registry().get(&id) {
        Some(job) => {
            job.cancel.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

// ------------------------------------------------------------ capability ---

/// Heuristic reasoning-model detection: these burn budget on thinking, so
/// they start with a larger token budget.
pub fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_lowercase();
    [
        "o1", "o3", "o4-", "r1", "reasoner", "think", "glm-z", "qwq", "-t1",
    ]
    .iter()
    .any(|k| m.contains(k))
}

fn start_budget(spec_budget: u32) -> u32 {
    match crate::ai::active_provider() {
        Ok((p, _)) if is_reasoning_model(&p.model) => spec_budget.max(8192),
        _ => spec_budget,
    }
}

// --------------------------------------------------------- stream engine ---

/// Stream one chat completion for a job: emits `delta` (and `think` length
/// for reasoning models), honours cancel between chunks, falls back to a
/// whole-body JSON parse for providers that ignore `stream: true`, and
/// ladders the budget when a reasoning pass eats it all.
pub fn chat_stream<R: tauri::Runtime>(
    app: &AppHandle<R>,
    job: &Arc<Job>,
    msgs: &[Value],
    spec_budget: u32,
    temperature: f64,
) -> Result<String, String> {
    let (p, key) = crate::ai::active_provider()?;
    let mut budget = start_budget(spec_budget).max(1024);
    loop {
        if job.cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let body = json!({
            "model": p.model,
            "messages": msgs,
            "temperature": temperature,
            "max_tokens": budget,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        let resp = crate::ai::call_completions_post(&p.base_url, &key, "/chat/completions", &body)?;
        let reader = BufReader::new(resp.into_reader());
        let mut content = String::new();
        let mut finish = String::from("?");
        let mut has_reasoning = false;
        let mut saw_sse = false;
        let mut raw_lines: Vec<String> = Vec::new();
        let mut cancelled = false;
        let mut net_err: Option<String> = None;
        for line in reader.lines() {
            if job.cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let l = match line {
                Ok(l) => l,
                Err(e) => {
                    net_err = Some(format!("流中断: {}", e));
                    break;
                }
            };
            raw_lines.push(l.clone());
            let Some(data) = l.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            saw_sse = true;
            if v.get("usage").and_then(|u| u.get("total_tokens")).is_some() {
                continue;
            }
            if let Some(rc) = v["choices"][0]["delta"]["reasoning_content"].as_str() {
                if !rc.is_empty() {
                    has_reasoning = true;
                    // DeepSeek-style: surface that thinking is happening
                    // without flooding the log with it.
                    if content.is_empty() {
                        emit(
                            app,
                            job.id,
                            &job.kind,
                            "think",
                            json!({ "chars": rc.len() }),
                        );
                    }
                }
            }
            if let Some(d) = v["choices"][0]["delta"]["content"].as_str() {
                if !d.is_empty() {
                    content.push_str(d);
                    emit(app, job.id, &job.kind, "delta", json!(d));
                }
            }
            if let Some(f) = v["choices"][0]["finish_reason"].as_str() {
                finish = f.to_string();
            }
            if let Some(err) = v.get("error") {
                return Err(format!(
                    "服务端错误: {}",
                    err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
                ));
            }
        }
        if cancelled {
            return Err(CANCELLED.into());
        }
        // Providers that ignore stream:true return one JSON body.
        if !saw_sse && content.is_empty() {
            let whole = raw_lines.join("\n");
            if let Ok(v) = serde_json::from_str::<Value>(&whole) {
                if let Some(err) = v.get("error") {
                    return Err(format!(
                        "服务端错误: {}",
                        err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
                    ));
                }
                if let Some(c) = v["choices"][0]["message"]["content"].as_str() {
                    content = c.to_string();
                    if !content.is_empty() {
                        emit(app, job.id, &job.kind, "delta", json!(content));
                    }
                }
                if let Some(f) = v["choices"][0]["finish_reason"].as_str() {
                    finish = f.to_string();
                }
                has_reasoning = v["choices"][0]["message"]
                    .get("reasoning_content")
                    .is_some();
            }
        }
        let trimmed = crate::ai::strip_think(&content).trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
        if let Some(e) = net_err {
            return Err(e);
        }
        if finish == "length" && budget < crate::ai::COMPLETE_BUDGET_CAP {
            budget = (budget * 2).min(crate::ai::COMPLETE_BUDGET_CAP);
            emit(
                app,
                job.id,
                &job.kind,
                "status",
                json!({ "text": format!("思考占满预算，自动提升输出预算至 {} 重试", budget) }),
            );
            continue;
        }
        return Err(crate::ai::empty_reply_diagnostic(&crate::ai::OnceReply {
            content,
            finish,
            has_reasoning,
        }));
    }
}

// ------------------------------------------------------------ task specs ---

/// A generate-validate-repair task, uniform across kinds. `extract` pulls
/// the artifact out of the model text; `validate` enforces the official MP
/// constraints (Err = blocking, fed back to the model on retry).
struct TaskSpec {
    budget: u32,
    temperature: f64,
    attempts: u32,
    system: fn() -> &'static str,
    user: fn(&Value, u32) -> String,
    extract: fn(&str) -> Result<String, String>,
    validate: fn(&str) -> Result<Vec<String>, String>,
}

fn spec(kind: &str) -> TaskSpec {
    match kind {
        "svg" => TaskSpec {
            budget: 4096,
            temperature: 0.8,
            attempts: 3,
            system: || crate::ai::SVG_EXPERT_SYSTEM,
            user: |p, attempt| {
                let desc = p["description"].as_str().unwrap_or("");
                if attempt == 0 {
                    format!("组件效果需求：{}", desc)
                } else {
                    format!("组件效果需求：{}\n\n注意：上一次输出未通过合规校验，错误：{}。请修正后重新输出完整片段。", desc, p["_last_err"].as_str().unwrap_or(""))
                }
            },
            extract: |text| crate::ai::extract_svg_snippet(text),
            validate: |s| crate::ai::validate_svg_snippet(s).map(|()| Vec::new()),
        },
        "poster" => TaskSpec {
            budget: 8192,
            temperature: 0.8,
            attempts: 3,
            system: || POSTER_SYSTEM,
            user: |p, attempt| poster_user(p, attempt),
            extract: |text| extract_poster_html(text),
            validate: |html| validate_poster_html(html),
        },
        _ => TaskSpec {
            budget: 4096,
            temperature: 0.8,
            attempts: 4,
            system: || crate::ai::THEME_SYSTEM,
            user: |p, attempt| {
                let base = format!(
                    "请设计一个微信公众号排版主题。风格要求：{}\n\n{}",
                    p["description"].as_str().unwrap_or(""),
                    crate::ai::theme_schema()
                );
                if attempt == 0 {
                    base
                } else {
                    format!(
                        "{}\n\n注意：上一次输出出现问题：{}。请修正后重新输出完整 TOML。",
                        base,
                        p["_last_err"].as_str().unwrap_or("")
                    )
                }
            },
            extract: |text| crate::ai::extract_toml(text),
            validate: |toml| crate::ai::validate_generated_theme(toml),
        },
    }
}

// ------------------------------------------------------------- poster -----

pub const POSTER_SYSTEM: &str = "你是公众号/社媒海报设计师。输出一个完整自包含的 HTML 文档：单文件、全部样式内联在 <style> 中。硬性规则（违反即废品）：
1. 严禁任何外部资源：外部图片/字体/<link>/<script>/@import/url(http...) 都不允许，图片只能内嵌 data URI；
2. html 与 body 尺寸固定为任务给定的宽高，overflow hidden，边距归零；
3. 不要 JavaScript；动画可有可无（产物是静态图）；
4. 使用系统字体栈（产物是图片，font 不受限）；设计需高级、极简、留白充分；
5. 若内容适合，在角落加署名「AI瑶 · 公众号 码聋」；
6. 只输出 HTML 代码，不要任何解释、不要 markdown 围栏。";

fn poster_user(p: &Value, attempt: u32) -> String {
    let desc = p["description"].as_str().unwrap_or("");
    let w = p["width"].as_u64().unwrap_or(1080);
    let h = p["height"].as_u64().unwrap_or(1440);
    let scene = p["scene"].as_str().unwrap_or("封面");
    let platform = p["platform"].as_str().unwrap_or("wechat");
    let style = if platform == "xhs" {
        "平台：小红书图片笔记。排版基调：明快、种草感、大标题+短句要点，可适度使用 emoji 符号，配色鲜亮但不过饱和。"
    } else {
        "场景：微信公众号图文配图。"
    };
    let mut s = format!("设计要求：{}\n{} 尺寸：{}×{}px。", desc, style, w, h);
    if scene != "封面" && !scene.is_empty() {
        s.push_str(&format!(" 场景：{}。", scene));
    }
    if attempt > 0 {
        s.push_str(&format!(
            "\n\n注意：上一次输出未通过自包含校验，错误：{}。请修正后重新输出完整 HTML。",
            p["_last_err"].as_str().unwrap_or("")
        ));
    }
    s
}

/// Strip markdown fences from a model reply, if present.
fn strip_fence(text: &str) -> &str {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let after_line = rest.find('\n').map(|i| &rest[i + 1..]).unwrap_or(rest);
        return after_line.trim_end().trim_end_matches("```").trim();
    }
    t
}

fn extract_poster_html(text: &str) -> Result<String, String> {
    let t = strip_fence(text);
    let lower = t.to_lowercase();
    if lower.contains("<html") || lower.contains("<body") {
        Ok(t.to_string())
    } else {
        Err("输出中未找到完整 HTML 文档（缺少 <html>/<body>）".into())
    }
}

/// Rasterization runs through SVG foreignObject: ANY external reference
/// breaks it (or silently phones home). Enforce self-containment before we
/// ever try to rasterize, and feed violations back to the model.
fn validate_poster_html(html: &str) -> Result<Vec<String>, String> {
    let lower = html.to_lowercase();
    let mut issues: Vec<String> = Vec::new();
    for (pat, msg) in [
        ("<script", "包含 <script>（不允许脚本）"),
        ("<iframe", "包含 <iframe>（不允许内嵌框架）"),
        ("<link", "包含 <link>（外部字体/样式不允许）"),
        ("@import", "包含 @import（外部样式不允许）"),
        ("<base ", "包含 <base>"),
    ] {
        if lower.contains(pat) {
            issues.push(msg.into());
        }
    }
    for pat in ["url(http", "url('http", "url(\"http"] {
        if lower.contains(pat) {
            issues.push("CSS url() 引用了外部资源（只允许 data URI）".into());
            break;
        }
    }
    // src= must not be http(s); href= may only be http(s) on plain <a>.
    for idx in find_attr_values(&lower, "src=") {
        if idx.starts_with("http") {
            issues.push("存在外部图片/资源 src（图片必须内嵌 data URI）".into());
            break;
        }
    }
    let mut ext_href = 0usize;
    for idx in find_attr_values(&lower, "href=") {
        if idx.starts_with("http") {
            ext_href += 1;
        }
    }
    if ext_href > 0 {
        // count <a tags to see whether every external href is a plain link
        let anchors = lower.matches("<a ").count();
        if ext_href > anchors {
            issues.push("存在非链接的外部 href（如字体/样式）".into());
        }
    }
    if issues.is_empty() {
        Ok(Vec::new())
    } else {
        Err(issues.join("; "))
    }
}

/// Values of `attr="..."` / `attr='...'` occurrences (lowercased input).
fn find_attr_values<'a>(lower: &'a str, attr: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(attr) {
        let start = from + rel + attr.len();
        let bytes = lower.as_bytes();
        if start < bytes.len() {
            let quote = bytes[start];
            if quote == b'"' || quote == b'\'' {
                if let Some(end) = lower[start + 1..].find(quote as char) {
                    out.push(&lower[start + 1..start + 1 + end]);
                    from = start + 1 + end;
                    continue;
                }
            }
        }
        from = start;
    }
    out
}

// -------------------------------------------------------------- runner ----

/// Execute a chat-based task with the generate→extract→validate→repair loop,
/// streaming progress. `finish` persists the artifact and builds the payload.
fn run_chat_task<R: tauri::Runtime>(
    app: &AppHandle<R>,
    job: &Arc<Job>,
    kind: &str,
    params: &Value,
    finish: impl Fn(&str, &Vec<String>) -> Result<Value, String>,
) -> Result<Value, String> {
    let spec = spec(kind);
    let system = (spec.system)();
    let mut last_err = String::new();
    for attempt in 0..spec.attempts {
        if job.cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let mut p = params.clone();
        p["_last_err"] = json!(last_err);
        let user = (spec.user)(&p, attempt);
        let msgs = vec![
            json!({ "role": "system", "content": system }),
            json!({ "role": "user", "content": user }),
        ];
        let content = chat_stream(app, job, &msgs, spec.budget, spec.temperature)?;
        match (spec.extract)(&content)
            .and_then(|artifact| (spec.validate)(&artifact).map(|warnings| (artifact, warnings)))
        {
            Ok((artifact, warnings)) => return finish(&artifact, &warnings),
            Err(e) => {
                last_err = e;
                emit(
                    app,
                    job.id,
                    kind,
                    "status",
                    json!({ "text": format!("第 {} 次输出未通过校验：{}；自动重试", attempt + 1, last_err) }),
                );
            }
        }
    }
    Err(last_err)
}

/// Dispatch a job kind. Runs on the blocking pool; emits done/error itself.
fn run_job(app: AppHandle, job: Arc<Job>, kind: String, params: Value) {
    let started = Instant::now();
    let result: Result<Value, String> = match kind.as_str() {
        "theme" => run_chat_task(&app, &job, "theme", &params, |artifact, warnings| {
            crate::ai::save_theme_artifact(artifact)
                .map(|t| json!({ "id": t["id"], "name": t["name"], "name_zh": t["name_zh"], "file": t["file"], "warnings": warnings }))
        }),
        "svg" => run_chat_task(&app, &job, "svg", &params, |artifact, _w| {
            Ok(json!(artifact))
        }),
        "poster" => run_chat_task(&app, &job, "poster", &params, |artifact, _w| {
            Ok(json!(artifact))
        }),
        "image" => crate::ai::generate_image(
            params["prompt"].as_str().unwrap_or(""),
            params["width"].as_u64().unwrap_or(1024) as u32,
            params["height"].as_u64().unwrap_or(1024) as u32,
        )
        .map(|paths| json!({ "paths": paths })),
        "comfy" => run_comfy(&job, &params),
        other => Err(format!("未知任务类型: {}", other)),
    };
    match result {
        Ok(data) => emit(
            &app,
            job.id,
            &kind,
            "done",
            json!({ "result": data, "elapsed": started.elapsed().as_secs_f32() }),
        ),
        Err(e) if e == CANCELLED => emit(
            &app,
            job.id,
            &kind,
            "done",
            json!({ "stopped": true, "elapsed": started.elapsed().as_secs_f32() }),
        ),
        Err(e) => emit(&app, job.id, &kind, "error", json!(e)),
    }
}

fn run_comfy(job: &Arc<Job>, params: &Value) -> Result<Value, String> {
    let mode = params["mode"].as_str().unwrap_or("t2i");
    let prompt = params["prompt"].as_str().unwrap_or("");
    let negative = params["negative"].as_str().unwrap_or("");
    let steps = params["steps"].as_u64().unwrap_or(20) as u32;
    let paths = match mode {
        "cloud" => crate::ai::generate_image(
            prompt,
            params["width"].as_u64().unwrap_or(1024) as u32,
            params["height"].as_u64().unwrap_or(1024) as u32,
        )?,
        "i2i" => crate::comfy::img2img(
            params["sourcePath"].as_str().unwrap_or(""),
            prompt,
            negative,
            params["denoise"].as_f64().unwrap_or(0.55),
            steps,
            Some(&job.cancel),
        )?,
        _ => crate::comfy::txt2img(
            prompt,
            negative,
            params["width"].as_u64().unwrap_or(512) as u32,
            params["height"].as_u64().unwrap_or(512) as u32,
            steps,
            Some(&job.cancel),
        )?,
    };
    Ok(json!({ "paths": paths }))
}

/// Public entry used by the `ai_job_start` command.
pub fn start_job(kind: &str, params: Value, app: AppHandle) -> Result<u64, String> {
    if !matches!(kind, "theme" | "svg" | "poster" | "image" | "comfy") {
        return Err(format!("未知任务类型: {}", kind));
    }
    if matches!(kind, "theme" | "svg" | "poster") {
        // fail fast when no provider is configured
        crate::ai::active_provider()?;
    }
    let kind_owned = kind.to_string();
    Ok(spawn(kind, app, move |job, app| {
        run_job(app, job, kind_owned, params)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poster_validator_rejects_external_resources() {
        let ok = "<!doctype html><html><body style=\"background:url(data:image/png;base64,xx)\"><a href=\"https://example.com\">link ok</a></body></html>";
        assert!(validate_poster_html(ok).is_ok());
        let bad = vec![
            "<html><body><script>1</script></body></html>",
            "<html><body><img src=\"http://x/y.png\"></body></html>",
            "<html><head><link rel=\"stylesheet\" href=\"https://x\"></head></html>",
            "<html><style>@import url('http://x');</style></html>",
            "<html><style>a{background:url(http://x/y.png)}</style></html>",
            "<html><body><link href=\"https://fonts.x\"></body></html>",
        ];
        for html in bad {
            assert!(
                validate_poster_html(html).is_err(),
                "should reject: {}",
                html
            );
        }
    }

    #[test]
    fn poster_extractor_accepts_document_and_fence() {
        assert!(extract_poster_html("<html><body>x</body></html>").is_ok());
        assert!(extract_poster_html("```html\n<html><body>x</body></html>\n```").is_ok());
        assert!(extract_poster_html("这是一个海报：……（只有说明文字）").is_err());
    }

    #[test]
    fn reasoning_hint_matches_known_models() {
        assert!(is_reasoning_model("glm-z1") || is_reasoning_model("GLM-Z-5"));
        assert!(is_reasoning_model("deepseek-reasoner"));
        assert!(is_reasoning_model("o4-mini"));
        assert!(!is_reasoning_model("deepseek-chat"));
        assert!(!is_reasoning_model("gpt-4o"));
        assert!(!is_reasoning_model("kimi-latest"));
    }

    #[test]
    fn spec_table_covers_all_chat_kinds() {
        for kind in ["theme", "svg", "poster"] {
            let s = spec(kind);
            assert!(s.attempts >= 2 && s.budget >= 1024);
        }
    }

    /// End-to-end smoke against the real AI provider: exercises the theme
    /// engine shared by the sync path AND the jobs.rs button path (same
    /// prompts, budget ladder, strip_think, extract_toml,
    /// validate_generated_theme, retry-feed and save_theme_artifact).
    /// Opt-in because it costs tokens: `cargo test -p wxwright-gui
    /// live_theme_generation_smoke -- --ignored --nocapture`
    /// (run_chat_task itself can't run under cargo test on Windows:
    /// tauri::test::mock_app binaries fail to launch, tauri#11028.)
    #[test]
    #[ignore = "live smoke: calls the real AI provider (needs a configured key)"]
    fn live_theme_generation_smoke() {
        match crate::ai::active_provider() {
            Err(e) => {
                println!("skip: no active AI provider configured: {e}");
                return;
            }
            Ok((p, _)) => println!("live provider: {} / {}", p.name, p.model),
        }
        let result =
            crate::ai::generate_theme("深夜代码风：深色底、青色强调、等宽感标题，适合编程教程文章");
        match result {
            Ok(v) => {
                println!("theme ok: {}", serde_json::to_string_pretty(&v).unwrap());
                let file = v["file"].as_str().expect("file path in result");
                let meta = std::fs::metadata(file).expect("artifact written to user themes dir");
                assert!(meta.len() > 200, "artifact suspiciously small: {file}");
            }
            Err(e) => panic!("live theme generation failed: {e}"),
        }
    }

    /// Live smoke for the AI assistant's article-generation engine via the
    /// sync complete() path (same provider layer and budget ladder the
    /// streaming drawer harness sits on). Costs tokens:
    /// `cargo test -p wxwright-gui live_article_generation_smoke -- --ignored --nocapture`
    #[test]
    #[ignore = "live smoke: calls the real AI provider (needs a configured key)"]
    fn live_article_generation_smoke() {
        match crate::ai::active_provider() {
            Err(e) => {
                println!("skip: no active AI provider configured: {e}");
                return;
            }
            Ok((p, _)) => println!("live provider: {} / {}", p.name, p.model),
        }
        let article = crate::ai::complete(
            "你是微信公众号写作助手。只输出 Markdown 正文，不要任何解释或围栏包裹。",
            "写一篇 250 字左右的短文，主题：为什么写作工具要把排版自动化。要求：一个二级标题、一个三要素列表、结尾一句话总结。",
            4096,
            0.7,
        )
        .expect("article generation should succeed");
        println!("---- generated article ----\n{article}");
        let n = article.chars().count();
        assert!(n > 150, "article suspiciously short: {n} chars");
        assert!(article.contains("##"), "article should contain a heading");
    }

    /// Live matrix smoke for the core loop: generate several WILDLY different
    /// themes (each must pass validate_generated_theme with the retry loop,
    /// land in the user themes dir and be usable by id). Costs tokens:
    /// `cargo test -p wxwright-gui live_theme_matrix_smoke -- --ignored --nocapture`
    #[test]
    #[ignore = "live smoke: calls the real AI provider (needs a configured key)"]
    fn live_theme_matrix_smoke() {
        match crate::ai::active_provider() {
            Err(e) => {
                println!("skip: no active AI provider configured: {e}");
                return;
            }
            Ok((p, _)) => println!("live provider: {} / {}", p.name, p.model),
        }
        let cases = [
            "赛博朋克深空：纯黑底、霓虹紫与电光蓝双强调色、发光描边卡片、等宽字体标题、暗色代码面板",
            "和风静雅：米白和纸底、墨灰正文、朱红印章式强调色、衬线标题、淡雅引用卡",
            "清新草木：淡绿草甸底、深绿正文、藤绿强调色、圆润大圆角卡片、明亮空气感",
        ];
        let mut ids = Vec::new();
        for desc in cases {
            let v = crate::ai::generate_theme(desc)
                .unwrap_or_else(|e| panic!("theme generation failed for {desc}: {e}"));
            let id = v["id"].as_str().expect("theme id").to_string();
            let file = v["file"].as_str().expect("theme file").to_string();
            let meta = std::fs::metadata(&file).expect("theme artifact written");
            assert!(
                meta.len() > 200,
                "theme artifact suspiciously small: {file}"
            );
            println!("matrix theme ok: {id} <- {desc}");
            ids.push(id);
        }
        assert_eq!(ids.len(), 3, "three distinct themes generated");
    }

    /// Diagnose the draft-push image upload chain: runs the exact pipeline
    /// wx_push_draft uses (Upload mode + MP transport) and prints per-image
    /// outcomes. `cargo test -p wxwright-gui diagnostic_push_chain_upload -- --ignored --nocapture`
    #[test]
    #[ignore = "live: uploads a real image to the MP material library (needs bound MP credentials)"]
    fn diagnostic_push_chain_upload() {
        let creds = wxwright_mp::load_credentials().expect("MP credentials bound");
        let md_path = std::path::Path::new(
            "C:/Users/yao/Documents/wxwright/articles/20260927-100000-opensource.md",
        );
        let raw = std::fs::read_to_string(md_path).unwrap();
        let md = wxwright_core::util::strip_frontmatter(&raw).1;
        let mut opts =
            wxwright_core::ConvertOptions::new(crate::commands::load_theme_or_default("magazine"));
        opts.image_mode = wxwright_core::img::ImageMode::Upload;
        opts.transport = Some(std::sync::Arc::new(wxwright_mp::MpClient::new(creds)));
        let result = wxwright_core::pipeline(&md, &opts).expect("pipeline ok");
        println!("images found: {}", result.images.len());
        for img in &result.images {
            println!(
                "source: {} | media_id: {:?} | mmbiz: {} | inlined: {} | warning: {:?}",
                img.source, img.media_id, img.mmbiz, img.inlined, img.warning
            );
        }
        println!(
            "blocking violations: {}",
            result.blocking_violations().len()
        );
    }
}
