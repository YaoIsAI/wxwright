//! wxwright MCP server: local stdio JSON-RPC server exposing the engine to
//! AI agents (PRD 7). Hand-rolled against the MCP stdio transport contract:
//! newline-delimited JSON-RPC 2.0 messages on stdin/stdout.
//!
//! Language contract (PRD 3.7-B): tool names, parameters, descriptions and
//! the machine-readable parts of responses are English constants. Resources
//! carry bilingual content where useful.

pub mod install;

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use wxwright_core::theme;
use wxwright_core::{pipeline, ConvertOptions};

pub const SERVER_NAME: &str = "wxwright";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Protocol version we speak; we accept and mirror the client's when known.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Serve MCP over the given reader/writer until EOF. Returns the number of
/// requests served (useful for tests).
pub fn serve<R: BufRead, W: Write>(input: R, output: &mut W) -> std::io::Result<usize> {
    let mut served = 0usize;
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                writeln!(output, "{}", err)?;
                output.flush()?;
                continue;
            }
        };
        let method = req
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let id = req.get("id").cloned();
        let is_notification = id.is_none();

        // Notifications (no id) never get a response.
        if is_notification {
            if method == "initialized" || method == "notifications/initialized" {
                continue;
            }
            continue;
        }

        // A handler panic must not drop the agent's session: catch it per
        // request and answer with a JSON-RPC internal error instead.
        let method_d = method.clone();
        let params_d = req.get("params").cloned();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            dispatch(&method_d, params_d.as_ref())
        }))
        .unwrap_or_else(|_| Err((-32603, "internal error: handler panicked".to_string())));
        let response = match result {
            Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
            Err((code, msg)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": msg }
            }),
        };
        writeln!(output, "{}", response)?;
        output.flush()?;
        served += 1;
        if method == "shutdown" {
            break;
        }
    }
    Ok(served)
}

type DispatchResult = Result<Value, (i64, String)>;

fn dispatch(method: &str, params: Option<&Value>) -> DispatchResult {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str())
                .unwrap_or(PROTOCOL_VERSION),
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": {},
                "prompts": {}
            },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list()),
        "tools/call" => tools_call(params),
        "resources/list" => Ok(resources_list()),
        "resources/read" => resources_read(params),
        "prompts/list" => Ok(prompts_list()),
        "prompts/get" => prompts_get(params),
        "shutdown" => Ok(json!({})),
        _ => Err((-32601, format!("Method not found: {}", method))),
    }
}

fn text_result(v: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&v).unwrap_or_default() }],
        "structuredContent": v
    })
}

/// Tools the GUI's assistant may call: the read-only ones.
///
/// The side-effectful tools - clipboard write, material upload, draft create -
/// are deliberately not offered to the model. An assistant that can push to the
/// user's 公众号 on its own is not an assistant. What makes tools worth having
/// here is self-checking (validate / convert / export), which the read-only set
/// covers completely.
pub const ASSISTANT_TOOLS: &[&str] = &[
    "wxwright_validate",
    "wxwright_convert",
    "wxwright_export",
    "wxwright_themes_list",
    "wxwright_draft_list",
];

/// The OpenAI `tools` array for `names`, in that order.
///
/// Built from `tools_list()` so a tool's description and schema are declared
/// once and cannot drift between the MCP surface and the assistant's view of
/// it. A name that is not declared fails loudly instead of being dropped.
pub fn openai_tool_schemas(names: &[&str]) -> Result<Vec<serde_json::Value>, String> {
    let listed = tools_list()["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    names
        .iter()
        .map(|n| {
            let t = listed
                .iter()
                .find(|t| t.get("name").and_then(|x| x.as_str()) == Some(*n))
                .ok_or_else(|| format!("tool {n} is not declared in this server"))?;
            Ok(serde_json::json!({
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["inputSchema"],
                }
            }))
        })
        .collect()
}

/// Run one tool in-process and return what the model should read.
///
/// Same dispatch as the stdio `tools/call` path, minus the JSON-RPC envelope:
/// one implementation, two consumers. Two deliberate differences from the stdio
/// path:
/// - the MCP wrapper (`content` / `structuredContent`) is unwrapped, because the
///   model wants the payload and not a JSON string of itself inside a JSON
///   string;
/// - it never errors. A failed tool becomes a readable failure message, which is
///   what a harness needs - the model sees why it failed and can adapt. The
///   payload is truncated so a full article of HTML cannot eat the context
///   window in one tool result.
pub fn call_tool_for_model(name: &str, arguments: &serde_json::Value) -> String {
    const MAX_TOOL_CHARS: usize = 6_000;
    let params = serde_json::json!({ "name": name, "arguments": arguments });
    let envelope = tools_call(Some(&params)).unwrap_or_else(|(code, msg)| {
        text_result(serde_json::json!({
            "isError": true,
            "error": format!("code {code}: {msg}"),
        }))
    });
    let payload = envelope
        .get("structuredContent")
        .cloned()
        .unwrap_or(envelope);
    let failed = payload
        .get("isError")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let text = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let text = wxwright_core::util::truncate(&text, MAX_TOOL_CHARS);
    if failed {
        format!("工具 {name} 执行失败：{text}")
    } else {
        text
    }
}

fn tools_list() -> Value {
    json!({
        "tools": [
            tool(
                "wxwright_convert",
                "Convert Markdown to WeChat MP compliant dialect HTML. Returns html plus warnings, image outcomes and stats.",
                json!({
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string", "description": "Markdown source (GFM). Required." },
                        "theme": { "type": "string", "description": "Theme id (minimal|techblue|magazine) or path to a theme TOML. Default: minimal." },
                        "image_mode": { "type": "string", "enum": ["inline", "keep"], "description": "inline: local images become base64 (file export). keep: URLs kept as-is. Default: inline." }
                    },
                    "required": ["markdown"]
                })
            ),
            tool(
                "wxwright_validate",
                "Validate Markdown or HTML against the official WeChat editor spec. Returns violations array; empty = compliant.",
                json!({
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string", "description": "Markdown source (converted with default theme first)." },
                        "html": { "type": "string", "description": "HTML source (validated directly). One of markdown/html required." }
                    }
                })
            ),
            tool(
                "wxwright_copy",
                "Full chain: parse, render, normalize, validate, write rich text/html to the user's clipboard. The user then pastes into the MP editor. Requires a desktop session.",
                json!({
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string" },
                        "theme": { "type": "string" },
                        "dry_run": { "type": "boolean", "description": "Run the chain but skip the clipboard write." }
                    },
                    "required": ["markdown"]
                })
            ),
            tool(
                "wxwright_themes_list",
                "List built-in themes with metadata.",
                json!({ "type": "object", "properties": {} })
            ),
            tool(
                "wxwright_upload_images",
                "Upload image files to the WeChat MP permanent material library and return mmbiz URLs (requires credentials from `wxwright login`). Decouples image upload from draft creation for unattended publishing.",
                json!({
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Absolute or relative local image file paths."
                        }
                    },
                    "required": ["paths"]
                })
            ),
            tool(
                "wxwright_draft_create",
                "Create a draft in the WeChat MP 草稿箱 (Drafts). Requires configured API credentials (wxwright login) and image mmbiz upload.",
                json!({
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string" },
                        "title": { "type": "string" },
                        "author": { "type": "string" },
                        "digest": { "type": "string" },
                        "theme": { "type": "string" }
                    },
                    "required": ["markdown", "title"]
                })
            ),
            tool(
                "wxwright_draft_list",
                "List existing drafts (requires credentials).",
                json!({
                    "type": "object",
                    "properties": {
                        "offset": { "type": "integer", "default": 0 },
                        "count": { "type": "integer", "default": 10 }
                    }
                })
            ),
            tool(
                "wxwright_draft_delete",
                "Delete one draft from the WeChat MP 草稿箱 by media_id (get ids from wxwright_draft_list). Requires credentials.",
                json!({
                    "type": "object",
                    "properties": {
                        "media_id": { "type": "string" }
                    },
                    "required": ["media_id"]
                })
            ),
            tool(
                "wxwright_export",
                "Export one Markdown source for a specific platform. Returns the artifact that platform actually consumes: WeChat gets dialect rich text (html), Xiaohongshu/Facebook/Instagram/X/LinkedIn get a plain-text caption, Zhihu gets Markdown unchanged. Use `wxwright platforms` for the id list.",
                json!({
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string", "description": "Markdown source (GFM). Required." },
                        "platform": { "type": "string", "description": "Platform id: wechat | xhs | zhihu | meta | instagram | x | linkedin. Default: wechat. An unknown id is reported via platform_known=false." },
                        "title": { "type": "string", "description": "Article title, used for caption-style exports." },
                        "theme": { "type": "string", "description": "Theme id or TOML path (WeChat dialect only). Default: minimal." }
                    },
                    "required": ["markdown"]
                })
            )
        ]
    })
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn tools_call(params: Option<&Value>) -> DispatchResult {
    let params = params.ok_or((-32602, "Missing params".to_string()))?;
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let get_str = |k: &str| args.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
    let theme = get_str("theme").unwrap_or_else(|| "minimal".to_string());

    let result = (|| -> Result<Value, String> {
        let t = theme::load_theme(&theme).map_err(|e| e.to_string())?;
        match name {
            "wxwright_convert" => {
                let md = get_str("markdown").ok_or("missing argument: markdown")?;
                let mode = match get_str("image_mode").as_deref() {
                    Some("keep") => wxwright_core::img::ImageMode::Keep,
                    _ => wxwright_core::img::ImageMode::Inline,
                };
                let opts = ConvertOptions {
                    theme: t,
                    image_mode: mode,
                    base_dir: None,
                    transport: None,
                };
                let out = wxwright_core::convert_markdown(&md, &opts).map_err(|e| e.to_string())?;
                let mut v = serde_json::to_value(&out).map_err(|e| e.to_string())?;
                if out.html.len() > 64 * 1024 {
                    v["html_truncated"] = json!(true);
                    v["note"] = json!(
                        "html exceeds 64KB; use wxwright CLI convert --out for the full artifact"
                    );
                }
                Ok(v)
            }
            "wxwright_validate" => {
                let md = get_str("markdown");
                let html_arg = get_str("html");
                let html = if let Some(h) = html_arg {
                    h
                } else if let Some(md) = md {
                    let opts = ConvertOptions::new(t);
                    wxwright_core::convert_markdown(&md, &opts)
                        .map_err(|e| e.to_string())?
                        .html
                } else {
                    return Err("provide markdown or html".into());
                };
                let violations = wxwright_core::validator::validate_html(&html);
                let blocking = violations.iter().filter(|v| v.is_block()).count();
                Ok(json!({
                    "compliant": blocking == 0,
                    "blocking_count": blocking,
                    "violations": violations,
                    "rules_reference": "wxwright://spec/rules"
                }))
            }
            "wxwright_copy" => {
                let md = get_str("markdown").ok_or("missing argument: markdown")?;
                let dry = args
                    .get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let opts = ConvertOptions::new(t);
                let out = pipeline(&md, &opts).map_err(|e| e.to_string())?;
                let blocking: Vec<_> = out.blocking_violations();
                if !blocking.is_empty() {
                    return Ok(json!({
                        "copied": false,
                        "reason": "blocking violations; fix the markdown first",
                        "violations": blocking,
                    }));
                }
                // The one predicate (AGENTS.md iron law 12): a hand-rolled
                // `inlined && !mmbiz` here silently let plain-http URLs and
                // images that failed to load through, while the CLI blocked
                // them - same input, two different answers.
                let local_images: Vec<_> = out
                    .images
                    .iter()
                    .filter(|i| i.paste_hostile())
                    .map(|i| i.source.clone())
                    .collect();
                if !local_images.is_empty() && !dry {
                    return Ok(json!({
                        "copied": false,
                        "reason": "clipboard copy requires mmbiz or https images (PRD I-03); local images found",
                        "local_images": local_images,
                        "hint": "configure credentials with `wxwright login`, or export a file instead"
                    }));
                }
                if dry {
                    return Ok(
                        json!({ "copied": false, "dry_run": true, "warnings": out.violations, "images": out.images, "stats": out.stats }),
                    );
                }
                let outcome = wxwright_core::clipboard::copy_to_clipboard(&out.html)
                    .map_err(|e| e.to_string())?;
                Ok(json!({
                    "copied": true,
                    "html_flavor": outcome.html_flavor,
                    "warnings": out.violations,
                    "images": out.images,
                    "stats": out.stats,
                    "next_step": "user opens the MP editor and pastes (Ctrl+V)"
                }))
            }
            "wxwright_themes_list" => {
                let themes: Vec<Value> = theme::BUILTIN_THEMES
                    .iter()
                    .filter_map(|(_, src)| theme::parse_theme(src).ok())
                    .map(|t| serde_json::to_value(&t.meta).unwrap_or_default())
                    .collect();
                Ok(json!({ "themes": themes }))
            }
            "wxwright_draft_create" => {
                let md = get_str("markdown").ok_or("missing argument: markdown")?;
                let title = get_str("title").ok_or("missing argument: title")?;
                let creds = wxwright_mp::load_credentials()
                    .ok_or("no credentials configured; run `wxwright login` first")?;
                let client = std::sync::Arc::new(wxwright_mp::MpClient::new(creds));
                let mut opts = ConvertOptions::new(t);
                opts.image_mode = wxwright_core::img::ImageMode::Upload;
                opts.transport = Some(client.clone());
                let out = pipeline(&md, &opts).map_err(|e| e.to_string())?;
                // This handler used to push straight to the drafts box with no
                // checks at all - no blocking-violation gate and no image gate,
                // so an agent could create a draft that the MP editor renders
                // with broken images and rule violations. Both gates now match
                // the GUI push path.
                let blocking = out.blocking_violations();
                if !blocking.is_empty() {
                    let list = blocking
                        .iter()
                        .map(|v| format!("{} {}", v.rule_id, v.message))
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(format!(
                        "{} blocking violations; fix the markdown first: {}",
                        blocking.len(),
                        list
                    ));
                }
                let hostile: Vec<String> = out
                    .images
                    .iter()
                    .filter(|i| i.paste_hostile())
                    .map(|i| i.source.clone())
                    .collect();
                if !hostile.is_empty() {
                    return Err(format!(
                        "{} image(s) would render broken in the MP editor (mmbiz or https required); draft not created: {}",
                        hostile.len(),
                        hostile.join("; ")
                    ));
                }
                let thumb = out.images.iter().find_map(|i| i.media_id.clone()).ok_or(
                    "no uploaded image available for thumb_media_id; include at least one image",
                )?;
                let article = wxwright_mp::DraftArticle {
                    title,
                    author: get_str("author").unwrap_or_default(),
                    digest: get_str("digest").unwrap_or_default(),
                    content_html: out.html.clone(),
                    content_source_url: String::new(),
                    thumb_media_id: thumb,
                };
                let media_id = client.draft_add(&article).map_err(|e| e.to_string())?;
                Ok(
                    json!({ "draft_media_id": media_id, "images": out.images, "violations": out.violations }),
                )
            }
            "wxwright_upload_images" => {
                let paths: Vec<String> = args
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                if paths.is_empty() {
                    return Err("missing argument: paths (non-empty array of image files)".into());
                }
                let creds = wxwright_mp::load_credentials()
                    .ok_or("no credentials configured; run `wxwright login` first")?;
                let client = wxwright_mp::MpClient::new(creds);
                let mut uploads = Vec::new();
                let mut all_ok = true;
                for p in &paths {
                    let bytes =
                        std::fs::read(p).map_err(|e| format!("cannot read {}: {}", p, e))?;
                    let name = std::path::Path::new(p)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "image.png".into());
                    match client.upload_material(bytes, &name) {
                        Ok((media_id, url)) => uploads.push(
                            json!({ "source": p, "media_id": media_id, "url": url, "ok": true }),
                        ),
                        Err(e) => {
                            all_ok = false;
                            uploads
                                .push(json!({ "source": p, "ok": false, "error": e.to_string() }));
                        }
                    }
                }
                Ok(json!({ "ok": all_ok, "uploads": uploads }))
            }
            "wxwright_export" => {
                let md = get_str("markdown").ok_or("missing argument: markdown")?;
                let platform = get_str("platform").unwrap_or_else(|| "wechat".into());
                let (spec, known) = wxwright_core::platform::resolve_platform(&platform);
                let title_arg = get_str("title").unwrap_or_default();
                let title = if title_arg.trim().is_empty() {
                    None
                } else {
                    Some(title_arg.as_str())
                };
                // Rich-text platforms need a theme and the image pipeline, which
                // are host concerns, so they stay here; the caption and Markdown
                // artifacts come from core so this tool cannot drift from the
                // CLI's `convert --platform`.
                match wxwright_core::platform::export_text_artifact(spec.id, &md, title) {
                    Some((artifact, violations)) => Ok(json!({
                        "platform": spec.id,
                        "platform_known": known,
                        "export_kind": artifact.kind(),
                        "text": artifact.text(),
                        "violations": violations,
                    })),
                    None => {
                        let opts = ConvertOptions::new(t);
                        let out = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
                        let blocking = out.blocking_violations().len();
                        Ok(json!({
                            "platform": spec.id,
                            "platform_known": known,
                            "export_kind": "rich_text_dialect",
                            "html": out.html,
                            "blocking_violations": blocking,
                            "warnings": out.violations.iter().filter(|v| !v.is_block()).count(),
                        }))
                    }
                }
            }
            "wxwright_draft_list" => {
                let creds = wxwright_mp::load_credentials()
                    .ok_or("no credentials configured; run `wxwright login` first")?;
                let client = wxwright_mp::MpClient::new(creds);
                let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
                let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
                client.draft_list(offset, count).map_err(|e| e.to_string())
            }
            "wxwright_draft_delete" => {
                let creds = wxwright_mp::load_credentials()
                    .ok_or("no credentials configured; run `wxwright login` first")?;
                let client = wxwright_mp::MpClient::new(creds);
                let media_id = args
                    .get("media_id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .ok_or("media_id is required (see wxwright_draft_list)")?;
                client
                    .draft_delete(media_id.trim())
                    .map(|_| json!({ "deleted": media_id.trim() }))
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("unknown tool: {}", name)),
        }
    })();

    match result {
        Ok(v) => Ok(text_result(v)),
        Err(e) => Ok(text_result(json!({ "isError": true, "error": e }))),
    }
}

fn resources_list() -> Value {
    json!({
        "resources": [
            {
                "uri": "wxwright://themes",
                "name": "Built-in themes",
                "description": "Theme metadata for minimal / techblue / magazine",
                "mimeType": "application/json"
            },
            {
                "uri": "wxwright://spec/rules",
                "name": "Official editor spec rules (wxwright 5.3 mapping)",
                "description": "The rule table the engine enforces; bilingual (en + zh-CN). Feed this to the agent so it knows the red lines.",
                "mimeType": "text/markdown"
            }
        ]
    })
}

fn resources_read(params: Option<&Value>) -> DispatchResult {
    let uri = params
        .and_then(|p| p.get("uri"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match uri {
        "wxwright://themes" => {
            let themes: Vec<Value> = theme::BUILTIN_THEMES
                .iter()
                .filter_map(|(_, src)| theme::parse_theme(src).ok())
                .map(|t| serde_json::to_value(&t.meta).unwrap_or_default())
                .collect();
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string(&json!({ "themes": themes })).unwrap_or_default()
                }]
            }))
        }
        "wxwright://spec/rules" => {
            let mut md =
                String::from("# wxwright rule table (official WeChat editor spec mapping)\n\n");
            for r in wxwright_core::rules::RULES {
                md.push_str(&format!(
                    "- **{}** [{}]{}\n  - en: {}\n  - zh: {}\n",
                    r.id,
                    match r.severity {
                        wxwright_core::rules::Severity::Block => "BLOCK",
                        wxwright_core::rules::Severity::Warn => "WARN",
                    },
                    if r.auto_fixable {
                        " (auto-fixable)"
                    } else {
                        ""
                    },
                    r.en,
                    r.zh
                ));
            }
            Ok(json!({
                "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": md }]
            }))
        }
        _ => Err((-32602, format!("Unknown resource: {}", uri))),
    }
}

fn prompts_list() -> Value {
    json!({
        "prompts": [
            {
                "name": "wxwright-publish-guide",
                "description": "Standard flow for an agent to publish an article to WeChat MP via wxwright",
                "arguments": [
                    { "name": "markdown", "description": "The article in Markdown", "required": true }
                ]
            }
        ]
    })
}

fn prompts_get(params: Option<&Value>) -> DispatchResult {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match name {
        "wxwright-publish-guide" => {
            let md = params
                .and_then(|p| p.get("arguments"))
                .and_then(|a| a.get("markdown"))
                .and_then(|v| v.as_str())
                .unwrap_or("<article markdown>");
            Ok(json!({
                "description": "wxwright one-shot publishing flow",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Publish this article to WeChat MP:\n\n1. Validate it: wxwright_validate(markdown=...) - fix any blocking violations in the source first.\n2. Copy to clipboard: wxwright_copy(markdown=..., theme=\"minimal\") - tell the user to paste into the MP editor, OR create a draft via wxwright_draft_create if credentials exist.\n3. Report warnings and per-image status from the JSON result. Never fabricate image URLs; images must be mmbiz or reachable https.\n\nArticle:\n\n{}",
                            md
                        )
                    }
                }]
            }))
        }
        _ => Err((-32602, format!("Unknown prompt: {}", name))),
    }
}

/// Entry point used by `wxwright mcp serve`.
pub fn run_stdio() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let reader = stdin.lock();
    serve(reader, &mut out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(lines: &[String]) -> Vec<Value> {
        let input = lines.join("\n");
        let mut out = Vec::new();
        serve(std::io::Cursor::new(input), &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// The GUI assistant calls tools in-process, so the dispatch has to work
    /// without a stdio envelope - and the read-only tool set is the contract:
    /// adding a side-effectful tool to ASSISTANT_TOOLS fails here.
    #[test]
    fn assistant_tools_dispatch_in_process_and_stay_read_only() {
        // A real validation round-trip, no subprocess involved. The MCP
        // envelope must be unwrapped: the model gets the payload, not a JSON
        // string of the payload inside a JSON string.
        let out = call_tool_for_model(
            "wxwright_validate",
            &serde_json::json!({ "markdown": "# 标题\n\n正文一段。" }),
        );
        assert!(out.contains("compliant"), "got: {out}");
        assert!(out.contains("violations"), "got: {out}");
        assert!(
            !out.contains("structuredContent"),
            "the envelope must be unwrapped: {out}"
        );

        // A failed tool becomes a readable failure, not an Err: the model has
        // to see why it failed so it can adapt.
        let err = call_tool_for_model("wxwright_nope", &serde_json::json!({}));
        assert!(err.contains("执行失败"), "got: {err}");
        assert!(err.contains("unknown tool"), "got: {err}");

        // The side-effectful tools must never be offered to the model.
        for dangerous in [
            "wxwright_copy",
            "wxwright_upload_images",
            "wxwright_draft_create",
            "wxwright_draft_delete",
        ] {
            assert!(
                !crate::ASSISTANT_TOOLS.contains(&dangerous),
                "{dangerous} has side effects and must not be exposed to the assistant"
            );
        }

        // And every name in ASSISTANT_TOOLS must actually be declared, so the
        // schema request cannot fail at chat time.
        let schemas = openai_tool_schemas(crate::ASSISTANT_TOOLS)
            .expect("every assistant tool must be declared");
        assert_eq!(schemas.len(), crate::ASSISTANT_TOOLS.len());
        for (schema, name) in schemas.iter().zip(crate::ASSISTANT_TOOLS) {
            assert_eq!(schema["type"], "function");
            assert_eq!(schema["function"]["name"], *name);
        }
    }

    #[test]
    fn tool_surface_matches_agent_card() {
        let impl_names: Vec<String> = tools_list()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        let card = wxwright_core::agentcard::card_json();
        let card_names: Vec<String> = card["mcp"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            impl_names, card_names,
            "MCP tool surface drifted from the agent card contract"
        );
    }

    #[test]
    fn unknown_tool_names_are_errors_not_panics() {
        let responses = roundtrip(&[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wxwright_upload_images","arguments":{}}}).to_string(),
        ]);
        let sc = &responses[0]["result"]["structuredContent"];
        assert_eq!(sc["isError"], json!(true));
    }

    #[test]
    fn initialize_and_tools() {
        let responses = roundtrip(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}).to_string(),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string(),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"wxwright_themes_list","arguments":{}}}).to_string(),
        ]);
        assert_eq!(responses.len(), 3, "notifications get no response");
        let init = &responses[0];
        assert_eq!(init["result"]["serverInfo"]["name"], "wxwright");
        let tools = &responses[1]["result"]["tools"];
        assert!(tools.as_array().unwrap().len() >= 4);
        let themes = &responses[2]["result"]["structuredContent"]["themes"];
        assert_eq!(themes.as_array().unwrap().len(), 3);
    }

    #[test]
    fn convert_tool_and_validate_tool() {
        let responses = roundtrip(&[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wxwright_convert","arguments":{"markdown":"# Hi\n\nbody with [link](https://a.com)"}}}).to_string(),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wxwright_validate","arguments":{"html":"<p style=\"font-family: serif\">x</p>"}}}).to_string(),
            json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"wxwright://spec/rules"}}).to_string(),
        ]);
        let conv = &responses[0]["result"]["structuredContent"];
        assert!(conv["html"].as_str().unwrap().contains("<section"));
        let val = &responses[1]["result"]["structuredContent"];
        assert_eq!(val["compliant"], json!(false));
        let rules_text = &responses[2]["result"]["contents"][0]["text"];
        assert!(rules_text.as_str().unwrap().contains("R-3.1"));
    }

    #[test]
    fn unknown_method_and_prompt() {
        let responses = roundtrip(&[
            json!({"jsonrpc":"2.0","id":1,"method":"nope"}).to_string(),
            json!({"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"wxwright-publish-guide","arguments":{"markdown":"# t"}}}).to_string(),
        ]);
        assert_eq!(responses[0]["error"]["code"], -32601);
        assert!(responses[1]["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("wxwright_copy"));
    }
}
