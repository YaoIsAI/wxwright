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
    let doc = wrap_document(&out.html, opts.theme.canvas());
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
    crate::articles::save_article(
        id,
        &title,
        &theme,
        platform.as_deref().unwrap_or("wechat"),
        &markdown,
    )
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
            let resolved: Vec<String> = out.images.iter().map(|i| i.final_src.clone()).collect();
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
            let violations =
                wxwright_core::platform::validate_platform_caption(&platform, &title, &cap, images);
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
    wxwright_mp::clear_credentials()
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// Push the current article to the MP draft box: dialect-render with the
/// article's theme, upload local images to mmbiz (Upload mode), gate on
/// blocking violations, then draft_add. The GUI's missing half of the
/// 「推送草稿」 promise - the same path `wxwright draft create` walks.
#[tauri::command]
pub async fn wx_push_draft(
    title: String,
    markdown: String,
    theme_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let creds = wxwright_mp::load_credentials()
            .ok_or("公众号未绑定：请先在 设置 → 公众号 API 完成绑定")?;
        let client = wxwright_mp::MpClient::new(creds.clone());
        let md = wxwright_core::util::strip_frontmatter(&markdown).1;
        let mut opts = ConvertOptions::new(load_theme_or_default(&theme_id));
        opts.image_mode = ImageMode::Upload;
        opts.transport = Some(std::sync::Arc::new(wxwright_mp::MpClient::new(creds)));
        let result = wxwright_core::pipeline(&md, &opts).map_err(|e| e.to_string())?;
        let blocks = result.blocking_violations();
        if !blocks.is_empty() {
            let list = blocks
                .iter()
                .map(|v| format!("{} {}", v.rule_id, v.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "存在 {} 个阻断级违规，草稿未推送：{}",
                blocks.len(),
                list
            ));
        }
        let thumb = result
            .images
            .iter()
            .find_map(|i| i.media_id.clone())
            .ok_or("公众号草稿 API 要求封面图：文章需要至少一张图片（本地图片会自动上传转存）")?;
        let article = wxwright_mp::DraftArticle {
            title,
            author: String::new(),
            digest: String::new(),
            content_html: result.html.clone(),
            content_source_url: String::new(),
            thumb_media_id: thumb,
        };
        let media_id = client.draft_add(&article).map_err(|e| e.to_string())?;
        let warns = result.violations.iter().filter(|v| !v.is_block()).count();
        Ok(serde_json::json!({ "draft_media_id": media_id, "warnings": warns }))
    })
    .await
    .map_err(|e| format!("task join failed: {}", e))?
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
    tauri::async_runtime::spawn_blocking(move || crate::jobs::start_job(&k, params, app))
        .await
        .map_err(|e| format!("task join failed: {}", e))?
}

#[tauri::command]
pub fn ai_job_stop(id: u64) -> bool {
    crate::jobs::stop(id)
}

#[tauri::command]
pub fn list_platforms() -> serde_json::Value {
    serde_json::to_value(wxwright_core::platform::list_platforms()).unwrap_or(serde_json::json!([]))
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

const SAMPLE: &str = r#"# wxwright 使用手册

一份 Markdown，写一次，全网发。wxwright 把公众号排版、多平台适配、AI 创作、图片生产全部收进一个桌面应用：左边写作，中间预览，右边就是你文章在真实手机上的样子。

## 一、三分钟上手

1. 左侧文章库点「新建文章」，或直接把 .md 文件拖进窗口。
2. 在中间编辑器写 Markdown，右侧真机预览实时刷新。
3. 点「校验」确认合规，点「复制富文本」，到公众号编辑器 Ctrl+V，排版零失真。

界面三栏：文章库、编辑器、真机预览（iPhone 15 Pro / Pixel 8 可切换，深浅色随你）。底部状态栏有字数统计、合规徽标，以及一只叫墨仔的猫——写作时它会陪你，记得摸摸它。

## 二、文章库

- 新建、导入（多选 .md）、搜索、重命名、删除都在侧栏完成。
- 文章头部 frontmatter 记录 title、theme、platform，切平台状态随文章保存。
- Ctrl+S 随时保存；底部「校验」按钮打开违规面板，每条违规都给出规则号与修法。

## 三、平台与渠道三件套

顶栏平台下拉可切换 7 个平台：微信公众号、小红书、知乎、Facebook、Instagram、X、LinkedIn。

切换的不只是预览外壳，而是「渠道三件套」整体联动：

- 预览形态：微信方言长文 / 小红书笔记详情 / 知乎文章页 / Facebook 卡片 / Instagram 帖子 / X 帖子 / LinkedIn 卡片，全部按真实产品的字号与配色 1:1 还原。
- 规则表：微信 R-1 至 R-4 官方规范、小红书 XHS 规则、知乎宽松模式，校验结果随平台切换。
- 导出物：「复制富文本」按钮按平台自动变形——微信出方言富文本，小红书出笔记文案，知乎出 Markdown 原文；小红书还支持一键导出 3:4 图组。

## 四、主题系统

内置三套主题：素黑、科技蓝、杂志。下拉即切，只影响微信公众号方言排版。

想要独特的？点主题旁的魔法棒，用一句话描述风格（例如「奶茶铺配色，奶咖色底，焦糖强调色」），AI 生成的新主题会自动通过公众号合规校验，CLI 也能复用。

## 五、AI 助手

顶栏「AI 助手」打开对话抽屉：

- 快捷指令：润色当前文章、续写、起 5 个标题、列提纲、生成头图文案。
- 附件能力：PDF / DOCX / HTML / 文本自动提取要点，图片走视觉模型。
- 对话里的「插入」「替换文章」「复制」按钮把生成结果直接落进正文。
- Enter 发送，Shift+Enter 换行；生成中按钮变成停止，随时中断。

AI 也能生成文章草稿：描述主题与结构，生成后一键替换正文，再人工润色，是效率最高的工作流。

## 六、四大工坊

- 海报工坊：HTML 生成 PNG，完全本地光栅化。头图 / 金句卡 / 图文卡三种模板，可用 AI 生成，可导出图组，产物自动进素材库。
- 尺寸工坊：任意图片适配平台标准尺寸（头图 2.35:1、方图、3:4 等），居中裁切或补白，支持 2x 导出。
- SVG 组件库：6 个公众号互动组件（点击闪烁、金句渐显等），参数可视化编辑，也可 AI 生成全新组件。插入时自动放在文末独立块并通过合规复检。
- AI 绘图：ComfyUI 本地出图（完全离线）与云端图像 API 双源，文生图与图生图都支持，产物直进素材库。

## 七、发布绑定（海外平台）

工具不提供云服务，海外平台发布使用你自己的开发者应用（BYO）：

- X 与 LinkedIn 已支持一键登录：填入自己应用的 Client ID（LinkedIn 另需 Secret），点「一键登录」，系统浏览器完成授权，令牌只存本机钥匙串。
- Facebook 与 Instagram 的凭据接口已就绪，登录流开放状态见引导页说明（Instagram 要求专业账户与公网图片地址）。
- 每个平台的逐步申请教程都在顶栏问号的「配置引导」里，含官方入口直达。

## 八、Agent 与自动化

- Agent 接入面板：一键把 MCP 配置写入 Claude Desktop、Cursor、VS Code、OpenCode；重启客户端即可看到 7 个 wxwright 工具（转换、校验、复制、主题、传图、草稿、列表）。
- Agent 接手卡：整段复制进任意 AI Agent 的系统提示，Agent 立刻接手。
- CLI 速查：`wxwright convert` 转换、`wxwright validate --strict` 校验（退出码 0/1/2）、`wxwright copy` 全链路到剪贴板、`wxwright doctor` 环境体检、`wxwright mcp serve` 手动启动 MCP。

## 九、设置与密钥安全

- AI Providers：支持 OpenAI / DeepSeek / 通义 / Kimi / 智谱等云服务，以及本地 Ollama 与 LM Studio；顶部厂商预设一键填充。
- 公众号 API 绑定：AppID 与 AppSecret 存入后可直接推送文章到公众号草稿箱。
- 所有密钥只进系统钥匙串（失败时回退本机用户目录），不进仓库、不进日志、不进导出文件。

## 十、常见问题

- 浏览器演示模式（无后端）里 AI 与文件功能提示不可用是预期行为，桌面端功能完整。
- 剪贴板粘贴要求图片为 mmbiz 或 https 直链；本地图片建议走引擎转换自动内联。
- 深色预览是近似效果，公众号深色模式的最终表现由微信自己的转换算法决定。
- 校验规则覆盖官方 R-1 至 R-4 全表：字体禁令、行高、固定宽度、渐变文字等都有违规样例与修复建议。

一句话总结：把排版交给 wxwright，你只管写。
"#;

#[tauri::command]
pub fn load_sample() -> String {
    SAMPLE.to_string()
}
