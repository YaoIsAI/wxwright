//! ComfyUI integration: detect a locally running ComfyUI (default
//! 127.0.0.1:8188) and drive its API for text-to-image / image-to-image.
//! Generated images land in the managed assets library (articles::assets).

use std::io::Read;
use std::time::Duration;

use serde_json::{json, Value};

use crate::articles;

pub const DEFAULT_URL: &str = "http://127.0.0.1:8188";
pub const DEFAULT_MODEL: &str = "v1-5-pruned-emaonly.safetensors";
pub const DEFAULT_NEGATIVE: &str =
    "lowres, bad anatomy, bad hands, watermark, text, jpeg artifacts, blurry";

pub fn comfy_settings_full() -> (String, String, String) {
    let path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("wxwright")
        .join("settings.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            let g = |pointer: &str, dflt: &str| {
                v.pointer(pointer)
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(dflt)
                    .to_string()
            };
            return (
                g("/comfy/url", DEFAULT_URL),
                g("/comfy/model", DEFAULT_MODEL),
                g("/comfy/launch_path", ""),
            );
        }
    }
    (
        DEFAULT_URL.to_string(),
        DEFAULT_MODEL.to_string(),
        String::new(),
    )
}

/// One-click start: spawn the user's local ComfyUI launcher (bat/cmd/exe/py),
/// then poll until the HTTP endpoint answers (max 60s).
pub fn launch(launch_path: &str) -> Result<Value, String> {
    let (url, _model, stored) = comfy_settings_full();
    let path = if launch_path.trim().is_empty() {
        stored.as_str()
    } else {
        launch_path.trim()
    };
    if path.is_empty() {
        return Err("请先在设置中填写 ComfyUI 启动程序路径（如 run_nvidia_gpu.bat）".into());
    }
    if !std::path::Path::new(path).exists() {
        return Err(format!("启动程序不存在: {}", path));
    }
    if probe(url.trim_end_matches('/')) {
        return Ok(status());
    }
    let p = std::path::PathBuf::from(path);
    let dir = p.parent().map(|d| d.to_path_buf());
    let lower = path.to_ascii_lowercase();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if lower.ends_with(".bat") || lower.ends_with(".cmd") {
            std::process::Command::new("cmd")
                .args(["/C", "start", "/MIN", path])
                .current_dir(dir.unwrap_or_else(|| std::path::PathBuf::from(".")))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("启动失败: {}", e))?;
        } else if lower.ends_with(".py") {
            std::process::Command::new("python")
                .args([path, "--port", "8188"])
                .current_dir(dir.unwrap_or_else(|| std::path::PathBuf::from(".")))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("启动失败（需要 python 在 PATH）: {}", e))?;
        } else {
            std::process::Command::new(path)
                .current_dir(dir.unwrap_or_else(|| std::path::PathBuf::from(".")))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("启动失败: {}", e))?;
        }
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(path)
            .current_dir(dir.unwrap_or_else(|| std::path::PathBuf::from(".")))
            .spawn()
            .map_err(|e| format!("启动失败: {}", e))?;
    }
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(1000));
        if probe(url.trim_end_matches('/')) {
            return Ok(status());
        }
    }
    Err(
        "ComfyUI 进程已拉起，但 60 秒内未响应（首次启动加载模型可能较慢，请稍后重试或检查端口）"
            .into(),
    )
}

fn comfy_settings() -> (String, String) {
    let path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("wxwright")
        .join("settings.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            let url = v
                .pointer("/comfy/url")
                .and_then(|x| x.as_str())
                .unwrap_or(DEFAULT_URL)
                .to_string();
            let model = v
                .pointer("/comfy/model")
                .and_then(|x| x.as_str())
                .unwrap_or(DEFAULT_MODEL)
                .to_string();
            return (url.trim_end_matches('/').to_string(), model);
        }
    }
    (DEFAULT_URL.to_string(), DEFAULT_MODEL.to_string())
}

pub fn save_config(url: &str, model: &str, launch_path: &str) -> Result<Value, String> {
    let path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("wxwright")
        .join("settings.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut root: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    root["comfy"] = json!({
        "url": url.trim().trim_end_matches('/'),
        "model": model.trim(),
        "launch_path": launch_path.trim(),
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(status())
}

pub fn status() -> Value {
    let (url, model, launch_path) = comfy_settings_full();
    json!({
        "url": url,
        "model": model,
        "launch_path": launch_path,
        "online": probe(&url),
    })
}

/// Any HTTP response (including 4xx) proves the ComfyUI HTTP path works.
pub fn probe(url: &str) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .build();
    match agent
        .get(&format!("{}/system_stats", url.trim_end_matches('/')))
        .call()
    {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

fn agent_long() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(600))
        .build()
}

fn txt2img_workflow(
    model: &str,
    prompt: &str,
    negative: &str,
    w: u32,
    h: u32,
    steps: u32,
    seed: u64,
) -> Value {
    json!({
        "3": { "class_type": "KSampler", "inputs": {
            "seed": seed, "steps": steps, "cfg": 7.0,
            "sampler_name": "euler", "scheduler": "normal", "denoise": 1.0,
            "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0] } },
        "4": { "class_type": "CheckpointLoaderSimple", "inputs": { "ckpt_name": model } },
        "5": { "class_type": "EmptyLatentImage", "inputs": { "width": w, "height": h, "batch_size": 1 } },
        "6": { "class_type": "CLIPTextEncode", "inputs": { "text": prompt, "clip": ["4", 1] } },
        "7": { "class_type": "CLIPTextEncode", "inputs": { "text": negative, "clip": ["4", 1] } },
        "8": { "class_type": "VAEDecode", "inputs": { "samples": ["3", 0], "vae": ["4", 2] } },
        "9": { "class_type": "SaveImage", "inputs": { "filename_prefix": "wxwright/t2i", "images": ["8", 0] } }
    })
}

fn img2img_workflow(
    model: &str,
    image_name: &str,
    prompt: &str,
    negative: &str,
    steps: u32,
    denoise: f64,
    seed: u64,
) -> Value {
    json!({
        "3": { "class_type": "KSampler", "inputs": {
            "seed": seed, "steps": steps, "cfg": 7.0,
            "sampler_name": "euler", "scheduler": "normal", "denoise": denoise,
            "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["10", 0] } },
        "4": { "class_type": "CheckpointLoaderSimple", "inputs": { "ckpt_name": model } },
        "6": { "class_type": "CLIPTextEncode", "inputs": { "text": prompt, "clip": ["4", 1] } },
        "7": { "class_type": "CLIPTextEncode", "inputs": { "text": negative, "clip": ["4", 1] } },
        "10": { "class_type": "VAEEncode", "inputs": { "pixels": ["11", 0], "vae": ["4", 2] } },
        "11": { "class_type": "LoadImage", "inputs": { "image": image_name } },
        "8": { "class_type": "VAEDecode", "inputs": { "samples": ["3", 0], "vae": ["4", 2] } },
        "9": { "class_type": "SaveImage", "inputs": { "filename_prefix": "wxwright/i2i", "images": ["8", 0] } }
    })
}

/// Queue a workflow and wait for its output images. Returns raw PNG/JPG bytes.
fn run_workflow(url: &str, workflow: Value) -> Result<Vec<Vec<u8>>, String> {
    let url = url.trim_end_matches('/');
    let agent = agent_long();
    let resp = agent
        .post(&format!("{}/prompt", url))
        .send_json(json!({ "prompt": workflow }))
        .map_err(|e| format!("提交队列失败: {}（请确认 ComfyUI 已启动）", e))?;
    let v: Value = resp
        .into_json()
        .map_err(|e| format!("响应解析失败: {}", e))?;
    if let Some(err) = v.get("error") {
        return Err(format!("ComfyUI 拒绝了工作流: {}", err));
    }
    let prompt_id = v
        .get("prompt_id")
        .and_then(|x| x.as_str())
        .ok_or("missing prompt_id")?
        .to_string();

    // Poll history until the prompt completes (max 10 min).
    for _ in 0..600 {
        std::thread::sleep(Duration::from_millis(1000));
        let hist_raw = agent
            .get(&format!("{}/history/{}", url, prompt_id))
            .call()
            .map_err(|e| format!("查询进度失败: {}", e))?;
        let hist: Value = hist_raw.into_json().map_err(|e| e.to_string())?;
        let entry = hist.get(&prompt_id);
        if let Some(entry) = entry {
            let status = entry.pointer("/status/status_str").and_then(|x| x.as_str());
            match status {
                Some("error") => return Err("ComfyUI 执行出错（查看 ComfyUI 控制台日志）".into()),
                Some("success") | None => {
                    let outputs = entry.get("outputs").cloned().unwrap_or(json!({}));
                    let mut images = Vec::new();
                    if let Some(obj) = outputs.as_object() {
                        for (_node, out) in obj {
                            if let Some(imgs) = out.get("images").and_then(|x| x.as_array()) {
                                for img in imgs {
                                    let filename =
                                        img.get("filename").and_then(|x| x.as_str()).unwrap_or("");
                                    let subfolder =
                                        img.get("subfolder").and_then(|x| x.as_str()).unwrap_or("");
                                    let r#type = img
                                        .get("type")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("output");
                                    if filename.is_empty() {
                                        continue;
                                    }
                                    let view = agent
                                        .get(&format!(
                                            "{}/view?filename={}&subfolder={}&type={}",
                                            url,
                                            urlencode(filename),
                                            urlencode(subfolder),
                                            urlencode(r#type)
                                        ))
                                        .call()
                                        .map_err(|e| format!("下载结果失败: {}", e))?;
                                    let mut buf = Vec::new();
                                    view.into_reader()
                                        .take(64 * 1024 * 1024)
                                        .read_to_end(&mut buf)
                                        .map_err(|e| e.to_string())?;
                                    images.push(buf);
                                }
                            }
                        }
                    }
                    if !images.is_empty() {
                        return Ok(images);
                    }
                    // success but no images yet: keep polling briefly
                }
                _ => {}
            }
        }
    }
    Err("等待 ComfyUI 生成超时（10 分钟）".into())
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

/// Text-to-image: queue, wait, save outputs into the assets library.
/// Returns asset file paths.
pub fn txt2img(
    prompt: &str,
    negative: &str,
    w: u32,
    h: u32,
    steps: u32,
) -> Result<Vec<String>, String> {
    let (url, model) = comfy_settings();
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(42);
    let neg = if negative.trim().is_empty() {
        DEFAULT_NEGATIVE
    } else {
        negative
    };
    // SD latent spaces need multiples of 8.
    let w = (w.max(64) / 8) * 8;
    let h = (h.max(64) / 8) * 8;
    let images = run_workflow(
        &url,
        txt2img_workflow(&model, prompt, neg, w, h, steps, seed),
    )?;
    let mut paths = Vec::new();
    for (i, bytes) in images.iter().enumerate() {
        let p = articles::import_from_bytes(&format!("t2i-{}-{}.png", seed, i), bytes)?;
        paths.push(p.to_string_lossy().to_string());
    }
    Ok(paths)
}

/// Image-to-image: upload a source image, queue, save outputs.
pub fn img2img(
    source_path: &str,
    prompt: &str,
    negative: &str,
    denoise: f64,
    steps: u32,
) -> Result<Vec<String>, String> {
    let (url, model) = comfy_settings();
    let bytes = std::fs::read(source_path).map_err(|e| format!("read failed: {}", e))?;
    let filename = source_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("image.png")
        .to_string();
    let server_name = upload_image(&url, &bytes, &filename)?;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(42);
    let neg = if negative.trim().is_empty() {
        DEFAULT_NEGATIVE
    } else {
        negative
    };
    let images = run_workflow(
        &url,
        img2img_workflow(&model, &server_name, prompt, neg, steps, denoise, seed),
    )?;
    let mut paths = Vec::new();
    for (i, b) in images.iter().enumerate() {
        let p = articles::import_from_bytes(&format!("i2i-{}-{}.png", seed, i), b)?;
        paths.push(p.to_string_lossy().to_string());
    }
    Ok(paths)
}

fn upload_image(url: &str, bytes: &[u8], filename: &str) -> Result<String, String> {
    let boundary = "wxwrightcomfyboundary";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"image\"; filename=\"{}\"\r\nContent-Type: image/png\r\n\r\n",
            filename.replace('"', "")
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
    let resp = agent_long()
        .post(&format!("{}/upload/image", url.trim_end_matches('/')))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={}", boundary),
        )
        .send_bytes(&body)
        .map_err(|e| format!("上传失败: {}", e))?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    Ok(v.get("name")
        .and_then(|x| x.as_str())
        .unwrap_or(filename)
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_basic() {
        assert_eq!(urlencode("a b/c.png"), "a%20b%2Fc.png");
        assert_eq!(urlencode("ok.png"), "ok.png");
    }

    #[test]
    fn workflow_has_required_nodes() {
        let wf = txt2img_workflow("m.safetensors", "cat", "bad", 512, 512, 20, 1);
        for node in ["3", "4", "5", "6", "7", "8", "9"] {
            assert!(wf.get(node).is_some(), "missing node {}", node);
        }
        let wf2 = img2img_workflow("m.safetensors", "in.png", "cat", "bad", 20, 0.5, 1);
        assert!(wf2.get("10").is_some());
        assert!(wf2.get("11").is_some());
    }
}
