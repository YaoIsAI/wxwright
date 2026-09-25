//! Tauri commands: thin wrappers over the core pipeline.
//! Image handling in the GUI: preview/export inline local files as base64;
//! clipboard copy enforces the I-03 paste-safety rule.

use serde::Serialize;
use wxwright_core::img::ImageMode;
use wxwright_core::theme::{self, Theme};
use wxwright_core::{clipboard, convert_markdown, pipeline, wrap_document, ConvertOptions};

#[derive(Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub name_zh: String,
    pub description: String,
    pub description_zh: String,
}

#[tauri::command]
pub fn list_themes() -> Vec<ThemeInfo> {
    let mut all: Vec<ThemeInfo> = theme::builtin_themes()
        .unwrap_or_default()
        .into_iter()
        .map(|t| ThemeInfo {
            id: t.meta.id.clone(),
            name: t.meta.name.clone(),
            name_zh: t.meta.name_zh.clone(),
            description: t.meta.description.clone(),
            description_zh: t.meta.description_zh.clone(),
        })
        .collect();
    for t in theme::list_user_themes() {
        all.push(ThemeInfo {
            id: t.meta.id.clone(),
            name: t.meta.name.clone(),
            name_zh: t.meta.name_zh.clone(),
            description: t.meta.description.clone(),
            description_zh: t.meta.description_zh.clone(),
        });
    }
    all
}

fn load_theme_or_default(id: &str) -> Theme {
    theme::load_theme(id)
        .unwrap_or_else(|_| theme::load_builtin_theme("minimal").expect("builtin minimal"))
}

#[derive(Serialize)]
pub struct PreviewResult {
    pub html: String,
    pub stats: wxwright_core::ir::Stats,
    pub violations: Vec<wxwright_core::validator::Violation>,
    pub fixes: Vec<wxwright_core::normalizer::FixRecord>,
    pub image_warnings: Vec<String>,
}

#[tauri::command]
pub fn convert_preview(markdown: String, theme_id: String) -> Result<PreviewResult, String> {
    let opts = ConvertOptions {
        theme: load_theme_or_default(&theme_id),
        image_mode: ImageMode::Inline,
        base_dir: None,
        transport: None,
    };
    let out = pipeline(&markdown, &opts).map_err(|e| e.to_string())?;
    Ok(PreviewResult {
        html: out.html,
        stats: out.stats,
        violations: out.violations,
        fixes: out.fixes,
        image_warnings: out
            .images
            .iter()
            .filter_map(|i| i.warning.clone())
            .collect(),
    })
}

#[derive(Serialize)]
pub struct CopyResult {
    pub copied: bool,
    pub html_flavor: bool,
    pub reason: Option<String>,
    pub blocking: Vec<wxwright_core::validator::Violation>,
    pub paste_hostile_images: Vec<String>,
}

#[tauri::command]
pub fn copy_rich(markdown: String, theme_id: String) -> CopyResult {
    let opts = ConvertOptions {
        theme: load_theme_or_default(&theme_id),
        image_mode: ImageMode::Keep,
        base_dir: None,
        transport: None,
    };
    let out = match pipeline(&markdown, &opts) {
        Ok(o) => o,
        Err(e) => {
            return CopyResult {
                copied: false,
                html_flavor: false,
                reason: Some(e.to_string()),
                blocking: Vec::new(),
                paste_hostile_images: Vec::new(),
            }
        }
    };
    let blocking: Vec<_> = out.blocking_violations().into_iter().cloned().collect();
    if !blocking.is_empty() {
        return CopyResult {
            copied: false,
            html_flavor: false,
            reason: Some("blocking violations".into()),
            blocking,
            paste_hostile_images: Vec::new(),
        };
    }
    let paste_hostile: Vec<String> = out
        .images
        .iter()
        .filter(|i| (i.inlined && !i.mmbiz) || i.source.starts_with("data:"))
        .map(|i| i.source.clone())
        .collect();
    if !paste_hostile.is_empty() {
        return CopyResult {
            copied: false,
            html_flavor: false,
            reason: Some("paste-hostile images".into()),
            blocking: Vec::new(),
            paste_hostile_images: paste_hostile,
        };
    }
    match clipboard::copy_to_clipboard(&out.html) {
        Ok(o) => CopyResult {
            copied: true,
            html_flavor: o.html_flavor,
            reason: None,
            blocking: Vec::new(),
            paste_hostile_images: Vec::new(),
        },
        Err(e) => CopyResult {
            copied: false,
            html_flavor: false,
            reason: Some(e.to_string()),
            blocking: Vec::new(),
            paste_hostile_images: Vec::new(),
        },
    }
}

#[tauri::command]
pub fn export_html(markdown: String, theme_id: String, path: String) -> Result<String, String> {
    let opts = ConvertOptions {
        theme: load_theme_or_default(&theme_id),
        image_mode: ImageMode::Inline,
        base_dir: None,
        transport: None,
    };
    let out = convert_markdown(&markdown, &opts).map_err(|e| e.to_string())?;
    let doc = wrap_document(&out.html);
    std::fs::write(&path, doc).map_err(|e| format!("write failed: {}", e))?;
    Ok(path)
}

#[tauri::command]
pub fn validate_md(
    markdown: String,
    theme_id: String,
) -> Result<Vec<wxwright_core::validator::Violation>, String> {
    let opts = ConvertOptions::new(load_theme_or_default(&theme_id));
    let out = pipeline(&markdown, &opts).map_err(|e| e.to_string())?;
    Ok(out.violations)
}

#[tauri::command]
pub fn agent_card_markdown() -> String {
    wxwright_core::agentcard::card_markdown()
}

#[tauri::command]
pub fn copy_agent_card() -> Result<bool, String> {
    let card = wxwright_core::agentcard::card_markdown();
    wxwright_core::clipboard::copy_text(&card).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn mcp_install(target: String) -> Result<wxwright_mcp::install::InstallOutcome, String> {
    wxwright_mcp::install::install(&target)
}

// ------------------------------------------------------ article library ---

use crate::articles::ArticleMeta;

#[tauri::command]
pub fn list_articles() -> Vec<ArticleMeta> {
    crate::articles::list_articles()
}

#[tauri::command]
pub fn library_dir() -> String {
    crate::articles::library_dir().to_string_lossy().to_string()
}

#[tauri::command]
pub fn read_article(id: String) -> Result<crate::articles::Article, String> {
    crate::articles::read_article(&id)
}

#[tauri::command]
pub fn save_article(
    id: Option<String>,
    title: String,
    theme: String,
    platform: Option<String>,
    markdown: String,
) -> Result<ArticleMeta, String> {
    crate::articles::save_article(id, &title, &theme, platform.as_deref().unwrap_or("wechat"), &markdown)
}

// ------------------------------------------------ platform export adapter --
/// Render the article for the active platform's primary copy action.
/// WeChat keeps its dedicated rich-text copy path; other platforms get
/// plain text / Markdown produced by the core export adapters.
#[tauri::command]
pub fn platform_export_text(
    platform: String,
    title: String,
    markdown: String,
) -> Result<String, String> {
    match wxwright_core::platform::export_kind(&platform) {
        wxwright_core::platform::ExportKind::RichTextDialect => {
            Err("wechat uses the dedicated rich-text copy".into())
        }
        wxwright_core::platform::ExportKind::Markdown => Ok(markdown),
        wxwright_core::platform::ExportKind::Caption => {
            let doc = wxwright_core::parser::parse_markdown(&markdown);
            Ok(wxwright_core::platform::render_caption(&doc, Some(&title)))
        }
    }
}

/// Per-platform preview model: WeChat keeps the dialect article; image-note
/// platforms get a note model (caption + resolved image list) rendered into
/// a feed-style shell by the frontend; Markdown-friendly hosts get plain
/// typographic HTML. This is the "same article, different channel shape" seam.
#[tauri::command]
pub fn platform_preview(
    platform: String,
    title: String,
    markdown: String,
    theme_id: String,
) -> Result<serde_json::Value, String> {
    match wxwright_core::platform::export_kind(&platform) {
        wxwright_core::platform::ExportKind::RichTextDialect => {
            Ok(serde_json::json!({ "mode": "article" }))
        }
        wxwright_core::platform::ExportKind::Markdown => {
            let doc = wxwright_core::parser::parse_markdown(&markdown);
            // resolve images through the same pipeline the dialect uses
            let opts = ConvertOptions {
                theme: load_theme_or_default(&theme_id),
                image_mode: ImageMode::Inline,
                base_dir: None,
                transport: None,
            };
            let out = pipeline(&markdown, &opts).map_err(|e| e.to_string())?;
            let resolved: Vec<String> = out
                .images
                .iter()
                .map(|i| i.final_src.clone())
                .collect();
            Ok(serde_json::json!({
                "mode": "plain",
                "html": wxwright_core::platform::render_plain_html(&doc, &resolved),
            }))
        }
        wxwright_core::platform::ExportKind::Caption => {
            let opts = ConvertOptions {
                theme: load_theme_or_default(&theme_id),
                image_mode: ImageMode::Inline,
                base_dir: None,
                transport: None,
            };
            let out = pipeline(&markdown, &opts).map_err(|e| e.to_string())?;
            let images: Vec<String> = out
                .images
                .iter()
                .map(|i| i.final_src.clone())
                .filter(|s| !s.is_empty())
                .collect();
            let doc = wxwright_core::parser::parse_markdown(&markdown);
            let caption = wxwright_core::platform::render_caption(&doc, Some(&title));
            Ok(serde_json::json!({
                "mode": "note",
                "platform": platform,
                "title": title,
                "caption": caption,
                "images": images,
            }))
        }
    }
}

/// Platform-specific rule table (PRD §16). WeChat articles are validated by
/// the dialect engine; Xiaohongshu captions get their own limits here.
#[tauri::command]
pub fn platform_validate(
    platform: String,
    title: String,
    markdown: String,
    images: usize,
) -> Result<serde_json::Value, String> {
    match wxwright_core::platform::export_kind(&platform) {
        wxwright_core::platform::ExportKind::RichTextDialect => {
            Ok(serde_json::json!({ "mode": "dialect" }))
        }
        mode => {
            let doc = wxwright_core::parser::parse_markdown(&markdown);
            let cap = wxwright_core::platform::render_caption(&doc, Some(&title));
            let violations = wxwright_core::platform::validate_platform_caption(
                &platform,
                &title,
                &cap,
                images,
            );
            let _ = mode;
            Ok(serde_json::json!({ "mode": "platform", "violations": violations }))
        }
    }
}

#[tauri::command]
pub fn delete_article(id: String) -> Result<bool, String> {
    crate::articles::delete_article(&id)
}

// -------------------------------------------------------------- AI layer ---

#[tauri::command]
pub fn ai_settings() -> serde_json::Value {
    crate::ai::settings()
}

#[tauri::command]
pub fn ai_save_provider(provider: serde_json::Value) -> Result<serde_json::Value, String> {
    let p: crate::ai::Provider = serde_json::from_value(provider).map_err(|e| e.to_string())?;
    crate::ai::save_provider(p, "")
}

#[tauri::command]
pub fn ai_save_provider_with_key(
    provider: serde_json::Value,
    api_key: String,
) -> Result<serde_json::Value, String> {
    let p: crate::ai::Provider = serde_json::from_value(provider).map_err(|e| e.to_string())?;
    crate::ai::save_provider(p, &api_key)
}

#[tauri::command]
pub fn ai_delete_provider(id: String) -> Result<serde_json::Value, String> {
    crate::ai::delete_provider(&id)
}

#[tauri::command]
pub fn ai_set_active(id: String) -> Result<serde_json::Value, String> {
    crate::ai::set_active(&id)
}

#[tauri::command]
pub async fn ai_test(id: String) -> Result<String, String> {
    let id2 = id.clone();
    tauri::async_runtime::spawn_blocking(move || crate::ai::test_provider(&id2))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub async fn ai_generate_svg(description: String) -> Result<String, String> {
    let d = description.clone();
    tauri::async_runtime::spawn_blocking(move || crate::ai::generate_svg_component(&d))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub async fn ai_generate_theme(description: String) -> Result<serde_json::Value, String> {
    let d = description.clone();
    tauri::async_runtime::spawn_blocking(move || crate::ai::generate_theme(&d))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub async fn ai_complete(
    system: String,
    user: String,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let mt = max_tokens.unwrap_or(4096);
    tauri::async_runtime::spawn_blocking(move || crate::ai::complete(&system, &user, mt, 0.7))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn list_assets() -> Vec<crate::articles::AssetMeta> {
    crate::articles::list_assets()
}

#[tauri::command]
pub fn asset_data_uri(path: String) -> Result<String, String> {
    crate::articles::asset_data_uri(&path)
}

#[tauri::command]
pub fn asset_thumb(path: String, size: Option<u32>) -> Result<String, String> {
    crate::articles::asset_thumb(&path, size.unwrap_or(320))
}

#[tauri::command]
pub fn delete_asset(path: String) -> Result<bool, String> {
    crate::articles::delete_asset(&path)
}

#[tauri::command]
pub fn read_binary_file(path: String) -> Result<String, String> {
    use base64::Engine as _;
    let bytes = std::fs::read(&path).map_err(|e| format!("read failed: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn extract_document_text(path: String) -> Result<crate::extract::ExtractedDoc, String> {
    crate::extract::extract(&path)
}

#[tauri::command]
pub fn comfy_status() -> serde_json::Value {
    crate::comfy::status()
}

// ------------------------------------------------------ wechat mp binding ---
#[tauri::command]
pub fn wx_bind_status() -> serde_json::Value {
    let creds = wxwright_mp::load_credentials();
    match creds {
        Some(c) => serde_json::json!({ "bound": true, "appid": wxwright_mp::mask(&c.appid) }),
        None => serde_json::json!({ "bound": false, "appid": "" }),
    }
}

#[tauri::command]
pub fn wx_bind(appid: String, secret: String) -> Result<serde_json::Value, String> {
    if appid.trim().is_empty() || secret.trim().is_empty() {
        return Err("AppID 和 AppSecret 不能为空".into());
    }
    let creds = wxwright_mp::Credentials {
        appid: appid.trim().to_string(),
        secret: secret.trim().to_string(),
    };
    // keyring first; file fallback stays in %APPDATA% (outside any repo)
    wxwright_mp::save_credentials(&creds, true).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "bound": true, "appid": wxwright_mp::mask(&creds.appid) }))
}

#[tauri::command]
pub fn wx_unbind() -> Result<bool, String> {
    wxwright_mp::clear_credentials().map(|_| true).map_err(|e| e.to_string())
}

// ---------------------------------------------------- AI generation jobs ---
#[tauri::command]
pub async fn ai_job_start(
    kind: String,
    params: serde_json::Value,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    // registration is cheap but may fail fast (no provider): do it off-thread
    let k = kind.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::jobs::start_job(&k, params, app)
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn ai_job_stop(id: u64) -> bool {
    crate::jobs::stop(id)
}

#[tauri::command]
pub fn list_platforms() -> serde_json::Value {
    serde_json::to_value(wxwright_core::platform::list_platforms())
        .unwrap_or(serde_json::json!([]))
}

#[tauri::command]
pub fn comfy_save_config(
    url: String,
    model: String,
    launch_path: Option<String>,
) -> Result<serde_json::Value, String> {
    crate::comfy::save_config(&url, &model, launch_path.as_deref().unwrap_or(""))
}

#[tauri::command]
pub async fn comfy_launch(launch_path: Option<String>) -> Result<serde_json::Value, String> {
    let lp = launch_path.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || crate::comfy::launch(&lp))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub async fn ai_image(prompt: String, width: u32, height: u32) -> Result<Vec<String>, String> {
    let pr = prompt.clone();
    tauri::async_runtime::spawn_blocking(move || crate::ai::generate_image(&pr, width, height))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn ai_save_image_model(model: String) -> Result<(), String> {
    crate::ai::set_image_model(&model)
}

#[tauri::command]
pub async fn comfy_txt2img(
    prompt: String,
    negative: Option<String>,
    width: u32,
    height: u32,
    steps: Option<u32>,
) -> Result<Vec<String>, String> {
    let neg = negative.unwrap_or_default();
    let steps_n = steps.unwrap_or(20);
    tauri::async_runtime::spawn_blocking(move || {
        crate::comfy::txt2img(&prompt, &neg, width, height, steps_n, None)
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub async fn comfy_img2img(
    source_path: String,
    prompt: String,
    negative: Option<String>,
    denoise: Option<f64>,
    steps: Option<u32>,
) -> Result<Vec<String>, String> {
    let neg = negative.unwrap_or_default();
    let dn = denoise.unwrap_or(0.5);
    let steps_n = steps.unwrap_or(20);
    tauri::async_runtime::spawn_blocking(move || {
        crate::comfy::img2img(&source_path, &prompt, &neg, dn, steps_n, None)
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn write_file_base64(path: String, base64_data: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data.trim())
        .map_err(|e| format!("invalid base64: {}", e))?;
    std::fs::write(&path, bytes).map_err(|e| format!("write failed: {}", e))?;
    Ok(path)
}

#[tauri::command]
pub fn import_image_from_path(path: String) -> Result<String, String> {
    crate::articles::import_from_path(&std::path::PathBuf::from(&path))
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn import_image_bytes(filename: String, base64_data: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data.trim())
        .map_err(|e| format!("invalid base64: {}", e))?;
    crate::articles::import_from_bytes(&filename, &bytes).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn copy_text_plain(text: String) -> Result<(), String> {
    wxwright_core::clipboard::copy_text(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_stop() {
    crate::ai::stop();
}

#[tauri::command]
pub async fn ai_chat(
    app: tauri::AppHandle,
    messages: serde_json::Value,
    temperature: Option<f64>,
) -> Result<(), String> {
    let temp = temperature.unwrap_or(0.7);
    tauri::async_runtime::spawn_blocking(move || crate::ai::chat(app, messages, temp))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {}", e))
}

const SAMPLE: &str = r#"# 欢迎使用 wxwright

在左侧粘贴或编写 **Markdown**，右侧即刻呈现公众号规范排版。支持 *斜体*、~~删除线~~、`行内代码` 与[官方文档](https://mp.weixin.qq.com)链接。

## 组件示例

> [!NOTE]
> 这是一个提示卡片，用 GitHub 风格的 alert 语法触发。

> [!KEYPOINT] 这是划重点卡片，一句话结论最醒目。

> 普通引用呈现为浅灰卡片。

任务清单：

- [x] 编辑 Markdown
- [x] 实时预览
- [ ] 点击「复制富文本」粘贴到公众号编辑器

| 功能 | 快捷入口 |
|:-----|:---------|
| 主题 | 顶部下拉切换 |
| 深色预览 | 月亮图标 |
| 导出 HTML | 下载图标 |

```rust
fn main() {
    println!("hello 公众号");
}
```

> [!WARNING]
> 剪贴板粘贴要求图片为 mmbiz 或 https 直链；本地图片请用 CLI `wxwright login` 解锁上传。

拖拽一个 .md 文件到窗口即可直接载入。
"#;

#[tauri::command]
pub fn load_sample() -> String {
    SAMPLE.to_string()
}
